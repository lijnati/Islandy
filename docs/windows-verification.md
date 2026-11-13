# Windows verification checklist

Islandy is developed largely on Linux; Windows-native behaviour must be checked by hand on real
Windows 10 and 11 PCs before a release. Run `pnpm tauri dev` (and later the installed release
build) and tick each item. Record hardware, scaling and app versions next to failures.

**Window (Phase 0)**

- [ ] The pill appears at the top center of the primary monitor, 8 px below the work area top,
      with no white flash, border, shadow, or visible window rectangle.
- [ ] Correct size and position at 100 %, 125 %, 150 % and 175 % scaling. Change scaling while
      the app runs: the island re-centers.
- [ ] It stays above normal and maximized windows.
- [ ] No taskbar button, and absent from Alt+Tab and Task View (also after pinning and
      collapsing, which toggles focusability).
- [ ] Tray icon → **Quit Islandy** exits; no `islandy.exe` or orphaned `msedgewebview2.exe`
      remains. The shortcut is free again afterwards.
- [ ] Multi-monitor: the island sits on the primary monitor, including when the primary is
      not the leftmost.

**Frame and click-through (Phase 1, highest risk)**

- [ ] The island renders normally (not blank) with the default region strategy. If it is
      blank or shows artifacts, retry with `ISLANDY_FRAME_STRATEGY=resize` and record which works.
- [ ] Collapsed: clicks immediately beside and below the pill reach the window underneath.
- [ ] Expanded: clicks just outside the dashboard reach the window underneath.
- [ ] Expanding and collapsing: the island stays horizontally centered and top-anchored, with
      no sideways jump, flicker, or white frame.

**Interaction (Phase 1)**

- [ ] Resting the pointer on the pill expands it after a short delay; brushing past does not.
- [ ] Moving away collapses it after ~0.4 s; moving back in time keeps it open.
- [ ] Hovering and clicking media controls while typing in another app does **not** move the
      caret or focus.
- [ ] Clicking the pill pins it: it stays open after the pointer leaves; Tab moves focus
      through controls with a visible focus ring; Escape collapses it and focus returns to the
      previous app.
- [ ] Clicking another window while pinned collapses the island.
- [ ] **Ctrl + Alt + Space** toggles the island from any app. With another app already owning
      the shortcut, Islandy logs the conflict and does not take it over.
- [ ] Quick controls open their panels in place; pressing the open one closes it. The
      Settings button opens Windows Settings and the island collapses.
- [ ] Opening a widget (click its title) shows the detail view; Back and Escape return.
- [ ] Settings › Integrations › Developer events › **Send test** shows a peek that disappears after a few seconds.

**Media (Phase 2)** — record app name and version for each; if an app lacks a capability
(e.g. no seeking), note it as the app's limitation, not an Islandy failure.

- [ ] Browser (Edge or Chrome) playing a YouTube video: detected within ~1 s; title, channel,
      artwork; play/pause, next/previous (on a playlist), seeking.
- [ ] A second player (Spotify, Media Player, VLC with SMTC enabled, …): same checks.
- [ ] Pause and resume from the app: the pill's bars stop/start; the widget's button flips.
- [ ] Pause/resume, next/previous from Islandy: the app reacts; Islandy's state changes only
      after the app does.
- [ ] Track change (auto-advance and skip): metadata and artwork update, a peek appears while
      collapsed, no stale artwork.
- [ ] Seeking by click/drag and arrow keys lands where expected; unsupported → no slider.
- [ ] Two players at once: the switcher lists both; a manual choice sticks while the other
      starts/stops; "Automatic" follows Windows' current session again.
- [ ] Close the player while it plays: the session disappears; the island returns to idle (or
      to the other session) without errors.
- [ ] Quit Islandy while media plays: it exits promptly.
- [ ] Unicode and very long titles render and truncate cleanly.
      **System (Phase 3)**. Record the hardware for each check (laptop/desktop, monitor model,
      adapters).

- [ ] Sound: the panel shows the default output and the same volume as the taskbar flyout.
      Dragging the slider changes Windows' volume smoothly. Changing volume with keyboard keys
      or the flyout moves Islandy's slider. Mute/unmute both ways.
