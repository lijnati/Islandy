import { ChevronLeft, ChevronRight } from "lucide-react";
import { useState } from "react";

import { addMonths } from "../../features/calendar/calendar";
import { dispatchIsland } from "../../features/island/island-store";
import { formatMonth, formatMonthYear, formatWeekday } from "../../features/island/format";
import { useNow } from "../../hooks/useNow";
import { IconButton, focusRing } from "../ui/IconButton";
import { cx } from "../ui/cx";
import { WidgetSurface } from "../ui/WidgetSurface";
import { MonthGrid } from "./MonthGrid";

/** Real calendar for the current system date. Local only; no accounts. */
export function CalendarWidget({ className }: { className?: string | undefined }) {
  const now = useNow();
  return (
    <WidgetSurface
      title={formatMonth(now)}
      className={className}
      onOpen={() => {
        dispatchIsland({ type: "OPEN_WIDGET", widget: "calendar" });
      }}
      accessory={
        <span className="text-caption font-medium text-fg-muted">
          {formatWeekday(now).slice(0, 3)} <span className="text-fg">{now.getDate()}</span>
        </span>
      }
    >
      <MonthGrid year={now.getFullYear()} month={now.getMonth()} today={now} />
    </WidgetSurface>
  );
}

export function CalendarDetail() {
  const now = useNow();
  const [view, setView] = useState(() => ({ year: now.getFullYear(), month: now.getMonth() }));
  const isCurrent = view.year === now.getFullYear() && view.month === now.getMonth();
  const move = (delta: number) => {
    setView((v) => addMonths(v.year, v.month, delta));
  };

  return (
    <div className="flex h-full gap-3">
      <div className="flex w-[180px] shrink-0 flex-col justify-between rounded-tile border border-hairline bg-surface-secondary p-4 shadow-surface">
        <p className="text-caption font-medium tracking-label text-fg-muted uppercase">
          {formatWeekday(now)}
        </p>
        <div>
          <p className="text-display font-semibold tracking-display text-fg tabular-nums">
            {now.getDate()}
          </p>
          <p className="mt-1 text-body text-fg-muted">
            {formatMonthYear(now.getFullYear(), now.getMonth())}
          </p>
        </div>
      </div>
      <section
        aria-label={formatMonthYear(view.year, view.month)}
        className="flex min-w-0 flex-1 flex-col rounded-tile border border-hairline bg-surface-secondary px-4 py-2.5 shadow-surface"
      >
        <div className="mb-1 flex items-center justify-between">
          <h3 aria-live="polite" className="text-body font-semibold text-fg">
            {formatMonthYear(view.year, view.month)}
          </h3>
          <div className="flex items-center gap-1">
            {!isCurrent && (
              <button
                type="button"
                onClick={() => {
                  setView({ year: now.getFullYear(), month: now.getMonth() });
                }}
                className={cx(
                  "mr-1 rounded-full px-2 py-0.5 text-caption font-medium text-fg-muted hover:bg-surface-hover hover:text-fg",
                  focusRing,
                )}
              >
                Today
              </button>
            )}
            <IconButton
              icon={ChevronLeft}
              size="sm"
              label="Previous month"
              onClick={() => {
                move(-1);
              }}
            />
            <IconButton
              icon={ChevronRight}
              size="sm"
              label="Next month"
              tooltipAlign="end"
              onClick={() => {
                move(1);
              }}
            />
          </div>
        </div>
        <MonthGrid year={view.year} month={view.month} today={now} size="md" />
      </section>
    </div>
  );
}
