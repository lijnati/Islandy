# Islandy: Product

## Vision

Islandy is a beautiful, lightweight, context-aware floating island for Windows. It lives at the
top center of the desktop and gives quick access to relevant information and controls without
interrupting the user's work.

The visual reference is a phone's dynamic island combined with a premium floating control
center. The goal is for it to feel like part of the operating system, not a third-party widget.

## Target users

- **Knowledge workers and creators** who listen to music while working and want playback, time
  and calendar at a glance without switching windows.
- **Developers** who run long tasks (builds, AI coding agents such as Claude Code, downloads)
  and want ambient status without notification spam.
- **Design-conscious Windows users** who want a polished desktop and would not tolerate a
  heavy, ugly, or resource-hungry utility.

## MVP scope

| Area           | MVP                                                                    |
| -------------- | ---------------------------------------------------------------------- |
| Resting pill   | Time; a contextual glyph when something is active (e.g. music playing) |
| Expanded panel | Opens on hover or click; closes on leave, Escape, or click-away        |
| Media          | Now playing (title, artist, artwork), play/pause, next/previous        |
| Audio          | Output volume and mute                                                 |
| Date           | Today's date and a month view (local only; no accounts)                |
| System         | Battery, CPU, memory, network status (shown only when expanded)        |
| Activity       | Peeks for Claude Code agent status (opt-in, local)                     |
| App            | Tray menu, start with Windows, minimal settings                        |

## Core interactions

| State       | Trigger                                       | Ends when                                   |
| ----------- | --------------------------------------------- | ------------------------------------------- |
| `collapsed` | Default / resting                             | n/a                                         |
| `peek`      | A contextual event (track change, agent done) | Dwell timeout (~4 s), or the user interacts |
| `expanded`  | Pointer hover (with intent delay)             | Pointer leaves                              |
| `focused`   | Click                                         | Escape, click-away, or focus loss           |

Rules: the user always wins over automatic behavior; automatic events never interrupt an
expanded or focused island; only one transition animates at a time.

## Non-goals

- Replacing the Windows taskbar, Start menu, or notification center.
- Reading other applications' notifications (not possible for unpackaged apps; see
  `windows-integration.md`).
- Cloud accounts, sync, telemetry, analytics, or ads.
- A plugin marketplace or arbitrary third-party code execution.
- macOS or Linux as supported targets (Linux is a development host only).
- Heavy theming. A small number of polished choices is better than many settings.

## Product principles

1. Beautiful by default.
2. Lightweight and resource-efficient.
3. Native-feeling Windows behavior.
4. Context-aware, not notification-heavy.
5. Offline-first.
6. Modular and extensible.
7. Minimal configuration.
8. Privacy-respecting.
9. No unnecessary background processes.
10. No premature abstractions.
