import { describe, expect, it } from "vitest";

import {
  ambientLabel,
  ambientOf,
  elapsedSince,
  filterEvents,
  formatElapsed,
  formatMinutes,
  peekForActivity,
  sessionStatusLabel,
  sessionTone,
  shortTitle,
} from "./activity-model";
import { event, peekRequest, session, snapshot, T0 } from "./activity-test-utils";

describe("ambient indicator", () => {
  it("is empty without sessions or running tasks", () => {
    expect(ambientOf(null)).toBeNull();
    expect(ambientOf(snapshot())).toBeNull();
    // Idle, ended and stale sessions don't occupy the pill.
    const quiet = snapshot({
      sessions: [
        session({ status: "idle" }),
        session({ id: "c2", status: "ended" }),
        session({ id: "c3", stale: true }),
      ],
    });
    expect(ambientOf(quiet)).toBeNull();
  });

  it("shows one working session as project · Working", () => {
    const a = ambientOf(snapshot({ sessions: [session()] }));
    expect(a && ambientLabel(a)).toEqual({ primary: "Islandy", secondary: "Working" });
  });

  it("puts attention first and counts multiple sessions", () => {
    const sessions = [
      session({ id: "c1" }),
      session({ id: "c2", project: "Web", status: "attention", attentionEvent: "a9" }),
      session({ id: "c3", project: "Api" }),
    ];
    const a = ambientOf(snapshot({ sessions }));
    expect(a?.kind).toBe("claude");
    if (a?.kind !== "claude") return;
    expect(a.session.project).toBe("Web");
    expect(ambientLabel(a)).toEqual({ primary: "3 sessions", secondary: "1 needs you" });
  });

  it("drops dismissed attention from the indicator", () => {
    const a = ambientOf(
      snapshot({ sessions: [session({ status: "attention", attentionDismissed: true })] }),
    );
    expect(a).toBeNull();
  });

  it("falls back to a running build", () => {
    const build = event({
      source: "developer",
      kind: "buildStarted",
      lifecycle: "active",
      title: "Build started",
      severity: "info",
      priority: 3,
    });
    const a = ambientOf(snapshot({ events: [build] }));
    expect(a && ambientLabel(a)).toEqual({ primary: "Islandy", secondary: "Building" });
    const done = ambientOf(snapshot({ events: [{ ...build, lifecycle: "resolved" }] }));
    expect(done).toBeNull();
  });
});

describe("session wording", () => {
  it("never claims completion and says unknown when stale", () => {
    expect(sessionStatusLabel(session({ status: "idle" }))).toBe("Finished responding");
    expect(sessionStatusLabel(session({ stale: true }))).toBe("Status unknown");
    expect(sessionTone(session({ stale: true }))).toBe("neutral");
    expect(sessionStatusLabel(session({ status: "waiting" }))).toBe("Waiting for input");
    expect(sessionTone(session({ status: "attention" }))).toBe("waiting");
    expect(sessionStatusLabel(session({ status: "attention", attentionDismissed: true }))).toBe(
      "Waiting (dismissed)",
    );
  });

  it("times the current turn, or the session, never an estimate", () => {
    expect(elapsedSince(session({ turnStartedAt: T0 + 5 }))).toBe(T0 + 5);
    expect(elapsedSince(session({ status: "idle", startedAt: T0 }))).toBe(T0);
    expect(elapsedSince(session({ status: "idle", startedAt: null }))).toBeNull();
    expect(formatElapsed(272_000)).toBe("04:32");
    expect(formatElapsed(3_723_000)).toBe("1:02:03");
    expect(formatElapsed(-5)).toBe("00:00");
    expect(formatMinutes(30_000)).toBe("< 1 min");
    expect(formatMinutes(4 * 60_000)).toBe("4 min");
    expect(formatMinutes(65 * 60_000)).toBe("1 h 05 min");
  });
});

describe("events", () => {
  it("turn requests into peeks with icon, tone and dwell", () => {
    const attention = event({
      kind: "claudeAttention",
      title: "Claude Code needs attention",
      description: "Permission requested",
      severity: "attention",
      priority: 0,
      lifecycle: "active",
    });
    expect(peekForActivity(peekRequest(attention, 8000))).toEqual({
      source: "Claude Code",
      title: "Needs attention",
      detail: "Islandy · Permission requested",
      tone: "waiting",
      icon: "claude",
      dwellMs: 8000,
    });
    const build = event({
      source: "developer",
      kind: "buildFailed",
      title: "Build failed",
      description: "Release",
      severity: "error",
      session: null,
    });
    expect(peekForActivity(peekRequest(build, 6000))).toMatchObject({
      source: "Islandy",
      detail: "Release",
      tone: "attention",
      icon: "build",
    });
    const network = event({
      source: "system",
      kind: "networkLost",
      title: "Network disconnected",
      severity: "warning",
      session: null,
      project: null,
    });
    expect(peekForActivity(peekRequest(network))).toMatchObject({
      source: "Network",
      icon: "offline",
    });
  });

  it("shortens Claude titles where the source is already shown", () => {
    expect(shortTitle(event({ title: "Claude Code is waiting for input" }))).toBe(
      "Waiting for input",
    );
    expect(shortTitle(event({ title: "Claude Code session started" }))).toBe("Session started");
    expect(shortTitle(event({ title: "Claude finished responding" }))).toBe(
      "Claude finished responding",
    );
    expect(shortTitle(event({ source: "developer", title: "Claude Code x" }))).toBe(
      "Claude Code x",
    );
  });

  it("filters by source", () => {
    const events = [event(), event({ source: "developer" }), event({ source: "system" })];
    expect(filterEvents(events, "all")).toHaveLength(3);
    expect(filterEvents(events, "developer").map((e) => e.source)).toEqual(["developer"]);
  });
});
