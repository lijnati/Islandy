import { AudioLines, Pause, Play, SkipBack, SkipForward } from "lucide-react";
import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";

import { dispatchIsland } from "../../features/island/island-store";
import { formatDuration } from "../../features/island/format";
import {
  canPerform,
  displaySubtitle,
  displayTitle,
  isPlaying,
} from "../../features/media/media-model";
import { useMedia, useSelectedSession } from "../../features/media/media-store";
import type { MediaAction, MediaSession } from "../../services/types";
import { IconButton } from "../ui/IconButton";
import { WidgetSurface } from "../ui/WidgetSurface";
import { cx } from "../ui/cx";
import { MediaArtwork } from "./Artwork";
import { SessionSwitcher } from "./SessionSwitcher";
import { useMediaPosition } from "./useMediaPosition";

/* ------------------------------------------------------------------------ */
/* View state                                                                */
/* ------------------------------------------------------------------------ */

type MediaView =
  | { kind: "loading" }
  | { kind: "unavailable"; reason: "browser" | "unsupported" | "error" }
  | { kind: "empty" }
  | { kind: "session"; session: MediaSession };

function useMediaView(): MediaView {
  const connection = useMedia((s) => s.connection);
  const status = useMedia((s) => s.snapshot?.status ?? null);
  const session = useSelectedSession();
  if (connection === "unavailable") return { kind: "unavailable", reason: "browser" };
  if (connection !== "connected" || status === null || status === "starting") {
    return { kind: "loading" };
  }
  if (status === "unsupported") return { kind: "unavailable", reason: "unsupported" };
  if (status === "unavailable") return { kind: "unavailable", reason: "error" };
  return session ? { kind: "session", session } : { kind: "empty" };
}

const UNAVAILABLE_TEXT = {
  browser: ["Media unavailable", "Live media appears in the Islandy desktop app."],
  unsupported: ["Media unavailable", "Windows media sessions aren't available on this system."],
  error: ["Media unavailable", "Couldn't reach Windows media sessions. Trying again shortly."],
} as const;

/* ------------------------------------------------------------------------ */
/* Controls                                                                  */
/* ------------------------------------------------------------------------ */

export function MediaControls({
  session,
  size = "md",
}: {
  session: MediaSession;
  size?: "md" | "lg";
}) {
  const command = useMedia((s) => s.command);
  const pending = useMedia((s) => s.pending[session.id]);
  const playing = isPlaying(session);
  const app = session.sourceAppName;

  const button = (action: MediaAction, label: string) => {
    const supported = canPerform(session, action);
    return {
      label: supported ? label : `${label} (not supported by ${app})`,
      tooltip: supported ? label : `Not supported by ${app}`,
      disabled: !supported,
      busy: pending === action,
      onClick: () => void command(action),
    };
  };

  return (
    <div className={cx("flex items-center", size === "lg" ? "gap-3" : "gap-1.5")}>
      <IconButton
        icon={SkipBack}
        {...button("previous", "Previous track")}
        tooltipSide="top"
        size={size === "lg" ? "md" : "sm"}
      />
      <IconButton
        icon={playing ? Pause : Play}
        {...button("togglePlayPause", playing ? "Pause" : "Play")}
        tooltipSide="top"
        variant="solid"
        size={size === "lg" ? "lg" : "md"}
      />
      <IconButton
        icon={SkipForward}
        {...button("next", "Next track")}
        tooltipSide="top"
        tooltipAlign={size === "lg" ? "end" : "center"}
        size={size === "lg" ? "md" : "sm"}
      />
    </div>
  );
}

/* ------------------------------------------------------------------------ */
/* Timeline                                                                  */
/* ------------------------------------------------------------------------ */

const KEY_STEP_MS = 5000;

/**
 * Progress bar with elapsed / remaining time. Seekable (pointer drag, arrow
 * keys) only when the app supports it and reports a duration.
 */
