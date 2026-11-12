import type { LucideIcon } from "lucide-react";
import type { MouseEventHandler } from "react";

import { cx } from "./cx";
import { Tooltip } from "./Tooltip";

interface IconButtonProps {
  /** Accessible name; also shown as the tooltip unless `tooltip` is given. */
  label: string;
  tooltip?: string;
  icon: LucideIcon;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  disabled?: boolean;
  /** An action is in flight: repeated presses are ignored by the caller. */
  busy?: boolean;
  /** Toggle buttons expose their state to assistive technology. */
  pressed?: boolean;
  size?: "sm" | "md" | "lg";
  variant?: "ghost" | "solid";
  tooltipSide?: "top" | "bottom";
  tooltipAlign?: "start" | "center" | "end";
}

const sizes = {
  sm: { box: "size-7", icon: 15 },
  md: { box: "size-8", icon: 16 },
  lg: { box: "size-10", icon: 18 },
} as const;

export const focusRing =
  "outline-hidden focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-focus";

export function IconButton({
  label,
  tooltip,
  icon: Icon,
  onClick,
  disabled = false,
  busy = false,
  pressed,
  size = "md",
  variant = "ghost",
  tooltipSide = "bottom",
  tooltipAlign = "center",
}: IconButtonProps) {
  const s = sizes[size];
  return (
    <Tooltip label={tooltip ?? label} side={tooltipSide} align={tooltipAlign}>
      <button
        type="button"
        aria-label={label}
        aria-pressed={pressed}
        aria-disabled={disabled || undefined}
        aria-busy={busy || undefined}
        onClick={disabled ? undefined : onClick}
        className={cx(
          "inline-flex shrink-0 items-center justify-center rounded-full transition-[background-color,color,transform] duration-fast ease-standard",
          s.box,
          focusRing,
          variant === "solid"
            ? "bg-accent text-on-accent shadow-control hover:bg-accent/90 active:scale-95"
            : "text-fg-muted hover:bg-surface-hover hover:text-fg active:scale-95",
          pressed && variant === "ghost" && "bg-surface-raised text-fg",
          busy && "opacity-60",
          disabled &&
            "cursor-not-allowed text-fg-subtle hover:bg-transparent hover:text-fg-subtle active:scale-100",
        )}
      >
        <Icon size={s.icon} strokeWidth={1.75} aria-hidden="true" />
      </button>
    </Tooltip>
  );
}
