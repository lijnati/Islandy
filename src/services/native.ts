/**
 * The only module allowed to talk to the Rust backend.
 *
 * UI code imports typed functions from here instead of calling `invoke`
 * directly, so the IPC surface stays small, typed and easy to audit.
 */
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

import type {
  AppInfo,
  IslandConnection,
  IslandLayouts,
  IslandSignal,
  NativeErrorKind,
  NativeErrorPayload,
  SettingsPage,
  Size,
} from "./types";

export class NativeError extends Error {
  readonly kind: NativeErrorKind | "unavailable" | "unknown";
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
