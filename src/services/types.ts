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
  "window" | "monitor" | "invalid" | "unsupported" | "platform" | "internal";

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
