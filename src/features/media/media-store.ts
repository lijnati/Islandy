/**
 * Media state fed by the Rust media worker (Windows GSMTC).
 *
 * - Snapshots arrive over one channel (`connectMedia`) and are applied only
 *   if their revision is newer, so a late command response can never
 *   overwrite newer state (including a newer session selection).
 * - Commands never fake their effect: the play/pause state changes only when
 *   Windows reports it. While a command is in flight the session is marked
 *   `pending` and repeated presses are ignored.
 * - Artwork is fetched by content key, once, and only for sessions that exist.
 * - This store is independent of the island's expansion state.
 */
import { createStore, useStore } from "zustand";

import * as native from "../../services/native";
import { NativeError } from "../../services/native";
import type { MediaAction, MediaSnapshot } from "../../services/types";
import {
  canPerform,
  commandErrorMessage,
  selectedSession,
  validateSeek,
  type SeekPreview,
} from "./media-model";

export type MediaConnection = "idle" | "connecting" | "connected" | "unavailable";

export interface MediaNotice {
  id: number;
  message: string;
}

export interface MediaStoreState {
  connection: MediaConnection;
  snapshot: MediaSnapshot | null;
  /** In-flight command per session id. */
  pending: Readonly<Record<string, MediaAction | "seek">>;
  seekPreview: SeekPreview | null;
  /** A transient, neutral message about a failed command. */
  notice: MediaNotice | null;
  /** artworkKey → data URL, only for keys referenced by current sessions. */
  artwork: Readonly<Record<string, string>>;
  /** Picking a session is in flight. */
  selecting: boolean;
}

export interface MediaStoreActions {
  /** Connects to the native media service once; later calls are no-ops. */
  start: () => void;
  /** Returns false when the snapshot is stale and was ignored. */
  applySnapshot: (snapshot: MediaSnapshot) => boolean;
  command: (action: MediaAction) => Promise<void>;
  seek: (positionMs: number) => Promise<void>;
  select: (sessionId: string | null) => Promise<void>;
}

export type MediaStore = MediaStoreState & MediaStoreActions;

export interface MediaDeps {
  available: () => boolean;
  connectMedia: typeof native.connectMedia;
  mediaControl: typeof native.mediaControl;
  mediaSeek: typeof native.mediaSeek;
  selectMediaSession: typeof native.selectMediaSession;
  getMediaArtwork: typeof native.getMediaArtwork;
  now: () => number;
}

const defaultDeps: MediaDeps = {
  available: native.isNativeAvailable,
  connectMedia: native.connectMedia,
  mediaControl: native.mediaControl,
  mediaSeek: native.mediaSeek,
  selectMediaSession: native.selectMediaSession,
  getMediaArtwork: native.getMediaArtwork,
  now: Date.now,
};

const NOTICE_MS = 3500;

export const initialMediaState: MediaStoreState = {
  connection: "idle",
  snapshot: null,
  pending: {},
  seekPreview: null,
  notice: null,
  artwork: {},
  selecting: false,
};

function withoutKey<T>(record: Readonly<Record<string, T>>, key: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([k]) => k !== key));
}

function errorKind(error: unknown): NativeError["kind"] {
  return error instanceof NativeError ? error.kind : "unknown";
}

