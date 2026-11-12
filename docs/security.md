# Security and privacy

Islandy runs continuously on the user's desktop, so it is designed to be trustworthy by
construction: local-only, least-privilege, and without any surface that executes arbitrary
input.

## Decisions (Phase 0)

| Area                | Decision                                                                                                                                                                                                                                                                        |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Network             | No outbound requests. No telemetry, analytics SDK, crash reporter, update checker, or remote fonts (Geist is bundled).                                                                                                                                                          |
| Accounts            | None. No cloud services.                                                                                                                                                                                                                                                        |
| Privileges          | Runs as the normal user. The NSIS installer uses `installMode: currentUser`, which installs per-user without UAC elevation.                                                                                                                                                     |
| Tauri plugins       | None. No shell, filesystem, HTTP, or opener plugins.                                                                                                                                                                                                                            |
| Capabilities        | `capabilities/island.json` grants exactly one permission, `allow-get-app-info`, to the `island` window. No `core:*` permissions: the frontend cannot move or close windows, read paths, or listen to arbitrary events.                                                          |
| Command gating      | `build.rs` declares an **app manifest**. Tauri then denies every app command that no capability grants, so a new command cannot be exposed by accident.                                                                                                                         |
| CSP                 | Production: `default-src 'self'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src ipc: http://ipc.localhost`. No inline scripts, no `eval`, no remote origins. The dev CSP additionally allows inline styles and the Vite HMR websocket. |
| Prototype pollution | `freezePrototype: true`.                                                                                                                                                                                                                                                        |
| Global API          | `withGlobalTauri: false`. The Tauri API is imported as a module only by `src/services/native.ts`.                                                                                                                                                                               |
| Error messages      | Errors cross IPC as `{ kind, message }` with no stack traces or paths.                                                                                                                                                                                                          |
| Logging             | The debug build logs `get_app_info` responses to stderr. Release builds log nothing about user data.                                                                                                                                                                            |

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
