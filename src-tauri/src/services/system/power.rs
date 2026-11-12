//! Battery and power source: normalization of `GetSystemPowerStatus` and
//! de-duplicated power alerts.
//!
//! `SYSTEM_POWER_STATUS` reference (Win32):
//! - `ACLineStatus`: 0 offline, 1 online, 255 unknown.
//! - `BatteryFlag`: bit flags 1 high, 2 low, 4 critical, 8 charging,
//!   128 no system battery; 255 unknown.
//! - `BatteryLifePercent`: 0–100, or 255 unknown.
//! - `SystemStatusFlag`: 0 battery saver off, 1 on (Windows 10+).

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use super::models::{Availability, BatteryInfo, ChargeState, PowerSource, PowerState, SystemAlert};

/// The raw fields of `SYSTEM_POWER_STATUS` that Islandy uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawPowerStatus {
    pub ac_line_status: u8,
    pub battery_flag: u8,
    pub battery_life_percent: u8,
    pub system_status_flag: u8,
}

const FLAG_CHARGING: u8 = 8;
const FLAG_NO_BATTERY: u8 = 128;
const UNKNOWN: u8 = 255;

pub fn normalize(raw: RawPowerStatus) -> PowerState {
    let source = match raw.ac_line_status {
        0 => PowerSource::Battery,
        1 => PowerSource::Ac,
        _ => PowerSource::Unknown,
    };
    let battery = if raw.battery_flag == UNKNOWN {
        BatteryInfo::Unknown
    } else if raw.battery_flag & FLAG_NO_BATTERY != 0 {
        // Desktops (and VMs) report no battery: never invent a percentage.
        BatteryInfo::Absent
    } else {
        let percent = (raw.battery_life_percent <= 100).then_some(raw.battery_life_percent);
        let state = if raw.battery_flag & FLAG_CHARGING != 0 {
            ChargeState::Charging
        } else {
            match source {
                PowerSource::Battery => ChargeState::Discharging,
                PowerSource::Ac => ChargeState::NotCharging,
                PowerSource::Unknown => ChargeState::Unknown,
            }
        };
        BatteryInfo::Present { percent, state }
    };
    let saver = match raw.system_status_flag {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    PowerState {
        status: Availability::Ready,
        source,
        battery,
        saver,
    }
}

/// Alert thresholds (percent, while discharging).
pub const LOW_PERCENT: u8 = 20;
pub const CRITICAL_PERCENT: u8 = 10;
/// An alert re-arms only after the level rises this far above its threshold
/// (or the PC is plugged in), so a level hovering at 20/19/20 alerts once.
const REARM_MARGIN: u8 = 5;

/// Turns power state changes into at most one alert per real event.
#[derive(Debug, Default)]
pub struct PowerAlerts {
    previous: Option<PowerState>,
    low_fired: bool,
    critical_fired: bool,
}

impl PowerAlerts {
    pub fn observe(&mut self, next: &PowerState) -> Option<SystemAlert> {
        if next.status != Availability::Ready {
            return None;
        }
        let BatteryInfo::Present { percent, .. } = next.battery else {
            self.previous = Some(next.clone());
            return None;
        };
        let first = self.previous.is_none();
        let previous_source = self
            .previous
            .replace(next.clone())
            .map(|p| p.source)
            .unwrap_or(PowerSource::Unknown);

        // Re-arm on AC power or after recovering above the threshold.
        let on_battery = next.source == PowerSource::Battery;
        if !on_battery || percent.is_some_and(|p| p >= LOW_PERCENT + REARM_MARGIN) {
            self.low_fired = false;
        }
        if !on_battery || percent.is_some_and(|p| p >= CRITICAL_PERCENT + REARM_MARGIN) {
            self.critical_fired = false;
        }

        let mut alert = None;
        if !first && previous_source != next.source {
            alert = match (previous_source, next.source) {
                (PowerSource::Battery, PowerSource::Ac) => {
                    Some(SystemAlert::PowerConnected { percent })
                }
                (PowerSource::Ac, PowerSource::Battery) => {
                    Some(SystemAlert::PowerDisconnected { percent })
                }
                _ => None,
            };
        }
        if let (true, Some(p)) = (on_battery, percent) {
            // Starting up already low only arms the state; no alert.
            if p <= CRITICAL_PERCENT && !self.critical_fired {
                self.critical_fired = true;
                self.low_fired = true;
                if !first {
                    alert = Some(SystemAlert::BatteryCritical { percent: p });
                }
            } else if p <= LOW_PERCENT && !self.low_fired {
                self.low_fired = true;
                if !first {
                    alert = Some(SystemAlert::BatteryLow { percent: p });
                }
            }
        }
        alert
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(ac: u8, flag: u8, percent: u8) -> RawPowerStatus {
        RawPowerStatus {
            ac_line_status: ac,
            battery_flag: flag,
            battery_life_percent: percent,
            system_status_flag: 0,
        }
    }

    #[test]
    fn normalizes_laptops() {
        let s = normalize(raw(0, 1, 86));
        assert_eq!(s.source, PowerSource::Battery);
        assert_eq!(
            s.battery,
            BatteryInfo::Present {
                percent: Some(86),
                state: ChargeState::Discharging
            }
        );
        assert_eq!(s.saver, Some(false));
        assert_eq!(
            normalize(raw(1, 8 | 2, 30)).battery,
            BatteryInfo::Present {
                percent: Some(30),
                state: ChargeState::Charging
            }
        );
        assert_eq!(
            normalize(raw(1, 1, 100)).battery,
            BatteryInfo::Present {
                percent: Some(100),
                state: ChargeState::NotCharging
            }
        );
    }

    #[test]
    fn desktops_have_no_battery_and_no_percentage() {
        let s = normalize(raw(1, 128, 255));
        assert_eq!(s.battery, BatteryInfo::Absent);
        assert_eq!(s.source, PowerSource::Ac);
        // Some firmware reports 100% with "no battery": still absent.
        assert_eq!(normalize(raw(1, 128, 100)).battery, BatteryInfo::Absent);
    }

    #[test]
    fn unknown_values_stay_unknown() {
        assert_eq!(normalize(raw(255, 255, 255)).battery, BatteryInfo::Unknown);
        assert_eq!(normalize(raw(255, 0, 50)).source, PowerSource::Unknown);
        assert_eq!(
            normalize(raw(0, 0, 255)).battery,
            BatteryInfo::Present {
                percent: None,
                state: ChargeState::Discharging
            }
        );
        assert_eq!(
            normalize(raw(0, 0, 150)).battery,
            normalize(raw(0, 0, 255)).battery
        );
        let saver = RawPowerStatus {
            system_status_flag: 1,
            ..raw(0, 0, 50)
        };
        assert_eq!(normalize(saver).saver, Some(true));
        let odd = RawPowerStatus {
            system_status_flag: 7,
            ..raw(0, 0, 50)
        };
        assert_eq!(normalize(odd).saver, None);
    }

    fn run(alerts: &mut PowerAlerts, steps: &[(u8, u8)]) -> Vec<SystemAlert> {
        steps
            .iter()
            .filter_map(|&(ac, p)| alerts.observe(&normalize(raw(ac, 0, p))))
            .collect()
    }

    #[test]
    fn low_battery_alerts_once_per_discharge() {
        let mut a = PowerAlerts::default();
        let got = run(
            &mut a,
            &[(0, 25), (0, 21), (0, 20), (0, 19), (0, 20), (0, 18)],
        );
        assert_eq!(got, [SystemAlert::BatteryLow { percent: 20 }]);
        let got = run(&mut a, &[(0, 11), (0, 10), (0, 9), (0, 10)]);
        assert_eq!(got, [SystemAlert::BatteryCritical { percent: 10 }]);
    }

    #[test]
    fn plugging_in_re_arms_and_reports_power_changes() {
        let mut a = PowerAlerts::default();
        let got = run(&mut a, &[(0, 19), (0, 18), (1, 18), (0, 18)]);
        assert_eq!(
            got,
            [
                SystemAlert::PowerConnected { percent: Some(18) },
                // Unplugged while low: the low alert takes priority.
                SystemAlert::BatteryLow { percent: 18 },
            ]
        );
        let got = run(&mut a, &[(1, 18)]);
        assert_eq!(got, [SystemAlert::PowerConnected { percent: Some(18) }]);
        let got = run(&mut a, &[(0, 60)]);
        assert_eq!(got, [SystemAlert::PowerDisconnected { percent: Some(60) }]);
    }

    #[test]
    fn startup_and_desktops_are_quiet() {
        let mut a = PowerAlerts::default();
        // Already critical at startup: no alert, and none right after.
        assert_eq!(run(&mut a, &[(0, 8), (0, 7)]), []);
        let mut desktop = PowerAlerts::default();
        let s = normalize(raw(1, 128, 255));
        assert_eq!(desktop.observe(&s), None);
        assert_eq!(desktop.observe(&normalize(raw(0, 128, 255))), None);
        let mut a = PowerAlerts::default();
        assert_eq!(
            a.observe(&PowerState::with_status(Availability::Loading)),
            None
        );
    }
}
