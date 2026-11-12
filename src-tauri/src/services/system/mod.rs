//! Native system integration: audio, power, network, radios, metrics,
//! display brightness and Windows Settings shortcuts.
//!
//! ```text
//!  Core Audio callbacks ──AudioEvent──► audio worker (MTA) ───┐
//!  WinRT power/network/radio events ──► monitor worker (MTA) ─┼─► SystemHub ──Channel<SystemUpdate>──► UI
//!  (no events; polled while visible) ──► display worker (MTA) ─┘   (latest state per domain,
//!  Tauri commands ──Request──► the owning worker ──► Result<()>      revisions, alerts)
//! ```
//!
//! - [`models`]   IPC types (mirrored in `src/services/types.ts`)
//! - [`events`]   the hub: per-domain updates and alerts
//! - [`audio`]    volume scale, output-change alerts, audio worker
//! - [`power`]    `SYSTEM_POWER_STATUS` normalization, battery alerts
//! - [`network`]  connectivity mapping, debounced network alerts
//! - [`radios`]   Wi-Fi/Bluetooth radio state and toggle eligibility
//! - [`metrics`]  CPU/RAM calculations
//! - [`monitor`]  power + network + radios + metrics worker
//! - [`display`]  brightness math, display worker
//! - [`settings`] the `ms-settings:` allowlist
//! - `windows`    the native backends (Windows only)
//!
//! Each worker owns its native objects on its own thread: callbacks only post
//! messages, commands never block the UI thread, and a slow DDC/CI monitor
//! can't delay a volume change.

pub mod audio;
pub mod display;
pub mod events;
pub mod metrics;
pub mod models;
pub mod monitor;
pub mod network;
pub mod power;
pub mod radios;
pub mod settings;
#[cfg(windows)]
mod windows;

use std::{
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use self::{
    audio::{AudioMessage, AudioRequest},
    display::{DisplayMessage, DisplayRequest},
    events::SystemHub,
    models::{SystemFocus, SystemSnapshot, SystemUpdate},
    monitor::{MonitorMessage, MonitorRequest},
};

/// How long a command waits for a worker. Native calls time out sooner,
/// except DDC/CI, which cannot be cancelled once sent.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
/// How long quitting waits for each worker to unregister its callbacks.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(400);

/// System failures. Messages are technical (for logs); the UI maps `kind`
/// to its own wording.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SystemError {
    #[error("the system service is unavailable")]
    Unavailable,
    #[error("not supported on this platform or hardware")]
    Unsupported,
    #[error("the device is no longer available")]
    NotFound,
    #[error("Windows did not allow this change")]
    Denied,
    #[error("the device did not respond in time")]
    Timeout,
    #[cfg_attr(not(windows), allow(dead_code))] // only constructed by native code
    #[error("system integration failed: {0}")]
    Platform(String),
}

impl SystemError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
            Self::NotFound => "notFound",
            Self::Denied => "denied",
            Self::Timeout => "timeout",
            Self::Platform(_) => "platform",
        }
    }
}

/// Where a worker sends a request's outcome.
pub type Reply = mpsc::Sender<Result<(), SystemError>>;

/// Requests the UI can make, routed to the owning worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemRequest {
    Audio(AudioRequest),
    Monitor(MonitorRequest),
    Display(DisplayRequest),
}

struct Workers {
    audio: mpsc::Sender<AudioMessage>,
    monitor: mpsc::Sender<MonitorMessage>,
    display: mpsc::Sender<DisplayMessage>,
    threads: Vec<JoinHandle<()>>,
}

/// Handle to the system workers, managed as Tauri state.
pub struct SystemService {
    hub: Arc<SystemHub>,
    audio: mpsc::Sender<AudioMessage>,
    monitor: mpsc::Sender<MonitorMessage>,
    display: mpsc::Sender<DisplayMessage>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new().name(name.into()).spawn(f)
}

/// Off Windows: power, network, radios are unsupported; CPU/RAM come from
/// `/proc` on Linux (the development host) so the pipeline stays testable.
#[cfg(not(windows))]
struct PortableMonitor;

