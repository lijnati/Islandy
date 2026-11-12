import { Download, Hammer, SquareTerminal, type LucideIcon } from "lucide-react";

import { statusLabel, statusTone, type Activity } from "../../features/activity/demo-activity";
import { formatRelative } from "../../features/island/format";
import { StatusIndicator } from "../ui/StatusIndicator";
import { cx } from "../ui/cx";

const ICONS: Record<Activity["icon"], LucideIcon> = {
  terminal: SquareTerminal,
  hammer: Hammer,
  download: Download,
};

interface ActivityCardProps {
  activity: Activity;
  now: Date;
  /** `full` shows the app name, title and status; `compact` is a single line. */
  variant?: "full" | "compact";
  className?: string | undefined;
}

/** Reusable presentation of one activity; knows nothing about where it came from. */
export function ActivityCard({ activity, now, variant = "full", className }: ActivityCardProps) {
  const Icon = ICONS[activity.icon];
  const when = formatRelative(activity.since, now.getTime());
  const tone = statusTone[activity.status];
  const status = statusLabel[activity.status];

  if (variant === "compact") {
    return (
      <div className={cx("flex items-center gap-2 text-caption", className)}>
        <Icon size={12} strokeWidth={1.75} aria-hidden="true" className="shrink-0 text-fg-muted" />
        <span className="min-w-0 flex-1 truncate text-fg">{activity.title}</span>
        <StatusIndicator tone={tone} srLabel={status} />
        <span className="shrink-0 text-fg-subtle tabular-nums">{when}</span>
      </div>
    );
  }

  return (
    <div className={cx("flex items-start gap-2.5", className)}>
      <span
        aria-hidden="true"
        className="flex size-8 shrink-0 items-center justify-center rounded-[10px] border border-hairline bg-surface-elevated text-fg"
      >
        <Icon size={16} strokeWidth={1.75} />
      </span>
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-caption font-medium text-fg-muted">{activity.app}</span>
        <span className="truncate text-body font-semibold tracking-tight text-fg">
          {activity.title}
        </span>
        <span className="mt-1 flex items-center gap-2">
          <StatusIndicator tone={tone} label={status} />
          <span className="text-caption text-fg-subtle tabular-nums">{when}</span>
        </span>
      </div>
    </div>
  );
}
