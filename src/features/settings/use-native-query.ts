import { useCallback, useEffect, useState } from "react";

import { isNativeAvailable } from "../../services/native";

export interface Query<T> {
  data: T | null;
  error: string | null;
  /** Re-fetch now (e.g. after an action). */
  reload: () => void;
}

/**
 * `load` must be stable (a module-level function).
 *
 * Loads native data for the settings window and, with `pollMs`, refreshes
 * it while the component is mounted (the settings window reads snapshots;
 * it never takes over the island's push channels). Outside the desktop app
 * it returns `browserFallback`.
 */
export function useNativeQuery<T>(
  load: () => Promise<T>,
  { pollMs, browserFallback = null }: { pollMs?: number; browserFallback?: T | null } = {},
): Query<T> {
  const [data, setData] = useState<T | null>(isNativeAvailable() ? null : browserFallback);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (!isNativeAvailable()) return undefined;
    let cancelled = false;
    const run = () => {
      load().then(
        (value) => {
          if (!cancelled) {
            setData(value);
            setError(null);
          }
        },
        (cause: unknown) => {
          if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
        },
      );
    };
    run();
    const timer = pollMs ? setInterval(run, pollMs) : null;
    return () => {
      cancelled = true;
      if (timer) clearInterval(timer);
    };
  }, [load, pollMs, tick]);

  const reload = useCallback(() => {
    setTick((t) => t + 1);
  }, []);
  return { data, error, reload };
}
