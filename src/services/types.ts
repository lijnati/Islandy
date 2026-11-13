/**
 * TypeScript mirrors of the Rust IPC types (`src-tauri/src/services`, `commands.rs`,
 * `signals.rs`, `shortcut.rs`, `window/` and `error.rs`). Rust serializes with `camelCase` field names.
 * Keep these in sync when changing either side.
 */

export interface AppInfo {
  name: string;
  version: string;
  identifier: string;
  /** `std::env::consts::OS`, e.g. "windows". */
  os: string;
  /** `std::env::consts::ARCH`, e.g. "x86_64". */
  arch: string;
  tauriVersion: string;
  /** WebView2 (Windows) / WebKitGTK (Linux) version, when it can be queried. */
  webviewVersion: string | null;
  debug: boolean;
}

/** Error categories produced by the Rust side (`IslandyError`). */
export type NativeErrorKind =
  | "window"
  | "monitor"
  | "invalid"
  | "unsupported"
  | "platform"
  | "internal"
  | "unavailable"
  | "notFound"
  | "rejected"
  | "timeout"
  | "denied"
  | "shortcut"
  | "network";

export interface NativeErrorPayload {
  kind: NativeErrorKind;
  message: string;
}

/** A size in logical (CSS) pixels. Mirrors `geometry::LogicalSize`. */
export interface Size {
  width: number;
  height: number;
}

/** Layout sizes fitted to the island's monitor. Mirrors `geometry::Layouts`. */
export interface IslandLayouts {
  collapsed: Size;
  /** The resting pill while ambient activity is shown. */
  ambient: Size;
  peek: Size;
  expanded: Size;
  /** How the surface is aligned in the window (follows the anchor). */
  align?: SurfaceAlign;
}

/** Mirrors `geometry::Align`. */
export type SurfaceAlign = "start" | "center" | "end";

/** Mirrors `shortcut::ShortcutStatus`. */
export interface ShortcutStatus {
  accelerator: string;
  registered: boolean;
  error: string | null;
}

/** Mirrors `window::FrameStrategy`. */
export type FrameStrategy = "region" | "resize";

/** Mirrors `commands::IslandConnection`. */
export interface IslandConnection {
  shortcut: ShortcutStatus;
  frameStrategy: FrameStrategy;
  /** First launch: show a one-time hint. */
  firstRun: boolean;
}

/** Rust → frontend push messages. Mirrors `signals::IslandSignal`. */
export type IslandSignal =
  | { type: "toggle" }
  | { type: "focusLost" }
  | { type: "layoutsChanged" }
  /** Tray left click: open the dashboard. */
  | { type: "open" }
  /** The island is being hidden (tray, fullscreen app): collapse. */
  | { type: "hidden" };

/** Windows Settings pages Islandy may open. Mirrors `system::settings::SettingsPage`. */
export type SettingsPage =
  "home" | "wifi" | "network" | "bluetooth" | "sound" | "display" | "battery";

/* ------------------------------------------------------------------------ */
/* Media (mirrors `src-tauri/src/services/media/models.rs`)                   */
/* ------------------------------------------------------------------------ */

/** Mirrors `models::PlaybackStatus`. */
export type PlaybackStatus = "playing" | "paused" | "stopped" | "changing" | "unknown";

/** What the source app currently accepts. Mirrors `models::MediaControls`. */
export interface MediaControls {
  play: boolean;
  pause: boolean;
  togglePlayPause: boolean;
  next: boolean;
  previous: boolean;
  /** Position changes accepted *and* a known duration. */
  seek: boolean;
}

/** Position reported by the app at `updatedAtMs` (Unix ms). Mirrors `models::Timeline`. */
export interface MediaTimeline {
  positionMs: number;
  durationMs: number | null;
  updatedAtMs: number;
}

/** Mirrors `models::MediaSession` (metadata fields are flattened). */
export interface MediaSession {
  /** Stable for the session's lifetime, e.g. "s3". */
  id: string;
  sourceAppId: string;
  sourceAppName: string;
  title: string | null;
  artist: string | null;
  album: string | null;
  albumArtist: string | null;
  trackNumber: number | null;
  /** Content key; resolve with `getMediaArtwork`. */
  artworkKey: string | null;
  metadataLoaded: boolean;
  playbackStatus: PlaybackStatus;
  playbackRate: number;
  controls: MediaControls;
  timeline: MediaTimeline | null;
  isSystemCurrent: boolean;
}

