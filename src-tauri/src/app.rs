//! Application lifecycle: builder configuration, startup, tray and shutdown.

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    App, Manager, WindowEvent,
};

use crate::{commands, window::IslandWindow};

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::get_app_info])
        .setup(|app| {
            setup_tray(app)?;
            let island = IslandWindow::get(app.handle())?;
            // The window starts hidden (`visible: false`) so it never flashes
            // at the wrong size or position.
            island.initialize()?;
            island.show()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::ScaleFactorChanged { .. } = event {
                if let Ok(island) = IslandWindow::get(window.app_handle()) {
                    if let Err(e) = island.reposition() {
                        eprintln!("[islandy] failed to reposition after DPI change: {e}");
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Islandy");
}

/// The island has no taskbar entry and never takes focus, so the tray icon is
/// the user's way to quit.
fn setup_tray(app: &App) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit Islandy", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit])?;
    let mut tray = TrayIconBuilder::with_id("islandy")
        .tooltip("Islandy")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            if event.id() == "quit" {
                app.exit(0);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
