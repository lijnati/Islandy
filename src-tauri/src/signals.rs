//! Rust → frontend push channel.
//!
//! The frontend hands us a Tauri `Channel` through the `connect_island`
//! command. Using a channel (instead of global events) means the frontend
//! needs no `core:event` permissions: it can only receive what we send here.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager, Runtime};

/// Mirrored by `IslandSignal` in `src/services/types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum IslandSignal {
    /// The global toggle shortcut was pressed.
    Toggle,
    /// The island window lost keyboard focus (click outside, Alt+Tab, …).
    FocusLost,
    /// Monitor geometry or scaling changed; re-fetch layouts.
    LayoutsChanged,
    /// Open the dashboard (tray left click).
    Open,
    /// The island is being hidden (tray, fullscreen app): collapse.
    Hidden,
}

#[derive(Default)]
pub struct SignalHub(Mutex<Option<Channel<IslandSignal>>>);

impl SignalHub {
    /// Replaces the current channel (a reloaded frontend reconnects).
    pub fn connect(&self, channel: Channel<IslandSignal>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(channel);
    }

    fn send(&self, signal: IslandSignal) {
        if let Some(channel) = self.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if let Err(e) = channel.send(signal) {
                crate::log_warn!("signals", "failed to deliver {signal:?}: {e}");
            }
        }
    }
}

pub fn emit<R: Runtime>(app: &AppHandle<R>, signal: IslandSignal) {
    #[cfg(debug_assertions)]
    crate::log_warn!("signals", "signal -> {signal:?}");
    app.state::<SignalHub>().send(signal);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_tagged_objects() {
        let json = serde_json::to_value(IslandSignal::LayoutsChanged).unwrap();
        assert_eq!(json, serde_json::json!({ "type": "layoutsChanged" }));
        let json = serde_json::to_value(IslandSignal::FocusLost).unwrap();
        assert_eq!(json, serde_json::json!({ "type": "focusLost" }));
    }
}
