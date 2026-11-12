import { createStore, useStore } from "zustand";

import type { AppInfo, IslandConnection } from "../../services/types";

/** What the native side reported about itself. Written by useNativeBridge. */
interface NativeStatus {
  connection: IslandConnection | null;
  appInfo: AppInfo | null;
}

export const nativeStatusStore = createStore<NativeStatus>()(() => ({
  connection: null,
  appInfo: null,
}));

export function useNativeStatus<T>(selector: (s: NativeStatus) => T): T {
  return useStore(nativeStatusStore, selector);
}

/** Windows Settings pages can only be opened inside the desktop app on Windows. */
export function useCanOpenSystemSettings(): boolean {
  return useNativeStatus((s) => s.appInfo?.os === "windows");
}
