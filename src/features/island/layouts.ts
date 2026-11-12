import { createStore, useStore } from "zustand";

import { designLayouts } from "../../design/tokens";
import { getIslandLayouts, isNativeAvailable } from "../../services/native";
import type { IslandLayouts, Size } from "../../services/types";
import type { IslandMode } from "./island-store";

const EDGE_MARGIN = 12;

/** Browser fallback: fit the design targets to the viewport like Rust does. */
export function fitToViewport(
  viewport: Size,
  layouts: IslandLayouts = designLayouts,
): IslandLayouts {
  const maxWidth = Math.max(1, Math.floor(viewport.width - 2 * EDGE_MARGIN));
  const maxHeight = Math.max(1, Math.floor(viewport.height));
  const fit = (s: Size): Size => ({
    width: Math.min(s.width, maxWidth),
    height: Math.min(s.height, maxHeight),
  });
  return {
    collapsed: fit(layouts.collapsed),
    ambient: fit(layouts.ambient),
    peek: fit(layouts.peek),
    expanded: fit(layouts.expanded),
  };
}

/** `ambient`: the collapsed pill shows activity (wider pill). */
export function layoutFor(mode: IslandMode, layouts: IslandLayouts, ambient = false): Size {
  switch (mode) {
    case "collapsed":
      return ambient ? layouts.ambient : layouts.collapsed;
    case "peek":
      return layouts.peek;
    case "expanded":
    case "focused":
      return layouts.expanded;
  }
}

interface LayoutState {
  layouts: IslandLayouts;
  source: "design" | "native";
}

export const layoutStore = createStore<LayoutState>()(() => ({
  layouts: designLayouts,
  source: "design",
}));

export function useLayouts(): IslandLayouts {
  return useStore(layoutStore, (s) => s.layouts);
}

/** Fetches fitted layouts from Rust, or fits to the viewport in a browser. */
export async function refreshLayouts(): Promise<void> {
  if (!isNativeAvailable()) {
    layoutStore.setState({
      layouts: fitToViewport({ width: window.innerWidth, height: window.innerHeight }),
      source: "design",
    });
    return;
  }
  try {
    layoutStore.setState({ layouts: await getIslandLayouts(), source: "native" });
  } catch (error) {
    console.warn("[islandy] get_island_layouts failed:", error);
  }
}
