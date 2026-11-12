import { useEffect, useState } from "react";

/**
 * Current epoch ms, re-rendering every second only while `active` (e.g. a
 * visible elapsed timer for a working session). Otherwise it returns
 * `fallback` and runs no timer at all.
 */
export function useSeconds(active: boolean, fallback: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return undefined;
    const timer = setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      clearInterval(timer);
    };
  }, [active]);
  return active ? Math.max(now, fallback) : fallback;
}
