//! The settings window: a normal, decorated, resizable window created on
//! demand and destroyed when closed (no WebView kept in memory while it
//! isn't used). It is separate from the compact island dashboard.

use serde::{Deserialize, Serialize};
use tauri::{window::Color, AppHandle, Manager, Runtime, Theme, WebviewUrl, WebviewWindowBuilder};

use crate::{error::Result, services::settings::SettingsService};

pub const SETTINGS_LABEL: &str = "settings";

/// Pages of the settings window. Mirrored by `SettingsPageId` in
/// `src/features/settings/settings-schema.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingsPage {
    #[default]
    General,
    Appearance,
    Island,
    Widgets,
    Integrations,
    Notifications,
    Privacy,
    About,
    /// First-run welcome (not in the sidebar).
    Welcome,
}

impl SettingsPage {
    pub fn slug(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Island => "island",
            Self::Widgets => "widgets",
            Self::Integrations => "integrations",
            Self::Notifications => "notifications",
            Self::Privacy => "privacy",
            Self::About => "about",
            Self::Welcome => "welcome",
        }
    }
}

/// Opens the settings window from an event handler on the main thread (tray
/// menu, second launch). The window is created on a worker thread: Tauri
/// hands the creation to the event loop, which must not be blocked by the
/// caller while the new WebView2 initializes.
pub fn open_detached<R: Runtime>(app: &AppHandle<R>, page: SettingsPage) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = open(&app, page) {
            crate::log_error!("settings", "could not open settings: {e}");
        }
    });
}

/// Opens (or focuses) the settings window on `page`. Never call it
/// synchronously on the main thread (use `open_detached` there).
pub fn open<R: Runtime>(app: &AppHandle<R>, page: SettingsPage) -> Result<()> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.unminimize();
        window.show()?;
        window.set_focus()?;
        app.state::<SettingsService>()
            .navigate(SETTINGS_LABEL, page.slug());
        return Ok(());
    }
    let url = WebviewUrl::App(format!("settings.html#{}", page.slug()).into());
    WebviewWindowBuilder::new(app, SETTINGS_LABEL, url)
        .title("Islandy Settings")
        .inner_size(880.0, 620.0)
        .min_inner_size(720.0, 500.0)
        .resizable(true)
        .maximizable(false)
        .theme(Some(Theme::Dark))
        // Matches the page background, so there is no white flash.
        .background_color(Color(14, 14, 15, 255))
        .center()
        .focused(true)
        .build()?;
    crate::log_debug!("settings", "settings window opened on {}", page.slug());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_round_trip_through_their_slugs() {
        for page in [
            SettingsPage::General,
            SettingsPage::Appearance,
            SettingsPage::Island,
            SettingsPage::Widgets,
            SettingsPage::Integrations,
            SettingsPage::Notifications,
            SettingsPage::Privacy,
            SettingsPage::About,
            SettingsPage::Welcome,
        ] {
            let json = serde_json::to_value(page).unwrap();
            assert_eq!(json, page.slug());
            assert_eq!(serde_json::from_value::<SettingsPage>(json).unwrap(), page);
        }
        assert!(serde_json::from_value::<SettingsPage>("../etc".into()).is_err());
    }
}
