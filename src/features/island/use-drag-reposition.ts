import { useRef, type PointerEvent } from "react";

import { previewIslandOffset, isNativeAvailable } from "../../services/native";
import { settingsStore } from "../settings/settings-store";
import { dispatchIsland } from "./island-store";

/** Movement (px) after which a press counts as a drag, not a click. */
export const DRAG_THRESHOLD = 4;

interface DragState {
  pointerId: number;
  startX: number;
  startOffset: number;
  applied: number;
  moved: boolean;
  frame: number | null;
  pendingX: number;
}

/**
 * Alt + drag on the collapsed pill moves the island horizontally (when
 * "Drag to reposition" is on). Deliberate by design: a plain press still
 * opens the dashboard, and widgets are never draggable. The offset is
 * previewed natively while dragging (clamped on screen by Rust) and saved
 * on release.
 */
export function useDragReposition() {
  const drag = useRef<DragState | null>(null);
  /** Set after a drag so the click that ends it doesn't open the dashboard. */
  const suppressClick = useRef(false);

  const enabled = () =>
    isNativeAvailable() && settingsStore.getState().state.settings.island.dragToReposition;

  const onPointerDown = (event: PointerEvent<HTMLElement>) => {
    if (!event.altKey || event.button !== 0 || !enabled()) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    // Cancel the hover-intent timer: a drag must never expand the island.
    dispatchIsland({ type: "POINTER_LEAVE" });
    const offset = settingsStore.getState().state.settings.island.offsetX;
    drag.current = {
      pointerId: event.pointerId,
      startX: event.screenX,
      startOffset: offset,
      applied: offset,
      moved: false,
      frame: null,
      pendingX: event.screenX,
    };
  };

  const onPointerMove = (event: PointerEvent<HTMLElement>) => {
    const d = drag.current;
    if (d?.pointerId !== event.pointerId) return;
    d.pendingX = event.screenX;
    if (Math.abs(d.pendingX - d.startX) >= DRAG_THRESHOLD) d.moved = true;
    if (!d.moved || d.frame !== null) return;
    // At most one native move per animation frame.
    d.frame = requestAnimationFrame(() => {
      d.frame = null;
      const wanted = d.startOffset + (d.pendingX - d.startX);
      previewIslandOffset(wanted).then(
        (applied) => {
          d.applied = applied;
        },
        (error: unknown) => {
          console.warn("[islandy] preview_island_offset failed:", error);
        },
      );
    });
  };

  const finish = (event: PointerEvent<HTMLElement>) => {
    const d = drag.current;
    if (d?.pointerId !== event.pointerId) return;
    drag.current = null;
    if (d.frame !== null) cancelAnimationFrame(d.frame);
    if (!d.moved) return;
    suppressClick.current = true;
    const offset = Math.round(d.applied);
    void settingsStore
      .getState()
      .update((s) => ({ ...s, island: { ...s.island, offsetX: offset } }));
  };

  /** Call from onClick: returns true if this click ended a drag. */
  const consumeDragClick = () => {
    const was = suppressClick.current;
    suppressClick.current = false;
    return was;
  };

  return {
    handlers: { onPointerDown, onPointerMove, onPointerUp: finish, onPointerCancel: finish },
    consumeDragClick,
  };
}
