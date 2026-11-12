import { useEffect, useState } from "react";

import { positionAt, useDemoMedia } from "../../features/media/demo-media";

/**
 * Live playback position for a *visible* media view. Ticks once per second
 * only while mounted and playing; the collapsed pill does not use it, so
 * nothing ticks while the island is collapsed.
 */
export function useMediaProgress(): number {
  const state = useDemoMedia((s) => s);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!state.isPlaying) return undefined;
    const id = setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      clearInterval(id);
    };
  }, [state.isPlaying, state.updatedAt]);

  return positionAt(state, Math.max(now, state.updatedAt));
}
