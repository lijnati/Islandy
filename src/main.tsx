import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { dispatchIsland, islandStore } from "./features/island/island-store";
import { mediaStore } from "./features/media/media-store";
import "./styles/index.css";

// Dev server only (stripped from production builds): lets browser-based
// visual checks drive the state machine (e.g. preview a peek) and inject
// media snapshots to preview media states without Windows.
if (import.meta.env.DEV) {
  Object.assign(window, {
    __islandy: { dispatch: dispatchIsland, store: islandStore, media: mediaStore },
  });
}

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