/** Mirrors `models::IntegrationStatus`. */
export type MediaIntegrationStatus = "starting" | "ready" | "unavailable" | "unsupported";

/** Mirrors `models::MediaSnapshot`. */
export interface MediaSnapshot {
  /** Strictly increasing; older snapshots must be ignored. */
  revision: number;
  status: MediaIntegrationStatus;
  sessions: MediaSession[];
  selectedSessionId: string | null;
  selectionMode: "auto" | "user";
}

/** Mirrors `models::MediaAction`. */
export type MediaAction = "play" | "pause" | "togglePlayPause" | "next" | "previous";

/* ------------------------------------------------------------------------ */
/* System (mirrors `src-tauri/src/services/system/models.rs`)                 */
/* ------------------------------------------------------------------------ */

/** Mirrors `models::Availability`. */
export type Availability = "loading" | "ready" | "unavailable" | "unsupported";

export interface AudioDevice {
  /** Opaque key, stable while the device exists. */
  id: string;
  name: string;
  isDefault: boolean;
}

export interface AudioOutput {
  deviceId: string;
  name: string;
  /** 0–100, the number the Windows volume flyout shows. */
  volume: number;
  muted: boolean;
}

export interface AudioState {
  status: Availability;
  /** `null` when there is no active output device. */
  output: AudioOutput | null;
  devices: AudioDevice[];
}

export type PowerSource = "ac" | "battery" | "unknown";
export type ChargeState = "charging" | "discharging" | "notCharging" | "unknown";

/** Mirrors `models::BatteryInfo` (tagged by `presence`). */
export type BatteryInfo =
  | { presence: "present"; percent: number | null; state: ChargeState }
  | { presence: "absent" }
  | { presence: "unknown" };

export interface PowerState {
  status: Availability;
  source: PowerSource;
  battery: BatteryInfo;
  saver: boolean | null;
}

/** Windows' own connectivity assessment. Mirrors `models::Connectivity`. */
export type Connectivity = "internet" | "constrained" | "local" | "none";
export type ConnectionKind = "wifi" | "ethernet" | "cellular" | "other";

export interface NetworkState {
  status: Availability;
  connectivity: Connectivity;
  connection: { kind: ConnectionKind; name: string | null } | null;
}

export type RadioKind = "wifi" | "bluetooth";
export type RadioState = "on" | "off" | "disabled" | "unknown";

export interface RadioInfo {
  state: RadioState;
  /** Windows lets Islandy switch it, and it is in a switchable state. */
  canToggle: boolean;
}

export interface RadiosState {
  status: Availability;
  /** `null`: no such adapter. */
  wifi: RadioInfo | null;
  bluetooth: RadioInfo | null;
}

export interface MemoryUsage {
  totalBytes: number;
  availableBytes: number;
  usedPercent: number;
}

export interface MetricsState {
  status: Availability;
  /** `null` until two samples exist. */
  cpuPercent: number | null;
  memory: MemoryUsage | null;
  sampledAtMs: number;
  /** `null` while sampling is suspended. */
  intervalMs: number | null;
}

export type DisplayKind = "internal" | "external";

export interface DisplayInfo {
  id: string;
  name: string;
  kind: DisplayKind;
  /** `null` when the display stopped answering. */
  brightness: number | null;
}

export interface DisplayState {
  status: Availability;
  displays: DisplayInfo[];
  /** Connected displays without a supported brightness interface. */
  unsupportedCount: number;
}

/** Mirrors `models::SystemAlert` (tagged by `type`). */
export type SystemAlert =
  | { type: "batteryLow"; percent: number }
  | { type: "batteryCritical"; percent: number }
  | { type: "powerConnected"; percent: number | null }
  | { type: "powerDisconnected"; percent: number | null }
  | { type: "networkLost" }
  | { type: "networkRestored"; kind: ConnectionKind; name: string | null }
  | { type: "audioOutputChanged"; name: string }
  | { type: "updateAvailable"; version: string; automatic: boolean };

