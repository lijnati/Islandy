# Claude Code integration (Phase 4)

Islandy can show Claude Code sessions: working, needing attention, finished responding, ended.
It uses Claude Code's documented [hooks](https://code.claude.com/docs/en/hooks). The integration
is opt-in, observational and local.

**Verified:** against Claude Code **2.1.322** (the version installed in the development
environment) and the hooks reference, read on 2026-11-12. Real hook events were delivered to a
running Islandy on Linux (see [Verification](#verification)). **Not verified on Windows.**

## What Islandy installs

Enabling the integration (Activity → ⚙ Integrations → Claude Code → Enable → Add hooks) adds one
handler to each of these events in **user-level** settings: `~/.claude/settings.json`, or
`$CLAUDE_CONFIG_DIR/settings.json`.

`SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
`PostToolUseFailure`, `PermissionRequest`, `Notification`, `Stop`, `StopFailure`,
`SubagentStart`, `SubagentStop`.

Each handler looks like this:

```json
{
  "hooks": [
    {
      "type": "command",
      "command": "C:/Users/you/AppData/Local/Islandy/islandy.exe",
      "args": ["hook"],
      "async": true,
      "timeout": 10
    }
  ]
}
```

- **Exec form** (`command` + `args`): no shell, so a path with spaces needs no quoting under Git
  Bash or PowerShell.
- **`async: true`:** Claude Code never waits for Islandy (the documented semantics for async
  command hooks), and the hook's output is never shown or fed to Claude.
- **No matcher:** all tools, all notification types. Islandy filters on its side.

### Installer guarantees (`claude_settings.rs`)

- Runs only when you click **Add hooks** or **Remove hooks**. Nothing is installed silently.
- Never touches project settings (`.claude/settings.json`, `.claude/settings.local.json`).
- Recognizes its own handlers exactly: `type: "command"`, `args: ["hook"]`, and a command file
  named `islandy`/`islandy.exe`. Other hooks, matchers, settings and key order are preserved.
- **Idempotent:** a second install changes nothing. If Islandy moved, **Repair** replaces the old
  path instead of duplicating it.
- **Removal** deletes only Islandy's handlers. It drops a group or event only if _the removal_
  emptied it, and restores the file to its pre-install content (tested).
- **Malformed JSON** (including comments), a non-object `hooks`, or an event that isn't an array:
  nothing is written, and the panel says so.
- **Before writing:** the original is copied to `settings.islandy-backup-<ms>.json` (the newest 5
  are kept), the file is re-read and the write is aborted if it changed in the meantime, and the
  new content is written atomically (temp file + rename, Unix permissions preserved).
- If `~/.claude` doesn't exist, Claude Code is "not detected" and nothing is created.

The hooks reference says settings-file edits are normally picked up by Claude Code's file
watcher. Islandy still tells you a running session _may_ need a restart, because this wasn't
verified. Interactive sessions hold back settings-file hooks until the folder's workspace-trust
dialog is accepted.

### Manual setup

If automatic setup isn't possible (malformed file, managed settings), add the same handlers by
hand. Replace the path with Islandy's executable, using forward slashes. To remove the
integration manually, delete every handler whose `command` is Islandy's executable and whose
`args` is `["hook"]`. Hooks can also be turned off globally with `"disableAllHooks": true`.

## Event mapping

The bridge (`src-tauri/src/cli.rs`, `islandy hook`) maps events conservatively:

| Hook                                                                               | Session status                    | Recorded / peek                                        |
| ---------------------------------------------------------------------------------- | --------------------------------- | ------------------------------------------------------ |
| `SessionStart`                                                                     | ready                             | "Session started" (P3, no peek)                        |
| `UserPromptSubmit`                                                                 | working (turn timer starts)       | nothing (the prompt is never read)                     |
| `PreToolUse`                                                                       | working, last activity = category | nothing; does **not** resolve a permission prompt      |
| `PostToolUse`, `PostToolUseFailure`                                                | working; resolves open attention  | nothing (a failed tool is not a session error)         |
| `PermissionRequest`                                                                | attention                         | "Needs attention · Permission requested" (P0)          |
| `Notification` `permission_prompt`                                                 | attention (same request as above) | deduplicated                                           |
| `Notification` `idle_prompt`                                                       | waiting                           | "Waiting for input" (P0)                               |
| `Notification` `elicitation_dialog`, `elicitation_url_dialog`, `agent_needs_input` | attention                         | "Needs attention · Input requested" (P0)               |
| other notifications                                                                | unchanged                         | ignored                                                |
| `Stop` (not `stop_hook_active`)                                                    | idle                              | "Claude finished responding" (P1)                      |
| `StopFailure`                                                                      | error                             | "Stopped with an error · Rate limit reached" etc. (P1) |
| `SubagentStart` / `SubagentStop`                                                   | subagent count                    | nothing                                                |
| `SessionEnd`                                                                       | ended                             | "Session ended" (P3)                                   |

Deliberate limits:

- `Stop` means the turn stopped, not that the task is complete. The wording is "finished
  responding".
- A permission request means Claude Code asked. In auto or accept modes no dialog may be shown,
  and Islandy says "needs attention", not "blocked".
- There are no progress percentages, no estimates, and no claims about Claude's reasoning.
- No remote approval, no keystrokes sent to terminals, no automatic approval. Hook output never
  influences Claude Code: the bridge prints nothing and exits 0.
- "Open the relevant project or terminal" is not offered. Islandy doesn't store paths, and
  there's no verified way to focus the right terminal, so the panel says "Respond in the
  terminal where Claude Code is running".

## Bridge reliability (`islandy hook`)

- Reads stdin up to 8 MiB and drains the rest, so Claude Code's pipe never blocks. Only the fields
  listed under [Privacy](#privacy) are parsed. Values of an unexpected type are ignored, never fatal.
- Connect timeout 300 ms, I/O timeout 700 ms. When Islandy is closed it exits immediately.
- Always exits 0 and prints nothing, even on malformed input.
- Measured on Linux: 31–54 ms per call when Islandy is running (including a 3 MB payload), 26–58 ms
  when it isn't.

The bridge is a mode of the main executable (`islandy.exe hook`), so no second binary or
Node.js runtime is needed. It returns before Tauri or WebView2 start.

## Transport and receiver security

**Decision: loopback TCP with a per-installation token, using a tiny line protocol (not HTTP).**

| Option                                                         | For                                                                                                            | Against                                                                                                                                       |
| -------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Named pipe, ACL'd to the user SID (the earlier recommendation) | OS-enforced per-user access, no port                                                                           | Windows-only code path that couldn't be built _and run_ in this project's environment; overlapped I/O and a security descriptor written blind |
| Loopback HTTP + token (Claude `http` hooks)                    | No bridge needed                                                                                               | HTTP parsing, CORS/DNS-rebinding surface, token in `settings.json` or environment variables, no `SessionStart` support                        |
| **Loopback TCP + token, line protocol**                        | Portable, so the same code was tested end to end with real Claude Code; no HTTP, CORS or browser surface; tiny | Any local process can _connect_, so authentication depends on the token file's privacy                                                        |

Receiver rules (`receiver.rs`, `protocol.rs`):

- Binds `127.0.0.1` only, on an OS-assigned port, so there are no port conflicts. The port and a
  256-bit token (OS CSPRNG, reused per installation) are written atomically to `receiver.json`
  in the per-user data directory (mode 0600 on Unix; on Windows the user profile's ACL applies).
- Each connection carries one message: `ISLANDY1 <token>\n<json>\n`, at most 4 KiB, read with a
  500 ms timeout. The token is compared in constant time **before** the JSON is parsed.
- The JSON must match a closed schema (`deny_unknown_fields`, enums, length and character limits).
  An HTTP request, a wrong token, an unknown field or an unknown event is rejected with
  `error <code>`.
- Two token buckets: connections (burst 100, 50/s) and authenticated events (burst 60, 20/s).
- A message can only create an activity event. Nothing is executed, opened, or written on a
  client's behalf.
- On exit the listener stops and `receiver.json` is removed, if it still describes this
  listener. After a crash the file stays, and clients get "connection refused" immediately.

Threat model: another user on the same machine can't read the token. A process running as
_you_ can, but it could already do anything you can, so the receiver adds no privilege. A web
page can't read the file and can't produce this protocol.

**Unverified on Windows:** binding a loopback socket shouldn't trigger a Windows Firewall prompt
(prompts are for listeners on non-loopback interfaces). Check this during Windows verification.

## Privacy

The bridge forwards **only**:

- the event name (from a closed list);
- `c` + a 64-bit FNV hash of `session_id`;
- the last component of `cwd` (≤ 64 chars, control and bidi characters removed);
- a tool _category_ (`editing`, `running`, … MCP tools become `mcp`, hiding server names);
- the notification type, reduced to 4 values;
- the `StopFailure` error, reduced to 8 values;
- whether the event came from a subagent.

It never reads or forwards prompts, `tool_input`, `tool_response`, `message`,
`last_assistant_message`, `error_details`, `transcript_path`, full paths or environment
variables. A test feeds a payload containing an API key and checks that the output contains
none of it. See [privacy.md](privacy.md).

### Claude Desktop

To show when Claude Desktop (the chat app) is writing a reply, Islandy uses Windows UI
Automation. While a Claude Desktop window exists it polls about every 1.2 s, and it reads only
whether a button named "Stop response" or "Stop generating" is present. No chat content, window
text or titles are read, and nothing is stored beyond the normal activity events ("Claude
Desktop · Working" and "Claude finished responding"). It is turned off with the Claude Code
source toggle in Settings › Notifications. Claude Code in the Code tab uses the hooks described in this document.

## Developer events (`islandy notify`)

```powershell
# PowerShell: `| Out-Null` makes PowerShell wait for the GUI executable
$islandy = "$env:LOCALAPPDATA\Islandy\islandy.exe"
& $islandy notify build started --label Release | Out-Null
cargo build --release
if ($LASTEXITCODE -eq 0) { & $islandy notify build succeeded --label Release | Out-Null }
else { & $islandy notify build failed --label Release | Out-Null }
```

```
islandy notify build  started|succeeded|failed  [--project NAME] [--label TEXT]
islandy notify tests  started|passed|failed     [--project NAME] [--label TEXT]
islandy notify server started|stopped           [--project NAME] [--label TEXT]
islandy notify message --title TEXT [--severity info|success|warning|error] [--project NAME]
islandy notify --title TEXT [--type info|success|warning|error]
islandy status        # is Islandy running and reachable?
```

- The project defaults to the current folder's name.
- Exit codes: 0 delivered, 2 usage error, 3 Islandy not running, 4 rejected.
- Nothing watches terminals or processes. Every event is sent explicitly.
- Messages can't use the `attention` severity, which is reserved for Claude Code requests.

The install folder isn't on `PATH`. Use the full path, or add a PowerShell function to your
profile. Release builds use the GUI subsystem: PowerShell doesn't wait for them unless piped
(`| Out-Null`), and text output appears only when redirected. Exit codes are reliable when
waited for.

## Managing the integration (Phase 5)

**Settings › Integrations › Claude Code** shows the status, the settings file path, when the last
event arrived, Install/Reinstall/Repair and Remove (with confirmation), and troubleshooting tips.
**Developer events** shows the receiver state and a **Send test** button that sends a message
through the real receiver with the token, exactly like `islandy notify`.

**Uninstall:** the uninstaller runs `islandy.exe uninstall-cleanup`, which removes Islandy's
hooks (only if present, with a backup, keeping every other hook) and the sign-in entry. It prints
nothing and always exits 0. Updates don't run it.

## Troubleshooting

| Symptom                              | Check                                                                                                                                              |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| No sessions appear                   | Integrations panel says **Enabled**; `islandy status` prints "running"; the folder's workspace trust was accepted; start a new Claude Code session |
| "Needs repair"                       | Islandy moved or some events are missing: press **Repair hooks**                                                                                   |
| "Settings file can't be read safely" | `settings.json` isn't plain JSON (comments, trailing commas): fix it or use manual setup                                                           |
| "Local receiver unavailable"         | No per-user data directory, or the bind failed. Restart Islandy                                                                                    |
| `Stop` missing in `claude -p` runs   | Known: async hooks still running when a non-interactive run exits are killed by Claude Code                                                        |
| A session shows "Status unknown"     | No hook event for 15 min, e.g. a long command or a missed event. Islandy doesn't guess                                                             |
| Debug details                        | Debug builds log each received event's name and opaque session key to stderr (never content)                                                       |

## Verification

**On Linux, with the real Claude Code 2.1.322 and a real Islandy debug build** (hooks given
through `--settings`, in the exact shape the installer writes):

- Hook events were received by the running app: `SessionStart`, `UserPromptSubmit`, `PreToolUse`,
  `PostToolUse` (categories `reading`, `running`), `Stop`, `SessionEnd`.
- The desktop app showed "DemoProject · Working" in the ambient pill and a "Claude finished
  responding · DemoProject" peek. Screenshots: `docs/images/phase4/80-…`, `81-…`.
- The persisted history contained only kinds, a folder name and opaque keys: no prompt, path,
  file name or transcript.
- In `claude -p` runs, async `Stop` and `SessionEnd` hooks could be cut off when the process exited.
  `Stop` arrived reliably when the run continued, or with synchronous handlers. Interactive
  sessions weren't tested.

**Simulated only (unit and integration tests, not real hooks):** `PermissionRequest`,
`Notification` (all types), `StopFailure`, `SubagentStart`/`SubagentStop`, multiple concurrent
real sessions, and the installer against a real `~/.claude/settings.json`. The installer's JSON
shape was verified by Claude Code accepting identical handlers.

**Not verified:** anything on Windows (Git Bash or PowerShell hook execution, `%LOCALAPPDATA%`
paths, the firewall behavior of the loopback listener, GUI-subsystem CLI behavior).