export function createMediaStore(overrides: Partial<MediaDeps> = {}) {
  const deps: MediaDeps = { ...defaultDeps, ...overrides };
  const artworkInFlight = new Set<string>();
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  let noticeSeq = 0;

  const store = createStore<MediaStore>()((set, get) => {
    const notify = (message: string) => {
      clearTimeout(noticeTimer);
      noticeSeq += 1;
      const id = noticeSeq;
      set({ notice: { id, message } });
      noticeTimer = setTimeout(() => {
        if (get().notice?.id === id) set({ notice: null });
      }, NOTICE_MS);
    };

    const fetchArtwork = (key: string) => {
      if (get().artwork[key] !== undefined || artworkInFlight.has(key)) return;
      artworkInFlight.add(key);
      deps.getMediaArtwork(key).then(
        (url) => {
          artworkInFlight.delete(key);
          // Keep it only if a session still references it (no stale artwork).
          const referenced = get().snapshot?.sessions.some((s) => s.artworkKey === key);
          if (url && referenced) set({ artwork: { ...get().artwork, [key]: url } });
        },
        (error: unknown) => {
          artworkInFlight.delete(key);
          console.warn("[islandy] get_media_artwork failed:", error);
        },
      );
    };

    const applySnapshot = (snapshot: MediaSnapshot): boolean => {
      const current = get().snapshot;
      if (current && snapshot.revision <= current.revision) return false;

      const keys = new Set(snapshot.sessions.flatMap((s) => (s.artworkKey ? [s.artworkKey] : [])));
      const artwork = Object.fromEntries(
        Object.entries(get().artwork).filter(([key]) => keys.has(key)),
      );
      const pending = Object.fromEntries(
        Object.entries(get().pending).filter(([id]) => snapshot.sessions.some((s) => s.id === id)),
      );
      // A native timeline update after the seek replaces the preview.
      let seekPreview = get().seekPreview;
      if (seekPreview) {
        const { sessionId, at } = seekPreview;
        const session = snapshot.sessions.find((s) => s.id === sessionId);
        if (!session || (session.timeline && session.timeline.updatedAtMs >= at))
          seekPreview = null;
      }
      set({ snapshot, artwork, pending, seekPreview });

      const selected = selectedSession(snapshot);
      if (selected?.artworkKey) fetchArtwork(selected.artworkKey);
      return true;
    };

    return {
      ...initialMediaState,

      start: () => {
        if (get().connection !== "idle") return;
        if (!deps.available()) {
          set({ connection: "unavailable" });
          return;
        }
        set({ connection: "connecting" });
        deps.connectMedia(applySnapshot).then(
          (snapshot) => {
            applySnapshot(snapshot);
            set({ connection: "connected" });
          },
          (error: unknown) => {
            console.warn("[islandy] connect_media failed:", error);
            set({ connection: "unavailable" });
          },
        );
      },

      applySnapshot,

      command: async (action) => {
        const session = selectedSession(get().snapshot);
        if (!session || get().pending[session.id] !== undefined) return;
        if (!canPerform(session, action)) {
          notify(commandErrorMessage("unsupported", session.sourceAppName));
          return;
        }
        const id = session.id;
        set({ pending: { ...get().pending, [id]: action } });
        try {
          applySnapshot(await deps.mediaControl(id, action));
        } catch (error) {
          console.warn(`[islandy] media_control(${action}) failed:`, error);
          notify(commandErrorMessage(errorKind(error), session.sourceAppName));
        } finally {
          set({ pending: withoutKey(get().pending, id) });
        }
      },

      seek: async (positionMs) => {
        const session = selectedSession(get().snapshot);
        if (!session || get().pending[session.id] !== undefined) return;
        const target = validateSeek(session, positionMs);
        if (target === null) return;
        const id = session.id;
        set({
          pending: { ...get().pending, [id]: "seek" },
          seekPreview: { sessionId: id, positionMs: target, at: deps.now() },
        });
        try {
          applySnapshot(await deps.mediaSeek(id, target));
        } catch (error) {
          console.warn("[islandy] media_seek failed:", error);
          if (get().seekPreview?.sessionId === id) set({ seekPreview: null });
          notify(commandErrorMessage(errorKind(error), session.sourceAppName));
        } finally {
          set({ pending: withoutKey(get().pending, id) });
        }
      },

      select: async (sessionId) => {
        set({ selecting: true });
        try {
          applySnapshot(await deps.selectMediaSession(sessionId));
        } catch (error) {
          console.warn("[islandy] select_media_session failed:", error);
          notify("Couldn't switch players");
        } finally {
          set({ selecting: false });
        }
      },
    };
  });

  return Object.assign(store, {
    dispose() {
      clearTimeout(noticeTimer);
    },
  });
}

export const mediaStore = createMediaStore();

export function useMedia<T>(selector: (state: MediaStore) => T): T {
  return useStore(mediaStore, selector);
}

/** The selected session (stable reference until a new snapshot arrives). */
export function useSelectedSession() {
  return useMedia((s) => selectedSession(s.snapshot));
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    mediaStore.dispose();
  });
}
