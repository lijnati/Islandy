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
  | "denied";

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
}

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
}

/** Rust → frontend push messages. Mirrors `signals::IslandSignal`. */
export type IslandSignal =
  { type: "toggle" } | { type: "focusLost" } | { type: "layoutsChanged" } | { type: "demoPeek" };

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
  | { type: "audioOutputChanged"; name: string };

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
  | "audioOutputChanged";

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
}

export interface SourceToggles {
  claudeCode: boolean;
  developer: boolean;
  system: boolean;
}

export interface ActivitySettings {
  quiet: boolean;
  sources: SourceToggles;
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
