import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { activityStore } from "./features/activity/activity-store";
import { dispatchIsland, islandStore } from "./features/island/island-store";
import { mediaStore } from "./features/media/media-store";
import { systemStore } from "./features/system/system-store";
import "./styles/index.css";

// Dev server only (stripped from production builds): lets browser-based
// visual checks drive the state machine (e.g. preview a peek) and inject
// media/system state to preview Windows states on other platforms.
if (import.meta.env.DEV) {
  Object.assign(window, {
    __islandy: {
      dispatch: dispatchIsland,
      store: islandStore,
      media: mediaStore,
      system: systemStore,
      activity: activityStore,
    },
  });
}

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
