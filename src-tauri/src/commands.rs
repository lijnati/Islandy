//! The IPC surface. Each command must also be listed in `build.rs` (app
//! manifest) and granted in `capabilities/island.json`, and have a typed
//! wrapper in `src/services/native.ts`.
//!
//! Arguments are typed and validated: sizes are checked numbers, settings
//! pages are an enum. No command accepts paths, URLs or command lines (the
//! Claude Code integration derives every path itself).

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager, Runtime, State};

use crate::{
    app::{self, LaunchInfo},
    error::{IslandyError, Result},
    services::{
        activity::{
            claude_settings::{self, ClaudeIntegration, IntegrationError},
            model::{ActivitySettings, ActivitySnapshot, ActivitySource, ActivityUpdate},
            now_ms,
            protocol::{self, DeveloperEvent, DeveloperWire, SendError, WireEvent},
            ActivityCommand, ActivityService,
        },
        app_info::{self, AppInfo},
        diagnostics::{self, log},
        media::{
            models::{MediaAction, MediaSnapshot, SessionId},
            worker::Request,
            MediaService,
        },
        paths,
        settings::{storage::LoadStatus, Settings, SettingsPush, SettingsService, SettingsState},
        startup::{self, StartupStatus},
        system::{
            audio::AudioRequest,
            display::{DisplayId, DisplayRequest},
            models::{RadioKind, SystemFocus, SystemSnapshot, SystemUpdate},
            monitor::MonitorRequest,
            parse_percent,
            settings::{self as system_settings, SettingsPage},
            SystemRequest, SystemService,
        },
    },
    settings_window::{self, SettingsPage as WindowPage},
    shortcut::{self, ShortcutManager, ShortcutStatus},
    signals::{self, IslandSignal, SignalHub},
    tray,
    window::{
        geometry::{Layouts, LogicalSize},
        FrameStrategy, IslandWindow, MonitorInfo,
    },
};

#[tauri::command]
pub fn get_app_info<R: Runtime>(app: AppHandle<R>) -> AppInfo {
    let info = app_info::collect(&app);
    #[cfg(debug_assertions)]
    crate::log_warn!("ipc", "get_app_info -> {info:?}");
    info
}

/// Mirrored by `IslandConnection` in `src/services/types.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IslandConnection {
    pub shortcut: ShortcutStatus,
    pub frame_strategy: FrameStrategy,
    /// No settings existed before this launch: show a one-time hint.
    pub first_run: bool,
}

