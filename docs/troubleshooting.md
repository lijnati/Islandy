# Troubleshooting

Start with **Settings › Privacy › Copy diagnostic summary**. It lists versions, Windows version,
feature availability, integration status and recent warnings, and contains no activity, tokens
or file contents. Logs are in `%LOCALAPPDATA%\com.islandy.desktop\logs` (Settings › Privacy ›
Open folders › Logs). Turn on **Detailed logging** only while reproducing a problem.

| Symptom                                             | What to check                                                                                                                                                                                                                                                                  |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| I can't see the island                              | Is it hidden? Tray → **Show Islandy**, or press the shortcut. Is a fullscreen app on that monitor (Settings › General › Hide during fullscreen apps)? Was the selected display disconnected (it falls back to the primary display)?                                            |
| Nothing happens on hover                            | Settings › Island › Pointer: **Open on hover** may be off; click the island instead.                                                                                                                                                                                           |
| The shortcut does nothing                           | Settings › General shows whether it is registered. Another app may own it (Islandy never takes it over); record a different one. The tray icon always works.                                                                                                                   |
| The island is blank or shows a rectangle            | WebView2 or GPU issue with window regions. Quit, then start once with `ISLANDY_FRAME_STRATEGY=resize` set and report which works.                                                                                                                                              |
| Clicks near the island don't reach the window below | Only the visible island should take clicks. Report it with the diagnostic summary (strategy and scaling).                                                                                                                                                                      |
| No media shown                                      | The app must publish a Windows media session (Windows' own media flyout shows the same). Settings › Integrations › Windows › Media sessions.                                                                                                                                   |
| Brightness, Wi-Fi or Bluetooth controls missing     | They appear only where Windows supports them (WMI/DDC-CI brightness, radio access). Settings › Integrations shows what this PC offers.                                                                                                                                         |
| Claude Code sessions don't appear                   | Settings › Integrations › Claude Code: hooks installed? Restart Claude Code sessions that were running before. Check “Last event received”. Run `islandy status`. In `claude -p` runs the final event can be cut off. Folders must be trusted in Claude Code before hooks run. |
| "Settings file can't be read safely"                | `~/.claude/settings.json` isn't plain JSON (comments, trailing commas). Islandy won't edit it; add the snippet from docs/claude-code-integration.md by hand.                                                                                                                   |
| `islandy notify` exits 3                            | Islandy isn't running, or the local receiver couldn't start (Settings › Integrations › Developer events › Local receiver).                                                                                                                                                     |
| Too many or too few peeks                           | Settings › Notifications: per-source and per-outcome switches, quiet mode, duration. Each source peeks at most 4 times a minute.                                                                                                                                               |
| Settings reset unexpectedly                         | The settings window says so if `settings.json` was unreadable; the old file is kept as `settings.corrupt.json`.                                                                                                                                                                |
| Islandy doesn't start at sign-in                    | Settings › General › Launch at startup. If Task Manager › Startup apps disabled it, the switch is off with a note; turning it on re-enables it.                                                                                                                                |
| Windows "protected your PC"                         | The 0.1.0 installer is unsigned. See installation.md.                                                                                                                                                                                                                          |

## Clean reset

1. Tray → Quit Islandy.
2. Delete `%LOCALAPPDATA%\com.islandy.desktop` (settings, history, logs). Hooks in Claude Code
   are separate: remove them first in Settings › Integrations if you want them gone.
3. Start Islandy again: it behaves like a first launch.
