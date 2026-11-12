//! Application lifecycle: builder configuration, startup, tray and shutdown.

use std::time::Duration;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    App, AppHandle, Manager, RunEvent, Runtime, WindowEvent,
};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{
    commands,
    services::media::MediaService,
    shortcut,
    signals::{self, IslandSignal, SignalHub},
    window::{IslandWindow, WindowState},
};

/// How long a focus loss must last before the island treats it as a click
/// outside.
const FOCUS_LOSS_DEBOUNCE: Duration = Duration::from_millis(150);

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(WindowState::new())
        .manage(SignalHub::default())
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
        ])
        .setup(|app| {
            setup_tray(app)?;
            // Media runs on its own worker thread; starting it never blocks.
            app.manage(MediaService::start()?);
            let status = shortcut::register(app.handle());
            app.manage(status);
            let island = IslandWindow::get(app.handle())?;
            // The window starts hidden (`visible: false`) so it never flashes
            // at the wrong size or position.
            island.initialize()?;
            island.show()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(debug_assertions)]
            if let WindowEvent::Focused(f) = event {
                eprintln!("[islandy] window focused = {f}");
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
                            let focused = w.is_focused().unwrap_or(false);
                            #[cfg(debug_assertions)]
                            eprintln!("[islandy] focus loss settled; focused now = {focused}");
                            if !focused {
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
            // Release the system-wide hotkey promptly on quit.
            let _ = app.global_shortcut().unregister_all();
            // Unregister WinRT media event handlers (bounded wait).
            if let Some(media) = app.try_state::<MediaService>() {
                media.shutdown();
            }
        }
    });
}

fn refresh_placement<R: Runtime>(app: &AppHandle<R>) {
    match IslandWindow::get(app).and_then(|island| island.refresh()) {
        Ok(_) => signals::emit(app, IslandSignal::LayoutsChanged),
        Err(e) => eprintln!("[islandy] failed to reposition after DPI change: {e}"),
    }
}

/// The island has no taskbar entry and only takes focus on demand, so the
/// tray icon is the user's way to quit.
fn setup_tray(app: &App) -> tauri::Result<()> {
    let peek = MenuItem::with_id(app, "demo-peek", "Preview peek (demo)", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Islandy", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&peek, &separator, &quit])?;
    let mut tray = TrayIconBuilder::with_id("islandy")
        .tooltip("Islandy")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            "demo-peek" => signals::emit(app, IslandSignal::DemoPeek),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