/// Registers the frontend's signal channel and reports native status.
#[tauri::command]
pub fn connect_island<R: Runtime>(
    app: AppHandle<R>,
    channel: Channel<IslandSignal>,
) -> Result<IslandConnection> {
    app.state::<SignalHub>().connect(channel);
    Ok(IslandConnection {
        shortcut: app.state::<ShortcutManager>().status(),
        frame_strategy: IslandWindow::get(&app)?.strategy(),
        first_run: app.state::<LaunchInfo>().first_run,
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
    crate::log_warn!("ipc", "set_island_interactive({interactive})");
    IslandWindow::get(&app)?.set_interactive(interactive)
}

/// Opens one of a fixed set of Windows Settings pages.
#[tauri::command]
pub fn open_system_settings(page: SettingsPage) -> Result<()> {
    system_settings::open(page)
}

/* -------------------------------------------------------------------------- */
/* Media (GSMTC). Commands that reach the media worker are async, so they     */
/* never block the main thread; the worker applies native timeouts.           */
/* -------------------------------------------------------------------------- */

fn session_id(value: &str) -> Result<SessionId> {
    SessionId::parse(value).ok_or(IslandyError::InvalidArgument("unknown media session id"))
}

/// Registers the media channel and returns the current snapshot atomically.
#[tauri::command]
pub fn connect_media(
    media: State<'_, MediaService>,
    channel: Channel<MediaSnapshot>,
) -> MediaSnapshot {
    media.connect(channel)
}

#[tauri::command]
pub fn get_media_snapshot(media: State<'_, MediaService>) -> MediaSnapshot {
    media.snapshot()
}

/// `sessionId: null` returns to automatic selection.
#[tauri::command]
pub async fn select_media_session(
    media: State<'_, MediaService>,
    session_id: Option<String>,
) -> Result<MediaSnapshot> {
    let id = session_id.as_deref().map(self::session_id).transpose()?;
    Ok(media.request(Request::Select(id)).await?)
}

/// Sends a playback command. The returned snapshot does not yet contain its
/// effect: confirmed state arrives through the channel once Windows reports it.
#[tauri::command]
pub async fn media_control(
    media: State<'_, MediaService>,
    session_id: String,
    action: MediaAction,
) -> Result<MediaSnapshot> {
    let id = self::session_id(&session_id)?;
    Ok(media.request(Request::Control(id, action)).await?)
}

/// Seeks to `positionMs` from the start of the media (clamped to its duration).
#[tauri::command]
pub async fn media_seek(
    media: State<'_, MediaService>,
    session_id: String,
    position_ms: f64,
) -> Result<MediaSnapshot> {
    let id = self::session_id(&session_id)?;
    if !position_ms.is_finite() || position_ms < 0.0 {
        return Err(IslandyError::InvalidArgument(
            "seek position must be finite and >= 0",
        ));
    }
    Ok(media.request(Request::Seek(id, position_ms as u64)).await?)
}

/// Artwork by content key as a size-limited `data:image/jpeg` URL, or `null`
/// when it is not (or no longer) cached.
#[tauri::command]
pub fn get_media_artwork(media: State<'_, MediaService>, key: String) -> Result<Option<String>> {
    let valid =
        key.len() <= 40 && key.starts_with('a') && key[1..].bytes().all(|b| b.is_ascii_hexdigit());
    if !valid {
        return Err(IslandyError::InvalidArgument("invalid artwork key"));
    }
    Ok(media.artwork(&key))
}

/* -------------------------------------------------------------------------- */
/* System (audio, power, network, radios, metrics, brightness). Commands that */
/* change something are async and run on the owning worker; they return once */
/* Windows accepted the request. The resulting state arrives on the channel.  */
/* -------------------------------------------------------------------------- */

/// Registers the system channel and returns the current snapshot atomically.
#[tauri::command]
pub fn connect_system(
    system: State<'_, SystemService>,
    channel: Channel<SystemUpdate>,
) -> SystemSnapshot {
    system.connect(move |update| {
        if let Err(e) = channel.send(update) {
            crate::log_warn!("ipc", "failed to deliver system update: {e}");
        }
    })
}

#[tauri::command]
pub fn get_system_snapshot(system: State<'_, SystemService>) -> SystemSnapshot {
    system.snapshot()
}

/// What the UI shows: starts/stops CPU/RAM sampling and brightness polling.
#[tauri::command]
pub fn set_system_focus(system: State<'_, SystemService>, focus: SystemFocus) {
    system.set_focus(focus);
}

/// Sets the default output's master volume, 0–100 (Windows' own scale).
#[tauri::command]
pub async fn set_master_volume(system: State<'_, SystemService>, volume: f64) -> Result<()> {
    let volume =
        parse_percent(volume).ok_or(IslandyError::InvalidArgument("volume must be 0–100"))?;
    Ok(system
        .request(SystemRequest::Audio(AudioRequest::SetVolume(volume)))
        .await?)
}

#[tauri::command]
pub async fn set_audio_muted(system: State<'_, SystemService>, muted: bool) -> Result<()> {
    Ok(system
        .request(SystemRequest::Audio(AudioRequest::SetMuted(muted)))
        .await?)
}

/// Switches the Wi-Fi or Bluetooth radio, if Windows granted access.
#[tauri::command]
pub async fn set_radio_state(
    system: State<'_, SystemService>,
    radio: RadioKind,
    on: bool,
) -> Result<()> {
    Ok(system
        .request(SystemRequest::Monitor(MonitorRequest::SetRadio(radio, on)))
        .await?)
}

#[tauri::command]
pub async fn set_display_brightness(
    system: State<'_, SystemService>,
    display_id: String,
    brightness: f64,
) -> Result<()> {
    let id =
        DisplayId::parse(&display_id).ok_or(IslandyError::InvalidArgument("unknown display id"))?;
    let brightness = parse_percent(brightness)
        .ok_or(IslandyError::InvalidArgument("brightness must be 0–100"))?;
    Ok(system
        .request(SystemRequest::Display(DisplayRequest::SetBrightness(
            id, brightness,
        )))
        .await?)
}

/* -------------------------------------------------------------------------- */
/* Activity (Phase 4). Commands only change Islandy's own state; the result   */
/* arrives on the channel. Claude Code integration edits only the user-level  */
/* settings file, and only when the user presses Enable / Remove.             */
/* -------------------------------------------------------------------------- */

fn activity_id(value: &str) -> Result<String> {
    let valid = value.len() <= 24
        && value.len() > 1
        && value.starts_with('a')
        && value[1..].bytes().all(|b| b.is_ascii_hexdigit());
    if valid {
        Ok(value.to_owned())
    } else {
        Err(IslandyError::InvalidArgument("invalid activity id"))
    }
}

/// Registers the activity channel and returns the latest snapshot atomically.
#[tauri::command]
pub fn connect_activity(
    activity: State<'_, ActivityService>,
    channel: Channel<ActivityUpdate>,
) -> ActivitySnapshot {
    activity.connect(move |update| {
        if let Err(e) = channel.send(update) {
            crate::log_warn!("ipc", "failed to deliver activity update: {e}");
        }
    })
}

#[tauri::command]
pub fn get_activity_snapshot(activity: State<'_, ActivityService>) -> ActivitySnapshot {
    activity.snapshot()
}

/// Dismisses an active event, or removes a finished one from history.
#[tauri::command]
pub fn dismiss_activity(activity: State<'_, ActivityService>, id: String) -> Result<()> {
    activity.command(ActivityCommand::Dismiss(activity_id(&id)?));
    Ok(())
}

#[tauri::command]
pub fn clear_activity_history(activity: State<'_, ActivityService>) {
    activity.command(ActivityCommand::ClearHistory);
}

/// Islandy's own quiet mode (suppresses peeks). Not Windows Do Not Disturb.
#[tauri::command]
pub fn set_quiet_mode<R: Runtime>(
    app: AppHandle<R>,
    activity: State<'_, ActivityService>,
    quiet: bool,
) {
    activity.command(ActivityCommand::SetQuiet(quiet));
    tray::set_quiet_checked(&app, quiet);
}

/// Notification preferences, quiet mode, sources and retention at once
/// (normalized by the engine).
#[tauri::command]
pub fn set_activity_settings<R: Runtime>(
    app: AppHandle<R>,
    activity: State<'_, ActivityService>,
    settings: ActivitySettings,
) {
    let quiet = settings.quiet;
    activity.command(ActivityCommand::SetSettings(settings));
    tray::set_quiet_checked(&app, quiet);
}

#[tauri::command]
pub fn set_activity_source(
    activity: State<'_, ActivityService>,
    source: ActivitySource,
    enabled: bool,
) {
    activity.command(ActivityCommand::SetSource(source, enabled));
}

fn own_hook_command() -> Result<String> {
    let exe = std::env::current_exe().map_err(|_| IntegrationError::NoExecutable)?;
    Ok(claude_settings::hook_command(&exe))
}

/// Reads the user-level Claude Code settings (never modifies them).
#[tauri::command]
pub async fn get_claude_integration() -> Result<ClaudeIntegration> {
    let command = own_hook_command()?;
    Ok(claude_settings::status(
        claude_settings::Location::user().as_ref(),
        &command,
    ))
}

/// Adds Islandy's hooks to `~/.claude/settings.json` (explicit user action).
#[tauri::command]
pub async fn install_claude_integration() -> Result<ClaudeIntegration> {
    let command = own_hook_command()?;
    let location = claude_settings::Location::user().ok_or(IntegrationError::NotDetected)?;
    Ok(claude_settings::install(&location, &command, now_ms())?)
}

/// Removes Islandy's hooks (and nothing else) from the user-level settings.
#[tauri::command]
pub async fn remove_claude_integration() -> Result<ClaudeIntegration> {
    let command = own_hook_command()?;
    let location = claude_settings::Location::user().ok_or(IntegrationError::NotDetected)?;
    Ok(claude_settings::remove(&location, &command, now_ms())?)
}

/// Sends a harmless test message through the real local receiver (token,
/// framing and all), exactly like `islandy notify` would.
#[tauri::command]
pub async fn test_local_receiver() -> Result<()> {
    let dir = paths::data_dir().ok_or(IslandyError::Unsupported("no data directory"))?;
    let event = WireEvent::Developer(DeveloperWire {
        event: DeveloperEvent::Message,
        project: None,
        label: None,
        title: Some("Test event from Islandy settings".into()),
        severity: None,
        at: None,
    });
    protocol::send(&dir, &event).map_err(|e| {
        IslandyError::Platform(match e {
            SendError::NotRunning => "the local receiver isn't running".into(),
            SendError::Unreachable => "the local receiver didn't answer".into(),
            SendError::Rejected(code) => format!("the local receiver rejected the event ({code})"),
            SendError::Encode => "the test event couldn't be encoded".into(),
        })
    })
}

/* -------------------------------------------------------------------------- */
/* Settings (Phase 5). Every value is validated and normalized in Rust; the   */
/* stored result is pushed to every connected window.                         */
/* -------------------------------------------------------------------------- */

/// Registers a window's settings channel and returns the current state.
#[tauri::command]
pub fn connect_settings<R: Runtime>(
    window: tauri::Window<R>,
    settings: State<'_, SettingsService>,
    channel: Channel<SettingsPush>,
) -> SettingsState {
    settings.connect(window.label(), channel)
}

/// Stores new settings (normalized) and applies their native side effects.
/// The toggle shortcut is changed only through `set_toggle_shortcut`, so an
/// unregistered combination is never stored.
#[tauri::command]
pub fn update_settings<R: Runtime>(
    app: AppHandle<R>,
    settings: State<'_, SettingsService>,
    next: Settings,
) -> SettingsState {
    let mut next = next;
    next.general.toggle_shortcut = settings.get().general.toggle_shortcut;
    let (previous, state) = settings.update(next);
    app::apply_settings(&app, &previous, &state.settings);
    state
}

/// Restores default preferences (and notification settings). Activity
/// history, Claude Code hooks and launch at sign-in are left as they are;
/// they have their own controls.
#[tauri::command]
pub fn reset_settings<R: Runtime>(
    app: AppHandle<R>,
    settings: State<'_, SettingsService>,
    activity: State<'_, ActivityService>,
) -> SettingsState {
    let defaults = Settings::default();
    let shortcut_ok = shortcut::change(&app, &defaults.general.toggle_shortcut).is_ok();
    let (previous, mut state) = settings.reset();
    if !shortcut_ok {
        // Keep the shortcut that is actually registered.
        let current = app.state::<ShortcutManager>().status().accelerator;
        state = settings.modify(|s| s.general.toggle_shortcut = current).1;
    }
    app::apply_settings(&app, &previous, &state.settings);
    let quiet = activity.snapshot().settings.quiet;
    activity.command(ActivityCommand::SetSettings(ActivitySettings {
        quiet,
        ..ActivitySettings::default()
    }));
    crate::log_info!("settings", "settings reset to defaults");
    state
}

/// Changes the global toggle shortcut. The previous shortcut stays active if
/// the new one can't be registered; the error says why.
#[tauri::command]
pub fn set_toggle_shortcut<R: Runtime>(
    app: AppHandle<R>,
    settings: State<'_, SettingsService>,
    accelerator: String,
) -> Result<ShortcutStatus> {
    let status = shortcut::change(&app, &accelerator).map_err(IslandyError::Shortcut)?;
    settings.modify(|s| s.general.toggle_shortcut = status.accelerator.clone());
    Ok(status)
}

#[tauri::command]
pub fn get_shortcut_status<R: Runtime>(app: AppHandle<R>) -> ShortcutStatus {
    app.state::<ShortcutManager>().status()
}

#[tauri::command]
pub fn get_startup_status() -> StartupStatus {
    startup::status()
}

/// Launch at sign-in (per-user, no administrator rights).
#[tauri::command]
pub fn set_launch_at_login<R: Runtime>(app: AppHandle<R>, enabled: bool) -> Result<StartupStatus> {
    let status = startup::set(enabled)?;
    tray::sync(&app);
    Ok(status)
}

#[tauri::command]
pub fn list_monitors<R: Runtime>(app: AppHandle<R>) -> Result<Vec<MonitorInfo>> {
    IslandWindow::get(&app)?.monitors()
}

/// Live horizontal offset while the user drags the pill (logical px);
/// `null` ends the preview. Returns the offset actually applied.
#[tauri::command]
pub fn preview_island_offset<R: Runtime>(app: AppHandle<R>, offset: Option<f64>) -> Result<f64> {
    if offset.is_some_and(|o| !o.is_finite()) {
        return Err(IslandyError::InvalidArgument("offset must be finite"));
    }
    IslandWindow::get(&app)?.preview_offset(offset)
}

/// Hides the island (Islandy keeps running in the tray) or shows it.
#[tauri::command]
pub fn set_island_visible<R: Runtime>(app: AppHandle<R>, visible: bool) {
    app::set_island_visible(&app, visible);
}

/// Async on purpose: a synchronous command runs on the main thread inside a
/// WebView2 callback, and creating another WebView2 window there deadlocks on
/// Windows (black, unresponsive window that can't be closed). See wry#583.
#[tauri::command]
pub async fn open_settings_window<R: Runtime>(
    app: AppHandle<R>,
    page: Option<WindowPage>,
) -> Result<()> {
    settings_window::open(&app, page.unwrap_or_default())
}

/// Islandy's own folders (never a path from the frontend).
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IslandyFolder {
    Data,
    Logs,
}

#[tauri::command]
pub fn open_islandy_folder(folder: IslandyFolder) -> Result<()> {
    let data = paths::data_dir().ok_or(IslandyError::Unsupported("no data directory"))?;
    let path = match folder {
        IslandyFolder::Data => data,
        IslandyFolder::Logs => data.join("logs"),
    };
    paths::ensure_private_dir(&path)?;
    crate::platform::open_folder(&path)
}

/// Where Islandy keeps its files (shown in Settings › Privacy).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub data_dir: Option<String>,
    pub files: Vec<StoredFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredFile {
    pub name: &'static str,
    pub purpose: &'static str,
    pub bytes: Option<u64>,
}

#[tauri::command]
pub fn get_storage_info() -> StorageInfo {
    let dir = paths::data_dir();
    let size = |name: &str| {
        dir.as_ref()
            .and_then(|d| std::fs::metadata(d.join(name)).ok())
            .map(|m| m.len())
    };
    let logs = dir.as_ref().map(|d| {
        std::fs::read_dir(d.join("logs"))
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok()?.metadata().ok())
                    .map(|m| m.len())
                    .sum::<u64>()
            })
            .unwrap_or(0)
    });
    StorageInfo {
        data_dir: dir.as_ref().map(|d| d.display().to_string()),
        files: vec![
            StoredFile {
                name: "settings.json",
                purpose: "Your preferences",
                bytes: size("settings.json"),
            },
            StoredFile {
                name: "activity.json",
                purpose: "Activity history and notification settings",
                bytes: size("activity.json"),
            },
            StoredFile {
                name: "receiver.json",
                purpose: "Local receiver port and token (while Islandy runs)",
                bytes: size("receiver.json"),
            },
            StoredFile {
                name: "logs",
                purpose: "Diagnostic log (bounded, about 3 MB at most)",
                bytes: logs,
            },
        ],
    }
}

