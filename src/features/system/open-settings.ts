import { openSystemSettings } from "../../services/native";
import type { SettingsPage } from "../../services/types";
import { dispatchIsland } from "../island/island-store";

/** Opens a Windows Settings page, then gets out of its way. */
export function openSettings(page: SettingsPage) {
  openSystemSettings(page).then(
    () => {
      dispatchIsland({ type: "DISMISS" });
    },
    (error: unknown) => {
      console.warn("[islandy] open_system_settings failed:", error);
    },
  );
}
