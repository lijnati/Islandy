//! Publishing media state to the UI.
//!
//! The UI hands Rust a Tauri `Channel` through `connect_media` (like the
//! island's signal channel, so no `core:event` permission is needed). The hub
//! keeps the latest published snapshot and the artwork cache; both are read
//! by commands without going through the media worker.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::{Arc, Mutex, MutexGuard};

use tauri::ipc::Channel;

use super::{artwork::ArtworkCache, models::MediaSnapshot};

/// Native notifications, reduced to "what to re-read". WinRT event handlers
/// only send these; all reading happens on the media worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // named after the WinRT events
pub enum NativeEvent {
    SessionsChanged,
    CurrentSessionChanged,
    MediaPropertiesChanged(super::models::SessionId),
    PlaybackInfoChanged(super::models::SessionId),
    TimelinePropertiesChanged(super::models::SessionId),
}

struct HubInner {
    channel: Option<Channel<MediaSnapshot>>,
    latest: MediaSnapshot,
    artwork: ArtworkCache,
}

pub struct MediaHub(Mutex<HubInner>);

impl Default for MediaHub {
    fn default() -> Self {
        Self(Mutex::new(HubInner {
            channel: None,
            latest: MediaSnapshot::initial(),
            artwork: ArtworkCache::default(),
        }))
    }
}

impl MediaHub {
    fn lock(&self) -> MutexGuard<'_, HubInner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Replaces the UI channel (a reloaded frontend reconnects) and returns
    /// the current snapshot. Done under one lock, so no update can fall
    /// between the returned snapshot and the first pushed one.
    pub fn connect(&self, channel: Channel<MediaSnapshot>) -> MediaSnapshot {
        let mut inner = self.lock();
        inner.channel = Some(channel);
        inner.latest.clone()
    }

    pub fn latest(&self) -> MediaSnapshot {
        self.lock().latest.clone()
    }

    pub fn publish(&self, snapshot: MediaSnapshot) {
        let mut inner = self.lock();
        if snapshot.revision <= inner.latest.revision && inner.latest.revision != 0 {
            return;
        }
        #[cfg(debug_assertions)]
        crate::log_debug!(
            "media",
            "media snapshot r{} ({:?}, {} sessions, selected {:?})",
            snapshot.revision,
            snapshot.status,
            snapshot.sessions.len(),
            snapshot.selected_session_id.map(|id| id.to_string())
        );
        if let Some(channel) = inner.channel.as_ref() {
            if let Err(e) = channel.send(snapshot.clone()) {
                crate::log_warn!("media", "failed to deliver media snapshot: {e}");
            }
        }
        inner.latest = snapshot;
    }

    pub fn has_artwork(&self, key: &str) -> bool {
        self.lock().artwork.contains(key)
    }

    pub fn insert_artwork(&self, key: String, url: String) {
        self.lock().artwork.insert(key, url);
    }

    pub fn artwork(&self, key: &str) -> Option<Arc<str>> {
        self.lock().artwork.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_out_of_order_snapshots() {
        let hub = MediaHub::default();
        let mut s = MediaSnapshot::initial();
        s.revision = 2;
        hub.publish(s.clone());
        s.revision = 1;
        hub.publish(s);
        assert_eq!(hub.latest().revision, 2);
    }

    #[test]
    fn serves_cached_artwork() {
        let hub = MediaHub::default();
        assert!(!hub.has_artwork("k"));
        hub.insert_artwork("k".into(), "data:image/jpeg;base64,AA==".into());
        assert!(hub.has_artwork("k"));
        assert_eq!(
            hub.artwork("k").as_deref(),
            Some("data:image/jpeg;base64,AA==")
        );
    }
}
