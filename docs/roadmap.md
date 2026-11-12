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

## Phase 1: The island experience

- Install Motion and Lucide. Expand, collapse, and peek animations driven by the state model.
- Window resize presets (`PEEK`, `EXPANDED`) via `IslandWindow::apply_size`, with an atomic
  `SetWindowPos` helper and a grow-then-animate / animate-then-shrink sequence.
- Hover intent delay, click-to-focus, Escape and click-away to dismiss. Toggle focusable only
  in `focused` mode and re-apply `WS_EX_TOOLWINDOW`.
- Expanded dashboard shell (layout and tiles) with the date widget (local).
- Display-change handling (`WM_DISPLAYCHANGE`, work-area changes) and repositioning.
- Single-instance enforcement; start with Windows (opt-in).
- Windows benchmark pass (see `performance.md`).

## Phase 2: Media and audio

- GSMTC service: now playing, artwork, play/pause/next/previous, progress interpolation.
- Peek on track change.
- Core Audio: volume and mute with change notifications; read-only output device list.

## Phase 3: System and context

- Battery, network, CPU, and memory widgets (CPU and memory only while visible).
- Fullscreen and exclusive-fullscreen detection: hide on that monitor.
- Downloads-folder activity heuristic.

## Phase 4: Developer workflow

- Named-pipe event intake with an ACL and a reduced schema; a bundled `islandy-hook` helper.
- Claude Code integration (opt-in setup that shows a diff of the settings change); agent status
  peeks.
- Local event API for user scripts.

## Phase 5: Polish and release

- Settings surface (minimal), light accessibility pass (screen reader labels, reduced motion).
- Final identifier, icon, and branding; code signing; signed updater.
- Optional: ICS calendar subscription; MSIX packaging evaluation (would enable notification
  listener and appointments APIs).
