//! The media worker: one dedicated thread that owns the native backend.
//!
//! - WinRT event handlers (any thread) only send a [`NativeEvent`] here.
//! - Events are coalesced: the first one opens a short window; everything that
//!   arrives in it is folded into one re-read per session and part, and at
//!   most one snapshot is published (and only if something changed).
//! - UI requests are executed here too, so all native access is serialized
//!   and never runs on the Tauri main thread.
//! - If Windows media is unavailable, connecting is retried with exponential
//!   backoff, never in a tight loop.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{Receiver, RecvTimeoutError, Sender},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::{
    artwork,
    events::{MediaHub, NativeEvent},
    models::{
        IntegrationStatus, MediaAction, MediaControls, MediaSnapshot, Metadata, PlaybackStatus,
        SessionId, Timeline,
    },
    playback::{self, NativeAction},
    state::MediaState,
    MediaError,
};

/// How long native events are gathered before re-reading.
pub const COALESCE: Duration = Duration::from_millis(60);
/// Re-read delay after a successful command, in case the app sends no event.
const CONFIRM_DELAY: Duration = Duration::from_millis(250);
const RETRY_INITIAL: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(300);

pub struct SessionInfo {
    pub id: SessionId,
    pub app_id: String,
    pub app_name: String,
}

/// Result of re-enumerating native sessions.
pub struct SessionSync {
    pub added: Vec<SessionInfo>,
    pub removed: Vec<SessionId>,
    pub current: Option<SessionId>,
}

pub struct NativeMetadata {
    pub metadata: Metadata,
    /// Raw thumbnail bytes (size-limited by the backend), if any.
    pub thumbnail: Option<Vec<u8>>,
}

pub struct NativePlayback {
    pub status: PlaybackStatus,
    pub rate: f64,
    /// `seek` here means "the app accepts position changes".
    pub controls: MediaControls,
}

/// The platform side. Implementations live on the worker thread only.
pub trait Backend {
    fn sync_sessions(&mut self) -> Result<SessionSync, MediaError>;
    fn read_metadata(&mut self, id: SessionId) -> Result<NativeMetadata, MediaError>;
    fn read_playback(&mut self, id: SessionId) -> Result<NativePlayback, MediaError>;
    fn read_timeline(&mut self, id: SessionId, now_ms: u64)
        -> Result<Option<Timeline>, MediaError>;
    /// Decodes and re-encodes a thumbnail as a bounded JPEG.
    fn encode_artwork(&mut self, source: &[u8]) -> Result<Vec<u8>, MediaError>;
    /// `Ok(false)` when the app declined the request.
    fn execute(&mut self, id: SessionId, action: NativeAction) -> Result<bool, MediaError>;
    fn seek(&mut self, id: SessionId, position_ms: u64) -> Result<bool, MediaError>;
    /// Removes every native event registration.
    fn shutdown(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Select(Option<SessionId>),
    Control(SessionId, MediaAction),
    Seek(SessionId, u64),
}

pub type Reply = Sender<Result<MediaSnapshot, MediaError>>;

pub enum Message {
    Native(NativeEvent),
    Request(Request, Reply),
    Shutdown(Sender<()>),
}

#[derive(Debug, Default, Clone, Copy)]
struct Parts {
    metadata: bool,
    playback: bool,
    timeline: bool,
}

const ALL_PARTS: Parts = Parts {
    metadata: true,
    playback: true,
    timeline: true,
};

#[derive(Debug, Default)]
struct Dirty {
    sessions: bool,
    parts: BTreeMap<SessionId, Parts>,
}

impl Dirty {
    fn is_empty(&self) -> bool {
        !self.sessions && self.parts.is_empty()
    }

