/**
 * The only module allowed to talk to the Rust backend.
 *
 * UI code imports typed functions from here instead of calling `invoke`
 * directly, so the IPC surface stays small, typed and easy to audit.
 */
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

import type {
  ActivitySettings,
  PermissionDecision,
  SoundVolume,
  ActivitySnapshot,
  ActivitySource,
  ActivityUpdate,
  AppInfo,
  ClaudeIntegration,
  IslandConnection,
  IslandLayouts,
  IslandSignal,
  IslandyFolder,
  MediaAction,
  MediaSnapshot,
  MonitorInfo,
  NativeErrorKind,
  PlaceResult,
  NativeErrorPayload,
  RadioKind,
  Settings,
  SettingsPage,
  SettingsPageId,
  SettingsPush,
  SettingsState,
  ShortcutStatus,
  Size,
  StartupStatus,
  StorageInfo,
  SystemFocus,
  SystemSnapshot,
  SystemUpdate,
  UpdateStatus,
  WeatherSnapshot,
} from "./types";

export class NativeError extends Error {
  readonly kind: NativeErrorKind | "unknown";
  readonly command: string;

  constructor(command: string, kind: NativeError["kind"], message: string) {
    super(`${command}: ${message}`);
    this.name = "NativeError";
    this.command = command;
    this.kind = kind;
  }
}

function isNativeErrorPayload(value: unknown): value is NativeErrorPayload {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value &&
    typeof value.kind === "string" &&
    typeof value.message === "string"
  );
}

function toNativeError(command: string, cause: unknown): NativeError {
  if (isNativeErrorPayload(cause)) {
    return new NativeError(command, cause.kind, cause.message);
  }
  // Tauri rejects with a plain string for unknown commands / denied permissions.
  if (typeof cause === "string") {
    return new NativeError(command, "unknown", cause);
  }
  return new NativeError(command, "unknown", cause instanceof Error ? cause.message : "unknown");
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new NativeError(command, "unavailable", "not running inside the Islandy desktop shell");
  }
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toNativeError(command, cause);
  }
}

export function getAppInfo(): Promise<AppInfo> {
  return call<AppInfo>("get_app_info");
}

/** Whether native commands can be called (false in a plain browser / tests). */
export function isNativeAvailable(): boolean {
  return isTauri();
}

/**
 * Registers the handler for Rust → frontend signals. Calling it again
 * replaces the previous handler (Rust keeps only the latest channel).
 */
export function connectIsland(onSignal: (signal: IslandSignal) => void): Promise<IslandConnection> {
  if (!isTauri()) {
    return Promise.reject(
      new NativeError(
        "connect_island",
        "unavailable",
        "not running inside the Islandy desktop shell",
      ),
    );
  }
  const channel = new Channel<IslandSignal>();
  channel.onmessage = onSignal;
  return call<IslandConnection>("connect_island", { channel });
}

/** Layout sizes (logical px) fitted to the island's monitor. */
export function getIslandLayouts(): Promise<IslandLayouts> {
  return call<IslandLayouts>("get_island_layouts");
}

/** Sets the native frame (hit/visible area) to `size` logical px. */
export function setIslandFrame(size: Size): Promise<void> {
  return call<undefined>("set_island_frame", { width: size.width, height: size.height });
}

/** Lets the island take keyboard focus, or returns it to non-activating mode. */
export function setIslandInteractive(interactive: boolean): Promise<void> {
  return call<undefined>("set_island_interactive", { interactive });
}

/** Opens a fixed Windows Settings page (Windows only). */
export function openSystemSettings(page: SettingsPage): Promise<void> {
  return call<undefined>("open_system_settings", { page });
}

/* ------------------------------------------------------------------------ */
/* Media                                                                     */
/* ------------------------------------------------------------------------ */

/**
 * Registers the media snapshot handler and returns the current snapshot.
 * Calling it again replaces the previous handler (Rust keeps one channel).
 */
export function connectMedia(
  onSnapshot: (snapshot: MediaSnapshot) => void,
): Promise<MediaSnapshot> {
  if (!isTauri()) {
    return Promise.reject(
      new NativeError(
        "connect_media",
        "unavailable",
        "not running inside the Islandy desktop shell",
      ),
    );
  }
  const channel = new Channel<MediaSnapshot>();
  channel.onmessage = onSnapshot;
  return call<MediaSnapshot>("connect_media", { channel });
}

export function getMediaSnapshot(): Promise<MediaSnapshot> {
  return call<MediaSnapshot>("get_media_snapshot");
}

