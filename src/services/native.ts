/**
 * The only module allowed to talk to the Rust backend.
 *
 * UI code imports typed functions from here instead of calling `invoke`
 * directly, so the IPC surface stays small, typed and easy to audit.
 */
import { invoke, isTauri } from "@tauri-apps/api/core";

import type { AppInfo, NativeErrorKind, NativeErrorPayload } from "./types";

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
