# System integration (Phase 3)

Audio, battery and power, CPU and memory, network, Wi-Fi and Bluetooth radios, display
brightness and Windows Settings shortcuts. Everything here reads or changes **real Windows
state**: no toggle or value is simulated in the UI.

> **Verification status.** The Windows backends compile and pass `clippy -D warnings` for
> `x86_64-pc-windows-msvc`, and a Windows build cross-compiles. All platform-independent
> logic is unit-tested. **None of the Windows behavior has been exercised on a Windows
> machine yet.** The "System (Phase 3)" checklist in the README is the exit gate. On the Linux
> development host, CPU and memory are real (read from `/proc`) and the other domains report
> "not available on this system".

## Architecture

```text
 Core Audio callbacks ──AudioEvent──► audio worker (MTA) ───┐
 WinRT power/network/radio events ──► monitor worker (MTA) ─┼─► SystemHub ──Channel<SystemUpdate>──► system-store ──► panels
 (no change events; polled while visible) ► display worker ─┘   (latest state per domain,
 Tauri commands ──Request──► owning worker ──► Result<()>          revisions, alerts)
```

- **Three worker threads**, each in the multithreaded COM apartment and each owning its own
  native objects. Native callbacks only post a message. A slow DDC/CI monitor therefore can't
  delay a volume change, and nothing runs on the UI thread.
- **Per-domain updates.** When something changes, only that domain's state is pushed
  (`{ domain: "audio", revision, state }`); unchanged reads are dropped by the hub. Revisions
  rise monotonically. The frontend keeps a revision **per domain** and ignores anything
  older.
- **Commands** (`set_master_volume`, `set_audio_muted`, `set_radio_state`,
  `set_display_brightness`) resolve once Windows accepted the request. The new state is what
  Windows reports afterwards, delivered through the channel. Commands are async and wait with
  `spawn_blocking` plus a timeout, so the Tauri main thread never blocks.
- **Shutdown:** on `RunEvent::Exit` every worker unregisters its callbacks and event tokens
  (bounded waits).

Code: `src-tauri/src/services/system/` (pure, tested modules and the worker loops) plus
`system/windows/` (native backends). On the frontend: `src/features/system/` (store and pure
model) and `src/components/system/` (panels).

## Native APIs

| Domain     | Read                                                                                                                                                                                                                    | Change                                                                 | Change notifications                                                                                                                  |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Audio      | `IMMDeviceEnumerator` (`GetDefaultAudioEndpoint(eRender, eConsole)`, `EnumAudioEndpoints`), `IPropertyStore` (`PKEY_Device_FriendlyName`), `IAudioEndpointVolume` (`GetMasterVolumeLevelScalar`, `GetMute`)             | `SetMasterVolumeLevelScalar`, `SetMute`                                | `IAudioEndpointVolumeCallback` (volume/mute from any source), `IMMNotificationClient` (default device, added/removed/state, renames)  |
| Power      | `GetSystemPowerStatus`                                                                                                                                                                                                  | –                                                                      | WinRT `PowerManager`: `BatteryStatusChanged`, `PowerSupplyStatusChanged`, `RemainingChargePercentChanged`, `EnergySaverStatusChanged` |
| Network    | WinRT `NetworkInformation.GetInternetConnectionProfile` / `GetConnectionProfiles`, `ConnectionProfile.GetNetworkConnectivityLevel`, `ProfileName`, `IsWlan/IsWwanConnectionProfile`, `NetworkAdapter.IanaInterfaceType` | –                                                                      | `NetworkInformation.NetworkStatusChanged`                                                                                             |
| Radios     | WinRT `Radio.GetRadiosAsync` (`Kind`, `State`)                                                                                                                                                                          | `Radio.RequestAccessAsync` (first toggle only), `Radio.SetStateAsync`  | `Radio.StateChanged`                                                                                                                  |
| CPU/memory | `GetSystemTimes`, `GlobalMemoryStatusEx`                                                                                                                                                                                | –                                                                      | none: sampled only while the System panel is open                                                                                     |
| Brightness | Built-in panels: WMI `root\WMI` `WmiMonitorBrightness`. External monitors: `GetPhysicalMonitorsFromHMONITOR`, `GetMonitorBrightness` (DDC/CI)                                                                           | `WmiMonitorBrightnessMethods.WmiSetBrightness`, `SetMonitorBrightness` | none usable without a window: re-read every 4 s while the Display panel is open                                                       |
| Settings   | –                                                                                                                                                                                                                       | `ShellExecuteW` with an allow-listed `ms-settings:` URI                | –                                                                                                                                     |

