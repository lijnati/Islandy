# Islandy

A lightweight, context-aware floating island for Windows 10/11.

Islandy sits at the top center of the desktop as a compact black pill and, in later phases,
expands into a small dashboard for music, time, calendar, system status and developer activity.
It is built to feel like a native OS component: no taskbar entry, no focus stealing, near-zero
idle cost, fully local.

## Status

**Phase 1: Dynamic island experience.** What exists today:

- A black pill at the top center of the primary monitor (time, ambient glyph, media
  indicator) that expands with a spring animation into an ~820 × 248 dashboard on hover
  (150 ms intent delay), click, or **Ctrl + Alt + Space**.
- Dashboard: toolbar (time, date, quick controls, pin, collapse) and three widgets:
  **media** (demo data), **calendar** (real, locale-aware, with month navigation), and
  **activity** (demo data). Each widget has a detail view.
- Interaction: hover-to-expand, delayed leave-collapse, click-to-pin, Escape, click outside
  (focus loss), peeks for contextual events (tray → _Preview peek (demo)_).
- Native frame coordination: the window is a fixed canvas and a window region limits drawing
  and clicks to the visible island, so nothing invisible blocks the desktop. Layouts are fitted
  to the monitor (DPI-aware, narrower on small screens).
- Quick controls open the Windows Settings page for Wi-Fi, Bluetooth, sound and display; they
  do not pretend to toggle anything. Islandy settings are marked "coming soon".

**Demo data.** The media and activity widgets show local sample data, labelled _Demo_. They
are not connected to Windows media sessions (Phase 2) or Claude Code (Phase 4).

Not built yet: real media/audio, system widgets, settings, autostart, single instance,
fullscreen handling. See [docs/roadmap.md](docs/roadmap.md).

> **Verification note.** Islandy was developed on a Linux host. The Windows code paths compile
> and pass `clippy` for `x86_64-pc-windows-msvc`, and a Windows installer cross-builds, but
> Windows-native runtime behavior (WebView2 transparency and window regions, focus handling,
> Alt+Tab, the global shortcut, Settings pages, DPI on real monitors) has **not** been verified
> on a Windows machine. See the checklist below.

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
  components/widgets/     Media, Calendar (+ MonthGrid), Activity (+ ActivityCard), details
  components/ui/          IconButton, Tooltip, WidgetSurface, StatusIndicator
  features/island/        island-store (state machine + timers), window-coordinator,
                          layouts, useIslandFrame, useNativeBridge, native-status, format
  features/calendar/      calendar math (pure, tested)
  features/media/         demo media store (Phase 2 replaces it)
  features/activity/      demo activity data (Phase 4 replaces it)
  hooks/                  useClock (minute-aligned), useNow, usePrefersReducedMotion
  services/               native.ts: the only module that calls Rust; types.ts: IPC types
  design/, styles/        design tokens (CSS @theme + small TS mirror)
src-tauri/                Rust backend
  src/app.rs              lifecycle: builder, setup, tray, window events, shutdown
  src/window/             geometry.rs (pure, tested) + manager.rs (IslandWindow, frame)
  src/platform/           isolated OS code: windows.rs (Win32), linux.rs (dev host)
  src/commands.rs         IPC surface
  src/signals.rs          Rust → UI channel
  src/shortcut.rs         global toggle shortcut
  src/services/           app_info.rs, system_settings.rs
  src/error.rs            serializable error type
  capabilities/           least-privilege permissions
docs/                     product, architecture, Windows integration, security,
                          performance, roadmap
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
- [ ] Quick controls open the Wi-Fi, Bluetooth, Sound and Display Settings pages, and the
      island collapses.
- [ ] Opening a widget (click its title) shows the detail view; Back and Escape return.
- [ ] Tray → **Preview peek (demo)** shows a peek that disappears after ~4 s.
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
- **Contextual events while the dashboard is open are dropped**, not queued.
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
