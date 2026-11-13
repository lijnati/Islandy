/**
 * Default preferences. Mirrors `Default` impls in
 * `src-tauri/src/services/settings/models.rs` (checked by a test).
 */
import type { ActivitySettings, Settings } from "../../services/types";

export const DEFAULT_SHORTCUT = "Ctrl+Alt+Space";

export const DEFAULT_SETTINGS: Settings = {
  version: 2,
  general: {
    toggleShortcut: DEFAULT_SHORTCUT,
    startHiddenAtLogin: false,
    hideInFullscreen: true,
    verboseLogging: false,
  },
  appearance: {
    theme: "midnight",
    accent: "white",
    opacity: 100,
    corners: "standard",
    animation: "full",
    reducedMotion: "system",
  },
  island: {
    anchor: "topCenter",
    offsetX: 0,
    topMargin: 8,
    monitor: "primary",
    size: "standard",
    dragToReposition: false,
  },
  interaction: {
    openOnHover: true,
    hoverDelayMs: 150,
    openOnClick: true,
    pinOnClick: true,
    autoCollapse: false,
    collapseDelayMs: 360,
  },
  widgets: {
    items: [
      { id: "media", enabled: true },
      { id: "calendar", enabled: true },
      { id: "activity", enabled: true },
      { id: "system", enabled: false },
      { id: "audio", enabled: false },
      { id: "weather", enabled: false },
    ],
  },
  weather: { location: null, unit: "celsius" },
  updates: { autoCheck: true, autoInstall: true },
  // Fresh installs see the welcome page; Rust migrates version 1 files (every
  // existing install) to `true`.
  onboarding: { welcomed: false },
};

/** Mirrors `ActivitySettings::default()` in `services/activity/model.rs`. */
export const DEFAULT_ACTIVITY_SETTINGS: ActivitySettings = {
  quiet: false,
  sources: { claudeCode: true, developer: true, system: true },
  peeks: {
    claudeCode: true,
    developer: true,
    system: true,
    media: true,
    success: true,
    errors: true,
  },
  peekDuration: "standard",
  retentionDays: 7,
  answerPermissions: false,
};
