import { createStore, useStore } from "zustand";

import { interaction } from "../../design/tokens";

/**
 * Island interaction model. See docs/architecture.md → "Island state machine".
 *
 * - collapsed: resting pill.
 * - peek:      temporary, system-initiated contextual capsule. Expires on its own.
 * - expanded:  the dashboard. Auto-collapses after the pointer leaves, unless pinned.
 * - focused:   a widget's detail view inside the dashboard. Never auto-collapses.
 */
export type IslandMode = "collapsed" | "peek" | "expanded" | "focused";

/**
 * Views shown in `focused` mode: widget detail views and the system panels
 * opened from the quick controls (same state machine, same geometry).
 */
export type WidgetId =
  | "media"
  | "calendar"
  | "activity"
  | "claude"
  | "integrations"
  | "audio"
  | "system"
  | "network"
  | "bluetooth"
  | "display";

export type PeekIcon =
  | "battery"
  | "batteryLow"
  | "plug"
  | "wifi"
  | "network"
  | "offline"
  | "speaker"
  | "claude"
  | "build"
  | "tests"
  | "server"
  | "message";

/**
 * - `working`: amber (system/legacy "in progress")
 * - `waiting`: amber, used for "needs your attention" (Claude Code requests)
 * - `attention`: coral, problems (errors, low battery, offline)
 */
export type StatusTone = "neutral" | "working" | "waiting" | "success" | "attention";

/** A contextual event shown in `peek` mode. */
export interface PeekContent {
  source: string;
  title: string;
  detail: string;
  tone: StatusTone;
  /** Media peeks show the track's artwork instead of the glyph. */
  artworkKey?: string | null;
  /** System and activity peeks show a matching icon. */
  icon?: PeekIcon;
  /** How long this peek stays (default: the global peek dwell). */
  dwellMs?: number;
}

export type IslandEvent =
  | { type: "POINTER_ENTER" }
  | { type: "POINTER_LEAVE" }
  /** The hover-intent delay elapsed while the pointer stayed on the island. */
  | { type: "HOVER_INTENT" }
  /** The leave delay elapsed while the pointer stayed away. */
  | { type: "LEAVE_INTENT" }
  /** Click/tap on the collapsed pill or peek, or Enter/Space on it. */
  | { type: "PRESS" }
  | { type: "TOGGLE_PIN" }
  | { type: "OPEN_WIDGET"; widget: WidgetId }
  | { type: "CLOSE_WIDGET" }
  | { type: "ESCAPE" }
  /** Click outside / focus loss / an action that hands focus elsewhere. */
  | { type: "DISMISS" }
  /** The global keyboard shortcut. */
  | { type: "SHORTCUT" }
  | { type: "CONTEXT_EVENT"; peek: PeekContent }
  /** A peek's dwell time elapsed; `seq` identifies which peek. */
  | { type: "PEEK_TIMEOUT"; seq: number }
  /** A pinned dashboard was left alone for `PINNED_IDLE_MS` (auto-collapse). */
  | { type: "IDLE_TIMEOUT" };

export interface IslandState {
  mode: IslandMode;
  /** Pinned islands ignore pointer-leave. Set by clicking, cleared on collapse. */
  pinned: boolean;
  /** Whether the pointer is over the visible island surface. */
  hovering: boolean;
  /** The widget shown in `focused` mode. */
  activeWidget: WidgetId | null;
  /** Content of the current peek (only meaningful in `peek` mode). */
  peek: PeekContent | null;
  /** Increments for every new peek, so stale dwell timers can't close a newer one. */
  peekSeq: number;
}

export const initialIslandState: IslandState = {
  mode: "collapsed",
  pinned: false,
  hovering: false,
  activeWidget: null,
  peek: null,
  peekSeq: 0,
};

const COLLAPSED: Pick<IslandState, "mode" | "pinned" | "hovering" | "activeWidget" | "peek"> = {
  mode: "collapsed",
  pinned: false,
  // Hover must be re-armed by a fresh pointer-enter: otherwise a pointer
  // resting over the shrinking surface would immediately re-expand it.
  hovering: false,
  activeWidget: null,
  peek: null,
};

const isDashboard = (mode: IslandMode) => mode === "expanded" || mode === "focused";

function open(state: IslandState, pinned: boolean): IslandState {
  return { ...state, mode: "expanded", pinned, activeWidget: null, peek: null };
}

