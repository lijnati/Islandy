import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_ACTIVITY_SETTINGS } from "../settings/defaults";

import { NativeError } from "../../services/native";
import type { ActivitySnapshot, ActivityUpdate, ClaudeIntegration } from "../../services/types";
import { createActivityStore, type ActivityDeps } from "./activity-store";
import { event, peekRequest, snapshot } from "./activity-test-utils";

const stores: ReturnType<typeof createActivityStore>[] = [];

const integration: ClaudeIntegration = {
  state: "notInstalled",
  settingsPath: "/home/u/.claude/settings.json",
  events: ["Stop"],
  snippet: "{}",
};

function setup(overrides: Partial<ActivityDeps> = {}, initial: ActivitySnapshot = snapshot()) {
  let push: ((u: ActivityUpdate) => void) | undefined;
  const deps: ActivityDeps = {
    available: () => true,
    connectActivity: vi.fn((onUpdate: (u: ActivityUpdate) => void) => {
      push = onUpdate;
      return Promise.resolve(initial);
    }),
    dismissActivity: vi.fn(() => Promise.resolve()),
    clearActivityHistory: vi.fn(() => Promise.resolve()),
    setQuietMode: vi.fn(() => Promise.resolve()),
    setActivitySource: vi.fn(() => Promise.resolve()),
    getClaudeIntegration: vi.fn(() => Promise.resolve(integration)),
    installClaudeIntegration: vi.fn(() =>
      Promise.resolve({ ...integration, state: "installed" as const }),
    ),
    removeClaudeIntegration: vi.fn(() => Promise.resolve(integration)),
    ...overrides,
  };
  const store = createActivityStore(deps);
  stores.push(store);
  store.getState().start();
  return { store, deps, push: (u: ActivityUpdate) => push?.(u) };
}

const flush = () => vi.advanceTimersByTimeAsync(0);

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  stores.splice(0).forEach((s) => {
    s.dispose();
  });
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("connection and snapshots", () => {
  it("connects once, applies newer snapshots and ignores stale ones", async () => {
    const { store, deps, push } = setup();
    store.getState().start();
    await flush();
    expect(deps.connectActivity).toHaveBeenCalledTimes(1);
    expect(store.getState().connection).toBe("connected");
    push({ type: "snapshot", snapshot: snapshot({ revision: 5, events: [event()] }) });
    expect(store.getState().snapshot?.events).toHaveLength(1);
    push({ type: "snapshot", snapshot: snapshot({ revision: 4 }) });
    expect(store.getState().snapshot?.revision).toBe(5);
  });

  it("keeps a snapshot pushed before the connect response", async () => {
    let resolve!: (s: ActivitySnapshot) => void;
    const { store } = setup({
      connectActivity: vi.fn((onUpdate: (u: ActivityUpdate) => void) => {
        setTimeout(() => {
          onUpdate({ type: "snapshot", snapshot: snapshot({ revision: 3, events: [event()] }) });
        }, 0);
        return new Promise<ActivitySnapshot>((r) => (resolve = r));
      }),
    });
    await flush();
    resolve(snapshot({ revision: 2 }));
    await flush();
    expect(store.getState().snapshot?.revision).toBe(3);
    expect(store.getState().snapshot?.events).toHaveLength(1);
  });

  it("is unavailable outside the desktop app", () => {
    const { store, deps } = setup({ available: () => false });
    expect(store.getState().connection).toBe("unavailable");
    expect(deps.connectActivity).not.toHaveBeenCalled();
  });
});

describe("peeks", () => {
  it("hands peek requests to listeners without storing them", async () => {
    const { store, push } = setup();
    await flush();
    const seen: string[] = [];
    const off = store.getState().onPeek((p) => seen.push(p.event.title));
    push({ type: "peek", peek: peekRequest(event({ title: "Build failed" })) });
    expect(seen).toEqual(["Build failed"]);
    off();
    push({ type: "peek", peek: peekRequest(event()) });
    expect(seen).toHaveLength(1);
    // A reconnect never replays peeks: they are not part of the snapshot.
    expect(JSON.stringify(store.getState().snapshot)).not.toContain("Build failed");
  });
});

describe("commands are not optimistic", () => {
  it("quiet mode stays pending until Rust confirms it in a snapshot", async () => {
    const { store, deps, push } = setup();
    await flush();
    store.getState().setQuiet(true);
    store.getState().setQuiet(true); // repeated press while pending
    expect(deps.setQuietMode).toHaveBeenCalledTimes(1);
    expect(store.getState().pending.quiet).toBe(true);
    expect(store.getState().snapshot?.settings.quiet).toBe(false);
    push({
      type: "snapshot",
      snapshot: snapshot({
        revision: 2,
        settings: { ...DEFAULT_ACTIVITY_SETTINGS, quiet: true },
      }),
    });
    expect(store.getState().pending.quiet).toBeUndefined();
    expect(store.getState().snapshot?.settings.quiet).toBe(true);
  });

  it("clears pending when a command fails", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { store } = setup({ setActivitySource: vi.fn(() => Promise.reject(new Error("x"))) });
    await flush();
    store.getState().setSource("developer", false);
    expect(store.getState().pending["source:developer"]).toBe(true);
    await flush();
    expect(store.getState().pending["source:developer"]).toBeUndefined();
  });

  it("forwards dismiss and clear", async () => {
    const { store, deps } = setup();
    await flush();
    store.getState().dismiss("a1f");
    store.getState().clearHistory();
    expect(deps.dismissActivity).toHaveBeenCalledWith("a1f");
    expect(deps.clearActivityHistory).toHaveBeenCalledTimes(1);
  });
});

describe("Claude Code integration", () => {
  it("reads the status on start and installs only when asked", async () => {
    const { store, deps } = setup();
    await flush();
    expect(deps.getClaudeIntegration).toHaveBeenCalledTimes(1);
    expect(deps.installClaudeIntegration).not.toHaveBeenCalled();
    expect(store.getState().integration.status?.state).toBe("notInstalled");
    await store.getState().installIntegration();
    expect(store.getState().integration).toMatchObject({ busy: false, error: null });
    expect(store.getState().integration.status?.state).toBe("installed");
  });

  it("explains failures without changing the known status", async () => {
    const { store } = setup({
      installClaudeIntegration: vi.fn(() =>
        Promise.reject(new NativeError("install_claude_integration", "invalid", "x")),
      ),
    });
    await flush();
    await store.getState().installIntegration();
    const view = store.getState().integration;
    expect(view.status?.state).toBe("notInstalled");
    expect(view.error).toMatch(/couldn't be read safely/);
  });
});
