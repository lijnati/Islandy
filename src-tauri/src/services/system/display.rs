//! Display brightness: normalization and the display worker.
//!
//! Two documented interfaces, chosen per display:
//! - **Built-in panels** (laptops, tablets): WMI `root\WMI`
//!   `WmiMonitorBrightness` / `WmiMonitorBrightnessMethods.WmiSetBrightness`.
//! - **External monitors**: the Monitor Configuration API over DDC/CI
//!   (`GetMonitorBrightness` / `SetMonitorBrightness`). Many monitors don't
//!   implement DDC/CI or have it switched off in their on-screen menu; those
//!   are counted as unsupported.
//!
//! DDC/CI calls are slow (tens to hundreds of ms) and synchronous, so they run
//! on this dedicated worker, never on the UI thread or the other workers.
//! Brightness has no change notification Islandy can use without a window, so
//! it is re-read only while the display panel is open.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use super::{
    events::{Part, SystemHub},
    models::{Availability, DisplayState},
    Reply, SystemError,
};

/// DDC/CI reports a device-specific range; maps it to 0–100.
pub fn ddc_to_percent(min: u32, current: u32, max: u32) -> Option<u8> {
    if max <= min {
        return None;
    }
    let current = current.clamp(min, max);
    Some(((f64::from(current - min) * 100.0 / f64::from(max - min)).round()) as u8)
}

/// 0–100 → the monitor's own range.
pub fn percent_to_ddc(percent: u8, min: u32, max: u32) -> u32 {
    if max <= min {
        return min;
    }
    let span = f64::from(max - min);
    min + (span * f64::from(percent.min(100)) / 100.0).round() as u32
}

/// A display id from the UI: `"i<n>"` or `"e<n>"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayId {
    Internal(u32),
    External(u32),
}

impl DisplayId {
    pub fn parse(value: &str) -> Option<Self> {
        let (kind, digits) = value.split_at_checked(1)?;
        if digits.is_empty() || digits.len() > 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n = digits.parse().ok()?;
        match kind {
            "i" => Some(Self::Internal(n)),
            "e" => Some(Self::External(n)),
            _ => None,
        }
    }

    pub fn wire(self) -> String {
        match self {
            Self::Internal(n) => format!("i{n}"),
            Self::External(n) => format!("e{n}"),
        }
    }
}

/// Connected displays that neither interface could control. Internal panels
/// are not attached to monitor handles here, so they are subtracted from the
/// total.
pub fn unsupported_count(total_monitors: u32, controllable_external: u32, internal: u32) -> u32 {
    total_monitors.saturating_sub(controllable_external + internal)
}

/* ------------------------------------------------------------------------ */
/* Worker                                                                    */
/* ------------------------------------------------------------------------ */

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRequest {
    SetBrightness(DisplayId, u8),
}

pub enum DisplayMessage {
    /// The display panel opened (`true`) or closed.
    Watch(bool),
    Request(DisplayRequest, Reply),
    Shutdown(mpsc::Sender<()>),
}

pub trait DisplayBackend {
    /// Finds controllable displays and reads their brightness (slow).
    fn enumerate(&mut self) -> Result<DisplayState, SystemError>;
    /// Re-reads the brightness of the displays found by `enumerate` (cheaper).
    fn refresh(&mut self) -> Result<DisplayState, SystemError>;
    fn set_brightness(&mut self, id: DisplayId, percent: u8) -> Result<(), SystemError>;
}

/// Re-read interval while the display panel is open.
pub const WATCH_INTERVAL: Duration = Duration::from_secs(4);

fn publish_result(hub: &SystemHub, result: Result<DisplayState, SystemError>) {
    let state = match result {
        Ok(state) => state,
        Err(SystemError::Unsupported) => DisplayState::with_status(Availability::Unsupported),
        Err(e) => {
            crate::log_warn!("display", "display: {e}");
            DisplayState::with_status(Availability::Unavailable)
        }
    };
    hub.publish(Part::Display(state));
}

