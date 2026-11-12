import { cx } from "./cx";

/** A thin, quiet utilization bar (0–100). `null` renders an empty track. */
export function MeterBar({
  value,
  label,
  tone = "neutral",
  className,
}: {
  value: number | null;
  label: string;
  tone?: "neutral" | "attention" | "success";
  className?: string;
}) {
  return (
    <div
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value ?? undefined}
      aria-valuetext={value === null ? "Unknown" : `${value}%`}
      className={cx("h-1 overflow-hidden rounded-full bg-surface-hover", className)}
    >
      <div
        className={cx(
          "h-full rounded-full transition-[width] duration-slow ease-standard",
          tone === "attention"
            ? "bg-status-attention"
            : tone === "success"
              ? "bg-status-success"
              : "bg-fg",
        )}
        style={{ width: `${value ?? 0}%` }}
      />
    </div>
  );
}
