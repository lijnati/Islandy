//! Global toggle shortcut (official `tauri-plugin-global-shortcut`).
//!
//! Registered from Rust only; the frontend gets no plugin permissions.
//! The accelerator is a user setting. Changing it registers the new
//! combination *first* and only then releases the old one, so a combination
//! that another application already owns (or Windows reserves) is reported
//! and the previous shortcut keeps working. Nothing is ever overridden.
//! The tray icon is the fallback way in when no shortcut works.

use std::{str::FromStr, sync::Mutex};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub use crate::services::settings::models::DEFAULT_SHORTCUT as DEFAULT_TOGGLE;

/// Mirrored by `ShortcutStatus` in `src/services/types.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub accelerator: String,
    pub registered: bool,
    pub error: Option<String>,
}

/// The registered shortcut (managed by Tauri).
pub struct ShortcutManager {
    status: Mutex<ShortcutStatus>,
    active: Mutex<Option<Shortcut>>,
}

impl ShortcutManager {
    pub fn status(&self) -> ShortcutStatus {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

/// Combinations Windows uses itself (or that would be unusable as a toggle).
const RESERVED: &[&str] = &[
    "Ctrl+Alt+Delete",
    "Ctrl+Shift+Escape",
    "Ctrl+Escape",
    "Alt+Tab",
    "Alt+Shift+Tab",
    "Alt+Escape",
    "Alt+F4",
    "Alt+Space",
    "Win+L",
    "Win+D",
    "Win+Tab",
];

/// A user-facing name for a key code: `KeyA` → `A`, `Digit1` → `1`.
fn key_name(code: Code) -> String {
    let raw = code.to_string();
    raw.strip_prefix("Key")
        .or_else(|| raw.strip_prefix("Digit"))
        .filter(|rest| rest.len() == 1)
        .map(str::to_owned)
        .unwrap_or(raw)
}

/// The canonical text of a shortcut, e.g. `Ctrl+Alt+Space`.
pub fn canonical(shortcut: &Shortcut) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mods = shortcut.mods;
    if mods.contains(Modifiers::CONTROL) {
        parts.push("Ctrl".into());
    }
    if mods.contains(Modifiers::ALT) {
        parts.push("Alt".into());
    }
    if mods.contains(Modifiers::SHIFT) {
        parts.push("Shift".into());
    }
    if mods.contains(Modifiers::SUPER) {
        parts.push("Win".into());
    }
    parts.push(key_name(shortcut.key));
    parts.join("+")
}

/// Parses and checks an accelerator. It needs Ctrl, Alt or Win (Shift alone
/// would capture ordinary typing), a non-modifier key, and must not be a
/// combination Windows reserves.
pub fn validate(accelerator: &str) -> Result<Shortcut, String> {
    let text = accelerator.trim();
    if text.is_empty() || text.len() > 64 {
        return Err("Enter a key combination.".into());
    }
    // `Win` is Islandy's display name for the Super key.
    let parseable = text
        .split('+')
        .map(|p| match p.trim() {
            "Win" | "Windows" => "Super",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+");
    let shortcut = Shortcut::from_str(&parseable)
        .map_err(|_| format!("“{text}” isn't a key combination Islandy understands."))?;
    if !shortcut
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
    {
        return Err("Use Ctrl, Alt or Win with another key.".into());
    }
    if matches!(
        shortcut.key,
        Code::ControlLeft
            | Code::ControlRight
            | Code::AltLeft
            | Code::AltRight
            | Code::ShiftLeft
            | Code::ShiftRight
            | Code::MetaLeft
            | Code::MetaRight
    ) {
        return Err("Add a key that isn't a modifier.".into());
    }
    let name = canonical(&shortcut);
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&name)) {
        return Err(format!("{name} is reserved by Windows."));
    }
    Ok(shortcut)
}

fn register_one<R: Runtime>(app: &AppHandle<R>, shortcut: Shortcut) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::app::toggle_from_shortcut(app);
            }
        })
        .map_err(|e| {
            let text = e.to_string();
            if text.to_lowercase().contains("already") {
                "Another app already uses this shortcut.".to_owned()
            } else {
                format!("Windows refused this shortcut ({text}).")
            }
        })
}