#[cfg(not(windows))]
impl monitor::MonitorBackend for PortableMonitor {
    fn read_power(&mut self) -> Result<models::PowerState, SystemError> {
        Err(SystemError::Unsupported)
    }
    fn read_network(&mut self) -> Result<models::NetworkState, SystemError> {
        Err(SystemError::Unsupported)
    }
    fn read_radios(&mut self) -> Result<models::RadiosState, SystemError> {
        Err(SystemError::Unsupported)
    }
    fn read_metrics(&mut self) -> metrics::RawSample {
        #[cfg(target_os = "linux")]
        {
            metrics::read_linux()
        }
        #[cfg(not(target_os = "linux"))]
        {
            metrics::RawSample {
                cpu: None,
                memory: None,
            }
        }
    }
    fn set_radio(&mut self, _: models::RadioKind, _: bool) -> Result<(), SystemError> {
        Err(SystemError::Unsupported)
    }
    fn shutdown(&mut self) {}
}

#[cfg(not(windows))]
enum NoBackend {}

#[cfg(not(windows))]
impl audio::AudioBackend for NoBackend {
    fn refresh_devices(
        &mut self,
    ) -> Result<(Option<audio::DefaultDevice>, Vec<models::AudioDevice>), SystemError> {
        match *self {}
    }
    fn read_volume(&mut self) -> Result<Option<(u8, bool)>, SystemError> {
        match *self {}
    }
    fn set_volume(&mut self, _: u8) -> Result<(), SystemError> {
        match *self {}
    }
    fn set_muted(&mut self, _: bool) -> Result<(), SystemError> {
        match *self {}
    }
    fn shutdown(&mut self) {
        match *self {}
    }
}

#[cfg(not(windows))]
impl display::DisplayBackend for NoBackend {
    fn enumerate(&mut self) -> Result<models::DisplayState, SystemError> {
        match *self {}
    }
    fn refresh(&mut self) -> Result<models::DisplayState, SystemError> {
        match *self {}
    }
    fn set_brightness(&mut self, _: display::DisplayId, _: u8) -> Result<(), SystemError> {
        match *self {}
    }
}

