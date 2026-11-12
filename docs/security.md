# Security and privacy

Islandy runs continuously on the user's desktop, so it is designed to be trustworthy by
construction: local-only, least-privilege, and without any surface that executes arbitrary
input.

## Decisions (Phases 0–1)

| Area                | Decision                                                                                                                                                                                                                                                                                                                           |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Network             | No outbound requests. No telemetry, analytics SDK, crash reporter, update checker, or remote fonts (Geist is bundled).                                                                                                                                                                                                             |
| Accounts            | None. No cloud services.                                                                                                                                                                                                                                                                                                           |
| Privileges          | Runs as the normal user. The NSIS installer uses `installMode: currentUser`, which installs per-user without UAC elevation.                                                                                                                                                                                                        |
| Tauri plugins       | Only `global-shortcut`, used from Rust; the frontend is granted none of its permissions. No shell, filesystem, HTTP, or opener plugins.                                                                                                                                                                                            |
| Capabilities        | `capabilities/island.json` grants only Islandy's six commands to the `island` window: `get_app_info`, `connect_island`, `get_island_layouts`, `set_island_frame`, `set_island_interactive`, `open_system_settings`. No `core:*` permissions: the frontend cannot move or close windows, read paths, or listen to arbitrary events. |
| Command arguments   | Typed and validated: frame sizes must be finite and positive and are clamped to the monitor-fitted canvas; settings pages are an enum mapped to four fixed `ms-settings:` URIs. No command takes a path, URL, or command line.                                                                                                     |
| Push channel        | Rust → UI messages use one Tauri `Channel` passed in by the UI (`connect_island`), not global events, so no event permissions are needed.                                                                                                                                                                                          |
| Global shortcut     | `Ctrl+Alt+Space` via `RegisterHotKey` (plugin). It never overrides another app's hotkey; a conflict is reported. No keyboard hook and no mouse hook are installed.                                                                                                                                                                 |
| Command gating      | `build.rs` declares an **app manifest**. Tauri then denies every app command that no capability grants, so a new command cannot be exposed by accident.                                                                                                                                                                            |
| CSP                 | Production: `default-src 'self'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src ipc: http://ipc.localhost`. No inline scripts, no `eval`, no remote origins. The dev CSP additionally allows inline styles and the Vite HMR websocket.                                                    |
| Prototype pollution | `freezePrototype: true`.                                                                                                                                                                                                                                                                                                           |
| Global API          | `withGlobalTauri: false`. The Tauri API is imported as a module only by `src/services/native.ts`.                                                                                                                                                                                                                                  |
| Error messages      | Errors cross IPC as `{ kind, message }` with no stack traces or paths.                                                                                                                                                                                                                                                             |
| Logging             | Debug builds log IPC calls and focus/signal events to stderr. Release builds log only errors, never user data.                                                                                                                                                                                                                     |
| Demo data           | Media and activity widgets show local, hard-coded sample data labelled "Demo". Nothing is read from other apps.                                                                                                                                                                                                                    |

## Rules for future work

1. **Every new IPC command** must be added to the app manifest (`build.rs`), granted
   explicitly in a capability, and wrapped in `native.ts`. Commands take typed, validated
   arguments, never paths, URLs, or command lines from the UI.
2. **No arbitrary execution.** Islandy never runs shell commands built from external input.
3. **Local event producers** (Claude Code hooks, user scripts) use a per-user named pipe with
   an ACL restricted to the current user's SID and `PIPE_REJECT_REMOTE_CLIENTS`. Messages
   follow a reduced, allow-listed schema with size and rate limits. If a localhost HTTP
   endpoint is ever added, it requires a random per-install token stored in a user-only file,
   binds to `127.0.0.1`, and rejects requests with an `Origin` header. See
   `windows-integration.md`.
4. **Minimize data.** Do not ingest prompts, tool inputs, file contents, or transcript paths
   from agent hooks. Media artwork and metadata stay in memory.
5. **Opt-in network features only** (for example, calendar accounts). Each one is explicit,
   documented, and off by default.
6. **Third-party settings files** (for example, `~/.claude/settings.json`) are modified only
   after showing the user the exact change and getting confirmation.
7. **Dependencies:** add a dependency only when it is needed, prefer official and widely used
   crates and packages, and review the advisories (`cargo audit`, `pnpm audit`) before release.

## Not yet addressed

- Code signing (Authenticode) for the binary and installer. Unsigned builds trigger
  SmartScreen warnings.
- An update mechanism. If added, it must use signed updates (Tauri updater with a public key).
- Dependency advisory scanning in CI.
