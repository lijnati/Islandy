import { useEffect } from "react";

import {
  connectIsland,
  getAppInfo,
  isNativeAvailable,
  setIslandInteractive,
} from "../../services/native";
import type { IslandSignal } from "../../services/types";
import { DEMO_PEEK } from "../activity/demo-activity";
import { peekForChange } from "../media/media-model";
import { mediaStore } from "../media/media-store";
import { alertToPeek } from "../system/system-model";
import { systemStore } from "../system/system-store";
import type { SystemFocus } from "../../services/types";
import { dispatchIsland, islandStore, type IslandState } from "./island-store";
import { refreshLayouts } from "./layouts";
import { nativeStatusStore } from "./native-status";

/** Which system view is visible: drives native CPU/RAM and brightness polling. */
export function systemFocusFor(state: Pick<IslandState, "mode" | "activeWidget">): SystemFocus {
  if (state.mode !== "focused") return "none";
  if (state.activeWidget === "system") return "overview";
  if (state.activeWidget === "display") return "display";
  return "none";
}

function onSignal(signal: IslandSignal) {
  switch (signal.type) {
    case "toggle":
      dispatchIsland({ type: "SHORTCUT" });
      break;
    case "focusLost":
      // Click outside / Alt+Tab while the island was interactive.
      if (islandStore.getState().pinned) dispatchIsland({ type: "DISMISS" });
      break;
    case "layoutsChanged":
      void refreshLayouts();
      break;
    case "demoPeek":
      dispatchIsland({ type: "CONTEXT_EVENT", peek: DEMO_PEEK });
      break;
  }
}

/**
 * Wires the island to the native shell: signals from Rust, keyboard focus
 * hand-off, Escape, and viewport changes. Mount once, at the root.
 */
export function useNativeBridge(): void {
  // Layouts + signal channel.
  useEffect(() => {
    void refreshLayouts();
    if (!isNativeAvailable()) {
      const onResize = () => void refreshLayouts();
      window.addEventListener("resize", onResize);
      return () => {
        window.removeEventListener("resize", onResize);
      };
    }
    connectIsland(onSignal).then(
      (connection) => {
        nativeStatusStore.setState({ connection });
      },
      (error: unknown) => {
        console.warn("[islandy] connect_island failed:", error);
      },
    );
    getAppInfo().then(
      (appInfo) => {
        nativeStatusStore.setState({ appInfo });
      },
      (error: unknown) => {
        console.warn("[islandy] get_app_info failed:", error);
      },
    );
    return undefined;
  }, []);

  // Keyboard focus follows `pinned`: the island only becomes focusable after
  // an explicit click / shortcut / widget open, never on hover.
  useEffect(() => {
    if (!isNativeAvailable()) return undefined;
    let interactive = false;
    return islandStore.subscribe((state) => {
      if (state.pinned === interactive) return;
      interactive = state.pinned;
      setIslandInteractive(interactive).catch((error: unknown) => {
        console.warn("[islandy] set_island_interactive failed:", error);
      });
    });
  }, []);

  // Escape: leave a detail view, then collapse.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        dispatchIsland({ type: "ESCAPE" });
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  // Media: connect once (idempotent, so Strict Mode's double effect is
  // harmless) and peek briefly when a new track starts playing. Peeks never
  // interrupt the dashboard (the state machine drops them there).
  useEffect(() => {
    const media = mediaStore.getState();
    media.start();
    let previous = media.snapshot;
    return mediaStore.subscribe((state) => {
      const next = state.snapshot;
      if (next === previous || next === null) return;
      const peek = peekForChange(previous, next);
      previous = next;
      if (peek) dispatchIsland({ type: "CONTEXT_EVENT", peek });
    });
  }, []);

  // System: connect once; tell Rust what is visible (sampling runs only
  // while a panel needs it); turn de-duplicated alerts into peeks. The
  // island state machine drops peeks while the dashboard is open, so they
  // never interrupt the user.
  useEffect(() => {
    const system = systemStore.getState();
    system.start();
    const sync = (state: IslandState) => {
      systemStore.getState().setFocus(systemFocusFor(state));
    };
    sync(islandStore.getState());
    const unsubscribeIsland = islandStore.subscribe(sync);
    const unsubscribeAlerts = system.onAlert((alert) => {
      dispatchIsland({ type: "CONTEXT_EVENT", peek: alertToPeek(alert) });
    });
    return () => {
      unsubscribeIsland();
      unsubscribeAlerts();
      systemStore.getState().setFocus("none");
    };
  }, []);
}
