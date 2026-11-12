# Release process

## Versioning

Semantic versioning, starting at **0.1.0**. The version lives in `package.json`,
`src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` (keep all three equal). Release notes
are in `docs/releases/`. Tags are `v<version>`; creating tags and GitHub releases needs explicit
approval from the maintainer.

## Building a release

On Windows with the MSVC toolchain (the supported release path):

```sh
pnpm install --frozen-lockfile
pnpm check                 # typecheck, lint, format, tests, clippy, cargo test
pnpm tauri build           # src-tauri/target/release/islandy.exe + bundle/nsis/*.exe
```

### CI release (GitHub Actions)

`.github/workflows/release.yml` builds the NSIS installer with MSVC on `windows-latest` and
publishes it to a GitHub Release. Push a `v<version>` tag to run it (or run it manually for an
existing tag). Notes come from `docs/releases/<tag>.md`; tags containing `-` (e.g. `-rc.1`) are
marked as pre-releases.

Cross-builds from Linux use the MinGW toolchain
(`pnpm tauri build --target x86_64-pc-windows-gnu`, needs `mingw-w64` and `nsis`). They are fine
for testing the installer; prefer an MSVC build for distribution.

The installer is NSIS, per-user (`installMode: currentUser`): no administrator rights, installs to
`%LOCALAPPDATA%\Islandy`, uninstall key under `HKCU`. MSI was not chosen: it is per-machine by
default (admin), and per-user MSI with WiX adds complexity without a benefit for this app.
`src-tauri/windows/hooks.nsh` cleans up the sign-in entry and Claude Code hooks on uninstall
only (never during an update).

## Signing (not configured)

The 0.1.0 RC is **unsigned**, so SmartScreen warns on first run. To sign:

1. Obtain an Authenticode certificate (OV/EV, or Azure Trusted Signing).
2. Set `bundle.windows.certificateThumbprint` (+ `digestAlgorithm: "sha256"`,
   `timestampUrl`) or a `signCommand` in `tauri.conf.json`.
3. Sign both `islandy.exe` and the installer; verify with `signtool verify /pa /v`.

No signing material is stored in this repository.

## Updates (decision for 0.1.0)

Tauri's official updater plugin was evaluated and **not enabled**:

- It needs an update endpoint (a static JSON or a server) and a minisign key pair
  (`tauri signer generate`) whose private key must be kept secret outside the repository.
- Neither exists yet, and an updater pointing at an invented endpoint, or shipped with a throwaway
  key, would be worse than none.

Supported path: users install a newer installer over the old one; settings, history, hooks and the
startup entry are kept. To enable updates later: generate the key pair, publish `latest.json` with
each release (GitHub Releases works), add `tauri-plugin-updater` with the public key and endpoint,
and expose "Check for updates" in Settings › About. Islandy must never auto-install without the
user's consent.

## Release checklist

- [ ] Versions bumped in the three files; release notes written.
- [ ] `pnpm check` passes; `pnpm tauri build` succeeds (MSVC).
- [ ] README Windows checklist run on **Windows 11 and Windows 10**, including 100/125/150 %
      scaling, multi-monitor (primary not leftmost), sleep/resume, Explorer restart, a borderless
      fullscreen game/video, and taskbar auto-hide.
- [ ] Install, first launch, launch at startup across a sign-out, update over the previous version,
      uninstall (with and without "delete app data"); no `islandy.exe`/`msedgewebview2.exe` left.
- [ ] Idle CPU/RAM measured on Windows and recorded in docs/performance.md.
- [ ] `cargo audit` / `pnpm audit` reviewed.
- [ ] Binaries signed (once configured).
- [ ] Tag and GitHub release created **only after approval**.

## Status of v0.1.0-rc.1

See [releases/v0.1.0-rc.1.md](releases/v0.1.0-rc.1.md). Windows runtime verification is still open,
so this is a release candidate for limited testing, not a public release.
