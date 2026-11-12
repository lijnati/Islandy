//! Normalized system types that cross the IPC boundary.
//!
//! Mirrored by the `System*`/`Audio*`/`Power*`/… types in
//! `src/services/types.ts`. No raw Windows values or handles leave Rust:
//! everything is reduced to small, explicit enums and plain numbers, and
//! "unknown" is always representable instead of guessed.

// Some variants are only constructed by the Windows backends.
#![cfg_attr(not(windows), allow(dead_code))]

use serde::{Deserialize, Serialize};

/// Whether a domain could be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    /// Not read yet.
    Loading,
    Ready,
    /// The platform supports it, but reading failed (retried later).
    Unavailable,
    /// Not supported on this platform or hardware.
    Unsupported,
}

/* ------------------------------------------------------------------------ */
/* Audio                                                                     */
/* ------------------------------------------------------------------------ */

/// An active audio output endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    /// Opaque key (a hash of the endpoint id), stable while the device exists.
    pub id: String,
    /// Friendly name, e.g. "Speakers (Realtek(R) Audio)".
    pub name: String,
    pub is_default: bool,
}

/// The default output's master volume, as Windows shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutput {
    pub device_id: String,
    pub name: String,
    /// 0–100, the same number the Windows volume flyout shows.
    pub volume: u8,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioState {
    pub status: Availability,
    /// `None` when there is no active output device.
    pub output: Option<AudioOutput>,
    pub devices: Vec<AudioDevice>,
}

impl AudioState {
    pub const fn with_status(status: Availability) -> Self {
        Self {
            status,
            output: None,
            devices: Vec::new(),
        }
    }
}

/* ------------------------------------------------------------------------ */
/* Power                                                                     */
/* ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PowerSource {
    /// Plugged in.
    Ac,
    Battery,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChargeState {
    Charging,
    Discharging,
    /// On AC power and not charging (full, or held by a charge limit).
    NotCharging,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "presence")]
pub enum BatteryInfo {
    /// A battery is installed.
    #[serde(rename_all = "camelCase")]
    Present {
        /// `None` when Windows reports the level as unknown.
        percent: Option<u8>,
        state: ChargeState,
    },
    /// No system battery (desktop PCs). Never shown as "100%".
    Absent,
    /// Windows could not say.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerState {
    pub status: Availability,
    pub source: PowerSource,
    pub battery: BatteryInfo,
    /// Battery saver / energy saver; `None` when not reported.
    pub saver: Option<bool>,
}

impl PowerState {
    pub const fn with_status(status: Availability) -> Self {
        Self {
            status,
            source: PowerSource::Unknown,
            battery: BatteryInfo::Unknown,
            saver: None,
        }
    }
}

/* ------------------------------------------------------------------------ */
/* Network                                                                   */
/* ------------------------------------------------------------------------ */

/// Windows' own connectivity assessment (Network Connectivity Status
/// Indicator). Islandy makes no network requests to determine it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Connectivity {
    /// Connected and Windows has verified internet access.
    Internet,
    /// Connected, but internet access is limited (e.g. a captive portal).
    Constrained,
    /// Connected to a network without internet access.
    Local,
    /// No network connection.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionKind {
    Wifi,
    Ethernet,
    Cellular,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConnection {
    pub kind: ConnectionKind,
    /// Network (profile) name when Windows provides it. May be withheld by
    /// Windows privacy settings; never guessed.
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkState {
    pub status: Availability,
    pub connectivity: Connectivity,
    /// The connection Windows uses for internet traffic, if any.
    pub connection: Option<NetworkConnection>,
}

impl NetworkState {
    pub const fn with_status(status: Availability) -> Self {
        Self {
            status,
            connectivity: Connectivity::None,
            connection: None,
        }
    }
}

/* ------------------------------------------------------------------------ */
/* Radios (Wi-Fi, Bluetooth)                                                 */
/* ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RadioKind {
    Wifi,
    Bluetooth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RadioState {
    On,
    Off,
    /// The adapter is disabled (Device Manager, hardware switch, BIOS).
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioInfo {
    pub state: RadioState,
    /// Windows granted Islandy access to switch this radio, and the radio is
    /// in a state that can be switched. Otherwise the UI offers Settings.
    pub can_toggle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadiosState {
    pub status: Availability,
    /// `None`: no such adapter.
    pub wifi: Option<RadioInfo>,
    pub bluetooth: Option<RadioInfo>,
}

impl RadiosState {
    pub const fn with_status(status: Availability) -> Self {
        Self {
            status,
            wifi: None,
            bluetooth: None,
        }
    }
}

/* ------------------------------------------------------------------------ */
/* Metrics                                                                   */
/* ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUsage {
    pub total_bytes: u64,
    pub available_bytes: u64,
    /// 0–100, rounded.
    pub used_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsState {
    pub status: Availability,
    /// Whole-system CPU utilization over the last interval, 0–100. `None`
    /// until two samples exist.
    pub cpu_percent: Option<u8>,
    pub memory: Option<MemoryUsage>,
    /// Unix ms of the sample; 0 before the first one.
    pub sampled_at_ms: u64,
    /// Current sampling interval; `None` while sampling is suspended.
    pub interval_ms: Option<u64>,
}

impl MetricsState {
    pub const fn with_status(status: Availability) -> Self {
        Self {
            status,
            cpu_percent: None,
            memory: None,
            sampled_at_ms: 0,
            interval_ms: None,
        }
    }
}

/* ------------------------------------------------------------------------ */
/* Display                                                                   */
/* ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayKind {
    /// Built-in panel, controlled through the WMI brightness interface.
    Internal,
    /// External monitor, controlled over DDC/CI.
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    /// `"i<n>"` (internal) or `"e<n>"` (external); valid until the next
    /// enumeration.
    pub id: String,
    pub name: String,
    pub kind: DisplayKind,
    /// 0–100. `None` if the display stopped answering.
    pub brightness: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayState {
    pub status: Availability,
    /// Displays whose brightness Islandy can control.
    pub displays: Vec<DisplayInfo>,
    /// Connected displays without a supported brightness interface.
    pub unsupported_count: u32,
}

impl DisplayState {
    pub const fn with_status(status: Availability) -> Self {
        Self {
            status,
            displays: Vec::new(),
            unsupported_count: 0,
        }
    }
}

/* ------------------------------------------------------------------------ */
/* Alerts and the wire format                                                */
/* ------------------------------------------------------------------------ */

/// A meaningful, de-duplicated system change worth a brief peek. Routine
/// changes (CPU, RAM, volume) never produce alerts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SystemAlert {
    #[serde(rename_all = "camelCase")]
    BatteryLow {
        percent: u8,
    },
    #[serde(rename_all = "camelCase")]
    BatteryCritical {
        percent: u8,
    },
    #[serde(rename_all = "camelCase")]
    PowerConnected {
        percent: Option<u8>,
    },
    #[serde(rename_all = "camelCase")]
    PowerDisconnected {
        percent: Option<u8>,
    },
    NetworkLost,
    #[serde(rename_all = "camelCase")]
    NetworkRestored {
        kind: ConnectionKind,
        name: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    AudioOutputChanged {
        name: String,
    },
}

/// Everything at once: returned by `connect_system`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSnapshot {
    pub revision: u64,
    pub audio: AudioState,
    pub power: PowerState,
    pub network: NetworkState,
    pub radios: RadiosState,
    pub metrics: MetricsState,
    pub display: DisplayState,
}

