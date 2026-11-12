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
  | "timeout";

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

/** Windows Settings pages the quick controls may open. Mirrors `SettingsPage`. */
export type SettingsPage = "wifi" | "bluetooth" | "sound" | "display";

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
