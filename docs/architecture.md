# Architecture

## Overview

```
┌──────────────────────────── Islandy process (Rust) ─────────────────────────────┐
│  app.rs        lifecycle: Builder, setup, tray, window events, shutdown        │
│  window/       geometry.rs (pure math) ─► manager.rs (IslandWindow, frame)     │
│  platform/     windows.rs (Win32: styles, region, SetWindowPos, focus, Settings)│
│                linux.rs (dev host: GTK sizing, X11 input shape)                │
│  commands.rs   IPC surface ─► services/ (app_info, system_settings, media/)    │
│  services/media/  GSMTC worker thread (MTA) ─► MediaHub ─► media Channel       │
│  signals.rs    Rust → UI push channel (toggle, focus lost, layouts changed)    │
│  shortcut.rs   global toggle shortcut (tauri-plugin-global-shortcut)           │
│  error.rs      IslandyError → { kind, message }                                │
└───────────────┬─────────────────────────────────────────────────────────────────┘
                │ Tauri IPC (invoke + two Channels), gated by capabilities/island.json
┌───────────────▼──────────── WebView2 (React) ───────────────────────────────────┐
│  services/native.ts         ◄── the only caller of invoke()                    │
│  features/island/           island-store (state machine + timers),             │
│                             window-coordinator, layouts, native bridge         │
│  features/media/            media-store (snapshots, commands) + media-model    │
│  features/calendar|activity calendar math, demo activity                       │
│  components/island/         IslandRoot, Collapsed/Peek/ExpandedIsland, toolbar │
│  components/widgets/        Media, Calendar, Activity (+ detail views)         │
│  components/ui/             IconButton, Tooltip, WidgetSurface, StatusIndicator│
│  styles/tokens.css          design tokens (Tailwind v4 @theme)                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Responsibilities

**Rust owns the native layer:** window lifecycle, size and position in physical pixels,
OS integrations (later: media, audio, system sensors, local event intake), and anything that
must keep running or be event-driven when the UI is idle. Rust pushes state to the UI; the UI
should never poll the OS.

**React owns presentation:** rendering, layout, animation, the island state model, and user
interaction. It has no direct OS access; all native capabilities go through
`src/services/native.ts`.

## IPC boundary

- Commands live in `src-tauri/src/commands.rs`. Each command delegates to a service.
- Return types derive `Serialize` with `camelCase` fields. TypeScript mirrors live in
  `src/services/types.ts`. Rust tests pin the serialized key sets so drift fails CI.
- Errors serialize as `{ kind, message }` (`IslandyError`). `native.ts` converts any rejection
  into a typed `NativeError` (`kind` of `window | monitor | platform | internal | unavailable |
