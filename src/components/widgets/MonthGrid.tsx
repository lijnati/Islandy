import { useMemo } from "react";

import {
  buildMonthGrid,
  toCalendarDay,
  weekdayLabels,
  weekStartFor,
} from "../../features/calendar/calendar";
import { formatFullDate, locale } from "../../features/island/format";
import { cx } from "../ui/cx";

const weekStartsOn = weekStartFor(locale);
const narrowLabels = weekdayLabels(undefined, weekStartsOn, "narrow");
const longLabels = weekdayLabels(undefined, weekStartsOn, "long");

interface MonthGridProps {
  year: number;
  month: number;
  today: Date;
  size?: "sm" | "md";
}

/** A read-only month grid. Exposed as a table so rows/columns are announced. */
export function MonthGrid({ year, month, today, size = "sm" }: MonthGridProps) {
  const todayDay = toCalendarDay(today);
  const weeks = useMemo(
    () => buildMonthGrid(year, month, { weekStartsOn, today: todayDay }),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- todayDay is derived from `today`
    [year, month, todayDay.year, todayDay.month, todayDay.day],
  );
  const cell = size === "sm" ? "h-4 text-[10px]" : "h-[18px] text-caption";

  return (
    <table className="w-full table-fixed border-collapse text-center tabular-nums">
      <thead>
        <tr>
          {narrowLabels.map((label, i) => (
            <th
              key={i}
              scope="col"
              abbr={longLabels[i]}
              className={cx(
                "pb-0.5 font-medium text-fg-subtle",
                size === "sm" ? "text-[9px]" : "text-[10px]",
              )}
            >
              {label}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {weeks.map((week, r) => (
          <tr key={r}>
            {week.map((c) => (
              <td key={`${c.month}-${c.day}`} className={cx("p-0", cell)}>
                <span
                  aria-current={c.isToday ? "date" : undefined}
                  aria-label={c.isToday ? `Today, ${formatFullDate(today)}` : undefined}
                  className={cx(
                    "mx-auto flex items-center justify-center rounded-full",
                    size === "sm" ? "size-4" : "size-[18px]",
                    c.isToday
                      ? "bg-accent font-semibold text-surface"
                      : c.inMonth
                        ? "text-fg"
                        : "text-fg-subtle/60",
                  )}
                >
                  {c.day}
                </span>
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
