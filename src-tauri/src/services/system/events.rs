//! The system event bridge: one hub, one UI channel, per-domain updates.
//!
//! Workers publish a domain's *new* state; the hub drops it if nothing
//! changed, otherwise assigns the next revision, stores it and pushes a
//! [`SystemUpdate`] for that domain only. The UI applies an update only if its
//! revision is newer than the one it holds for that domain.
//!
//! Like the island and media channels, this uses a Tauri `Channel` handed
//! over by `connect_system`, so no `core:event` permission is required. A
//! reloaded frontend reconnects and simply replaces the sink.

use std::sync::{Mutex, MutexGuard};

use super::models::{
    AudioState, DisplayState, MetricsState, NetworkState, PowerState, RadiosState, SystemAlert,
    SystemSnapshot, SystemUpdate,
};

/// A domain's new state.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Audio(AudioState),
    Power(PowerState),
    Network(NetworkState),
    Radios(RadiosState),
    Metrics(MetricsState),
    Display(DisplayState),
}

type Sink = Box<dyn Fn(SystemUpdate) + Send>;

struct Inner {
    sink: Option<Sink>,
    snapshot: SystemSnapshot,
}

pub struct SystemHub(Mutex<Inner>);

impl Default for SystemHub {
    fn default() -> Self {
        Self(Mutex::new(Inner {
            sink: None,
            snapshot: SystemSnapshot::initial(),
        }))
    }
}

impl SystemHub {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Replaces the UI sink and returns the current snapshot, under one lock:
    /// no update can fall between the snapshot and the first pushed update.
    pub fn connect(&self, sink: impl Fn(SystemUpdate) + Send + 'static) -> SystemSnapshot {
        let mut inner = self.lock();
        inner.sink = Some(Box::new(sink));
        inner.snapshot.clone()
    }

    pub fn snapshot(&self) -> SystemSnapshot {
        self.lock().snapshot.clone()
    }

    /// Stores and pushes a domain's state. Returns `false` if it was unchanged.
    pub fn publish(&self, part: Part) -> bool {
        let mut inner = self.lock();
        let s = &mut inner.snapshot;
        let revision = s.revision + 1;
        macro_rules! apply {
            ($field:ident, $variant:ident, $state:expr) => {{
                if s.$field == $state {
                    return false;
                }
                s.$field = $state.clone();
                SystemUpdate::$variant {
                    revision,
                    state: $state,
                }
            }};
        }
        let update = match part {
            Part::Audio(state) => apply!(audio, Audio, state),
            Part::Power(state) => apply!(power, Power, state),
            Part::Network(state) => apply!(network, Network, state),
            Part::Radios(state) => apply!(radios, Radios, state),
            Part::Metrics(state) => apply!(metrics, Metrics, state),
            Part::Display(state) => apply!(display, Display, state),
        };
        s.revision = revision;
        if let Some(sink) = inner.sink.as_ref() {
            sink(update);
        }
        true
    }

    /// Pushes a contextual alert. Alerts are not stored: a UI that connects
    /// later doesn't replay old alerts.
    pub fn alert(&self, alert: SystemAlert) {
        let mut inner = self.lock();
        inner.snapshot.revision += 1;
        let revision = inner.snapshot.revision;
        #[cfg(debug_assertions)]
        eprintln!("[islandy] system alert r{revision}: {alert:?}");
        if let Some(sink) = inner.sink.as_ref() {
            sink(SystemUpdate::Alert { revision, alert });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::services::system::models::Availability;

    #[test]
    fn publishes_only_changes_with_increasing_revisions() {
        let hub = SystemHub::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let initial = hub.connect(move |u| sink.lock().unwrap().push(u));
        assert_eq!(initial.revision, 0);

        let ready = PowerState::with_status(Availability::Ready);
        assert!(hub.publish(Part::Power(ready.clone())));
        assert!(!hub.publish(Part::Power(ready.clone())), "unchanged");
        assert!(hub.publish(Part::Network(NetworkState::with_status(
            Availability::Unsupported
        ))));
        hub.alert(SystemAlert::NetworkLost);

        let seen = seen.lock().unwrap();
        let revisions: Vec<u64> = seen
            .iter()
            .map(|u| match u {
                SystemUpdate::Power { revision, .. }
                | SystemUpdate::Network { revision, .. }
                | SystemUpdate::Alert { revision, .. } => *revision,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(revisions, [1, 2, 3]);
        assert_eq!(hub.snapshot().power, ready);
        assert_eq!(hub.snapshot().revision, 3);
    }

    #[test]
    fn works_without_a_connected_ui() {
        let hub = SystemHub::default();
        assert!(hub.publish(Part::Audio(AudioState::with_status(
            Availability::Unavailable
        ))));
        hub.alert(SystemAlert::NetworkLost);
        assert_eq!(hub.snapshot().audio.status, Availability::Unavailable);
    }
}
