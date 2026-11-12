# Roadmap

Each phase ends with automated checks passing **and** verification on a real Windows 10 and
Windows 11 machine, including at least one non-100 % scaling setting and a multi-monitor setup.

## Phase 0: Foundation ✅ (Windows runtime verification pending)

- Tauri 2 + React + strict TypeScript + Tailwind v4 + Zustand scaffold, plus lint, format, and
  test tooling.
- Transparent, borderless, topmost, non-activating window with no taskbar entry, sized to the
  pill and DPI-aware top-center placement.
- Collapsed pill with a minute-aligned clock.
- Typed IPC (`get_app_info`) with least-privilege capabilities.
- Island state model (collapsed / peek / expanded / focused) as a pure, tested transition
  function.
- Design tokens; tray with Quit.
- Documentation: product, architecture, Windows integration, security, performance.

**Exit gate still open:** run the Windows verification checklist in `README.md` and record the
results.

## Phase 1: The island experience ✅ (Windows runtime verification pending)

- Motion and Lucide installed. Expand, collapse and peek animations (springs) driven by the
  state machine; reduced-motion support.
- Window frame coordinator: fixed native canvas + window region (`SetWindowRgn`), with a
  resize fallback using one atomic `SetWindowPos`. Grow-then-animate / animate-then-shrink.
- Monitor-fitted layouts (collapsed, peek, expanded) computed in Rust, DPI-aware, centered,
  top-anchored, clamped to the work area; re-read on every grow and on DPI changes.
- State machine: hover intent, delayed leave-collapse, click-to-pin, widget detail views,
  Escape, click-outside (focus loss), peeks with stale-timer protection. Timers derived from
  state.
- Focus: non-activating by default; focusable only after a click/shortcut; previous
  foreground window restored on keyboard dismiss.
- Global shortcut `Ctrl+Alt+Space` (official plugin, Rust-side only), conflict reported.
- Dashboard: toolbar with time/date and honest quick controls (open Windows Settings pages),
  media (demo), real calendar with month navigation, activity (demo).
- Accessibility basics: labels, visible focus rings, keyboard navigation, live regions.

**Exit gate still open:** run the Windows checklist in `README.md`.

**Moved to later phases:** display-change events beyond lazy re-fit (`WM_DISPLAYCHANGE`),
single-instance enforcement and start-with-Windows (Phase 5), Windows benchmark pass
(before Phase 2 sign-off).

## Phase 2: Native media ✅ (Windows runtime verification pending)

- GSMTC media service (`src-tauri/src/services/media/`): dedicated MTA worker, event-driven
  discovery of all sessions, stable session ids, selection policy with user override, metadata,
  artwork (WIC-transcoded, cached by content), playback state and capabilities, timeline.
- Real controls: play, pause, toggle, next, previous, seek, each checked against the app's
  enabled controls and confirmed by Windows (no optimistic UI). Timeouts on every native call.
- Revisioned, de-duplicated snapshots over a dedicated channel; stale responses ignored.
- UI: real media widget and detail view (all empty/loading/error/unsupported states), session
  switcher, artwork + playback indicator in the pill, peek on track change. Demo media removed.
- Tests: 42 Rust media tests (selection, identity, normalization, timeline, coalescing,
  commands, artwork, errors) and media store/model/UI tests in Vitest.

**Exit gate still open:** run the Media section of the Windows checklist in `README.md` with a
browser and at least one other player.

**Moved to Phase 3:** Core Audio volume/mute and output devices (they belong with the native
system controls).

## Phase 3: System controls and dashboard integration ✅ (Windows runtime verification pending)

- System service (`src-tauri/src/services/system/`):
  - Three MTA workers (audio, monitor, display) publishing per-domain, revisioned updates
    through one channel. Native callbacks only post messages.
  - All callbacks and event tokens are unregistered on exit.
- **Audio:**
  - Core Audio master volume (Windows' scalar scale) and mute.
  - Active output devices.
  - External changes, default-device changes and device add/remove via
    `IAudioEndpointVolumeCallback` and `IMMNotificationClient`.
- **Power:** `GetSystemPowerStatus` normalized (no fake 100 % on desktops) and refreshed by
  WinRT `PowerManager` events. Low/critical/plug/unplug alerts, once each.
- **Metrics:** CPU (`GetSystemTimes` deltas) and memory (`GlobalMemoryStatusEx`), sampled
  only while the System panel is open.
- **Network:** NCSI connectivity (internet / limited / local / offline), connection type and
  profile name; debounced lost/restored alerts.
- **Radios:** Wi-Fi and Bluetooth state; switching via `Radio.SetStateAsync` only where
  Windows grants access, otherwise a Settings link.
- **Brightness:** WMI for built-in panels and DDC/CI for monitors; unsupported and
  unreachable displays are reported as such.
- **Settings:** an allow-list of seven `ms-settings:` pages.
- **UI:**
  - Functional quick controls opening Sound, System, Network, Bluetooth and Display panels
    inside the existing `focused` state and frame.
  - Coalesced, latest-wins sliders that never jump back to stale values.
  - Non-optimistic toggles.
  - Peeks for system alerts.
- **Tests:** 45 Rust system tests (normalization, bounds, alert de-duplication, worker
  behavior with fake backends) and system store/model/panel tests in Vitest.

**Exit gate still open:** run the System section of the Windows checklist in `README.md` on a
laptop and a desktop, ideally with a DDC/CI monitor.

**Not done, by decision:** switching the default audio output. There is no documented API,
and `IPolicyConfig` was rejected. Paired Bluetooth device lists were left out deliberately.

**Moved to Phase 5:** fullscreen / exclusive-fullscreen detection (hide on that monitor).
**Dropped for now:** the downloads-folder activity heuristic (it would need file-system
watching that Phase 3 did not otherwise require).

## Phase 4: Smart activity engine and Claude Code ✅ (Windows runtime verification pending)

- Unified activity model (typed, no free-form metadata), deterministic Rust engine: P0–P3
  priorities, source-specific deduplication, lifecycles and expiry, bounded history
  (300 events / 7 days, `activity.json`).
- Local receiver: loopback TCP with a per-installation token and a line protocol instead of
  the named pipe planned earlier (trade-offs in claude-code-integration.md). Size and rate
  limits, closed schema.
- `islandy hook`: observational Claude Code bridge (always exits 0, prints nothing, forwards
  only allow-listed fields). Opt-in, reversible installation into user settings with backup and
  conflict detection.
- Claude Code sessions tracked independently, conservative status mapping, stale-session
  policy ("status unknown", never "done").
- `islandy notify`: build/tests/dev-server/message events. `islandy status`.
- Presentation coordinator (priority replacement, keyed queue, no interruptions), ambient pill
  layout, activity center with filters, dismiss and clear, Claude Code view with a session
  switcher, integrations panel, quiet mode.
- System alerts now flow through the activity engine.

**Verified on Linux with real Claude Code 2.1.322:** hook delivery, the working pill, the
finished peek, the CLI. **Exit gate still open:** the Activity section of the Windows checklist
in `README.md`.

**Not done, by decision:** remote approval of permissions, sending keystrokes to terminals,
"open terminal/project" (no verified target, and paths are not stored), terminal scraping,
process monitoring.

## Phase 5: Polish and release

- Quiet-mode scheduling; customizable peek durations.
- Settings surface (minimal: shortcut, top offset), accessibility audit with a screen reader.
- Single instance; start with Windows (opt-in).
- Final identifier, icon, and branding; code signing; signed updater.
- Optional: ICS calendar subscription; MSIX packaging evaluation (would enable notification
  listener and appointments APIs).