export interface SystemSnapshot {
  revision: number;
  audio: AudioState;
  power: PowerState;
  network: NetworkState;
  radios: RadiosState;
  metrics: MetricsState;
  display: DisplayState;
}

export type SystemDomain = "audio" | "power" | "network" | "radios" | "metrics" | "display";

/** One domain's new state, or an alert. Mirrors `models::SystemUpdate`. */
export type SystemUpdate =
  | { domain: "audio"; revision: number; state: AudioState }
  | { domain: "power"; revision: number; state: PowerState }
  | { domain: "network"; revision: number; state: NetworkState }
  | { domain: "radios"; revision: number; state: RadiosState }
  | { domain: "metrics"; revision: number; state: MetricsState }
  | { domain: "display"; revision: number; state: DisplayState }
  | { domain: "alert"; revision: number; alert: SystemAlert };

/** What the UI shows; drives optional sampling. Mirrors `models::SystemFocus`. */
export type SystemFocus = "none" | "overview" | "display";

/* ------------------------------------------------------------------------ */
/* Activity (mirrors `src-tauri/src/services/activity/model.rs`)              */
/* ------------------------------------------------------------------------ */

export type ActivitySource = "claudeCode" | "developer" | "system";
export type ActivitySeverity = "info" | "success" | "warning" | "error" | "attention";
export type ActivityLifecycle = "active" | "resolved" | "dismissed" | "expired";
/** 0 needs attention · 1 important outcome · 2 informational · 3 ambient. */
export type ActivityPriority = 0 | 1 | 2 | 3;

export type ActivityKind =
  | "claudeSessionStarted"
  | "claudeAttention"
  | "claudeWaitingForInput"
  | "claudeTurnFinished"
  | "claudeError"
  | "claudeSessionEnded"
  | "buildStarted"
  | "buildSucceeded"
  | "buildFailed"
  | "testsStarted"
  | "testsPassed"
  | "testsFailed"
  | "serverStarted"
  | "serverStopped"
  | "message"
  | "batteryLow"
  | "batteryCritical"
  | "powerConnected"
  | "powerDisconnected"
  | "networkLost"
  | "networkRestored"
  | "audioOutputChanged"
  | "updateAvailable";

export interface ActivityEvent {
  /** Stable, unique, increasing (`a<hex>`). */
  id: string;
  source: ActivitySource;
  kind: ActivityKind;
  /** Opaque Claude session key (`c<16 hex>`). */
  session: string | null;
  /** A folder name, never a path. */
  project: string | null;
  title: string;
  description: string | null;
  severity: ActivitySeverity;
  priority: ActivityPriority;
  lifecycle: ActivityLifecycle;
  /** Epoch ms, assigned by Islandy. */
  timestamp: number;
  expiresAt: number | null;
  dedupeKey: string | null;
}

export type SessionStatus =
  "ready" | "working" | "attention" | "waiting" | "idle" | "error" | "ended";

export type ToolCategory =
  | "reading"
  | "editing"
  | "searching"
  | "running"
  | "web"
  | "delegating"
  | "planning"
  | "mcp"
  | "other";

export interface ClaudeSession {
  id: string;
  project: string | null;
  status: SessionStatus;
  /** `null`: the session was already running when Islandy first saw it. */
  startedAt: number | null;
  turnStartedAt: number | null;
  lastSeenAt: number;
  lastTool: ToolCategory | null;
  subagents: number;
  attentionEvent: string | null;
  attentionDismissed: boolean;
  endedAt: number | null;
  /** Working, but no event for a long time: status unknown. */
  stale: boolean;
  /** An open permission request answerable on the island (memory only). */
  question: PermissionQuestion | null;
  /** Turns (prompts) seen in this session. */
  turns: number;
  /** Times this session needed the user. */
  neededYou: number;
}

