//! Application lifecycle: builder configuration, startup, tray, background
//! watcher and shutdown.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use tauri::{AppHandle, Manager, RunEvent, Runtime, WindowEvent};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{
    commands, platform,
    services::{
        activity::ActivityService,
        diagnostics::log,
        media::MediaService,
        paths,
        settings::{storage::LoadStatus, Settings, SettingsService},
        startup,
        system::SystemService,
    },
    settings_window::{self, SettingsPage},
    shortcut,
    signals::{self, IslandSignal, SignalHub},
    tray,
    window::{IslandWindow, PlacementConfig, WindowState, ISLAND_LABEL},
};

/// How long a focus loss must last before the island treats it as a click
/// outside.
const FOCUS_LOSS_DEBOUNCE: Duration = Duration::from_millis(150);

/// How often the watcher re-checks monitor geometry and fullscreen apps.
/// Each check is a handful of cheap Win32 calls on the main thread.
const WATCH_INTERVAL: Duration = Duration::from_secs(2);

/// Facts about this launch (managed by Tauri).
pub struct LaunchInfo {
    pub at_login: bool,
    pub first_run: bool,
}

/// Stops the watcher thread on exit.
struct WatcherStop(Arc<AtomicBool>);

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let at_login = startup::launched_at_login(&args);

    let data_dir = paths::data_dir();
    let settings = SettingsService::load();
    let initial = settings.get();
    log::init(
        data_dir.as_ref().map(|d| d.join("logs")),
        initial.general.verbose_logging,
    );
    crate::log_info!(
        "app",
        "Islandy {} starting{}",
        env!("CARGO_PKG_VERSION"),
        if at_login { " at sign-in" } else { "" }
    );
    let first_run = !at_login && settings.state().load_status == LoadStatus::Fresh;
    settings.ensure_saved();
    let start_hidden = at_login && initial.general.start_hidden_at_login;

    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch hands over and exits
        // before anything else (tray icon, receiver, shortcut) is created.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            crate::log_info!("app", "second launch: showing settings");
            set_island_visible(app, true);
            if let Err(e) = settings_window::open(app, SettingsPage::General) {
                crate::log_warn!("app", "could not open settings: {e}");
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(WindowState::new(
            PlacementConfig::from_settings(&initial.island),
            start_hidden,
        ))
        .manage(SignalHub::default())
        .manage(settings)
        .manage(LaunchInfo {
            at_login,
            first_run,
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::connect_island,
            commands::get_island_layouts,
            commands::set_island_frame,
            commands::set_island_interactive,
            commands::open_system_settings,
            commands::connect_media,
            commands::get_media_snapshot,
            commands::select_media_session,
            commands::media_control,
            commands::media_seek,
            commands::get_media_artwork,
            commands::connect_system,
            commands::get_system_snapshot,
            commands::set_system_focus,
            commands::set_master_volume,
            commands::set_audio_muted,
            commands::set_radio_state,
            commands::set_display_brightness,
            commands::connect_activity,
            commands::get_activity_snapshot,
            commands::dismiss_activity,
            commands::clear_activity_history,
            commands::set_quiet_mode,
            commands::set_activity_source,
            commands::set_activity_settings,
            commands::get_claude_integration,
            commands::install_claude_integration,
            commands::remove_claude_integration,
            commands::test_local_receiver,
            commands::connect_settings,
            commands::update_settings,
            commands::reset_settings,
            commands::set_toggle_shortcut,
            commands::get_shortcut_status,
            commands::get_startup_status,
            commands::set_launch_at_login,
            commands::list_monitors,
            commands::preview_island_offset,
            commands::set_island_visible,
            commands::open_settings_window,
            commands::open_islandy_folder,
            commands::get_storage_info,
            commands::get_diagnostics,
        ])
        .setup(move |app| {
            // Media runs on its own worker thread; starting it never blocks.
            app.manage(MediaService::start()?);
            // Audio, power, network and display run on their own workers.
            let system = SystemService::start()?;
            // Activity: the local receiver, Claude Code sessions and history.
            // System alerts flow into it, so it decides on their peeks too.
            let activity = ActivityService::start()?;
            system.observe_alerts(activity.alert_observer());
            app.manage(system);
            app.manage(activity);
            let shortcut = shortcut::initialize(app.handle(), &initial.general.toggle_shortcut);
            app.manage(shortcut);
            tray::setup(app)?;
            startup::repair();
            let island = IslandWindow::get(app.handle())?;
            // The window starts hidden (`visible: false`) so it never flashes
            // at the wrong size or position.
            island.initialize()?;
            island.show()?;
            let stop = Arc::new(AtomicBool::new(false));
            spawn_watcher(app.handle().clone(), stop.clone());
            app.manage(WatcherStop(stop));
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != ISLAND_LABEL {
                return;
            }
            match event {
                WindowEvent::ScaleFactorChanged { .. } => {
                    // Deferred to the next loop iteration: we may be inside a
                    // SetWindowPos call that triggered WM_DPICHANGED.
                    let app = window.app_handle().clone();
                    let _ = window
                        .app_handle()
                        .run_on_main_thread(move || refresh_placement(&app));
                }
                WindowEvent::Focused(false) => {
                    // Debounced: changing focusability can blur the window for
                    // an instant (seen on X11). Report a real loss only.
                    let window = window.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(FOCUS_LOSS_DEBOUNCE);
                        let w = window.clone();
                        let _ = window.run_on_main_thread(move || {
                            if !w.is_focused().unwrap_or(false) {
                                signals::emit(w.app_handle(), IslandSignal::FocusLost);
                            }
                        });
                    });
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Islandy");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            shutdown(app);
        }
    });
}

