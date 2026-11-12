import { create } from "zustand";

/**
 * Island presentation modes. See docs/architecture.md → "Island state model".
 *
 * - collapsed: resting pill. The only mode rendered in Phase 0.
 * - peek:      temporary, system-initiated (a contextual event). Auto-returns.
 * - expanded:  transient, user-initiated by hover. Ends when the pointer leaves.
 * - focused:   sticky, user-initiated by click. Ends only on explicit dismissal.
 */
export type IslandMode = "collapsed" | "peek" | "expanded" | "focused";

export type IslandEvent =
  /** A contextual event (media change, download done, agent status…) wants attention. */
  | { type: "CONTEXT_EVENT" }
  /** The peek dwell time elapsed. */
  | { type: "PEEK_TIMEOUT" }
  | { type: "POINTER_ENTER" }
  | { type: "POINTER_LEAVE" }
  | { type: "CLICK" }
  /** Escape, click-away, or focus loss. */
  | { type: "DISMISS" };

export interface IslandState {
  mode: IslandMode;
  /**
   * Increments on every peek so the UI can restart the dwell timer when a new
   * contextual event replaces the current one.
   */
  peekId: number;
}

export const initialIslandState: IslandState = { mode: "collapsed", peekId: 0 };

/** Modes that end on their own without user input. */
export const TEMPORARY_MODES: ReadonlySet<IslandMode> = new Set(["peek", "expanded"]);

/** Modes the user is actively driving; automatic events must not interrupt them. */
export const USER_MODES: ReadonlySet<IslandMode> = new Set(["expanded", "focused"]);

/**
 * Pure transition function. Rules:
 * 1. User interaction always wins over automatic behavior.
 * 2. Contextual events only surface from `collapsed` or replace an active `peek`.
 * 3. `focused` is sticky: only DISMISS leaves it.
 * Unhandled events return the same state object (a no-op for subscribers).
 */
export function transition(state: IslandState, event: IslandEvent): IslandState {
  const { mode } = state;
  switch (event.type) {
    case "CONTEXT_EVENT":
      if (USER_MODES.has(mode)) return state;
      return { mode: "peek", peekId: state.peekId + 1 };
    case "PEEK_TIMEOUT":
      return mode === "peek" ? { ...state, mode: "collapsed" } : state;
    case "POINTER_ENTER":
      return mode === "collapsed" || mode === "peek" ? { ...state, mode: "expanded" } : state;
    case "POINTER_LEAVE":
      return mode === "expanded" ? { ...state, mode: "collapsed" } : state;
    case "CLICK":
      return mode === "focused" ? state : { ...state, mode: "focused" };
    case "DISMISS":
      return mode === "collapsed" ? state : { ...state, mode: "collapsed" };
  }
}

interface IslandStore extends IslandState {
  dispatch: (event: IslandEvent) => void;
}

export const useIslandStore = create<IslandStore>()((set, get) => ({
  ...initialIslandState,
  dispatch: (event) => {
    const current = get();
    const next = transition(current, event);
    if (next !== current) set(next);
  },
}));
