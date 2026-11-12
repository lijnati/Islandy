import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  createIslandStore,
  desiredTimers,
  initialIslandState,
  transition,
  type IslandEvent,
  type IslandState,
  type IslandStore,
} from "./island-store";

const peek = { source: "Test", title: "Hello", detail: "world", tone: "neutral" } as const;
const delays = { hoverMs: 150, leaveMs: 360, peekMs: 4000 };

function run(events: IslandEvent[], from: IslandState = initialIslandState): IslandState {
  return events.reduce(transition, from);
}

describe("transition (pure)", () => {
  it("hover alone does not expand; hover intent does", () => {
    const hovering = run([{ type: "POINTER_ENTER" }]);
    expect(hovering).toMatchObject({ mode: "collapsed", hovering: true });
    expect(transition(hovering, { type: "HOVER_INTENT" })).toMatchObject({
      mode: "expanded",
      pinned: false,
    });
  });

  it("ignores a hover intent that arrives after the pointer left", () => {
    const s = run([{ type: "POINTER_ENTER" }, { type: "POINTER_LEAVE" }, { type: "HOVER_INTENT" }]);
    expect(s.mode).toBe("collapsed");
  });

  it("click pins: collapsed → expanded+pinned, and pins a hover-expanded dashboard", () => {
    expect(run([{ type: "PRESS" }])).toMatchObject({ mode: "expanded", pinned: true });
    const hovered = run([{ type: "POINTER_ENTER" }, { type: "HOVER_INTENT" }]);
    expect(transition(hovered, { type: "PRESS" })).toMatchObject({
      mode: "expanded",
      pinned: true,
    });
  });

  it("leave intent collapses only an unpinned dashboard with the pointer away", () => {
    const open = run([
      { type: "POINTER_ENTER" },
      { type: "HOVER_INTENT" },
      { type: "POINTER_LEAVE" },
    ]);
    expect(transition(open, { type: "LEAVE_INTENT" }).mode).toBe("collapsed");
    const pinned = transition(open, { type: "TOGGLE_PIN" });
    expect(transition(pinned, { type: "LEAVE_INTENT" })).toBe(pinned);
    const back = transition(open, { type: "POINTER_ENTER" });
    expect(transition(back, { type: "LEAVE_INTENT" })).toBe(back);
  });

  it("widget detail: expanded ⇄ focused, Escape steps back then collapses", () => {
    const focused = run([{ type: "PRESS" }, { type: "OPEN_WIDGET", widget: "calendar" }]);
    expect(focused).toMatchObject({ mode: "focused", activeWidget: "calendar", pinned: true });
    const back = transition(focused, { type: "ESCAPE" });
    expect(back).toMatchObject({ mode: "expanded", activeWidget: null, pinned: true });
    expect(transition(back, { type: "ESCAPE" })).toMatchObject({
      mode: "collapsed",
      pinned: false,
    });
    expect(transition(focused, { type: "CLOSE_WIDGET" }).mode).toBe("expanded");
  });

  it("opening a widget from a hover-expanded dashboard pins it", () => {
    const s = run([
      { type: "POINTER_ENTER" },
      { type: "HOVER_INTENT" },
      { type: "OPEN_WIDGET", widget: "media" },
    ]);
    expect(s).toMatchObject({ mode: "focused", pinned: true });
    expect(transition(s, { type: "LEAVE_INTENT" })).toBe(s);
  });

  it("dismiss resets everything, from any mode", () => {
    for (const events of [
      [{ type: "PRESS" }],
      [{ type: "PRESS" }, { type: "OPEN_WIDGET", widget: "activity" }],
      [{ type: "CONTEXT_EVENT", peek }],
    ] as IslandEvent[][]) {
      expect(run([...events, { type: "DISMISS" }])).toMatchObject({
        mode: "collapsed",
        pinned: false,
        activeWidget: null,
        peek: null,
      });
    }
    expect(transition(initialIslandState, { type: "DISMISS" })).toBe(initialIslandState);
  });

  it("collapsing clears hover, so a pointer resting on the shrinking surface cannot re-expand it", () => {
    const s = run([{ type: "POINTER_ENTER" }, { type: "PRESS" }, { type: "ESCAPE" }]);
    expect(s).toMatchObject({ mode: "collapsed", hovering: false });
    expect(transition(s, { type: "HOVER_INTENT" })).toBe(s);
  });

  it("the shortcut toggles open (pinned) and closed", () => {
    const open = transition(initialIslandState, { type: "SHORTCUT" });
    expect(open).toMatchObject({ mode: "expanded", pinned: true });
    expect(transition(open, { type: "SHORTCUT" }).mode).toBe("collapsed");
    const focused = transition(open, { type: "OPEN_WIDGET", widget: "media" });
    expect(transition(focused, { type: "SHORTCUT" }).mode).toBe("collapsed");
  });

  it("contextual events: surface from collapsed, replace a peek, never interrupt the dashboard", () => {
    const p1 = transition(initialIslandState, { type: "CONTEXT_EVENT", peek });
    expect(p1).toMatchObject({ mode: "peek", peekSeq: 1, peek });
    const p2 = transition(p1, { type: "CONTEXT_EVENT", peek: { ...peek, title: "Newer" } });
    expect(p2).toMatchObject({ peekSeq: 2, peek: { title: "Newer" } });
    const open = run([{ type: "PRESS" }]);
    expect(transition(open, { type: "CONTEXT_EVENT", peek })).toBe(open);
  });

  it("a stale peek timeout cannot close a newer peek", () => {
    const p2 = run([
      { type: "CONTEXT_EVENT", peek },
      { type: "CONTEXT_EVENT", peek },
    ]);
    expect(transition(p2, { type: "PEEK_TIMEOUT", seq: 1 })).toBe(p2);
    expect(transition(p2, { type: "PEEK_TIMEOUT", seq: 2 }).mode).toBe("collapsed");
  });

  it("peek → expanded on interaction", () => {
    const p = transition(initialIslandState, { type: "CONTEXT_EVENT", peek });
    expect(run([{ type: "POINTER_ENTER" }, { type: "HOVER_INTENT" }], p)).toMatchObject({
      mode: "expanded",
      peek: null,
    });
    expect(transition(p, { type: "PRESS" })).toMatchObject({ mode: "expanded", pinned: true });
  });

  it("derives at most one timer per slot", () => {
    const hovering = run([{ type: "POINTER_ENTER" }]);
    expect(desiredTimers(hovering, delays)).toMatchObject({
      hover: { ms: 150 },
      leave: null,
      peek: null,
    });
    const pinned = run([{ type: "PRESS" }]);
    expect(desiredTimers(pinned, delays)).toEqual({ hover: null, leave: null, peek: null });
  });
});

