/**
 * Pure helpers for the weather widget: WMO weather codes → words and icons,
 * temperatures, and day names. Kept free of React so they are unit-tested.
 */
import type { TemperatureUnit, WeatherSnapshot } from "../../services/types";

export type WeatherIconName =
  | "clear"
  | "clearNight"
  | "partly"
  | "partlyNight"
  | "cloudy"
  | "fog"
  | "drizzle"
  | "rain"
  | "snow"
  | "showers"
  | "thunder";

export interface Condition {
  label: string;
  icon: WeatherIconName;
}

/** WMO weather interpretation codes (as used by Open-Meteo). */
export function describeWeather(code: number, isDay = true): Condition {
  if (code === 0) return { label: "Clear", icon: isDay ? "clear" : "clearNight" };
  if (code === 1) return { label: "Mostly clear", icon: isDay ? "clear" : "clearNight" };
  if (code === 2) return { label: "Partly cloudy", icon: isDay ? "partly" : "partlyNight" };
  if (code === 3) return { label: "Overcast", icon: "cloudy" };
  if (code === 45 || code === 48) return { label: "Fog", icon: "fog" };
  if (code >= 51 && code <= 57) return { label: "Drizzle", icon: "drizzle" };
  if (code >= 61 && code <= 67) {
    return {
      label: code >= 66 ? "Freezing rain" : code === 65 ? "Heavy rain" : "Rain",
      icon: "rain",
    };
  }
  if (code >= 71 && code <= 77) return { label: code === 75 ? "Heavy snow" : "Snow", icon: "snow" };
  if (code >= 80 && code <= 82) return { label: "Showers", icon: "showers" };
  if (code === 85 || code === 86) return { label: "Snow showers", icon: "snow" };
  if (code >= 95) return { label: "Thunderstorm", icon: "thunder" };
  return { label: "Cloudy", icon: "cloudy" };
}

/** `12.8` → `"13°"`; `-0.4` → `"0°"` (never "-0°"). */
export function formatTemperature(value: number): string {
  const rounded = Math.round(value);
  return `${Object.is(rounded, -0) ? 0 : rounded}°`;
}

export function unitSymbol(unit: TemperatureUnit): string {
  return unit === "fahrenheit" ? "°F" : "°C";
}

export function windUnit(unit: TemperatureUnit): string {
  return unit === "fahrenheit" ? "mph" : "km/h";
}

/**
 * Short weekday of a `YYYY-MM-DD` date (the place's local date), parsed as
 * a calendar date so the viewer's time zone can't shift it.
 */
export function weekdayOf(date: string, locale?: string): string {
  const [y, m, d] = date.split("-").map(Number);
  if (!y || !m || !d) return "";
  return new Intl.DateTimeFormat(locale, { weekday: "short", timeZone: "UTC" }).format(
    new Date(Date.UTC(y, m - 1, d)),
  );
}

/** "Updated just now" / "Updated 12 min ago" / "Updated 2 h ago". */
export function updatedAgo(updatedAtMs: number, nowMs: number): string {
  const minutes = Math.max(0, Math.floor((nowMs - updatedAtMs) / 60_000));
  if (minutes < 1) return "Updated just now";
  if (minutes < 60) return `Updated ${minutes} min ago`;
  return `Updated ${Math.floor(minutes / 60)} h ago`;
}

/** Sample data for the browser preview (never shown in the desktop app). */
export const PREVIEW_WEATHER: WeatherSnapshot = {
  status: "ready",
  location: { name: "Lisbon", detail: "Portugal", latitude: 38.72, longitude: -9.14 },
  unit: "celsius",
  forecast: {
    current: {
      temperature: 21.4,
      feelsLike: 20.6,
      code: 2,
      isDay: true,
      humidity: 58,
      windSpeed: 12,
    },
    days: [
      { date: "2026-06-12", code: 2, high: 24, low: 16 },
      { date: "2026-06-13", code: 0, high: 26, low: 17 },
      { date: "2026-06-14", code: 61, high: 21, low: 15 },
      { date: "2026-06-15", code: 3, high: 22, low: 15 },
    ],
  },
  updatedAtMs: 0,
  stale: false,
};
