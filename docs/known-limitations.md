# Known limitations

- **Windows runtime coverage is partial.** Islandy is developed largely on Linux and has been
  run by the maintainer on Windows, but the full [checklist](windows-verification.md) (scaling,
  multi-monitor, media players, radios, brightness, Claude Code under Git Bash/PowerShell) has
  not been completed on Windows 10 and 11.
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
  available. See [claude-code-integration.md](claude-code-integration.md).
- **Local receiver:** loopback TCP with a token, instead of the named pipe that earlier docs
  proposed (trade-offs in docs/claude-code-integration.md).
- **Media** shows only what each app publishes through Windows media sessions; see
  [media.md](media.md) for media-specific limitations.
- **System:**
  - The default audio output can't be switched from Islandy (no documented API).
  - Brightness depends on WMI (built-in panels) or DDC/CI (monitors), and many monitors,
    docks and adapters lack the latter.
  - Paired Bluetooth devices are not listed.
  - Radio switches depend on Windows granting access.
  - See [system.md](system.md).
- **12/24-hour format** follows the WebView language, not a custom Windows regional format.
- **Display changes:** detected by a 2 s watcher (no native event), so the pill can take up to
  2 s to move after a resolution, taskbar or monitor change.
- **`WS_EX_TOOLWINDOW` is fragile:** Tauri/tao rewrites extended styles when its own window flags
  change; `IslandWindow` re-applies it (see `src-tauri/src/platform/windows.rs`).
- **Fullscreen detection** polls every 2 s (cheap Win32 calls) and compares the foreground window
  with the island's monitor; see [windows-integration.md](windows-integration.md).
- **Frost** uses translucency without blurring the desktop (no native acrylic).
- **Unsigned, no auto-update:** SmartScreen warns; updates are manual (docs/release.md).
- **Windows session end** terminates Islandy without its shutdown path; the last ~1.5 s of activity
  history may be lost.
- **Idle memory on Windows** is unmeasured; the WebView is the dominant cost.
- **Linux** is a development host only; transparency there needs a compositor, and only the
  input region is applied.
- **Weather** needs a place chosen by hand (Islandy doesn't use Windows location services or IP
  geolocation). Forecast quality is Open-Meteo's.
- **Updates** install only from full GitHub releases (not pre-releases), need the signing secrets
  in CI, and are installed while idle only on Windows (elsewhere: on request).
