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

`.github/workflows/release.yml` builds the NSIS installer with MSVC on `windows-latest`, signs
the update artifacts and publishes a GitHub Release with the installer, its signature and
`latest.json`. Either push a `v<version>` tag, or run **Actions › Release › Run workflow** with a
tag name: a missing tag is created on the selected branch's commit. The tag must match the
version in `tauri.conf.json`, and the two signing secrets must be set (the workflow stops with an
explanation otherwise). Notes come from `docs/releases/<tag>.md`; tags containing `-` (e.g.
`-rc.1`) are marked as pre-releases.

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

## Automatic updates

Islandy updates itself from this repository's GitHub Releases with Tauri's updater plugin
(`tauri-plugin-updater`). The app-side behaviour is described in
[settings.md](settings.md#about) and [privacy.md](privacy.md#network).

- **Endpoint:** `https://github.com/lijnati/Islandy/releases/latest/download/latest.json`
  (`plugins.updater.endpoints` in `src-tauri/tauri.conf.json`). GitHub's "latest" release
  excludes pre-releases, so `-rc`/`-beta` tags never reach installed copies.
- **Signing:** every update is signed with a minisign key. The public key is
  `plugins.updater.pubkey`; the updater refuses an installer whose signature doesn't verify.
  The private key and its password exist only as the repository secrets
  `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (and in the maintainer's
  password manager). They are never committed. **If the private key is lost, installed copies
  can't be updated any more**; they would need a manual install of a build with a new public
  key.
- **Artifacts:** with `bundle.createUpdaterArtifacts: true`, `tauri build` also writes
  `Islandy_<v>_x64-setup.exe.sig`. The release workflow uploads the installer, the `.sig` and a
  generated `latest.json` (version, notes, date, URL and signature per platform).
- **Install:** the NSIS installer runs in passive mode (`/UPDATE`, progress bar only) and
  restarts Islandy. `windows/hooks.nsh` skips its uninstall cleanup in update mode, so the
  startup entry and Claude Code hooks are kept.
- **Rotating the key:** generate a new pair (`pnpm tauri signer generate -w islandy-updater.key`),
  put the public key in `tauri.conf.json`, update both secrets, and release. Only copies that
  already run a build with the new public key accept updates signed with it.

Local builds that bundle (not `--no-bundle`) need the two variables set in the environment, or
set `createUpdaterArtifacts` to `false` for a local test build.

## Release checklist

- [ ] Versions bumped in the three files; release notes written.
- [ ] `pnpm check` passes; `pnpm tauri build` succeeds (MSVC).
- [ ] [Windows checklist](windows-verification.md) run on **Windows 11 and Windows 10**, including 100/125/150 %
      scaling, multi-monitor (primary not leftmost), sleep/resume, Explorer restart, a borderless
      fullscreen game/video, and taskbar auto-hide.
- [ ] Install, first launch, launch at startup across a sign-out, update over the previous version,
      uninstall (with and without "delete app data"); no `islandy.exe`/`msedgewebview2.exe` left.
- [ ] Idle CPU/RAM measured on Windows and recorded in docs/performance.md.
- [ ] `cargo audit` / `pnpm audit` reviewed.
- [ ] Binaries signed (once configured).
- [ ] Signing secrets present; `latest.json` attached to the release and pointing at the `.sig`
      of the uploaded installer.
- [ ] An installed previous version finds, downloads and installs the update (About › Check now).
- [ ] Tag and GitHub release created **only after approval**.

## Status of v0.1.0-rc.1

See [releases/v0.1.0-rc.1.md](releases/v0.1.0-rc.1.md). Windows runtime verification is still open,
so this is a release candidate for limited testing, not a public release.
