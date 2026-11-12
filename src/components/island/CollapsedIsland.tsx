import { Sparkle } from "lucide-react";

import { dispatchIsland } from "../../features/island/island-store";
import { formatTime } from "../../features/island/format";
import { displayTitle, isActive, isPlaying } from "../../features/media/media-model";
import { useSelectedSession } from "../../features/media/media-store";
import { useNow } from "../../hooks/useNow";
import { MediaArtwork } from "../widgets/Artwork";
import { focusRing } from "../ui/IconButton";
import { cx } from "../ui/cx";
import { ActivityBars } from "./ActivityBars";

/**
 * The resting pill: (ambient glyph | track artwork) · time · playback indicator.
 * With an active media session the glyph becomes the artwork thumbnail and
 * the bars animate while it plays (a playback-state animation, not an audio
 * visualization). The whole pill is one button (click or Enter/Space to open
 * and pin).
 */
export function CollapsedIsland() {
  const now = useNow();
  const session = useSelectedSession();
  const media = isActive(session) ? session : null;
  const playing = isPlaying(media);
  const time = formatTime(now);
  const mediaLabel = media
    ? `, ${playing ? "playing" : "paused"} ${displayTitle(media)}${media.artist ? ` by ${media.artist}` : ""}`
    : "";

  return (
    <button
      type="button"
      onClick={() => {
        dispatchIsland({ type: "PRESS" });
      }}
      aria-label={`Islandy, ${time}${mediaLabel}. Open dashboard`}
      className={cx(
        "group/pill flex h-full w-full items-center justify-between rounded-full",
        media ? "pr-4 pl-2.5" : "px-4",
        focusRing,
        "focus-visible:-outline-offset-4",
      )}
    >
      {media ? (
        <MediaArtwork
          artworkKey={media.artworkKey}
          label=""
          iconSize={12}
          rounded="rounded-[7px]"
          className="size-6 shadow-none"
        />
      ) : (
        <Sparkle
          size={13}
          strokeWidth={2}
          aria-hidden="true"
          className="text-fg-muted transition-colors duration-fast group-hover/pill:text-fg"
        />
      )}
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
