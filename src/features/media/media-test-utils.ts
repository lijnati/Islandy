import type { MediaSession, MediaSnapshot } from "../../services/types";

/** Test fixtures shaped exactly like Rust's serialized snapshots. */
export function session(overrides: Partial<MediaSession> = {}): MediaSession {
  return {
    id: "s1",
    sourceAppId: "Player.exe",
    sourceAppName: "Player",
    title: "Song",
    artist: "Artist",
    album: "Album",
    albumArtist: null,
    trackNumber: null,
    artworkKey: null,
    metadataLoaded: true,
    playbackStatus: "playing",
    playbackRate: 1,
    controls: {
      play: true,
      pause: true,
      togglePlayPause: true,
      next: true,
      previous: true,
      seek: true,
    },
    timeline: { positionMs: 10_000, durationMs: 200_000, updatedAtMs: 1_000_000 },
    isSystemCurrent: true,
    ...overrides,
  };
}

export function snapshot(overrides: Partial<MediaSnapshot> = {}): MediaSnapshot {
  const sessions = overrides.sessions ?? [session()];
  return {
    revision: 1,
    status: "ready",
    sessions,
    selectedSessionId: sessions[0]?.id ?? null,
    selectionMode: "auto",
    ...overrides,
  };
}
