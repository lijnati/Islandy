import { useEffect, useState } from "react";

import { positionAt } from "../../features/media/media-model";
import { useMedia } from "../../features/media/media-store";
import type { MediaSession } from "../../services/types";

/**
 * Displayed playback position for a *visible* media view, interpolated from
 * the last native timeline update. Ticks once per second only while mounted
 * and playing; the collapsed pill does not use it, so nothing ticks while the
 * island is collapsed or the media is paused.
 */
export function useMediaPosition(session: MediaSession | null): number | null {
  const preview = useMedia((s) => s.seekPreview);
  const [now, setNow] = useState(() => Date.now());
  const ticking = session?.playbackStatus === "playing" && session.timeline !== null;

  useEffect(() => {
    if (!ticking) return undefined;
    const id = setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      clearInterval(id);
    };
  }, [ticking]);

  if (!session) return null;
  // `now` may lag a fresh native update by up to a tick; never go backwards.
  const t = Math.max(now, session.timeline?.updatedAtMs ?? 0, preview?.at ?? 0);
  return positionAt(session, t, preview);
}
