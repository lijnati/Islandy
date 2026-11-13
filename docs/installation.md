# Installing Islandy

Islandy is distributed as a per-user NSIS installer
(`Islandy_<version>_x64-setup.exe`) from
[GitHub Releases](https://github.com/lijnati/Islandy/releases). It needs no administrator rights.

> **Release-candidate status.** The installer is built from this repository with Tauri's
> bundler. Installing and uninstalling were exercised under Wine on Linux (files, shortcuts,
> registry cleanup); they have **not yet been run on a real Windows 10 or 11 PC**. See
> [release.md](release.md) for the open verification items.

## Requirements

- Windows 10 (1809 or later) or Windows 11, 64-bit (x64).
- Microsoft Edge WebView2 Runtime. It is preinstalled on Windows 11 and current Windows 10;
  otherwise the installer downloads it (internet connection needed once).

## Install

1. Download `Islandy_<version>_x64-setup.exe` from the latest release and run it.
2. **SmartScreen:** this build is not code-signed, so Windows may show "Windows protected your
   PC". Choose **More info → Run anyway** only if you obtained the installer from a source you
   trust. Signed builds are planned (see [release.md](release.md)).
3. The installer copies Islandy to `%LOCALAPPDATA%\Islandy` and adds a Start menu entry
   (**Islandy**). Nothing is installed for other users, and no service or scheduled task is
   created.
4. Islandy starts after installation. On first launch a short hint appears on the island:
   hover it, or press **Ctrl + Alt + Space**.

## Everyday use

- **Open the dashboard:** hover the island, click it, or press the shortcut.
- **Tray icon** (notification area): left-click opens the dashboard; right-click shows
  Show/Hide Islandy, Settings…, Quiet mode, Launch at startup, About and Quit.
- **Hide the island** without quitting: tray → Hide Islandy (or Settings › General). Show it
  again from the tray or with the shortcut.
- **Quit:** tray → Quit Islandy. This stops every background part (media, system, activity
  receiver); nothing keeps running.
- Starting Islandy again while it runs opens its Settings instead of a second island.

## Start with Windows

Off by default. Turn it on in **Settings › General › Launch at startup** or from the tray.
Islandy adds one per-user entry (`HKCU\…\CurrentVersion\Run`, value `Islandy`) with the quoted
path to `islandy.exe` and `--autostart`. You can also switch it off in Task Manager › Startup
apps; Islandy shows that state. **Start hidden** keeps the island hidden after sign-in until you
open it from the tray or with the shortcut.

## Update

From 0.2.0 on, Islandy updates itself. It checks this project's GitHub Releases shortly after it
starts and then every 6 hours, downloads a newer version in the background, verifies its
signature, and installs it while you're away from the PC (a small progress bar appears for a few
seconds and Islandy restarts). You can also install at once from the tray (**Restart to update**)
or **Settings › About**. Both switches (checking, and installing automatically) are in
**Settings › About**.

Versions before 0.2.0 (the 0.1.0 release candidate) can't update themselves: install 0.2.0 once
over them. Running a newer installer by hand always works too; it keeps your settings, history
and startup choice.

## Uninstall

**Settings › Apps › Installed apps › Islandy › Uninstall** (or the uninstaller in
`%LOCALAPPDATA%\Islandy`). The uninstaller:

- removes the program files and Start menu entry;
- removes the start-at-sign-in entry;
- removes Islandy's hooks from your user-level Claude Code settings, if they are installed
  (a backup is saved first; your other hooks and settings are untouched);
- optionally deletes Islandy's data (`%LOCALAPPDATA%\com.islandy.desktop`: settings, activity
  history, logs) when you tick **Delete the application data**.

Updates don't do any of this cleanup; only an uninstall does.

## Command-line tool

`islandy.exe` doubles as a small command-line tool for scripts (it never opens a window in these
modes):

```powershell
& "$env:LOCALAPPDATA\Islandy\islandy.exe" notify build failed --project api | Out-Null
& "$env:LOCALAPPDATA\Islandy\islandy.exe" status | Write-Output
```

Add `%LOCALAPPDATA%\Islandy` to `PATH` to call it as `islandy`. See
[claude-code-integration.md](claude-code-integration.md) for all options and exit codes.

## Build from source

See the README (Prerequisites, Build) and [release.md](release.md).
