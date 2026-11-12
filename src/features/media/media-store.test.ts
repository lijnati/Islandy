import { afterEach, describe, expect, it, vi } from "vitest";

import { NativeError } from "../../services/native";
import type { MediaSnapshot } from "../../services/types";
import { createMediaStore, type MediaDeps } from "./media-store";
import { session, snapshot } from "./media-test-utils";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function setup(overrides: Partial<MediaDeps> = {}) {
  let push: ((s: MediaSnapshot) => void) | undefined;
  const deps: MediaDeps = {
    available: () => true,
    connectMedia: vi.fn((onSnapshot: (s: MediaSnapshot) => void) => {
      push = onSnapshot;
      return Promise.resolve(snapshot());
    }),
    mediaControl: vi.fn(() => Promise.resolve(snapshot({ revision: 1 }))),
    mediaSeek: vi.fn(() => Promise.resolve(snapshot({ revision: 1 }))),
    selectMediaSession: vi.fn(() => Promise.resolve(snapshot({ revision: 1 }))),
    getMediaArtwork: vi.fn((key: string) => Promise.resolve(`data:image/jpeg;base64,${key}`)),
    now: () => 5_000_000,
    ...overrides,
  };
  const store = createMediaStore(deps);
  stores.push(store);
  return { store, deps, push: (s: MediaSnapshot) => push?.(s) };
}

const stores: ReturnType<typeof createMediaStore>[] = [];
afterEach(() => {
  stores.splice(0).forEach((s) => {
    s.dispose();
  });
});

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("connection", () => {
  it("connects once, even if started repeatedly (Strict Mode)", async () => {
    const { store, deps } = setup();
    store.getState().start();
    store.getState().start();
    expect(store.getState().connection).toBe("connecting");
    await flush();
    expect(deps.connectMedia).toHaveBeenCalledTimes(1);
    expect(store.getState()).toMatchObject({ connection: "connected" });
    expect(store.getState().snapshot?.revision).toBe(1);
  });

  it("reports unavailability outside the desktop shell or on failure", async () => {
    const browser = setup({ available: () => false });
    browser.store.getState().start();
    expect(browser.store.getState().connection).toBe("unavailable");
    expect(browser.deps.connectMedia).not.toHaveBeenCalled();

    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const failing = setup({ connectMedia: () => Promise.reject(new Error("boom")) });
    failing.store.getState().start();
    await flush();
    expect(failing.store.getState().connection).toBe("unavailable");
  });
});

describe("snapshots", () => {
  it("ignores stale revisions", () => {
    const { store } = setup();
    expect(store.getState().applySnapshot(snapshot({ revision: 5 }))).toBe(true);
    expect(store.getState().applySnapshot(snapshot({ revision: 4, sessions: [] }))).toBe(false);
    expect(store.getState().applySnapshot(snapshot({ revision: 5, sessions: [] }))).toBe(false);
    expect(store.getState().snapshot?.sessions).toHaveLength(1);
  });

  it("a slow selection response cannot overwrite newer pushed state", async () => {
    const slow = deferred<MediaSnapshot>();
    const { store, push } = setup({ selectMediaSession: () => slow.promise });
    store.getState().start();
    await flush();
    const a = session({ id: "s1" });
    const b = session({ id: "s2" });
    const selection = store.getState().select("s1");
    expect(store.getState().selecting).toBe(true);
    push(
      snapshot({ revision: 3, sessions: [a, b], selectedSessionId: "s2", selectionMode: "user" }),
    );
    slow.resolve(
      snapshot({ revision: 2, sessions: [a, b], selectedSessionId: "s1", selectionMode: "user" }),
    );
    await selection;
    expect(store.getState().snapshot?.selectedSessionId).toBe("s2");
    expect(store.getState().selecting).toBe(false);
  });

  it("loads artwork for the selected session once and drops unreferenced artwork", async () => {
    const { store, deps } = setup();
    store
      .getState()
      .applySnapshot(snapshot({ revision: 1, sessions: [session({ artworkKey: "a1" })] }));
    store
      .getState()
      .applySnapshot(
        snapshot({ revision: 2, sessions: [session({ artworkKey: "a1", title: "x" })] }),
      );
    await flush();
    expect(deps.getMediaArtwork).toHaveBeenCalledTimes(1);
    expect(store.getState().artwork).toEqual({ a1: "data:image/jpeg;base64,a1" });
    store
      .getState()
      .applySnapshot(snapshot({ revision: 3, sessions: [session({ artworkKey: "a2" })] }));
    expect(store.getState().artwork).toEqual({});
    await flush();
    expect(store.getState().artwork).toEqual({ a2: "data:image/jpeg;base64,a2" });
  });

  it("discards artwork that arrives after the track changed", async () => {
    const slow = deferred<string | null>();
    const { store } = setup({ getMediaArtwork: () => slow.promise });
    store
      .getState()
      .applySnapshot(snapshot({ revision: 1, sessions: [session({ artworkKey: "old" })] }));
    store
      .getState()
      .applySnapshot(snapshot({ revision: 2, sessions: [session({ artworkKey: null })] }));
    slow.resolve("data:image/jpeg;base64,old");
    await flush();
    expect(store.getState().artwork).toEqual({});
  });
});

