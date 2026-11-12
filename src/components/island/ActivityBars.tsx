import { usePrefersReducedMotion } from "../../hooks/usePrefersReducedMotion";

import { cx } from "../ui/cx";

const HEIGHTS = [0.55, 1, 0.7, 0.85] as const;
const DELAYS = ["0ms", "-420ms", "-180ms", "-640ms"] as const;

/**
 * Four small vertical bars. They animate (compositor-only `transform`) only
 * while `active`, and never when the user prefers reduced motion; otherwise
 * they rest as a static, dimmed glyph.
 */
export function ActivityBars({ active, className }: { active: boolean; className?: string }) {
  const reduce = usePrefersReducedMotion();
  const animate = active && !reduce;
  return (
    <span aria-hidden="true" className={cx("flex h-3.5 items-center gap-[2px]", className)}>
      {HEIGHTS.map((h, i) => (
        <span
          key={i}
          className={cx(
            "h-full w-[3px] origin-center rounded-full transition-colors duration-base",
            active ? "bg-fg" : "bg-fg-subtle",
            animate && "animate-bar",
          )}
          style={{
            transform: animate ? undefined : `scaleY(${active ? h : h * 0.6})`,
            animationDelay: animate ? DELAYS[i] : undefined,
          }}
        />
      ))}
    </span>
  );
}
