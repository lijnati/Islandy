//! Automatic updates from the project's GitHub Releases.
//!
//! - **Source:** `latest.json` of the latest GitHub release (see
//!   `plugins.updater` in `tauri.conf.json`). It names the installer and its
//!   minisign signature; the updater refuses anything not signed with the key
//!   whose public half is built into Islandy.
//! - **Checking** (Settings › About › "Check for updates automatically", on by
//!   default): [`FIRST_CHECK_AFTER`] after start, then every [`CHECK_EVERY`].
//!   A check is one HTTPS request to github.com; nothing about the user or
//!   this PC is sent.
//! - **Installing** ("Install updates automatically", on by default): a found
//!   update is downloaded and verified, then installed once the user has been
//!   away for [`INSTALL_WHEN_IDLE_FOR`] (Windows only) or right away with
//!   "Restart to update". Installing runs the NSIS installer in passive mode
//!   (a progress bar, no questions) and restarts Islandy. Settings, history,
//!   Claude Code hooks and launch at startup are kept (the installer's
//!   uninstall cleanup is skipped for updates).
//! - With automatic installation off, Islandy only tells the user (a peek,
//!   the tray menu and Settings › About).

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::services::{
    activity::{now_ms, ActivityService},
    settings::SettingsService,
    system::models::SystemAlert,
};

pub const FIRST_CHECK_AFTER: Duration = Duration::from_secs(45);
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
pub const INSTALL_WHEN_IDLE_FOR: Duration = Duration::from_secs(4 * 60);
const TICK: Duration = Duration::from_secs(15);
/// Release notes longer than this are cut (they are shown as plain text).
const MAX_NOTES: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdatePhase {
    /// Not checked yet in this session.
    Idle,
    Checking,
    UpToDate,
    /// Found; waiting for the user (automatic installation is off).
    Available,
    Downloading,
    /// Downloaded and verified; installs when idle or on request.
    Ready,
    Installing,
    Failed,
}

/// Mirrored by `UpdateStatus` in `src/services/types.ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub phase: UpdatePhase,
    pub current_version: String,
    /// The newer version, once one was found.
    pub version: Option<String>,
    /// Release notes (plain text, possibly Markdown), shortened.
    pub notes: Option<String>,
    /// Download progress, 0–100, while downloading.
    pub progress: Option<u8>,
    pub last_checked_ms: Option<u64>,
    /// User-facing reason of the last failure.
    pub error: Option<String>,
}

struct Inner {
    status: UpdateStatus,
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
    /// Versions already announced with a peek (once per version).
    announced: Option<String>,
    next_check: Instant,
}

pub struct UpdateService {
    inner: Mutex<Inner>,
    busy: AtomicBool,
}

impl Default for UpdateService {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                status: UpdateStatus {
                    phase: UpdatePhase::Idle,
                    current_version: env!("CARGO_PKG_VERSION").to_owned(),
                    version: None,
                    notes: None,
                    progress: None,
                    last_checked_ms: None,
                    error: None,
                },
                update: None,
                bytes: None,
                announced: None,
                next_check: Instant::now() + FIRST_CHECK_AFTER,
            }),
            busy: AtomicBool::new(false),
        }
    }
}

/// Clears `busy` when dropped, so an early return can't leave it set.
struct BusyGuard<'a>(&'a AtomicBool);
impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn short_notes(body: Option<&str>) -> Option<String> {
    let body = body?.trim();
    if body.is_empty() {
        return None;
    }
    let mut notes: String = body
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .take(MAX_NOTES)
        .collect();
    if body.chars().count() > MAX_NOTES {
        notes.push('…');
    }
    Some(notes)
}

/// The updater's error, reduced to a sentence for the UI (no URLs).
fn describe(e: &tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error as E;
    match e {
        E::Reqwest(_) | E::Network(_) => "Couldn't reach the update server.".into(),
        E::ReleaseNotFound => "No published release was found.".into(),
        E::Minisign(_)
        | E::Base64(_)
        | E::SignatureUtf8(_)
        | E::SignedVersionMismatch { .. }
        | E::MissingSignedVersion => {
            "The update's signature didn't verify; it was not installed.".into()
        }
        E::EmptyEndpoints | E::InsecureTransportProtocol => {
            "Updates aren't configured in this build.".into()
        }
        E::TargetNotFound(_) | E::TargetsNotFound(_) => {
            "The latest release has no installer for this PC.".into()
        }
        _ => "The update failed.".into(),
    }
}