All of these are documented, user-level APIs: no administrator rights, no drivers, no
registry edits, no undocumented interfaces.

## Behavior by domain

### Audio

- **Scale.** Islandy uses the endpoint _scalar_ (0.0–1.0), not decibels. Windows already maps
  the scalar onto an audio-tapered curve, and it is exactly what the Windows volume flyout
  shows, so "40%" in Islandy is "40" in Windows. `percent = round(scalar × 100)`, clamped. A
  round trip over 0–100 is lossless (tested).
- **Which device.** The default _console_ render endpoint, the one the taskbar volume
  controls. On a default-device change the volume callback is moved to the new device and an
  "Audio output changed" peek appears. The first reading never peeks.
- **Device list.** Active render endpoints with friendly names. The list is re-enumerated
  only when `IMMNotificationClient` reports a change, never on volume ticks.
- **Switching the default output is not offered.** Windows has no documented API for it. The
  `IPolicyConfig` interface some tools use is undocumented and has changed between Windows
  releases, so it was rejected. The panel links to Sound settings instead.
- **Slider.** "Latest wins" (`system-store.ts`): at most one request in flight per slider,
  newer values replace the queued one, and requests are at least 50 ms apart. While the user
  drags, the thumb shows their value (an _intent_). After the last request settles, the
  intent is dropped once Windows reports a newer value, or after 1.5 s. An update from an
  older request therefore can't make the thumb jump back. If the request fails, the slider
  returns to Windows' value with a short notice.
- **Mute** is never optimistic: the button shows "busy" until the request settles, and the
  icon changes only when Windows reports the new state.
- Volume changes from Windows or other apps arrive through the callback. Bursts (dragging
  the Windows slider) are coalesced into one read per 40 ms.

### Power and battery

- Normalized from `SYSTEM_POWER_STATUS`:
  - **Battery:** `present` (with percent, or `null` when unknown), `absent`
    (`BatteryFlag & 128`, i.e. a desktop), or `unknown` (flag 255).
  - **Power source:** `ac` / `battery` / `unknown`.
  - **Charge state:** `charging` / `discharging` / `notCharging` (on AC, full or held) /
    `unknown`.
  - **Battery saver:** from `SystemStatusFlag`, `null` if not reported.
- **Desktops never show "100%".** The tile reads "No battery · Runs on AC power".
- **Alerts** (`power.rs`):
  - "Battery low" at ≤ 20 % and "critically low" at ≤ 10 %, while discharging, **once per
    discharge**. They re-arm only after plugging in or recovering 5 points above the
    threshold.
  - "Power connected" and "On battery" when the source changes.
  - Startup state never alerts. Desktops never alert.

### CPU and memory

- CPU = busy share of elapsed time between two `GetSystemTimes` samples (kernel time
  includes idle). The first sample after sampling starts is only a baseline, so the UI shows
  "Measuring…" rather than a guess. A second sample is taken 0.7 s later, then every 2 s.
- Memory = `(total − available) / total` from `GlobalMemoryStatusEx`.
- **Sampling runs only while the System panel is open** (`set_system_focus("overview")`). It
  stops when the panel closes, and the last values are marked paused. Collapsed or on the
  default dashboard there is no sampling at all.

### Network

- Connectivity is Windows' own NCSI assessment:
  - `internet`: connected with verified internet access.
  - `constrained`: limited access, for example a captive portal.
  - `local`: connected, but no internet.
  - `none`: not connected.
- An associated adapter is never treated as internet access, and Islandy makes no network
  requests to test connectivity.
- The connection type (Wi-Fi / Ethernet / cellular / other) comes from the profile flags and
  the adapter's IANA type. The network name is the profile name, if Windows provides it; it
  is never guessed.
- **Alerts:**
  - "Network disconnected" only after connectivity has stayed lost for **3 s** (Wi-Fi
    roaming and adapter resets are ignored), and only after a fresh re-read confirms it.
  - "Connected to Wi-Fi / Ethernet" when coming back online after a confirmed loss, or after
    starting offline. A blip shorter than 3 s produces neither alert.

### Wi-Fi and Bluetooth radios

- State: `on` / `off` / `disabled` (adapter disabled or hardware switch) / `unknown`. No
  adapter → `null` ("No Bluetooth adapter found").
- **Switching** uses `Radio.SetStateAsync`, and only for radios that are `on` or `off`.
  `Radio.RequestAccessAsync` is called on the **first toggle**, not at startup, so nothing
  can prompt the user unprompted. A denial is cached, and the switch then turns into a
  Settings shortcut ("Change it in Settings").
