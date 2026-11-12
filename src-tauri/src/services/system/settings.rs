//! Opening Windows Settings pages.
//!
//! The frontend sends an enum, never a URI: only the fixed `ms-settings:`
//! pages below can be opened (via `ShellExecuteW`, no shell, no arguments).

use serde::Deserialize;

use crate::{error::Result, platform};

/// Mirrored by `SettingsPage` in `src/services/types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingsPage {
    /// Settings home.
    Home,
    Wifi,
    Network,
    Bluetooth,
    Sound,
    Display,
    Battery,
}

impl SettingsPage {
    #[cfg(test)]
    pub const ALL: [Self; 7] = [
        Self::Home,
        Self::Wifi,
        Self::Network,
        Self::Bluetooth,
        Self::Sound,
        Self::Display,
        Self::Battery,
    ];

    /// Documented `ms-settings:` URIs (Windows 10 and 11).
    pub const fn uri(self) -> &'static str {
        match self {
            Self::Home => "ms-settings:",
            Self::Wifi => "ms-settings:network-wifi",
            Self::Network => "ms-settings:network-status",
            Self::Bluetooth => "ms-settings:bluetooth",
            Self::Sound => "ms-settings:sound",
            Self::Display => "ms-settings:display",
            // "Battery saver" on Windows 10; "Power & battery" on Windows 11.
            Self::Battery => "ms-settings:batterysaver",
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
        for bad in [
            "\"ms-settings:privacy\"",
            "\"https://example.com\"",
            "\"cmd.exe\"",
            "\"Wifi\"",
            "\"\"",
        ] {
            assert!(serde_json::from_str::<SettingsPage>(bad).is_err(), "{bad}");
        }
        for page in SettingsPage::ALL {
            let uri = page.uri();
            assert!(uri.starts_with("ms-settings:"), "{uri}");
            assert!(
                uri["ms-settings:".len()..]
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'-'),
                "{uri}"
            );
        }
    }
}
