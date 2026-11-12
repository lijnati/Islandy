//! The IPC surface. Each command must also be listed in `build.rs` (app
//! manifest) and granted in `capabilities/island.json`, and have a typed
//! wrapper in `src/services/native.ts`.
//!
//! Arguments are typed and validated: sizes are checked numbers, settings
//! pages are an enum. No command accepts paths, URLs or command lines.

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager, Runtime};

use crate::{
    error::{IslandyError, Result},
    services::{
        app_info::{self, AppInfo},
        system_settings::{self, SettingsPage},
    },
    shortcut::ShortcutStatus,
    signals::{self, IslandSignal, SignalHub},
    window::{
        geometry::{Layouts, LogicalSize},
        FrameStrategy, IslandWindow,
    },
};

#[tauri::command]
pub fn get_app_info<R: Runtime>(app: AppHandle<R>) -> AppInfo {
    let info = app_info::collect(&app);
    #[cfg(debug_assertions)]
    eprintln!("[islandy] get_app_info -> {info:?}");
    info
}

/// Mirrored by `IslandConnection` in `src/services/types.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IslandConnection {
    pub shortcut: ShortcutStatus,
    pub frame_strategy: FrameStrategy,
}

/// Registers the frontend's signal channel and reports native status.
#[tauri::command]
pub fn connect_island<R: Runtime>(
    app: AppHandle<R>,
    channel: Channel<IslandSignal>,
) -> Result<IslandConnection> {
    app.state::<SignalHub>().connect(channel);
    Ok(IslandConnection {
        shortcut: app.state::<ShortcutStatus>().inner().clone(),
        frame_strategy: IslandWindow::get(&app)?.strategy(),
    })
}

/// Layout sizes (logical px) fitted to the island's monitor.
#[tauri::command]
pub fn get_island_layouts<R: Runtime>(app: AppHandle<R>) -> Result<Layouts> {
    IslandWindow::get(&app)?.layouts()
}

/// Applies the visible frame size (logical px). Called by the frontend's
/// window coordinator only at transition boundaries, never per frame.
#[tauri::command]
pub fn set_island_frame<R: Runtime>(app: AppHandle<R>, width: f64, height: f64) -> Result<()> {
    let frame = LogicalSize::validated(width, height).ok_or(IslandyError::InvalidArgument(
        "frame size must be finite and positive",
    ))?;
    if IslandWindow::get(&app)?.set_frame(frame)? {
        signals::emit(&app, IslandSignal::LayoutsChanged);
    }
    Ok(())
}

/// Lets the island take keyboard focus (after an explicit user action) or
/// returns it to its non-activating default.
#[tauri::command]
pub fn set_island_interactive<R: Runtime>(app: AppHandle<R>, interactive: bool) -> Result<()> {
    #[cfg(debug_assertions)]
    eprintln!("[islandy] set_island_interactive({interactive})");
    IslandWindow::get(&app)?.set_interactive(interactive)
}

/// Opens one of a fixed set of Windows Settings pages.
#[tauri::command]
pub fn open_system_settings(page: SettingsPage) -> Result<()> {
    system_settings::open(page)
}
