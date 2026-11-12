# Architecture

## Overview

```
┌──────────────────────────── Islandy process (Rust) ─────────────────────────────┐
│  app.rs        lifecycle: Builder, setup, tray, window events                  │
│  window/       geometry.rs (pure math) ─► manager.rs (IslandWindow)            │
│  platform/     windows.rs (Win32 styles) · linux.rs (dev host)                 │
│  commands.rs   IPC surface ─► services/ (app_info)                              │
│  error.rs      IslandyError → { kind, message }                                │
└───────────────┬─────────────────────────────────────────────────────────────────┘
                │ Tauri IPC (invoke), gated by capabilities/island.json
┌───────────────▼──────────── WebView2 (React) ───────────────────────────────────┐
│  services/native.ts  ◄── the only caller of invoke()                           │
│  store/island.ts     Zustand: mode + pure transition()                         │
│  hooks/              useClock (minute-aligned), useAppInfo                     │
│  features/island/    Island, Clock                                             │
│  styles/tokens.css   design tokens (Tailwind v4 @theme)                        │
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
- Future push data (media changes, system stats) will use Tauri events or channels from Rust.
  It will be subscribed in `native.ts` and written into stores, never polled from components.

## Window lifecycle

1. `tauri.conf.json` creates the `island` window hidden (`visible: false`), undecorated,
   transparent, `shadow: false`, `alwaysOnTop`, `skipTaskbar`, `focusable: false`,
   non-resizable, 220 × 44 logical px.
2. `setup` (`app.rs`) creates the tray, then `IslandWindow::initialize()`:
   - `platform::configure_island_window()` applies the native styles Tauri doesn't expose
     (Windows: `WS_EX_TOOLWINDOW` to hide from Alt+Tab; Linux: allow a small GTK window).
   - `apply_size(COLLAPSED)` picks the target monitor (primary → current → any), reads its
     **work area** and scale factor, computes physical bounds with `geometry::top_center`, and
     applies them: position, then size, then position again so a cross-monitor DPI change
     can't leave it off-center.
3. The window is shown only after it is correctly placed, so it never flashes.
4. On `ScaleFactorChanged`, the island re-centers.
5. **Quit** from the tray calls `app.exit(0)`.

**Pixel units.** Design sizes are logical pixels. Conversion to physical pixels happens in one
place (`geometry.rs`), using the target monitor's own scale factor. Positions are in physical
virtual-desktop coordinates, which can be negative for monitors left of or above the primary.

**Resizing (Phase 1).** `apply_size(LogicalSize)` is the hook for collapsed/peek/expanded
presets. The window is sized to the visible surface at all times, so nothing invisible
blocks clicks. Expansion therefore resizes the native window. To avoid a visible jump, the
recommended sequence is: grow the window first, then animate the content inside it. To
collapse, animate the content first, then shrink the window. Changing size and position
together in one `SetWindowPos` call (a small Win32 helper) will avoid a frame where the
window has its new size but its old position.

**Native style caveat.** On Windows, tao recomputes and rewrites `GWL_EXSTYLE` whenever one of
its own flags changes (always-on-top, focusable, decorations, …). That drops
`WS_EX_TOOLWINDOW`. All flag changes must go through `IslandWindow`, which must re-apply
`platform::configure_island_window` afterwards.

## Island state model

Defined in `src/store/island.ts` as a pure `transition(state, event)` function. It is unit
tested, and wrapped by a Zustand store whose `dispatch` only notifies subscribers when the
state actually changes.

| Mode        | Initiator | Temporary?  | Leaves on                                                                            |
| ----------- | --------- | ----------- | ------------------------------------------------------------------------------------ |
| `collapsed` | n/a       | no          | `CONTEXT_EVENT` → peek, `POINTER_ENTER` → expanded, `CLICK` → focused                |
| `peek`      | system    | yes         | `PEEK_TIMEOUT` → collapsed, `POINTER_ENTER` → expanded, `CLICK` → focused, `DISMISS` |
| `expanded`  | user      | yes         | `POINTER_LEAVE` → collapsed, `CLICK` → focused, `DISMISS`                            |
| `focused`   | user      | no (sticky) | `DISMISS` → collapsed only                                                           |

Override rules:

1. **User beats system.** `CONTEXT_EVENT` and `PEEK_TIMEOUT` are ignored in `expanded` and
   `focused`. A contextual event that arrives meanwhile can be surfaced after the user leaves
   (Phase 1 decides whether to queue or drop).
2. **Newest peek wins.** A `CONTEXT_EVENT` during `peek` increments `peekId`, so the UI
   restarts the dwell timer instead of stacking animations.
3. **No conflicting animations.** Because there is one `mode` value and every transition is a
   pure function of the current mode, an animation is always keyed to a single target mode.
   With Motion, `AnimatePresence`/layout animations keyed on `mode` interrupt and retarget
   instead of running concurrently.

Phase 0 renders only `collapsed`, and pointer events are not wired yet. Wiring them would
change state without any visible effect.

## Frontend conventions

- Components do not call `invoke`; they call `services/native.ts`.
- Timers are minute-aligned or event-driven, never per-frame or per-second while idle.
- Visual values come from `styles/tokens.css`. The few values JavaScript needs are mirrored
  in `design/tokens.ts`.
- The document, body and `#root` stay transparent. Only the island surface paints.

## Related documents

- [security.md](security.md): capabilities, CSP, and local-integration rules.
- [performance.md](performance.md): budgets and the benchmark plan.
- [windows-integration.md](windows-integration.md): research for future native features.