function collapse(state: IslandState): IslandState {
  const { mode, pinned, hovering, activeWidget, peek } = state;
  if (mode === "collapsed" && !pinned && !hovering && activeWidget === null && peek === null) {
    return state;
  }
  return { ...state, ...COLLAPSED };
}

/** User interaction preferences (Settings › Island). */
export interface IslandBehavior {
  /** Hovering the pill opens the dashboard (after the hover delay). */
  openOnHover: boolean;
  /** Clicking the pill opens the dashboard. */
  openOnClick: boolean;
  /** A click opens the dashboard pinned (keyboard focus, stays open). */
  pinOnClick: boolean;
  /** A pinned dashboard collapses after being left alone for `PINNED_IDLE_MS`. */
  autoCollapse: boolean;
}

export const defaultBehavior: IslandBehavior = {
  openOnHover: true,
  openOnClick: true,
  pinOnClick: true,
  autoCollapse: false,
};

/** Mirrors `PINNED_IDLE_MS` in `services/settings/models.rs`. */
export const PINNED_IDLE_MS = 15_000;

/**
 * Pure transition function. Rules:
 * 1. The user beats the system: contextual events never interrupt the
 *    dashboard; they surface only from `collapsed` or replace a `peek`.
 * 2. Hover expands only after an intent delay; leaving collapses only after a
 *    delay, and never when pinned or in a detail view.
 * 3. Any click on the island pins it, so it stays until Escape, click-outside
 *    or the shortcut.
 * Unhandled events return the same object (a no-op for subscribers).
 */
export function transition(
  state: IslandState,
  event: IslandEvent,
  behavior: IslandBehavior = defaultBehavior,
): IslandState {
  const { mode } = state;
  switch (event.type) {
    case "POINTER_ENTER":
      return state.hovering ? state : { ...state, hovering: true };
    case "POINTER_LEAVE":
      return state.hovering ? { ...state, hovering: false } : state;
    case "HOVER_INTENT":
      return behavior.openOnHover && state.hovering && (mode === "collapsed" || mode === "peek")
        ? open(state, false)
        : state;
    case "LEAVE_INTENT":
      return !state.hovering && mode === "expanded" && !state.pinned ? collapse(state) : state;
    case "PRESS":
      if (mode === "collapsed" || mode === "peek") {
        return behavior.openOnClick ? open(state, behavior.pinOnClick) : state;
      }
      return mode === "expanded" && !state.pinned ? { ...state, pinned: true } : state;
    case "TOGGLE_PIN":
      return mode === "expanded" || mode === "focused"
        ? { ...state, pinned: !state.pinned }
        : state;
    case "OPEN_WIDGET":
      if (mode !== "expanded" && mode !== "focused") return state;
      if (mode === "focused" && state.activeWidget === event.widget) return state;
      return { ...state, mode: "focused", pinned: true, activeWidget: event.widget };
    case "CLOSE_WIDGET":
      return mode === "focused" ? { ...state, mode: "expanded", activeWidget: null } : state;
    case "ESCAPE":
      return mode === "focused"
        ? { ...state, mode: "expanded", activeWidget: null }
        : collapse(state);
    case "DISMISS":
      return collapse(state);
    case "SHORTCUT":
      return mode === "collapsed" || mode === "peek" ? open(state, true) : collapse(state);
    case "CONTEXT_EVENT":
      if (mode !== "collapsed" && mode !== "peek") return state;
      return { ...state, mode: "peek", peek: event.peek, peekSeq: state.peekSeq + 1 };
    case "PEEK_TIMEOUT":
      return mode === "peek" && event.seq === state.peekSeq ? collapse(state) : state;
    case "IDLE_TIMEOUT":
      return behavior.autoCollapse && state.pinned && !state.hovering && isDashboard(mode)
        ? collapse(state)
        : state;
  }
}

/* ------------------------------------------------------------------------ */
/* Timers                                                                    */
/* ------------------------------------------------------------------------ */

export interface IslandDelays {
  /** Hover-intent delay before expanding. */
  hoverMs: number;
  /** Delay after the pointer leaves before an unpinned dashboard collapses. */
  leaveMs: number;
  /** How long a peek stays. */
  peekMs: number;
}

type TimerSlot = "hover" | "leave" | "peek" | "idle";

interface DesiredTimer {
  /** Identity: a timer is restarted only when its key changes. */
  key: string;
  ms: number;
  event: IslandEvent;
}

