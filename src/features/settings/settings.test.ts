import { describe, expect, it, vi } from "vitest";

import type { Settings, SettingsPush, SettingsState } from "../../services/types";
import { NativeError } from "../../services/native";
import fixture from "./__fixtures__/default-settings.json";
import { DEFAULT_SETTINGS } from "./defaults";
import { applyAppearance, motionProfile, resolveReducedMotion } from "./appearance";
import { moveWidget, normalizeSettings } from "./settings-schema";
import { createSettingsStore } from "./settings-store";
import { recordKey } from "./shortcut-keys";
import { formatAgo, formatSize } from "./format";

const clone = (): Settings => structuredClone(DEFAULT_SETTINGS);

describe("settings schema", () => {
  it("defaults match the fixture Rust is tested against", () => {
    expect(DEFAULT_SETTINGS).toEqual(fixture);
    expect(normalizeSettings(clone())).toEqual(DEFAULT_SETTINGS);
  });

  it("clamps numbers like Rust", () => {
    const s = clone();
    s.appearance.opacity = 5;
    s.island.topMargin = 999;
    s.island.offsetX = -1e9;
    s.interaction.hoverDelayMs = Number.NaN;
    s.interaction.collapseDelayMs = 50_000;
    const n = normalizeSettings(s);
    expect(n.appearance.opacity).toBe(70);
    expect(n.island.topMargin).toBe(48);
    expect(n.island.offsetX).toBe(-2000);
    expect(n.interaction.hoverDelayMs).toBe(150);
    expect(n.interaction.collapseDelayMs).toBe(1000);
  });

  it("keeps the island openable with the pointer", () => {
    const s = clone();
    s.interaction.openOnHover = false;
    s.interaction.openOnClick = false;
    expect(normalizeSettings(s).interaction.openOnClick).toBe(true);
  });

  it("repairs the widget list and invalid strings", () => {
    const s = clone();
    s.widgets.items = [
      { id: "audio", enabled: true },
      { id: "audio", enabled: false },
      { id: "media", enabled: false },
    ];
    s.general.toggleShortcut = "  ";
    s.island.monitor = "bad\u0007name";
    const n = normalizeSettings(s);
    expect(n.widgets.items.map((w) => `${w.id}:${String(w.enabled)}`)).toEqual([
      "audio:true",
      "media:false",
      "calendar:true",
      "activity:true",
      "system:false",
    ]);
    expect(n.general.toggleShortcut).toBe("Ctrl+Alt+Space");
    expect(n.island.monitor).toBe("primary");
  });

  it("moves widgets within bounds", () => {
    const items = DEFAULT_SETTINGS.widgets.items;
    expect(moveWidget(items, 2, 0).map((w) => w.id)).toEqual([
      "activity",
      "media",
      "calendar",
      "system",
      "audio",
    ]);
    expect(moveWidget(items, 4, 99).map((w) => w.id)).toEqual(items.map((w) => w.id));
    expect(moveWidget(items, -1, 0)).toEqual(items);
  });
});

describe("settings store", () => {
  const state = (settings: Settings): SettingsState => ({
    settings,
    loadStatus: { kind: "loaded" },
    saveError: false,
  });

  function setup(update = vi.fn((next: Settings) => Promise.resolve(state(next)))) {
    let push: ((p: SettingsPush) => void) | null = null;
    const store = createSettingsStore({
      native: true,
      connect: (onPush) => {
        push = onPush;
        return Promise.resolve(state(DEFAULT_SETTINGS));
      },
      update,
      reset: () => Promise.resolve(state(DEFAULT_SETTINGS)),
    });
    store.getState().start();
    return { store, update, push: (p: SettingsPush) => push?.(p) };
  }

  it("applies updates optimistically and keeps what Rust stored", async () => {
    const { store, update } = setup();
    await Promise.resolve();
    const done = store
      .getState()
      .update((s) => ({ ...s, island: { ...s.island, topMargin: 300 } }));
    expect(store.getState().state.settings.island.topMargin).toBe(48);
    await done;
    expect(update).toHaveBeenCalledOnce();
    expect(store.getState().state.settings.island.topMargin).toBe(48);
  });

  it("rolls back and reports a rejected update", async () => {
    const { store } = setup(
      vi.fn(() => Promise.reject(new NativeError("update_settings", "invalid", "nope"))),
    );
    await Promise.resolve();
    await store
      .getState()
      .update((s) => ({ ...s, appearance: { ...s.appearance, theme: "frost" } }));
    expect(store.getState().state.settings.appearance.theme).toBe("midnight");
    expect(store.getState().error).toBe("nope");
  });

  it("follows changes made in another window and navigation requests", async () => {
    const { store, push } = setup();
    await Promise.resolve();
    const pages: string[] = [];
    const off = store.getState().onNavigate((p) => pages.push(p));
    const other = clone();
    other.appearance.accent = "violet";
    push({ type: "state", state: state(other) });
    push({ type: "navigate", page: "about" });
    off();
    push({ type: "navigate", page: "privacy" });
    expect(store.getState().state.settings.appearance.accent).toBe("violet");
    expect(pages).toEqual(["about"]);
  });

  it("works in memory outside the desktop app", async () => {
    const store = createSettingsStore({
      native: false,
      connect: () => Promise.reject(new Error("no")),
      update: () => Promise.reject(new Error("no")),
      reset: () => Promise.reject(new Error("no")),
    });
    store.getState().start();
    expect(store.getState().connection).toBe("browser");
    await store.getState().update((s) => ({ ...s, island: { ...s.island, anchor: "topRight" } }));
    expect(store.getState().state.settings.island.anchor).toBe("topRight");
    await store.getState().reset();
    expect(store.getState().state.settings).toEqual(DEFAULT_SETTINGS);
  });
});

