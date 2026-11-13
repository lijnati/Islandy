# Changelog

All notable changes to Islandy. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

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
