import type { StatusTone } from "../../features/island/island-store";
import { cx } from "./cx";

const toneClass: Record<StatusTone, string> = {
  neutral: "bg-status-neutral",
  working: "bg-status-working",
  success: "bg-status-success",
  attention: "bg-status-attention",
};

const toneText: Record<StatusTone, string> = {
  neutral: "text-fg-muted",
  working: "text-status-working",
  success: "text-status-success",
  attention: "text-status-attention",
};

interface StatusIndicatorProps {
  tone: StatusTone;
  /** Visible label. Without it, provide `srLabel` so the status is announced. */
  label?: string;
  srLabel?: string;
  className?: string | undefined;
}

/** A small status dot, optionally with a label. Never animates. */
export function StatusIndicator({ tone, label, srLabel, className }: StatusIndicatorProps) {
  return (
    <span className={cx("inline-flex items-center gap-1.5", className)}>
      <span aria-hidden="true" className={cx("size-1.5 shrink-0 rounded-full", toneClass[tone])} />
      {label ? (
        <span className={cx("text-caption font-medium", toneText[tone])}>{label}</span>
      ) : srLabel ? (
        <span className="sr-only">{srLabel}</span>
      ) : null}
    </span>
  );
}
