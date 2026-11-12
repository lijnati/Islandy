// Formatters are relatively expensive to construct; build them once.
// `undefined` locale follows the WebView's language, which on Windows follows
// the display language (not necessarily the user's custom regional format).
const timeFormatter = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const dateFormatter = new Intl.DateTimeFormat(undefined, { weekday: "short", day: "numeric" });

export function formatTime(date: Date): string {
  return timeFormatter.format(date);
}

export function formatShortDate(date: Date): string {
  return dateFormatter.format(date);
}