/** `null` returns to automatic session selection. */
export function selectMediaSession(sessionId: string | null): Promise<MediaSnapshot> {
  return call<MediaSnapshot>("select_media_session", { sessionId });
}

/** Sends a playback command; the confirmed effect arrives as a later snapshot. */
export function mediaControl(sessionId: string, action: MediaAction): Promise<MediaSnapshot> {
  return call<MediaSnapshot>("media_control", { sessionId, action });
}

export function mediaSeek(sessionId: string, positionMs: number): Promise<MediaSnapshot> {
  return call<MediaSnapshot>("media_seek", { sessionId, positionMs });
}

/** A `data:image/jpeg` URL for an artwork key, or `null` if no longer cached. */
export function getMediaArtwork(key: string): Promise<string | null> {
  return call<string | null>("get_media_artwork", { key });
}

/* ------------------------------------------------------------------------ */
/* System                                                                    */
/* ------------------------------------------------------------------------ */

/**
 * Registers the system update handler and returns the current snapshot.
 * Calling it again replaces the previous handler (Rust keeps one channel).
 */
export function connectSystem(onUpdate: (update: SystemUpdate) => void): Promise<SystemSnapshot> {
  if (!isTauri()) {
    return Promise.reject(
      new NativeError(
        "connect_system",
        "unavailable",
        "not running inside the Islandy desktop shell",
      ),
    );
  }
  const channel = new Channel<SystemUpdate>();
  channel.onmessage = onUpdate;
  return call<SystemSnapshot>("connect_system", { channel });
}

export function getSystemSnapshot(): Promise<SystemSnapshot> {
  return call<SystemSnapshot>("get_system_snapshot");
}

/** Starts/stops CPU/RAM sampling and brightness polling. */
export function setSystemFocus(focus: SystemFocus): Promise<void> {
  return call<undefined>("set_system_focus", { focus });
}

/** 0–100. Resolves once Windows accepted it; the new state arrives as an update. */
export function setMasterVolume(volume: number): Promise<void> {
  return call<undefined>("set_master_volume", { volume });
}

export function setAudioMuted(muted: boolean): Promise<void> {
  return call<undefined>("set_audio_muted", { muted });
}

export function setRadioState(radio: RadioKind, on: boolean): Promise<void> {
  return call<undefined>("set_radio_state", { radio, on });
}

export function setDisplayBrightness(displayId: string, brightness: number): Promise<void> {
  return call<undefined>("set_display_brightness", { displayId, brightness });
}

/* ------------------------------------------------------------------------ */
/* Activity                                                                  */
/* ------------------------------------------------------------------------ */

/**
 * Registers the activity handler and returns the latest snapshot.
 * Calling it again replaces the previous handler (Rust keeps one channel).
 */
export function connectActivity(
  onUpdate: (update: ActivityUpdate) => void,
): Promise<ActivitySnapshot> {
  if (!isTauri()) {
    return Promise.reject(
      new NativeError(
        "connect_activity",
        "unavailable",
        "not running inside the Islandy desktop shell",
      ),
    );
  }
  const channel = new Channel<ActivityUpdate>();
  channel.onmessage = onUpdate;
  return call<ActivitySnapshot>("connect_activity", { channel });
}

/** The current activity snapshot (the settings window reads; it doesn't subscribe). */
export function getActivitySnapshot(): Promise<ActivitySnapshot> {
  return call<ActivitySnapshot>("get_activity_snapshot");
}

/** Dismisses an active event, or removes a finished one from history. */
export function dismissActivity(id: string): Promise<void> {
  return call<undefined>("dismiss_activity", { id });
}

/** Plays the notification chime once (settings preview). */
export function previewNotificationSound(volume: SoundVolume): Promise<void> {
  return call<undefined>("preview_notification_sound", { volume });
}

/** Answers a Claude Code permission request shown on the island. */
export function answerPermission(
  session: string,
  question: number,
  decision: PermissionDecision,
): Promise<void> {
  return call<undefined>("answer_permission", { session, question, decision });
}

export function clearActivityHistory(): Promise<void> {
  return call<undefined>("clear_activity_history");
}

/** Islandy's own quiet mode (not Windows Do Not Disturb). */
export function setQuietMode(quiet: boolean): Promise<void> {
  return call<undefined>("set_quiet_mode", { quiet });
}

export function setActivitySource(source: ActivitySource, enabled: boolean): Promise<void> {
  return call<undefined>("set_activity_source", { source, enabled });
}

