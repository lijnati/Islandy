//! Power, network, radios and CPU/RAM on Windows.
//!
//! Change notifications (all documented WinRT events, all agile):
//! - `Windows.System.Power.PowerManager`: battery status, power supply,
//!   remaining charge, energy saver.
//! - `Windows.Networking.Connectivity.NetworkInformation.NetworkStatusChanged`.
//! - `Windows.Devices.Radios.Radio.StateChanged` per radio.
//!
//! Handlers only post a [`MonitorEvent`]. Values are then read on the
//! monitor worker: power with `GetSystemPowerStatus` (one consistent
//! snapshot, including "no system battery"), network from the internet
//! connection profile, radios from `Radio::GetRadiosAsync`.

use std::{sync::mpsc::Sender, time::Duration};

use windows::{
    core::IInspectable,
    Devices::Radios::{
        Radio, RadioAccessStatus, RadioKind as WinRadioKind, RadioState as WinRadioState,
    },
    Foundation::{EventHandler, TypedEventHandler},
    Networking::Connectivity::{
        ConnectionProfile, NetworkInformation, NetworkStatusChangedEventHandler,
    },
    System::Power::PowerManager,
    Win32::{
        Foundation::FILETIME,
        System::{
            Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
            SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
            Threading::GetSystemTimes,
        },
    },
};

use super::{
    super::{
        metrics::{CpuTimes, RawSample},
        models::{
            Availability, Connectivity, NetworkConnection, NetworkState, PowerState, RadioKind,
            RadiosState,
        },
        monitor::{MonitorBackend, MonitorEvent, MonitorMessage},
        network, power, radios, SystemError,
    },
    platform,
};
use crate::services::winrt::{wait, WaitError};

const RADIO_TIMEOUT: Duration = Duration::from_secs(3);
const RADIO_SET_TIMEOUT: Duration = Duration::from_secs(5);

type Remove = fn(i64) -> windows::core::Result<()>;

pub struct WinMonitor {
    events: Sender<MonitorMessage>,
    /// Static event registrations and their removal functions.
    tokens: Vec<(Remove, i64)>,
    /// Radios found by the last `read_radios`, with their `StateChanged` token.
    radios: Vec<(Radio, Option<i64>)>,
    /// `RequestAccessAsync` result, asked on the first toggle (not at startup).
    radio_access: Option<bool>,
}

fn post(events: &Sender<MonitorMessage>, event: MonitorEvent) {
    let _ = events.send(MonitorMessage::Native(event));
}

fn waited<T>(result: Result<T, WaitError>, context: &str) -> Result<T, SystemError> {
    result.map_err(|e| match e {
        WaitError::Timeout => SystemError::Timeout,
        WaitError::Failed(e) => platform(context, &e),
    })
}

fn filetime(t: FILETIME) -> u64 {
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}

impl WinMonitor {
    /// Subscribes to change events. Never fails as a whole: a part whose
    /// subscription fails is still read on demand (e.g. when a panel opens).
    pub fn connect(events: Sender<MonitorMessage>) -> Self {
        let mut monitor = Self {
            events,
            tokens: Vec::new(),
            radios: Vec::new(),
            radio_access: None,
        };
        monitor.subscribe_power();
        monitor.subscribe_network();
        monitor
    }

    fn subscribe_power(&mut self) {
        let handler = || {
            let events = self.events.clone();
            EventHandler::<IInspectable>::new(move |_, _| {
                post(&events, MonitorEvent::Power);
                Ok(())
            })
        };
        let registrations: [(windows::core::Result<i64>, Remove); 4] = [
            (
                PowerManager::BatteryStatusChanged(&handler()),
                PowerManager::RemoveBatteryStatusChanged,
            ),
            (
                PowerManager::PowerSupplyStatusChanged(&handler()),
                PowerManager::RemovePowerSupplyStatusChanged,
            ),
            (
                PowerManager::RemainingChargePercentChanged(&handler()),
                PowerManager::RemoveRemainingChargePercentChanged,
            ),
            (
                PowerManager::EnergySaverStatusChanged(&handler()),
                PowerManager::RemoveEnergySaverStatusChanged,
            ),
        ];
        for (result, remove) in registrations {
            match result {
                Ok(token) => self.tokens.push((remove, token)),
                Err(e) => eprintln!("[islandy] power events unavailable: {e}"),
            }
        }
    }

    fn subscribe_network(&mut self) {
        let events = self.events.clone();
        let handler = NetworkStatusChangedEventHandler::new(move |_| {
            post(&events, MonitorEvent::Network);
            Ok(())
        });
        match NetworkInformation::NetworkStatusChanged(&handler) {
            Ok(token) => self
                .tokens
                .push((NetworkInformation::RemoveNetworkStatusChanged, token)),
            Err(e) => eprintln!("[islandy] network events unavailable: {e}"),
        }
    }

    fn unsubscribe_radios(&mut self) {
        for (radio, token) in self.radios.drain(..) {
            if let Some(token) = token {
                let _ = radio.RemoveStateChanged(token);
            }
        }
    }

    /// Asks once, on the first toggle, whether Islandy may switch radios.
    /// Not asked at startup, so nothing can prompt the user unprompted. The
    /// answer is cached; a denial turns the toggles into Settings shortcuts.
    fn radio_access(&mut self) -> bool {
        if let Some(allowed) = self.radio_access {
            return allowed;
        }
        let allowed = matches!(
            wait(Radio::RequestAccessAsync(), RADIO_TIMEOUT),
            Ok(RadioAccessStatus::Allowed)
        );
        self.radio_access = Some(allowed);
        allowed
    }