describe("island store timers", () => {
  let store: IslandStore;
  const dispatch = (event: IslandEvent) => {
    store.getState().dispatch(event);
  };
  const mode = () => store.getState().mode;

  beforeEach(() => {
    vi.useFakeTimers();
    store = createIslandStore({ delays });
  });
  afterEach(() => {
    store.dispose();
    vi.useRealTimers();
  });

  it("expands after the hover-intent delay, not before", () => {
    dispatch({ type: "POINTER_ENTER" });
    vi.advanceTimersByTime(149);
    expect(mode()).toBe("collapsed");
    vi.advanceTimersByTime(1);
    expect(mode()).toBe("expanded");
  });

  it("a brief pass over the pill does not expand it (timer cancelled)", () => {
    dispatch({ type: "POINTER_ENTER" });
    vi.advanceTimersByTime(100);
    dispatch({ type: "POINTER_LEAVE" });
    expect(store.pendingTimers()).toEqual([]);
    vi.advanceTimersByTime(1000);
    expect(mode()).toBe("collapsed");
  });

  it("auto-collapses after the leave delay; re-entering cancels it", () => {
    dispatch({ type: "POINTER_ENTER" });
    vi.advanceTimersByTime(150);
    dispatch({ type: "POINTER_LEAVE" });
    vi.advanceTimersByTime(300);
    dispatch({ type: "POINTER_ENTER" }); // came back in time
    vi.advanceTimersByTime(1000);
    expect(mode()).toBe("expanded");
    dispatch({ type: "POINTER_LEAVE" });
    vi.advanceTimersByTime(359);
    expect(mode()).toBe("expanded");
    vi.advanceTimersByTime(1);
    expect(mode()).toBe("collapsed");
  });

  it("a pinned dashboard never auto-collapses; unpinning restarts the leave delay", () => {
    dispatch({ type: "PRESS" });
    dispatch({ type: "POINTER_LEAVE" });
    vi.advanceTimersByTime(10_000);
    expect(mode()).toBe("expanded");
    dispatch({ type: "TOGGLE_PIN" });
    vi.advanceTimersByTime(360);
    expect(mode()).toBe("collapsed");
  });

  it("does not flicker under rapid enter/leave", () => {
    for (let i = 0; i < 20; i++) {
      dispatch({ type: "POINTER_ENTER" });
      vi.advanceTimersByTime(60);
      dispatch({ type: "POINTER_LEAVE" });
      vi.advanceTimersByTime(60);
    }
    expect(mode()).toBe("collapsed");
    expect(store.pendingTimers()).toEqual([]);
  });

  it("peeks expire, newer peeks restart the dwell, and hovering pauses it", () => {
    dispatch({ type: "CONTEXT_EVENT", peek });
    vi.advanceTimersByTime(3000);
    dispatch({ type: "CONTEXT_EVENT", peek }); // replaces; restarts dwell
    vi.advanceTimersByTime(3000);
    expect(mode()).toBe("peek");
    vi.advanceTimersByTime(1000);
    expect(mode()).toBe("collapsed");

    dispatch({ type: "CONTEXT_EVENT", peek });
    dispatch({ type: "POINTER_ENTER" });
    dispatch({ type: "POINTER_LEAVE" }); // left before the hover intent
    vi.advanceTimersByTime(3999);
    expect(mode()).toBe("peek");
    vi.advanceTimersByTime(1);
    expect(mode()).toBe("collapsed");
  });

  it("dismissing cancels every pending timer", () => {
    dispatch({ type: "CONTEXT_EVENT", peek });
    dispatch({ type: "DISMISS" });
    expect(store.pendingTimers()).toEqual([]);
  });

  it("notifies subscribers only on real changes", () => {
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);
    dispatch({ type: "POINTER_LEAVE" });
    dispatch({ type: "LEAVE_INTENT" });
    dispatch({ type: "DISMISS" });
    expect(listener).not.toHaveBeenCalled();
    dispatch({ type: "PRESS" });
    expect(listener).toHaveBeenCalledTimes(1);
    unsubscribe();
  });
});

describe("per-peek dwell (Phase 4)", () => {
  it("uses the peek's own dwell time, else the default", () => {
    const s = transition(initialIslandState, {
      type: "CONTEXT_EVENT",
      peek: { ...peek, dwellMs: 8000 },
    });
    expect(desiredTimers(s, delays).peek?.ms).toBe(8000);
    const d = transition(initialIslandState, { type: "CONTEXT_EVENT", peek });
    expect(desiredTimers(d, delays).peek?.ms).toBe(4000);
  });
});
