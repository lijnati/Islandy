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

## Window behavior (Phase 0: partly implemented)

| Behavior                | Mechanism                                                  | Status                             |
| ----------------------- | ---------------------------------------------------------- | ---------------------------------- |
| Borderless, transparent | `decorations: false`, `transparent: true`, `shadow: false` | Implemented; unverified on Windows |
| Topmost                 | `alwaysOnTop` → `HWND_TOPMOST`                             | Implemented; unverified            |
| No taskbar button       | `skipTaskbar` → `ITaskbarList::DeleteTab`                  | Implemented; unverified            |
| Hidden from Alt+Tab     | `WS_EX_TOOLWINDOW` (Win32, `platform/windows.rs`)          | Implemented; unverified            |
| Doesn't steal focus     | `focusable: false` → tao sets `WS_EX_NOACTIVATE`           | Implemented; unverified            |
| DPI-aware top-center    | monitor work area × scale factor (`window/geometry.rs`)    | Implemented; math unit-tested      |

Notes and risks:

- `shadow: false` matters. With `shadow: true`, Tauri documents that an undecorated window
  gets a 1px border and, on Windows 11, rounded corners from DWM.
- **WebView2 and `WS_EX_NOACTIVATE`.** The top-level window refuses activation, but WebView2
  hosts its content in child HWNDs, and there are community reports of WebView2 taking focus
  when clicked. Verify on Windows: click the island while typing in another app and confirm the
  caret stays. Fallback: subclass the top-level window and return `MA_NOACTIVATE` from
  `WM_MOUSEACTIVATE`.
- **Keyboard input in `focused` mode** needs a focusable window. Plan: call `set_focusable(true)`
  when entering `focused` and `set_focusable(false)` on leaving. Re-apply
  `WS_EX_TOOLWINDOW` after each change (see `architecture.md`, native style caveat).
- **Display changes.** Tauri has no monitor-change event. Plan: handle `WM_DISPLAYCHANGE`
  (resolution, monitor hot-plug) and `WM_SETTINGCHANGE` with `SPI_SETWORKAREA` (taskbar
  moved or resized), then call `IslandWindow::reposition()`. Both arrive at top-level windows,
  so a small window subclass, or a hidden message-only window, can catch them.
- **Click-through corners.** The window is exactly the pill's bounding rectangle. To make the
  transparent corner pixels click-through as well, use `SetWindowRgn` with a rounded region.
  The tradeoff is pixel-aligned (aliased) region edges. A per-pixel alternative is to handle
  `WM_NCHITTEST` and return `HTTRANSPARENT` outside the shape.

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

## Media: Global System Media Transport Controls (GSMTC)

Namespace `Windows.Media.Control`; `windows` crate features `Media_Control`, `Foundation`,
`Storage_Streams`. Callable from unpackaged desktop apps.

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
- WinRT event handlers fire on background threads. Forward them over a channel to a single
  service task that debounces bursts (track changes often fire several events) and emits one
  Tauri event.

## Audio: Windows Core Audio

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

## System status

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

## Claude Code activity

Verified against the official hooks reference (https://code.claude.com/docs/en/hooks) on
2026-11-12:

- Hooks are configured under `"hooks"` in `~/.claude/settings.json` (user),
  `.claude/settings.json` (project), `.claude/settings.local.json` (local), managed policy,
  plugin `hooks/hooks.json`, and skill/subagent frontmatter.
- Handler types: `command` (event JSON on stdin), `http` (event JSON as an HTTP POST body),
  `mcp_tool`, `prompt`, and `agent` (experimental). Command hooks can be `async: true`
  (non-blocking).
- Every event includes `session_id`, `transcript_path`, `cwd`, `permission_mode`, and
  `hook_event_name`.
- Relevant events: `SessionStart`, `UserPromptSubmit`, `PreToolUse` / `PostToolUse`
  (`tool_name`, `tool_input`), `PermissionRequest`, `Notification` (`message`, `title`,
  `notification_type`), `Stop`, `StopFailure` (`error`), `SubagentStart` / `SubagentStop`,
  `TaskCreated` / `TaskCompleted`, and `SessionEnd`.
- Matching hooks run in parallel. Default timeout is 600 s, with shorter defaults on some
  events. Command hooks run with the user's full permissions. Interactive sessions wait for
  workspace trust before running settings-file hooks.

**Recommended architecture:**

```
Claude Code ──(async command hook)──► islandy-hook.exe ──► named pipe \\.\pipe\islandy-<user-SID>
                                        (tiny, ships with Islandy)          │ (ACL: current user only)
                                                                              ▼
                                                     Islandy: event service → peek / activity state
```

- **Transport: a Windows named pipe, not a localhost HTTP port.** Create it with a security
  descriptor that grants access only to the current user's SID, and
  `PIPE_REJECT_REMOTE_CLIENTS`. Other users and remote machines cannot connect, no TCP port is
  exposed, and there is no DNS-rebinding or browser-origin surface.
- **Producer: an `async: true` command hook** running a small helper binary that ships with
  Islandy. It reads the hook JSON from stdin, **drops everything except an allow-listed
  subset** (`hook_event_name`, `session_id`, a hash or basename of `cwd`, `tool_name`,
  `notification_type`, timestamps), writes one line to the pipe, and exits 0. If Islandy is
  not running, it exits 0 immediately. It never blocks or alters Claude Code.
  Not forwarding `tool_input`, prompts, or transcript paths keeps secrets out of Islandy.
- **Optional `http` hook alternative:** if a hook must be HTTP, bind to `127.0.0.1` on a
  random port, write the port and a random 256-bit token to a per-user file under
  `%LOCALAPPDATA%\Islandy` (user-only ACL), require the token in a header, reject any request
  that carries an `Origin` header, and cap body size. This has more attack surface than the
  pipe, so it is not the default.
- **Validation in Islandy:** strict schema (serde with `deny_unknown_fields` on the reduced
  message), size limit, rate limit, and a bounded session map that expires idle sessions.
- **Setup:** Islandy can offer to add the hook to `~/.claude/settings.json`. It must show the
  diff and ask first, never edit silently, and it must not touch project settings.
- **UI mapping:** `UserPromptSubmit`/`PreToolUse` → working; `Notification` or
  `PermissionRequest` → needs attention (peek); `Stop` → done (peek); `StopFailure` → error
  (peek).

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
- A future local event API over the same named pipe for user scripts (for example, "build
  finished"), with the same reduced, validated schema.

All sources feed one event service that de-duplicates and rate-limits them, then dispatches
`CONTEXT_EVENT` into the island state model. That model already prevents automatic events
from interrupting the user.
