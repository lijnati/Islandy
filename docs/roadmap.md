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

## Phase 2: Media and audio

- GSMTC service replacing `src/features/media/demo-media.ts` (same shape: track, playing,
  position + timestamp): now playing, artwork, play/pause/next/previous.
- Peek on track change (the demo already does this through the same `CONTEXT_EVENT`).
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

- Settings surface (minimal: shortcut, top offset), accessibility audit with a screen reader.
- Single instance; start with Windows (opt-in).
- Final identifier, icon, and branding; code signing; signed updater.
- Optional: ICS calendar subscription; MSIX packaging evaluation (would enable notification
  listener and appointments APIs).
