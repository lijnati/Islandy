import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { NativeError } from "../../services/native";
import type { SystemSnapshot, SystemUpdate } from "../../services/types";
import {
  ALERT_REPEAT_MS,
  createSystemStore,
  INTENT_GRACE_MS,
  SEND_INTERVAL_MS,
  type SystemDeps,
} from "./system-store";
import { audio, systemSnapshot } from "./system-test-utils";

function deferred<T = void>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const stores: ReturnType<typeof createSystemStore>[] = [];
let clock = 1_000_000;

function setup(overrides: Partial<SystemDeps> = {}, initial: SystemSnapshot = systemSnapshot()) {
  let push: ((u: SystemUpdate) => void) | undefined;
  const deps: SystemDeps = {
    available: () => true,
    connectSystem: vi.fn((onUpdate: (u: SystemUpdate) => void) => {
      push = onUpdate;
      return Promise.resolve(initial);
    }),
    setSystemFocus: vi.fn(() => Promise.resolve()),
    setMasterVolume: vi.fn(() => Promise.resolve()),
    setAudioMuted: vi.fn(() => Promise.resolve()),
    setRadioState: vi.fn(() => Promise.resolve()),
    setDisplayBrightness: vi.fn(() => Promise.resolve()),
    now: () => clock,
    ...overrides,
  };
  const store = createSystemStore(deps);
  stores.push(store);
  store.getState().start();
  return { store, deps, push: (u: SystemUpdate) => push?.(u) };
}

const flush = async () => {
  await vi.advanceTimersByTimeAsync(0);
};

function volumeUpdate(revision: number, volume: number, muted = false): SystemUpdate {
  return {
    domain: "audio",
    revision,
    state: audio({
      output: { deviceId: "d1", name: "Speakers (Realtek Audio)", volume, muted },
    }),
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  clock = 1_000_000;
});

