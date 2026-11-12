import { Sparkle } from "lucide-react";

import { dispatchIsland } from "../../features/island/island-store";
import { formatTime } from "../../features/island/format";
import { currentTrack, useDemoMedia } from "../../features/media/demo-media";
import { useNow } from "../../hooks/useNow";
import { focusRing } from "../ui/IconButton";
import { cx } from "../ui/cx";
import { ActivityBars } from "./ActivityBars";

/**
 * The resting pill: ambient glyph · time · media indicator.
 * The whole pill is one button (click or Enter/Space to open and pin).
 */
export function CollapsedIsland() {
  const now = useNow();
  const playing = useDemoMedia((s) => s.isPlaying);
  const track = useDemoMedia(currentTrack);
  const time = formatTime(now);

  return (
    <button
      type="button"
      onClick={() => {
        dispatchIsland({ type: "PRESS" });
      }}
      aria-label={`Islandy, ${time}${playing ? `, playing ${track.title}` : ""}. Open dashboard`}
      className={cx(
        "group/pill flex h-full w-full items-center justify-between rounded-full px-4",
        focusRing,
        "focus-visible:-outline-offset-4",
      )}
    >
      <Sparkle
        size={13}
        strokeWidth={2}
        aria-hidden="true"
        className="text-fg-muted transition-colors duration-fast group-hover/pill:text-fg"
      />
      <time
        dateTime={now.toISOString()}
        className="text-body font-semibold tracking-tight text-fg tabular-nums"
      >
        {time}
      </time>
      <ActivityBars active={playing} />
    </button>
  );
}
