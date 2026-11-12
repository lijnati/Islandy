/**
 * TypeScript mirrors of the Rust IPC types in `src-tauri/src/services/app_info.rs`
 * and `src-tauri/src/error.rs`. Rust serializes with `camelCase` field names.
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
export type NativeErrorKind = "window" | "monitor" | "platform" | "internal";

export interface NativeErrorPayload {
  kind: NativeErrorKind;
  message: string;
}
