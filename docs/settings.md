# Settings

Islandy's preferences live in a separate **settings window** (tray → Settings…, the gear in the
dashboard toolbar, or launching Islandy again). It is a normal Windows window, created when
opened and destroyed when closed, so it costs nothing while you don't use it.

## Storage and format

| What                                     | Where (`%LOCALAPPDATA%\com.islandy.desktop\`)  | Owner                 |
| ---------------------------------------- | ---------------------------------------------- | --------------------- |
| Appearance, island, interaction, widgets | `settings.json`                                | `services/settings/`  |
| Notifications, quiet mode, sources       | `activity.json` (with the activity history)    | `services/activity/`  |
| Launch at startup                        | Windows' `Run` key (read live, never cached)   | `services/startup.rs` |
| Toggle shortcut                          | `settings.json`, after successful registration | `shortcut.rs`         |

- **Typed and versioned.** `settings.json` carries `"version": 1`. The schema is
  `src-tauri/src/services/settings/models.rs`, mirrored by
  `src/features/settings/settings-schema.ts`. Both sides are tested against one fixture
  (`src/features/settings/__fixtures__/default-settings.json`).
- **Validated in Rust.** Every update is normalized (`Settings::normalize`): numbers are
  clamped, unknown widgets dropped, missing ones appended, and the pointer always has a way to
  open the island.
- **Lenient loading.** Each section is read field by field on top of the defaults: a wrong value
  falls back to that field's default; valid neighbours are kept; unknown keys are ignored.
- **Atomic writes** (temporary file + rename; mode 0600 on Unix).
- **Corruption recovery.** A file that isn't a JSON object (or is over 256 KiB) is moved to
  `settings.corrupt.json`, defaults are used, and the settings window says so.
- **Migrations.** `storage::MIGRATIONS` holds one step per format version (none yet: v1 is the
  first released format). A file from a _newer_ Islandy is copied to
  `settings.v<N>.backup.json` before this version writes it.
- **No secrets.** The local receiver token lives in `receiver.json` (per-user, never shown);
  nothing in `settings.json` is sensitive.
- **Immediate sync.** Rust pushes every stored change to every open window (island and settings),
  so changes apply without a restart.
- **Reset** (Settings › General) restores defaults for preferences and notifications. History,
  Claude Code hooks and launch at startup are left alone; they have their own controls.

## Pages

### General

- **Launch at startup** (off by default) and **Start hidden** (only when Windows starts it).
- **Shortcut** to show/hide the dashboard (default **Ctrl + Alt + Space**). Recording a new one
  registers it _before_ saving and only then releases the old one. It must use Ctrl, Alt or Win
  plus a non-modifier key; combinations Windows reserves (Alt+Tab, Win+L, Ctrl+Alt+Delete, …) are
  refused, and one that another app already owns is reported, never taken over. The tray icon
  always works.
- **Hide during fullscreen apps** (on): see [windows-integration.md](windows-integration.md).
- **Hide the island** (keeps running in the tray).

### Appearance

- **Theme:** Midnight (near-black), Graphite (charcoal), Frost (translucent).
- **Accent:** white, blue, violet, green, amber, coral. Used for switches, sliders, selection and
  focus. Status colours never change.
- **Background opacity** 70–100 %.
- **Corners:** compact, standard, round.
- **Animation:** full or subtle (no overshoot). **Reduce motion:** system / on / off.

All of these are CSS custom properties set on `<html>` (`data-theme`, `data-accent`,
`data-corners`, `--island-alpha`); components use tokens only.

**Frost and blur.** A transparent WebView can't blur the desktop behind it, and native acrylic
(`window-vibrancy`) is not enabled: it would apply to the whole rectangular canvas (visible
outside the rounded island) and is known to lag on Windows 10 while windows move. Frost
therefore uses translucent, tinted surfaces only.

### Island

- **Display:** primary, or a specific monitor (by Windows' monitor name, e.g. `\\.\DISPLAY2`). A
  disconnected monitor falls back to the primary one and the choice is kept for when it returns.
- **Alignment:** top left, top center, top right. **Horizontal offset** and **distance from top**
  (0–48 px). Positions are clamped 12 px inside the monitor's work area at every scaling.
- **Alt + drag to move** (off by default): hold Alt and drag the collapsed pill sideways. A plain
  press still opens the dashboard; widgets are never draggable.
- **Size:** compact 200 × 40, standard 220 × 44, comfortable 260 × 48 logical px (peeks and the
  activity pill scale with it). The dashboard keeps 820 × 248 and narrows on small monitors.
- **Pointer:** open on hover (with a 100–500 ms delay), open on click, pin when clicked, collapse
  delay 200–1000 ms, auto-collapse a pinned dashboard after 15 s alone. At least one of hover or
  click stays on.

An unpinned (hover-opened) dashboard always collapses when the pointer leaves: it has no keyboard
focus, so nothing else could dismiss it. That's why "auto-collapse" applies to pinned dashboards.

### Widgets

A reorderable list of the built-in widgets: Now Playing, Calendar, Activity, System (battery,
network, Bluetooth) and Sound (output, volume, mute). The dashboard shows the turned-on widgets in
order and keeps those that fit its width; the list marks the ones that don't. **Restore default
layout** brings back media, calendar and activity. There is no third-party widget support.

### Integrations

Claude Code (status, settings path, last event, install/reinstall/remove with confirmation,
troubleshooting), developer events (receiver status, **Send test** through the real receiver,
example commands) and what Windows offers on this PC (media sessions, volume, battery, network,
radios, brightness).

### Notifications

Quiet mode; peeks per source (Claude Code, builds/tests/scripts, system alerts, track changes);
successes and errors; peek duration (short ×0.75, standard, long ×1.5); which sources are recorded
at all. Each source may peek at most 4 times a minute (P0 attention requests are exempt). Windows
notification settings and Do Not Disturb are never touched.

### Privacy

A summary of what Islandy does and doesn't collect, history retention (1, 3, 7, 14 or 30 days;
at most 300 events), Clear history, the files Islandy stores with their sizes, buttons to open the
data and log folders, detailed logging, and **Copy diagnostic summary**.

### About

Version, runtime (Tauri, WebView), update policy, and a non-affiliation note.

## Environment variables

`ISLANDY_FRAME_STRATEGY=resize` still selects the fallback window strategy (diagnostics only).
`ISLANDY_TOGGLE_SHORTCUT` from earlier phases is no longer read: the shortcut is a setting.
