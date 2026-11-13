# Changelog

All notable changes to Islandy. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

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
