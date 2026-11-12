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
import {
  createPresentationCoordinator,
  type PresentationCoordinator,
} from "../activity/presentation";
import { peekForChange } from "../media/media-model";
import { mediaStore } from "../media/media-store";
import { settingsStore } from "../settings/settings-store";
import { systemStore } from "../system/system-store";
import type { InteractionSettings, ShortcutStatus } from "../../services/types";
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
    case "open": {
      // Tray left click: open the dashboard (pinned, with keyboard focus).
      const mode = islandStore.getState().mode;
      if (mode === "collapsed" || mode === "peek") dispatchIsland({ type: "SHORTCUT" });
      break;
    }
    case "hidden":
      dispatchIsland({ type: "DISMISS" });
      break;
  }
}

/** The one-time hint on first launch. */
export function welcomePeek(shortcut: ShortcutStatus | null) {
  return {
    source: "Islandy",
    title: "Islandy is running",
    detail: shortcut?.registered
      ? `Hover or ${shortcut.accelerator}`
      : "Hover, or use the tray icon",
    tone: "neutral" as const,
    icon: "message" as const,
    dwellMs: 7000,
  };
}

/** Interaction settings → island store timers and behavior. */
export function islandConfigFor(i: InteractionSettings) {
  return {
    delays: { hoverMs: i.hoverDelayMs, leaveMs: i.collapseDelayMs },
    behavior: {
      openOnHover: i.openOnHover,
      openOnClick: i.openOnClick,
      pinOnClick: i.pinOnClick,
      autoCollapse: i.autoCollapse,
    },
  };
}

/**
 * Wires the island to the native shell: signals from Rust, keyboard focus
 * hand-off, Escape, and viewport changes. Mount once, at the root.
 */
export function useNativeBridge(): void {
  // Settings: connect once; interaction preferences apply immediately, and a
  // size preset change re-fits the browser preview (Rust re-fits natively).
  useEffect(() => {
    const settings = settingsStore.getState();
    settings.start();
    let interaction = settings.state.settings.interaction;
    let size = settings.state.settings.island.size;
    islandStore.configure(islandConfigFor(interaction));
    return settingsStore.subscribe((state) => {
      const next = state.state.settings;
      if (next.interaction !== interaction) {
        interaction = next.interaction;
        islandStore.configure(islandConfigFor(interaction));
      }
      if (next.island.size !== size) {
        size = next.island.size;
        if (!isNativeAvailable()) void refreshLayouts();
      }
    });
  }, []);

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
        if (connection.firstRun) {
          presentation().offer({
            key: "welcome",
            priority: 1,
            peek: welcomePeek(connection.shortcut),
          });
        }
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
      const allowed = activityStore.getState().snapshot?.settings.peeks.media ?? true;
      if (peek && allowed) presentation().offer({ key: "media", priority: 3, peek });
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