afterEach(() => {
  stores.splice(0).forEach((s) => {
    s.dispose();
  });
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("connection and updates", () => {
  it("connects once and applies updates per domain, ignoring stale ones", async () => {
    const { store, deps, push } = setup();
    store.getState().start();
    await flush();
    expect(deps.connectSystem).toHaveBeenCalledTimes(1);
    expect(store.getState().connection).toBe("connected");
    push(volumeUpdate(3, 55));
    expect(store.getState().snapshot?.audio.output?.volume).toBe(55);
    push(volumeUpdate(2, 10)); // older audio update
    expect(store.getState().snapshot?.audio.output?.volume).toBe(55);
    // A different domain at an older global revision is still newer for itself.
    const metrics = { ...systemSnapshot().metrics, cpuPercent: 12 };
    push({ domain: "metrics", revision: 2, state: metrics });
    expect(store.getState().snapshot?.metrics.cpuPercent).toBe(12);
  });

  it("keeps updates that arrive before the connect response", async () => {
    const response = deferred<SystemSnapshot>();
    const { store } = setup({ connectSystem: vi.fn(() => response.promise) });
    store.getState().applyUpdate(volumeUpdate(5, 77)); // pushed before the response
    response.resolve(systemSnapshot({ revision: 4 }));
    await flush();
    const s = store.getState();
    expect(s.connection).toBe("connected");
    expect(s.snapshot?.audio.output?.volume).toBe(77);
    expect(s.snapshot?.power.status).toBe("ready");
  });

  it("is unavailable outside the desktop app", () => {
    const { store, deps } = setup({ available: () => false });
    expect(store.getState().connection).toBe("unavailable");
    expect(deps.connectSystem).not.toHaveBeenCalled();
  });
});

describe("volume slider", () => {
  it("coalesces a drag: one request in flight, latest value wins", async () => {
    const first = deferred();
    const calls: number[] = [];
    const { store } = setup({
      setMasterVolume: vi.fn((v: number) => {
        calls.push(v);
        return calls.length === 1 ? first.promise : Promise.resolve();
      }),
    });
    await flush();
    for (const v of [41, 45, 50, 58, 63]) store.getState().setVolume(v);
    expect(calls).toEqual([41]);
    expect(store.getState().intents.volume?.value).toBe(63); // the thumb follows the user
    first.resolve();
    await flush();
    await vi.advanceTimersByTimeAsync(SEND_INTERVAL_MS);
    expect(calls).toEqual([41, 63]);
  });

  it("doesn't let an older confirmation move the thumb back", async () => {
    const pending = deferred();
    const { store, push } = setup({ setMasterVolume: vi.fn(() => pending.promise) });
    await flush();
    store.getState().setVolume(80);
    push(volumeUpdate(2, 60)); // Windows reports an intermediate value
    expect(store.getState().snapshot?.audio.output?.volume).toBe(60);
    expect(store.getState().intents.volume?.value).toBe(80);
    pending.resolve();
    await flush();
    // Settled, but Windows hasn't confirmed 80 yet: keep showing 80…
    expect(store.getState().intents.volume?.value).toBe(80);
    push(volumeUpdate(3, 80));
    // …until a newer native value arrives; then Windows' value is shown.
    expect(store.getState().intents.volume).toBeUndefined();
    expect(store.getState().snapshot?.audio.output?.volume).toBe(80);
  });

  it("drops the intent after a grace period even without a confirmation", async () => {
    const { store } = setup();
    await flush();
    store.getState().setVolume(90);
    await flush();
    expect(store.getState().intents.volume).toBeDefined();
    await vi.advanceTimersByTimeAsync(INTENT_GRACE_MS);
    expect(store.getState().intents.volume).toBeUndefined();
  });

  it("reverts to Windows' value and explains when a change fails", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { store } = setup({
      setMasterVolume: vi.fn(() =>
        Promise.reject(new NativeError("set_master_volume", "notFound", "x")),
      ),
    });
    await flush();
    store.getState().setVolume(5);
    await flush();
    expect(store.getState().intents.volume).toBeUndefined();
    expect(store.getState().snapshot?.audio.output?.volume).toBe(40);
    expect(store.getState().notice?.message).toBe("That device is no longer available");
  });

  it("ignores volume without an output device, and clamps values", async () => {
    const { store, deps } = setup({}, systemSnapshot({ audio: audio({ output: null }) }));
    await flush();
    store.getState().setVolume(50);
    expect(deps.setMasterVolume).not.toHaveBeenCalled();

    const other = setup();
    await flush();
    other.store.getState().setVolume(140);
    expect(other.deps.setMasterVolume).toHaveBeenCalledWith(100);
    other.store.getState().setVolume(Number.NaN);
    expect(other.deps.setMasterVolume).toHaveBeenCalledTimes(1);
  });
});

describe("brightness", () => {
  it("sends brightness per display and only for known displays", async () => {
    const { store, deps } = setup();
    await flush();
    store.getState().setBrightness("i0", 30);
    store.getState().setBrightness("e9", 30);
    expect(deps.setDisplayBrightness).toHaveBeenCalledTimes(1);
    expect(deps.setDisplayBrightness).toHaveBeenCalledWith("i0", 30);
  });
});

describe("toggles are never optimistic", () => {
  it("mute shows busy, not muted, until Windows reports it", async () => {
    const pending = deferred();
    const { store, deps, push } = setup({ setAudioMuted: vi.fn(() => pending.promise) });
    await flush();
    const done = store.getState().toggleMute();
    void store.getState().toggleMute(); // repeated press while pending
    expect(deps.setAudioMuted).toHaveBeenCalledTimes(1);
    expect(deps.setAudioMuted).toHaveBeenCalledWith(true);
    expect(store.getState().pending.mute).toBe(true);
    expect(store.getState().snapshot?.audio.output?.muted).toBe(false);
    pending.resolve();
    await done;
    expect(store.getState().pending.mute).toBeUndefined();
    push(volumeUpdate(2, 40, true));
    expect(store.getState().snapshot?.audio.output?.muted).toBe(true);
  });

  it("radios: only switchable radios are sent; denials are explained", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { store, deps } = setup({
      setRadioState: vi.fn(() => Promise.reject(new NativeError("set_radio_state", "denied", "x"))),
    });
    await flush();
    await store.getState().setRadio("bluetooth", true); // canToggle: false
    expect(deps.setRadioState).not.toHaveBeenCalled();
    await store.getState().setRadio("wifi", false);
    expect(deps.setRadioState).toHaveBeenCalledWith("wifi", false);
    expect(store.getState().snapshot?.radios.wifi?.state).toBe("on");
    expect(store.getState().notice?.message).toBe("Windows didn't allow that change");
  });
});

describe("alerts and focus", () => {
  it("delivers alerts once per window and supports unsubscribing", async () => {
    const { store, push } = setup();
    await flush();
    const seen: string[] = [];
    const off = store.getState().onAlert((a) => seen.push(a.type));
    push({ domain: "alert", revision: 2, alert: { type: "batteryLow", percent: 15 } });
    push({ domain: "alert", revision: 3, alert: { type: "batteryLow", percent: 14 } });
    push({ domain: "alert", revision: 4, alert: { type: "networkLost" } });
    expect(seen).toEqual(["batteryLow", "networkLost"]);
    clock += ALERT_REPEAT_MS;
    push({ domain: "alert", revision: 5, alert: { type: "batteryLow", percent: 12 } });
    expect(seen).toEqual(["batteryLow", "networkLost", "batteryLow"]);
    off();
    clock += ALERT_REPEAT_MS;
    push({ domain: "alert", revision: 6, alert: { type: "networkLost" } });
    expect(seen).toHaveLength(3);
  });

  it("sends focus changes once, and a pre-connect focus after connecting", async () => {
    const response = deferred<SystemSnapshot>();
    const { store, deps } = setup({ connectSystem: vi.fn(() => response.promise) });
    store.getState().setFocus("overview");
    expect(deps.setSystemFocus).not.toHaveBeenCalled();
    response.resolve(systemSnapshot());
    await flush();
    expect(deps.setSystemFocus).toHaveBeenCalledWith("overview");
    store.getState().setFocus("overview");
    store.getState().setFocus("none");
    expect(deps.setSystemFocus).toHaveBeenCalledTimes(2);
    expect(deps.setSystemFocus).toHaveBeenLastCalledWith("none");
  });
});
