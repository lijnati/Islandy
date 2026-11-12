/**
 * Design values that JavaScript needs at runtime.
 *
 * Visual tokens (colors, radii, typography) live in `src/styles/tokens.css`.
 * Only mirror a value here when code must read it, and keep both in sync.
 */
import type { Transition } from "motion/react";

import type { IslandLayouts, SizePreset } from "../services/types";

/** Interaction timing (ms). */
export const interaction = {
  /** Hover-intent delay before the pill expands (spec: 120–180 ms). */
  hoverIntentMs: 150,
  /** Delay after the pointer leaves before an unpinned dashboard collapses. */
  leaveDelayMs: 360,
  /** How long a peek stays before returning to the pill. */
  peekDwellMs: 4000,
} as const;

/**
 * Motion. Springs use `visualDuration` (the time to visually settle), which
 * keeps durations designable while staying interruptible and velocity-aware.
 */
export const motion = {
  /** Milliseconds; mirrors --duration-* in tokens.css. */
  duration: { fast: 120, base: 220, slow: 360 },
  /** Collapsed/peek → expanded: ~340 ms, a hint of overshoot (a few px). */
  expand: {
    type: "spring",
    visualDuration: 0.34,
    bounce: 0.14,
    restDelta: 0.5,
  } satisfies Transition,
  /**
   * Expanded → collapsed: ~240 ms, no overshoot. `restDelta` ends the spring's
   * imperceptible tail early, so the native frame shrinks back sooner.
   */
  collapse: {
    type: "spring",
    visualDuration: 0.24,
    bounce: 0,
    restDelta: 0.5,
  } satisfies Transition,
  /** Collapsed ↔ peek: ~220 ms. */
  peek: { type: "spring", visualDuration: 0.22, bounce: 0.12 } satisfies Transition,
  /** Content entering a surface: ~200 ms, delayed until the surface has room. */
  contentIn: { duration: 0.2, ease: [0.2, 0, 0, 1] } satisfies Transition,
  /** Content leaving: quick so it never overlaps the next view. */
  contentOut: { duration: 0.09, ease: [0.4, 0, 1, 1] } satisfies Transition,
  /** Per-widget stagger on dashboard entrance (s). */
  stagger: 0.035,
} as const;

/** Corner radius per layout (logical px). Mirrors --radius-* in tokens.css. */
export const radius = {
  /** Half the pill height: a full capsule. */
  pill: (height: number) => height / 2,
  panel: 30,
} as const;

/**
 * Design-target layouts. The Rust side fits these to the monitor and is the
 * source of truth inside the desktop app; this copy is the fallback for the
 * browser dev server and tests. Mirrors `window::geometry::size`.
 */
export const designLayouts: IslandLayouts = {
  collapsed: { width: 220, height: 44 },
  ambient: { width: 300, height: 44 },
  peek: { width: 360, height: 48 },
  expanded: { width: 820, height: 248 },
  align: "center",
};

/** Pill sizes per size preset. Mirrors `geometry::PillSizes::for_preset`. */
export const PILL_SIZES: Record<
  SizePreset,
  Pick<IslandLayouts, "collapsed" | "ambient" | "peek">
> = {
  compact: {
    collapsed: { width: 200, height: 40 },
    ambient: { width: 280, height: 40 },
    peek: { width: 340, height: 44 },
  },
  standard: {
    collapsed: { width: 220, height: 44 },
    ambient: { width: 300, height: 44 },
    peek: { width: 360, height: 48 },
  },
  comfortable: {
    collapsed: { width: 260, height: 48 },
    ambient: { width: 330, height: 48 },
    peek: { width: 390, height: 52 },
  },
};

export const zIndex = {
  base: 0,
  content: 10,
  overlay: 20,
  toast: 30,
} as const;
