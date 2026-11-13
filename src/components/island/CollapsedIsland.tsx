import { Moon, Sparkle } from "lucide-react";
import type { ReactNode } from "react";

import {
  ambientLabel,
  isWorking,
  needsAttention,
  type Ambient,
} from "../../features/activity/activity-model";
import { useActivity } from "../../features/activity/activity-store";
import { openClaudeSession } from "../../features/activity/navigation";
import { useAmbient, useQuiet } from "../../features/activity/use-ambient";
import { dispatchIsland } from "../../features/island/island-store";
import { useDragReposition } from "../../features/island/use-drag-reposition";
import { formatTime } from "../../features/island/format";
import { displayTitle, isActive, isPlaying } from "../../features/media/media-model";
import { useSelectedSession } from "../../features/media/media-store";
import { useNow } from "../../hooks/useNow";
import { ACTIVITY_ICONS } from "../activity/icons";
import { ClaudeGlyph, type GlyphState } from "../activity/ClaudeGlyph";
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
        <ClaudeMark>
          <ClaudeGlyph state={attention ? "attention" : "working"} size={14} />
        </ClaudeMark>
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

/** Marks the Claude logo: a click on it opens the Claude status panel. */
const CLAUDE_MARK = "data-claude-mark";

function ClaudeMark({ children }: { children: ReactNode }) {
  return (
    <span
      {...{ [CLAUDE_MARK]: "" }}
      title="Claude status"
      className="-m-1 flex shrink-0 cursor-pointer items-center justify-center rounded-full p-1 transition-[background-color,transform] duration-fast hover:scale-110 hover:bg-surface-hover active:scale-95"
    >
      {children}
    </span>
  );
}

const STATE_SPEECH: Record<GlyphState, string> = {
  idle: "Claude idle",
  working: "Claude working",
  attention: "Claude needs attention",
};

/** Claude's overall state for the resting pill, or `null` with the Claude source off. */
function useClaudeState(): GlyphState | null {
  return useActivity((s) => {
    const snapshot = s.snapshot;
    if (snapshot && !snapshot.settings.sources.claudeCode) return null;
    const sessions = snapshot?.sessions ?? [];
    if (sessions.some(needsAttention)) return "attention";
    if (sessions.some(isWorking)) return "working";
    return "idle";
  });
}

function ambientSpeech(a: Ambient): string {
  const { primary, secondary } = ambientLabel(a);
  if (a.kind === "claude" && a.active === 1) {
    return `Claude, ${primary}: ${needsAttention(a.session) ? "needs attention" : secondary}`;
  }
  return a.kind === "claude" ? `Claude, ${primary}, ${secondary}` : `${primary}: ${secondary}`;
}

/**
 * The resting pill: (ambient glyph | track artwork) · time · Claude mark ·
 * playback indicator. The Claude mark is orange and breathes slowly at rest,
 * turns while Claude works and pulses amber when it needs you (hidden with
 * the Claude source off; still with reduced motion).
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
  const claude = useClaudeState();
  const { handlers, consumeDragClick } = useDragReposition();
  const media = isActive(session) ? session : null;
  const playing = isPlaying(media);
  const time = formatTime(now);
  const mediaLabel = media
    ? `, ${playing ? "playing" : "paused"} ${displayTitle(media)}${media.artist ? ` by ${media.artist}` : ""}`
    : "";
  const claudeLabel = claude ? `. ${STATE_SPEECH[claude]}` : "";
  const label = ambient
    ? `Islandy, ${time}. ${ambientSpeech(ambient)}${quiet ? ". Quiet mode on" : ""}. Open dashboard`
    : `Islandy, ${time}${mediaLabel}${claudeLabel}${quiet ? ". Quiet mode on" : ""}. Open dashboard`;

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
      onClick={(e) => {
        if (consumeDragClick()) return;
        dispatchIsland({ type: "PRESS" });
        // A click on the Claude logo goes straight to the Claude status panel.
        if (e.target instanceof Element && e.target.closest(`[${CLAUDE_MARK}]`)) {
          openClaudeSession(null);
        }
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
          {claude && (
            <ClaudeMark>
              <ClaudeGlyph state={claude} size={15} brand />
            </ClaudeMark>
          )}
          <ActivityBars active={playing} />
        </>
      )}
    </button>
  );
}
