//! Master volume: normalization and the audio worker.
//!
//! # Volume scale
//! Windows exposes the endpoint volume two ways: in decibels
//! (`GetMasterVolumeLevel`) and as a *scalar* in 0.0–1.0
//! (`GetMasterVolumeLevelScalar`). The scalar is not linear amplitude: Windows
//! already maps it onto an audio-tapered curve, and it is exactly the value
//! the Windows volume flyout and Settings show (×100). Islandy therefore uses
//! the scalar, so "40%" in Islandy is "40" in Windows. It never converts
//! decibels or amplitudes itself.
//!
//! # Threading
//! The worker thread owns every Core Audio object. Callbacks
//! (`IAudioEndpointVolumeCallback`, `IMMNotificationClient`) only post an
//! [`AudioEvent`]; bursts (dragging the Windows slider) are coalesced.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use super::{
    events::{Part, SystemHub},
    models::{AudioDevice, AudioOutput, AudioState, Availability, SystemAlert},
    Reply, SystemError,
};

/// Windows scalar (0.0–1.0) → UI percent. Out-of-range or NaN values clamp.
pub fn scalar_to_percent(scalar: f32) -> u8 {
    if !scalar.is_finite() {
        return 0;
    }
    (scalar.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// UI percent → Windows scalar.
pub fn percent_to_scalar(percent: u8) -> f32 {
    f32::from(percent.min(100)) / 100.0
}

/// An opaque, stable key for an endpoint id (FNV-1a). The raw endpoint id is
/// not needed by the UI.
pub fn device_key(endpoint_id: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in endpoint_id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("d{hash:016x}")
}

/// Endpoint names come from drivers; keep them printable and bounded.
pub fn clean_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let cleaned: String = cleaned.chars().take(120).collect();
    if cleaned.is_empty() {
        "Audio device".into()
    } else {
        cleaned
    }
}

/// A peek-worthy change: the default output switched to another device
/// (not the first reading, not a volume change).
pub fn output_change_alert(prev: &AudioState, next: &AudioState) -> Option<SystemAlert> {
    match (&prev.output, &next.output) {
        (Some(a), Some(b))
            if prev.status == Availability::Ready
                && next.status == Availability::Ready
                && a.device_id != b.device_id =>
        {
            Some(SystemAlert::AudioOutputChanged {
                name: b.name.clone(),
            })
        }
        _ => None,
    }
}

/* ------------------------------------------------------------------------ */
/* Worker                                                                    */
/* ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioEvent {
    /// Volume or mute changed (from anywhere, including Islandy).
    VolumeChanged,
    /// Default device changed, or a device was added/removed/renamed.
    DevicesChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioRequest {
    SetVolume(u8),
    SetMuted(bool),
}

pub enum AudioMessage {
    Native(AudioEvent),
    Request(AudioRequest, Reply),
    Shutdown(mpsc::Sender<()>),
}

/// The default output device as reported by the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultDevice {
    pub id: String,
    pub name: String,
}

/// The native side, implemented by Core Audio on Windows and fakes in tests.
pub trait AudioBackend {
    /// Enumerates active outputs and binds the volume callback to the current
    /// default output. `None`: there is no default output.
    fn refresh_devices(&mut self)
        -> Result<(Option<DefaultDevice>, Vec<AudioDevice>), SystemError>;
    /// Volume (0–100) and mute of the bound default output.
    fn read_volume(&mut self) -> Result<Option<(u8, bool)>, SystemError>;
    fn set_volume(&mut self, percent: u8) -> Result<(), SystemError>;
    fn set_muted(&mut self, muted: bool) -> Result<(), SystemError>;
    /// Unregisters callbacks. Also called from `Drop` implementations.
    fn shutdown(&mut self);
}

/// Bursts of native events within this window become one re-read.
const COALESCE: Duration = Duration::from_millis(40);
const RETRY_MIN: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(300);

struct Worker<B> {
    hub: Arc<SystemHub>,
    backend: Option<B>,
    state: AudioState,
    default: Option<DefaultDevice>,
    pending_volume: bool,
    pending_devices: bool,
    flush_at: Option<Instant>,
    retry_at: Option<Instant>,
    retry_delay: Duration,
    supported: bool,
}

impl<B: AudioBackend> Worker<B> {
    fn publish(&mut self, next: AudioState) {
        if let Some(alert) = output_change_alert(&self.state, &next) {
            self.hub.alert(alert);
        }
        self.state = next.clone();
        self.hub.publish(Part::Audio(next));
    }

    fn connect(&mut self, connect: &mut impl FnMut() -> Result<B, SystemError>) {
        match connect() {
            Ok(backend) => {
                self.backend = Some(backend);
                self.retry_at = None;
                self.retry_delay = RETRY_MIN;
                self.reload(true);
            }
            Err(SystemError::Unsupported) => {
                self.supported = false;
                self.retry_at = None;
                self.publish(AudioState::with_status(Availability::Unsupported));
            }
            Err(e) => {
                eprintln!("[islandy] audio: connect failed: {e}");
                self.fail();
            }
        }
    }

    /// Drops the backend and schedules a reconnect with backoff.
    fn fail(&mut self) {
        if let Some(mut backend) = self.backend.take() {
            backend.shutdown();
        }
        self.default = None;
        self.retry_at = Some(Instant::now() + self.retry_delay);
        self.retry_delay = (self.retry_delay * 2).min(RETRY_MAX);
        self.publish(AudioState::with_status(Availability::Unavailable));
    }

    fn reload(&mut self, devices: bool) {
        let Some(backend) = self.backend.as_mut() else {
            return;
        };
        let mut list = self.state.devices.clone();
        if devices || self.state.status != Availability::Ready {
            match backend.refresh_devices() {
                Ok((default, all)) => {
                    self.default = default;
                    list = all;
                }
                Err(e) => {
                    eprintln!("[islandy] audio: enumerating devices failed: {e}");
                    return self.fail();
                }
            }
        }
        let volume = match backend.read_volume() {
            Ok(v) => v,
            Err(SystemError::NotFound) => {
                // The bound device vanished between events: re-enumerate soon.
                self.schedule(AudioEvent::DevicesChanged);
                return;
            }
            Err(e) => {
                eprintln!("[islandy] audio: reading volume failed: {e}");
                return self.fail();
            }
        };
        let output = match (&self.default, volume) {
            (Some(device), Some((volume, muted))) => Some(AudioOutput {
                device_id: device_key(&device.id),
                name: device.name.clone(),
                volume,
                muted,
            }),
            _ => None,
        };
        self.publish(AudioState {
            status: Availability::Ready,
            output,
            devices: list,
        });
    }

    fn schedule(&mut self, event: AudioEvent) {
        match event {
            AudioEvent::VolumeChanged => self.pending_volume = true,
            AudioEvent::DevicesChanged => self.pending_devices = true,
        }
        self.flush_at
            .get_or_insert_with(|| Instant::now() + COALESCE);
    }

    fn flush(&mut self) {
        if self.flush_at.take().is_none() {
            return;
        }
        let devices = std::mem::take(&mut self.pending_devices);
        self.pending_volume = false;
        self.reload(devices);
    }

    fn handle(&mut self, request: AudioRequest) -> Result<(), SystemError> {
        self.flush();
        let Some(backend) = self.backend.as_mut() else {
            return Err(if self.supported {
                SystemError::Unavailable
            } else {
                SystemError::Unsupported
            });
        };
        if self.state.output.is_none() {
            return Err(SystemError::NotFound);
        }
        let result = match request {
            AudioRequest::SetVolume(percent) => backend.set_volume(percent.min(100)),
            AudioRequest::SetMuted(muted) => backend.set_muted(muted),
        };
        if result.is_ok() {
            // Confirm from Windows (the callback would also report it).
            self.reload(false);
        } else if result == Err(SystemError::NotFound) {
            self.schedule(AudioEvent::DevicesChanged);
        }
        result
    }
}

/// The audio worker loop. Runs until `Shutdown` or the sender is dropped.
pub fn run<B: AudioBackend>(
    rx: mpsc::Receiver<AudioMessage>,
    hub: Arc<SystemHub>,
    mut connect: impl FnMut() -> Result<B, SystemError>,
) {
    let mut w = Worker {
        hub,
        backend: None,
        state: AudioState::with_status(Availability::Loading),
        default: None,
        pending_volume: false,
        pending_devices: false,
        flush_at: None,
        retry_at: None,
        retry_delay: RETRY_MIN,
        supported: true,
    };
    w.connect(&mut connect);
    loop {
        let deadline = [w.flush_at, w.retry_at].into_iter().flatten().min();
        let message = match deadline {
            Some(at) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(m) => Some(m),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            },
            None => match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => break,
            },
        };
        match message {
            Some(AudioMessage::Native(event)) => w.schedule(event),
            Some(AudioMessage::Request(request, reply)) => {
                let _ = reply.send(w.handle(request));
            }
            Some(AudioMessage::Shutdown(ack)) => {
                if let Some(mut backend) = w.backend.take() {
                    backend.shutdown();
                }
                let _ = ack.send(());
                return;
            }
            None => {
                let now = Instant::now();
                if w.flush_at.is_some_and(|at| at <= now) {
                    w.flush();
                }
                if w.retry_at.is_some_and(|at| at <= now) {
                    w.retry_at = None;
                    w.connect(&mut connect);
                }
            }
        }
    }
    if let Some(mut backend) = w.backend.take() {
        backend.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::services::system::models::{SystemSnapshot, SystemUpdate};

    #[test]
    fn converts_between_scalar_and_percent() {
        assert_eq!(scalar_to_percent(0.0), 0);
        assert_eq!(scalar_to_percent(0.404), 40);
        assert_eq!(scalar_to_percent(0.405), 41);
        assert_eq!(scalar_to_percent(1.0), 100);
        assert_eq!(scalar_to_percent(1.7), 100);
        assert_eq!(scalar_to_percent(-0.2), 0);
        assert_eq!(scalar_to_percent(f32::NAN), 0);
        assert_eq!(percent_to_scalar(40), 0.4);
        assert_eq!(percent_to_scalar(250), 1.0);
        for p in 0..=100u8 {
            assert_eq!(scalar_to_percent(percent_to_scalar(p)), p, "round trip {p}");
        }
    }

    #[test]
    fn device_keys_are_stable_and_opaque() {
        let id = "{0.0.0.00000000}.{6b0b0b0b-1111-2222-3333-444455556666}";
        assert_eq!(device_key(id), device_key(id));
        assert_ne!(device_key(id), device_key("other"));
        assert!(device_key(id).starts_with('d'));
        assert!(!device_key(id).contains("6b0b"));
    }

    #[test]
    fn cleans_driver_names() {
        assert_eq!(
            clean_name("  Speakers\t(Realtek\u{0}) "),
            "Speakers (Realtek )"
        );
        assert_eq!(clean_name(""), "Audio device");
        assert_eq!(clean_name(&"x".repeat(500)).len(), 120);
    }

    fn state(device: &str) -> AudioState {
        AudioState {
            status: Availability::Ready,
            output: Some(AudioOutput {
                device_id: device.into(),
                name: format!("{device} name"),
                volume: 10,
                muted: false,
            }),
            devices: vec![],
        }
    }

    #[test]
    fn alerts_only_on_a_real_output_switch() {
        assert_eq!(
            output_change_alert(&state("a"), &state("b")),
            Some(SystemAlert::AudioOutputChanged {
                name: "b name".into()
            })
        );
        assert_eq!(output_change_alert(&state("a"), &state("a")), None);
        let loading = AudioState::with_status(Availability::Loading);
        assert_eq!(output_change_alert(&loading, &state("a")), None);
        let none = AudioState {
            output: None,
            ..state("a")
        };
        assert_eq!(output_change_alert(&none, &state("a")), None);
    }

    /* -------------------------- worker tests --------------------------- */

    #[derive(Default)]
    struct Native {
        default: Option<&'static str>,
        volume: u8,
        muted: bool,
        enumerations: u32,
        fail_set: Option<SystemError>,
    }

    struct Fake(Arc<Mutex<Native>>);

    impl AudioBackend for Fake {
        fn refresh_devices(
            &mut self,
        ) -> Result<(Option<DefaultDevice>, Vec<AudioDevice>), SystemError> {
            let mut n = self.0.lock().unwrap();
            n.enumerations += 1;
            let default = n.default.map(|id| DefaultDevice {
                id: id.into(),
                name: id.to_uppercase(),
            });
            let devices = ["spk", "hdmi"]
                .iter()
                .map(|id| AudioDevice {
                    id: device_key(id),
                    name: id.to_uppercase(),
                    is_default: Some(*id) == n.default,
                })
                .collect();
            Ok((default, devices))
        }
        fn read_volume(&mut self) -> Result<Option<(u8, bool)>, SystemError> {
            let n = self.0.lock().unwrap();
            Ok(n.default.map(|_| (n.volume, n.muted)))
        }
        fn set_volume(&mut self, percent: u8) -> Result<(), SystemError> {
            let mut n = self.0.lock().unwrap();
            if let Some(e) = n.fail_set.clone() {
                return Err(e);
            }
            n.volume = percent;
            Ok(())
        }
        fn set_muted(&mut self, muted: bool) -> Result<(), SystemError> {
            self.0.lock().unwrap().muted = muted;
            Ok(())
        }
        fn shutdown(&mut self) {}
    }

    struct Harness {
        tx: mpsc::Sender<AudioMessage>,
        hub: Arc<SystemHub>,
        updates: Arc<Mutex<Vec<SystemUpdate>>>,
        native: Arc<Mutex<Native>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Harness {
        fn start(native: Native) -> Self {
            let hub = Arc::new(SystemHub::default());
            let updates = Arc::new(Mutex::new(Vec::new()));
            let sink = updates.clone();
            hub.connect(move |u| sink.lock().unwrap().push(u));
            let native = Arc::new(Mutex::new(native));
            let (tx, rx) = mpsc::channel();
            let (h, n) = (hub.clone(), native.clone());
            let thread = std::thread::spawn(move || run(rx, h, move || Ok(Fake(n.clone()))));
            Self {
                tx,
                hub,
                updates,
                native,
                thread: Some(thread),
            }
        }
        fn request(&self, request: AudioRequest) -> Result<(), SystemError> {
            let (reply, rx) = mpsc::channel();
            self.tx.send(AudioMessage::Request(request, reply)).unwrap();
            rx.recv_timeout(Duration::from_secs(2)).unwrap()
        }
        fn snapshot(&self) -> SystemSnapshot {
            self.hub.snapshot()
        }
        fn settle(&self) {
            // A request is processed after everything queued before it.
            let (reply, rx) = mpsc::channel();
            self.tx
                .send(AudioMessage::Request(
                    AudioRequest::SetMuted(self.native.lock().unwrap().muted),
                    reply,
                ))
                .unwrap();
            let _ = rx.recv_timeout(Duration::from_secs(2));
        }
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            let (ack, rx) = mpsc::channel();
            let _ = self.tx.send(AudioMessage::Shutdown(ack));
            let _ = rx.recv_timeout(Duration::from_secs(2));
            if let Some(t) = self.thread.take() {
                t.join().unwrap();
            }
        }
    }

    #[test]
    fn reads_the_default_output_and_applies_requests() {
        let h = Harness::start(Native {
            default: Some("spk"),
            volume: 30,
            ..Native::default()
        });
        h.settle();
        let audio = h.snapshot().audio;
        assert_eq!(audio.status, Availability::Ready);
        assert_eq!(audio.output.as_ref().unwrap().volume, 30);
        assert_eq!(audio.output.as_ref().unwrap().name, "SPK");
        assert_eq!(audio.devices.len(), 2);

        assert_eq!(h.request(AudioRequest::SetVolume(64)), Ok(()));
        assert_eq!(h.snapshot().audio.output.unwrap().volume, 64);
        assert_eq!(h.request(AudioRequest::SetMuted(true)), Ok(()));
        assert!(h.snapshot().audio.output.unwrap().muted);
    }

    #[test]
    fn external_changes_are_coalesced_without_re_enumerating() {
        let h = Harness::start(Native {
            default: Some("spk"),
            volume: 10,
            ..Native::default()
        });
        h.settle();
        let before = h.native.lock().unwrap().enumerations;
        for v in 11..=20 {
            h.native.lock().unwrap().volume = v;
            h.tx.send(AudioMessage::Native(AudioEvent::VolumeChanged))
                .unwrap();
        }
        std::thread::sleep(COALESCE * 4);
        assert_eq!(h.snapshot().audio.output.unwrap().volume, 20);
        assert_eq!(h.native.lock().unwrap().enumerations, before);
        let volume_updates = h
            .updates
            .lock()
            .unwrap()
            .iter()
            .filter(|u| matches!(u, SystemUpdate::Audio { .. }))
            .count();
        assert!(volume_updates <= 3, "coalesced, got {volume_updates}");
    }

    #[test]
    fn default_device_switch_re_enumerates_and_alerts_once() {
        let h = Harness::start(Native {
            default: Some("spk"),
            volume: 10,
            ..Native::default()
        });
        h.settle();
        h.native.lock().unwrap().default = Some("hdmi");
        h.tx.send(AudioMessage::Native(AudioEvent::DevicesChanged))
            .unwrap();
        h.tx.send(AudioMessage::Native(AudioEvent::VolumeChanged))
            .unwrap();
        std::thread::sleep(COALESCE * 4);
        let audio = h.snapshot().audio;
        assert_eq!(audio.output.unwrap().name, "HDMI");
        assert!(audio
            .devices
            .iter()
            .any(|d| d.is_default && d.name == "HDMI"));
        let alerts: Vec<_> = h
            .updates
            .lock()
            .unwrap()
            .iter()
            .filter_map(|u| match u {
                SystemUpdate::Alert { alert, .. } => Some(alert.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            alerts,
            [SystemAlert::AudioOutputChanged {
                name: "HDMI".into()
            }]
        );
    }

    #[test]
    fn no_output_device_and_failures_are_reported_honestly() {
        let h = Harness::start(Native::default());
        h.settle();
        let audio = h.snapshot().audio;
        assert_eq!(audio.status, Availability::Ready);
        assert!(audio.output.is_none());
        assert_eq!(
            h.request(AudioRequest::SetVolume(50)),
            Err(SystemError::NotFound)
        );

        let h = Harness::start(Native {
            default: Some("spk"),
            volume: 10,
            fail_set: Some(SystemError::Platform("x".into())),
            ..Native::default()
        });
        h.settle();
        assert_eq!(
            h.request(AudioRequest::SetVolume(90)),
            Err(SystemError::Platform("x".into()))
        );
        // Never reports the requested value as applied.
        assert_eq!(h.snapshot().audio.output.unwrap().volume, 10);
    }

    #[test]
    fn unsupported_platforms_say_so() {
        let hub = Arc::new(SystemHub::default());
        let (tx, rx) = mpsc::channel();
        let h = hub.clone();
        let thread = std::thread::spawn(move || {
            run::<Fake>(rx, h, || Err(SystemError::Unsupported));
        });
        let (reply, reply_rx) = mpsc::channel();
        tx.send(AudioMessage::Request(AudioRequest::SetVolume(5), reply))
            .unwrap();
        assert_eq!(reply_rx.recv().unwrap(), Err(SystemError::Unsupported));
        assert_eq!(hub.snapshot().audio.status, Availability::Unsupported);
        drop(tx);
        thread.join().unwrap();
    }
}
