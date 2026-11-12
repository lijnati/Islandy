import { Asterisk } from "lucide-react";

import { usePrefersReducedMotion } from "../../hooks/usePrefersReducedMotion";
import { cx } from "../ui/cx";

export type GlyphState = "working" | "attention" | "idle";

/**
 * The Claude Code status glyph. It turns slowly only while a session is
 * working (compositor-only `transform`, off with reduced motion), is amber
 * and still when Claude Code asks for attention, and is static otherwise.
 */
export function ClaudeGlyph({
  state,
  size = 14,
  className,
}: {
  state: GlyphState;
  size?: number;
  className?: string;
}) {
  const reduce = usePrefersReducedMotion();
  return (
    <Asterisk
      size={size}
      strokeWidth={2.25}
      aria-hidden="true"
      className={cx(
        "shrink-0",
        state === "attention"
          ? "text-status-waiting"
          : state === "working"
            ? "text-fg"
            : "text-fg-muted",
        state === "working" && !reduce && "animate-glyph",
        className,
      )}
    />
  );
}