describe("appearance", () => {
  it("applies tokens through attributes only", () => {
    const root = document.createElement("html");
    applyAppearance(root, {
      ...DEFAULT_SETTINGS.appearance,
      theme: "graphite",
      accent: "blue",
      opacity: 85,
      corners: "round",
    });
    expect(root.dataset).toMatchObject({ theme: "graphite", accent: "blue", corners: "round" });
    expect(root.style.getPropertyValue("--island-alpha")).toBe("0.85");
    applyAppearance(root, { ...DEFAULT_SETTINGS.appearance, opacity: 10 });
    expect(root.style.getPropertyValue("--island-alpha")).toBe("0.7");
  });

  it("resolves reduced motion and intensity", () => {
    expect(resolveReducedMotion("system", true)).toBe(true);
    expect(resolveReducedMotion("system", false)).toBe(false);
    expect(resolveReducedMotion("on", false)).toBe(true);
    expect(resolveReducedMotion("off", true)).toBe(false);
    expect(motionProfile("full", true).expand).toEqual({ duration: 0 });
    expect(motionProfile("subtle", false).expand).toMatchObject({ bounce: 0 });
    expect(motionProfile("full", false).expand).toMatchObject({ bounce: 0.14 });
  });
});

describe("shortcut recorder keys", () => {
  const key = (
    code: string,
    mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {},
  ) => ({
    key: "",
    code,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...mods,
  });

  it("builds accelerators Rust accepts", () => {
    expect(recordKey(key("Space", { ctrlKey: true, altKey: true }))).toEqual({
      kind: "done",
      accelerator: "Ctrl+Alt+Space",
    });
    expect(recordKey(key("KeyI", { ctrlKey: true, shiftKey: true }))).toEqual({
      kind: "done",
      accelerator: "Ctrl+Shift+I",
    });
    expect(recordKey(key("Digit1", { altKey: true, metaKey: true }))).toEqual({
      kind: "done",
      accelerator: "Alt+Win+1",
    });
  });

  it("waits for a key, cancels on Esc and refuses unsafe combinations", () => {
    expect(recordKey(key("ControlLeft", { ctrlKey: true }))).toEqual({
      kind: "pending",
      preview: "Ctrl + …",
    });
    expect(recordKey(key("Escape"))).toEqual({ kind: "cancel" });
    expect(recordKey(key("KeyA")).kind).toBe("invalid");
    expect(recordKey(key("KeyA", { shiftKey: true })).kind).toBe("invalid");
    expect(recordKey(key("NumLock", { ctrlKey: true })).kind).toBe("invalid");
  });
});

describe("formatting", () => {
  it("formats ages and sizes", () => {
    const now = 1_700_000_000_000;
    expect(formatAgo(now - 10_000, now)).toBe("just now");
    expect(formatAgo(now - 5 * 60_000, now)).toBe("5 min ago");
    expect(formatAgo(now - 3 * 3_600_000, now)).toBe("3 h ago");
    expect(formatSize(null)).toBe("—");
    expect(formatSize(512)).toBe("512 B");
    expect(formatSize(1536)).toBe("1.5 KB");
    expect(formatSize(3 * 1024 * 1024)).toBe("3.0 MB");
  });
});
