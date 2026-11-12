//! The IPC surface. Each command must also be listed in `build.rs` (app
//! manifest) and granted in `capabilities/island.json`, and have a typed
//! wrapper in `src/services/native.ts`.

use tauri::{AppHandle, Runtime};

use crate::services::app_info::{self, AppInfo};

#[tauri::command]
pub fn get_app_info<R: Runtime>(app: AppHandle<R>) -> AppInfo {
    let info = app_info::collect(&app);
    #[cfg(debug_assertions)]
    eprintln!("[islandy] get_app_info -> {info:?}");
    info
}
