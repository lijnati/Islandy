/**
 * Design values that JavaScript needs at runtime.
 *
 * Visual tokens (colors, radii, typography) live in `src/styles/tokens.css`.
 * Only mirror a value here when code must read it, and keep both in sync.
 */
export const motion = {
  /** Milliseconds; mirrors --duration-* in tokens.css. */
  duration: { fast: 120, base: 220, slow: 360 },
  /** How long a `peek` stays visible before returning to `collapsed`. */
  peekDwellMs: 4000,
} as const;

export const zIndex = {
  base: 0,
  content: 10,
  overlay: 20,
  toast: 30,
} as const;
