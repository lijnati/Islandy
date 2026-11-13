# Contributing to Islandy

Thanks for helping. Islandy aims to feel like part of Windows, so small, careful changes beat
big ones.

## Before you start

- For anything larger than a fix, open an issue first and describe the problem and your plan.
- Read [docs/architecture.md](docs/architecture.md) and the doc for the area you touch
  (media, system, activity engine, settings, release).

## Setup

See [Development](README.md#development) in the README. In short:

```sh
pnpm install
pnpm tauri dev
```

## Quality gates

Every pull request must pass:

```sh
pnpm check   # typecheck, ESLint (zero warnings), Prettier + rustfmt, Vitest, Clippy -D warnings, cargo test
```

- Add tests for new logic. Pure models (`features/*/…-model.ts`, Rust services) are tested
  directly; UI behaviour with Testing Library.
- Rust and TypeScript mirror each other for settings and IPC types; the shared fixture
  `src/features/settings/__fixtures__/default-settings.json` keeps the defaults in sync.
- A new IPC command needs four things: the Rust command, its entry in `src-tauri/build.rs`, a
  grant in the right `capabilities/*.json` (least privilege), and a typed wrapper in
  `src/services/native.ts`.

## Rules that don't bend

- **Privacy:** no telemetry, analytics, crash reporters or accounts. Never collect prompts,
  source code, terminal history, environment variables or raw hook payloads. Any new network
  request must be opt-in or clearly user-controlled and documented in
  [docs/privacy.md](docs/privacy.md).
- **Security:** no shell execution from the UI, no paths or URLs from the frontend, signed
  updates only. See [docs/security.md](docs/security.md).
- **Respect the user's system:** never change Windows settings (Do Not Disturb, default devices)
  or other apps' configuration without an explicit action.

## Windows verification

Behaviour that depends on Windows (windows, focus, DPI, media sessions, radios, installers) must
be checked on real Windows. Use [docs/windows-verification.md](docs/windows-verification.md) and
say in the pull request what you checked, on which Windows version and hardware.

## Commits and pull requests

- One topic per pull request; describe what changed and why, and how you tested it.
- Keep commit messages in the imperative ("Fix …", "Add …").
- Update the docs and [CHANGELOG.md](CHANGELOG.md) when behaviour changes.
