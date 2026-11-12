import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { SettingsApp } from "./features/settings/components/SettingsApp";
import { settingsStore } from "./features/settings/settings-store";
import "./styles/index.css";
import "./styles/settings.css";

if (import.meta.env.DEV) {
  Object.assign(window, { __islandy: { settings: settingsStore } });
}

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from settings.html");

createRoot(root).render(
  <StrictMode>
    <SettingsApp />
  </StrictMode>,
);