/** Mirrors `model::PermissionQuestion`. */
export interface PermissionQuestion {
  id: number;
  tool: ToolCategory | null;
  /** The command, path, URL or tool, one line. */
  summary: string;
  /** `summary` is the whole request; otherwise only Deny is offered. */
  complete: boolean;
  askedAt: number;
  /** After this, Claude Code asks in its own prompt instead. */
  expiresAt: number;
}

/** Mirrors `protocol::Decision`. */
export type PermissionDecision = "allow" | "deny" | "pass";

/** Claude activity over the last 24 hours (since Islandy started). */
export interface ClaudeUsage {
  turns: number;
  workedMs: number;
  neededYou: number;
  /** Islandy has run for less than the whole day. */
  partial: boolean;
}

export interface SourceToggles {
  claudeCode: boolean;
  developer: boolean;
  system: boolean;
}

/** Which recorded events may also peek. Mirrors `model::PeekToggles`. */
export interface PeekToggles {
  claudeCode: boolean;
  developer: boolean;
  system: boolean;
  media: boolean;
  success: boolean;
  errors: boolean;
}

export type PeekDuration = "short" | "standard" | "long";

export interface ActivitySettings {
  quiet: boolean;
  /** Sources Islandy records at all. */
  sources: SourceToggles;
  peeks: PeekToggles;
  peekDuration: PeekDuration;
  /** 1–30 days. */
  retentionDays: number;
  /** Claude Code permission requests can be answered on the island. */
  answerPermissions: boolean;
  /** A short chime when something new peeks on the island. */
  sound: boolean;
  soundVolume: SoundVolume;
}

/** Mirrors `sound::SoundVolume`. */
export type SoundVolume = "soft" | "normal" | "loud";

export interface LastReceived {
  claudeCode: number | null;
  developer: number | null;
}

export type ReceiverStatus = "starting" | "listening" | "unavailable";

export interface ActivitySnapshot {
  revision: number;
  sessions: ClaudeSession[];
  /** Newest first, bounded. */
  events: ActivityEvent[];
  settings: ActivitySettings;
  receiver: ReceiverStatus;
  historyError: boolean;
  lastReceived: LastReceived;
  usage: ClaudeUsage;
}

export interface PeekRequest {
  event: ActivityEvent;
  dwellMs: number;
}

export type ActivityUpdate =
  { type: "snapshot"; snapshot: ActivitySnapshot } | { type: "peek"; peek: PeekRequest };

/** Mirrors `claude_settings::IntegrationState`. */
export type IntegrationState =
  "notDetected" | "notInstalled" | "installed" | "needsUpdate" | "unreadable";

export interface ClaudeIntegration {
  state: IntegrationState;
  settingsPath: string | null;
  events: string[];
  /** The exact JSON Islandy adds. */
  snippet: string;
}

/* ------------------------------------------------------------------------ */
/* Settings, startup, diagnostics (Phase 5; mirrors services/settings,       */
/* services/startup.rs, commands.rs)                                          */
/* ------------------------------------------------------------------------ */

export type Theme = "midnight" | "graphite" | "frost";
export type Accent = "white" | "blue" | "violet" | "green" | "amber" | "coral";
export type Corners = "standard" | "compact" | "round";
export type AnimationLevel = "full" | "subtle";
export type ReducedMotionSetting = "system" | "on" | "off";
export type Anchor = "topCenter" | "topLeft" | "topRight";
export type SizePreset = "standard" | "compact" | "comfortable";
export type DashboardWidgetId = "media" | "calendar" | "activity" | "system" | "audio" | "weather";
export type TemperatureUnit = "celsius" | "fahrenheit";

export interface GeneralSettings {
  toggleShortcut: string;
  startHiddenAtLogin: boolean;
  hideInFullscreen: boolean;
  verboseLogging: boolean;
}

export interface AppearanceSettings {
  theme: Theme;
  accent: Accent;
  /** 70–100 (%). */
  opacity: number;
  corners: Corners;
  animation: AnimationLevel;
  reducedMotion: ReducedMotionSetting;
}

export interface IslandSettings {
  anchor: Anchor;
  /** Logical px from the anchor. */
  offsetX: number;
  /** 0–48 logical px. */
  topMargin: number;
  /** `"primary"` or a monitor id from `listMonitors`. */
  monitor: string;
  size: SizePreset;
  dragToReposition: boolean;
}

