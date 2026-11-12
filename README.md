# Islandy

A lightweight, context-aware floating island for Windows 10/11.

Islandy sits at the top center of the desktop as a compact black pill and, in later phases,
expands into a small dashboard for music, time, calendar, system status and developer activity.
It is built to feel like a native OS component: no taskbar entry, no focus stealing, near-zero
idle cost, fully local.

## Status

**Phase 4: Smart activity engine and Claude Code integration.** What exists today:

- A black pill at the top center of the primary monitor (time, ambient glyph, media
  indicator) that expands with a spring animation into an ~820 × 248 dashboard on hover
  (150 ms intent delay), click, or **Ctrl + Alt + Space**.
- Dashboard: toolbar (time, date, quick controls, pin, collapse) and three widgets:
  **media** (real Windows media sessions), **calendar** (real, locale-aware, with month
  navigation), and **activity** (real activity, Phase 4). Each widget has a detail view.
- **Media (Phase 2):** any app that publishes a Windows media session (browsers, Spotify,
  Media Player, …) is detected automatically through GSMTC. Title, artist, album and artwork;
  play/pause, previous/next and seeking where the app supports them; a session switcher when
  several apps play; the collapsed pill shows the artwork and a playback indicator, and a new
  track shows a brief peek. See [docs/media.md](docs/media.md).
- Interaction: hover-to-expand, delayed leave-collapse, click-to-pin, Escape, click outside
  (focus loss), peeks for contextual events (tray → _Preview peek (demo)_).
- Native frame coordination: the window is a fixed canvas and a window region limits drawing
  and clicks to the visible island, so nothing invisible blocks the desktop. Layouts are fitted
  to the monitor (DPI-aware, narrower on small screens).
- **System (Phase 3):** the toolbar's quick controls open compact panels inside the dashboard:
  - **Sound:** the real master volume (Core Audio), mute, and the active output devices,
    with a link to Sound settings for switching outputs.
  - **System:** CPU, memory and battery. Sampling runs only while this panel is open.
  - **Network:** internet connectivity as Windows assesses it, and the Wi-Fi radio.
  - **Bluetooth:** the Bluetooth radio.
  - **Display:** brightness for built-in panels (WMI) and DDC/CI monitors.
  - Radio switches appear only where Windows allows them; otherwise the panels link to the
    matching Settings page.
  - Brief peeks for low battery, power changes, network loss and restore, and audio output
    changes.
  - See [docs/system.md](docs/system.md).

- **Activity (Phase 4):**
  - **Claude Code (opt-in):** Islandy adds observational hooks to your user settings, after a
    confirmation step, keeping a backup and every other hook. Each session is tracked
    separately: working, needs attention, waiting for input, finished responding, error, ended.
    It shows no progress estimates, offers no remote approval, and never stores prompts,
    commands, file contents or paths.
  - **Pill and peeks:** a wider pill shows "Islandy · Working", "3 sessions · 1 needs you" or
    "Islandy · Building". Attention keeps an amber indicator until it is answered or dismissed.
    Peeks are prioritized (P0–P3), deduplicated, queued, and never interrupt the open dashboard.
  - **Developer events:** `islandy notify build failed`, `islandy notify tests passed`, …
    through the same authenticated local receiver.
  - **Activity center:** recent events with source filters, dismiss and clear history. History
    is kept locally for 7 days (at most 300 events).
  - **Quiet mode:** no automatic peeks, with indicators and history kept. Each source can be
    switched off.
  - See [docs/activity-engine.md](docs/activity-engine.md),
    [docs/claude-code-integration.md](docs/claude-code-integration.md) and
    [docs/privacy.md](docs/privacy.md).

Not built yet: Islandy's own settings, autostart, single instance, fullscreen handling,
switching the default audio output (no documented Windows API). See [docs/roadmap.md](docs/roadmap.md).

