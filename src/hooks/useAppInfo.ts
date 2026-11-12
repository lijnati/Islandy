import { useEffect, useState } from "react";

import { getAppInfo, type NativeError } from "../services/native";
import type { AppInfo } from "../services/types";

export type AppInfoState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo }
  | { status: "error"; error: NativeError };

/** Loads static application/runtime info once from the Rust backend. */
export function useAppInfo(): AppInfoState {
  const [state, setState] = useState<AppInfoState>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    getAppInfo().then(
      (info) => {
        if (!cancelled) setState({ status: "ready", info });
      },
      (error: unknown) => {
        if (cancelled) return;
        setState({ status: "error", error: error as NativeError });
        console.warn("[islandy] get_app_info failed:", error);
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  return state;
}
