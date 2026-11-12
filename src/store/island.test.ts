import { describe, expect, it } from "vitest";

import { initialIslandState, transition, type IslandEvent, type IslandMode } from "./island";

const at = (mode: IslandMode, peekId = 0) => ({ mode, peekId });
const run = (mode: IslandMode, type: IslandEvent["type"]) => transition(at(mode), { type }).mode;

describe("island transition model", () => {
  it("starts collapsed", () => {
    expect(initialIslandState.mode).toBe("collapsed");
  });

  it("surfaces contextual events as a peek from collapsed", () => {
    expect(run("collapsed", "CONTEXT_EVENT")).toBe("peek");
  });

  it("restarts the peek when a new event replaces the current one", () => {
    const next = transition(at("peek", 3), { type: "CONTEXT_EVENT" });
    expect(next).toEqual({ mode: "peek", peekId: 4 });
  });

  it("never lets automatic events interrupt the user", () => {
    for (const mode of ["expanded", "focused"] as const) {
      const state = at(mode);
      expect(transition(state, { type: "CONTEXT_EVENT" })).toBe(state);
      expect(transition(state, { type: "PEEK_TIMEOUT" })).toBe(state);
    }
  });

  it("returns from peek to collapsed on timeout", () => {
    expect(run("peek", "PEEK_TIMEOUT")).toBe("collapsed");
  });

  it("expands on hover and collapses when the pointer leaves", () => {
    expect(run("collapsed", "POINTER_ENTER")).toBe("expanded");
    expect(run("peek", "POINTER_ENTER")).toBe("expanded");
    expect(run("expanded", "POINTER_LEAVE")).toBe("collapsed");
  });

  it("keeps focused sticky until dismissed", () => {
    expect(run("expanded", "CLICK")).toBe("focused");
    expect(run("focused", "POINTER_LEAVE")).toBe("focused");
    expect(run("focused", "CLICK")).toBe("focused");
    expect(run("focused", "DISMISS")).toBe("collapsed");
  });

  it("returns the same object for no-op transitions", () => {
    const state = at("collapsed");
    expect(transition(state, { type: "POINTER_LEAVE" })).toBe(state);
    expect(transition(state, { type: "DISMISS" })).toBe(state);
  });
});