    fn mark(&mut self, event: NativeEvent) {
        let mut part = |id, f: fn(&mut Parts)| f(self.parts.entry(id).or_default());
        match event {
            NativeEvent::SessionsChanged | NativeEvent::CurrentSessionChanged => {
                self.sessions = true
            }
            NativeEvent::MediaPropertiesChanged(id) => part(id, |p| p.metadata = true),
            NativeEvent::PlaybackInfoChanged(id) => part(id, |p| p.playback = true),
            NativeEvent::TimelinePropertiesChanged(id) => part(id, |p| p.timeline = true),
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn log_error(context: &str, error: &MediaError) {
    crate::log_warn!("media", "media: {context}: {error}");
}

struct Worker<B, F> {
    hub: Arc<MediaHub>,
    connect: F,
    backend: Option<B>,
    state: MediaState,
    dirty: Dirty,
    flush_at: Option<Instant>,
    retry_at: Option<Instant>,
    retry_delay: Duration,
}

impl<B: Backend, F: FnMut() -> Result<B, MediaError>> Worker<B, F> {
    fn try_connect(&mut self, now: Instant) {
        match (self.connect)() {
            Ok(backend) => {
                self.backend = Some(backend);
                self.retry_at = None;
                self.retry_delay = RETRY_INITIAL;
                self.state.set_status(IntegrationStatus::Ready);
                self.dirty.sessions = true;
                self.flush_at = Some(now);
            }
            Err(MediaError::Unsupported) => {
                self.retry_at = None;
                self.state.set_status(IntegrationStatus::Unsupported);
                self.publish();
            }
            Err(e) => {
                log_error(
                    "could not connect to Windows media sessions (will retry)",
                    &e,
                );
                self.retry_at = Some(now + self.retry_delay);
                self.retry_delay = (self.retry_delay * 2).min(RETRY_MAX);
                self.state.set_status(IntegrationStatus::Unavailable);
                self.publish();
            }
        }
    }

    fn schedule_flush(&mut self, delay: Duration) {
        let at = Instant::now() + delay;
        self.flush_at = Some(self.flush_at.map_or(at, |t| t.min(at)));
    }

    fn publish(&mut self) {
        if let Some(snapshot) = self.state.publish() {
            self.hub.publish(snapshot);
        }
    }

    /// Re-reads everything marked dirty, then publishes if anything changed.
    fn flush(&mut self) {
        self.flush_at = None;
        let dirty = std::mem::take(&mut self.dirty);
        let Some(backend) = self.backend.as_mut() else {
            return;
        };
        let mut resync = false;

        if dirty.sessions {
            match backend.sync_sessions() {
                Ok(sync) => {
                    for id in sync.removed {
                        self.state.remove_session(id);
                    }
                    for info in sync.added {
                        self.state.add_session(info.id, info.app_id, info.app_name);
                        self.dirty.parts.insert(info.id, ALL_PARTS);
                    }
                    self.state.set_system_current(sync.current);
                }
                Err(e) => log_error("listing sessions failed", &e),
            }
        }
        // Newly added sessions are read in this same pass.
        let mut parts = std::mem::take(&mut self.dirty.parts);
        for (id, p) in dirty.parts {
            let entry = parts.entry(id).or_default();
            entry.metadata |= p.metadata;
            entry.playback |= p.playback;
            entry.timeline |= p.timeline;
        }

        for (id, p) in parts {
            if self.state.session(id).is_none() {
                continue;
            }
            let mut gone = |e: MediaError, what: &str| {
                if e == MediaError::SessionNotFound {
                    resync = true;
                } else {
                    log_error(what, &e);
                }
            };
            if p.playback {
                match backend.read_playback(id) {
                    Ok(pb) => self
                        .state
                        .update_playback(id, pb.status, pb.rate, pb.controls),
                    Err(e) => gone(e, "reading playback info failed"),
                }
            }
            if p.timeline {
                match backend.read_timeline(id, now_ms()) {
                    Ok(t) => self.state.update_timeline(id, t),
                    Err(e) => gone(e, "reading timeline failed"),
                }
            }
            if p.metadata {
                match backend.read_metadata(id) {
                    Ok(native) => {
                        let key = native
                            .thumbnail
                            .and_then(|bytes| prepare_artwork(backend, &self.hub, &bytes));
                        self.state.update_metadata(id, native.metadata, key);
                    }
                    Err(e) => gone(e, "reading media properties failed"),
                }
            }
        }

        if resync {
            self.dirty.sessions = true;
            self.schedule_flush(COALESCE);
        }
        self.publish();
    }

    fn handle(&mut self, request: Request) -> Result<MediaSnapshot, MediaError> {
        match self.state.published().status {
            IntegrationStatus::Unsupported => return Err(MediaError::Unsupported),
            IntegrationStatus::Unavailable | IntegrationStatus::Starting => {
                return Err(MediaError::Unavailable)
            }
            IntegrationStatus::Ready => {}
        }
        if !self.dirty.is_empty() {
            self.flush();
        }
        let backend = self.backend.as_mut().ok_or(MediaError::Unavailable)?;
        match request {
            Request::Select(id) => {
                self.state
                    .select(id)
                    .map_err(|_| MediaError::SessionNotFound)?;
                self.publish();
            }
            Request::Control(id, action) => {
                let session = self.state.session(id).ok_or(MediaError::SessionNotFound)?;
                let native =
                    playback::resolve_action(action, session.status, &session.native_controls)
                        .ok_or(MediaError::ControlUnsupported)?;
                if !backend.execute(id, native)? {
                    return Err(MediaError::Rejected);
                }
                self.confirm(id);
            }
            Request::Seek(id, position_ms) => {
                let published = self.state.published();
                let can_seek = published
                    .sessions
                    .iter()
                    .find(|s| s.id == id)
                    .ok_or(MediaError::SessionNotFound)?
                    .controls
                    .seek;
                if !can_seek {
                    return Err(MediaError::ControlUnsupported);
                }
                if !backend.seek(id, position_ms)? {
                    return Err(MediaError::Rejected);
                }
                self.confirm(id);
            }
        }
        Ok(self.state.published().clone())
    }

    /// The UI shows a command's effect only once Windows reports it.
    fn confirm(&mut self, id: SessionId) {
        let p = self.dirty.parts.entry(id).or_default();
        p.playback = true;
        p.timeline = true;
        self.schedule_flush(CONFIRM_DELAY);
    }
}

/// Reads the artwork key for thumbnail bytes, transcoding only unseen images.
fn prepare_artwork<B: Backend>(backend: &mut B, hub: &MediaHub, source: &[u8]) -> Option<String> {
    if source.is_empty() {
        return None;
    }
    let key = artwork::artwork_key(source);
    if hub.has_artwork(&key) {
        return Some(key);
    }
    match backend.encode_artwork(source) {
        Ok(jpeg) => match artwork::data_url("image/jpeg", &jpeg) {
            Some(url) => {
                hub.insert_artwork(key.clone(), url);
                Some(key)
            }
            None => {
                log_error(
                    "artwork",
                    &MediaError::Platform("encoded image is empty or too large".into()),
                );
                None
            }
        },
        Err(e) => {
            log_error("artwork could not be decoded", &e);
            None
        }
    }
}

/// Runs the worker until `Shutdown` (or until every sender is gone).
pub fn run<B, F>(rx: Receiver<Message>, hub: Arc<MediaHub>, connect: F)
where
    B: Backend,
    F: FnMut() -> Result<B, MediaError>,
{
    let mut worker = Worker {
        hub,
        connect,
        backend: None,
        state: MediaState::new(),
        dirty: Dirty::default(),
        flush_at: None,
        retry_at: Some(Instant::now()),
        retry_delay: RETRY_INITIAL,
    };

    loop {
        let now = Instant::now();
        if worker.backend.is_none() && worker.retry_at.is_some_and(|t| t <= now) {
            worker.try_connect(now);
        }
        if worker.flush_at.is_some_and(|t| t <= now) {
            worker.flush();
            continue;
        }
        let deadline = match (worker.flush_at, worker.retry_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let message = match deadline {
            Some(at) => rx.recv_timeout(at.saturating_duration_since(now)),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match message {
            Ok(Message::Native(event)) => {
                if worker.backend.is_some() {
                    worker.dirty.mark(event);
                    worker.schedule_flush(COALESCE);
                }
            }
            Ok(Message::Request(request, reply)) => {
                let result = worker.handle(request);
                if let Err(e) = &result {
                    #[cfg(debug_assertions)]
                    crate::log_warn!("media", "media request {request:?} failed: {e}");
                    #[cfg(not(debug_assertions))]
                    let _ = e;
                }
                let _ = reply.send(result);
            }
            Ok(Message::Shutdown(ack)) => {
                if let Some(backend) = worker.backend.as_mut() {
                    backend.shutdown();
                }
                let _ = ack.send(());
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if let Some(backend) = worker.backend.as_mut() {
                    backend.shutdown();
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{mpsc, Mutex};

    use super::*;

    #[derive(Default)]
    struct Fake {
        sessions: Vec<(SessionId, &'static str)>,
        current: Option<SessionId>,
        /// `None` = paused.
        status: Option<PlaybackStatus>,
        controls: MediaControls,
        accept: bool,
        thumbnail: Option<Vec<u8>>,
        reads: usize,
        encodes: usize,
        executed: Vec<NativeAction>,
        seeks: Vec<u64>,
        known: Vec<SessionId>,
        shut_down: bool,
    }

    struct FakeBackend(Arc<Mutex<Fake>>);

    impl Backend for FakeBackend {
        fn sync_sessions(&mut self) -> Result<SessionSync, MediaError> {
            let mut f = self.0.lock().unwrap();
            let ids: Vec<SessionId> = f.sessions.iter().map(|s| s.0).collect();
            let removed = f
                .known
                .iter()
                .copied()
                .filter(|id| !ids.contains(id))
                .collect();
            let added = f
                .sessions
                .iter()
                .filter(|(id, _)| !f.known.contains(id))
                .map(|(id, app)| SessionInfo {
                    id: *id,
                    app_id: (*app).into(),
                    app_name: (*app).into(),
                })
                .collect();
            f.known = ids;
            Ok(SessionSync {
                added,
                removed,
                current: f.current,
            })
        }
        fn read_metadata(&mut self, id: SessionId) -> Result<NativeMetadata, MediaError> {
            let mut f = self.0.lock().unwrap();
            if !f.sessions.iter().any(|s| s.0 == id) {
                return Err(MediaError::SessionNotFound);
            }
            f.reads += 1;
            Ok(NativeMetadata {
                metadata: Metadata {
                    title: Some(format!("Track {}", f.reads)),
                    ..Metadata::default()
                },
                thumbnail: f.thumbnail.clone(),
            })
        }
        fn read_playback(&mut self, _: SessionId) -> Result<NativePlayback, MediaError> {
            let f = self.0.lock().unwrap();
            Ok(NativePlayback {
                status: f.status.unwrap_or(PlaybackStatus::Paused),
                rate: 1.0,
                controls: f.controls,
            })
        }
        fn read_timeline(
            &mut self,
            _: SessionId,
            now: u64,
        ) -> Result<Option<Timeline>, MediaError> {
            Ok(Some(Timeline {
                position_ms: 0,
                duration_ms: Some(60_000),
                updated_at_ms: now,
            }))
        }
        fn encode_artwork(&mut self, source: &[u8]) -> Result<Vec<u8>, MediaError> {
            self.0.lock().unwrap().encodes += 1;
            if source == b"bad" {
                return Err(MediaError::Platform("undecodable".into()));
            }
            Ok(vec![0xff, 0xd8, 0xff])
        }
        fn execute(&mut self, _: SessionId, action: NativeAction) -> Result<bool, MediaError> {
            let mut f = self.0.lock().unwrap();
            f.executed.push(action);
            Ok(f.accept)
        }
        fn seek(&mut self, _: SessionId, position_ms: u64) -> Result<bool, MediaError> {
            let mut f = self.0.lock().unwrap();
            f.seeks.push(position_ms);
            Ok(f.accept)
        }
        fn shutdown(&mut self) {
            self.0.lock().unwrap().shut_down = true;
        }
    }

    struct Harness {
        tx: Sender<Message>,
        hub: Arc<MediaHub>,
        fake: Arc<Mutex<Fake>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Harness {
        fn start(fake: Fake) -> Self {
            let fake = Arc::new(Mutex::new(fake));
            let hub = Arc::new(MediaHub::default());
            let (tx, rx) = mpsc::channel();
            let (h, f) = (hub.clone(), fake.clone());
            let thread = std::thread::spawn(move || run(rx, h, move || Ok(FakeBackend(f.clone()))));
            Self {
                tx,
                hub,
                fake,
                thread: Some(thread),
            }
        }

        fn request(&self, request: Request) -> Result<MediaSnapshot, MediaError> {
            let (reply, rx) = mpsc::channel();
            self.tx.send(Message::Request(request, reply)).unwrap();
            rx.recv_timeout(Duration::from_secs(5)).unwrap()
        }

        fn event(&self, event: NativeEvent) {
            self.tx.send(Message::Native(event)).unwrap();
        }

        fn wait_for(&self, what: &str, f: impl Fn(&MediaSnapshot) -> bool) -> MediaSnapshot {
            let start = Instant::now();
            loop {
                let s = self.hub.latest();
                if f(&s) {
                    return s;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "timed out waiting for {what}: {s:?}"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn stop(mut self) -> Arc<Mutex<Fake>> {
            let (ack, rx) = mpsc::channel();
            self.tx.send(Message::Shutdown(ack)).unwrap();
            rx.recv_timeout(Duration::from_secs(5)).unwrap();
            self.thread.take().unwrap().join().unwrap();
            self.fake.clone()
        }
    }

    const S1: SessionId = SessionId(1);
    const S2: SessionId = SessionId(2);
    const ALL: MediaControls = MediaControls {
        play: true,
        pause: true,
        toggle_play_pause: true,
        next: true,
        previous: true,
        seek: true,
    };

    #[test]
    fn discovers_sessions_and_reads_them_once() {
        let h = Harness::start(Fake {
            sessions: vec![(S1, "A.exe")],
            current: Some(S1),
            ..Fake::default()
        });
        let s = h.wait_for("first session", |s| {
            s.sessions.len() == 1 && s.sessions[0].metadata_loaded
        });
        assert_eq!(s.status, IntegrationStatus::Ready);
        assert_eq!(s.selected_session_id, Some(S1));
        assert_eq!(s.sessions[0].metadata.title.as_deref(), Some("Track 1"));
        let fake = h.stop();
        assert!(fake.lock().unwrap().shut_down);
    }

    #[test]
    fn coalesces_event_bursts_into_one_read() {
        let h = Harness::start(Fake {
            sessions: vec![(S1, "A.exe")],
            ..Fake::default()
        });
        h.wait_for("loaded", |s| {
            s.sessions.first().is_some_and(|x| x.metadata_loaded)
        });
        let before = h.hub.latest().revision;
        for _ in 0..50 {
            h.event(NativeEvent::MediaPropertiesChanged(S1));
        }
        let s = h.wait_for("track 2", |s| s.revision > before);
        assert_eq!(s.revision, before + 1);
        assert_eq!(s.sessions[0].metadata.title.as_deref(), Some("Track 2"));
        std::thread::sleep(COALESCE * 3);
        assert_eq!(
            h.fake.lock().unwrap().reads,
            2,
            "one read for the whole burst"
        );
        h.stop();
    }

    #[test]
    fn handles_removed_sessions_and_session_switching() {
        let h = Harness::start(Fake {
            sessions: vec![(S1, "A.exe"), (S2, "B.exe")],
            current: Some(S1),
            ..Fake::default()
        });
        h.wait_for("two sessions", |s| s.sessions.len() == 2);
        let s = h.request(Request::Select(Some(S2))).unwrap();
        assert_eq!(s.selected_session_id, Some(S2));
        assert_eq!(
            h.request(Request::Select(Some(SessionId(9)))),
            Err(MediaError::SessionNotFound)
        );
        {
            let mut f = h.fake.lock().unwrap();
            f.sessions.retain(|s| s.0 != S2);
        }
        h.event(NativeEvent::SessionsChanged);
        let s = h.wait_for("removal", |s| s.sessions.len() == 1);
        assert_eq!(s.selected_session_id, Some(S1));
        assert_eq!(
            h.request(Request::Control(S2, MediaAction::Play)),
            Err(MediaError::SessionNotFound)
        );
        h.stop();
    }

    #[test]
    fn commands_check_capabilities_and_native_results() {
        let h = Harness::start(Fake {
            sessions: vec![(S1, "A.exe")],
            ..Fake::default()
        });
        h.wait_for("loaded", |s| s.sessions.len() == 1);
        assert_eq!(
            h.request(Request::Control(S1, MediaAction::Next)),
            Err(MediaError::ControlUnsupported)
        );
        assert_eq!(
            h.request(Request::Seek(S1, 1_000)),
            Err(MediaError::ControlUnsupported)
        );
        {
            let mut f = h.fake.lock().unwrap();
            f.controls = ALL;
        }
        h.event(NativeEvent::PlaybackInfoChanged(S1));
        h.wait_for("controls", |s| s.sessions[0].controls.next);
        assert_eq!(
            h.request(Request::Control(S1, MediaAction::Next)),
            Err(MediaError::Rejected)
        );
        h.fake.lock().unwrap().accept = true;
        assert!(h
            .request(Request::Control(S1, MediaAction::TogglePlayPause))
            .is_ok());
        assert!(h.request(Request::Seek(S1, 5_000)).is_ok());
        let fake = h.stop();
        let f = fake.lock().unwrap();
        assert_eq!(f.executed, [NativeAction::Next, NativeAction::Play]);
        assert_eq!(f.seeks, [5_000]);
    }

    #[test]
    fn artwork_is_decoded_once_per_image_and_failures_are_tolerated() {
        let h = Harness::start(Fake {
            sessions: vec![(S1, "A.exe")],
            thumbnail: Some(b"img".to_vec()),
            ..Fake::default()
        });
        let s = h.wait_for("artwork", |s| {
            s.sessions.first().is_some_and(|x| x.artwork_key.is_some())
        });
        let key = s.sessions[0].artwork_key.clone().unwrap();
        assert!(h
            .hub
            .artwork(&key)
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        h.event(NativeEvent::MediaPropertiesChanged(S1));
        h.wait_for("track 2", |s| {
            s.sessions[0].metadata.title.as_deref() == Some("Track 2")
        });
        assert_eq!(h.fake.lock().unwrap().encodes, 1);
        h.fake.lock().unwrap().thumbnail = Some(b"bad".to_vec());
        h.event(NativeEvent::MediaPropertiesChanged(S1));
        let s = h.wait_for("no artwork", |s| s.sessions[0].artwork_key.is_none());
        assert_eq!(s.sessions[0].metadata.title.as_deref(), Some("Track 3"));
        h.stop();
    }

    #[test]
    fn unsupported_platforms_report_once_and_reject_requests() {
        let hub = Arc::new(MediaHub::default());
        let (tx, rx) = mpsc::channel();
        let h = hub.clone();
        let thread = std::thread::spawn(move || {
            run::<FakeBackend, _>(rx, h, || Err(MediaError::Unsupported))
        });
        let (reply, reply_rx) = mpsc::channel();
        tx.send(Message::Request(Request::Select(None), reply))
            .unwrap();
        assert_eq!(reply_rx.recv().unwrap(), Err(MediaError::Unsupported));
        assert_eq!(hub.latest().status, IntegrationStatus::Unsupported);
        let (ack, ack_rx) = mpsc::channel();
        tx.send(Message::Shutdown(ack)).unwrap();
        ack_rx.recv().unwrap();
        thread.join().unwrap();
    }
}
