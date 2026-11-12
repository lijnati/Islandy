# Windows integration strategy

Research notes for features after Phase 0. Nothing in this document is implemented yet, except
what the "Window behavior" section marks as implemented.

Principles:

- Prefer supported, documented APIs: WinRT `Windows.*` namespaces and Win32 via the official
  [`windows`](https://crates.io/crates/windows) crate.
- Prefer event subscriptions over polling. Where polling is unavoidable, poll only while the
  data is on screen.
- All native code lives in `src-tauri/src/platform/` behind small functions. Services own the
  subscriptions and forward compact, typed updates to the UI through Tauri events.
- Islandy ships **unpackaged** (NSIS, no MSIX), so it has no _package identity_. Several WinRT
  APIs that require a declared capability (appointments, notification listener) are therefore
  unavailable. Each section notes this.

Unless stated otherwise, every API below is available on Windows 10 1809 (build 17763) and
later.

---

## Window behavior (Phases 0–1: implemented, unverified on Windows)

| Behavior                | Mechanism                                                  | Status                             |
| ----------------------- | ---------------------------------------------------------- | ---------------------------------- |
| Borderless, transparent | `decorations: false`, `transparent: true`, `shadow: false` | Implemented; unverified on Windows |
| Topmost                 | `alwaysOnTop` → `HWND_TOPMOST`                             | Implemented; unverified            |
| No taskbar button       | `skipTaskbar` → `ITaskbarList::DeleteTab`                  | Implemented; unverified            |
| Hidden from Alt+Tab     | `WS_EX_TOOLWINDOW` (Win32, `platform/windows.rs`)          | Implemented; unverified            |
| Doesn't steal focus     | `focusable: false` → tao sets `WS_EX_NOACTIVATE`           | Implemented; unverified            |
| DPI-aware top-center    | monitor work area × scale factor (`window/geometry.rs`)    | Implemented; math unit-tested      |
| Click-through outside   | `SetWindowRgn` on a fixed canvas (`FrameStrategy::Region`) | Implemented; unverified            |
| Atomic move + resize    | one `SetWindowPos` (resize fallback)                       | Implemented; unverified            |
| Keyboard when pinned    | `set_focusable(true)` + focus; restore previous window     | Implemented; unverified            |
| Click outside           | debounced `WindowEvent::Focused(false)`, no mouse hook     | Implemented; unverified            |
| Global shortcut         | `tauri-plugin-global-shortcut` (`RegisterHotKey`)          | Implemented; unverified            |
| Settings pages          | `ShellExecuteW("ms-settings:…")`, fixed allow-list         | Implemented; unverified            |

Notes and risks:

- `shadow: false` matters. With `shadow: true`, Tauri documents that an undecorated window
  gets a 1px border and, on Windows 11, rounded corners from DWM.
- **WebView2 and `WS_EX_NOACTIVATE`.** The top-level window refuses activation, but WebView2
  hosts its content in child HWNDs, and there are community reports of WebView2 taking focus
  when clicked. Verify on Windows: click the island while typing in another app and confirm the
  caret stays. Fallback: subclass the top-level window and return `MA_NOACTIVATE` from
  `WM_MOUSEACTIVATE`.
- **Keyboard input** needs a focusable window. Implemented (Phase 1): pinning calls
  `set_focusable(true)` and focuses the window; collapsing calls `set_focusable(false)`.
  `WS_EX_TOOLWINDOW` is re-applied after each change (see `architecture.md`). Foreground
  rules: `SetForegroundWindow` succeeds because the island just received the user's click or
  hotkey. Verify on Windows that focus really moves on click and on the shortcut.
- **Window regions with transparent WebView2 (top Phase 1 risk).** On the Linux dev host, an
  X11 bounding shape stopped WebKitGTK from painting, so Linux uses only an input shape.
  Whether `SetWindowRgn` clips DirectComposition-based WebView2 content cleanly on Windows
  10/11 is **unverified**. If the island renders blank or with artifacts, run with
  `ISLANDY_FRAME_STRATEGY=resize` and report it; a per-pixel alternative is to keep the
  canvas and return `HTTRANSPARENT` from `WM_NCHITTEST` outside the frame (window subclass).
- **Display changes.** Tauri has no monitor-change event. Phase 1 re-reads the monitor on
  every layout request and frame change (i.e. on the next hover), and on DPI changes. Still
  planned: handle `WM_DISPLAYCHANGE` and `WM_SETTINGCHANGE`/`SPI_SETWORKAREA` so the resting
  pill moves immediately when the taskbar or resolution changes.
- **Click-through corners.** The region is the frame's bounding rectangle, so the few
  transparent pixels at the rounded corners still receive clicks. A rounded region would fix
  it at the cost of aliased edges; `WM_NCHITTEST` → `HTTRANSPARENT` is the per-pixel option.

### Fullscreen and maximized applications (recommendation for a later phase)

| Situation                                                | What happens with a topmost window                                                                                              | Recommendation                                                                        |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Maximized window                                         | The island overlaps the center of its title bar. Windows 11 apps usually put the title on the left, so this is mostly harmless. | Keep visible. Possibly offer "hide over maximized windows" later.                     |
| Borderless fullscreen (browser video, most modern games) | The island is drawn over the content.                                                                                           | Hide the island on that monitor while a fullscreen window is in the foreground there. |
| Exclusive fullscreen (D3D)                               | Topmost windows can force the game out of exclusive mode or minimize it.                                                        | Must hide.                                                                            |
| Multiple monitors                                        | Fullscreen state is per monitor.                                                                                                | Evaluate only the island's monitor.                                                   |

Detection approach (event-driven, no polling):

1. `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, …, WINEVENT_OUTOFCONTEXT)` to learn when the
   foreground window changes. Also consider `EVENT_SYSTEM_MOVESIZEEND` and
   `EVENT_OBJECT_LOCATIONCHANGE` filtered to the foreground HWND, to catch an app entering
   fullscreen in place.
2. On change, compare the foreground window's rect with its monitor's full rect
   (`MonitorFromWindow` + `GetMonitorInfoW`). Exclude the shell (`Progman`, `WorkerW`,
   `Shell_TrayWnd`) and Islandy itself.
3. Use `SHQueryUserNotificationState` as a secondary signal. `QUNS_RUNNING_D3D_FULL_SCREEN`,
   `QUNS_BUSY` and `QUNS_PRESENTATION_MODE` mean "stay out of the way".
4. Hide with `ShowWindow(SW_HIDE)` and restore when the condition clears. Do not change
   z-order to "fix" it.

---

## Media: Global System Media Transport Controls (GSMTC) (Phase 2: implemented, unverified on Windows)

Implemented in `src-tauri/src/services/media/`; full design in [media.md](media.md).
Namespace `Windows.Media.Control`; `windows` 0.62 features `Media_Control`, `Foundation`,
`Storage_Streams`, `Graphics_Imaging` (artwork transcoding), `ApplicationModel` (app names),
`Win32_System_Com` (MTA worker); plus `windows-future` 0.3 to name the async types (they are
awaited with `when` + `GetResults` and a timeout, not `join`). Callable from unpackaged
desktop apps without any capability or elevation.

| Need              | API                                                                                                                                                                           |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Discover sessions | `GlobalSystemMediaTransportControlsSessionManager::RequestAsync()` → `GetCurrentSession()` (the one Windows considers current), `GetSessions()` (all)                         |
| Track metadata    | `session.TryGetMediaPropertiesAsync()` → `Title`, `Artist`, `AlbumTitle`, `AlbumArtist`, `TrackNumber`, `PlaybackType`, `Thumbnail`                                           |
| Artwork           | `Thumbnail` is an `IRandomAccessStreamReference`. Call `OpenReadAsync()`, read the bytes, downscale (e.g. to 128 px) in Rust, then send the bytes or a cached file to the UI. |
| Playback state    | `session.GetPlaybackInfo()` → `PlaybackStatus`, `Controls` (`IsPlayEnabled`, `IsPauseEnabled`, `IsNextEnabled`, …), shuffle/repeat                                            |
| Position          | `session.GetTimelineProperties()` → `Position`, `StartTime`, `EndTime`, `LastUpdatedTime`                                                                                     |
| Control           | `TryPlayAsync`, `TryPauseAsync`, `TryTogglePlayPauseAsync`, `TrySkipNextAsync`, `TrySkipPreviousAsync`, `TryChangePlaybackPositionAsync`                                      |
| Subscribe         | Manager: `CurrentSessionChanged`, `SessionsChanged`. Session: `MediaPropertiesChanged`, `PlaybackInfoChanged`, `TimelinePropertiesChanged`                                    |

Caveats:

- Many apps update timeline properties rarely. Interpolate progress in the UI from
  `Position + (now − LastUpdatedTime)` while playing. Do not poll.
- Metadata quality depends on the source app. Browsers report per-tab media sessions;
  `SourceAppUserModelId` identifies the app.
- Artwork can be absent or arrive after the title; render a placeholder and update.
- WinRT event handlers fire on background threads. Islandy forwards them over a channel to a
  single worker that coalesces bursts (track changes often fire several events) and pushes at
  most one snapshot through a Tauri channel.
- Session objects returned by `GetSessions()` are matched to stable ids by COM identity; the
  AUMID is not a session id (one browser can own several sessions).

## Audio: Windows Core Audio (Phase 3: implemented, unverified on Windows)

Implemented in `src-tauri/src/services/system/windows/audio.rs`; behavior and decisions in
[system.md](system.md). The notes below are the original research and still apply.

Win32 COM (MMDevice + EndpointVolume); `windows` crate features `Win32_Media_Audio`,
`Win32_Media_Audio_Endpoints`, `Win32_System_Com`, `Win32_UI_Shell_PropertiesSystem`,
`Win32_Devices_FunctionDiscovery`.

| Need                  | API                                                                                                                                                                                          |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Default output device | `CoCreateInstance(MMDeviceEnumerator)` → `IMMDeviceEnumerator::GetDefaultAudioEndpoint(eRender, eConsole)`                                                                                   |
| Read/set volume       | `IMMDevice::Activate::<IAudioEndpointVolume>()` → `GetMasterVolumeLevelScalar` / `SetMasterVolumeLevelScalar(level, &event_context)`                                                         |
| Mute                  | `GetMute` / `SetMute`                                                                                                                                                                        |
| Volume/mute changes   | `RegisterControlChangeNotify(IAudioEndpointVolumeCallback)`. `OnNotify` delivers new volume and mute. Pass an Islandy event-context GUID when setting, so our own changes can be told apart. |
| Enumerate outputs     | `EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)`; names via `OpenPropertyStore` + `PKEY_Device_FriendlyName`                                                                               |
| Device changes        | `RegisterEndpointNotificationCallback(IMMNotificationClient)`: `OnDefaultDeviceChanged`, `OnDeviceStateChanged`, `OnDeviceAdded`/`Removed`                                                   |

Caveats:

- **Switching the default output device has no public API.** The commonly used `IPolicyConfig`
  interface is undocumented and can break between Windows builds. Show the device list
  read-only at first. Treat switching as an explicit, isolated, best-effort feature, if added
  at all.
- When the default device changes, re-acquire `IAudioEndpointVolume` and re-register the
  callback.
- COM must be initialized on the thread that owns these objects (a dedicated audio thread
  with `CoInitializeEx(COINIT_MULTITHREADED)`).

## System status (Phase 3: implemented, unverified on Windows)

Implemented as follows (details in [system.md](system.md)):

- **Battery:** `GetSystemPowerStatus` for one consistent reading, which also reports "no
  system battery". It is re-read on the `PowerManager` events.
- **CPU and memory:** `GetSystemTimes` and `GlobalMemoryStatusEx`. No `sysinfo` dependency.
- **Network:** `NetworkInformation`, with the `NetworkStatusChanged` event.
- **Radios:** Wi-Fi and Bluetooth via `Windows.Devices.Radios`.
- **Brightness:** WMI `WmiMonitorBrightness*` for built-in panels and the DDC/CI Monitor
  Configuration API for external monitors.

Original research:

| Metric    | Recommended API                                                                                                                                                                                                                                                                   | Update model                                                   |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Battery   | WinRT `Windows.System.Power.PowerManager`: `RemainingChargePercent`, `BatteryStatus`, `PowerSupplyStatus`, and the matching `*Changed` events. `Windows.Devices.Power.Battery::AggregateBattery()` + `ReportUpdated` for more detail. Win32 `GetSystemPowerStatus` as a fallback. | Event-driven                                                   |
| CPU usage | Win32 `GetSystemTimes`: diff idle/kernel/user between samples. Per-core needs PDH or `NtQuerySystemInformation`; avoid unless needed.                                                                                                                                             | Poll every ~1–2 s, **only while the system widget is visible** |
| Memory    | Win32 `GlobalMemoryStatusEx` (`dwMemoryLoad`, `ullTotalPhys`, `ullAvailPhys`)                                                                                                                                                                                                     | Same as CPU                                                    |
| Network   | WinRT `Windows.Networking.Connectivity.NetworkInformation`: `GetInternetConnectionProfile()`, `GetNetworkConnectivityLevel()`, `IsWlanConnectionProfile`, `NetworkStatusChanged` event                                                                                            | Event-driven                                                   |

Caveats:

- Wi-Fi SSID lookups (WLAN API, or `WlanConnectionProfileDetails`) are restricted by location
  privacy on recent Windows 11 builds and may return nothing without location consent. Show
  "Wi-Fi connected" without the SSID unless the user opts in.
- The `sysinfo` crate is a reasonable shortcut for CPU and memory. Benchmark it first: it
  refreshes more than needed unless refreshes are scoped.

## Calendar

Two distinct features:

1. **Date widget (MVP, local only).** Today's date, a month grid, and optionally local events
   stored in Islandy's app-data folder. No permissions, no network. Recommended first.
2. **External calendar accounts (later, opt-in).**
   - The WinRT `Windows.ApplicationModel.Appointments.AppointmentManager.RequestStoreAsync`
     requires the `appointments` capability, i.e. package identity. It is not available to the
     unpackaged NSIS build.
   - Practical options, in order of privacy and simplicity:
     a. Import or subscribe to an **ICS file or URL** the user provides. Parse locally;
     fetching a URL is the only network use.
     b. **Microsoft Graph / Google Calendar** via OAuth (PKCE, loopback redirect), with tokens
     in Windows Credential Manager. This is network-dependent and account-bound, so it must
     be explicitly opt-in, and it is out of scope for the MVP.

## Claude Code activity (Phase 4: implemented; verified on Linux with real hooks, unverified on Windows)

Re-checked against the hooks reference and the installed Claude Code 2.1.322 on 2026-11-12.
Full design: [claude-code-integration.md](claude-code-integration.md).

```
Claude Code ──(async, exec-form command hook)──► islandy.exe hook ──► 127.0.0.1:<port> ──► activity engine
                                                  (reduced event,       (token from receiver.json
                                                   always exit 0)        in %LOCALAPPDATA%)
```

- **Transport changed from the earlier named-pipe recommendation** to loopback TCP with a
  per-installation token and a non-HTTP line protocol. It is portable, so the same code ran end
  to end with real Claude Code in development, and it avoids writing an untested pipe security
  descriptor blind. Trade-offs and the threat model are in claude-code-integration.md.
- **Windows-specific points to verify:**
  - Hooks run under Git Bash or PowerShell. The exec form (`command` + `args`) avoids both
    shells' quoting.
  - The GUI-subsystem `islandy.exe` runs as a hook without flashing a console.
  - No firewall prompt for a loopback listener.
  - `%LOCALAPPDATA%\com.islandy.desktop` holds `receiver.json` with the profile's user-only ACL.
- **Setup** edits only `%USERPROFILE%\.claude\settings.json` (or `CLAUDE_CONFIG_DIR`), after an
  explicit confirmation, with a backup. It never edits project settings.

## Notifications

Reading other apps' notifications is not a general capability on Windows:

- `Windows.UI.Notifications.Management.UserNotificationListener` requires the
  `userNotificationListener` capability (package identity) **and** explicit user consent. It
  only exposes toast notifications currently in the notification center, not a full history
  and not in-app UI. Unpackaged Islandy cannot use it. MSIX packaging could enable it later,
  with privacy implications.
- There is no supported API for other apps' notification history, and scraping the
  notification database is unsupported and fragile.

Recommended alternative: **first-party contextual sources** that Islandy observes directly.

- Media changes (GSMTC).
- Agent activity (Claude Code hooks, above).
- Downloads: watch the user's Downloads folder (`ReadDirectoryChangesW`, e.g. via the
  `notify` crate) for temporary-file patterns (`.crdownload`, `.part`, `.partial`) appearing
  and being renamed to the final name. This is a heuristic: browsers expose no progress API,
  so show "downloading…" / "downloaded", not a percentage, unless the size is known.
- Developer events from scripts: `islandy notify …` (Phase 4) over the same authenticated
  local receiver, with a reduced, validated schema.

All sources feed one event service that de-duplicates and rate-limits them, then dispatches
`CONTEXT_EVENT` into the island state model. That model already prevents automatic events
from interrupting the user.
