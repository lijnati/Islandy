# Activity engine (Phase 4)

Islandy shows developer and desktop activity: Claude Code sessions, build and test results, and
Phase 3 system alerts. One deterministic engine in Rust decides what is recorded and how
important it is. A small coordinator in the UI decides _when_ a peek may appear.

Status: implemented and tested on Linux, including real Claude Code hooks (see
[claude-code-integration.md](claude-code-integration.md)). Not yet run on Windows.

## Event flow

```
Claude Code ──hook──► islandy hook ─┐
scripts ───────────► islandy notify ┤  reduced events, token, loopback TCP
                                    ▼
                    receiver thread (auth, size, rate limits)
                                    │
Phase 3 SystemHub alerts ───────────┤
Tauri commands (dismiss, quiet, …) ─┤
                                    ▼
                  activity worker: ActivityEngine (no I/O, no clock of its own)
                     │ snapshot (coalesced)   │ peek request       │ debounced write
                     ▼                        ▼                    ▼
              Channel<ActivityUpdate> ──► activity store ──► presentation coordinator ──► island
                                                                                 activity.json
```

- `src-tauri/src/services/activity/engine.rs`: the engine. Every function takes `now`, so it is
  fully deterministic and tested without timers.
- `mod.rs`: the worker thread. It sleeps until the next message or the engine's next deadline.
  There is no polling and no fixed tick.
- `src/features/activity/presentation.ts`: the presentation coordinator.

## Event model

`ActivityEvent` (`model.rs`, mirrored in `src/services/types.ts`):

| Field                  | Meaning                                                                                 |
| ---------------------- | --------------------------------------------------------------------------------------- |
| `id`                   | `a<hex>`: unique and increasing, also across restarts                                   |
| `source`               | `claudeCode`, `developer` or `system`                                                   |
| `kind`                 | a closed enum (`claudeAttention`, `buildFailed`, `networkLost`, …)                      |
| `session`              | opaque Claude session key `c<16 hex>` (a hash), Claude events only                      |
| `project`              | a folder name, never a path                                                             |
| `title`, `description` | short text written by Islandy (a `notify` message title is the only user-supplied text) |
| `severity`             | `info`, `success`, `warning`, `error`, `attention`                                      |
| `priority`             | 0–3 (below)                                                                             |
| `lifecycle`            | `active`, `resolved`, `dismissed`, `expired`                                            |
| `timestamp`            | assigned by Islandy on receipt (client clocks are never displayed)                      |
| `expiresAt`            | when an `active` event expires on its own                                               |
| `dedupeKey`            | source-specific key; never derived from a title alone                                   |

There is no free-form metadata map. Records are deserialized with `deny_unknown_fields`, so
they are rejected rather than silently widened.

## Priority rules

Priorities govern presentation only.

| Priority | Examples                                                                                            | Peek                                        |
| -------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| P0       | Claude Code permission prompt, input request, waiting for input                                     | 8 s, then a persistent amber pill indicator |
| P1       | Claude finished responding, Claude API error, build/tests failed, battery low, network lost         | 4 s on success, 6 s on error or warning     |
| P2       | build succeeded, tests passed, dev server started, network restored, power connected, info messages | 3 s info, 4 s success                       |
| P3       | session started/ended, build/tests started, dev server stopped, tool activity                       | never (ambient indicator or history only)   |

Tool calls never peek and are not recorded in history. They only update the session's
"last activity" (a coarse category).

## Deduplication

Each source has its own keys and bounded time windows:

- **Claude:** `claude:<session>:stop` (3 s), `claude:<session>:error` (10 s), and
  `claude:<session>:attention` while a request is open. A `PermissionRequest` followed by a
  `permission_prompt` notification is one request, not two.
- **Developer:** `dev:<family>:<project>:<label>:<event>` (5 s). Messages also include their
  title in the key. The same title from another project is _not_ a duplicate.
- **System:** `system:battery` (60 s), `system:network` (30 s), `system:audio:<device>` (10 s), and so on.

The dedupe table holds at most 256 keys. Old entries are pruned first, then the oldest. A burst
of 2,000 distinct events was tested to stay within the bound.

**Out-of-order CLI events:** PowerShell doesn't wait for GUI programs, so a `build started` can
arrive after its `build failed`. The CLI sends its own clock (`at`), used _only_ to drop an event
older than one already seen for the same task, within ±10 min of Islandy's clock.

## Lifecycles and expiry