> **Verification note.** Islandy was developed on a Linux host. The Windows code paths compile
> and pass `clippy` for `x86_64-pc-windows-msvc`, and a Windows build and installer
> cross-compile (Phase 2: with the MinGW toolchain, `--target x86_64-pc-windows-gnu`), but
> Windows-native runtime behavior (WebView2 transparency and window regions, focus handling,
> Alt+Tab, the global shortcut, Settings pages, DPI on real monitors, **all GSMTC media
> behavior with real players**, and **all Phase 3 system integration: Core Audio, battery,
> network, radios, brightness**, and **all Phase 4 Windows paths: hook execution under Git
> Bash/PowerShell, the loopback receiver and firewall, `%LOCALAPPDATA%` files**) has **not**
> been verified on a Windows machine. The Phase 4 pipeline _was_ verified end to end on Linux
> with the real Claude Code 2.1.322 (see docs/claude-code-integration.md → Verification). See
> the checklist below.

## Tech stack

| Layer    | Choice                                                                   |
| -------- | ------------------------------------------------------------------------ |
| Shell    | Tauri 2 (WebView2 on Windows)                                            |
| Backend  | Rust (edition 2021), `windows` crate for isolated Win32 calls            |
| Frontend | React 19, TypeScript 6 (strict), Vite 8                                  |
| Styling  | Tailwind CSS v4 (CSS-first tokens), Geist Variable bundled locally (OFL) |
| State    | Zustand 5                                                                |
| Tooling  | pnpm, ESLint 10 + typescript-eslint (strict, type-checked), Prettier     |
| Tests    | Vitest + Testing Library (frontend), `cargo test` (Rust)                 |

Animation uses Motion (`motion/react`), icons use Lucide React, and the global shortcut uses
the official `tauri-plugin-global-shortcut` (Rust side only). TypeScript is pinned to 6.0.x
because typescript-eslint does not yet support TypeScript 7.

## Prerequisites (Windows)