impl SystemSnapshot {
    pub const fn initial() -> Self {
        Self {
            revision: 0,
            audio: AudioState::with_status(Availability::Loading),
            power: PowerState::with_status(Availability::Loading),
            network: NetworkState::with_status(Availability::Loading),
            radios: RadiosState::with_status(Availability::Loading),
            metrics: MetricsState::with_status(Availability::Loading),
            display: DisplayState::with_status(Availability::Loading),
        }
    }
}

/// One domain's new state (or an alert), pushed through the system channel.
/// Only the domain that changed is sent.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "domain")]
pub enum SystemUpdate {
    Audio { revision: u64, state: AudioState },
    Power { revision: u64, state: PowerState },
    Network { revision: u64, state: NetworkState },
    Radios { revision: u64, state: RadiosState },
    Metrics { revision: u64, state: MetricsState },
    Display { revision: u64, state: DisplayState },
    Alert { revision: u64, alert: SystemAlert },
}

/// What the UI is currently showing, which drives optional sampling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SystemFocus {
    /// Nothing system-related is visible: no polling at all.
    #[default]
    None,
    /// The system overview (CPU, RAM, battery) is open.
    Overview,
    /// The display panel is open.
    Display,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn battery_presence_is_explicit() {
        let present = BatteryInfo::Present {
            percent: None,
            state: ChargeState::Unknown,
        };
        assert_eq!(
            serde_json::to_value(present).unwrap(),
            json!({ "presence": "present", "percent": null, "state": "unknown" })
        );
        assert_eq!(
            serde_json::to_value(BatteryInfo::Absent).unwrap(),
            json!({ "presence": "absent" })
        );
    }

    #[test]
    fn updates_are_tagged_by_domain() {
        let update = SystemUpdate::Network {
            revision: 3,
            state: NetworkState::with_status(Availability::Ready),
        };
        assert_eq!(
            serde_json::to_value(update).unwrap(),
            json!({
                "domain": "network",
                "revision": 3,
                "state": { "status": "ready", "connectivity": "none", "connection": null }
            })
        );
        let alert = SystemUpdate::Alert {
            revision: 4,
            alert: SystemAlert::BatteryLow { percent: 15 },
        };
        assert_eq!(
            serde_json::to_value(alert).unwrap(),
            json!({ "domain": "alert", "revision": 4, "alert": { "type": "batteryLow", "percent": 15 } })
        );
    }

    #[test]
    fn snapshot_keys_are_camel_case() {
        let json = serde_json::to_value(SystemSnapshot::initial()).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            ["audio", "display", "metrics", "network", "power", "radios", "revision"]
        );
        assert_eq!(json["display"]["unsupportedCount"], 0);
        assert_eq!(json["metrics"]["cpuPercent"], serde_json::Value::Null);
    }

    #[test]
    fn focus_and_radio_kinds_deserialize_strictly() {
        assert_eq!(
            serde_json::from_str::<SystemFocus>("\"overview\"").unwrap(),
            SystemFocus::Overview
        );
        assert!(serde_json::from_str::<SystemFocus>("\"everything\"").is_err());
        assert_eq!(
            serde_json::from_str::<RadioKind>("\"bluetooth\"").unwrap(),
            RadioKind::Bluetooth
        );
        assert!(serde_json::from_str::<RadioKind>("\"nfc\"").is_err());
    }
}
