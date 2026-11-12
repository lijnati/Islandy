import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { dispatchIsland, islandStore } from "./features/island/island-store";
import "./styles/index.css";

// Dev server only (stripped from production builds): lets browser-based
// visual checks drive the state machine, e.g. to preview a peek.
if (import.meta.env.DEV) {
  Object.assign(window, { __islandy: { dispatch: dispatchIsland, store: islandStore } });
}

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
