# Islandy

A lightweight, context-aware floating island for Windows 10/11.

Islandy sits at the top center of the desktop as a compact black pill and, in later phases,
expands into a small dashboard for music, time, calendar, system status and developer activity.
It is built to feel like a native OS component: no taskbar entry, no focus stealing, near-zero
idle cost, fully local.

## Status

**Phase 0 — Foundation.** What exists today:

- A borderless, transparent, always-on-top window with no taskbar entry, sized to the pill
  (220 × 44 logical px) and centered at the top of the primary monitor's work area, DPI-aware.
- A minimal pill showing the local time and short date, updated once per minute.
- A typed Rust ↔ React IPC bridge (`get_app_info`), locked down by a least-privilege capability.
- A Zustand store defining the island's state model (`collapsed`, `peek`, `expanded`, `focused`).
  Only `collapsed` is rendered.
- A tray icon with **Quit Islandy** (the island never takes focus, so this is how you quit).

Not built yet: expansion/animation, widgets, media, audio, calendar, settings, autostart,
fullscreen handling. See [docs/roadmap.md](docs/roadmap.md).

> **Verification note.** Phase 0 was developed on a Linux host. The Windows code paths compile
> and pass `clippy` for `x86_64-pc-windows-msvc`, but Windows-native runtime behavior (WebView2
> transparency, `WS_EX_NOACTIVATE`/`WS_EX_TOOLWINDOW`, Alt+Tab, DPI on real monitors) has **not**
> yet been verified on a Windows machine. See [Known limitations](#known-limitations).

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

Motion and Lucide React are planned for Phase 1 (animation and icons). They are intentionally
not installed yet because nothing in Phase 0 uses them. TypeScript is pinned to 6.0.x because
typescript-eslint does not yet support TypeScript 7.

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
  features/island/        the island UI (Island, Clock, formatters)
  hooks/                  useClock (minute-aligned), useAppInfo
  services/               native.ts: the only module that calls Rust; types.ts: IPC types
  store/                  island.ts: Zustand store + pure transition model
  design/, styles/        design tokens (CSS @theme + small TS mirror)
src-tauri/                Rust backend
  src/app.rs              lifecycle: builder, setup, tray, window events
  src/window/             geometry.rs (pure, tested) + manager.rs (IslandWindow)
  src/platform/           isolated OS code: windows.rs (Win32), linux.rs (dev host)
  src/commands.rs         IPC surface
  src/services/           app_info.rs
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

- [ ] The pill appears at the top center of the primary monitor, 8 px below the work area top,
      with no white flash, border, shadow, or visible window rectangle.
- [ ] Correct size and position at 100 %, 125 %, 150 % and 175 % scaling. Change scaling while
      the app runs: the pill re-centers.
- [ ] It stays above normal and maximized windows.
- [ ] No taskbar button, and absent from Alt+Tab and Task View.
- [ ] Clicking the pill while typing in another app does **not** move focus or the caret.
- [ ] Clicks immediately beside the pill reach the window underneath.
- [ ] The clock is correct and rolls over on the minute, including after sleep/resume.
- [ ] Hover tooltip shows `Islandy 0.1.0` (IPC `get_app_info` succeeded). Under `tauri dev`,
      the terminal logs `[islandy] get_app_info -> …`.
- [ ] Tray icon → **Quit Islandy** exits; no `islandy.exe` or orphaned `msedgewebview2.exe`
      remains.
- [ ] Multi-monitor: the pill sits on the primary monitor, including when the primary is
      not the leftmost.

## Known limitations

- **Windows behavior unverified at runtime** (developed on Linux; see note above).
- **Click-through at rounded corners:** the native window is exactly the pill's bounding box, so
  the few transparent pixels outside the rounded corners still receive clicks.
- **Keyboard input:** the window is non-focusable (`WS_EX_NOACTIVATE`), so it cannot receive
  keyboard input. Phase 1 must make it focusable only in the `focused` state.
- **12/24-hour format** follows the WebView language, not a custom Windows regional format.
- **Display changes:** repositioning happens at startup and on DPI changes; resolution changes,
  monitor hot-plug, and taskbar moves are not handled yet.
- **`WS_EX_TOOLWINDOW` is fragile:** Tauri/tao rewrites extended styles when its own window flags
  change; the window manager must re-apply it (see `src-tauri/src/platform/windows.rs`).
- **Single instance:** launching twice shows two islands (planned with autostart).
- **Fullscreen apps:** the island stays on top of them; see `docs/windows-integration.md`.
- **Linux** is a development host only; transparency there needs a compositor.
