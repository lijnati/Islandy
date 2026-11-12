/**
 * DEMO media state for Phase 1.
 *
 * Nothing here is connected to Windows media sessions (GSMTC) or any player.
 * The controls only change this local demo state, and the UI labels it as a
 * demo. Phase 2 replaces this module with a store fed by Rust.
 *
 * Playback position is derived from timestamps, so no timer runs to "play"
 * the demo. One timeout per track handles the end of the track while playing.
 */
import { createStore, useStore } from "zustand";

export interface Track {
  id: string;
  title: string;
  artist: string;
  album: string;
  durationMs: number;
  /** Two colors for the generated artwork. */
  artwork: readonly [string, string];
}

export const DEMO_TRACKS: readonly Track[] = [
  {
    id: "t1",
    title: "Midnight Transit",
    artist: "Lumen Fields",
    album: "Night Lines",
    durationMs: 214_000,
    artwork: ["#3a2a6b", "#d9774b"],
  },
  {
    id: "t2",
    title: "Paper Satellites",
    artist: "Oda Marlow",
    album: "Low Orbit",
    durationMs: 187_000,
    artwork: ["#1f4d4a", "#9fd3b3"],
  },
  {
    id: "t3",
    title: "Glass Harbor",
    artist: "The Quiet Hours",
    album: "Tidewater",
    durationMs: 251_000,
    artwork: ["#2b3550", "#e2b85c"],
  },
];

export interface MediaState {
  index: number;
  isPlaying: boolean;
  /** Position at `updatedAt`. */
  positionMs: number;
  updatedAt: number;
}

export interface MediaActions {
  togglePlay: () => void;
  next: () => void;
  previous: () => void;
  seek: (positionMs: number) => void;
}

/** Called when the demo advances to a new track on its own (for a peek). */
type TrackChangeListener = (track: Track) => void;

export function positionAt(state: MediaState, now: number): number {
  const track = DEMO_TRACKS[state.index] ?? DEMO_TRACKS[0];
  const elapsed = state.isPlaying ? now - state.updatedAt : 0;
  return Math.min(track?.durationMs ?? 0, Math.max(0, state.positionMs + elapsed));
}

export function createDemoMediaStore(now: () => number = Date.now) {
  let endTimer: ReturnType<typeof setTimeout> | undefined;
  const listeners = new Set<TrackChangeListener>();

  const store = createStore<MediaState & MediaActions>()((set, get) => {
    const schedule = (state: MediaState) => {
      clearTimeout(endTimer);
      endTimer = undefined;
      if (!state.isPlaying) return;
      const track = DEMO_TRACKS[state.index];
      if (!track) return;
      endTimer = setTimeout(
        () => {
          go(1, true);
        },
        track.durationMs - positionAt(state, now()),
      );
    };

    const update = (patch: Partial<MediaState>) => {
      const t = now();
      const current = get();
      const next: MediaState = {
        index: current.index,
        isPlaying: current.isPlaying,
        positionMs: positionAt(current, t),
        updatedAt: t,
        ...patch,
      };
      set(next);
      schedule(next);
    };

    const go = (delta: number, automatic = false) => {
      const count = DEMO_TRACKS.length;
      const index = (get().index + delta + count) % count;
      update({ index, positionMs: 0 });
      const track = DEMO_TRACKS[index];
      if (automatic && track)
        listeners.forEach((l) => {
          l(track);
        });
    };

    return {
      index: 0,
      isPlaying: false,
      positionMs: 72_000,
      updatedAt: now(),
      togglePlay: () => {
        update({ isPlaying: !get().isPlaying });
      },
      next: () => {
        go(1);
      },
      // Like most players: restart the track unless we're near its start.
      previous: () => {
        if (positionAt(get(), now()) > 3_000) update({ positionMs: 0 });
        else go(-1);
      },
      seek: (positionMs) => {
        update({ positionMs });
      },
    };
  });

  return Object.assign(store, {
    onAutoAdvance(listener: TrackChangeListener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    dispose() {
      clearTimeout(endTimer);
    },
  });
}

export const demoMedia = createDemoMediaStore();

export function useDemoMedia<T>(selector: (s: MediaState & MediaActions) => T): T {
  return useStore(demoMedia, selector);
}

export function currentTrack(state: MediaState): Track {
  return DEMO_TRACKS[state.index] ?? (DEMO_TRACKS[0] as Track);
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    demoMedia.dispose();
  });
}
