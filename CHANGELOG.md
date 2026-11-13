# Changelog

All notable changes to Islandy. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- The resting island shows Claude's orange mark between the clock and the playback bars. It
  breathes slowly at rest, turns while Claude works and pulses amber when Claude needs you
  (still with reduced motion; hidden when the Claude source is off).
- Clicking the Claude logo on the island opens the Claude status panel directly (sessions,
  permission requests and the usage glance); the rest of the pill still opens the dashboard.
- The Claude panel's 24-hour totals moved into the panel header (they overflowed the panel while
  a permission request was open). README screenshots show the Claude logo on the island.

### Fixed

- Automatic updates never worked: the repository was private, so GitHub refused Islandy's
  anonymous download of `latest.json` and every check failed with "No published release was
  found". Updates need the repository (and so its releases) to be public. The error now says
  the update information couldn't be downloaded. A new "Update test" workflow installs an older
  release on Windows and checks that it updates itself.

## [0.3.0] — 2026-11-13

### Added

- **Answer Claude Code permission requests on the island** (opt-in: Settings › Integrations).
  The Claude panel shows what Claude Code asks to do (the command, file, URL or tool) with
  **Allow once**, **Deny** and **Answer in Claude Code**. Claude Code waits up to 30 seconds,
  then asks in its own prompt as before. Allow is offered only when the whole request fits on
  one line; Islandy never answers by itself. The request summary stays in memory only while
  it's open. Needs the hooks repaired once (the permission hook now runs synchronously).
- **Claude usage glance**: turns, time Claude spent working and how often it needed you over
  the last 24 hours, plus per-session counts, in the Claude panel. Computed from hook timing
  only, in memory; nothing is stored or sent.
- **Weather "Use my location"**: reads your position once from Windows location services when
  you click it, rounded to about 1 km, instead of typing a city.
- **Welcome page** on first launch (fresh installs only) pointing to the Claude Code hooks, the
  weather place and the island shortcut. Reopen it from Settings › About.
- **Windows CI**: every push runs the full check suite on Windows and starts the app as a
  smoke test.

### Changed

- Claude Desktop detection matches the stop button's name case-insensitively and in more
  languages (French, German, Spanish, Italian, Portuguese, Japanese, Korean, Hindi,
  Indonesian, Chinese). The non-English names are best-effort and unverified.

## [0.2.1] — 2026-11-13

### Added

- Claude Desktop activity: while Claude Desktop (the chat app) writes a reply, the island shows
  "Claude Desktop · Working", and a "Claude finished responding" peek appears when it's done.
  Windows only. Detection uses Windows UI Automation and checks only whether Claude's "Stop
  response" button is on screen; no chat content is read. Claude Code in Claude Desktop's Code
  tab uses the existing hooks (install them in Settings › Integrations, then start a new
  session).

### Changed

- The island shows the official Claude mark in Claude's brand colour, turning and breathing
  while Claude works (still respecting reduced motion).

### Fixed

- The play/pause button didn't pause YouTube and other browser media (Chrome and Edge ignore
  the "toggle" command); Islandy now sends explicit Play and Pause.

## [0.2.0] — 2026-11-13

### Added

- Automatic updates from GitHub Releases: signed installers, checks every 6 hours, background
  download, installation while you're away, "Restart to update" in the tray and Settings ›
  About, with switches for checking and installing.
- Weather widget (Open-Meteo): current conditions, feels-like, humidity, wind and a three-day
  outlook for a place chosen in Settings › Widgets; °C/°F.
- "Update available/ready" peek through the activity engine (quiet mode applies).

### Fixed

- The settings window opened black and couldn't be closed on Windows when opened from the
  dashboard's settings button.

### Changed

- The release workflow signs update artifacts, publishes `latest.json`, and can create the tag
  itself when run from the Actions tab.

## [0.1.0-rc.1] — 2026-11-12

First release candidate: the island, media, system, activity and Claude Code integration, the
settings window, tray, launch at startup and the per-user installer. See
[docs/releases/v0.1.0-rc.1.md](docs/releases/v0.1.0-rc.1.md).

[0.2.0]: https://github.com/lijnati/Islandy/releases/tag/v0.2.0
[0.1.0-rc.1]: https://github.com/lijnati/Islandy/blob/HEAD/docs/releases/v0.1.0-rc.1.md
