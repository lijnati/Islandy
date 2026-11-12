import { X } from "lucide-react";

import {
  dismissLabel,
  eventDetail,
  eventIcon,
  severityTone,
  shortTitle,
} from "../../features/activity/activity-model";
import { activityStore } from "../../features/activity/activity-store";
import { formatRelative } from "../../features/island/format";
import type { ActivityEvent } from "../../services/types";
import { focusRing } from "../ui/IconButton";
import { StatusIndicator } from "../ui/StatusIndicator";
import { cx } from "../ui/cx";
import { ACTIVITY_ICONS } from "./icons";

const TONE_LABEL = {
  neutral: "",
  working: "In progress",
  waiting: "Needs attention",
  success: "Success",
  attention: "Problem",
} as const;

/** One activity event. `compact` is a single line for the dashboard card. */
export function ActivityRow({
  event,
  now,
  compact = false,
  dismissible = false,
}: {
  event: ActivityEvent;
  now: number;
  compact?: boolean;
  dismissible?: boolean;
}) {
  const Icon = ACTIVITY_ICONS[eventIcon(event.kind)];
  const tone = severityTone(event.severity);
  const detail = eventDetail(event);
  const when = formatRelative(event.timestamp, now);
  const open = event.lifecycle === "active";
  const faded = event.lifecycle === "dismissed" || event.lifecycle === "expired";
  const projectPrefix = event.source === "developer" && event.project ? `${event.project} · ` : "";

  if (compact) {
    return (
      <div className={cx("flex items-center gap-2 text-caption", faded && "opacity-60")}>
        <Icon size={12} strokeWidth={1.75} aria-hidden="true" className="shrink-0 text-fg-muted" />
        <span className="min-w-0 flex-1 truncate text-fg">
          {projectPrefix}
          {shortTitle(event)}
        </span>
        {tone !== "neutral" && <StatusIndicator tone={tone} srLabel={TONE_LABEL[tone]} />}
        <span className="shrink-0 whitespace-nowrap text-fg-subtle tabular-nums">{when}</span>
      </div>
    );
  }

  return (
    <div
      className={cx(
        "group/row flex h-10 items-center gap-2.5 rounded-control px-2 transition-colors duration-fast hover:bg-surface-raised",
        faded && "opacity-60",
      )}
    >
      <span
        aria-hidden="true"
        className={cx(
          "flex size-6 shrink-0 items-center justify-center rounded-[8px] border border-hairline bg-surface-elevated",
          tone === "waiting" && open ? "text-status-waiting" : "text-fg",
        )}
      >
        <Icon size={13} strokeWidth={1.9} />
      </span>
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-body font-medium tracking-tight text-fg">
          {projectPrefix}
          {event.title}
        </span>
        {detail && <span className="truncate text-caption text-fg-muted">{detail}</span>}
      </div>
      {tone !== "neutral" && <StatusIndicator tone={tone} srLabel={TONE_LABEL[tone]} />}
      <span className="w-20 shrink-0 text-right text-caption whitespace-nowrap text-fg-subtle tabular-nums">
        {open && event.severity === "attention" ? "Open" : when}
      </span>
      {dismissible && (
        <button
          type="button"
          aria-label={`${dismissLabel(event)}: ${event.title}`}
          title={dismissLabel(event)}
          onClick={() => {
            activityStore.getState().dismiss(event.id);
          }}
          className={cx(
            "flex size-6 shrink-0 items-center justify-center rounded-full text-fg-subtle opacity-0 transition-opacity duration-fast group-hover/row:opacity-100 hover:bg-surface-hover hover:text-fg focus-visible:opacity-100",
            focusRing,
          )}
        >
          <X size={13} strokeWidth={2} aria-hidden="true" />
        </button>
      )}
    </div>
  );
}
