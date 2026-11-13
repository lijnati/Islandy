//! Persistent, typed preferences shared by every Islandy window.
//!
//! [`SettingsService`] owns the current [`Settings`], persists them
//! atomically on every change and pushes the normalized result to every
//! connected window (island and settings), so the UI updates immediately.
//! Native side effects (window placement, logging) are applied by the caller
//! in `app.rs`, which compares the previous and new values.

pub mod models;
pub mod storage;

use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

use serde::Serialize;
use tauri::ipc::Channel;

pub use models::Settings;
use storage::LoadStatus;

/// What the UI receives: the settings plus how they were loaded and whether
/// saving works (a read-only profile keeps settings in memory only).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsState {
    pub settings: Settings,
    pub load_status: LoadStatus,
    pub save_error: bool,
}

/// Pushed to connected windows. Mirrored by `SettingsPush` in
/// `src/services/types.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SettingsPush {
    State {
        state: SettingsState,
    },
    /// Show a page (settings window only).
    Navigate {
        page: &'static str,
    },
}

struct Inner {
    state: SettingsState,
    channels: Vec<(String, Channel<SettingsPush>)>,
}

pub struct SettingsService {
    dir: Option<PathBuf>,
    inner: Mutex<Inner>,
}

impl SettingsService {
    /// Loads `settings.json` (or defaults). Never fails: without a data
    /// directory the settings simply live in memory.
    pub fn load() -> Self {
        Self::load_from(crate::services::paths::data_dir())
    }

    pub fn load_from(dir: Option<PathBuf>) -> Self {
        let loaded = dir.as_deref().map(storage::load);
        let (settings, load_status) = loaded.map_or_else(
            || (Settings::default(), LoadStatus::Fresh),
            |l| (l.settings, l.status),
        );
        if matches!(load_status, LoadStatus::Recovered) {
            crate::log_warn!(
                "settings",
                "settings.json was unreadable; it was moved to {} and defaults are used",
                storage::CORRUPT_FILE
            );
        }
        Self {
            dir,
            inner: Mutex::new(Inner {
                state: SettingsState {
                    settings,
                    load_status,
                    save_error: false,
                },
                channels: Vec::new(),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self) -> Settings {
        self.lock().state.settings.clone()
    }

    pub fn state(&self) -> SettingsState {
        self.lock().state.clone()
    }

    /// Registers a window's channel (replacing that window's previous one)
    /// and returns the current state.
    pub fn connect(&self, window: &str, channel: Channel<SettingsPush>) -> SettingsState {
        let mut inner = self.lock();
        inner.channels.retain(|(label, _)| label != window);
        inner.channels.push((window.to_owned(), channel));
        inner.state.clone()
    }

    /// Normalizes, persists and broadcasts `next`. Returns the previous and
    /// the stored settings.
    pub fn update(&self, next: Settings) -> (Settings, SettingsState) {
        let next = next.normalize();
        let mut inner = self.lock();
        let previous = inner.state.settings.clone();
        if previous == next && !inner.state.save_error {
            return (previous, inner.state.clone());
        }
        let save_error = match self.dir.as_deref() {
            Some(dir) => match storage::save(dir, &next) {
                Ok(()) => false,
                Err(e) => {
                    crate::log_error!("settings", "could not save settings: {e}");
                    true
                }
            },
            None => true,
        };
        inner.state.settings = next;
        inner.state.save_error = save_error;
        let state = inner.state.clone();
        // A closed window's channel fails; drop it.
        let push = SettingsPush::State {
            state: state.clone(),
        };
        inner
            .channels
            .retain(|(_, channel)| channel.send(push.clone()).is_ok());
        (previous, state)
    }

    /// Asks the window `label` to show `page`.
    pub fn navigate(&self, label: &str, page: &'static str) {
        let inner = self.lock();
        for (_, channel) in inner.channels.iter().filter(|(l, _)| l == label) {
            let _ = channel.send(SettingsPush::Navigate { page });
        }
    }

    /// Writes the current settings if no file exists yet (first launch), so
    /// the next launch isn't treated as a first run again.
    pub fn ensure_saved(&self) {
        let mut inner = self.lock();
        if let (Some(dir), LoadStatus::Fresh) = (self.dir.as_deref(), inner.state.load_status) {
            match storage::save(dir, &inner.state.settings) {
                Ok(()) => inner.state.load_status = LoadStatus::Loaded,
                Err(e) => {
                    crate::log_warn!("settings", "could not create settings.json: {e}");
                    inner.state.save_error = true;
                }
            }
        }
    }

    /// Applies `f` to a copy of the current settings and stores the result.
    pub fn modify(&self, f: impl FnOnce(&mut Settings)) -> (Settings, SettingsState) {
        let mut next = self.get();
        f(&mut next);
        self.update(next)
    }

    /// Back to defaults. Whether the welcome was seen is not a preference,
    /// so a reset doesn't bring it back.
    pub fn reset(&self) -> (Settings, SettingsState) {
        let welcomed = self.get().onboarding.welcomed;
        let mut defaults = Settings::default();
        defaults.onboarding.welcomed = welcomed;
        self.update(defaults)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "islandy-settings-svc-{name}-{}-{}",
            std::process::id(),
            crate::services::activity::now_ms()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn updates_are_normalized_persisted_and_survive_a_restart() {
        let dir = temp_dir("persist");
        let service = SettingsService::load_from(Some(dir.clone()));
        assert_eq!(service.state().load_status, LoadStatus::Fresh);
        let (_, state) = service.modify(|s| {
            s.interaction.hover_delay_ms = 9_999;
            s.island.top_margin = 20;
        });
        assert_eq!(state.settings.interaction.hover_delay_ms, 500);
        assert!(!state.save_error);
        let restarted = SettingsService::load_from(Some(dir));
        assert_eq!(restarted.state().load_status, LoadStatus::Loaded);
        assert_eq!(restarted.get(), state.settings);
    }

    #[test]
    fn reset_restores_defaults_on_disk() {
        let dir = temp_dir("reset");
        let service = SettingsService::load_from(Some(dir.clone()));
        service.modify(|s| s.appearance.opacity = 80);
        let (previous, state) = service.reset();
        assert_eq!(previous.appearance.opacity, 80);
        assert_eq!(state.settings, Settings::default());
        assert_eq!(
            SettingsService::load_from(Some(dir)).get(),
            Settings::default()
        );
    }

    #[test]
    fn reset_keeps_the_welcome_seen() {
        let service = SettingsService::load_from(Some(temp_dir("reset-welcome")));
        service.modify(|s| s.onboarding.welcomed = true);
        let (_, state) = service.reset();
        assert!(state.settings.onboarding.welcomed);
        assert_eq!(state.settings.appearance, Settings::default().appearance);
    }

    #[test]
    fn without_a_data_directory_settings_live_in_memory() {
        let service = SettingsService::load_from(None);
        let (_, state) = service.modify(|s| s.island.offset_x = 10);
        assert!(state.save_error);
        assert_eq!(service.get().island.offset_x, 10);
    }

    #[test]
    fn unchanged_updates_do_not_write() {
        let dir = temp_dir("nochange");
        let service = SettingsService::load_from(Some(dir.clone()));
        service.update(Settings::default());
        assert!(
            !storage::path(&dir).exists(),
            "defaults equal to the current state are not written"
        );
    }
}
