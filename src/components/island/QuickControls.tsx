import { Bluetooth, Monitor, Settings, Volume2, Wifi, type LucideIcon } from "lucide-react";

import { dispatchIsland } from "../../features/island/island-store";
import { useCanOpenSystemSettings } from "../../features/island/native-status";
import { openSystemSettings } from "../../services/native";
import type { SettingsPage } from "../../services/types";
import { IconButton } from "../ui/IconButton";

interface Control {
  page: SettingsPage | null;
  label: string;
  icon: LucideIcon;
}

const CONTROLS: readonly Control[] = [
  { page: "wifi", label: "Wi-Fi", icon: Wifi },
  { page: "bluetooth", label: "Bluetooth", icon: Bluetooth },
  { page: "sound", label: "Sound", icon: Volume2 },
  { page: "display", label: "Display", icon: Monitor },
  { page: null, label: "Islandy settings", icon: Settings },
];

/**
 * Honest quick controls (Phase 1): each opens the matching Windows Settings
 * page — none pretends to toggle anything. Islandy's own settings are marked
 * as coming soon. Outside Windows they are disabled and say so.
 */
export function QuickControls() {
  const canOpen = useCanOpenSystemSettings();

  return (
    <div
      role="group"
      aria-label="Quick settings"
      className="flex items-center gap-0.5 rounded-full border border-hairline bg-surface-secondary p-0.5"
    >
      {CONTROLS.map(({ page, label, icon }, i) => {
        const last = i === CONTROLS.length - 1;
        if (page === null) {
          return (
            <IconButton
              key={label}
              size="sm"
              icon={icon}
              label={`${label} (coming soon)`}
              tooltip="Settings — coming soon"
              tooltipAlign="end"
              disabled
            />
          );
        }
        const tooltip = canOpen ? `${label} settings` : `${label} settings (Windows only)`;
        return (
          <IconButton
            key={label}
            size="sm"
            icon={icon}
            label={canOpen ? `Open ${label} settings` : `${label} settings, available on Windows`}
            tooltip={tooltip}
            tooltipAlign={last ? "end" : "center"}
            disabled={!canOpen}
            onClick={() => {
              openSystemSettings(page).then(
                () => {
                  // Settings takes focus; get out of its way.
                  dispatchIsland({ type: "DISMISS" });
                },
                (error: unknown) => {
                  console.warn("[islandy] open_system_settings failed:", error);
                },
              );
            }}
          />
        );
      })}
    </div>
  );
}
