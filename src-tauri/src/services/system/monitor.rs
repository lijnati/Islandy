//! The monitor worker: power, network, radios (event-driven) and CPU/RAM
//! (sampled only while the system overview is visible).
//!
//! Native callbacks (WinRT `PowerManager`, `NetworkInformation`, `Radio`
//! events) only post a [`MonitorEvent`]; the worker re-reads that one domain
//! after a short coalescing window and publishes it if it changed.
//!
//! Polling rules:
//! - Power, network and radios are never polled. They are re-read when their
//!   event fires, and once when a system panel opens (a cheap guard against a
//!   missed event).
//! - CPU/RAM are sampled every [`OVERVIEW_INTERVAL`] while the overview panel
//!   is open, and not at all otherwise.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::{
    events::{Part, SystemHub},
    metrics::{RawSample, Sampler},
    models::{
        Availability, MetricsState, NetworkState, PowerState, RadioKind, RadiosState, SystemFocus,
    },
    network::NetworkAlerts,
    power::PowerAlerts,
    Reply, SystemError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorEvent {
    Power,
    Network,
    Radios,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorRequest {
    SetRadio(RadioKind, bool),
}

pub enum MonitorMessage {
    Native(MonitorEvent),
    Focus(SystemFocus),
    Request(MonitorRequest, Reply),
    Shutdown(mpsc::Sender<()>),
}

pub trait MonitorBackend {
    fn read_power(&mut self) -> Result<PowerState, SystemError>;
    fn read_network(&mut self) -> Result<NetworkState, SystemError>;
    fn read_radios(&mut self) -> Result<RadiosState, SystemError>;
    fn read_metrics(&mut self) -> RawSample;
    /// Requests a radio state; the outcome is observed via `MonitorEvent::Radios`.
    fn set_radio(&mut self, kind: RadioKind, on: bool) -> Result<(), SystemError>;
    fn shutdown(&mut self);
}

pub const OVERVIEW_INTERVAL: Duration = Duration::from_secs(2);
/// The first CPU figure needs two samples; take the second one sooner.
pub const FIRST_SAMPLE_DELAY: Duration = Duration::from_millis(700);
const COALESCE: Duration = Duration::from_millis(150);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn status_of(e: &SystemError) -> Availability {
    if *e == SystemError::Unsupported {
        Availability::Unsupported
    } else {
        Availability::Unavailable
    }
}

struct Worker<B> {
    hub: Arc<SystemHub>,
    backend: B,
    focus: SystemFocus,
    sampler: Sampler,
    next_sample: Option<Instant>,
    pending: [bool; 3],
    flush_at: Option<Instant>,
    power_alerts: PowerAlerts,
    network_alerts: NetworkAlerts,
    /// Unix ms → Instant mapping for the network alert deadline.
    started: (Instant, u64),
}

impl<B: MonitorBackend> Worker<B> {
    fn instant_of(&self, unix_ms: u64) -> Instant {
        let (at, base) = self.started;
        at + Duration::from_millis(unix_ms.saturating_sub(base))
    }

    fn read_power(&mut self) {
        let state = self
            .backend
            .read_power()
            .unwrap_or_else(|e| PowerState::with_status(status_of(&e)));
        if let Some(alert) = self.power_alerts.observe(&state) {
            self.hub.alert(alert);
        }
        self.hub.publish(Part::Power(state));
    }

    fn read_network(&mut self) {
        let state = self
            .backend
            .read_network()
            .unwrap_or_else(|e| NetworkState::with_status(status_of(&e)));
        if let Some(alert) = self.network_alerts.observe(&state, now_ms()) {
            self.hub.alert(alert);
        }
        self.hub.publish(Part::Network(state));
    }

    fn read_radios(&mut self) {
        let state = self
            .backend
            .read_radios()
            .unwrap_or_else(|e| RadiosState::with_status(status_of(&e)));
        self.hub.publish(Part::Radios(state));
    }

    fn read_all(&mut self) {
        self.read_power();
        self.read_network();
        self.read_radios();
    }

    fn sample(&mut self) {
        let interval = (self.focus == SystemFocus::Overview).then_some(OVERVIEW_INTERVAL);
        let raw = self.backend.read_metrics();
        let state = self
            .sampler
            .sample(raw, now_ms(), interval.map(|d| d.as_millis() as u64));
        self.hub.publish(Part::Metrics(state));
    }

    fn set_focus(&mut self, focus: SystemFocus) {
        if focus == self.focus {
            return;
        }
        let was_overview = self.focus == SystemFocus::Overview;
        self.focus = focus;
        if focus == SystemFocus::Overview {
            self.sampler.reset();
            self.sample();
            self.next_sample = Some(Instant::now() + FIRST_SAMPLE_DELAY);
        } else if was_overview {
            self.next_sample = None;
            // Keep the last values, marked as no longer live.
            let mut state: MetricsState = self.hub.snapshot().metrics;
            state.interval_ms = None;
            self.hub.publish(Part::Metrics(state));
        }
        if focus != SystemFocus::None {
            self.read_all();
        }
    }

    fn schedule(&mut self, event: MonitorEvent) {
        self.pending[event as usize] = true;
        self.flush_at
            .get_or_insert_with(|| Instant::now() + COALESCE);
    }

    fn flush(&mut self) {
        self.flush_at = None;
        let [power, network, radios] = std::mem::take(&mut self.pending);
        if power {
            self.read_power();
        }
        if network {
            self.read_network();
        }
        if radios {
            self.read_radios();
        }
    }

    fn handle(&mut self, request: MonitorRequest) -> Result<(), SystemError> {
        match request {
            MonitorRequest::SetRadio(kind, on) => {
                let current = self.hub.snapshot().radios;
                let radio = match kind {
                    RadioKind::Wifi => current.wifi,
                    RadioKind::Bluetooth => current.bluetooth,
                };
                match radio {
                    None if current.status == Availability::Unsupported => {
                        Err(SystemError::Unsupported)
                    }
                    None => Err(SystemError::NotFound),
                    Some(r) if !r.can_toggle => Err(SystemError::Denied),
                    Some(_) => {
                        let result = self.backend.set_radio(kind, on);
                        // Report what Windows now says, whatever the result.
                        self.read_radios();
                        result
                    }
                }
            }
        }
    }

    fn deadline(&self) -> Option<Instant> {
        let network = self.network_alerts.deadline().map(|t| self.instant_of(t));
        [self.flush_at, self.next_sample, network]
            .into_iter()
            .flatten()
            .min()
    }

    fn on_timeout(&mut self) {
        let now = Instant::now();
        if self.flush_at.is_some_and(|t| t <= now) {
            self.flush();
        }
        if self.next_sample.is_some_and(|t| t <= now) {
            self.sample();
            self.next_sample = Some(now + OVERVIEW_INTERVAL);
        }
        if self
            .network_alerts
            .deadline()
            .is_some_and(|t| self.instant_of(t) <= now)
        {
            // Confirm with a fresh reading before announcing a loss.
            self.read_network();
            if let Some(alert) = self.network_alerts.poll(now_ms()) {
                self.hub.alert(alert);
            }
        }
    }
}

pub fn run<B: MonitorBackend>(rx: mpsc::Receiver<MonitorMessage>, hub: Arc<SystemHub>, backend: B) {
    let mut w = Worker {
        hub,
        backend,
        focus: SystemFocus::None,
        sampler: Sampler::default(),
        next_sample: None,
        pending: [false; 3],
        flush_at: None,
        power_alerts: PowerAlerts::default(),
        network_alerts: NetworkAlerts::default(),
        started: (Instant::now(), now_ms()),
    };
    w.read_all();
    // Metrics are idle until the overview opens.
    w.hub.publish(Part::Metrics(MetricsState::with_status(
        Availability::Ready,
    )));
    loop {
        let message = match w.deadline() {
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
            Some(MonitorMessage::Native(event)) => w.schedule(event),
            Some(MonitorMessage::Focus(focus)) => w.set_focus(focus),
            Some(MonitorMessage::Request(request, reply)) => {
                let _ = reply.send(w.handle(request));
            }
            Some(MonitorMessage::Shutdown(ack)) => {
                w.backend.shutdown();
                let _ = ack.send(());
                return;
            }
            None => w.on_timeout(),
        }
    }
    w.backend.shutdown();
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::services::system::{
        metrics::CpuTimes,
        models::{
            BatteryInfo, ChargeState, Connectivity, PowerSource, RadioInfo, RadioState,
            SystemAlert, SystemUpdate,
        },
        power::{normalize, RawPowerStatus},
    };

    #[derive(Default)]
    struct Native {
        percent: u8,
        on_ac: bool,
        online: bool,
        bluetooth: Option<RadioInfo>,
        samples: u64,
        set_radio_calls: u32,
    }

    struct Fake(Arc<Mutex<Native>>);

    impl MonitorBackend for Fake {
        fn read_power(&mut self) -> Result<PowerState, SystemError> {
            let n = self.0.lock().unwrap();
            Ok(normalize(RawPowerStatus {
                ac_line_status: u8::from(n.on_ac),
                battery_flag: 0,
                battery_life_percent: n.percent,
                system_status_flag: 0,
            }))
        }
        fn read_network(&mut self) -> Result<NetworkState, SystemError> {
            let online = self.0.lock().unwrap().online;
            Ok(crate::services::system::network::state(
                if online {
                    Connectivity::Internet
                } else {
                    Connectivity::None
                },
                None,
            ))
        }
        fn read_radios(&mut self) -> Result<RadiosState, SystemError> {
            Ok(RadiosState {
                status: Availability::Ready,
                wifi: None,
                bluetooth: self.0.lock().unwrap().bluetooth,
            })
        }
        fn read_metrics(&mut self) -> RawSample {
            let mut n = self.0.lock().unwrap();
            n.samples += 1;
            RawSample {
                cpu: Some(CpuTimes {
                    idle: n.samples * 50,
                    total: n.samples * 100,
                }),
                memory: Some((1000, 250)),
            }
        }
        fn set_radio(&mut self, _: RadioKind, on: bool) -> Result<(), SystemError> {
            let mut n = self.0.lock().unwrap();
            n.set_radio_calls += 1;
            if let Some(b) = n.bluetooth.as_mut() {
                b.state = if on { RadioState::On } else { RadioState::Off };
            }
            Ok(())
        }
        fn shutdown(&mut self) {}
    }

    struct Harness {
        tx: mpsc::Sender<MonitorMessage>,
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
            let thread = std::thread::spawn(move || run(rx, h, Fake(n)));
            Self {
                tx,
                hub,
                updates,
                native,
                thread: Some(thread),
            }
        }
        fn request(&self, r: MonitorRequest) -> Result<(), SystemError> {
            let (reply, rx) = mpsc::channel();
            self.tx.send(MonitorMessage::Request(r, reply)).unwrap();
            rx.recv_timeout(Duration::from_secs(2)).unwrap()
        }
        fn alerts(&self) -> Vec<SystemAlert> {
            self.updates
                .lock()
                .unwrap()
                .iter()
                .filter_map(|u| match u {
                    SystemUpdate::Alert { alert, .. } => Some(alert.clone()),
                    _ => None,
                })
                .collect()
        }
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            let (ack, rx) = mpsc::channel();
            let _ = self.tx.send(MonitorMessage::Shutdown(ack));
            let _ = rx.recv_timeout(Duration::from_secs(2));
            if let Some(t) = self.thread.take() {
                t.join().unwrap();
            }
        }
    }

    fn wait_for(cond: impl Fn() -> bool) {
        let end = Instant::now() + Duration::from_secs(3);
        while !cond() {
            assert!(Instant::now() < end, "timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn reads_everything_once_and_does_not_sample_until_the_overview_opens() {
        let h = Harness::start(Native {
            percent: 80,
            online: true,
            ..Native::default()
        });
        wait_for(|| h.hub.snapshot().metrics.status == Availability::Ready);
        let s = h.hub.snapshot();
        assert_eq!(s.power.source, PowerSource::Battery);
        assert_eq!(
            s.power.battery,
            BatteryInfo::Present {
                percent: Some(80),
                state: ChargeState::Discharging
            }
        );
        assert_eq!(s.network.connectivity, Connectivity::Internet);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            h.native.lock().unwrap().samples,
            0,
            "no sampling while hidden"
        );

        h.tx.send(MonitorMessage::Focus(SystemFocus::Overview))
            .unwrap();
        wait_for(|| h.hub.snapshot().metrics.cpu_percent.is_some());
        let m = h.hub.snapshot().metrics;
        assert_eq!(m.cpu_percent, Some(50));
        assert_eq!(m.memory.unwrap().used_percent, 75);
        assert_eq!(m.interval_ms, Some(2000));

        h.tx.send(MonitorMessage::Focus(SystemFocus::None)).unwrap();
        wait_for(|| h.hub.snapshot().metrics.interval_ms.is_none());
        let samples = h.native.lock().unwrap().samples;
        std::thread::sleep(FIRST_SAMPLE_DELAY + Duration::from_millis(200));
        assert_eq!(
            h.native.lock().unwrap().samples,
            samples,
            "sampling stopped"
        );
    }

    #[test]
    fn power_events_are_coalesced_and_alert_once() {
        let h = Harness::start(Native {
            percent: 22,
            online: true,
            ..Native::default()
        });
        wait_for(|| h.hub.snapshot().power.status == Availability::Ready);
        for p in [21, 20, 19] {
            h.native.lock().unwrap().percent = p;
            h.tx.send(MonitorMessage::Native(MonitorEvent::Power))
                .unwrap();
        }
        wait_for(|| {
            h.hub.snapshot().power.battery
                == BatteryInfo::Present {
                    percent: Some(19),
                    state: ChargeState::Discharging,
                }
        });
        // Coalesced into one read (19%) → still exactly one "low" alert.
        h.native.lock().unwrap().on_ac = true;
        h.tx.send(MonitorMessage::Native(MonitorEvent::Power))
            .unwrap();
        wait_for(|| h.hub.snapshot().power.source == PowerSource::Ac);
        assert_eq!(
            h.alerts(),
            [
                SystemAlert::BatteryLow { percent: 19 },
                SystemAlert::PowerConnected { percent: Some(19) }
            ]
        );
    }

    #[test]
    fn radio_requests_are_validated_and_confirmed_natively() {
        let h = Harness::start(Native {
            bluetooth: Some(RadioInfo {
                state: RadioState::Off,
                can_toggle: true,
            }),
            ..Native::default()
        });
        wait_for(|| h.hub.snapshot().radios.status == Availability::Ready);
        assert_eq!(
            h.request(MonitorRequest::SetRadio(RadioKind::Wifi, true)),
            Err(SystemError::NotFound)
        );
        assert_eq!(
            h.request(MonitorRequest::SetRadio(RadioKind::Bluetooth, true)),
            Ok(())
        );
        assert_eq!(
            h.hub.snapshot().radios.bluetooth.unwrap().state,
            RadioState::On
        );
        h.native.lock().unwrap().bluetooth = Some(RadioInfo {
            state: RadioState::Off,
            can_toggle: false,
        });
        h.tx.send(MonitorMessage::Native(MonitorEvent::Radios))
            .unwrap();
        wait_for(|| !h.hub.snapshot().radios.bluetooth.unwrap().can_toggle);
        assert_eq!(
            h.request(MonitorRequest::SetRadio(RadioKind::Bluetooth, true)),
            Err(SystemError::Denied)
        );
        assert_eq!(h.native.lock().unwrap().set_radio_calls, 1);
    }
}
