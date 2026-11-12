import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createIslandStore, type PeekContent } from "../island/island-store";
import { createPresentationCoordinator, GAP_MS, MAX_WAIT_MS } from "./presentation";

const peek = (title: string, dwellMs = 3000): PeekContent => ({
  source: "Test",
  title,
  detail: "",
  tone: "neutral",
  dwellMs,
});

let island: ReturnType<typeof createIslandStore>;
let quiet = false;

function coordinator() {
  return createPresentationCoordinator({
    island: {
      getState: () => island.getState(),
      subscribe: (l) => island.subscribe(l),
      dispatch: (e) => {
        island.getState().dispatch(e);
      },
    },
    quiet: () => quiet,
  });
}

const title = () => island.getState().peek?.title;

beforeEach(() => {
  vi.useFakeTimers();
  island = createIslandStore();
  quiet = false;
});

afterEach(() => {
  island.dispose();
  vi.useRealTimers();
});

describe("presentation coordinator", () => {
  it("shows an offer at once when collapsed, for its own dwell time", () => {
    const c = coordinator();
    c.offer({ key: "a", priority: 1, peek: peek("Build failed", 6000) });
    expect(island.getState().mode).toBe("peek");
    expect(title()).toBe("Build failed");
    vi.advanceTimersByTime(5999);
    expect(island.getState().mode).toBe("peek");
    vi.advanceTimersByTime(1);
    expect(island.getState().mode).toBe("collapsed");
    c.dispose();
  });

  it("lets a more important offer replace the current peek; others queue", () => {
    const c = coordinator();
    c.offer({ key: "info", priority: 2, peek: peek("Network restored") });
    c.offer({ key: "attn", priority: 0, peek: peek("Needs attention", 8000) });
    expect(title()).toBe("Needs attention");
    c.offer({ key: "done", priority: 1, peek: peek("Finished") });
    c.offer({ key: "media", priority: 3, peek: peek("Song") });
    expect(title()).toBe("Needs attention");
    expect(c.queued()).toEqual(["done", "media"]);
    // After the peek, the next one waits for a short gap (no flicker).
    vi.advanceTimersByTime(8000);
    expect(island.getState().mode).toBe("collapsed");
    vi.advanceTimersByTime(GAP_MS);
    expect(title()).toBe("Finished");
    vi.advanceTimersByTime(3000 + GAP_MS);
    expect(title()).toBe("Song");
    c.dispose();
  });

  it("coalesces queued offers by key", () => {
    const c = coordinator();
    c.offer({ key: "x", priority: 0, peek: peek("Current") });
    for (let i = 0; i < 20; i++) c.offer({ key: "build", priority: 1, peek: peek(`Build ${i}`) });
    expect(c.queued()).toEqual(["build"]);
    c.dispose();
  });

  it("never interrupts the dashboard and forgets the queue when it opens", () => {
    const c = coordinator();
    c.offer({ key: "a", priority: 0, peek: peek("First") });
    c.offer({ key: "b", priority: 1, peek: peek("Queued") });
    island.getState().dispatch({ type: "PRESS" }); // open + pin
    expect(island.getState().mode).toBe("expanded");
    c.offer({ key: "c", priority: 0, peek: peek("During dashboard") });
    expect(island.getState().mode).toBe("expanded");
    island.getState().dispatch({ type: "OPEN_WIDGET", widget: "media" });
    c.offer({ key: "d", priority: 0, peek: peek("During focus") });
    expect(island.getState().mode).toBe("focused");
    island.getState().dispatch({ type: "DISMISS" });
    vi.advanceTimersByTime(GAP_MS * 2);
    expect(island.getState().mode).toBe("collapsed");
    expect(c.queued()).toEqual([]);
    c.dispose();
  });

  it("shows nothing in quiet mode", () => {
    const c = coordinator();
    quiet = true;
    c.offer({ key: "a", priority: 0, peek: peek("Needs attention") });
    expect(island.getState().mode).toBe("collapsed");
    c.dispose();
  });

  it("discards queued offers that waited too long", () => {
    const c = coordinator();
    c.offer({ key: "long", priority: 0, peek: peek("Long", MAX_WAIT_MS + 1000) });
    c.offer({ key: "old", priority: 2, peek: peek("Old") });
    vi.advanceTimersByTime(MAX_WAIT_MS + 1000 + GAP_MS);
    expect(island.getState().mode).toBe("collapsed");
    c.dispose();
  });

  it("cancels its timer on dispose", () => {
    const c = coordinator();
    c.offer({ key: "a", priority: 0, peek: peek("A", 1000) });
    c.offer({ key: "b", priority: 1, peek: peek("B") });
    vi.advanceTimersByTime(1000);
    c.dispose();
    vi.advanceTimersByTime(GAP_MS * 2);
    expect(island.getState().mode).toBe("collapsed");
  });
});
