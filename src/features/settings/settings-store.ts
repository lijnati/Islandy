import { createStore, useStore } from "zustand";

import type { Settings, SettingsPageId, SettingsPush, SettingsState } from "../../services/types";
import { DEFAULT_SETTINGS } from "./defaults";
import { normalizeSettings } from "./settings-schema";
import {
  connectSettings,
  isNativeAvailable,
  resetSettings,
  updateSettings,
} from "./settings-service";

/**
 * Settings shared by every window. Rust owns them: each window connects a
 * channel and receives every change (so the island reacts immediately to the
 * settings window). Updates are applied optimistically after local
 * normalization, then replaced by what Rust stored.
 */
export interface SettingsStoreState {
  state: SettingsState;
  connection: "idle" | "connecting" | "connected" | "browser" | "error";
  /** The last update was rejected (shown in the settings window). */
  error: string | null;
  start: () => void;
  update: (change: (draft: Settings) => Settings) => Promise<void>;
  reset: () => Promise<void>;
  onNavigate: (listener: (page: SettingsPageId) => void) => () => void;
}

export const initialSettingsState: SettingsState = {
  settings: DEFAULT_SETTINGS,
  loadStatus: { kind: "fresh" },
  saveError: false,
};

export interface SettingsStoreDeps {
  native: boolean;
  connect: typeof connectSettings;
  update: typeof updateSettings;
  reset: typeof resetSettings;
}

const defaultDeps = (): SettingsStoreDeps => ({
  native: isNativeAvailable(),
  connect: connectSettings,
  update: updateSettings,
  reset: resetSettings,
});

function message(error: unknown): string {
  return error instanceof Error ? error.message.replace(/^[a-z_]+: /, "") : String(error);
}

export function createSettingsStore(deps: SettingsStoreDeps = defaultDeps()) {
  const navigateListeners = new Set<(page: SettingsPageId) => void>();
  /** The last state Rust confirmed (rollback target). */
  let confirmed: SettingsState = initialSettingsState;
  /** Updates are serialized so responses can't arrive out of order. */
  let queue: Promise<unknown> = Promise.resolve();

  return createStore<SettingsStoreState>()((set, get) => {
    const onPush = (push: SettingsPush) => {
      if (push.type === "state") {
        confirmed = push.state;
        set({ state: push.state });
      } else {
        for (const listener of navigateListeners) listener(push.page);
      }
    };

    return {
      state: initialSettingsState,
      connection: "idle",
      error: null,
      start: () => {
        if (get().connection !== "idle") return;
        if (!deps.native) {
          set({ connection: "browser" });
          return;
        }
        set({ connection: "connecting" });
        deps.connect(onPush).then(
          (state) => {
            confirmed = state;
            set({ state, connection: "connected" });
          },
          (error: unknown) => {
            console.warn("[islandy] connect_settings failed:", error);
            set({ connection: "error" });
          },
        );
      },
      update: (change) => {
        const next = normalizeSettings(change(structuredClone(get().state.settings)));
        set({ state: { ...get().state, settings: next }, error: null });
        if (!deps.native) return Promise.resolve();
        const run = queue.then(
          () => deps.update(next),
          () => deps.update(next),
        );
        queue = run;
        return run.then(
          (state) => {
            confirmed = state;
            set({ state });
          },
          (error: unknown) => {
            set({ state: confirmed, error: message(error) });
          },
        );
      },
      reset: () => {
        if (!deps.native) {
          set({ state: { ...get().state, settings: DEFAULT_SETTINGS }, error: null });
          return Promise.resolve();
        }
        return deps.reset().then(
          (state) => {
            confirmed = state;
            set({ state, error: null });
          },
          (error: unknown) => {
            set({ error: message(error) });
          },
        );
      },
      onNavigate: (listener) => {
        navigateListeners.add(listener);
        return () => {
          navigateListeners.delete(listener);
        };
      },
    };
  });
}

export const settingsStore = createSettingsStore();

export function useSettings<T>(selector: (s: Settings) => T): T {
  return useStore(settingsStore, (s) => selector(s.state.settings));
}

export function useSettingsStore<T>(selector: (s: SettingsStoreState) => T): T {
  return useStore(settingsStore, selector);
}
