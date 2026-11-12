// Formatters are relatively expensive to construct; build them once.
// `undefined` locale follows the WebView's language, which on Windows follows
// the display language (not necessarily the user's custom regional format).
export const locale: string = new Intl.DateTimeFormat().resolvedOptions().locale;

const timeFormatter = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const shortDateFormatter = new Intl.DateTimeFormat(undefined, {
  weekday: "short",
  month: "short",
  day: "numeric",
});
const weekdayFormatter = new Intl.DateTimeFormat(undefined, { weekday: "long" });
const monthFormatter = new Intl.DateTimeFormat(undefined, { month: "long" });
const monthYearFormatter = new Intl.DateTimeFormat(undefined, { month: "long", year: "numeric" });
const fullDateFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: "full" });
const relativeFormatter = new Intl.RelativeTimeFormat(undefined, {
  numeric: "auto",
  style: "short",
});

export function formatTime(date: Date): string {
  return timeFormatter.format(date);
}

/** e.g. "Thu, Nov 12". */
export function formatShortDate(date: Date): string {
  return shortDateFormatter.format(date);
}

export function formatWeekday(date: Date): string {
  return weekdayFormatter.format(date);
}

export function formatMonth(date: Date): string {
  return monthFormatter.format(date);
}

export function formatMonthYear(year: number, month: number): string {
  return monthYearFormatter.format(new Date(year, month, 1));
}

export function formatFullDate(date: Date): string {
  return fullDateFormatter.format(date);
}

/** m:ss, or h:mm:ss for long durations. */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/** "now", "4 min. ago", "2 hr. ago", … relative to `now`. */
export function formatRelative(then: number, now: number): string {
  const minutes = Math.round((then - now) / 60_000);
  if (Math.abs(minutes) < 1) return relativeFormatter.format(0, "minute");
  if (Math.abs(minutes) < 60) return relativeFormatter.format(minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return relativeFormatter.format(hours, "hour");
  return relativeFormatter.format(Math.round(hours / 24), "day");
}
