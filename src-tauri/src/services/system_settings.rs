//! Opening Windows Settings pages for the quick controls.
//!
//! The frontend sends an enum, never a URI: only these fixed pages can be
//! opened. Real toggles (Wi-Fi, Bluetooth, volume) are Phase 3.

use serde::Deserialize;

use crate::{error::Result, platform};

/// Mirrored by `SettingsPage` in `src/services/types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingsPage {
    Wifi,
    Bluetooth,
    Sound,
    Display,
}

impl SettingsPage {
    pub const fn uri(self) -> &'static str {
        match self {
            Self::Wifi => "ms-settings:network-wifi",
            Self::Bluetooth => "ms-settings:bluetooth",
            Self::Sound => "ms-settings:sound",
            Self::Display => "ms-settings:display",
        }
    }
}

pub fn open(page: SettingsPage) -> Result<()> {
    platform::open_settings_uri(page.uri())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_known_pages() {
        let page: SettingsPage = serde_json::from_str("\"bluetooth\"").unwrap();
        assert_eq!(page.uri(), "ms-settings:bluetooth");
        assert!(serde_json::from_str::<SettingsPage>("\"ms-settings:privacy\"").is_err());
        for page in [
            SettingsPage::Wifi,
            SettingsPage::Bluetooth,
            SettingsPage::Sound,
            SettingsPage::Display,
        ] {
            assert!(page.uri().starts_with("ms-settings:"));
        }
    }
}
