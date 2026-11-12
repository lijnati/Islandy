//! The media state engine (pure): tracked sessions, the selection policy,
//! and revisioned, de-duplicated snapshots.

use super::{
    models::{
        IntegrationStatus, MediaControls, MediaSession, MediaSnapshot, Metadata, PlaybackStatus,
        SelectionMode, SessionId, Timeline,
    },
    playback,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SessionEntry {
    pub id: SessionId,
    pub source_app_id: String,
    pub source_app_name: String,
    pub metadata: Metadata,
    pub artwork_key: Option<String>,
    pub metadata_loaded: bool,
    pub status: PlaybackStatus,
    pub rate: f64,
    /// As reported by the app; `seek` = "position changes enabled".
    pub native_controls: MediaControls,
    pub timeline: Option<Timeline>,
    /// Activity sequence number of the last switch to `Playing` (0 = never).
    pub last_played: u64,
}

/// Inputs of the selection policy for one session.
#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    pub id: SessionId,
    pub playing: bool,
    pub last_played: u64,
}

/// The selection policy:
/// 1. the user's choice, while that session exists;
/// 2. the session Windows designates as current;
/// 3. the most recently started playing session;
/// 4. the previous automatic choice, while it exists (stability);
/// 5. the most recently active session; 6. the first one.
pub fn choose_session(
    candidates: &[Candidate],
    user: Option<SessionId>,
    system: Option<SessionId>,
    previous: Option<SessionId>,
) -> Option<SessionId> {
    let exists = |id: Option<SessionId>| id.filter(|id| candidates.iter().any(|c| c.id == *id));
    exists(user)
        .or_else(|| exists(system))
        .or_else(|| {
            candidates
                .iter()
                .filter(|c| c.playing)
                .max_by_key(|c| c.last_played)
                .map(|c| c.id)
        })
        .or_else(|| exists(previous))
        .or_else(|| {
            candidates
                .iter()
                .filter(|c| c.last_played > 0)
                .max_by_key(|c| c.last_played)
                .map(|c| c.id)
        })
        .or_else(|| candidates.first().map(|c| c.id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotFound;

#[derive(Debug)]
pub struct MediaState {
    status: IntegrationStatus,
    sessions: Vec<SessionEntry>,
    system_current: Option<SessionId>,
    user_selected: Option<SessionId>,
    auto_selected: Option<SessionId>,
    activity_seq: u64,
    published: MediaSnapshot,
}

impl Default for MediaState {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaState {
    pub fn new() -> Self {
        Self {
            status: IntegrationStatus::Starting,
            sessions: Vec::new(),
            system_current: None,
            user_selected: None,
            auto_selected: None,
            activity_seq: 0,
            published: MediaSnapshot::initial(),
        }
    }

    pub fn set_status(&mut self, status: IntegrationStatus) {
        self.status = status;
        if status != IntegrationStatus::Ready {
            self.sessions.clear();
            self.system_current = None;
            self.user_selected = None;
            self.auto_selected = None;
        }
    }

    pub fn session(&self, id: SessionId) -> Option<&SessionEntry> {
        self.sessions.iter().find(|s| s.id == id)
    }

    fn session_mut(&mut self, id: SessionId) -> Option<&mut SessionEntry> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn add_session(&mut self, id: SessionId, source_app_id: String, source_app_name: String) {
        if self.session(id).is_some() {
            return;
        }
        self.sessions.push(SessionEntry {
            id,
            source_app_id,
            source_app_name,
            metadata: Metadata::default(),
            artwork_key: None,
            metadata_loaded: false,
            status: PlaybackStatus::Unknown,
            rate: 1.0,
            native_controls: MediaControls::default(),
            timeline: None,
            last_played: 0,
        });
    }

    pub fn remove_session(&mut self, id: SessionId) {
        self.sessions.retain(|s| s.id != id);
        for slot in [
            &mut self.user_selected,
            &mut self.auto_selected,
            &mut self.system_current,
        ] {
            if *slot == Some(id) {
                *slot = None;
            }
        }
    }

    pub fn set_system_current(&mut self, id: Option<SessionId>) {
        self.system_current = id;
    }

    pub fn update_metadata(
        &mut self,
        id: SessionId,
        metadata: Metadata,
        artwork_key: Option<String>,
    ) {
        if let Some(s) = self.session_mut(id) {
            s.metadata = metadata;
            s.artwork_key = artwork_key;
            s.metadata_loaded = true;
        }
    }

    pub fn update_playback(
        &mut self,
        id: SessionId,
        status: PlaybackStatus,
        rate: f64,
        controls: MediaControls,
    ) {
        let next_seq = self.activity_seq + 1;
        let Some(s) = self.session_mut(id) else {
            return;
        };
        let started = status == PlaybackStatus::Playing && s.status != PlaybackStatus::Playing;
        s.status = status;
        s.rate = rate;
        s.native_controls = controls;
        if started {
            s.last_played = next_seq;
            self.activity_seq = next_seq;
        }
    }

    pub fn update_timeline(&mut self, id: SessionId, timeline: Option<Timeline>) {
        if let Some(s) = self.session_mut(id) {
            s.timeline = timeline;
        }
    }

    /// `None` returns to automatic selection.
    pub fn select(&mut self, id: Option<SessionId>) -> Result<(), NotFound> {
        if let Some(id) = id {
            if self.session(id).is_none() {
                return Err(NotFound);
            }
        }
        self.user_selected = id;
        Ok(())
    }

    fn selected(&self) -> Option<SessionId> {
        let candidates: Vec<Candidate> = self
            .sessions
            .iter()
            .map(|s| Candidate {
                id: s.id,
                playing: s.status == PlaybackStatus::Playing,
                last_played: s.last_played,
            })
            .collect();
        choose_session(
            &candidates,
            self.user_selected,
            self.system_current,
            self.auto_selected,
        )
    }

    fn build(&self, revision: u64) -> MediaSnapshot {
        let sessions = self
            .sessions
            .iter()
            .map(|s| MediaSession {
                id: s.id,
                source_app_id: s.source_app_id.clone(),
                source_app_name: s.source_app_name.clone(),
                metadata: s.metadata.clone(),
                artwork_key: s.artwork_key.clone(),
                metadata_loaded: s.metadata_loaded,
                playback_status: s.status,
                playback_rate: s.rate,
                controls: MediaControls {
                    seek: playback::can_seek(s.native_controls.seek, s.timeline.as_ref()),
                    ..s.native_controls
                },
                timeline: s.timeline,
                is_system_current: self.system_current == Some(s.id),
            })
            .collect();
        MediaSnapshot {
            revision,
            status: self.status,
            sessions,
            selected_session_id: self.selected(),
            selection_mode: if self.user_selected.is_some() {
                SelectionMode::User
            } else {
                SelectionMode::Auto
            },
        }
    }

    /// The last published snapshot.
    pub fn published(&self) -> &MediaSnapshot {
        &self.published
    }

    /// Builds a snapshot and returns it if it differs meaningfully from the
    /// last published one (bumping the revision); otherwise `None`.
    pub fn publish(&mut self) -> Option<MediaSnapshot> {
        let selected = self.selected();
        if self.user_selected.is_none() {
            self.auto_selected = selected;
        }
        let candidate = self.build(self.published.revision + 1);
        if equivalent(&self.published, &candidate) {
            return None;
        }
        self.published = candidate.clone();
        Some(candidate)
    }
}

/// Equal apart from the revision and timeline updates that only confirm
/// what the UI already interpolates.
pub fn equivalent(a: &MediaSnapshot, b: &MediaSnapshot) -> bool {
    a.status == b.status
        && a.selected_session_id == b.selected_session_id
        && a.selection_mode == b.selection_mode
        && a.sessions.len() == b.sessions.len()
        && a.sessions.iter().zip(&b.sessions).all(|(x, y)| {
            let playing = x.playback_status == PlaybackStatus::Playing;
            MediaSession {
                timeline: None,
                ..x.clone()
            } == MediaSession {
                timeline: None,
                ..y.clone()
            } && !playback::timeline_changed(
                x.timeline.as_ref(),
                y.timeline.as_ref(),
                playing,
                x.playback_rate,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: u64, playing: bool, last_played: u64) -> Candidate {
        Candidate {
            id: SessionId(id),
            playing,
            last_played,
        }
    }
    const S1: SessionId = SessionId(1);
    const S2: SessionId = SessionId(2);
    const S3: SessionId = SessionId(3);

    #[test]
    fn selection_policy_order() {
        let cands = [c(1, false, 1), c(2, true, 3), c(3, true, 2)];
        assert_eq!(
            choose_session(&cands, Some(S1), Some(S3), None),
            Some(S1),
            "user first"
        );
        assert_eq!(
            choose_session(&cands, None, Some(S3), None),
            Some(S3),
            "then Windows"
        );
        assert_eq!(
            choose_session(&cands, None, None, Some(S1)),
            Some(S2),
            "then latest playing"
        );
        let paused = [c(1, false, 1), c(2, false, 3), c(3, false, 0)];
        assert_eq!(
            choose_session(&paused, None, None, Some(S1)),
            Some(S1),
            "then previous"
        );
        assert_eq!(
            choose_session(&paused, None, None, None),
            Some(S2),
            "then most recent"
        );
        let fresh = [c(3, false, 0), c(1, false, 0)];
        assert_eq!(
            choose_session(&fresh, None, None, None),
            Some(S3),
            "then first"
        );
        assert_eq!(choose_session(&[], Some(S1), Some(S2), Some(S3)), None);
    }

    #[test]
    fn missing_user_or_system_choices_are_skipped() {
        let cands = [c(2, false, 0)];
        assert_eq!(choose_session(&cands, Some(S1), Some(S3), None), Some(S2));
    }

    fn state_with(ids: &[u64]) -> MediaState {
        let mut s = MediaState::new();
        s.set_status(IntegrationStatus::Ready);
        for id in ids {
            s.add_session(SessionId(*id), format!("App{id}.exe"), format!("App{id}"));
        }
        s
    }

    #[test]
    fn user_selection_persists_until_the_session_disappears() {
        let mut s = state_with(&[1, 2]);
        s.set_system_current(Some(S1));
        assert_eq!(s.publish().unwrap().selected_session_id, Some(S1));
        s.select(Some(S2)).unwrap();
        s.set_system_current(Some(S1));
        let snap = s.publish().unwrap();
        assert_eq!(snap.selected_session_id, Some(S2));
        assert_eq!(snap.selection_mode, SelectionMode::User);
        s.remove_session(S2);
        let snap = s.publish().unwrap();
        assert_eq!(snap.selected_session_id, Some(S1));
        assert_eq!(snap.selection_mode, SelectionMode::Auto);
        assert_eq!(s.select(Some(SessionId(9))), Err(NotFound));
    }

    #[test]
    fn auto_selection_stays_put_on_unrelated_metadata_changes() {
        let mut s = state_with(&[1, 2]);
        s.update_playback(S2, PlaybackStatus::Playing, 1.0, MediaControls::default());
        assert_eq!(s.publish().unwrap().selected_session_id, Some(S2));
        s.update_playback(S2, PlaybackStatus::Paused, 1.0, MediaControls::default());
        s.update_metadata(
            S1,
            Metadata {
                title: Some("x".into()),
                ..Metadata::default()
            },
            None,
        );
        assert_eq!(s.publish().unwrap().selected_session_id, Some(S2));
        // A newly playing session takes over.
        s.update_playback(S1, PlaybackStatus::Playing, 1.0, MediaControls::default());
        assert_eq!(s.publish().unwrap().selected_session_id, Some(S1));
    }

    #[test]
    fn publishing_dedupes_and_increments_revisions() {
        let mut s = state_with(&[1]);
        let first = s.publish().unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(s.publish(), None);
        s.update_metadata(
            S1,
            Metadata {
                title: Some("Song".into()),
                ..Metadata::default()
            },
            Some("k".into()),
        );
        let second = s.publish().unwrap();
        assert_eq!(second.revision, 2);
        assert_eq!(second.sessions[0].metadata.title.as_deref(), Some("Song"));
        assert!(second.sessions[0].metadata_loaded);
        // Identical metadata again: nothing to publish.
        s.update_metadata(
            S1,
            Metadata {
                title: Some("Song".into()),
                ..Metadata::default()
            },
            Some("k".into()),
        );
        assert_eq!(s.publish(), None);
        assert_eq!(s.published().revision, 2);
    }

    #[test]
    fn timeline_ticks_matching_interpolation_are_not_published() {
        let mut s = state_with(&[1]);
        let controls = MediaControls {
            seek: true,
            ..MediaControls::default()
        };
        s.update_playback(S1, PlaybackStatus::Playing, 1.0, controls);
        s.update_timeline(
            S1,
            Some(Timeline {
                position_ms: 0,
                duration_ms: Some(100_000),
                updated_at_ms: 1_000,
            }),
        );
        let snap = s.publish().unwrap();
        assert!(snap.sessions[0].controls.seek);
        s.update_timeline(
            S1,
            Some(Timeline {
                position_ms: 1_000,
                duration_ms: Some(100_000),
                updated_at_ms: 2_000,
            }),
        );
        assert_eq!(s.publish(), None);
        s.update_timeline(
            S1,
            Some(Timeline {
                position_ms: 50_000,
                duration_ms: Some(100_000),
                updated_at_ms: 3_000,
            }),
        );
        assert_eq!(
            s.publish().unwrap().sessions[0]
                .timeline
                .unwrap()
                .position_ms,
            50_000
        );
    }

    #[test]
    fn seek_is_only_offered_with_a_known_duration() {
        let mut s = state_with(&[1]);
        let controls = MediaControls {
            seek: true,
            ..MediaControls::default()
        };
        s.update_playback(S1, PlaybackStatus::Playing, 1.0, controls);
        s.update_timeline(
            S1,
            Some(Timeline {
                position_ms: 5,
                duration_ms: None,
                updated_at_ms: 1,
            }),
        );
        assert!(!s.publish().unwrap().sessions[0].controls.seek);
    }

    #[test]
    fn leaving_ready_clears_sessions() {
        let mut s = state_with(&[1, 2]);
        s.select(Some(S2)).unwrap();
        s.set_status(IntegrationStatus::Unavailable);
        let snap = s.publish().unwrap();
        assert!(snap.sessions.is_empty());
        assert_eq!(snap.selected_session_id, None);
        assert_eq!(snap.status, IntegrationStatus::Unavailable);
    }
}