/// Stops every background service in order (bounded waits). Runs once, when
/// the event loop exits (Quit from the tray, or Windows shutting down).
fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    crate::log_info!("app", "shutting down");
    if let Some(stop) = app.try_state::<WatcherStop>() {
        stop.0.store(true, Ordering::Relaxed);
    }
    // Release the system-wide hotkey promptly on quit.
    let _ = app.global_shortcut().unregister_all();
    // Unregister WinRT media event handlers (bounded wait).
    if let Some(media) = app.try_state::<MediaService>() {
        media.shutdown();
    }
    // Unregister Core Audio / WinRT callbacks (bounded waits).
    if let Some(system) = app.try_state::<SystemService>() {
        system.shutdown();
    }
    // Stop the receiver (removing its connection file) and flush history.
    if let Some(activity) = app.try_state::<ActivityService>() {
        activity.shutdown();
    }
    crate::log_info!("app", "stopped");
}

fn refresh_placement<R: Runtime>(app: &AppHandle<R>) {
    match IslandWindow::get(app).and_then(|island| island.refresh()) {
        Ok(_) => signals::emit(app, IslandSignal::LayoutsChanged),
        Err(e) => crate::log_warn!("window", "failed to reposition after DPI change: {e}"),
    }
}

/// The global shortcut: shows a hidden island, then toggles the dashboard.
pub fn toggle_from_shortcut<R: Runtime>(app: &AppHandle<R>) {
    if IslandWindow::get(app).is_ok_and(|w| w.is_user_hidden()) {
        set_island_visible(app, true);
    }
    signals::emit(app, IslandSignal::Toggle);
}

/// Tray left click: show the island if hidden and open the dashboard.
pub fn show_and_open<R: Runtime>(app: &AppHandle<R>) {
    set_island_visible(app, true);
    signals::emit(app, IslandSignal::Open);
}

/// Hides the island ("Hide Islandy": the app keeps running in the tray) or
/// shows it again.
pub fn set_island_visible<R: Runtime>(app: &AppHandle<R>, visible: bool) {
    if !visible {
        // Collapse first, so it reappears as the resting pill.
        signals::emit(app, IslandSignal::Hidden);
    }
    match IslandWindow::get(app).and_then(|w| w.set_user_visible(visible)) {
        Ok(_) => crate::log_info!(
            "window",
            "island {}",
            if visible { "shown" } else { "hidden" }
        ),
        Err(e) => crate::log_warn!("window", "could not change island visibility: {e}"),
    }
    tray::sync(app);
}

/// Applies the native side effects of a settings change.
pub fn apply_settings<R: Runtime>(app: &AppHandle<R>, previous: &Settings, next: &Settings) {
    if previous.island != next.island {
        let config = PlacementConfig::from_settings(&next.island);
        match IslandWindow::get(app).and_then(|w| w.configure(config)) {
            Ok(()) => signals::emit(app, IslandSignal::LayoutsChanged),
            Err(e) => crate::log_warn!("window", "could not apply placement: {e}"),
        }
    }
    if previous.general.verbose_logging != next.general.verbose_logging {
        log::set_verbose(next.general.verbose_logging);
        crate::log_info!(
            "app",
            "verbose logging {}",
            if next.general.verbose_logging {
                "on"
            } else {
                "off"
            }
        );
    }
    if !next.general.hide_in_fullscreen {
        if let Ok(w) = IslandWindow::get(app) {
            let _ = w.set_fullscreen_hidden(false);
        }
    }
}

/// Every [`WATCH_INTERVAL`]: re-fit the island if its monitor changed
/// (resolution, scaling, taskbar, monitor (dis)connected, Explorer restart,
/// resume from sleep) and hide it while a fullscreen app covers its monitor.
fn spawn_watcher<R: Runtime>(app: AppHandle<R>, stop: Arc<AtomicBool>) {
    let spawned = std::thread::Builder::new()
        .name("islandy-watcher".into())
        .spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(WATCH_INTERVAL);
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || watch_once(&handle));
            }
        });
    if let Err(e) = spawned {
        crate::log_warn!("window", "monitor watcher unavailable: {e}");
    }
}

fn watch_once<R: Runtime>(app: &AppHandle<R>) {
    let Ok(island) = IslandWindow::get(app) else {
        return;
    };
    match island.refresh_if_changed() {
        Ok(true) => {
            crate::log_info!("window", "monitor changed: {}", island.describe());
            signals::emit(app, IslandSignal::LayoutsChanged);
        }
        Ok(false) => {}
        Err(e) => crate::log_debug!("window", "monitor check failed: {e}"),
    }
    let hide_in_fullscreen = app
        .try_state::<SettingsService>()
        .is_some_and(|s| s.get().general.hide_in_fullscreen);
    let fullscreen = hide_in_fullscreen
        && island
            .monitor_bounds()
            .is_some_and(|bounds| platform::fullscreen_on(bounds, island.window_handle()));
    if let Ok(true) = island.set_fullscreen_hidden(fullscreen) {
        if fullscreen {
            signals::emit(app, IslandSignal::Hidden);
        }
        crate::log_debug!("window", "fullscreen app: island hidden = {fullscreen}");
    }
}