fn start_workers(hub: &Arc<SystemHub>) -> std::io::Result<Workers> {
    let (audio_tx, audio_rx) = mpsc::channel();
    let (monitor_tx, monitor_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut threads = Vec::new();

    let h = hub.clone();
    #[cfg(windows)]
    let events = audio_tx.clone();
    threads.push(spawn("islandy-audio", move || {
        #[cfg(windows)]
        windows::with_mta(|| {
            audio::run(audio_rx, h, move || {
                windows::audio::CoreAudio::connect(events.clone())
            })
        });
        #[cfg(not(windows))]
        audio::run::<NoBackend>(audio_rx, h, || Err(SystemError::Unsupported));
    })?);

    let h = hub.clone();
    #[cfg(windows)]
    let events = monitor_tx.clone();
    threads.push(spawn("islandy-monitor", move || {
        #[cfg(windows)]
        windows::with_mta(|| {
            monitor::run(monitor_rx, h, windows::monitor::WinMonitor::connect(events))
        });
        #[cfg(not(windows))]
        monitor::run(monitor_rx, h, PortableMonitor);
    })?);

    let h = hub.clone();
    threads.push(spawn("islandy-display", move || {
        #[cfg(windows)]
        windows::with_mta(|| display::run(display_rx, h, windows::display::WinDisplay::connect));
        #[cfg(not(windows))]
        display::run::<NoBackend>(display_rx, h, || Err(SystemError::Unsupported));
    })?);

    Ok(Workers {
        audio: audio_tx,
        monitor: monitor_tx,
        display: display_tx,
        threads,
    })
}

impl SystemService {
    /// Starts the workers. Never blocks the caller: all native discovery
    /// happens on the worker threads.
    pub fn start() -> std::io::Result<Self> {
        let hub = Arc::new(SystemHub::default());
        let w = start_workers(&hub)?;
        Ok(Self {
            hub,
            audio: w.audio,
            monitor: w.monitor,
            display: w.display,
            threads: Mutex::new(w.threads),
        })
    }

    /// Registers the UI sink and returns the current snapshot atomically.
    pub fn connect(&self, sink: impl Fn(SystemUpdate) + Send + 'static) -> SystemSnapshot {
        self.hub.connect(sink)
    }

    pub fn snapshot(&self) -> SystemSnapshot {
        self.hub.snapshot()
    }

    /// Tells the workers what the UI shows, to start/stop optional polling.
    pub fn set_focus(&self, focus: SystemFocus) {
        let _ = self.monitor.send(MonitorMessage::Focus(focus));
        let _ = self
            .display
            .send(DisplayMessage::Watch(focus == SystemFocus::Display));
    }

    /// Runs a request on its worker without blocking an async runtime thread.
    pub async fn request(&self, request: SystemRequest) -> Result<(), SystemError> {
        let (reply, rx) = mpsc::channel();
        let sent = match request {
            SystemRequest::Audio(r) => self.audio.send(AudioMessage::Request(r, reply)).is_ok(),
            SystemRequest::Monitor(r) => {
                self.monitor.send(MonitorMessage::Request(r, reply)).is_ok()
            }
            SystemRequest::Display(r) => {
                self.display.send(DisplayMessage::Request(r, reply)).is_ok()
            }
        };
        if !sent {
            return Err(SystemError::Unavailable);
        }
        match tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(REQUEST_TIMEOUT)).await {
            Ok(Ok(result)) => result,
            Ok(Err(mpsc::RecvTimeoutError::Timeout)) => Err(SystemError::Timeout),
            Ok(Err(mpsc::RecvTimeoutError::Disconnected)) | Err(_) => Err(SystemError::Unavailable),
        }
    }

    /// Unregisters native callbacks and stops the workers (bounded waits).
    /// A worker stuck in a native call is left to the process exit.
    pub fn shutdown(&self) {
        let (a, audio) = mpsc::channel();
        let (m, monitor) = mpsc::channel();
        let (d, display) = mpsc::channel();
        let acks = [
            self.audio.send(AudioMessage::Shutdown(a)).is_ok(),
            self.monitor.send(MonitorMessage::Shutdown(m)).is_ok(),
            self.display.send(DisplayMessage::Shutdown(d)).is_ok(),
        ];
        let done = [audio, monitor, display]
            .iter()
            .zip(acks)
            .all(|(rx, sent)| sent && rx.recv_timeout(SHUTDOWN_TIMEOUT).is_ok());
        if done {
            let threads =
                std::mem::take(&mut *self.threads.lock().unwrap_or_else(|e| e.into_inner()));
            for thread in threads {
                let _ = thread.join();
            }
        }
    }
}

/// Validates a 0–100 value received over IPC.
pub fn parse_percent(value: f64) -> Option<u8> {
    (value.is_finite() && (0.0..=100.0).contains(&value)).then(|| value.round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kinds_match_the_frontend() {
        assert_eq!(SystemError::NotFound.kind(), "notFound");
        assert_eq!(SystemError::Denied.kind(), "denied");
        assert_eq!(SystemError::Unsupported.kind(), "unsupported");
        assert_eq!(SystemError::Platform("x".into()).kind(), "platform");
    }

    #[test]
    fn percent_arguments_are_validated() {
        assert_eq!(parse_percent(0.0), Some(0));
        assert_eq!(parse_percent(42.4), Some(42));
        assert_eq!(parse_percent(100.0), Some(100));
        for bad in [-1.0, 100.5, f64::NAN, f64::INFINITY] {
            assert_eq!(parse_percent(bad), None, "{bad}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn service_runs_off_windows_and_shuts_down() {
        let service = SystemService::start().unwrap();
        let rt = tauri::async_runtime::block_on(
            service.request(SystemRequest::Audio(AudioRequest::SetVolume(10))),
        );
        assert_eq!(rt, Err(SystemError::Unsupported));
        let rt = tauri::async_runtime::block_on(service.request(SystemRequest::Monitor(
            MonitorRequest::SetRadio(models::RadioKind::Wifi, true),
        )));
        assert_eq!(rt, Err(SystemError::Unsupported));
        let s = service.snapshot();
        use models::Availability::Unsupported;
        assert_eq!(s.audio.status, Unsupported);
        assert_eq!(s.power.status, Unsupported);
        assert_eq!(s.display.status, Unsupported);
        service.shutdown();
        assert!(service.threads.lock().unwrap().is_empty());
    }
}