- The switch never moves on its own: it shows "busy" while the request runs and reflects the
  `Radio.StateChanged` result.
- Paired Bluetooth devices are deliberately **not** enumerated. The panel links to Bluetooth
  settings.

### Display brightness

- **Built-in panels:** WMI `WmiMonitorBrightness` / `WmiSetBrightness`. On desktops, WMI has
  no instances (or returns `WBEM_E_NOT_SUPPORTED`), which is treated as "no built-in panel".
  WMI is secured per proxy (`CoSetProxyBlanket`), because process-wide
  `CoInitializeSecurity` belongs to the WebView2 host.
- **External monitors:** DDC/CI through the Monitor Configuration API. A monitor without
  DDC/CI, or with it switched off in its on-screen menu, fails `GetMonitorBrightness` and is
  counted as unsupported. The monitor's own range is mapped to 0–100.
- The panel distinguishes three cases:
  - Displays it can control, each with a slider.
  - **Not supported:** no display offers an interface. The panel says so and links to
    Display settings.
  - **Couldn't reach the displays:** a temporary error.
  - A display that stops answering shows "Not responding".
- Displays are detected at startup (on the display worker) and again whenever the Display
  panel opens (hot-plug). While the panel is open, brightness is re-read every 4 s.
- Brightness is not polled at any other time.

### Settings shortcuts

Allow-listed pages only. The frontend sends an enum and never a URI:

| Page      | URI                          |
| --------- | ---------------------------- |
| home      | `ms-settings:`               |
| wifi      | `ms-settings:network-wifi`   |
| network   | `ms-settings:network-status` |
| bluetooth | `ms-settings:bluetooth`      |
| sound     | `ms-settings:sound`          |
| display   | `ms-settings:display`        |
| battery   | `ms-settings:batterysaver`   |

After opening a page, the island collapses (Settings takes focus).

## UI

- **Quick controls** (toolbar): Network, Bluetooth, Sound, Display and System each open their
  panel inside the dashboard. The panels reuse the island's existing `focused` state and the
  same 820 × 248 frame, so the native window never resizes and no second overlay exists.
  Pressing the open panel's control closes it. The icons reflect live state (Wi-Fi off,
  Bluetooth off, muted/volume level), and the tooltips show it ("Sound · 46%"). The Settings
  button opens Windows Settings and is disabled, with an explanation, outside Windows.
- The **default dashboard is unchanged**: media, calendar, activity and the toolbar.
- **Peeks** for alerts use the existing peek state: only from collapsed or peek, never over
  an open dashboard, never taking keyboard focus. Repeats within 60 s are dropped again on
  the frontend.

## IPC

| Command                  | Arguments                                   | Notes                                |
| ------------------------ | ------------------------------------------- | ------------------------------------ |
| `connect_system`         | `channel: Channel<SystemUpdate>`            | returns `SystemSnapshot` atomically  |
| `get_system_snapshot`    | –                                           |                                      |
| `set_system_focus`       | `focus: "none" \| "overview" \| "display"`  | starts/stops optional polling        |
| `set_master_volume`      | `volume: number` (0–100, finite)            | async                                |
| `set_audio_muted`        | `muted: boolean`                            | async                                |
| `set_radio_state`        | `radio: "wifi" \| "bluetooth"`, `on`        | async; `denied` when Windows refuses |
| `set_display_brightness` | `displayId: "i<n>" \| "e<n>"`, `brightness` | async; ids validated                 |
| `open_system_settings`   | `page` (enum above)                         |                                      |

Errors are `{ kind, message }`, with kinds `unavailable`, `unsupported`, `notFound`,
`denied`, `timeout` and `platform`. The UI shows its own neutral wording and never the
message.

## Troubleshooting

- **The volume slider is disabled or says "No audio output device":** Windows has no active
  render endpoint. Check Sound settings.
- **Brightness "Not supported" on an external monitor:** enable DDC/CI in the monitor's
  on-screen menu. Some monitors, docks and adapters (many USB-C/DisplayLink) don't pass
  DDC/CI through at all.
- **The Wi-Fi/Bluetooth switch is missing:** Windows denied radio access, or the adapter is
  disabled or behind a hardware switch. Use the Settings link.
- **The network name is missing:** Windows did not expose a profile name. Recent Windows
  versions can restrict network names behind location privacy settings.
- Debug builds log every alert to stderr (`[islandy] system alert …`).