impl UpdateService {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn status(&self) -> UpdateStatus {
        self.lock().status.clone()
    }

    fn set(&self, f: impl FnOnce(&mut UpdateStatus)) {
        f(&mut self.lock().status);
    }

    fn fail(&self, message: String) {
        crate::log_warn!("updates", "{message}");
        self.set(|s| {
            s.phase = UpdatePhase::Failed;
            s.progress = None;
            s.error = Some(message);
        });
    }

    /// Looks for a newer release; with `download`, also fetches and verifies
    /// it. Returns at once (with the current status) if a check or download
    /// is already running.
    pub async fn check<R: Runtime>(&self, app: &AppHandle<R>, download: bool) -> UpdateStatus {
        if self.busy.swap(true, Ordering::AcqRel) {
            return self.status();
        }
        let _busy = BusyGuard(&self.busy);
        {
            let inner = self.lock();
            // Already downloaded: nothing to do until it's installed.
            if inner.status.phase == UpdatePhase::Ready {
                return inner.status.clone();
            }
        }
        self.set(|s| {
            s.phase = UpdatePhase::Checking;
            s.error = None;
        });
        let found = match app.updater() {
            Ok(updater) => updater.check().await,
            Err(e) => Err(e),
        };
        {
            let mut inner = self.lock();
            inner.next_check = Instant::now() + CHECK_EVERY;
            inner.status.last_checked_ms = Some(now_ms());
        }
        match found {
            Ok(None) => {
                crate::log_info!("updates", "Islandy is up to date");
                self.set(|s| {
                    s.phase = UpdatePhase::UpToDate;
                    s.version = None;
                    s.notes = None;
                });
            }
            Ok(Some(update)) => {
                crate::log_info!("updates", "update {} is available", update.version);
                {
                    let mut inner = self.lock();
                    inner.status.phase = UpdatePhase::Available;
                    inner.status.version = Some(update.version.clone());
                    inner.status.notes = short_notes(update.body.as_deref());
                    inner.update = Some(update);
                    inner.bytes = None;
                }
                if download {
                    self.download().await;
                }
                self.announce(app);
            }
            Err(e) => self.fail(describe(&e)),
        }
        self.status()
    }

    /// Downloads (and verifies) the found update. Keeps the bytes in memory
    /// (an installer is a few MB) so nothing unverified ever sits on disk.
    async fn download(&self) {
        let Some(update) = self.lock().update.clone() else {
            return;
        };
        self.set(|s| {
            s.phase = UpdatePhase::Downloading;
            s.progress = Some(0);
        });
        let mut received = 0usize;
        let result = update
            .download(
                |chunk, total| {
                    received += chunk;
                    if let Some(total) = total.filter(|t| *t > 0) {
                        let percent = (received as u64 * 100 / total).min(100) as u8;
                        self.set(|s| s.progress = Some(percent));
                    }
                },
                || {},
            )
            .await;
        match result {
            Ok(bytes) => {
                crate::log_info!(
                    "updates",
                    "update {} downloaded and verified",
                    update.version
                );
                let mut inner = self.lock();
                inner.bytes = Some(bytes);
                inner.status.phase = UpdatePhase::Ready;
                inner.status.progress = None;
            }
            Err(e) => self.fail(describe(&e)),
        }
    }

    /// A peek, once per version (through the activity engine, so quiet mode
    /// and the "System alerts" peek switch apply).
    fn announce<R: Runtime>(&self, app: &AppHandle<R>) {
        let (version, ready) = {
            let mut inner = self.lock();
            let Some(version) = inner.status.version.clone() else {
                return;
            };
            if inner.announced.as_ref() == Some(&version) {
                return;
            }
            inner.announced = Some(version.clone());
            (version, inner.status.phase == UpdatePhase::Ready)
        };
        let automatic = auto_install(app);
        if let Some(activity) = app.try_state::<ActivityService>() {
            (activity.alert_observer())(&SystemAlert::UpdateAvailable {
                version,
                automatic: ready && automatic && crate::platform::user_idle_for().is_some(),
            });
        }
        crate::tray::sync_update(app);
    }

