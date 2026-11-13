import { Moon, Sparkle } from "lucide-react";

import { ambientLabel, needsAttention, type Ambient } from "../../features/activity/activity-model";
import { useAmbient, useQuiet } from "../../features/activity/use-ambient";
import { dispatchIsland } from "../../features/island/island-store";
import { useDragReposition } from "../../features/island/use-drag-reposition";
import { formatTime } from "../../features/island/format";
import { displayTitle, isActive, isPlaying } from "../../features/media/media-model";
import { useSelectedSession } from "../../features/media/media-store";
import { useNow } from "../../hooks/useNow";
import { ACTIVITY_ICONS } from "../activity/icons";
import { ClaudeGlyph } from "../activity/ClaudeGlyph";
import { MediaArtwork } from "../widgets/Artwork";
import { focusRing } from "../ui/IconButton";
import { cx } from "../ui/cx";
import { ActivityBars } from "./ActivityBars";

function QuietMark() {
  return (
    <Moon size={11} strokeWidth={2.25} aria-hidden="true" className="shrink-0 text-fg-muted" />
  );
}

/** The left part of the ambient pill: glyph + "project · status". */
function AmbientContent({ ambient }: { ambient: Ambient }) {
  const { primary, secondary } = ambientLabel(ambient);
  const attention = ambient.kind === "claude" && ambient.attention > 0;
  const TaskIcon =
    ambient.kind === "task"
      ? ACTIVITY_ICONS[ambient.event.kind === "buildStarted" ? "build" : "tests"]
      : null;
  return (
    <span className="flex min-w-0 flex-1 items-center gap-2">
      {ambient.kind === "claude" ? (
        <ClaudeGlyph state={attention ? "attention" : "working"} size={14} />
      ) : (
        TaskIcon && (
          <TaskIcon size={13} strokeWidth={2} aria-hidden="true" className="shrink-0 text-fg" />
        )
      )}
      <span className="flex min-w-0 items-baseline text-body tracking-tight">
        <span className="min-w-0 truncate font-semibold text-fg">{primary}</span>
        <span
          className={cx(
            "shrink-0 font-regular whitespace-pre",
            attention ? "text-status-waiting" : "text-fg-muted",
          )}
        >
          {" · "}
          {secondary}
        </span>
      </span>
    </span>
  );
}

function ambientSpeech(a: Ambient): string {
  const { primary, secondary } = ambientLabel(a);
  if (a.kind === "claude" && a.active === 1) {
    return `Claude, ${primary}: ${needsAttention(a.session) ? "needs attention" : secondary}`;
  }
  return a.kind === "claude" ? `Claude, ${primary}, ${secondary}` : `${primary}: ${secondary}`;
}

/**
 * The resting pill: (ambient glyph | track artwork) · time · playback indicator.
 * With an active media session the glyph becomes the artwork thumbnail and
 * the bars animate while it plays (a playback-state animation, not an audio
 * visualization). While a Claude Code session works or needs attention (or a
 * build runs), the pill widens to show it instead of the media. A small moon
 * marks quiet mode. The whole pill is one button (click or Enter/Space to
 * open and pin).
 */
export function CollapsedIsland() {
  const now = useNow();
  const session = useSelectedSession();
  const ambient = useAmbient();
  const quiet = useQuiet();
  const { handlers, consumeDragClick } = useDragReposition();
  const media = isActive(session) ? session : null;
  const playing = isPlaying(media);
  const time = formatTime(now);
  const mediaLabel = media
    ? `, ${playing ? "playing" : "paused"} ${displayTitle(media)}${media.artist ? ` by ${media.artist}` : ""}`
    : "";
  const label = ambient
    ? `Islandy, ${time}. ${ambientSpeech(ambient)}${quiet ? ". Quiet mode on" : ""}. Open dashboard`
    : `Islandy, ${time}${mediaLabel}${quiet ? ". Quiet mode on" : ""}. Open dashboard`;

  const timeEl = (
    <time
      dateTime={now.toISOString()}
      className="shrink-0 text-body font-semibold tracking-tight text-fg tabular-nums"
    >
      {time}
    </time>
  );

  return (
    <button
      type="button"
      onClick={() => {
        if (consumeDragClick()) return;
        dispatchIsland({ type: "PRESS" });
      }}
      {...handlers}
      aria-label={label}
      className={cx(
        "group/pill flex h-full w-full items-center justify-between rounded-full",
        media && !ambient ? "pr-4 pl-2.5" : "px-4",
        focusRing,
        "focus-visible:-outline-offset-4",
      )}
    >
      {ambient ? (
        <>
          <AmbientContent ambient={ambient} />
          <span className="ml-3 flex shrink-0 items-center gap-1.5">
            {quiet && <QuietMark />}
            {timeEl}
          </span>
        </>
      ) : (
        <>
          {media ? (
            <MediaArtwork
              artworkKey={media.artworkKey}
              label=""
              iconSize={12}
              rounded="rounded-[7px]"
              className="size-6 shadow-none"
            />
          ) : quiet ? (
            <Moon size={13} strokeWidth={2} aria-hidden="true" className="text-fg-muted" />
          ) : (
            <Sparkle
              size={13}
              strokeWidth={2}
              aria-hidden="true"
              className="text-fg-muted transition-colors duration-fast group-hover/pill:text-fg"
            />
          )}
          <span className="flex items-center gap-1.5">
            {quiet && media && <QuietMark />}
            {timeEl}
          </span>
          <ActivityBars active={playing} />
        </>
      )}
    </button>
  );
}
