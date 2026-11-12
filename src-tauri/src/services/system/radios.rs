//! Wi-Fi and Bluetooth radios (`Windows.Devices.Radios`).
//!
//! Radio switching uses the documented `Radio.SetStateAsync`, and only after
//! `Radio.RequestAccessAsync` returned `Allowed`. A radio in the `Disabled`
//! state (adapter disabled, hardware switch) cannot be switched from
//! software, so it is never offered as a toggle. The UI shows the state
//! Windows reports afterwards; a request is never reported as successful on
//! its own.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use super::models::{RadioInfo, RadioState};

/// `Windows.Devices.Radios.RadioState` → [`RadioState`].
pub fn map_state(state: i32) -> RadioState {
    match state {
        1 => RadioState::On,
        2 => RadioState::Off,
        3 => RadioState::Disabled,
        _ => RadioState::Unknown,
    }
}

/// Whether a radio may be offered as a switch.
pub fn info(state: RadioState, access_allowed: bool) -> RadioInfo {
    RadioInfo {
        state,
        can_toggle: access_allowed && matches!(state, RadioState::On | RadioState::Off),
    }
}

/// Combines several radios of one kind (rare: e.g. two Wi-Fi adapters).
/// On if any is on; the first otherwise.
pub fn merge(radios: &[RadioInfo]) -> Option<RadioInfo> {
    radios
        .iter()
        .find(|r| r.state == RadioState::On)
        .or_else(|| radios.first())
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_states_and_toggle_eligibility() {
        assert_eq!(map_state(1), RadioState::On);
        assert_eq!(map_state(2), RadioState::Off);
        assert_eq!(map_state(3), RadioState::Disabled);
        assert_eq!(map_state(0), RadioState::Unknown);
        assert_eq!(map_state(9), RadioState::Unknown);
        assert!(info(RadioState::Off, true).can_toggle);
        assert!(!info(RadioState::Off, false).can_toggle, "access denied");
        assert!(!info(RadioState::Disabled, true).can_toggle);
        assert!(!info(RadioState::Unknown, true).can_toggle);
    }

    #[test]
    fn merges_multiple_adapters() {
        let off = info(RadioState::Off, true);
        let on = info(RadioState::On, true);
        assert_eq!(merge(&[off, on]), Some(on));
        assert_eq!(merge(&[off]), Some(off));
        assert_eq!(merge(&[]), None);
    }
}
