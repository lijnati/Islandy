/**
 * System state fed by the Rust system workers (audio, power, network,
 * radios, metrics, display).
 *
 * - Updates arrive per domain over one channel (`connectSystem`) and are
 *   applied only if newer than the revision held for that domain.
 * - Sliders (volume, brightness) are "latest wins": at most one request per
 *   control is in flight; while it is, newer values replace the queued one,
 *   and requests are spaced at least `SEND_INTERVAL_MS` apart. The slider
 *   shows the user's value (an *intent*) until Windows confirms a value after
 *   the last request, so an update from an older request can't make the
 *   thumb jump back. A failed request drops the intent and shows Windows'
 *   real value again.
 * - Toggles (mute, radios) are never optimistic: they show "busy" until the
 *   request settles, and the new state appears only when Windows reports it.
 * - Alerts are de-duplicated again here (a repeat within `ALERT_REPEAT_MS`
 *   is dropped) and handed to listeners (the island bridge turns them into
 *   peeks).
 * - Independent of the island's expansion state.
 */
import { createStore, useStore } from "zustand";

import * as native from "../../services/native";
import { NativeError } from "../../services/native";
import type {
  RadioKind,
  SystemAlert,
  SystemFocus,
  SystemSnapshot,
  SystemUpdate,
} from "../../services/types";
import {
  alertKey,
  applyUpdate,
  LOADING_SNAPSHOT,
  revisionsOf,
  SYSTEM_DOMAINS,
  systemErrorMessage,
  type DomainRevisions,
} from "./system-model";

export type SystemConnection = "idle" | "connecting" | "connected" | "unavailable";

/** A slider control: the master volume or one display's brightness. */
export type LevelKey = "volume" | `brightness:${string}`;

export interface LevelIntent {
  value: number;
  /** Set once the last request settled: the domain revision at that time. */
  settledAt: number | null;
}

export interface SystemNotice {
  id: number;
  message: string;
}

export interface SystemStoreState {
  connection: SystemConnection;
  snapshot: SystemSnapshot | null;
  revisions: DomainRevisions | null;
  intents: Readonly<Partial<Record<LevelKey, LevelIntent>>>;
  /** In-flight toggles: "mute", "radio:wifi", "radio:bluetooth". */
  pending: Readonly<Record<string, true>>;
  notice: SystemNotice | null;
  focus: SystemFocus;
}

export interface SystemStoreActions {
  start: () => void;
  /** Returns false when the update was stale or an alert. */
  applyUpdate: (update: SystemUpdate) => boolean;
  setVolume: (volume: number) => void;
  setBrightness: (displayId: string, brightness: number) => void;
  toggleMute: () => Promise<void>;
  setRadio: (radio: RadioKind, on: boolean) => Promise<void>;
  setFocus: (focus: SystemFocus) => void;
  /** Subscribes to de-duplicated alerts; returns an unsubscribe function. */
  onAlert: (listener: (alert: SystemAlert) => void) => () => void;
}

export type SystemStore = SystemStoreState & SystemStoreActions;

export interface SystemDeps {
  available: () => boolean;
  connectSystem: typeof native.connectSystem;
  setSystemFocus: typeof native.setSystemFocus;
  setMasterVolume: typeof native.setMasterVolume;
  setAudioMuted: typeof native.setAudioMuted;
  setRadioState: typeof native.setRadioState;
  setDisplayBrightness: typeof native.setDisplayBrightness;
  now: () => number;
}

const defaultDeps: SystemDeps = {
  available: native.isNativeAvailable,
  connectSystem: native.connectSystem,
  setSystemFocus: native.setSystemFocus,
  setMasterVolume: native.setMasterVolume,
  setAudioMuted: native.setAudioMuted,
  setRadioState: native.setRadioState,
  setDisplayBrightness: native.setDisplayBrightness,
  now: Date.now,
};

/** Minimum spacing between two requests for the same slider. */
export const SEND_INTERVAL_MS = 50;
/** An intent is dropped this long after its last request settled at the latest. */
export const INTENT_GRACE_MS = 1500;
/** The same alert again within this window is not shown twice. */
export const ALERT_REPEAT_MS = 60_000;
const NOTICE_MS = 3500;

export const initialSystemState: SystemStoreState = {
  connection: "idle",
  snapshot: null,
  revisions: null,
  intents: {},
  pending: {},
  notice: null,
  focus: "none",
};

