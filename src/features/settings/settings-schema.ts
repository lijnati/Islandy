/**
 * The typed settings schema on the TypeScript side: bounds, labels and a
 * `normalizeSettings` that mirrors `Settings::normalize` in Rust. Rust is the
 * authority (every update is normalized there); this copy keeps the UI from
 * ever proposing an out-of-range value and serves the browser preview.
 */
import type {
  DashboardWidgetId,
  Settings,
  SettingsPageId,
  WeatherLocation,
  WidgetEntry,
} from "../../services/types";
import { DEFAULT_SETTINGS, DEFAULT_SHORTCUT } from "./defaults";

export const RANGES = {
  opacity: { min: 70, max: 100, step: 5 },
  topMargin: { min: 0, max: 48, step: 2 },
  offsetX: { min: -2000, max: 2000, step: 10 },
  hoverDelayMs: { min: 100, max: 500, step: 10 },
  collapseDelayMs: { min: 200, max: 1000, step: 20 },
  retentionDays: { min: 1, max: 30 },
} as const;

export const WIDGET_IDS: readonly DashboardWidgetId[] = [
  "media",
  "calendar",
  "activity",
  "system",
  "audio",
  "weather",
];

export const SETTINGS_PAGES: readonly { id: SettingsPageId; label: string }[] = [
  { id: "general", label: "General" },
  { id: "appearance", label: "Appearance" },
  { id: "island", label: "Island" },
  { id: "widgets", label: "Widgets" },
  { id: "integrations", label: "Integrations" },
  { id: "notifications", label: "Notifications" },
  { id: "privacy", label: "Privacy" },
  { id: "about", label: "About" },
];

export function isSettingsPage(value: string): value is SettingsPageId {
  return SETTINGS_PAGES.some((p) => p.id === value);
}

const clamp = (value: number, { min, max }: { min: number; max: number }, fallback: number) =>
  Number.isFinite(value) ? Math.min(max, Math.max(min, Math.round(value))) : fallback;

function normalizeWidgets(items: readonly WidgetEntry[]): WidgetEntry[] {
  const seen = new Set<DashboardWidgetId>();
  const out: WidgetEntry[] = [];
  for (const item of items) {
    if (WIDGET_IDS.includes(item.id) && !seen.has(item.id)) {
      seen.add(item.id);
      out.push({ id: item.id, enabled: item.enabled });
    }
  }
  for (const entry of DEFAULT_SETTINGS.widgets.items) {
    if (!seen.has(entry.id)) out.push({ ...entry });
  }
  return out;
}

const validMonitor = (name: string) =>
  name.length > 0 &&
  name.length <= 128 &&
  !Array.from({ length: name.length }, (_, i) => name.charCodeAt(i)).some((code) => code < 32);

const round2 = (v: number) => Math.round(v * 100) / 100;
const placeText = (text: string) =>
  Array.from(text)
    .filter((c) => c.charCodeAt(0) >= 32 && c.charCodeAt(0) !== 127)
    .slice(0, 80)
    .join("")
    .trim();

/** Mirrors `WeatherLocation::sanitized`. */
export function normalizeLocation(location: WeatherLocation | null): WeatherLocation | null {
  if (!location) return null;
  const name = placeText(location.name);
  const { latitude, longitude } = location;
  const valid =
    name.length > 0 &&
    Number.isFinite(latitude) &&
    Number.isFinite(longitude) &&
    Math.abs(latitude) <= 90 &&
    Math.abs(longitude) <= 180;
  return valid
    ? {
        name,
        detail: placeText(location.detail),
        latitude: round2(latitude),
        longitude: round2(longitude),
      }
    : null;
}

/** Mirrors `Settings::normalize` (idempotent). */
export function normalizeSettings(s: Settings): Settings {
  const d = DEFAULT_SETTINGS;
  const shortcut = s.general.toggleShortcut.trim();
  const interaction = {
    ...s.interaction,
    hoverDelayMs: clamp(
      s.interaction.hoverDelayMs,
      RANGES.hoverDelayMs,
      d.interaction.hoverDelayMs,
    ),
    collapseDelayMs: clamp(
      s.interaction.collapseDelayMs,
      RANGES.collapseDelayMs,
      d.interaction.collapseDelayMs,
    ),
  };
  // The pointer must always be able to open the island.
  if (!interaction.openOnHover && !interaction.openOnClick) interaction.openOnClick = true;
  const monitor = s.island.monitor;
  return {
    version: 1,
    general: {
      ...s.general,
      toggleShortcut: shortcut && shortcut.length <= 64 ? shortcut : DEFAULT_SHORTCUT,
    },
    appearance: {
      ...s.appearance,
      opacity: clamp(s.appearance.opacity, RANGES.opacity, d.appearance.opacity),
    },
    island: {
      ...s.island,
      offsetX: clamp(s.island.offsetX, RANGES.offsetX, 0),
      topMargin: clamp(s.island.topMargin, RANGES.topMargin, d.island.topMargin),
      monitor: validMonitor(monitor) ? monitor : "primary",
    },
    interaction,
    widgets: { items: normalizeWidgets(s.widgets.items) },
    weather: { ...s.weather, location: normalizeLocation(s.weather.location) },
    updates: { ...s.updates },
  };
}

/** Moves the widget at `from` to `to` (both clamped). */
export function moveWidget(items: readonly WidgetEntry[], from: number, to: number): WidgetEntry[] {
  const next = [...items];
  if (from < 0 || from >= next.length) return next;
  const target = Math.max(0, Math.min(next.length - 1, to));
  const [moved] = next.splice(from, 1);
  if (moved) next.splice(target, 0, moved);
  return next;
}
