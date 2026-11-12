/**
 * Pure media helpers: selection, timeline interpolation, capability checks
 * and display text. No React, no IPC; see media-store.ts for state.
 */
import type { PeekContent } from "../island/island-store";
import type {
  MediaAction,
  MediaSession,
  MediaSnapshot,
  NativeErrorKind,
} from "../../services/types";

export const UNKNOWN_TRACK = "Unknown track";

export function selectedSession(snapshot: MediaSnapshot | null): MediaSession | null {
  if (!snapshot || snapshot.selectedSessionId === null) return null;
  return snapshot.sessions.find((s) => s.id === snapshot.selectedSessionId) ?? null;
}

export function isPlaying(session: MediaSession | null): boolean {
  return session?.playbackStatus === "playing";
}

/** Sessions that have something to show in the collapsed island. */
export function isActive(session: MediaSession | null): boolean {
  return (
    session !== null &&
    (session.playbackStatus === "playing" ||
      session.playbackStatus === "paused" ||
      session.playbackStatus === "changing")
  );
}

export function displayTitle(session: MediaSession): string {
  return session.title ?? UNKNOWN_TRACK;
}

/** Artist, else album artist, else the app's name. Never invented. */
export function displaySubtitle(session: MediaSession): string {
  return session.artist ?? session.albumArtist ?? session.sourceAppName;
}

/** A position the user just sought to, shown until Windows confirms it. */
export interface SeekPreview {
  sessionId: string;
  positionMs: number;
  /** Unix ms when the seek was issued. */
  at: number;
}

/** How long a seek preview may stand in for a missing native confirmation. */
export const SEEK_PREVIEW_MS = 3000;

/**
 * The position to display at `now`. Native timeline values are authoritative;
 * while playing, the position advances from the last native update at the
 * playback rate, clamped to [0, duration]. Paused/stopped positions do not move.
 */
export function positionAt(
  session: MediaSession,
  now: number,
  preview: SeekPreview | null = null,
): number | null {
  const timeline = session.timeline;
  if (!timeline) return null;
  const usePreview =
    preview !== null &&
    preview.sessionId === session.id &&
    now - preview.at < SEEK_PREVIEW_MS &&
    timeline.updatedAtMs < preview.at;
  const base = usePreview ? preview.positionMs : timeline.positionMs;
  const since = usePreview ? preview.at : timeline.updatedAtMs;
  const rate =
    Number.isFinite(session.playbackRate) && session.playbackRate > 0 ? session.playbackRate : 1;
  const elapsed = session.playbackStatus === "playing" ? Math.max(0, now - since) * rate : 0;
  const position = Math.max(0, base + elapsed);
  return timeline.durationMs === null ? position : Math.min(timeline.durationMs, position);
}

/** Mirrors the Rust `resolve_action`: whether the app supports `action` now. */
export function canPerform(session: MediaSession, action: MediaAction): boolean {
  const c = session.controls;
  switch (action) {
    case "play":
      return c.play;
    case "pause":
      return c.pause;
    case "next":
      return c.next;
    case "previous":
      return c.previous;
    case "togglePlayPause":
      return c.togglePlayPause || (session.playbackStatus === "playing" ? c.pause : c.play);
  }
}

/** A valid seek target (clamped to the duration), or `null` if seeking is impossible. */
export function validateSeek(session: MediaSession, positionMs: number): number | null {
  const duration = session.timeline?.durationMs ?? null;
  if (!session.controls.seek || duration === null || !Number.isFinite(positionMs)) return null;
  return Math.round(Math.min(duration, Math.max(0, positionMs)));
}

/** Identity of "the track playing in this session". */
export function trackKey(session: MediaSession): string {
  return [session.id, session.title ?? "", session.artist ?? "", session.album ?? ""].join(
    "\u0000",
  );
}

/**
 * Whether a snapshot change deserves a brief peek: the selected session
 * started a *different* track while playing (including music starting when
 * nothing played). Pause/resume, metadata refreshes and the very first
 * snapshot never peek.
 */
export function peekForChange(
  previous: MediaSnapshot | null,
  next: MediaSnapshot,
): PeekContent | null {
  if (!previous) return null;
  const now = selectedSession(next);
  if (!now || !isPlaying(now) || now.title === null) return null;
  const before = selectedSession(previous);
  if (before && trackKey(before) === trackKey(now)) return null;
  // A session that was already playing this track elsewhere (selection moved).
  const same = previous.sessions.find((s) => s.id === now.id);
  if (same && trackKey(same) === trackKey(now) && isPlaying(same)) return null;
  return {
    source: now.sourceAppName,
    title: now.title,
    detail: now.artist ?? now.albumArtist ?? "Now playing",
    tone: "neutral",
    artworkKey: now.artworkKey,
  };
}

/** Neutral, user-facing wording for a failed media command. */
export function commandErrorMessage(kind: NativeErrorKind | "unknown", appName: string): string {
  switch (kind) {
    case "unsupported":
      return `${appName} doesn't support that`;
    case "rejected":
      return `${appName} didn't respond to that`;
    case "notFound":
      return `${appName} is no longer playing`;
    case "timeout":
      return `${appName} isn't responding`;
    default:
      return "Couldn't control playback";
  }
}