/** Reads (never modifies) the user-level Claude Code settings. */
export function getClaudeIntegration(): Promise<ClaudeIntegration> {
  return call<ClaudeIntegration>("get_claude_integration");
}

/** Adds Islandy's hooks to `~/.claude/settings.json` (explicit user action). */
export function installClaudeIntegration(): Promise<ClaudeIntegration> {
  return call<ClaudeIntegration>("install_claude_integration");
}

/** Removes Islandy's hooks, and nothing else. */
export function removeClaudeIntegration(): Promise<ClaudeIntegration> {
  return call<ClaudeIntegration>("remove_claude_integration");
}

/** All activity settings at once (normalized in Rust). */
export function setActivitySettings(settings: ActivitySettings): Promise<void> {
  return call<undefined>("set_activity_settings", { settings });
}

/** Sends a harmless test message through the real local receiver. */
export function testLocalReceiver(): Promise<void> {
  return call<undefined>("test_local_receiver");
}

/* ------------------------------------------------------------------------ */
/* Settings, startup, diagnostics                                             */
/* ------------------------------------------------------------------------ */

/**
 * Registers this window's settings channel and returns the current state.
 * Every window (island, settings) receives every change.
 */
export function connectSettings(onPush: (push: SettingsPush) => void): Promise<SettingsState> {
  if (!isTauri()) {
    return Promise.reject(
      new NativeError(
        "connect_settings",
        "unavailable",
        "not running inside the Islandy desktop shell",
      ),
    );
  }
  const channel = new Channel<SettingsPush>();
  channel.onmessage = onPush;
  return call<SettingsState>("connect_settings", { channel });
}

export function updateSettings(next: Settings): Promise<SettingsState> {
  return call<SettingsState>("update_settings", { next });
}

export function resetSettings(): Promise<SettingsState> {
  return call<SettingsState>("reset_settings");
}

/** Registers a new toggle shortcut; the old one stays active on failure. */
export function setToggleShortcut(accelerator: string): Promise<ShortcutStatus> {
  return call<ShortcutStatus>("set_toggle_shortcut", { accelerator });
}

export function getShortcutStatus(): Promise<ShortcutStatus> {
  return call<ShortcutStatus>("get_shortcut_status");
}

export function getStartupStatus(): Promise<StartupStatus> {
  return call<StartupStatus>("get_startup_status");
}

export function setLaunchAtLogin(enabled: boolean): Promise<StartupStatus> {
  return call<StartupStatus>("set_launch_at_login", { enabled });
}

export function listMonitors(): Promise<MonitorInfo[]> {
  return call<MonitorInfo[]>("list_monitors");
}

/** Live offset while dragging; `null` ends the preview. Returns the applied offset. */
export function previewIslandOffset(offset: number | null): Promise<number> {
  return call<number>("preview_island_offset", { offset });
}

export function setIslandVisible(visible: boolean): Promise<void> {
  return call<undefined>("set_island_visible", { visible });
}

export function openSettingsWindow(page?: SettingsPageId): Promise<void> {
  return call<undefined>("open_settings_window", { page: page ?? null });
}

export function openIslandyFolder(folder: IslandyFolder): Promise<void> {
  return call<undefined>("open_islandy_folder", { folder });
}

export function getStorageInfo(): Promise<StorageInfo> {
  return call<StorageInfo>("get_storage_info");
}

/** Plain-text diagnostics summary (no user activity, no secrets). */
export function getDiagnostics(): Promise<string> {
  return call<string>("get_diagnostics");
}

/* ------------------------------------------------------------------------ */
/* Weather and updates                                                        */
/* ------------------------------------------------------------------------ */

/** The forecast for the chosen place (cached 30 min; `force` refreshes). */
export function getWeather(force = false): Promise<WeatherSnapshot> {
  return call<WeatherSnapshot>("get_weather", { force });
}

export function searchWeatherPlaces(query: string): Promise<PlaceResult[]> {
  return call<PlaceResult[]>("search_weather_places", { query });
}

/** One read of the device location (asks Windows; call only on a click). */
export function weatherLocate(): Promise<PlaceResult> {
  return call<PlaceResult>("weather_locate");
}

export function getUpdateStatus(): Promise<UpdateStatus> {
  return call<UpdateStatus>("get_update_status");
}

export function checkForUpdates(): Promise<UpdateStatus> {
  return call<UpdateStatus>("check_for_updates");
}

/** Installs the found update and restarts Islandy (resolves only if it couldn't). */
export function installUpdate(): Promise<UpdateStatus> {
  return call<UpdateStatus>("install_update");
}
