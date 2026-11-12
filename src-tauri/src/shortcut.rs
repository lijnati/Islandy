//! Global toggle shortcut (official `tauri-plugin-global-shortcut`).
//!
//! Registered from Rust only; the frontend gets no plugin permissions.
//! If another application already owns the combination, registration fails
//! and we report it instead of overriding anything.

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::signals::{self, IslandSignal};

/// Default toggle. Override with the `ISLANDY_TOGGLE_SHORTCUT` environment
/// variable (e.g. `Ctrl+Shift+I`) until a settings UI exists.
pub const DEFAULT_TOGGLE: &str = "Ctrl+Alt+Space";

/// Mirrored by `ShortcutStatus` in `src/services/types.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub accelerator: String,
    pub registered: bool,
    pub error: Option<String>,
}

fn configured_accelerator() -> String {
    std::env::var("ISLANDY_TOGGLE_SHORTCUT")
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_TOGGLE.to_owned())
}

pub fn register<R: Runtime>(app: &AppHandle<R>) -> ShortcutStatus {
    let accelerator = configured_accelerator();
    let result =
        app.global_shortcut()
            .on_shortcut(accelerator.as_str(), |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    signals::emit(app, IslandSignal::Toggle);
                }
            });
    match result {
        Ok(()) => ShortcutStatus {
            accelerator,
            registered: true,
            error: None,
        },
        Err(e) => {
            eprintln!("[islandy] could not register shortcut {accelerator}: {e}");
            ShortcutStatus {
                accelerator,
                registered: false,
                error: Some(e.to_string()),
            }
        }
    }
}
