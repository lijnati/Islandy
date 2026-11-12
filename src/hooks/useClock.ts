import { useEffect, useState } from "react";

/** Milliseconds until the next wall-clock minute boundary (always > 0). */
export function msUntilNextMinute(now: Date): number {
  return 60_000 - (now.getSeconds() * 1000 + now.getMilliseconds());
}

/**
 * Returns the current time, re-rendering once per minute, aligned to the
 * minute boundary. No per-second work happens while idle.
 *
 * Timers can drift or fire late (e.g. after system sleep), so every tick
 * re-reads the clock and reschedules, and the hook resynchronises when the
 * page becomes visible again.
 */
export function useClock(): Date {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = () => {
      const current = new Date();
      setNow(current);
      timer = setTimeout(tick, msUntilNextMinute(current));
    };

    const resync = () => {
      if (document.visibilityState !== "visible") return;
      clearTimeout(timer);
      tick();
    };

    timer = setTimeout(tick, msUntilNextMinute(new Date()));
    document.addEventListener("visibilitychange", resync);
    window.addEventListener("focus", resync);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", resync);
      window.removeEventListener("focus", resync);
    };
  }, []);

  return now;
}
