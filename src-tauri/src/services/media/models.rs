//! Normalized media types that cross the IPC boundary.
//!
//! Mirrored by the `Media*` types in `src/services/types.ts`. Everything is
//! serialized with `camelCase` keys; tests below pin the key sets.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

/// A stable identifier for one discovered media session, valid for the
/// session's lifetime (and never reused while Islandy runs).
///
/// Source app IDs are *not* unique per session (a browser can expose several
/// sessions under one AUMID), so ids are allocated by Islandy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

impl SessionId {
    /// Parses the wire form (`"s12"`). Rejects anything else.
    pub fn parse(value: &str) -> Option<Self> {
        let digits = value.strip_prefix('s')?;
        if digits.is_empty() || digits.len() > 19 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok().map(SessionId)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s{}", self.0)
    }
}

impl Serialize for SessionId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Normalized playback status.
///
/// GSMTC `Closed` and `Opened` both mean "a session exists but nothing is
/// playing", so they map to `Stopped`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
    /// The app is switching tracks / buffering.
    Changing,
    Unknown,
}

/// Which controls the source app currently accepts. Reported by the app;
/// Islandy never assumes a control works because another app supports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaControls {
    pub play: bool,
    pub pause: bool,
    pub toggle_play_pause: bool,
    pub next: bool,
    pub previous: bool,
    /// The app accepts position changes *and* reports a known duration.
    pub seek: bool,
}

/// Playback position as reported by the app at `updated_at_ms`.
/// The UI interpolates from here while playing; native updates win.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    /// Position relative to the start of the media, clamped to the duration.
    pub position_ms: u64,
    /// `None` when the app reports no usable end time (live streams, some players).
    pub duration_ms: Option<u64>,
    /// Unix epoch milliseconds when `position_ms` was valid.
    pub updated_at_ms: u64,
}

/// Track metadata. Missing values stay `None`; nothing is invented.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
}

/// One media session, normalized.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSession {
    pub id: SessionId,
    /// The app's AppUserModelID as reported by Windows (may be opaque).
    pub source_app_id: String,
    /// A human-readable app name (best effort).
    pub source_app_name: String,
    #[serde(flatten)]
    pub metadata: Metadata,
    /// Content key of the artwork; fetch with `get_media_artwork`.
    pub artwork_key: Option<String>,
    /// False until metadata has been read at least once (UI "loading").
    pub metadata_loaded: bool,
    pub playback_status: PlaybackStatus,
    /// Playback rate (1.0 = normal). Always finite and positive.
    pub playback_rate: f64,
    pub controls: MediaControls,
    pub timeline: Option<Timeline>,
    /// Windows considers this the current session.
    pub is_system_current: bool,
}

/// State of the native integration as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationStatus {
    /// The worker has not connected to Windows yet.
    Starting,
    Ready,
    /// Windows media sessions could not be reached (retried with backoff).
    Unavailable,
    /// Not supported on this platform (non-Windows development hosts).
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SelectionMode {
    /// Islandy follows the selection policy.
    Auto,
    /// The user picked a session; kept until it disappears or they pick again.
    User,
}

/// Everything the UI needs, pushed whenever it changes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSnapshot {
    /// Strictly increasing; the UI ignores snapshots older than what it has.
    pub revision: u64,
    pub status: IntegrationStatus,
    pub sessions: Vec<MediaSession>,
    pub selected_session_id: Option<SessionId>,
    pub selection_mode: SelectionMode,
}

impl MediaSnapshot {
    pub fn initial() -> Self {
        Self {
            revision: 0,
            status: IntegrationStatus::Starting,
            sessions: Vec::new(),
            selected_session_id: None,
            selection_mode: SelectionMode::Auto,
        }
    }
}

/// Playback commands accepted from the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaAction {
    Play,
    Pause,
    TogglePlayPause,
    Next,
    Previous,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    #[test]
    fn session_id_round_trips_and_rejects_garbage() {
        assert_eq!(SessionId(42).to_string(), "s42");
        assert_eq!(SessionId::parse("s42"), Some(SessionId(42)));
        for bad in [
            "",
            "s",
            "42",
            "s-1",
            "s1a",
            "S1",
            " s1",
            "s99999999999999999999",
        ] {
            assert_eq!(SessionId::parse(bad), None, "{bad:?}");
        }
        assert_eq!(serde_json::to_value(SessionId(7)).unwrap(), "s7");
    }

    #[test]
    fn session_serializes_with_flat_camel_case_keys() {
        let session = MediaSession {
            id: SessionId(1),
            source_app_id: "App.exe".into(),
            source_app_name: "App".into(),
            metadata: Metadata {
                title: Some("T".into()),
                ..Metadata::default()
            },
            artwork_key: None,
            metadata_loaded: true,
            playback_status: PlaybackStatus::Playing,
            playback_rate: 1.0,
            controls: MediaControls::default(),
            timeline: Some(Timeline {
                position_ms: 1,
                duration_ms: None,
                updated_at_ms: 2,
            }),
            is_system_current: true,
        };
        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(
            keys(&json),
            [
                "album",
                "albumArtist",
                "artist",
                "artworkKey",
                "controls",
                "id",
                "isSystemCurrent",
                "metadataLoaded",
                "playbackRate",
                "playbackStatus",
                "sourceAppId",
                "sourceAppName",
                "timeline",
                "title",
                "trackNumber"
            ]
        );
        assert_eq!(json["playbackStatus"], "playing");
        assert_eq!(
            keys(&json["controls"]),
            [
                "next",
                "pause",
                "play",
                "previous",
                "seek",
                "togglePlayPause"
            ]
        );
        assert_eq!(
            keys(&json["timeline"]),
            ["durationMs", "positionMs", "updatedAtMs"]
        );
    }

    #[test]
    fn snapshot_and_action_wire_formats() {
        let json = serde_json::to_value(MediaSnapshot::initial()).unwrap();
        assert_eq!(
            keys(&json),
            [
                "revision",
                "selectedSessionId",
                "selectionMode",
                "sessions",
                "status"
            ]
        );
        assert_eq!(json["status"], "starting");
        assert_eq!(json["selectionMode"], "auto");
        let action: MediaAction = serde_json::from_str("\"togglePlayPause\"").unwrap();
        assert_eq!(action, MediaAction::TogglePlayPause);
        assert!(serde_json::from_str::<MediaAction>("\"stop\"").is_err());
    }
}