/// A plain-text summary for bug reports: versions, feature availability,
/// integration status and sanitized recent problems. No user activity.
#[tauri::command]
pub async fn get_diagnostics<R: Runtime>(app: AppHandle<R>) -> String {
    use crate::services::{
        media::models::IntegrationStatus,
        system::models::{Availability, BatteryInfo},
    };
    let info = app_info::collect(&app);
    let island = IslandWindow::get(&app).ok();
    let shortcut = app.state::<ShortcutManager>().status();
    let settings = app.state::<SettingsService>().state();
    let media = app.state::<MediaService>().snapshot();
    let system = app.state::<SystemService>().snapshot();
    let activity = app.state::<ActivityService>().snapshot();
    let startup = startup::status();
    let at_login = app.state::<LaunchInfo>().at_login;
    let claude = own_hook_command().map(|command| {
        claude_settings::status(claude_settings::Location::user().as_ref(), &command).state
    });
    let availability = |a: Availability| match a {
        Availability::Loading => "loading",
        Availability::Ready => "ready",
        Availability::Unavailable => "unavailable",
        Availability::Unsupported => "unsupported",
    };
    let report = diagnostics::Report {
        version: info.version,
        build: if info.debug { "debug" } else { "release" },
        tauri: info.tauri_version,
        webview: info.webview_version,
        os: diagnostics::os_version(),
        arch: info.arch,
        frame_strategy: island.as_ref().map_or("unknown".into(), |w| {
            format!("{:?}", w.strategy()).to_lowercase()
        }),
        placement: island
            .as_ref()
            .map_or("not placed".into(), |w| w.describe()),
        shortcut: format!(
            "{} ({})",
            shortcut.accelerator,
            if shortcut.registered {
                "registered".to_owned()
            } else {
                format!("not registered: {}", shortcut.error.unwrap_or_default())
            }
        ),
        launch_at_login: format!(
            "{}{}",
            match (
                startup.supported,
                startup.enabled,
                startup.disabled_by_system
            ) {
                (false, _, _) => "unsupported",
                (_, true, _) => "on",
                (_, _, true) => "off (disabled in Task Manager)",
                _ => "off",
            },
            if at_login {
                "; this launch was at sign-in"
            } else {
                ""
            }
        ),
        settings: format!(
            "{}{}",
            match settings.load_status {
                LoadStatus::Fresh => "defaults".to_owned(),
                LoadStatus::Loaded => "loaded".to_owned(),
                LoadStatus::Migrated { from } => format!("migrated from v{from}"),
                LoadStatus::Recovered => "recovered from an unreadable file".to_owned(),
                LoadStatus::Newer { version } => format!("written by a newer version (v{version})"),
            },
            if settings.save_error {
                ", not saved (write failed)"
            } else {
                ""
            }
        ),
        media: match media.status {
            IntegrationStatus::Starting => "starting".into(),
            IntegrationStatus::Ready => format!("ready, {} session(s)", media.sessions.len()),
            IntegrationStatus::Unavailable => "unavailable".into(),
            IntegrationStatus::Unsupported => "unsupported on this platform".into(),
        },
        system: vec![
            (
                "Audio",
                format!(
                    "{}, {} output device(s)",
                    availability(system.audio.status),
                    system.audio.devices.len()
                ),
            ),
            (
                "Power",
                format!(
                    "{}, {}",
                    availability(system.power.status),
                    match system.power.battery {
                        BatteryInfo::Present { .. } => "battery present",
                        BatteryInfo::Absent => "no battery",
                        BatteryInfo::Unknown => "battery unknown",
                    }
                ),
            ),
            ("Network", availability(system.network.status).into()),
            (
                "Radios",
                format!(
                    "{}, Wi-Fi {}, Bluetooth {}",
                    availability(system.radios.status),
                    if system.radios.wifi.is_some() {
                        "present"
                    } else {
                        "absent"
                    },
                    if system.radios.bluetooth.is_some() {
                        "present"
                    } else {
                        "absent"
                    }
                ),
            ),
            (
                "Brightness",
                format!(
                    "{}, {} controllable, {} unsupported display(s)",
                    availability(system.display.status),
                    system.display.displays.len(),
                    system.display.unsupported_count
                ),
            ),
        ],
        activity: format!(
            "receiver {:?}, quiet {}, {} event(s) in history{}",
            activity.receiver,
            if activity.settings.quiet { "on" } else { "off" },
            activity.events.len(),
            if activity.history_error {
                ", history not saved"
            } else {
                ""
            }
        )
        .to_lowercase(),
        claude_code: match claude {
            Ok(state) => format!("{state:?}"),
            Err(e) => format!("unknown ({})", e.kind_name()),
        },
        problems: log::recent_problems(),
    };
    diagnostics::format_report(&report)
}