/// Registers the configured shortcut at start-up. An invalid stored value
/// falls back to the default; a conflict is reported, not overridden.
pub fn initialize<R: Runtime>(app: &AppHandle<R>, configured: &str) -> ShortcutManager {
    let (shortcut, mut error) = match validate(configured) {
        Ok(s) => (Some(s), None),
        Err(e) => {
            crate::log_warn!(
                "shortcut",
                "stored shortcut is invalid ({e}); using the default"
            );
            (validate(DEFAULT_TOGGLE).ok(), Some(e))
        }
    };
    let mut status = ShortcutStatus {
        accelerator: shortcut
            .as_ref()
            .map_or_else(|| DEFAULT_TOGGLE.to_owned(), canonical),
        registered: false,
        error: None,
    };
    let mut active = None;
    if let Some(shortcut) = shortcut {
        match register_one(app, shortcut) {
            Ok(()) => {
                status.registered = true;
                active = Some(shortcut);
            }
            Err(e) => {
                crate::log_warn!("shortcut", "could not register {}: {e}", status.accelerator);
                error = Some(e);
            }
        }
    }
    status.error = error;
    ShortcutManager {
        status: Mutex::new(status),
        active: Mutex::new(active),
    }
}

/// Replaces the shortcut. On failure the previous one stays registered and
/// the error is returned (and reported in the status).
pub fn change<R: Runtime>(app: &AppHandle<R>, accelerator: &str) -> Result<ShortcutStatus, String> {
    let manager = app.state::<ShortcutManager>();
    let shortcut = validate(accelerator)?;
    let mut active = manager.active.lock().unwrap_or_else(|e| e.into_inner());
    if *active == Some(shortcut) {
        return Ok(manager.status());
    }
    // New first, then release the old one.
    register_one(app, shortcut)?;
    if let Some(previous) = active.take() {
        let _ = app.global_shortcut().unregister(previous);
    }
    *active = Some(shortcut);
    let status = ShortcutStatus {
        accelerator: canonical(&shortcut),
        registered: true,
        error: None,
    };
    *manager.status.lock().unwrap_or_else(|e| e.into_inner()) = status.clone();
    crate::log_info!(
        "shortcut",
        "toggle shortcut changed to {}",
        status.accelerator
    );
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_canonicalizes_combinations() {
        let ok = |s: &str| canonical(&validate(s).unwrap());
        assert_eq!(ok("Ctrl+Alt+Space"), "Ctrl+Alt+Space");
        assert_eq!(ok("alt+ctrl+space"), "Ctrl+Alt+Space");
        assert_eq!(ok("Ctrl+Shift+I"), "Ctrl+Shift+I");
        assert_eq!(ok("Win+Alt+K"), "Alt+Win+K");
        assert_eq!(ok("Ctrl+Alt+1"), "Ctrl+Alt+1");
        assert_eq!(ok("Alt+F9"), "Alt+F9");
    }

    #[test]
    fn rejects_unsafe_or_meaningless_combinations() {
        for bad in [
            "",
            "Space",
            "A",
            "Shift+A",
            "Ctrl+Alt+Delete",
            "Alt+Tab",
            "alt+f4",
            "Win+L",
            "Ctrl+Banana",
            "Ctrl+",
        ] {
            assert!(validate(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(validate(&"Ctrl+".repeat(30)).is_err());
    }

    #[test]
    fn the_default_is_valid() {
        assert_eq!(
            canonical(&validate(DEFAULT_TOGGLE).unwrap()),
            DEFAULT_TOGGLE
        );
    }
}
