# Privacy

Islandy is local software. It has no account, no analytics, no telemetry and no cloud service.
This page lists what it reads, keeps and sends. It covers Phases 0–4.

## Never collected

- Claude Code prompts, assistant responses, tool arguments (`tool_input`), tool output
  (`tool_response`), notification message text, error details, transcript paths or transcripts.
- Source code, file contents, file names or full paths.
- Environment variables, API keys, tokens or terminal history.
- Keystrokes or screen contents. Islandy doesn't scrape terminals or inject into processes.

## What each feature uses

| Feature                       | Read                                                                                                                                                    | Kept                                                                                                                               |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Media (Phase 2)               | Title, artist, album, artwork and playback state from Windows media sessions                                                                            | In memory only; never logged in release builds                                                                                     |
| System (Phase 3)              | Volume, battery, connectivity, radio state, CPU/RAM, brightness; the Wi-Fi profile name and audio device names Windows reports                          | In memory. Network names and audio device names appear in activity history when an alert mentions them (see below)                 |
| Claude Code (Phase 4, opt-in) | Hook event name, a hash of the session id, the _last folder name_ of the working directory, a tool category, a reduced notification type and error type | Session state in memory; history events (kind, title written by Islandy, folder name, opaque session key, time) in `activity.json` |
| Developer events              | What you pass to `islandy notify` (event, project, label, message title)                                                                                | History events in `activity.json`                                                                                                  |

## Local files

All in your per-user data directory: `%LOCALAPPDATA%\com.islandy.desktop` on Windows,
`~/.local/share/com.islandy.desktop` on Linux.

| File                    | Contents                                                                       | Lifetime                                                        |
| ----------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| `activity.json`         | Up to 300 activity events from the last 7 days; quiet mode and source switches | Retention enforced on every write; **Clear history** deletes it |
| `receiver.json`         | The local receiver's port and the per-installation token                       | Rewritten at start, removed on exit                             |
| `activity.corrupt.json` | A history file that couldn't be read, moved aside                              | Deleted by **Clear history**                                    |

Claude Code's settings file is changed only when you press **Add hooks** or **Remove hooks**. A
timestamped backup (`settings.islandy-backup-*.json`, newest 5) is kept next to it in
`~/.claude`.

## Network

The only listener is the local receiver on `127.0.0.1`, reachable only from this machine and
only with the token. Islandy makes no network requests of its own. Network status comes from
Windows. (The installer may download WebView2 if it's missing; that's Microsoft's bootstrapper.)

## Your controls

| Control                                                                    | Where                                                                                    |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Enable or remove the Claude Code hooks                                     | Activity → ⚙ → Integrations → Claude Code                                                |
| Turn individual sources off (Claude Code, builds & scripts, system alerts) | Integrations → Sources. Off means not recorded at all, and Claude sessions are forgotten |
| Dismiss one event / clear all history                                      | Activity center (× on a row; trash → **Clear all history**)                              |
| Quiet mode (no automatic peeks)                                            | Moon button in the dashboard toolbar, or Integrations                                    |
| Turn off every Claude Code hook                                            | `"disableAllHooks": true` in Claude Code settings                                        |

Uninstalling Islandy doesn't remove the Claude Code hooks: press **Remove hooks** first. If
Islandy is already gone, the hooks point at a missing program and Claude Code may report hook
errors. Remove them by hand (see
[claude-code-integration.md](claude-code-integration.md#manual-setup)).

## Logging

Release builds log errors only, without activity content. Debug builds also log each received
activity event's _name_ and opaque session key to stderr, for troubleshooting.
