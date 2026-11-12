import { describe, expect, it } from "vitest";

import {
  addMonths,
  buildMonthGrid,
  daysInMonth,
  isLeapYear,
  normalizeMonth,
  weekdayLabels,
  weekdayOf,
  weekStartFor,
} from "./calendar";

describe("calendar math", () => {
  it("knows leap years, including century rules", () => {
    expect([2024, 2000, 2400, 1996].map(isLeapYear)).toEqual([true, true, true, true]);
    expect([2026, 1900, 2100, 2023].map(isLeapYear)).toEqual([false, false, false, false]);
  });

  it("returns correct month lengths", () => {
    const lengths2026 = Array.from({ length: 12 }, (_, m) => daysInMonth(2026, m));
    expect(lengths2026).toEqual([31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]);
    expect(daysInMonth(2024, 1)).toBe(29);
    expect(daysInMonth(2000, 1)).toBe(29);
    expect(daysInMonth(1900, 1)).toBe(28);
  });

  it("matches the platform's month lengths for 400 years", () => {
    for (let year = 1900; year < 2300; year++) {
      for (let month = 0; month < 12; month++) {
        expect(daysInMonth(year, month)).toBe(new Date(year, month + 1, 0).getDate());
      }
    }
  });

  it("computes weekdays without Date (time-zone independent)", () => {
    expect(weekdayOf(2026, 10, 12)).toBe(4); // Thu 12 Nov 2026
    expect(weekdayOf(2000, 1, 29)).toBe(2); // Tue 29 Feb 2000
    expect(weekdayOf(2024, 0, 1)).toBe(1); // Mon 1 Jan 2024
    for (let day = 1; day <= 366; day++) {
      const date = new Date(Date.UTC(2024, 0, day));
      expect(weekdayOf(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())).toBe(
        date.getUTCDay(),
      );
    }
  });

  it("wraps months across year boundaries", () => {
    expect(addMonths(2026, 11, 1)).toEqual({ year: 2027, month: 0 });
    expect(addMonths(2026, 0, -1)).toEqual({ year: 2025, month: 11 });
    expect(addMonths(2026, 5, -18)).toEqual({ year: 2024, month: 11 });
    expect(normalizeMonth(2026, 25)).toEqual({ year: 2028, month: 1 });
    expect(daysInMonth(2023, 13)).toBe(29); // = February 2024
  });
});

describe("month grid", () => {
  const today = { year: 2026, month: 10, day: 12 };

  it("aligns the first day with its weekday (Sunday start)", () => {
    // November 2026 starts on a Sunday.
    const grid = buildMonthGrid(2026, 10, { weekStartsOn: 0, today });
    expect(grid).toHaveLength(6);
    expect(grid[0]?.[0]).toMatchObject({ year: 2026, month: 10, day: 1, inMonth: true });
    const todayCells = grid.flat().filter((c) => c.isToday);
    expect(todayCells).toEqual([expect.objectContaining({ day: 12, weekday: 4 })]);
  });

  it("aligns the first day with its weekday (Monday start) and fills adjacent months", () => {
    const grid = buildMonthGrid(2026, 10, { weekStartsOn: 1 });
    // Mon 26 Oct … Sat 31 Oct, then Sun 1 Nov.
    expect(grid[0]?.map((c) => c.day)).toEqual([26, 27, 28, 29, 30, 31, 1]);
    expect(grid[0]?.[0]).toMatchObject({ month: 9, inMonth: false });
    expect(grid[0]?.[6]).toMatchObject({ month: 10, inMonth: true, weekday: 0 });
  });

  it("handles leap February and December → January rollover", () => {
    const feb = buildMonthGrid(2024, 1, { weekStartsOn: 0 }).flat();
    expect(feb.filter((c) => c.inMonth)).toHaveLength(29);
    const dec = buildMonthGrid(2026, 11, { weekStartsOn: 1 }).flat();
    const trailing = dec.filter((c) => !c.inMonth && c.month === 0);
    expect(trailing[0]).toMatchObject({ year: 2027, month: 0, day: 1 });
    const jan = buildMonthGrid(2027, 0, { weekStartsOn: 1 }).flat();
    expect(jan.filter((c) => !c.inMonth && c.year === 2026).at(-1)).toMatchObject({ day: 31 });
  });

  it("every in-month day appears exactly once, in order, with matching weekday", () => {
    for (const [year, month] of [
      [2026, 1],
      [2024, 1],
      [2027, 7],
      [2100, 1],
    ] as const) {
      for (const start of [0, 1, 6] as const) {
        const cells = buildMonthGrid(year, month, { weekStartsOn: start }).flat();
        const days = cells.filter((c) => c.inMonth).map((c) => c.day);
        expect(days).toEqual(Array.from({ length: daysInMonth(year, month) }, (_, i) => i + 1));
        for (const c of cells) expect(c.weekday).toBe(weekdayOf(c.year, c.month, c.day));
      }
    }
  });

  it("can drop unused rows", () => {
    // February 2026 starts on Sunday and has 28 days: exactly 4 weeks.
    expect(buildMonthGrid(2026, 1, { weekStartsOn: 0, fixedRows: false })).toHaveLength(4);
  });
});

describe("locale helpers", () => {
  it("picks a sensible first day of the week", () => {
    expect(weekStartFor("en-US")).toBe(0);
    expect(weekStartFor("de-DE")).toBe(1);
    expect(weekStartFor("not a locale!")).toBe(0);
  });

  it("produces localized weekday labels in display order", () => {
    expect(weekdayLabels("en-US", 0, "short")).toEqual([
      "Sun",
      "Mon",
      "Tue",
      "Wed",
      "Thu",
      "Fri",
      "Sat",
    ]);
    expect(weekdayLabels("en-US", 1, "narrow")).toEqual(["M", "T", "W", "T", "F", "S", "S"]);
  });
});
