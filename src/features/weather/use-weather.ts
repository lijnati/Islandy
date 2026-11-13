import { useCallback, useEffect, useState } from "react";

import { getWeather, isNativeAvailable } from "../../services/native";
import type { WeatherSnapshot } from "../../services/types";
import { useSettings } from "../settings/settings-store";
import { PREVIEW_WEATHER } from "./weather-model";

/** How often a mounted widget asks again (the backend caches for 30 min). */
const REFRESH_MS = 10 * 60_000;
/** Another request was running (e.g. for the previous place): ask again. */
const LOADING_RETRY_MS = 1500;

/**
 * The weather snapshot for the chosen place. Asks the backend while the
 * widget is mounted (i.e. while the dashboard is open) and whenever the
 * place or unit changes; the backend decides whether a request is due.
 */
export function useWeather(): { weather: WeatherSnapshot | null; retry: () => void } {
  const location = useSettings((s) => s.weather.location);
  const unit = useSettings((s) => s.weather.unit);
  const [weather, setWeather] = useState<WeatherSnapshot | null>(
    isNativeAvailable() ? null : PREVIEW_WEATHER,
  );
  const [force, setForce] = useState(0);

  useEffect(() => {
    if (!isNativeAvailable()) return undefined;
    let cancelled = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const load = (fresh: boolean) => {
      getWeather(fresh).then(
        (snapshot) => {
          if (cancelled) return;
          setWeather(snapshot);
          if (snapshot.status === "loading") {
            retry = setTimeout(() => {
              load(false);
            }, LOADING_RETRY_MS);
          }
        },
        () => {
          // The command itself never fails; ignore IPC teardown errors.
        },
      );
    };
    load(force > 0);
    const timer = setInterval(() => {
      load(false);
    }, REFRESH_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
      clearTimeout(retry);
    };
    // `location` / `unit` are dependencies so a new place loads at once.
  }, [location, unit, force]);

  const retry = useCallback(() => {
    setForce((n) => n + 1);
  }, []);
  return { weather, retry };
}
