import { describe, expect, it } from "vitest";

import {
  canPerform,
  commandErrorMessage,
  displaySubtitle,
  displayTitle,
  isActive,
  peekForChange,
  positionAt,
  SEEK_PREVIEW_MS,
  selectedSession,
  UNKNOWN_TRACK,
  validateSeek,
} from "./media-model";
import { session, snapshot } from "./media-test-utils";

const T0 = 1_000_000;

describe("selection", () => {
  it("returns the selected session or null", () => {
    const a = session({ id: "s1" });
    const b = session({ id: "s2" });
    expect(selectedSession(snapshot({ sessions: [a, b], selectedSessionId: "s2" }))).toBe(b);
    expect(selectedSession(snapshot({ sessions: [a], selectedSessionId: null }))).toBeNull();
    expect(selectedSession(snapshot({ sessions: [a], selectedSessionId: "gone" }))).toBeNull();
    expect(selectedSession(null)).toBeNull();
  });

  it("only playing/paused/changing sessions are active in the pill", () => {
    expect(isActive(session({ playbackStatus: "paused" }))).toBe(true);
    expect(isActive(session({ playbackStatus: "stopped" }))).toBe(false);
    expect(isActive(null)).toBe(false);
  });
});

describe("missing metadata", () => {
  it("uses neutral placeholders and never invents values", () => {
    const s = session({ title: null, artist: null, albumArtist: null, sourceAppName: "Browser" });
    expect(displayTitle(s)).toBe(UNKNOWN_TRACK);
    expect(displaySubtitle(s)).toBe("Browser");
    expect(displaySubtitle(session({ artist: null, albumArtist: "Band" }))).toBe("Band");
  });
});

describe("timeline interpolation", () => {
  it("advances while playing, at the playback rate", () => {
    const s = session();
    expect(positionAt(s, T0)).toBe(10_000);
    expect(positionAt(s, T0 + 2_000)).toBe(12_000);
    expect(positionAt(session({ playbackRate: 2 }), T0 + 2_000)).toBe(14_000);
  });

  it("stops when paused and clamps to known bounds", () => {
    expect(positionAt(session({ playbackStatus: "paused" }), T0 + 60_000)).toBe(10_000);
    expect(positionAt(session(), T0 + 10_000_000)).toBe(200_000);
    expect(positionAt(session(), T0 - 5_000)).toBe(10_000);
  });

  it("handles unknown duration and missing timelines", () => {
    const live = session({ timeline: { positionMs: 5_000, durationMs: null, updatedAtMs: T0 } });
    expect(positionAt(live, T0 + 1_000)).toBe(6_000);
    expect(positionAt(session({ timeline: null }), T0)).toBeNull();
  });

  it("uses a seek preview until a newer native update or its expiry", () => {
    const s = session();
    const preview = { sessionId: "s1", positionMs: 100_000, at: T0 + 500 };
    expect(positionAt(s, T0 + 1_500, preview)).toBe(101_000);
    expect(positionAt(s, T0 + 500 + SEEK_PREVIEW_MS, preview)).toBe(10_000 + 500 + SEEK_PREVIEW_MS);
    const confirmed = session({
      timeline: { positionMs: 100_200, durationMs: 200_000, updatedAtMs: T0 + 700 },
    });
    expect(positionAt(confirmed, T0 + 1_700, preview)).toBe(101_200);
    expect(positionAt(s, T0, { ...preview, sessionId: "other" })).toBe(10_000);
  });
});

describe("capabilities and seeking", () => {
  it("mirrors the Rust toggle fallback", () => {
    const noToggle = session({
      controls: {
        play: true,
        pause: false,
        togglePlayPause: false,
        next: false,
        previous: false,
        seek: false,
      },
    });
    expect(canPerform(noToggle, "togglePlayPause")).toBe(false); // playing, cannot pause
    expect(canPerform({ ...noToggle, playbackStatus: "paused" }, "togglePlayPause")).toBe(true);
    expect(canPerform(noToggle, "next")).toBe(false);
  });

  it("validates and clamps seek targets", () => {
    expect(validateSeek(session(), 50_000.4)).toBe(50_000);
    expect(validateSeek(session(), -5)).toBe(0);
    expect(validateSeek(session(), 999_999)).toBe(200_000);
    expect(validateSeek(session(), Number.NaN)).toBeNull();
    expect(
      validateSeek(session({ controls: { ...session().controls, seek: false } }), 1),
    ).toBeNull();
    expect(
      validateSeek(session({ timeline: { positionMs: 0, durationMs: null, updatedAtMs: T0 } }), 1),
    ).toBeNull();
  });
});

describe("peeks", () => {
  const before = snapshot({ revision: 1, sessions: [session({ title: "One" })] });

  it("peeks when a new track starts playing", () => {
    const next = snapshot({ revision: 2, sessions: [session({ title: "Two", artworkKey: "a1" })] });
    expect(peekForChange(before, next)).toEqual({
      source: "Player",
      title: "Two",
      detail: "Artist",
      tone: "neutral",
      artworkKey: "a1",
    });
  });

  it("does not peek on the first snapshot, pause/resume, refreshes or untitled tracks", () => {
    expect(peekForChange(null, before)).toBeNull();
    const paused = snapshot({
      revision: 2,
      sessions: [session({ title: "One", playbackStatus: "paused" })],
    });
    expect(peekForChange(before, paused)).toBeNull();
    expect(peekForChange(paused, { ...before, revision: 3 })).toBeNull(); // resume
    const moved = snapshot({ revision: 2, sessions: [session({ title: "One", timeline: null })] });
    expect(peekForChange(before, moved)).toBeNull();
    const untitled = snapshot({ revision: 2, sessions: [session({ title: null })] });
    expect(peekForChange(before, untitled)).toBeNull();
  });

  it("peeks when music starts after nothing was playing", () => {
    const empty = snapshot({ revision: 1, sessions: [] });
    expect(peekForChange(empty, { ...before, revision: 2 })?.title).toBe("One");
  });
});

describe("error wording", () => {
  it("is neutral and never exposes internals", () => {
    expect(commandErrorMessage("unsupported", "Player")).toBe("Player doesn't support that");
    expect(commandErrorMessage("platform", "Player")).toBe("Couldn't control playback");
    expect(commandErrorMessage("unknown", "Player")).toBe("Couldn't control playback");
  });
});
