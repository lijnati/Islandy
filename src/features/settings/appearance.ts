import type { Transition } from "motion/react";
import { useEffect } from "react";

import { motion } from "../../design/tokens";
import { usePrefersReducedMotion } from "../../hooks/usePrefersReducedMotion";
import type { AppearanceSettings, Corners } from "../../services/types";
import { useSettings } from "./settings-store";

/** Dashboard corner radius per preset. Mirrors `--radius-panel` in tokens.css. */
export const PANEL_RADIUS: Record<Corners, number> = { standard: 30, compact: 22, round: 34 };

/**
 * Applies the theme, accent, corners and opacity to a document root. Only
 * attributes and one custom property change; every color comes from the
 * tokens in tokens.css.
 */
export function applyAppearance(root: HTMLElement, a: AppearanceSettings): void {
  root.dataset.theme = a.theme;
  root.dataset.accent = a.accent;
  root.dataset.corners = a.corners;
  root.style.setProperty("--island-alpha", String(Math.min(100, Math.max(70, a.opacity)) / 100));
}

export function resolveReducedMotion(
  setting: AppearanceSettings["reducedMotion"],
  systemPrefersReduced: boolean,
): boolean {
  return setting === "on" || (setting === "system" && systemPrefersReduced);
}

export interface MotionProfile {
  reduce: boolean;
  expand: Transition;
  collapse: Transition;
  peek: Transition;
}

/** Springs for the island surface at the chosen intensity. */
export function motionProfile(
  level: AppearanceSettings["animation"],
  reduce: boolean,
): MotionProfile {
  if (reduce) {
    const none = { duration: 0 };
    return { reduce, expand: none, collapse: none, peek: none };
  }
  if (level === "subtle") {
    return {
      reduce,
      expand: { type: "spring", visualDuration: 0.26, bounce: 0, restDelta: 0.5 },
      collapse: { type: "spring", visualDuration: 0.2, bounce: 0, restDelta: 0.5 },
      peek: { type: "spring", visualDuration: 0.18, bounce: 0 },
    };
  }
  return { reduce, expand: motion.expand, collapse: motion.collapse, peek: motion.peek };
}

/** Keeps `document.documentElement` in sync with the appearance settings. */
export function useApplyAppearance(): void {
  const appearance = useSettings((s) => s.appearance);
  useEffect(() => {
    applyAppearance(document.documentElement, appearance);
  }, [appearance]);
}

/** The motion profile for the current settings and system preference. */
export function useMotionProfile(): MotionProfile {
  const level = useSettings((s) => s.appearance.animation);
  const setting = useSettings((s) => s.appearance.reducedMotion);
  const system = usePrefersReducedMotion();
  return motionProfile(level, resolveReducedMotion(setting, system));
}