    fn connection_of(profile: &ConnectionProfile) -> NetworkConnection {
        let iana = profile
            .NetworkAdapter()
            .and_then(|a| a.IanaInterfaceType())
            .ok();
        NetworkConnection {
            kind: network::connection_kind(
                iana,
                profile.IsWlanConnectionProfile().unwrap_or(false),
                profile.IsWwanConnectionProfile().unwrap_or(false),
            ),
            name: profile
                .ProfileName()
                .ok()
                .and_then(|n| network::clean_profile_name(&n.to_string())),
        }
    }
}

impl MonitorBackend for WinMonitor {
    fn read_power(&mut self) -> Result<PowerState, SystemError> {
        let mut status = SYSTEM_POWER_STATUS::default();
        // SAFETY: fills a caller-owned struct.
        unsafe { GetSystemPowerStatus(&mut status) }
            .map_err(|e| platform("GetSystemPowerStatus", &e))?;
        Ok(power::normalize(power::RawPowerStatus {
            ac_line_status: status.ACLineStatus,
            battery_flag: status.BatteryFlag,
            battery_life_percent: status.BatteryLifePercent,
            system_status_flag: status.SystemStatusFlag,
        }))
    }

    fn read_network(&mut self) -> Result<NetworkState, SystemError> {
        // The profile Windows routes internet traffic through. Without one
        // (offline, or connected without internet) fall back to the best
        // connected profile, so "connected, no internet" is reported as such.
        if let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() {
            let level = profile
                .GetNetworkConnectivityLevel()
                .map_err(|e| platform("GetNetworkConnectivityLevel", &e))?;
            return Ok(network::state(
                network::map_level(level.0),
                Some(Self::connection_of(&profile)),
            ));
        }
        let profiles = NetworkInformation::GetConnectionProfiles()
            .map_err(|e| platform("GetConnectionProfiles", &e))?;
        let mut best: Option<(Connectivity, ConnectionProfile)> = None;
        for profile in profiles {
            let level = profile
                .GetNetworkConnectivityLevel()
                .map(|l| network::map_level(l.0))
                .unwrap_or(Connectivity::None);
            let rank = |c: Connectivity| match c {
                Connectivity::Internet => 3,
                Connectivity::Constrained => 2,
                Connectivity::Local => 1,
                Connectivity::None => 0,
            };
            if level != Connectivity::None
                && best.as_ref().map_or(true, |(b, _)| rank(level) > rank(*b))
            {
                best = Some((level, profile));
            }
        }
        Ok(match best {
            Some((level, profile)) => network::state(level, Some(Self::connection_of(&profile))),
            None => network::state(Connectivity::None, None),
        })
    }

    fn read_radios(&mut self) -> Result<RadiosState, SystemError> {
        // Until asked, offer the toggle; a denial is reported when tried.
        let access = self.radio_access != Some(false);
        let list = waited(
            wait(Radio::GetRadiosAsync(), RADIO_TIMEOUT),
            "GetRadiosAsync",
        )?;
        self.unsubscribe_radios();
        let (mut wifi, mut bluetooth) = (Vec::new(), Vec::new());
        for radio in list {
            let Ok(kind) = radio.Kind() else { continue };
            let target = if kind == WinRadioKind::WiFi {
                &mut wifi
            } else if kind == WinRadioKind::Bluetooth {
                &mut bluetooth
            } else {
                continue;
            };
            let state = radio
                .State()
                .map_or(radios::map_state(0), |s| radios::map_state(s.0));
            target.push(radios::info(state, access));
            let events = self.events.clone();
            let handler = TypedEventHandler::<Radio, IInspectable>::new(move |_, _| {
                post(&events, MonitorEvent::Radios);
                Ok(())
            });
            let token = radio.StateChanged(&handler).ok();
            self.radios.push((radio, token));
        }
        Ok(RadiosState {
            status: Availability::Ready,
            wifi: radios::merge(&wifi),
            bluetooth: radios::merge(&bluetooth),
        })
    }

    fn read_metrics(&mut self) -> RawSample {
        let (mut idle, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        // SAFETY: fills caller-owned FILETIMEs.
        let cpu = unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }
            .ok()
            .map(|()| CpuTimes::from_windows(filetime(idle), filetime(kernel), filetime(user)));
        let mut memory = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        // SAFETY: `dwLength` is set as the API requires.
        let memory = unsafe { GlobalMemoryStatusEx(&mut memory) }
            .ok()
            .map(|()| (memory.ullTotalPhys, memory.ullAvailPhys));
        RawSample { cpu, memory }
    }

    fn set_radio(&mut self, kind: RadioKind, on: bool) -> Result<(), SystemError> {
        if !self.radio_access() {
            return Err(SystemError::Denied);
        }
        let wanted = if kind == RadioKind::Wifi {
            WinRadioKind::WiFi
        } else {
            WinRadioKind::Bluetooth
        };
        let target = if on {
            WinRadioState::On
        } else {
            WinRadioState::Off
        };
        let mut found = false;
        for (radio, _) in &self.radios {
            if radio.Kind().ok() != Some(wanted) {
                continue;
            }
            let state = radio.State().ok();
            if state == Some(WinRadioState::Disabled) {
                continue;
            }
            found = true;
            let status = waited(
                wait(radio.SetStateAsync(target), RADIO_SET_TIMEOUT),
                "SetStateAsync",
            )?;
            if status != RadioAccessStatus::Allowed {
                return Err(SystemError::Denied);
            }
        }
        if found {
            Ok(())
        } else {
            Err(SystemError::NotFound)
        }
    }

    fn shutdown(&mut self) {
        self.unsubscribe_radios();
        for (remove, token) in self.tokens.drain(..) {
            let _ = remove(token);
        }
    }
}

impl Drop for WinMonitor {
    fn drop(&mut self) {
        self.shutdown();
    }
}
