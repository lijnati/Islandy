/**
 * Calendar math on the proleptic Gregorian calendar, in local time.
 *
 * Months are 0-based (JavaScript convention). Dates are compared as plain
 * {year, month, day} values, so DST changes and time zones cannot shift a
 * day into the wrong cell.
 */

export interface CalendarDay {
  year: number;
  /** 0-based month. */
  month: number;
  day: number;
}

export interface CalendarCell extends CalendarDay {
  /** Belongs to the month being displayed (vs. leading/trailing days). */
  inMonth: boolean;
  isToday: boolean;
  /** Day of week, 0 = Sunday. */
  weekday: number;
}

/** 0 = Sunday … 6 = Saturday. */
export type Weekday = 0 | 1 | 2 | 3 | 4 | 5 | 6;

export function isLeapYear(year: number): boolean {
  return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
}

const MONTH_LENGTHS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31] as const;

export function daysInMonth(year: number, month: number): number {
  const { month: m } = normalizeMonth(year, month);
  if (m === 1 && isLeapYear(normalizeMonth(year, month).year)) return 29;
  return MONTH_LENGTHS[m] ?? 31;
}

/** Wraps out-of-range months into the adjacent years (e.g. month 12 → January next year). */
export function normalizeMonth(year: number, month: number): { year: number; month: number } {
  const total = year * 12 + month;
  const y = Math.floor(total / 12);
  return { year: y, month: total - y * 12 };
}

export function addMonths(year: number, month: number, delta: number) {
  return normalizeMonth(year, month + delta);
}

/** Day of week for a date, 0 = Sunday (Sakamoto's algorithm: no Date/time zone involved). */
export function weekdayOf(year: number, month: number, day: number): Weekday {
  const offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4] as const;
  const y = month < 2 ? year - 1 : year;
  const raw =
    y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) + (offsets[month] ?? 0) + day;
  return (((raw % 7) + 7) % 7) as Weekday;
}

export function toCalendarDay(date: Date): CalendarDay {
  return { year: date.getFullYear(), month: date.getMonth(), day: date.getDate() };
}

export function sameDay(a: CalendarDay, b: CalendarDay): boolean {
  return a.year === b.year && a.month === b.month && a.day === b.day;
}

/**
 * The cells of a month view: whole weeks starting on `weekStartsOn`,
 * including the trailing/leading days of adjacent months. Always 6 rows when
 * `fixedRows` is set, so the widget height never jumps between months.
 */
export function buildMonthGrid(
  year: number,
  month: number,
  options: { weekStartsOn?: Weekday; today?: CalendarDay; fixedRows?: boolean } = {},
): CalendarCell[][] {
  const { weekStartsOn = 0, today, fixedRows = true } = options;
  const first = weekdayOf(year, month, 1);
  const lead = (first - weekStartsOn + 7) % 7;
  const length = daysInMonth(year, month);
  const rows = fixedRows ? 6 : Math.ceil((lead + length) / 7);

  const prev = addMonths(year, month, -1);
  const next = addMonths(year, month, 1);
  const prevLength = daysInMonth(prev.year, prev.month);

  const weeks: CalendarCell[][] = [];
  for (let r = 0; r < rows; r++) {
    const week: CalendarCell[] = [];
    for (let c = 0; c < 7; c++) {
      const index = r * 7 + c - lead; // 0-based day index within `month`
      let day: CalendarDay;
      let inMonth = false;
      if (index < 0) day = { ...prev, day: prevLength + index + 1 };
      else if (index >= length) day = { ...next, day: index - length + 1 };
      else {
        day = { year, month, day: index + 1 };
        inMonth = true;
      }
      week.push({
        ...day,
        inMonth,
        isToday: today !== undefined && sameDay(day, today),
        weekday: (weekStartsOn + c) % 7,
      });
    }
    weeks.push(week);
  }
  return weeks;
}

/**
 * First day of the week for a locale, via `Intl.Locale#getWeekInfo` where
 * available (Chromium/WebView2). Falls back to Sunday for US-style regions
 * and Monday elsewhere.
 */
export function weekStartFor(locale: string): Weekday {
  try {
    const loc = new Intl.Locale(locale) as Intl.Locale & {
      getWeekInfo?: () => { firstDay: number };
      weekInfo?: { firstDay: number };
    };
    const info = loc.getWeekInfo?.() ?? loc.weekInfo;
    if (info) return (info.firstDay % 7) as Weekday; // ISO: 7 = Sunday
    const region = loc.maximize().region ?? "";
    return ["US", "CA", "JP", "BR", "MX", "PH", "IL", "KR", "TW", "HK", "IN"].includes(region)
      ? 0
      : 1;
  } catch {
    return 0;
  }
}

/** Narrow weekday labels starting at `weekStartsOn`, e.g. S M T W T F S. */
export function weekdayLabels(
  locale: string | undefined,
  weekStartsOn: Weekday,
  width: "narrow" | "short" | "long" = "narrow",
): string[] {
  const formatter = new Intl.DateTimeFormat(locale, { weekday: width, timeZone: "UTC" });
  // 2023-01-01 was a Sunday.
  return Array.from({ length: 7 }, (_, i) =>
    formatter.format(new Date(Date.UTC(2023, 0, 1 + ((weekStartsOn + i) % 7)))),
  );
}
