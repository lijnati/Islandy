import { useSyncExternalStore } from "react";

const QUERY = "(prefers-reduced-motion: reduce)";

function subscribe(onChange: () => void): () => void {
  const media = window.matchMedia(QUERY);
  media.addEventListener("change", onChange);
  return () => {
    media.removeEventListener("change", onChange);
  };
}

const getSnapshot = () => window.matchMedia(QUERY).matches;

/**
 * Live `prefers-reduced-motion` (Windows: Settings → Accessibility → Visual
 * effects → Animation effects). Unlike a one-time read, it follows changes
 * made while Islandy is running.
 */
export function usePrefersReducedMotion(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, () => false);
}
