import { Pause, Play, SkipBack, SkipForward } from "lucide-react";

import { dispatchIsland } from "../../features/island/island-store";
import { formatDuration } from "../../features/island/format";
import { currentTrack, useDemoMedia } from "../../features/media/demo-media";
import { IconButton } from "../ui/IconButton";
import { WidgetSurface } from "../ui/WidgetSurface";
import { cx } from "../ui/cx";
import { Artwork } from "./Artwork";
import { useMediaProgress } from "./useMediaProgress";

export function MediaControls({ size = "md" }: { size?: "md" | "lg" }) {
  const isPlaying = useDemoMedia((s) => s.isPlaying);
  const { togglePlay, next, previous } = useDemoMedia((s) => s);
  return (
    <div className={cx("flex items-center", size === "lg" ? "gap-3" : "gap-1.5")}>
      <IconButton
        icon={SkipBack}
        label="Previous track (demo)"
        tooltip="Previous"
        tooltipSide="top"
        size={size === "lg" ? "md" : "sm"}
        onClick={previous}
      />
      <IconButton
        icon={isPlaying ? Pause : Play}
        label={isPlaying ? "Pause (demo)" : "Play (demo)"}
        tooltip={isPlaying ? "Pause" : "Play"}
        tooltipSide="top"
        variant="solid"
        size={size === "lg" ? "lg" : "md"}
        onClick={togglePlay}
      />
      <IconButton
        icon={SkipForward}
        label="Next track (demo)"
        tooltip="Next"
        tooltipSide="top"
        size={size === "lg" ? "md" : "sm"}
        onClick={next}
      />
    </div>
  );
}

export function MediaProgress({ className }: { className?: string | undefined }) {
  const track = useDemoMedia(currentTrack);
  const position = useMediaProgress();
  const ratio = track.durationMs > 0 ? position / track.durationMs : 0;
  return (
    <div className={cx("flex flex-col gap-1", className)}>
      <div
        role="progressbar"
        aria-label="Playback position"
        aria-valuemin={0}
        aria-valuemax={Math.round(track.durationMs / 1000)}
        aria-valuenow={Math.round(position / 1000)}
        aria-valuetext={`${formatDuration(position)} of ${formatDuration(track.durationMs)}`}
        className="h-1 overflow-hidden rounded-full bg-surface-hover"
      >
        <div
          className="h-full origin-left rounded-full bg-fg transition-transform duration-[1000ms] ease-linear"
          style={{ transform: `scaleX(${ratio})` }}
        />
      </div>
      <div className="flex justify-between text-caption text-fg-muted tabular-nums">
        <span>{formatDuration(position)}</span>
        <span>-{formatDuration(track.durationMs - position)}</span>
      </div>
    </div>
  );
}

/** DEMO media widget. Not connected to Windows media sessions (Phase 2). */
export function MediaWidget({ className }: { className?: string | undefined }) {
  const track = useDemoMedia(currentTrack);
  return (
    <WidgetSurface
      title="Now Playing"
      demo
      className={className}
      onOpen={() => {
        dispatchIsland({ type: "OPEN_WIDGET", widget: "media" });
      }}
    >
      <div className="flex min-h-0 flex-1 gap-3">
        <Artwork track={track} className="size-[84px]" />
        <div className="flex min-w-0 flex-1 flex-col justify-center gap-0.5">
          <p className="truncate text-title font-semibold tracking-tight text-fg">{track.title}</p>
          <p className="truncate text-body text-fg-muted">{track.artist}</p>
        </div>
      </div>
      <div className="mt-2 flex items-end gap-3">
        <MediaProgress className="min-w-0 flex-1" />
        <MediaControls />
      </div>
    </WidgetSurface>
  );
}

export function MediaDetail() {
  const track = useDemoMedia(currentTrack);
  return (
    <div className="flex h-full gap-5 rounded-tile border border-hairline bg-surface-secondary p-3 shadow-surface">
      <Artwork track={track} className="aspect-square h-full" />
      <div className="flex min-w-0 flex-1 flex-col justify-between py-1">
        <div className="flex flex-col gap-1">
          <p className="truncate text-heading font-semibold tracking-tight text-fg">
            {track.title}
          </p>
          <p className="truncate text-body text-fg-muted">
            {track.artist} · {track.album}
          </p>
          <p className="mt-1 text-caption text-fg-subtle">
            Demo playback. Windows media integration arrives in Phase 2.
          </p>
        </div>
        <div className="flex items-end gap-5">
          <MediaProgress className="min-w-0 flex-1" />
          <MediaControls size="lg" />
        </div>
      </div>
    </div>
  );
}