/**
 * Timers are *derived from state*, not started ad hoc by handlers: after every
 * transition, each slot holds at most one timer, whose existence and identity
 * follow from the state. A stale timer therefore cannot fire into a state it
 * wasn't meant for, and competing timers can't exist.
 */
export function desiredTimers(
  state: IslandState,
  delays: IslandDelays,
  behavior: IslandBehavior = defaultBehavior,
): Record<TimerSlot, DesiredTimer | null> {
  const { mode, hovering } = state;
  return {
    hover:
      behavior.openOnHover && hovering && (mode === "collapsed" || mode === "peek")
        ? { key: "hover", ms: delays.hoverMs, event: { type: "HOVER_INTENT" } }
        : null,
    leave:
      !hovering && mode === "expanded" && !state.pinned
        ? { key: "leave", ms: delays.leaveMs, event: { type: "LEAVE_INTENT" } }
        : null,
    // The dwell pauses while the pointer rests on the peek.
    peek:
      mode === "peek" && !hovering
        ? {
            key: `peek:${state.peekSeq}`,
            ms: state.peek?.dwellMs ?? delays.peekMs,
            event: { type: "PEEK_TIMEOUT", seq: state.peekSeq },
          }
        : null,
    idle:
      behavior.autoCollapse && state.pinned && !hovering && isDashboard(mode)
        ? { key: "idle", ms: PINNED_IDLE_MS, event: { type: "IDLE_TIMEOUT" } }
        : null,
  };
}

export interface IslandStoreState extends IslandState {
  dispatch: (event: IslandEvent) => void;
}

export interface IslandStoreOptions {
  delays?: Partial<IslandDelays>;
  behavior?: Partial<IslandBehavior>;
}

export const defaultDelays: IslandDelays = {
  hoverMs: interaction.hoverIntentMs,
  leaveMs: interaction.leaveDelayMs,
  peekMs: interaction.peekDwellMs,
};

export function createIslandStore(options: IslandStoreOptions = {}) {
  let delays: IslandDelays = { ...defaultDelays, ...options.delays };
  let behavior: IslandBehavior = { ...defaultBehavior, ...options.behavior };
  const running = new Map<TimerSlot, { key: string; handle: ReturnType<typeof setTimeout> }>();

  let syncTimersFor: (state: IslandState) => void = () => undefined;

  const store = createStore<IslandStoreState>()((set, get) => {
    const syncTimers = (state: IslandState) => {
      const desired = desiredTimers(state, delays, behavior);
      for (const slot of Object.keys(desired) as TimerSlot[]) {
        const want = desired[slot];
        const have = running.get(slot);
        if (have && have.key === want?.key) continue;
        if (have) {
          clearTimeout(have.handle);
          running.delete(slot);
        }
        if (want) {
          const handle = setTimeout(() => {
            running.delete(slot);
            get().dispatch(want.event);
          }, want.ms);
          running.set(slot, { key: want.key, handle });
        }
      }
    };

    syncTimersFor = syncTimers;

    return {
      ...initialIslandState,
      dispatch: (event) => {
        const current = get();
        const next = transition(current, event, behavior);
        if (next === current) return;
        set(next);
        syncTimers(next);
      },
    };
  });

  /** Cancels all pending timers (tests, hot reload). */
  const dispose = () => {
    for (const { handle } of running.values()) clearTimeout(handle);
    running.clear();
  };

  /**
   * Applies new interaction preferences (Settings › Island) immediately.
   * Running timers whose delay changed restart; no restart needed.
   */
  const configure = (next: {
    delays?: Partial<IslandDelays>;
    behavior?: Partial<IslandBehavior>;
  }) => {
    delays = { ...delays, ...next.delays };
    behavior = { ...behavior, ...next.behavior };
    for (const { handle } of running.values()) clearTimeout(handle);
    running.clear();
    syncTimersFor(store.getState());
  };

  return Object.assign(store, {
    dispose,
    configure,
    pendingTimers: () => [...running.keys()],
  });
}

export type IslandStore = ReturnType<typeof createIslandStore>;

/** The app-wide island store. */
export const islandStore = createIslandStore();

export function useIsland<T>(selector: (state: IslandStoreState) => T): T {
  return useStore(islandStore, selector);
}

export const dispatchIsland = (event: IslandEvent) => {
  islandStore.getState().dispatch(event);
};

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    islandStore.dispose();
  });
}
