//! Native media integration (Windows GSMTC).
//!
//! ```text
//! WinRT event handlers ──NativeEvent──► worker thread (MTA) ──► MediaState ──► MediaHub ──Channel──► UI
//!   Tauri commands ──Request──────────►   owns the Backend      (selection,      (latest snapshot,
//!                  ◄──Result<MediaSnapshot>                      revisions)       artwork cache)
//! ```
//!
//! - [`models`]    IPC types (mirrored in `src/services/types.ts`)
//! - [`metadata`]  text cleanup, app names
//! - [`playback`]  status/controls/timeline normalization
//! - [`state`]     tracked sessions, selection policy, revisioned snapshots
//! - [`identity`]  native object → stable session id mapping
//! - [`artwork`]   content keys, size limits, data URLs, LRU cache
//! - [`events`]    native event kinds and the UI-facing hub
//! - [`worker`]    the worker loop and the `Backend` trait
//! - `windows`     the GSMTC backend (Windows only)

pub mod artwork;
pub mod events;
pub mod identity;
pub mod metadata;
pub mod models;
pub mod playback;
pub mod state;
#[cfg(windows)]
mod windows;
pub mod worker;

use std::{
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use tauri::ipc::Channel;

use self::{
    events::MediaHub,
    models::MediaSnapshot,
    worker::{Message, Request},
};

/// How long a command waits for the worker (native calls time out sooner).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
/// How long quitting waits for event handlers to be unregistered.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);

/// Media failures. Messages are technical (for logs); the UI maps `kind` to
/// its own neutral wording.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaError {
    #[error("Windows media sessions are unavailable")]
    Unavailable,
    #[error("media integration is not supported on this platform")]
    Unsupported,
    #[error("the media session no longer exists")]
    SessionNotFound,
    #[error("the media app does not support this control")]
    ControlUnsupported,
    #[error("the media app declined the request")]
    Rejected,
    #[error("the media app did not respond in time")]
    Timeout,
    #[error("media integration failed: {0}")]
    Platform(String),
}

impl MediaError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Unsupported | Self::ControlUnsupported => "unsupported",
            Self::SessionNotFound => "notFound",
            Self::Rejected => "rejected",
            Self::Timeout => "timeout",
            Self::Platform(_) => "platform",
        }
    }
}

/// Handle to the media worker, managed as Tauri state.
pub struct MediaService {
    tx: mpsc::Sender<Message>,
    hub: Arc<MediaHub>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

#[cfg(not(windows))]
enum NoBackend {}

#[cfg(not(windows))]
impl worker::Backend for NoBackend {
    fn sync_sessions(&mut self) -> Result<worker::SessionSync, MediaError> {
        match *self {}
    }
    fn read_metadata(
        &mut self,
        _: models::SessionId,
    ) -> Result<worker::NativeMetadata, MediaError> {
        match *self {}
    }
    fn read_playback(
        &mut self,
        _: models::SessionId,
    ) -> Result<worker::NativePlayback, MediaError> {
        match *self {}
    }
    fn read_timeline(
        &mut self,
        _: models::SessionId,
        _: u64,
    ) -> Result<Option<models::Timeline>, MediaError> {
        match *self {}
    }
    fn encode_artwork(&mut self, _: &[u8]) -> Result<Vec<u8>, MediaError> {
        match *self {}
    }
    fn execute(
        &mut self,
        _: models::SessionId,
        _: playback::NativeAction,
    ) -> Result<bool, MediaError> {
        match *self {}
    }
    fn seek(&mut self, _: models::SessionId, _: u64) -> Result<bool, MediaError> {
        match *self {}
    }
    fn shutdown(&mut self) {
        match *self {}
    }
}

impl MediaService {
    /// Starts the worker thread. Never blocks the caller.
    pub fn start() -> std::io::Result<Self> {
        let hub = Arc::new(MediaHub::default());
        let (tx, rx) = mpsc::channel();
        let worker_hub = hub.clone();
        #[cfg(windows)]
        let events = tx.clone();
        let thread = std::thread::Builder::new()
            .name("islandy-media".into())
            .spawn(move || {
                #[cfg(windows)]
                windows::with_mta(|| {
                    worker::run(rx, worker_hub, move || {
                        windows::WindowsBackend::connect(events.clone())
                    })
                });
                #[cfg(not(windows))]
                worker::run::<NoBackend, _>(rx, worker_hub, || Err(MediaError::Unsupported));
            })?;
        Ok(Self {
            tx,
            hub,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub fn connect(&self, channel: Channel<MediaSnapshot>) -> MediaSnapshot {
        self.hub.connect(channel)
    }

    pub fn snapshot(&self) -> MediaSnapshot {
        self.hub.latest()
    }

    pub fn artwork(&self, key: &str) -> Option<String> {
        self.hub.artwork(key).map(|url| url.to_string())
    }

    /// Runs a request on the worker without blocking an async runtime thread.
    pub async fn request(&self, request: Request) -> Result<MediaSnapshot, MediaError> {
        let (reply, rx) = mpsc::channel();
        self.tx
            .send(Message::Request(request, reply))
            .map_err(|_| MediaError::Unavailable)?;
        match tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(REQUEST_TIMEOUT)).await {
            Ok(Ok(result)) => result,
            Ok(Err(mpsc::RecvTimeoutError::Timeout)) => Err(MediaError::Timeout),
            Ok(Err(mpsc::RecvTimeoutError::Disconnected)) | Err(_) => Err(MediaError::Unavailable),
        }
    }

    /// Unregisters native event handlers and stops the worker (bounded wait).
    pub fn shutdown(&self) {
        let (ack, rx) = mpsc::channel();
        if self.tx.send(Message::Shutdown(ack)).is_ok() && rx.recv_timeout(SHUTDOWN_TIMEOUT).is_ok()
        {
            if let Some(thread) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kinds_match_the_frontend() {
        assert_eq!(MediaError::SessionNotFound.kind(), "notFound");
        assert_eq!(MediaError::ControlUnsupported.kind(), "unsupported");
        assert_eq!(MediaError::Rejected.kind(), "rejected");
        assert_eq!(MediaError::Timeout.kind(), "timeout");
        assert_eq!(MediaError::Platform("x".into()).kind(), "platform");
    }

    #[cfg(not(windows))]
    #[test]
    fn service_reports_unsupported_off_windows_and_shuts_down() {
        let service = MediaService::start().unwrap();
        let rt = tauri::async_runtime::block_on(service.request(Request::Select(None)));
        assert_eq!(rt, Err(MediaError::Unsupported));
        assert_eq!(
            service.snapshot().status,
            models::IntegrationStatus::Unsupported
        );
        service.shutdown();
        assert!(service.thread.lock().unwrap().is_none());
    }
}
