# Performance

Islandy runs all day, so idle cost matters more than peak speed.

## Budgets (targets, not yet measured on Windows)

| Metric                                      | Target                                       | Notes                                             |
| ------------------------------------------- | -------------------------------------------- | ------------------------------------------------- |
| Idle CPU (collapsed, nothing playing)       | < 0.1 % of one core, averaged over 60 s      | The UI does one React render per minute (clock)   |
| Idle wakeups                                | ≈ 1 timer per minute from the app's own code | WebView2 has its own baseline                     |
| Memory (private working set, all processes) | < 120 MB total                               | Islandy + WebView2 browser/renderer/GPU processes |
| Cold start to visible pill                  | < 500 ms on a mid-range SSD machine          | `visible: false` until placed                     |
| Expand/collapse animation                   | 60 fps, no dropped frames                    | Phase 1; GPU-composited transforms/opacity only   |
| Installer / binary size                     | Binary < 10 MB                               | WebView2 is shared and not bundled                |

## Design choices that serve these budgets

- Clock: one minute-aligned `setTimeout`, resynchronized on visibility or focus. No per-second
  rendering.
- No animation loops, `requestAnimationFrame`, or CSS animations while idle.
  `prefers-reduced-motion` disables transitions.
- No `backdrop-filter` blur on the resting pill. Blur is expensive on WebView2 and adds
  little on a near-black surface.
- The state store updates subscribers only when a transition actually changes state.
- Native integrations must be event-driven (GSMTC, Core Audio, PowerManager,
  NetworkInformation). CPU and memory sampling runs only while the system widget is visible.
- No background workers, and no plugins that spawn processes.
- Release profile: LTO, `codegen-units = 1`, `opt-level = "s"`, `panic = "abort"`, stripped.

## Measured so far (Linux development host, indicative only)

Measured on the Phase 0 release build under Xvfb with software rendering (WebKitGTK, not
WebView2), over 30 s idle:

- CPU: about 0.1 % of one core, summed over `islandy` + `WebKitWebProcess` + `WebKitNetworkProcess`.
- Memory (PSS): about 79 MB (`islandy`) + 176 MB (web process, inflated by software
  rendering) + 21 MB (network process).

These numbers do **not** represent Windows. They only show that nothing is busy-looping.

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