unknown`).
- **Adding a command:** implement it in `commands.rs`, register it in `generate_handler!`
  (`app.rs`), list it in `build.rs` (app manifest), grant `allow-<name>` in
  `capabilities/island.json`, and wrap it in `native.ts`. Because of the app manifest,
  an ungranted command is denied rather than silently allowed.
- Push data uses Tauri `Channel`s that the UI hands to Rust, so the UI needs no `core:event`
  permissions: `connect_island` (island signals) and `connect_media` (media snapshots; kept
  separate so media state is independent of the island state machine). Pushed data is written
  into stores, never polled from components.
- Commands that wait on a worker (media) are `async`, so they never block the main thread.

## Window lifecycle

1. `tauri.conf.json` creates the `island` window hidden (`visible: false`), undecorated,
   transparent, `shadow: false`, `alwaysOnTop`, `skipTaskbar`, `focusable: false`,
   non-resizable.
2. `setup` (`app.rs`) creates the tray, registers the global shortcut, then
   `IslandWindow::initialize()`:
   - `platform::configure_island_window()` applies native styles Tauri doesn't expose
     (Windows: `WS_EX_TOOLWINDOW` to hide from Alt+Tab; Linux: allow a small GTK window).
   - The target monitor (primary → current → any) is read: **work area** and scale factor.
     `geometry::fit_layouts` fits the collapsed/peek/expanded design sizes to it, and the
     window is placed top-center (see "Window frame coordination").
3. The window is shown only after it is placed, so it never flashes.
4. On `ScaleFactorChanged`, placement is recomputed (deferred to the next event-loop turn,
   because the event can fire inside our own `SetWindowPos`) and the UI is told to re-fetch
   layouts. Resolution, scaling and taskbar changes are also picked up lazily: every
   `get_island_layouts` / `set_island_frame` call re-reads the monitor.
5. **Quit** from the tray calls `app.exit(0)`; the global shortcut is unregistered on exit.

**Pixel units.** Design sizes are logical pixels. Conversion to physical pixels happens in
`geometry.rs`, using the target monitor's own scale factor. Positions are physical
virtual-desktop coordinates, which can be negative for monitors left of or above the primary.
Fitted widths are whole logical pixels so CSS layout and native geometry agree.

**Native style caveat.** On Windows, tao recomputes and rewrites `GWL_EXSTYLE` whenever one of
its own flags changes (always-on-top, focusable, decorations, …). That drops
`WS_EX_TOOLWINDOW`. All flag changes go through `IslandWindow`, which re-applies
`platform::configure_island_window` afterwards, on the event-loop thread.

## Window frame coordination

The island animates between a 220 × 44 pill and an ~820 × 248 dashboard. Two things must
stay true at every moment: the visible surface is never clipped, and there is no large
invisible area blocking clicks to the desktop.

**Strategy: fixed canvas + window region (default, `FrameStrategy::Region`).** The native
window is a _canvas_ as large as the biggest fitted layout, centered at the top of the work
area. A window region (`SetWindowRgn` on Windows) restricts both drawing and hit-testing to
the current _frame_, a rectangle the size of the visible surface, centered at the top of the
canvas. Pixels outside the region are not part of the window, so clicks there go to whatever
is below. The frontend centers the surface at the top of the WebView, so it lines up with the
region by construction (the region is padded by one physical pixel to absorb rounding).

Why not resize the window? Resizing a WebView2-hosting window and re-centering it moves the
window's origin. Until the WebView renders a frame at the new size, Windows shows the old
frame at the new origin, so the island visibly jumps sideways for a frame or two. With a
fixed canvas the WebView never resizes during an animation, so that cannot happen, and region
updates are cheap and atomic.

**Fallback: `FrameStrategy::Resize`** (`ISLANDY_FRAME_STRATEGY=resize`). The window itself is
resized and re-centered to each frame with one atomic `SetWindowPos`. It exists in case window
regions misbehave with transparent WebView2 windows on some systems (unverified, see README).

**Coordinator (frontend, `window-coordinator.ts`).** It decides _when_ the frame changes:

- Before animating to a larger size, grow the frame to `union(current, target)` and wait for
  the IPC call to finish; only then start the animation.
- When an animation finishes, shrink the frame to the target, unless a newer transition has
  started meanwhile (generation counter).
- Native calls are serialized and happen only at those two boundaries, never per animation
  frame. Rust clamps every frame to the canvas, so a bad value can't leave the monitor.

So the frame is larger than the visible surface only while a collapse animation is running
(~250 ms), and even then only by the area the surface is shrinking out of.

**Linux (dev host).** The region is an X11 _input_ shape: hit-testing matches Windows.
The drawing clip is omitted because a bounding shape stops WebKitGTK from painting on our
Xvfb test host. Outside the island the canvas is transparent anyway (black without a
compositor).

## Media

`services/media/` runs one worker thread in the multithreaded COM apartment that owns every
GSMTC object. WinRT event handlers only post a message to it; it coalesces events (60 ms),
re-reads what changed, applies the selection policy, and publishes a revisioned snapshot
through `MediaHub` only when something meaningful changed. Commands are executed on the same
worker with timeouts. The UI's `media-store` applies snapshots only if their revision is newer.
Details: [media.md](media.md).

## Focus and keyboard

- By default the window is non-activating (`WS_EX_NOACTIVATE`): hovering and clicking
  controls do not move focus away from the user's app.
- Pinning (click on the pill or empty dashboard space, opening a widget, the global shortcut)
  makes the window focusable and focuses it (`set_island_interactive(true)`). Only then do
  Escape and Tab work, and only then can "click outside" be detected, as focus loss.
  Before focusing, Rust records the foreground window.
- Collapsing makes it non-activating again and, if the island still had focus (Escape, the
  collapse button), gives focus back to the recorded window.
- **Click-outside** = `WindowEvent::Focused(false)`, debounced 150 ms and confirmed with
  `is_focused()`, because toggling focusability can cause an instant blur/re-focus. No global
  mouse hook is installed.
- **Global shortcut** `Ctrl+Alt+Space` (override: `ISLANDY_TOGGLE_SHORTCUT`). Registered from
  Rust with `tauri-plugin-global-shortcut`. If another app owns it, registration fails, the
  error is logged and reported to the UI (`connect_island`), and nothing is overridden.

## Island state machine

`src/features/island/island-store.ts`: a pure `transition(state, event)` plus a Zustand store.

State: `mode` (`collapsed | peek | expanded | focused`), `pinned`, `hovering`, `activeWidget`,
`peek` content, `peekSeq`.

| From → to            | Trigger                                                     |
| -------------------- | ----------------------------------------------------------- |
| collapsed → expanded | pointer rests 150 ms (`HOVER_INTENT`), or click (pins)      |
| expanded → collapsed | pointer away 360 ms (`LEAVE_INTENT`), unless pinned         |
| expanded → focused   | open a widget's detail view (also pins)                     |
| focused → expanded   | back button, Escape                                         |
| collapsed → peek     | `CONTEXT_EVENT` (tray demo, demo track auto-advance)        |
| peek → peek          | a newer `CONTEXT_EVENT` replaces it (`peekSeq + 1`)         |
| peek → collapsed     | dwell 4 s elapsed (`PEEK_TIMEOUT` with matching `peekSeq`)  |
| peek → expanded      | hover intent or click                                       |
| any → collapsed      | Escape (from expanded), focus loss, collapse button, toggle |

Rules:

1. **User beats system.** Contextual events never interrupt the dashboard; they are dropped
   (Phase 2 may queue them).
2. **Timers are derived from state.** After every transition, each timer slot (hover, leave,
   peek) holds at most one timer whose existence and identity follow from the new state.
   Obsolete timers are cancelled by construction; a stale peek timeout carries its `peekSeq`
   and is ignored by a newer peek.
3. **No flicker.** Collapsing clears `hovering`, so a pointer resting over the shrinking
   surface doesn't immediately re-expand it; a fresh pointer-enter is required.
4. **One surface, one target.** The surface animates to a single target size; a new target
   retargets the running spring instead of stacking animations.

## Animation

`IslandRoot` renders one `motion.div` surface that animates `width`, `height` and
`borderRadius` with springs (`design/tokens.ts`): expand ≈ 340 ms with slight overshoot,
collapse ≈ 240 ms without, peek ≈ 220 ms. Each view inside (pill, peek, dashboard) is laid
out at its final size and clipped by the surface, so nothing reflows mid-animation; views
cross-fade (out 90 ms, in 200 ms) and dashboard widgets enter with a 35 ms stagger.
`prefers-reduced-motion` (read live) turns all of this into instant changes. While idle
nothing animates; the collapsed activity bars animate (CSS `transform`) only while media is
playing.

## Frontend conventions

- Components do not call `invoke`; they call `services/native.ts`.
- Timers are minute-aligned or event-driven, never per-frame or per-second while idle
  (the demo media progress ticks once per second only while its widget is visible and playing).
- Demo data lives in `features/activity/demo-activity.ts`, is labelled "Demo" in the UI, and
  is replaced by a native store in Phase 4. Media is real since Phase 2.
- Native state lives in vanilla Zustand stores started once at the root (`useNativeBridge`);
  starting is idempotent so React Strict Mode cannot create duplicate subscriptions.
- Visual values come from `styles/tokens.css`. The few values JavaScript needs are mirrored
  in `design/tokens.ts`.
- The document, body and `#root` stay transparent. Only the island surface paints.

## Related documents

- [media.md](media.md): GSMTC integration, selection policy, events, artwork.
- [security.md](security.md): capabilities, CSP, and local-integration rules.
- [performance.md](performance.md): budgets and the benchmark plan.
- [windows-integration.md](windows-integration.md): research for future native features.