- **Claude attention:** `active` until a later event shows the request was answered
  (`PostToolUse`, `PostToolUseFailure`, `UserPromptSubmit`, `Stop`, `StopFailure`,
  `SessionEnd`), the user dismisses it, or the session is dropped. It has no timeout.
- **Build/tests started:** `active` for at most 2 h, then `expired` (not "failed"). An outcome or
  a new run resolves it.
- **Dev server started:** `active` for at most 12 h; resolved by `server stopped`.
- **Battery low, network lost:** `active` for at most 6 h; resolved by power connected or network
  restored.
- **On restart:** every persisted `active` event becomes `expired`, and nothing from history is
  ever peeked again.

## Claude Code sessions

Sessions are keyed by Claude Code's own `session_id` (hashed by the bridge), not by project.
Two sessions in one repository are tracked independently. Status values: `ready`, `working`,
`attention`, `waiting`, `idle` (turn finished), `error`, `ended`.

Stale-session policy (`engine.rs`):

| Situation                                             | What Islandy shows                        |
| ----------------------------------------------------- | ----------------------------------------- |
| `working`, no event for 15 min                        | "Status unknown" (neutral, no animation)  |
| `ended`                                               | listed 2 min, then removed                |
| `ready` / `idle` / `error`, no event for 1 h          | removed                                   |
| `working` / `attention` / `waiting`, no event for 3 h | removed (an open attention event expires) |

Silence is never reported as success. Sessions are not persisted. After Islandy restarts, a
session reappears on its next hook event with "started before Islandy" instead of a made-up
start time. At most 12 sessions are kept; ended and idle sessions are evicted first.

## Presentation coordinator

`src/features/activity/presentation.ts` sits between the engine's peek requests (and media
track changes) and the island state machine:

1. **Collapsed:** the peek shows immediately, for its own dwell time (`PeekContent.dwellMs`).
2. **A peek is showing:** a strictly more important offer replaces it. Others queue: at most 4
   entries, one per key, most important first.
3. **Dashboard open** (expanded or focused, pinned or not): offers are dropped and the queue is
   cleared. The user is already looking at the activity, and attention keeps its pill indicator.
4. **Quiet mode:** nothing peeks. The Rust engine also stops emitting peeks.
5. **After a peek:** the next queued peek waits 450 ms, so the pill settles instead of flickering.
   Offers that waited more than 15 s are dropped as stale.

There is exactly one timer at a time. The island's own state-derived timers, keyed by
`peekSeq`, make a stale dwell timer harmless.

## Ambient indicator

The collapsed pill grows from 220 to 300 logical px (a new `ambient` layout, part of the native
canvas union) to show, in priority order:

1. Claude Code sessions that need attention (amber glyph, "Needs attention" / "Waiting").
2. Working sessions ("Islandy · Working", or "3 sessions · 1 needs you").
3. A running build or test ("Islandy · Building").

Idle, ended, dismissed and stale sessions don't occupy the pill. The working glyph turns slowly
(a CSS `transform` animation, disabled with reduced motion). Attention and idle states are static.

## Quiet mode

Quiet mode is Islandy's own; it is not Windows Do Not Disturb and changes no Windows setting. It:

- suppresses automatic peeks (activity, system, media);
- keeps recording events and keeps the ambient and attention indicators;
- shows a moon in the pill and a pressed moon button in the toolbar;
- persists across restarts (`activity.json`).

Scheduling belongs to Phase 5.

## History and retention

`activity.json` in `%LOCALAPPDATA%\com.islandy.desktop` (Linux:
`~/.local/share/com.islandy.desktop`):

- At most 300 events, and nothing older than 7 days, enforced on load, on every write and by
  the engine's deadline.
- Written atomically (temp file + rename), debounced 1.5 s, and on exit.
- A malformed file is moved to `activity.corrupt.json`. Single bad records are skipped.
- **Clear history** in the activity center deletes the file and its contents. Dismissing an event
  removes it individually.

There is no SQLite. The project never had a database, and 300 small records don't justify one.

## Performance (measured on Linux, debug build)

- **Hook bridge:** 31–54 ms per invocation, including a 3 MB `tool_response` payload, which is
  skipped by the parser and never stored. 26–58 ms when Islandy is closed or its port is dead.
- **Snapshots:** coalesced (120 ms). 40 tool events produced at most 4 UI updates in tests.
- **Idle CPU:** 7 ticks (0.07 s) over 30 s for the app's main process (all Rust threads,
  including the receiver and activity worker; WebKitGTK's web process is separate and not
  counted). Windows numbers are not measured yet.
