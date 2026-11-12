//! The IPC surface. Each command must also be listed in `build.rs` (app
//! manifest) and granted in `capabilities/island.json`, and have a typed
//! wrapper in `src/services/native.ts`.
//!
//! Arguments are typed and validated: sizes are checked numbers, settings
//! pages are an enum. No command accepts paths, URLs or command lines.

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager, Runtime, State};

use crate::{
    error::{IslandyError, Result},
    services::{
        app_info::{self, AppInfo},
        media::{
            models::{MediaAction, MediaSnapshot, SessionId},
            worker::Request,
            MediaService,
        },
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
            eprintln!("[islandy] failed to deliver system update: {e}");
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
