/**
 * The dashboard widget registry: every widget Islandy can show, with a
 * stable id, a name, and its layout needs. Widgets are built-in components
 * only (no third-party or dynamically loaded code).
 *
 * The dashboard shows the user's enabled widgets in their chosen order and
 * keeps as many as fit the (monitor-fitted) dashboard width; the rest are
 * left out rather than squeezed.
 */
import type { DashboardWidgetId, WidgetEntry } from "../../services/types";

export interface WidgetDefinition {
  id: DashboardWidgetId;
  name: string;
  description: string;
  /** Narrowest usable width (logical px). */
  minWidth: number;
  /** `fixed`: always `minWidth` wide. `flex`: grows with this weight. */
  layout: { kind: "fixed" } | { kind: "flex"; grow: number };
  /** Needs native Windows data to be useful (shows a notice elsewhere). */
  native: boolean;
}

export const WIDGETS: Record<DashboardWidgetId, WidgetDefinition> = {
  media: {
    id: "media",
    name: "Now Playing",
    description: "The current media session with playback controls.",
    minWidth: 300,
    layout: { kind: "flex", grow: 1.6 },
    native: true,
  },
  calendar: {
    id: "calendar",
    name: "Calendar",
    description: "Today's date and a month view.",
    minWidth: 204,
    layout: { kind: "fixed" },
    native: false,
  },
  activity: {
    id: "activity",
    name: "Activity",
    description: "Claude Code sessions, builds and recent events.",
    minWidth: 200,
    layout: { kind: "flex", grow: 1 },
    native: true,
  },
  system: {
    id: "system",
    name: "System",
    description: "Battery, network and wireless status at a glance.",
    minWidth: 204,
    layout: { kind: "fixed" },
    native: true,
  },
  audio: {
    id: "audio",
    name: "Sound",
    description: "Output device, volume and mute.",
    minWidth: 220,
    layout: { kind: "fixed" },
    native: true,
  },
  weather: {
    id: "weather",
    name: "Weather",
    description: "Current conditions and a short forecast for a place you choose.",
    minWidth: 220,
    layout: { kind: "fixed" },
    native: true,
  },
};

/** Gap between widgets and the dashboard's horizontal padding (px). */
export const WIDGET_GAP = 12;
export const DASHBOARD_PADDING_X = 32;

/**
 * The enabled widgets that fit `dashboardWidth`, in the user's order.
 * A widget that doesn't fit is skipped; later, narrower ones may still fit.
 */
export function fitWidgets(
  items: readonly WidgetEntry[],
  dashboardWidth: number,
): DashboardWidgetId[] {
  const available = dashboardWidth - DASHBOARD_PADDING_X;
  const shown: DashboardWidgetId[] = [];
  let used = 0;
  for (const item of items) {
    if (!item.enabled) continue;
    const need = WIDGETS[item.id].minWidth + (shown.length > 0 ? WIDGET_GAP : 0);
    if (used + need <= available) {
      shown.push(item.id);
      used += need;
    }
  }
  return shown;
}
