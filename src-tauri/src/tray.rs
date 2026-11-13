//! The notification-area (tray) icon: the always-available way to reach
//! Islandy, including when the island is hidden or no shortcut works.
//!
//! - Left click: show the island (if hidden) and open the dashboard.
//! - Right click: the menu (Show/Hide, Settings, Quiet mode, Launch at
//!   startup, About, Check for updates / Restart to update, Quit).
//!
//! One icon per process, created once in `setup` (single-instance makes it
//! one per user session). The `tray-icon` crate re-adds it when Explorer
//! restarts (`TaskbarCreated`). Quit exits the event loop, which runs every
//! service's shutdown (see `app.rs`).

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, Runtime,
};

use crate::{
    services::{
        activity::{ActivityCommand, ActivityService},
        settings::SettingsService,
        startup,
        updates::{UpdatePhase, UpdateService, UpdateStatus},
    },
    settings_window::{self, SettingsPage},
    window::IslandWindow,
};

pub const TRAY_ID: &str = "islandy";

/// Menu items whose text or check state follows the app state.
pub struct TrayMenu<R: Runtime> {
    visibility: MenuItem<R>,
    quiet: CheckMenuItem<R>,
    startup: CheckMenuItem<R>,
    update: MenuItem<R>,
}

/// Menu entries, in order (pure, so the structure is unit-tested).
pub const ITEMS: &[(&str, &str)] = &[
    ("visibility", "Hide Islandy"),
    ("settings", "Settings…"),
    ("-", ""),
    ("quiet", "Quiet mode"),
    ("startup", "Launch at startup"),
    ("-", ""),
    ("about", "About Islandy"),
    ("update", "Check for updates"),
    ("quit", "Quit Islandy"),
];

/// The update entry's text, and whether it can be clicked.
pub fn update_label(status: &UpdateStatus) -> (String, bool) {
    let version = status.version.as_deref().unwrap_or("");
    match status.phase {
        UpdatePhase::Ready => (format!("Restart to update to {version}"), true),
        UpdatePhase::Available => (format!("Install update {version}"), true),
        UpdatePhase::Downloading => ("Downloading update…".into(), false),
        UpdatePhase::Checking => ("Checking for updates…".into(), false),
        UpdatePhase::Installing => ("Installing update…".into(), false),
        _ => ("Check for updates".into(), true),
    }
}

pub fn visibility_label(visible: bool) -> &'static str {
    if visible {
        "Hide Islandy"
    } else {
        "Show Islandy"
    }
}

pub fn setup<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let visibility = MenuItem::with_id(app, "visibility", ITEMS[0].1, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", ITEMS[1].1, true, None::<&str>)?;
    let quiet = CheckMenuItem::with_id(app, "quiet", ITEMS[3].1, true, false, None::<&str>)?;
    let startup_item =
        CheckMenuItem::with_id(app, "startup", ITEMS[4].1, true, false, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", ITEMS[6].1, true, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", ITEMS[7].1, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", ITEMS[8].1, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &visibility,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quiet,
            &startup_item,
            &PredefinedMenuItem::separator(app)?,
            &about,
            &update,
            &quit,
        ],
    )?;
    app.manage(TrayMenu {
        visibility,
        quiet,
        startup: startup_item,
        update,
    });

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Islandy")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::app::show_and_open(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    sync(app.handle());
    Ok(())
}

fn on_menu<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        "visibility" => {
            let hidden = IslandWindow::get(app).is_ok_and(|w| w.is_user_hidden());
            crate::app::set_island_visible(app, hidden);
        }
        "settings" => open_settings(app, SettingsPage::General),
        "about" => open_settings(app, SettingsPage::About),
        "quiet" => {
            if let Some(activity) = app.try_state::<ActivityService>() {
                let quiet = !activity.snapshot().settings.quiet;
                activity.command(ActivityCommand::SetQuiet(quiet));
                set_quiet_checked(app, quiet);
            }
        }
        "startup" => {
            let wanted = !startup::status().enabled;
            if let Err(e) = startup::set(wanted) {
                crate::log_warn!("tray", "launch at startup could not be changed: {e}");
            }
            sync(app);
        }
        "update" => {
            let ready = app.try_state::<UpdateService>().is_some_and(|u| {
                matches!(
                    u.status().phase,
                    UpdatePhase::Ready | UpdatePhase::Available
                )
            });
            let app = app.clone();
            if !ready {
                // Show the outcome where it is explained.
                open_settings(&app, SettingsPage::About);
            }
            tauri::async_runtime::spawn(async move {
                let Some(updates) = app.try_state::<UpdateService>() else {
                    return;
                };
                if ready {
                    updates.install(&app).await;
                } else {
                    let download = app
                        .try_state::<SettingsService>()
                        .is_some_and(|s| s.get().updates.auto_install);
                    sync_update(&app);
                    updates.check(&app, download).await;
                }
                sync_update(&app);
            });
        }
        "quit" => {
            crate::log_info!("app", "quit from the tray");
            app.exit(0);
        }
        _ => {}
    }
}

fn open_settings<R: Runtime>(app: &AppHandle<R>, page: SettingsPage) {
    settings_window::open_detached(app, page);
}

pub fn set_quiet_checked<R: Runtime>(app: &AppHandle<R>, quiet: bool) {
    if let Some(menu) = app.try_state::<TrayMenu<R>>() {
        let _ = menu.quiet.set_checked(quiet);
    }
}

/// Refreshes every dynamic menu entry from the real state.
pub fn sync<R: Runtime>(app: &AppHandle<R>) {
    let Some(menu) = app.try_state::<TrayMenu<R>>() else {
        return;
    };
    let visible = IslandWindow::get(app).is_ok_and(|w| !w.is_user_hidden());
    let _ = menu.visibility.set_text(visibility_label(visible));
    if let Some(activity) = app.try_state::<ActivityService>() {
        let _ = menu.quiet.set_checked(activity.snapshot().settings.quiet);
    }
    let status = startup::status();
    let _ = menu.startup.set_enabled(status.supported);
    let _ = menu.startup.set_checked(status.enabled);
    sync_update(app);
}

/// Refreshes the update entry (called when the update state changes).
pub fn sync_update<R: Runtime>(app: &AppHandle<R>) {
    let (Some(menu), Some(updates)) = (
        app.try_state::<TrayMenu<R>>(),
        app.try_state::<UpdateService>(),
    ) else {
        return;
    };
    let (text, enabled) = update_label(&updates.status());
    let _ = menu.update.set_text(text);
    let _ = menu.update.set_enabled(enabled);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_has_the_required_entries_in_order() {
        let ids: Vec<_> = ITEMS
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| *id != "-")
            .collect();
        assert_eq!(
            ids,
            [
                "visibility",
                "settings",
                "quiet",
                "startup",
                "about",
                "update",
                "quit"
            ]
        );
        assert_eq!(visibility_label(true), "Hide Islandy");
        assert_eq!(visibility_label(false), "Show Islandy");
        assert!(ITEMS
            .iter()
            .any(|(id, text)| *id == "quit" && text.starts_with("Quit")));
    }
}