- [ ] Sound: switching the default output (Settings, or plugging in headphones) updates the
      panel and shows an "Audio output changed" peek; unplugging the last device shows "No
      audio output device".
- [ ] Battery (laptop): the percentage and charge state match the Windows battery flyout.
      Unplug/plug shows "On battery" / "Power connected" once. Low battery (≤ 20 %) peeks
      once.
- [ ] Desktop without battery: the System panel says "No battery", never a percentage.
- [ ] CPU/RAM roughly match Task Manager; sampling stops when the panel closes (Islandy's CPU
      returns to idle).
- [ ] Network: disconnecting Wi-Fi/Ethernet shows "Offline" and, after ~3 s, a "Network
      disconnected" peek; reconnecting shows "Connected to …". A network without internet
      shows "No internet".
- [ ] Wi-Fi/Bluetooth switches: either they work (Windows' own quick settings follow), or
      they are absent with "Change it in Settings". Note whether Windows prompted for access.
- [ ] Brightness: works on a laptop panel; works on a DDC/CI monitor (or reports "Not
      supported" honestly); multi-monitor setups list each controllable display.
- [ ] Settings links open Sound, Network, Wi-Fi, Bluetooth, Display, Power & battery and
      Settings home.
- [ ] Quitting with the System or Display panel open exits promptly.
- [ ] Idle CPU with music playing and the island collapsed stays near the idle budget
      (see `docs/performance.md`).
      **Activity (Phase 4)**. Note the Claude Code version and the shell hooks run under (Git
      Bash or PowerShell).

- [ ] Settings › Integrations → Claude Code → Install hooks (or the dashboard's Integrations panel): `~/.claude/settings.json` gains
      Islandy's 12 handlers, other hooks are untouched, a `settings.islandy-backup-*.json`
      appears. Remove hooks restores the previous content.
- [ ] A new interactive Claude Code session: the pill shows "<project> · Working" during a
      turn and a "Claude finished responding" peek at the end. No console window flashes.
- [ ] A permission prompt in Claude Code: an amber "Needs attention · Permission requested"
      peek, then the amber pill indicator until the prompt is answered.
- [ ] Two sessions in the same folder: both listed separately in the Claude Code view.
- [ ] Quit Islandy while Claude Code runs: Claude Code is unaffected (no errors, no delay);
      restart Islandy: sessions reappear on their next event, nothing stale is shown.
- [ ] No Windows Firewall prompt when Islandy starts (the receiver binds 127.0.0.1 only).
- [ ] PowerShell: `& "$env:LOCALAPPDATA\Islandy\islandy.exe" notify build failed | Out-Null`
      peeks "Build failed"; `status | Write-Output` prints "running".
- [ ] Quiet mode (moon): no peeks; the pill shows the moon; attention still turns the pill amber.
- [ ] With Windows _Animation effects_ turned off, transitions are instant.
- [ ] The clock and calendar are correct and roll over at the minute and midnight, including
      after sleep/resume.

**Personalization and release (Phase 5).** Test on Windows 11 and Windows 10.

- [ ] Install the NSIS installer without admin rights; Start menu entry; first launch shows the
      hint peek once; WebView2 bootstrapper works on a machine without WebView2.
- [ ] Settings window: opens from the tray, the toolbar gear and a second launch (no second
      island or tray icon); dark title bar; no white flash; keyboard navigation with Tab.
- [ ] Themes, accent, opacity, corners and motion apply to the island immediately.
- [ ] Display selection on a multi-monitor setup (primary not leftmost, mixed 100/150 %);
      top left/center/right; offset; top margin; size presets; Alt+drag; positions survive restart;
      unplug the selected monitor (island moves to primary) and plug it back.
- [ ] Widgets: reorder, hide all (empty state), restore defaults; System and Sound widgets show
      real data.
- [ ] Shortcut: change to another combination; a combination owned by another app is refused with
      a message and the old one still works.
- [ ] Launch at startup: on → sign out/in → Islandy starts (hidden with "Start hidden"); Task
      Manager › Startup apps shows Islandy; disabling it there is reflected in Settings.
- [ ] Tray: left click opens the dashboard; right-click menu items work; Hide/Show; Quit leaves
      no `islandy.exe`/`msedgewebview2.exe`; Explorer restart (Task Manager) brings the icon back.
- [ ] Fullscreen: a borderless fullscreen video/game hides the island on that monitor and it returns
      afterwards; maximized windows don't hide it; taskbar auto-hide doesn't break placement.
- [ ] Sleep/resume and lock/unlock: the island is placed correctly and the clock is right.
- [ ] Notifications: each switch, duration, quiet mode; a burst of `islandy notify` messages peeks
      at most 4 times a minute.
- [ ] Privacy: clear history, retention, open folders, copy diagnostics (check its content).
- [ ] Update: install the same/newer installer over it; settings, history, startup and hooks kept.
- [ ] Uninstall: startup entry and Islandy's Claude Code hooks are removed (other hooks kept),
      optional data deletion works, nothing left running.

**Updates and weather (0.2.0)**

- [ ] With a previous 0.2.x installed: Settings › About › **Check now** finds the new release,
      shows its notes, downloads it (progress), and **Restart to update** installs it in passive
      mode and restarts Islandy; settings, history, startup entry and hooks are kept.
- [ ] Automatic path: leave the PC idle for 4+ minutes after "ready"; the update installs and
      Islandy comes back on its own.
- [ ] Tray: the update entry reads "Check for updates", then "Restart to update to …".
- [ ] Offline: "Couldn't reach the update server", no crash, retry later works.
- [ ] Weather: search a place, choose it (widget turns on), °C/°F switch, forecast matches a
      weather site; offline shows the error with **Try again**; removing the place stops requests.

**0.3 features (about 15 minutes, on the installed release)**

Claude logo on the island

- [ ] With music playing, the resting island shows: artwork · time · orange Claude logo · music
      bars. The logo breathes slowly.
- [ ] Ask Claude Code something: the island switches to "project · Working" and the logo turns.
- [ ] Click the logo: the Claude panel opens. Click the clock instead: the normal dashboard opens.
- [ ] Windows Settings › Accessibility › Visual effects › Animation effects **off**: the logo
      holds still.

Answering permission requests

- [ ] Settings › Integrations: the hooks show "Hooks need repair" (if they were installed
      before 0.3) → **Repair** → "Hooks installed".
- [ ] Turn on **Answer permission requests on the island**. Start a **new** Claude Code session
      in default permission mode and ask it to run a harmless command (e.g. `git status`).
- [ ] The Claude panel shows "Claude wants to run" with the command and a countdown.
      **Allow once** → the command runs in Claude Code.
- [ ] Ask again → **Deny** → Claude Code reports the command was denied ("Denied from Islandy.").
- [ ] Ask again → don't answer → after about 30 s Claude Code asks in its own prompt.
- [ ] Ask again → **Answer in Claude Code** → its own prompt appears right away.
- [ ] Record: while the island waits, does Claude Code's own prompt appear at once, or only
      after the island answers or gives up?
- [ ] A multi-line command (or an MCP tool) shows no **Allow once**, only Deny.

Usage glance

- [ ] After a few turns, the Claude panel header shows "Since Islandy started: N turns · …
      working · needed you …×", and the session line shows its own turns.

Weather location

- [ ] Settings › Widgets › Weather › **Use my location**: Windows may ask for permission; the
      place becomes "Current location" with rounded coordinates and the forecast loads.
- [ ] With Windows location turned off (Settings › Privacy & security › Location), the button
      shows a clear message about turning it on.

Claude Desktop

- [ ] While Claude Desktop writes a reply, the island shows "Claude Desktop · Working"; when
      it finishes, a "Claude finished responding" peek appears. If Claude Desktop is set to a
      language other than English, note the language and whether this works.

Welcome page and updates

- [ ] Upgrading kept your settings, and the welcome page did **not** appear.
- [ ] (Optional, on a spare PC or user account) a fresh install shows the welcome page once;
      **Done** closes it for good; Settings › About › **Show welcome** reopens it.
- [ ] Settings › About shows the current version and "Up to date" after **Check for updates**.

Report anything that fails with what you saw (a screenshot helps) and the Islandy version from
Settings › About.
