import { useCallback, useEffect, useRef, useState } from "react";

import { isNativeAvailable, setIslandFrame } from "../../services/native";
import type { Size } from "../../services/types";
import { useAmbient } from "../activity/use-ambient";
import { useIsland, type IslandMode } from "./island-store";
import { layoutFor, layoutStore, useLayouts } from "./layouts";
import { WindowCoordinator } from "./window-coordinator";

let warned = false;

/** The app-wide coordinator. In a plain browser there is no native frame. */
export const windowCoordinator = new WindowCoordinator(async (size) => {
  if (!isNativeAvailable()) return;
  try {
    await setIslandFrame(size);
  } catch (error) {
    if (!warned) console.warn("[islandy] set_island_frame failed:", error);
    warned = true;
  }
}, layoutStore.getState().layouts.collapsed);

export interface RenderedFrame {
  /** The mode whose content and size are rendered now. */
  mode: IslandMode;
  /** The previously rendered mode (picks the transition curve). */
  from: IslandMode;
  size: Size;
}

/**
 * Bridges the island mode to the animated surface and the native frame.
 *
 * The rendered mode lags the store's mode only by the time it takes to grow
 * the native frame (one IPC call), so the surface never animates beyond the
 * window's hit/visible area. Call `onAnimationComplete` when the surface's
 * size animation finishes, so the frame can shrink back.
 */
export function useIslandFrame(): RenderedFrame & { onAnimationComplete: () => void } {
  const mode = useIsland((s) => s.mode);
  const layouts = useLayouts();
  const ambient = useAmbient() !== null;
  const target = layoutFor(mode, layouts, ambient);
  const { width, height } = target;

  const [rendered, setRendered] = useState<RenderedFrame>({ mode, from: mode, size: target });
  const token = useRef(0);
  const renderedRef = useRef(rendered);

  useEffect(() => {
    let cancelled = false;
    const size = { width, height };
    void windowCoordinator.begin(size).then((t) => {
      if (cancelled) return;
      token.current = t;
      const next = { mode, from: renderedRef.current.mode, size };
      renderedRef.current = next;
      setRendered(next);
    });
    return () => {
      cancelled = true;
    };
  }, [mode, width, height]);

  const onAnimationComplete = useCallback(() => {
    void windowCoordinator.complete(token.current, renderedRef.current.size);
  }, []);

  return { ...rendered, onAnimationComplete };
}
