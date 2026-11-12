# Performance

Islandy runs all day, so idle cost matters more than peak speed.

## Budgets (targets, not yet measured on Windows)

| Metric                                      | Target                                       | Notes                                                                                       |
| ------------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Idle CPU (collapsed, nothing playing)       | < 0.1 % of one core, averaged over 60 s      | The UI does one React render per minute (clock)                                             |
| Idle wakeups                                | ≈ 1 timer per minute from the app's own code | WebView2 has its own baseline                                                               |
| Memory (private working set, all processes) | < 120 MB total                               | Islandy + WebView2 browser/renderer/GPU processes                                           |
| Cold start to visible pill                  | < 500 ms on a mid-range SSD machine          | `visible: false` until placed                                                               |
| Expand/collapse animation                   | 60 fps, no dropped frames                    | Surface animates width/height/radius of one element; content is laid out once at final size |
| Installer / binary size                     | Binary < 10 MB                               | WebView2 is shared and not bundled                                                          |

## Design choices that serve these budgets

- Clock: one minute-aligned `setTimeout`, resynchronized on visibility or focus. No per-second
  rendering.
- No animation loops, `requestAnimationFrame`, or CSS animations while idle. The only looping
  animation is the collapsed media indicator, and only while (demo) media is playing.
  `prefers-reduced-motion` (read live) disables transitions.
- Native window calls happen only at transition boundaries (grow before, shrink after an
  animation), never per frame. The WebView never resizes during an animation (fixed canvas +
  region), so there is no relayout or WebView2 resize cost mid-animation.
- The demo media progress ticks once per second only while its widget is visible and playing;
  playback position is derived from timestamps, not counted.
- Hover/leave/peek timers are derived from state: at most three, none while resting.
- No `backdrop-filter` blur on the resting pill. Blur is expensive on WebView2 and adds
  little on a near-black surface.
- The state store updates subscribers only when a transition actually changes state.
- Native integrations must be event-driven (GSMTC, Core Audio, PowerManager,
  NetworkInformation). CPU and memory sampling runs only while the system widget is visible.
- No background workers, and no plugins that spawn processes.
- Release profile: LTO, `codegen-units = 1`, `opt-level = "s"`, `panic = "abort"`, stripped.

## Measured so far (Linux development host, indicative only)

Phase 1 release build under Xvfb with software rendering (WebKitGTK, not WebView2):

- Idle CPU over 30 s, summed over `islandy` + WebKit processes: **0.20 %** of one core
  collapsed, **0.23 %** expanded and pinned (demo paused). Phase 0 measured about 0.1 %; the
  difference is within the noise of this method but not yet explained.
- Memory (PSS): about 81 MB (`islandy`) + 294 MB (web process; software rendering of the
  820 × 248 canvas inflates this, versus 176 MB for the 220 × 44 Phase 0 window) + 21 MB
  (network process). **The WebView2 equivalent must be measured on Windows** against the
  120 MB budget.

Animation timing, measured in headless Chromium by sampling the surface every frame (95 % of
the size change / within 1 px): expand ≈ 310 ms / 560 ms (3 px overshoot), collapse ≈ 275 ms
/ 425 ms, peek ≈ 195 ms / 230 ms. The horizontal center did not move by a pixel. Frame rate
was not measured (no GPU here).

These numbers do **not** represent Windows.

## How to benchmark on Windows (to do before Phase 1 sign-off)

1. **Release build only:** `pnpm tauri build`, then run the installed app.
2. **Idle CPU:** after 2 minutes of settling, record 5 minutes with
   `Get-Counter '\Process(islandy*)\% Processor Time','\Process(msedgewebview2*)\% Processor Time' -SampleInterval 1 -MaxSamples 300`.
   Attribute WebView2 processes to Islandy by their parent PID (`Get-CimInstance Win32_Process`).
   Report mean and p95.
3. **Wakeups / timers:** Windows Performance Recorder (`wpr -start CPU -start Power`), idle
   2 minutes, then `wpr -stop trace.etl`. Inspect in Windows Performance Analyzer: context
   switches and timer expirations per process.
4. **Memory:** private working set per process (Task Manager details or
   `Get-Process -Id … | Select PM, WS`) for Islandy and its WebView2 children. Record at
   start, after 1 hour, and after 8 hours to catch leaks.
5. **Startup:** time from process start to first `WindowEvent` show, using an
   `Instant` logged in debug builds, or WPR's process lifetime plus a frame trace.
   Measure both cold (after reboot) and warm starts.
6. **Animations (Phase 1):** Edge DevTools for WebView2
   (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`) Performance panel:
   no long tasks, no layout thrashing during expand and collapse.
7. Record results in this file with the machine specs, Windows build, display scaling, and the
   WebView2 version (from `get_app_info`).