pub fn run<B: DisplayBackend>(
    rx: mpsc::Receiver<DisplayMessage>,
    hub: Arc<SystemHub>,
    connect: impl FnOnce() -> Result<B, SystemError>,
) {
    let mut backend = match connect() {
        Ok(b) => Some(b),
        Err(e) => {
            publish_result(&hub, Err(e));
            None
        }
    };
    if let Some(b) = backend.as_mut() {
        publish_result(&hub, b.enumerate());
    }
    let mut next_poll: Option<Instant> = None;
    loop {
        let message = match next_poll {
            Some(at) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(m) => Some(m),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            },
            None => match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => return,
            },
        };
        match message {
            Some(DisplayMessage::Watch(watching)) => {
                next_poll = None;
                if let (true, Some(b)) = (watching, backend.as_mut()) {
                    // Opening the panel re-detects displays (hot-plug).
                    publish_result(&hub, b.enumerate());
                    next_poll = Some(Instant::now() + WATCH_INTERVAL);
                }
            }
            Some(DisplayMessage::Request(DisplayRequest::SetBrightness(id, percent), reply)) => {
                let result = match backend.as_mut() {
                    None => Err(SystemError::Unsupported),
                    Some(b) => {
                        let result = b.set_brightness(id, percent.min(100));
                        // Report what the display now says, success or not.
                        publish_result(&hub, b.refresh());
                        result
                    }
                };
                let _ = reply.send(result);
            }
            Some(DisplayMessage::Shutdown(ack)) => {
                drop(backend.take());
                let _ = ack.send(());
                return;
            }
            None => {
                if let Some(b) = backend.as_mut() {
                    publish_result(&hub, b.refresh());
                }
                next_poll = Some(Instant::now() + WATCH_INTERVAL);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::services::system::models::{DisplayInfo, DisplayKind};

    #[test]
    fn normalizes_ddc_ranges() {
        assert_eq!(ddc_to_percent(0, 50, 100), Some(50));
        assert_eq!(ddc_to_percent(0, 30, 60), Some(50));
        assert_eq!(ddc_to_percent(10, 5, 60), Some(0), "clamped low");
        assert_eq!(ddc_to_percent(0, 90, 60), Some(100), "clamped high");
        assert_eq!(ddc_to_percent(5, 5, 5), None, "empty range");
        assert_eq!(percent_to_ddc(50, 0, 60), 30);
        assert_eq!(percent_to_ddc(200, 0, 100), 100);
        assert_eq!(percent_to_ddc(0, 20, 80), 20);
        assert_eq!(percent_to_ddc(40, 7, 7), 7);
        for p in 0..=100 {
            assert_eq!(ddc_to_percent(0, percent_to_ddc(p, 0, 100), 100), Some(p));
        }
    }

    #[test]
    fn parses_display_ids_strictly() {
        assert_eq!(DisplayId::parse("i0"), Some(DisplayId::Internal(0)));
        assert_eq!(DisplayId::parse("e12"), Some(DisplayId::External(12)));
        assert_eq!(DisplayId::External(3).wire(), "e3");
        for bad in [
            "",
            "i",
            "x1",
            "e-1",
            "e1a",
            "e99999",
            "é1",
            "\\\\.\\DISPLAY1",
        ] {
            assert_eq!(DisplayId::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn counts_unsupported_displays() {
        assert_eq!(unsupported_count(2, 1, 0), 1);
        assert_eq!(unsupported_count(1, 0, 1), 0, "laptop panel");
        assert_eq!(unsupported_count(1, 1, 1), 0, "never negative");
    }

    struct Fake {
        brightness: Arc<Mutex<u8>>,
        enumerations: Arc<Mutex<u32>>,
    }

    impl Fake {
        fn state(&self) -> DisplayState {
            DisplayState {
                status: Availability::Ready,
                displays: vec![DisplayInfo {
                    id: "e0".into(),
                    name: "Monitor".into(),
                    kind: DisplayKind::External,
                    brightness: Some(*self.brightness.lock().unwrap()),
                }],
                unsupported_count: 1,
            }
        }
    }

    impl DisplayBackend for Fake {
        fn enumerate(&mut self) -> Result<DisplayState, SystemError> {
            *self.enumerations.lock().unwrap() += 1;
            Ok(self.state())
        }
        fn refresh(&mut self) -> Result<DisplayState, SystemError> {
            Ok(self.state())
        }
        fn set_brightness(&mut self, id: DisplayId, percent: u8) -> Result<(), SystemError> {
            if id != DisplayId::External(0) {
                return Err(SystemError::NotFound);
            }
            *self.brightness.lock().unwrap() = percent;
            Ok(())
        }
    }

    fn request(tx: &mpsc::Sender<DisplayMessage>, r: DisplayRequest) -> Result<(), SystemError> {
        let (reply, rx) = mpsc::channel();
        tx.send(DisplayMessage::Request(r, reply)).unwrap();
        rx.recv_timeout(Duration::from_secs(2)).unwrap()
    }

    #[test]
    fn sets_brightness_and_re_enumerates_when_watched() {
        let hub = Arc::new(SystemHub::default());
        let brightness = Arc::new(Mutex::new(40));
        let enumerations = Arc::new(Mutex::new(0));
        let (tx, rx) = mpsc::channel();
        let (h, b, e) = (hub.clone(), brightness.clone(), enumerations.clone());
        let thread = std::thread::spawn(move || {
            run(rx, h, move || {
                Ok(Fake {
                    brightness: b,
                    enumerations: e,
                })
            });
        });
        assert_eq!(
            request(
                &tx,
                DisplayRequest::SetBrightness(DisplayId::External(0), 70)
            ),
            Ok(())
        );
        let display = hub.snapshot().display;
        assert_eq!(display.displays[0].brightness, Some(70));
        assert_eq!(display.unsupported_count, 1);
        assert_eq!(
            request(
                &tx,
                DisplayRequest::SetBrightness(DisplayId::External(5), 10)
            ),
            Err(SystemError::NotFound)
        );
        assert_eq!(*enumerations.lock().unwrap(), 1);
        tx.send(DisplayMessage::Watch(true)).unwrap();
        // External change while watched: picked up by the next poll/request.
        *brightness.lock().unwrap() = 20;
        tx.send(DisplayMessage::Watch(false)).unwrap();
        request(
            &tx,
            DisplayRequest::SetBrightness(DisplayId::External(0), 20),
        )
        .unwrap();
        assert_eq!(*enumerations.lock().unwrap(), 2);
        assert_eq!(hub.snapshot().display.displays[0].brightness, Some(20));
        drop(tx);
        thread.join().unwrap();
    }

    #[test]
    fn unsupported_platform() {
        let hub = Arc::new(SystemHub::default());
        let (tx, rx) = mpsc::channel();
        let h = hub.clone();
        let thread =
            std::thread::spawn(move || run::<Fake>(rx, h, || Err(SystemError::Unsupported)));
        assert_eq!(
            request(
                &tx,
                DisplayRequest::SetBrightness(DisplayId::Internal(0), 10)
            ),
            Err(SystemError::Unsupported)
        );
        assert_eq!(hub.snapshot().display.status, Availability::Unsupported);
        drop(tx);
        thread.join().unwrap();
    }
}