describe("commands", () => {
  it("never fakes the result and ignores repeated presses while pending", async () => {
    const pending = deferred<MediaSnapshot>();
    const { store, deps } = setup({ mediaControl: vi.fn(() => pending.promise) });
    store.getState().applySnapshot(snapshot({ revision: 1 }));
    const first = store.getState().command("togglePlayPause");
    void store.getState().command("togglePlayPause");
    expect(deps.mediaControl).toHaveBeenCalledTimes(1);
    expect(store.getState().pending).toEqual({ s1: "togglePlayPause" });
    // Still "playing": only a native snapshot may change that.
    expect(store.getState().snapshot?.sessions[0]?.playbackStatus).toBe("playing");
    pending.resolve(snapshot({ revision: 1 }));
    await first;
    expect(store.getState().pending).toEqual({});
    store
      .getState()
      .applySnapshot(snapshot({ revision: 2, sessions: [session({ playbackStatus: "paused" })] }));
    expect(store.getState().snapshot?.sessions[0]?.playbackStatus).toBe("paused");
  });

  it("does not call unsupported controls and reports failures neutrally", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { store, deps } = setup({
      mediaControl: vi.fn(() => Promise.reject(new NativeError("media_control", "rejected", "x"))),
    });
    const noNext = session({ controls: { ...session().controls, next: false } });
    store.getState().applySnapshot(snapshot({ revision: 1, sessions: [noNext] }));
    await store.getState().command("next");
    expect(deps.mediaControl).not.toHaveBeenCalled();
    expect(store.getState().notice?.message).toBe("Player doesn't support that");
    await store.getState().command("previous");
    expect(store.getState().notice?.message).toBe("Player didn't respond to that");
    expect(store.getState().pending).toEqual({});
  });

  it("does nothing without a selected session", async () => {
    const { store, deps } = setup();
    store.getState().applySnapshot(snapshot({ revision: 1, sessions: [] }));
    await store.getState().command("play");
    expect(deps.mediaControl).not.toHaveBeenCalled();
  });
});

describe("seeking", () => {
  it("validates, previews, and reconciles with the next native timeline", async () => {
    const { store, deps } = setup();
    store.getState().applySnapshot(snapshot({ revision: 1 }));
    await store.getState().seek(1_000_000);
    expect(deps.mediaSeek).toHaveBeenCalledWith("s1", 200_000);
    expect(store.getState().seekPreview).toEqual({
      sessionId: "s1",
      positionMs: 200_000,
      at: 5_000_000,
    });
    // An older timeline update keeps the preview; a newer one replaces it.
    store.getState().applySnapshot(snapshot({ revision: 2, sessions: [session({ title: "t" })] }));
    expect(store.getState().seekPreview).not.toBeNull();
    store.getState().applySnapshot(
      snapshot({
        revision: 3,
        sessions: [
          session({
            timeline: { positionMs: 199_000, durationMs: 200_000, updatedAtMs: 5_000_100 },
          }),
        ],
      }),
    );
    expect(store.getState().seekPreview).toBeNull();
  });

  it("refuses unseekable sessions and rolls back failed seeks", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { store, deps } = setup({
      mediaSeek: vi.fn(() => Promise.reject(new NativeError("media_seek", "timeout", "x"))),
    });
    store.getState().applySnapshot(
      snapshot({
        revision: 1,
        sessions: [session({ controls: { ...session().controls, seek: false } })],
      }),
    );
    await store.getState().seek(10);
    expect(deps.mediaSeek).not.toHaveBeenCalled();
    store.getState().applySnapshot(snapshot({ revision: 2 }));
    await store.getState().seek(10);
    expect(store.getState().seekPreview).toBeNull();
    expect(store.getState().notice?.message).toBe("Player isn't responding");
  });
});