    /// Installs the found update now (downloading it first if needed) and
    /// restarts Islandy. Only returns on failure.
    pub async fn install<R: Runtime>(&self, app: &AppHandle<R>) -> UpdateStatus {
        let phase = self.status().phase;
        if matches!(phase, UpdatePhase::Available | UpdatePhase::Failed)
            && self.lock().update.is_some()
        {
            if self.busy.swap(true, Ordering::AcqRel) {
                return self.status();
            }
            let _busy = BusyGuard(&self.busy);
            self.download().await;
        }
        let (update, bytes) = {
            let inner = self.lock();
            match (&inner.update, &inner.bytes) {
                (Some(u), Some(b)) if inner.status.phase == UpdatePhase::Ready => {
                    (u.clone(), b.clone())
                }
                _ => return inner.status.clone(),
            }
        };
        self.set(|s| s.phase = UpdatePhase::Installing);
        crate::log_info!("updates", "installing {} and restarting", update.version);
        // Flush history and stop background services before the installer
        // replaces the program (on Windows `install` ends this process).
        crate::app::shutdown(app);
        match update.install(bytes) {
            Ok(()) => app.restart(),
            Err(e) => {
                crate::log_error!("updates", "install failed: {e}");
                // Services are stopped: come back cleanly on the old version.
                app.restart();
            }
        }
    }

    /// The background loop: periodic checks and idle installation. Runs on
    /// its own thread until `stop` is set.
    pub fn spawn_loop<R: Runtime>(app: AppHandle<R>, stop: Arc<AtomicBool>) {
        let spawned = std::thread::Builder::new()
            .name("islandy-updates".into())
            .spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(TICK);
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let Some(service) = app.try_state::<UpdateService>() else {
                        continue;
                    };
                    let prefs = app
                        .try_state::<SettingsService>()
                        .map(|s| s.get().updates)
                        .unwrap_or_default();
                    let (phase, due) = {
                        let inner = service.lock();
                        (inner.status.phase, Instant::now() >= inner.next_check)
                    };
                    if prefs.auto_check && due && phase != UpdatePhase::Ready {
                        tauri::async_runtime::block_on(service.check(&app, prefs.auto_install));
                    } else if prefs.auto_install
                        && phase == UpdatePhase::Ready
                        && crate::platform::user_idle_for()
                            .is_some_and(|idle| idle >= INSTALL_WHEN_IDLE_FOR)
                    {
                        crate::log_info!("updates", "user is away: installing the update");
                        tauri::async_runtime::block_on(service.install(&app));
                    }
                }
            });
        if let Err(e) = spawned {
            crate::log_warn!("updates", "update checks unavailable: {e}");
        }
    }
}

fn auto_install<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<SettingsService>()
        .is_some_and(|s| s.get().updates.auto_install)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_trimmed_and_bounded() {
        assert_eq!(short_notes(None), None);
        assert_eq!(short_notes(Some("  \n ")), None);
        assert_eq!(
            short_notes(Some("## Fixes\n- one\u{7}\n")).as_deref(),
            Some("## Fixes\n- one")
        );
        let long = "x".repeat(MAX_NOTES + 10);
        let notes = short_notes(Some(&long)).unwrap();
        assert_eq!(notes.chars().count(), MAX_NOTES + 1);
        assert!(notes.ends_with('…'));
    }

    #[test]
    fn starts_idle_with_the_running_version() {
        let s = UpdateService::default().status();
        assert_eq!(s.phase, UpdatePhase::Idle);
        assert_eq!(s.current_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(s.version, None);
    }

    #[test]
    fn errors_are_described_without_details() {
        assert_eq!(
            describe(&tauri_plugin_updater::Error::ReleaseNotFound),
            "No published release was found."
        );
        assert_eq!(
            describe(&tauri_plugin_updater::Error::EmptyEndpoints),
            "Updates aren't configured in this build."
        );
    }
}