export function MediaTimeline({
  session,
  className,
}: {
  session: MediaSession;
  className?: string | undefined;
}) {
  const position = useMediaPosition(session);
  const seek = useMedia((s) => s.seek);
  const pendingSeek = useMedia((s) => s.pending[session.id] === "seek");
  const trackRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<number | null>(null);

  const duration = session.timeline?.durationMs ?? null;
  const canSeek = session.controls.seek && duration !== null && duration > 0;
  const shown = drag ?? position;
  const ratio = shown !== null && duration ? Math.min(1, shown / duration) : 0;

  const fromPointer = (event: PointerEvent<HTMLDivElement>) => {
    const rect = trackRef.current?.getBoundingClientRect();
    if (!rect || !duration || rect.width === 0) return 0;
    return Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)) * duration;
  };

  const seekProps = canSeek
    ? {
        role: "slider" as const,
        tabIndex: 0,
        "aria-label": "Seek",
        "aria-valuemin": 0,
        "aria-valuemax": Math.round(duration / 1000),
        "aria-valuenow": Math.round((shown ?? 0) / 1000),
        "aria-valuetext": `${formatDuration(shown ?? 0)} of ${formatDuration(duration)}`,
        "aria-busy": pendingSeek || undefined,
        onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
          event.currentTarget.setPointerCapture(event.pointerId);
          setDrag(fromPointer(event));
        },
        onPointerMove: (event: PointerEvent<HTMLDivElement>) => {
          if (drag !== null) setDrag(fromPointer(event));
        },
        onPointerUp: (event: PointerEvent<HTMLDivElement>) => {
          if (drag === null) return;
          setDrag(null);
          void seek(fromPointer(event));
        },
        onPointerCancel: () => {
          setDrag(null);
        },
        onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => {
          const base = shown ?? 0;
          const target =
            event.key === "ArrowRight"
              ? base + KEY_STEP_MS
              : event.key === "ArrowLeft"
                ? base - KEY_STEP_MS
                : event.key === "Home"
                  ? 0
                  : event.key === "End"
                    ? duration
                    : null;
          if (target === null) return;
          event.preventDefault();
          void seek(target);
        },
      }
    : {
        role: "progressbar" as const,
        "aria-label": "Playback position",
        "aria-valuemin": 0,
        "aria-valuemax": duration ? Math.round(duration / 1000) : undefined,
        "aria-valuenow": shown !== null && duration ? Math.round(shown / 1000) : undefined,
        "aria-valuetext":
          shown === null
            ? "Position unknown"
            : duration
              ? `${formatDuration(shown)} of ${formatDuration(duration)}`
              : formatDuration(shown),
      };

  return (
    <div className={cx("flex flex-col gap-1", className)}>
      <div
        {...seekProps}
        className={cx(
          "group/seek relative -my-1.5 py-1.5 outline-hidden",
          canSeek && "cursor-pointer touch-none",
        )}
      >
        <div
          ref={trackRef}
          className={cx(
            "h-1 overflow-hidden rounded-full bg-surface-hover transition-[height] duration-fast",
            canSeek && "group-hover/seek:h-1.5 group-focus-visible/seek:h-1.5",
            "group-focus-visible/seek:outline-2 group-focus-visible/seek:outline-offset-2 group-focus-visible/seek:outline-focus group-focus-visible/seek:outline-solid",
          )}
        >
          <div
            className={cx(
              "h-full origin-left rounded-full bg-fg",
              // Smooth 1 s ticks while playing; jump instantly when dragging/seeking.
              drag === null &&
                isPlaying(session) &&
                "transition-transform duration-[1000ms] ease-linear",
            )}
            style={{ transform: `scaleX(${ratio})` }}
          />
        </div>
      </div>
      <div className="flex justify-between text-caption text-fg-muted tabular-nums">
        <span>{shown === null ? "--:--" : formatDuration(shown)}</span>
        <span>
          {duration === null
            ? shown === null
              ? "--:--"
              : "Live"
            : `-${formatDuration(duration - (shown ?? 0))}`}
        </span>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------------ */
/* Shared pieces                                                             */
/* ------------------------------------------------------------------------ */

function Notice() {
  const notice = useMedia((s) => s.notice);
  return (
    <p role="status" className="min-h-[14px] truncate text-caption text-status-attention">
      {notice?.message ?? ""}
    </p>
  );
}

function TrackText({ session, size }: { session: MediaSession; size: "md" | "lg" }) {
  if (!session.metadataLoaded) {
    return (
      <div aria-label="Loading track information" className="flex flex-col gap-2 py-0.5">
        <span className="h-3.5 w-3/4 animate-pulse rounded-full bg-surface-hover motion-reduce:animate-none" />
        <span className="h-3 w-1/2 animate-pulse rounded-full bg-surface-hover motion-reduce:animate-none" />
      </div>
    );
  }
  const title = displayTitle(session);
  const subtitle = displaySubtitle(session);
  const detail =
    size === "lg" && session.album && session.album !== session.title
      ? `${subtitle} · ${session.album}`
      : subtitle;
  return (
    <>
      <p
        title={title}
        className={cx(
          "truncate font-semibold tracking-tight",
          size === "lg" ? "text-heading" : "text-title",
          session.title === null ? "text-fg-muted" : "text-fg",
        )}
      >
        {title}
      </p>
      <p title={detail} className="truncate text-body text-fg-muted">
        {detail}
      </p>
    </>
  );
}

function StateMessage({ view }: { view: Exclude<MediaView, { kind: "session" }> }) {
  if (view.kind === "loading") {
    return (
      <div aria-label="Connecting to media" className="flex min-h-0 flex-1 gap-3">
        <div className="size-[84px] shrink-0 animate-pulse rounded-art bg-surface-elevated motion-reduce:animate-none" />
        <div className="flex flex-1 flex-col justify-center gap-2">
          <span className="h-3.5 w-3/4 animate-pulse rounded-full bg-surface-hover motion-reduce:animate-none" />
          <span className="h-3 w-1/2 animate-pulse rounded-full bg-surface-hover motion-reduce:animate-none" />
        </div>
      </div>
    );
  }
  const [title, body] =
    view.kind === "empty"
      ? ["Nothing playing", "Play music or video in an app that shows Windows media controls."]
      : UNAVAILABLE_TEXT[view.reason];
  return (
    <div className="flex min-h-0 flex-1 items-center gap-3">
      <span
        aria-hidden="true"
        className="flex size-[84px] shrink-0 items-center justify-center rounded-art border border-hairline bg-surface-raised text-fg-subtle"
      >
        <AudioLines size={24} strokeWidth={1.5} />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="text-title font-semibold tracking-tight text-fg">{title}</p>
        <p className="text-caption text-fg-muted">{body}</p>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------------ */
/* Widget + detail                                                           */
/* ------------------------------------------------------------------------ */

/** The dashboard media widget, fed by Windows media sessions (GSMTC). */
export function MediaWidget({ className }: { className?: string | undefined }) {
  const view = useMediaView();
  return (
    <WidgetSurface
      title="Now Playing"
      className={className}
      accessory={view.kind === "session" ? <SessionSwitcher /> : undefined}
      onOpen={() => {
        dispatchIsland({ type: "OPEN_WIDGET", widget: "media" });
      }}
    >
      {view.kind === "session" ? (
        <>
          <div className="flex min-h-0 flex-1 gap-3">
            <MediaArtwork
              artworkKey={view.session.artworkKey}
              label={`Artwork for ${view.session.album ?? displayTitle(view.session)}`}
              className="size-[84px]"
            />
            <div className="flex min-w-0 flex-1 flex-col justify-center gap-0.5">
              <TrackText session={view.session} size="md" />
              <Notice />
            </div>
          </div>
          <div className="mt-2 flex items-end gap-3">
            <MediaTimeline session={view.session} className="min-w-0 flex-1" />
            <MediaControls session={view.session} />
          </div>
        </>
      ) : (
        <StateMessage view={view} />
      )}
    </WidgetSurface>
  );
}

const STATUS_LABEL: Record<MediaSession["playbackStatus"], string> = {
  playing: "Playing",
  paused: "Paused",
  stopped: "Stopped",
  changing: "Loading",
  unknown: "Player",
};

export function MediaDetail() {
  const view = useMediaView();
  if (view.kind !== "session") {
    return (
      <div className="flex h-full rounded-tile border border-hairline bg-surface-secondary p-3 shadow-surface">
        <StateMessage view={view} />
      </div>
    );
  }
  const { session } = view;
  return (
    <div className="flex h-full gap-5 rounded-tile border border-hairline bg-surface-secondary p-3 shadow-surface">
      <MediaArtwork
        artworkKey={session.artworkKey}
        label={`Artwork for ${session.album ?? displayTitle(session)}`}
        className="aspect-square h-full"
        iconSize={36}
      />
      <div className="flex min-w-0 flex-1 flex-col justify-between py-1">
        <div className="flex flex-col gap-1">
          <div className="flex h-5 items-center justify-between gap-2">
            <span className="text-caption font-medium tracking-label text-fg-muted uppercase">
              {STATUS_LABEL[session.playbackStatus]}
            </span>
            <SessionSwitcher />
          </div>
          <TrackText session={session} size="lg" />
          <Notice />
        </div>
        <div className="flex items-end gap-5">
          <MediaTimeline session={session} className="min-w-0 flex-1" />
          <MediaControls session={session} size="lg" />
        </div>
      </div>
    </div>
  );
}
