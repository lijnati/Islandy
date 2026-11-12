import { useEffect } from "react";

import {
  connectIsland,
  getAppInfo,
  isNativeAvailable,
  setIslandInteractive,
} from "../../services/native";
import type { IslandSignal } from "../../services/types";
import { peekForActivity } from "../activity/activity-model";
import { activityStore } from "../activity/activity-store";
import { DEMO_PEEK } from "../activity/demo-activity";
import {
  createPresentationCoordinator,
  type PresentationCoordinator,
} from "../activity/presentation";
import { peekForChange } from "../media/media-model";
import { mediaStore } from "../media/media-store";
import { systemStore } from "../system/system-store";
import type { SystemFocus } from "../../services/types";
import { dispatchIsland, islandStore, type IslandState } from "./island-store";

/**
 * The app-wide presentation coordinator: every automatic peek (activity,
 * system alerts via the activity engine, media) goes through it, so
 * priorities, quiet mode and pacing apply to all of them.
 */
let coordinator: PresentationCoordinator | null = null;
function presentation(): PresentationCoordinator {
  coordinator ??= createPresentationCoordinator({
    island: {
      getState: () => islandStore.getState(),
      subscribe: (listener) => islandStore.subscribe(listener),
      dispatch: dispatchIsland,
    },
    quiet: () => activityStore.getState().snapshot?.settings.quiet ?? false,
  });
  return coordinator;
}
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
      // Media is ambient: it never replaces a more important peek.
      if (peek) presentation().offer({ key: "media", priority: 3, peek });
    });
  }, []);

  // System: connect once; tell Rust what is visible (sampling runs only
  // while a panel needs it). System alerts reach the island through the
  // activity engine (history, priority, quiet mode), not from here.
  useEffect(() => {
    const system = systemStore.getState();
    system.start();
    const sync = (state: IslandState) => {
      systemStore.getState().setFocus(systemFocusFor(state));
    };
    sync(islandStore.getState());
    const unsubscribeIsland = islandStore.subscribe(sync);
    return () => {
      unsubscribeIsland();
      systemStore.getState().setFocus("none");
    };
  }, []);

  // Activity: connect once; peek requests from the engine go through the
  // presentation coordinator, which never interrupts the dashboard.
  useEffect(() => {
    const activity = activityStore.getState();
    activity.start();
    const unsubscribe = activity.onPeek((request) => {
      presentation().offer({
        key: request.event.dedupeKey ?? request.event.id,
        priority: request.event.priority,
        peek: peekForActivity(request),
      });
    });
    // Turning quiet mode on drops anything queued.
    let quiet = activity.snapshot?.settings.quiet ?? false;
    const unsubscribeQuiet = activityStore.subscribe((state) => {
      const next = state.snapshot?.settings.quiet ?? false;
      if (next && !quiet) presentation().clear();
      quiet = next;
    });
    return () => {
      unsubscribe();
      unsubscribeQuiet();
    };
  }, []);
}
