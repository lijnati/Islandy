import type { ReactNode } from "react";

import { cx } from "./cx";

interface TooltipProps {
  label: string;
  children: ReactNode;
  /** Which side of the anchor the tip appears on. */
  side?: "top" | "bottom";
  /** Horizontal alignment relative to the anchor; use "end" near the right edge. */
  align?: "start" | "center" | "end";
}

/**
 * A small hover/keyboard-focus tooltip, CSS-only (no timers, no portals).
 * It is decorative: the anchor must carry its own accessible name, so the
 * tip is hidden from assistive technology.
 */
export function Tooltip({ label, children, side = "bottom", align = "center" }: TooltipProps) {
  return (
    <span className="group/tip relative inline-flex">
      {children}
      <span
        aria-hidden="true"
        className={cx(
          "pointer-events-none absolute z-(--z-overlay) whitespace-nowrap rounded-[8px] border border-hairline bg-surface-elevated px-2 py-1 text-caption font-medium text-fg opacity-0 shadow-control transition-opacity duration-fast",
          "group-hover/tip:opacity-100 group-hover/tip:delay-500 group-has-[:focus-visible]/tip:opacity-100",
          side === "bottom" ? "top-full mt-1.5" : "bottom-full mb-1.5",
          align === "center" && "left-1/2 -translate-x-1/2",
          align === "start" && "left-0",
          align === "end" && "right-0",
        )}
      >
        {label}
      </span>
    </span>
  );
}