function without<T>(record: Readonly<Record<string, T>>, key: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([k]) => k !== key));
}

function errorKind(error: unknown): NativeError["kind"] {
  return error instanceof NativeError ? error.kind : "unknown";
}

const domainOf = (key: LevelKey) => (key === "volume" ? "audio" : "display");

/** The value Windows currently reports for a slider. */
function nativeLevel(snapshot: SystemSnapshot | null, key: LevelKey): number | null {
  if (!snapshot) return null;
  if (key === "volume") return snapshot.audio.output?.volume ?? null;
  const id = key.slice("brightness:".length);
  return snapshot.display.displays.find((d) => d.id === id)?.brightness ?? null;
}

export function createSystemStore(overrides: Partial<SystemDeps> = {}) {
  const deps: SystemDeps = { ...defaultDeps, ...overrides };
  const listeners = new Set<(alert: SystemAlert) => void>();
  const lastAlert = new Map<string, number>();
  /** Per slider: a request is in flight / the next value to send. */
  const inFlight = new Set<LevelKey>();
  const queued = new Map<LevelKey, number>();
  const timers = new Set<ReturnType<typeof setTimeout>>();
  let noticeSeq = 0;
  let disposed = false;

  const later = (ms: number, f: () => void) => {
    const t = setTimeout(() => {
      timers.delete(t);
      if (!disposed) f();
    }, ms);
    timers.add(t);
  };

  const store = createStore<SystemStore>()((set, get) => {
    const notify = (message: string) => {
      noticeSeq += 1;
      const id = noticeSeq;
      set({ notice: { id, message } });
      later(NOTICE_MS, () => {
        if (get().notice?.id === id) set({ notice: null });
      });
    };

    const dropIntent = (key: LevelKey) => {
      if (get().intents[key] === undefined) return;
      set({ intents: without(get().intents, key) });
    };

    const send = (key: LevelKey, value: number) => {
      inFlight.add(key);
      const startedAt = deps.now();
      const request =
        key === "volume"
          ? deps.setMasterVolume(value)
          : deps.setDisplayBrightness(key.slice("brightness:".length), value);
      const finish = (error: unknown) => {
        const next = queued.get(key);
        queued.delete(key);
        if (next !== undefined) {
          // A newer value is waiting: send it (spaced), ignore this outcome.
          later(Math.max(0, SEND_INTERVAL_MS - (deps.now() - startedAt)), () => {
            send(key, next);
          });
          return;
        }
        inFlight.delete(key);
        if (error !== undefined) {
          console.warn(`[islandy] ${key} change failed:`, error);
          dropIntent(key);
          notify(systemErrorMessage(errorKind(error)));
          return;
        }
        const { snapshot, revisions, intents } = get();
        const intent = intents[key];
        if (!intent) return;
        if (nativeLevel(snapshot, key) === intent.value) {
          dropIntent(key);
          return;
        }
        const settledAt = revisions?.[domainOf(key)] ?? 0;
        set({ intents: { ...intents, [key]: { ...intent, settledAt } } });
        later(INTENT_GRACE_MS, () => {
          const current = get().intents[key];
          if (current?.settledAt === settledAt && !inFlight.has(key)) dropIntent(key);
        });
      };
      request.then(
        () => {
          finish(undefined);
        },
        (error: unknown) => {
          finish(error ?? new Error("failed"));
        },
      );
    };

    const setLevel = (key: LevelKey, raw: number) => {
      if (!Number.isFinite(raw)) return;
      const value = Math.round(Math.min(100, Math.max(0, raw)));
      set({ intents: { ...get().intents, [key]: { value, settledAt: null } } });
      if (inFlight.has(key)) {
        queued.set(key, value);
        return;
      }
      send(key, value);
    };

    return {
      ...initialSystemState,

      start: () => {
        if (get().connection !== "idle") return;
        if (!deps.available()) {
          set({ connection: "unavailable" });
          return;
        }
        set({ connection: "connecting" });
        deps
          .connectSystem((update) => {
            get().applyUpdate(update);
          })
          .then(
            (snapshot) => {
              // Updates pushed after the channel was registered can arrive
              // before this response; they are newer than the snapshot for
              // their domain, so they are kept.
              const { snapshot: held, revisions: heldRevisions } = get();
              let merged = snapshot;
              let revisions = revisionsOf(snapshot);
              if (held && heldRevisions) {
                for (const domain of SYSTEM_DOMAINS) {
                  if (heldRevisions[domain] > snapshot.revision) {
                    merged = { ...merged, [domain]: held[domain] };
                    revisions = { ...revisions, [domain]: heldRevisions[domain] };
                  }
                }
                merged = { ...merged, revision: Math.max(merged.revision, held.revision) };
              }
              set({ connection: "connected", snapshot: merged, revisions });
              const focus = get().focus;
              if (focus !== "none") {
                deps.setSystemFocus(focus).catch((error: unknown) => {
                  console.warn("[islandy] set_system_focus failed:", error);
                });
              }
            },
            (error: unknown) => {
              console.warn("[islandy] connect_system failed:", error);
              set({ connection: "unavailable" });
            },
          );
      },

      applyUpdate: (update) => {
        if (update.domain === "alert") {
          const key = alertKey(update.alert);
          const now = deps.now();
          const last = lastAlert.get(key);
          if (last !== undefined && now - last < ALERT_REPEAT_MS) return false;
          lastAlert.set(key, now);
          listeners.forEach((listener) => {
            listener(update.alert);
          });
          return false;
        }
        // An update can precede the initial snapshot: start from a "loading"
        // base, merged with the snapshot when it arrives.
        const { snapshot, revisions } = get();
        const base = snapshot ?? LOADING_SNAPSHOT;
        const applied = applyUpdate(base, revisions ?? revisionsOf(base), update);
        if (!applied) return false;
        // Confirmations newer than a settled request end that slider's intent.
        let intents = get().intents;
        for (const key of Object.keys(intents) as LevelKey[]) {
          const intent = intents[key];
          if (
            intent?.settledAt != null &&
            domainOf(key) === update.domain &&
            update.revision > intent.settledAt &&
            !inFlight.has(key)
          ) {
            intents = without(intents, key);
          }
        }
        set({ snapshot: applied.snapshot, revisions: applied.revisions, intents });
        return true;
      },

      setVolume: (volume) => {
        if (!get().snapshot?.audio.output) return;
        setLevel("volume", volume);
      },

      setBrightness: (displayId, brightness) => {
        const display = get().snapshot?.display.displays.find((d) => d.id === displayId);
        if (!display) return;
        setLevel(`brightness:${displayId}`, brightness);
      },

      toggleMute: async () => {
        const output = get().snapshot?.audio.output;
        if (!output || get().pending.mute) return;
        set({ pending: { ...get().pending, mute: true } });
        try {
          await deps.setAudioMuted(!output.muted);
        } catch (error) {
          console.warn("[islandy] set_audio_muted failed:", error);
          notify(systemErrorMessage(errorKind(error)));
        } finally {
          set({ pending: without(get().pending, "mute") });
        }
      },

      setRadio: async (radio, on) => {
        const key = `radio:${radio}`;
        const info = get().snapshot?.radios[radio];
        if (!info?.canToggle || get().pending[key]) return;
        set({ pending: { ...get().pending, [key]: true } });
        try {
          await deps.setRadioState(radio, on);
        } catch (error) {
          console.warn("[islandy] set_radio_state failed:", error);
          notify(systemErrorMessage(errorKind(error)));
        } finally {
          set({ pending: without(get().pending, key) });
        }
      },

      setFocus: (focus) => {
        if (get().focus === focus) return;
        set({ focus });
        if (get().connection !== "connected") return; // sent on connect
        deps.setSystemFocus(focus).catch((error: unknown) => {
          console.warn("[islandy] set_system_focus failed:", error);
        });
      },

      onAlert: (listener) => {
        listeners.add(listener);
        return () => {
          listeners.delete(listener);
        };
      },
    };
  });

  const dispose = () => {
    disposed = true;
    timers.forEach(clearTimeout);
    timers.clear();
    listeners.clear();
  };

  return Object.assign(store, { dispose });
}

export type SystemStoreApi = ReturnType<typeof createSystemStore>;

/** The app-wide system store. */
export const systemStore = createSystemStore();

export function useSystem<T>(selector: (state: SystemStore) => T): T {
  return useStore(systemStore, selector);
}

/** The slider value to show: the user's intent, else Windows' value. */
export function useLevel(key: LevelKey): number | null {
  return useSystem((s) => s.intents[key]?.value ?? nativeLevel(s.snapshot, key));
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    systemStore.dispose();
  });
}