1. **Microsoft C++ Build Tools** with the "Desktop development with C++" workload.
2. **WebView2 Runtime**: preinstalled on Windows 11 and current Windows 10.
3. **Rust** (stable, ≥ 1.80) via [rustup](https://rustup.rs), MSVC toolchain
   (`x86_64-pc-windows-msvc`).
4. **Node.js** ≥ 20.19 and **pnpm** 10 (`corepack enable` then `corepack prepare pnpm@10 --activate`).

See the official [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for details.
On Linux (development only) you also need `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
`libayatana-appindicator3-dev`, `librsvg2-dev` and `libxdo-dev`.

## Install

```sh
pnpm install
```

## Develop

```sh
pnpm tauri dev      # run the desktop app with hot reload (starts Vite automatically)
pnpm dev            # frontend only, in a browser (IPC calls fail gracefully)
```

In the browser dev server, `window.__islandy.dispatch({ type: "CONTEXT_EVENT", peek: … })`
drives the state machine (dev builds only), e.g. to preview a peek.

### Configuration (environment variables, until a settings UI exists)

| Variable                  | Default          | Effect                                                |
| ------------------------- | ---------------- | ----------------------------------------------------- |
| `ISLANDY_TOGGLE_SHORTCUT` | `Ctrl+Alt+Space` | Global toggle shortcut (e.g. `Ctrl+Shift+I`)          |
| `ISLANDY_FRAME_STRATEGY`  | `region`         | `resize` resizes the window instead of using a region |

The top offset (8 px) is `TOP_MARGIN` in `src-tauri/src/window/geometry.rs`.

## Check

```sh
pnpm typecheck      # TypeScript, strict
pnpm lint           # ESLint (type-checked rules), zero warnings allowed
pnpm test           # Vitest
pnpm lint:rust      # cargo clippy -D warnings
pnpm test:rust      # cargo test
pnpm format:check   # Prettier + rustfmt
pnpm check          # all of the above
```

## Build

```sh
pnpm build          # typecheck + production frontend bundle (dist/)
pnpm tauri build    # release binary + NSIS installer (per-user, no admin) on Windows
```

The release binary is at `src-tauri/target/release/islandy.exe`, the installer under
`src-tauri/target/release/bundle/nsis/`.

## Project layout

```
src/                      React frontend
  components/island/      IslandRoot (animated surface), Collapsed/Peek/ExpandedIsland,
                          IslandToolbar, QuickControls, ActivityBars, NowProvider
  components/widgets/     Media (+ SessionSwitcher, Artwork), Calendar (+ MonthGrid), details
  components/system/      Audio, System, Network, Bluetooth and Display panels
  components/activity/    ActivityWidget, ActivityCenter, ClaudePanel (session switcher),
                          IntegrationsPanel, ActivityRow, ClaudeGlyph
  components/ui/          IconButton, Tooltip, WidgetSurface, StatusIndicator, LevelSlider,
                          Switch, MeterBar
  features/island/        island-store (state machine + timers), window-coordinator,
                          layouts, useIslandFrame, useNativeBridge, native-status, format
  features/calendar/      calendar math (pure, tested)
  features/media/         media store (native snapshots, commands, artwork) + pure model
  features/system/        system store (per-domain updates, slider coalescing, alerts) +
                          pure model
  features/activity/      activity store (snapshots, peeks, integration), pure model,
                          presentation coordinator (peek priority/queue), tray demo peek
  hooks/                  useClock (minute-aligned), useNow, usePrefersReducedMotion
  services/               native.ts: the only module that calls Rust; types.ts: IPC types
  design/, styles/        design tokens (CSS @theme + small TS mirror)
src-tauri/                Rust backend
  src/app.rs              lifecycle: builder, setup, tray, window events, shutdown
  src/window/             geometry.rs (pure, tested) + manager.rs (IslandWindow, frame)
  src/platform/           isolated OS code: windows.rs (Win32), linux.rs (dev host)
  src/commands.rs         IPC surface
  src/cli.rs              `islandy hook` (Claude Code bridge), `notify`, `status`
  src/signals.rs          Rust → UI channel
  src/shortcut.rs         global toggle shortcut
  src/services/           app_info.rs, winrt.rs (shared WinRT helpers), media/ (GSMTC
                          worker, see docs/media.md), system/ (audio, power, network,
                          radios, metrics, brightness, settings; see docs/system.md),
                          activity/ (engine, receiver, protocol, store, claude_settings;
                          see docs/activity-engine.md), paths.rs (per-user data dir)
  src/error.rs            serializable error type
  capabilities/           least-privilege permissions
docs/                     product, architecture, media, system, activity engine, Claude Code
                          integration, privacy, Windows integration, security, performance,
                          roadmap
```

## Identifiers to finalize before release

| Item              | Current (temporary)    | Where                                          |
| ----------------- | ---------------------- | ---------------------------------------------- |
| Bundle identifier | `com.islandy.desktop`  | `src-tauri/tauri.conf.json` `identifier`       |
| Publisher/authors | `Islandy contributors` | `src-tauri/Cargo.toml`                         |
| License           | `UNLICENSED`           | `src-tauri/Cargo.toml`                         |
| App icon          | placeholder pill mark  | `src-tauri/icons/icon.svg` → `pnpm tauri icon` |

The identifier determines the app-data directory and Windows installer registry keys; changing it
after release orphans user data, so finalize it before the first public build. Code signing is
not configured.

## Windows verification checklist

Run `pnpm tauri dev` (and later the installed release build) on Windows 10 and 11:

**Window (Phase 0)**

- [ ] The pill appears at the top center of the primary monitor, 8 px below the work area top,
      with no white flash, border, shadow, or visible window rectangle.
- [ ] Correct size and position at 100 %, 125 %, 150 % and 175 % scaling. Change scaling while
      the app runs: the island re-centers.
- [ ] It stays above normal and maximized windows.
- [ ] No taskbar button, and absent from Alt+Tab and Task View (also after pinning and
      collapsing, which toggles focusability).
- [ ] Tray icon → **Quit Islandy** exits; no `islandy.exe` or orphaned `msedgewebview2.exe`
      remains. The shortcut is free again afterwards.
- [ ] Multi-monitor: the island sits on the primary monitor, including when the primary is
      not the leftmost.

**Frame and click-through (Phase 1, highest risk)**

- [ ] The island renders normally (not blank) with the default region strategy. If it is
      blank or shows artifacts, retry with `ISLANDY_FRAME_STRATEGY=resize` and record which works.
- [ ] Collapsed: clicks immediately beside and below the pill reach the window underneath.
- [ ] Expanded: clicks just outside the dashboard reach the window underneath.
- [ ] Expanding and collapsing: the island stays horizontally centered and top-anchored, with
      no sideways jump, flicker, or white frame.

**Interaction (Phase 1)**

- [ ] Resting the pointer on the pill expands it after a short delay; brushing past does not.
- [ ] Moving away collapses it after ~0.4 s; moving back in time keeps it open.
- [ ] Hovering and clicking media controls while typing in another app does **not** move the
      caret or focus.
- [ ] Clicking the pill pins it: it stays open after the pointer leaves; Tab moves focus
      through controls with a visible focus ring; Escape collapses it and focus returns to the
      previous app.
- [ ] Clicking another window while pinned collapses the island.
- [ ] **Ctrl + Alt + Space** toggles the island from any app. With another app already owning
      the shortcut, Islandy logs the conflict and does not take it over.
- [ ] Quick controls open their panels in place; pressing the open one closes it. The
      Settings button opens Windows Settings and the island collapses.
- [ ] Opening a widget (click its title) shows the detail view; Back and Escape return.
- [ ] Tray → **Preview peek (demo)** shows a peek that disappears after ~4 s.

**Media (Phase 2)** — record app name and version for each; if an app lacks a capability
(e.g. no seeking), note it as the app's limitation, not an Islandy failure.

- [ ] Browser (Edge or Chrome) playing a YouTube video: detected within ~1 s; title, channel,
      artwork; play/pause, next/previous (on a playlist), seeking.
- [ ] A second player (Spotify, Media Player, VLC with SMTC enabled, …): same checks.
- [ ] Pause and resume from the app: the pill's bars stop/start; the widget's button flips.
- [ ] Pause/resume, next/previous from Islandy: the app reacts; Islandy's state changes only
      after the app does.
- [ ] Track change (auto-advance and skip): metadata and artwork update, a peek appears while
      collapsed, no stale artwork.
- [ ] Seeking by click/drag and arrow keys lands where expected; unsupported → no slider.
- [ ] Two players at once: the switcher lists both; a manual choice sticks while the other
      starts/stops; "Automatic" follows Windows' current session again.
- [ ] Close the player while it plays: the session disappears; the island returns to idle (or
      to the other session) without errors.
- [ ] Quit Islandy while media plays: it exits promptly.
- [ ] Unicode and very long titles render and truncate cleanly.
      **System (Phase 3)**. Record the hardware for each check (laptop/desktop, monitor model,
      adapters).

- [ ] Sound: the panel shows the default output and the same volume as the taskbar flyout.
      Dragging the slider changes Windows' volume smoothly. Changing volume with keyboard keys
      or the flyout moves Islandy's slider. Mute/unmute both ways.
- [ ] Sound: switching the default output (Settings, or plugging in headphones) updates the
      panel and shows an "Audio output changed" peek; unplugging the last device shows "No
      audio output device".
- [ ] Battery (laptop): the percentage and charge state match the Windows battery flyout.
      Unplug/plug shows "On battery" / "Power connected" once. Low battery (≤ 20 %) peeks
      once.
- [ ] Desktop without battery: the System panel says "No battery", never a percentage.
- [ ] CPU/RAM roughly match Task Manager; sampling stops when the panel closes (Islandy's CPU
      returns to idle).
- [ ] Network: disconnecting Wi-Fi/Ethernet shows "Offline" and, after ~3 s, a "Network
      disconnected" peek; reconnecting shows "Connected to …". A network without internet
      shows "No internet".
- [ ] Wi-Fi/Bluetooth switches: either they work (Windows' own quick settings follow), or
      they are absent with "Change it in Settings". Note whether Windows prompted for access.
- [ ] Brightness: works on a laptop panel; works on a DDC/CI monitor (or reports "Not
      supported" honestly); multi-monitor setups list each controllable display.
- [ ] Settings links open Sound, Network, Wi-Fi, Bluetooth, Display, Power & battery and
      Settings home.
- [ ] Quitting with the System or Display panel open exits promptly.
- [ ] Idle CPU with music playing and the island collapsed stays near the idle budget
      (see `docs/performance.md`).
      **Activity (Phase 4)**. Note the Claude Code version and the shell hooks run under (Git
      Bash or PowerShell).

- [ ] Integrations → Claude Code → Enable → Add hooks: `~/.claude/settings.json` gains
      Islandy's 12 handlers, other hooks are untouched, a `settings.islandy-backup-*.json`
      appears. Remove hooks restores the previous content.
- [ ] A new interactive Claude Code session: the pill shows "<project> · Working" during a
      turn and a "Claude finished responding" peek at the end. No console window flashes.
- [ ] A permission prompt in Claude Code: an amber "Needs attention · Permission requested"
      peek, then the amber pill indicator until the prompt is answered.
- [ ] Two sessions in the same folder: both listed separately in the Claude Code view.
- [ ] Quit Islandy while Claude Code runs: Claude Code is unaffected (no errors, no delay);
      restart Islandy: sessions reappear on their next event, nothing stale is shown.
- [ ] No Windows Firewall prompt when Islandy starts (the receiver binds 127.0.0.1 only).
- [ ] PowerShell: `& "$env:LOCALAPPDATA\Islandy\islandy.exe" notify build failed | Out-Null`
      peeks "Build failed"; `status | Write-Output` prints "running".
- [ ] Quiet mode (moon): no peeks; the pill shows the moon; attention still turns the pill amber.
- [ ] With Windows _Animation effects_ turned off, transitions are instant.
- [ ] The clock and calendar are correct and roll over at the minute and midnight, including
      after sleep/resume.

## Known limitations

- **Windows behavior unverified at runtime** (developed on Linux; see note above). In
  particular, window regions with a transparent WebView2 window are untested.
- **Click-through at rounded corners:** the region is the island's bounding rectangle, so the
  few transparent pixels outside the rounded corners still receive clicks.
- **During a collapse animation** (~250 ms) the clickable frame is still the larger size.
- **Hover-expanded is not keyboard-interactive:** the island only takes focus once pinned
  (click, widget, shortcut). Escape and click-outside therefore apply to the pinned state;
  an unpinned dashboard closes when the pointer leaves.
- **Contextual events while the dashboard is open are not shown as peeks.** They are recorded
  in the activity center, and Claude Code attention keeps its pill indicator.
- **Claude Code:** status is what the hooks report. Missed events show "Status unknown", never
  "done". In non-interactive `claude -p` runs, the final async `Stop` hook can be cut off when
  the process exits. Opening the right terminal isn't possible, because no verified target is
  available. See [docs/claude-code-integration.md](docs/claude-code-integration.md).
- **Local receiver:** loopback TCP with a token, instead of the named pipe that earlier docs
  proposed (trade-offs in docs/claude-code-integration.md). A second Islandy instance takes
  over the receiver.
- **Media** shows only what each app publishes through Windows media sessions; see
  [docs/media.md](docs/media.md) for media-specific limitations.
- **System:**
  - The default audio output can't be switched from Islandy (no documented API).
  - Brightness depends on WMI (built-in panels) or DDC/CI (monitors), and many monitors,
    docks and adapters lack the latter.
  - Paired Bluetooth devices are not listed.
  - Radio switches depend on Windows granting access.
  - See [docs/system.md](docs/system.md).
- **12/24-hour format** follows the WebView language, not a custom Windows regional format.
- **Display changes:** the island re-fits on the next interaction and on DPI changes; a
  resolution or taskbar change does not move the resting pill immediately.
- **`WS_EX_TOOLWINDOW` is fragile:** Tauri/tao rewrites extended styles when its own window flags
  change; `IslandWindow` re-applies it (see `src-tauri/src/platform/windows.rs`).
- **Configuration** is by environment variable only (no settings UI).
- **Single instance:** launching twice shows two islands (planned with autostart).
- **Fullscreen apps:** the island stays on top of them; see `docs/windows-integration.md`.
- **Linux** is a development host only; transparency there needs a compositor, and only the
  input region is applied.
