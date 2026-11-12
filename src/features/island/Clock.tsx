import { useClock } from "../../hooks/useClock";
import { formatShortDate, formatTime } from "./format";

export function Clock() {
  const now = useClock();
  return (
    <time
      dateTime={now.toISOString()}
      className="flex items-baseline gap-2 whitespace-nowrap tracking-tight"
    >
      <span className="text-body font-medium text-fg">{formatTime(now)}</span>
      <span className="text-caption font-regular text-fg-muted">{formatShortDate(now)}</span>
    </time>
  );
}
