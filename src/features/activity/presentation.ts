/**
 * The presentation coordinator: decides *when* a contextual peek is shown,
 * given what the user is doing with the island.
 *
 * Rules (the island state machine already refuses peeks outside
 * `collapsed`/`peek`; these add priority and pacing on top):
 *
 * 1. Collapsed: show the offer now.
 * 2. A peek is showing: a strictly more important offer replaces it; others
 *    wait in a small queue (one entry per key, most important first).
 * 3. Dashboard open (expanded/focused, pinned or not): offers are dropped
 *    and the queue is cleared. The user is looking at the dashboard, where
 *    activity is visible; attention keeps its indicator in the pill.
 * 4. Quiet mode: nothing is shown.
 * 5. After a peek ends, the next queued one waits `GAP_MS` so the pill
 *    settles instead of flickering; queued offers older than `MAX_WAIT_MS`
 *    are discarded as stale.
 *
 * Exactly one timer exists at a time, so a stale timer can't show an offer
 * into a state it wasn't meant for.
 */
import type { IslandEvent, IslandState, PeekContent } from "../island/island-store";

export interface PeekOffer {
  /** Offers with the same key replace each other in the queue. */
  key: string;
  /** 0 most important … 3 least. */
  priority: number;
  peek: PeekContent;
}

interface Queued extends PeekOffer {
  at: number;
}

export const GAP_MS = 450;
export const MAX_WAIT_MS = 15_000;
export const MAX_QUEUE = 4;

export interface IslandLike {
  getState: () => IslandState;
  subscribe: (listener: (state: IslandState) => void) => () => void;
  dispatch: (event: IslandEvent) => void;
}

export interface CoordinatorDeps {
  island: IslandLike;
  quiet: () => boolean;
  now?: () => number;
  setTimer?: (fn: () => void, ms: number) => unknown;
  clearTimer?: (handle: unknown) => void;
}

export function createPresentationCoordinator(deps: CoordinatorDeps) {
  const now = deps.now ?? (() => Date.now());
  const setTimer = deps.setTimer ?? ((fn, ms) => setTimeout(fn, ms));
  const clearTimer =
    deps.clearTimer ??
    ((h) => {
      clearTimeout(h as ReturnType<typeof setTimeout>);
    });
  const { island } = deps;

  let queue: Queued[] = [];
  /** The peek this coordinator is showing: its island `peekSeq` and priority. */
  let showing: { seq: number; priority: number } | null = null;
  let timer: unknown = null;

  const cancelTimer = () => {
    if (timer !== null) clearTimer(timer);
    timer = null;
  };

  const show = (offer: PeekOffer) => {
    island.dispatch({ type: "CONTEXT_EVENT", peek: offer.peek });
    const state = island.getState();
    showing = state.mode === "peek" ? { seq: state.peekSeq, priority: offer.priority } : null;
  };

  /** Priority of the peek on screen (one we didn't show counts as P2). */
  const currentPriority = (state: IslandState): number =>
    showing && showing.seq === state.peekSeq ? showing.priority : 2;

  const enqueue = (offer: PeekOffer) => {
    queue = queue.filter((q) => q.key !== offer.key);
    queue.push({ ...offer, at: now() });
    queue.sort((a, b) => a.priority - b.priority || a.at - b.at);
    queue = queue.slice(0, MAX_QUEUE);
  };

  const takeNext = (): Queued | undefined => {
    const fresh = now() - MAX_WAIT_MS;
    queue = queue.filter((q) => q.at >= fresh);
    return queue.shift();
  };

  const scheduleNext = () => {
    if (timer !== null || queue.length === 0) return;
    timer = setTimer(() => {
      timer = null;
      const state = island.getState();
      if (state.mode !== "collapsed" || deps.quiet()) return;
      const next = takeNext();
      if (next) show(next);
    }, GAP_MS);
  };

  const unsubscribe = island.subscribe((state) => {
    if (state.mode === "expanded" || state.mode === "focused") {
      queue = [];
      showing = null;
      cancelTimer();
    } else if (state.mode === "collapsed") {
      showing = null;
      scheduleNext();
    }
  });

  return {
    offer(offer: PeekOffer) {
      if (deps.quiet()) return;
      const state = island.getState();
      if (state.mode === "expanded" || state.mode === "focused") return;
      if (state.mode === "collapsed" && timer === null) {
        show(offer);
        return;
      }
      if (state.mode === "peek" && offer.priority < currentPriority(state)) {
        show(offer);
        return;
      }
      enqueue(offer);
      if (state.mode === "collapsed") scheduleNext();
    },
    /** Drops everything queued (e.g. quiet mode turned on). */
    clear() {
      queue = [];
      cancelTimer();
    },
    queued: () => queue.map((q) => q.key),
    dispose() {
      unsubscribe();
      queue = [];
      cancelTimer();
    },
  };
}

export type PresentationCoordinator = ReturnType<typeof createPresentationCoordinator>;