export interface InteractionSettings {
  openOnHover: boolean;
  /** 100–500 ms. */
  hoverDelayMs: number;
  openOnClick: boolean;
  pinOnClick: boolean;
  autoCollapse: boolean;
  /** 200–1000 ms. */
  collapseDelayMs: number;
}

export interface WidgetEntry {
  id: DashboardWidgetId;
  enabled: boolean;
}

/** A place for the weather widget. Mirrors `models::WeatherLocation`. */
export interface WeatherLocation {
  name: string;
  /** Region and/or country ("Illinois, United States"). */
  detail: string;
  /** Rounded to two decimals. */
  latitude: number;
  longitude: number;
}

export interface WeatherSettings {
  location: WeatherLocation | null;
  unit: TemperatureUnit;
}

export interface UpdateSettings {
  autoCheck: boolean;
  autoInstall: boolean;
}

export interface Settings {
  version: number;
  general: GeneralSettings;
  appearance: AppearanceSettings;
  island: IslandSettings;
  interaction: InteractionSettings;
  widgets: { items: WidgetEntry[] };
  weather: WeatherSettings;
  updates: UpdateSettings;
  onboarding: OnboardingSettings;
}

/** Mirrors `OnboardingSettings`: the welcome page was finished or skipped. */
export interface OnboardingSettings {
  welcomed: boolean;
}

/* Weather (mirrors `services/weather.rs`) */

export interface CurrentWeather {
  temperature: number;
  feelsLike: number;
  /** WMO weather code. */
  code: number;
  isDay: boolean;
  humidity: number | null;
  /** km/h (Celsius) or mph (Fahrenheit). */
  windSpeed: number | null;
}

export interface DailyWeather {
  /** `YYYY-MM-DD`, local to the place. */
  date: string;
  code: number;
  high: number;
  low: number;
}

export type WeatherSnapshot =
  | { status: "unconfigured" }
  | { status: "loading"; location: WeatherLocation }
  | { status: "error"; location: WeatherLocation; message: string }
  | {
      status: "ready";
      location: WeatherLocation;
      unit: TemperatureUnit;
      forecast: { current: CurrentWeather; days: DailyWeather[] };
      updatedAtMs: number;
      stale: boolean;
    };

export type PlaceResult = WeatherLocation;

/* Updates (mirrors `services/updates.rs`) */

export type UpdatePhase =
  | "idle"
  | "checking"
  | "upToDate"
  | "available"
  | "downloading"
  | "ready"
  | "installing"
  | "failed";

export interface UpdateStatus {
  phase: UpdatePhase;
  currentVersion: string;
  version: string | null;
  notes: string | null;
  progress: number | null;
  lastCheckedMs: number | null;
  error: string | null;
}

export type SettingsLoadStatus =
  | { kind: "fresh" }
  | { kind: "loaded" }
  | { kind: "migrated"; from: number }
  | { kind: "recovered" }
  | { kind: "newer"; version: number };

export interface SettingsState {
  settings: Settings;
  loadStatus: SettingsLoadStatus;
  /** Saving failed: settings are kept in memory only. */
  saveError: boolean;
}

/** Pages of the settings window. Mirrors `settings_window::SettingsPage`. */
export type SettingsPageId =
  | "general"
  | "appearance"
  | "island"
  | "widgets"
  | "integrations"
  | "notifications"
  | "privacy"
  | "about"
  | "welcome";

export type SettingsPush =
  { type: "state"; state: SettingsState } | { type: "navigate"; page: SettingsPageId };

export interface StartupStatus {
  supported: boolean;
  enabled: boolean;
  /** Registered but switched off in Task Manager / Settings. */
  disabledBySystem: boolean;
}

export interface MonitorInfo {
  id: string;
  label: string;
  primary: boolean;
  width: number;
  height: number;
  scale: number;
  current: boolean;
}

export interface StoredFile {
  name: string;
  purpose: string;
  bytes: number | null;
}

export interface StorageInfo {
  dataDir: string | null;
  files: StoredFile[];
}

export type IslandyFolder = "data" | "logs";
