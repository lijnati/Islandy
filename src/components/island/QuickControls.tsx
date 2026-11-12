import { Gauge, Settings, Sun, type LucideIcon } from "lucide-react";

import { dispatchIsland, useIsland, type WidgetId } from "../../features/island/island-store";
import { useCanOpenSystemSettings } from "../../features/island/native-status";
import { networkSummary } from "../../features/system/system-model";
import { useSystem } from "../../features/system/system-store";
import { IconButton } from "../ui/IconButton";
import { bluetoothIcon, networkIcon, volumeIcon } from "../system/icons";
import { openSettings } from "../../features/system/open-settings";

type Panel = Extract<WidgetId, "network" | "bluetooth" | "audio" | "display" | "system">;

interface Control {
  panel: Panel;
  label: string;
  icon: LucideIcon;
  /** Short live status for the tooltip. */
  status: string | null;
}

function useControls(): Control[] {
  const network = useSystem((s) => s.snapshot?.network);
  const bluetooth = useSystem((s) => s.snapshot?.radios.bluetooth);
  const radiosReady = useSystem((s) => s.snapshot?.radios.status === "ready");
  const output = useSystem((s) => s.snapshot?.audio.output);
  return [
    {
      panel: "network",
      label: "Network",
      icon: networkIcon(network),
      status: network?.status === "ready" ? networkSummary(network).title : null,
    },
    {
      panel: "bluetooth",
      label: "Bluetooth",
      icon: bluetoothIcon(bluetooth),
      status: radiosReady
        ? bluetooth
          ? bluetooth.state === "on"
            ? "On"
            : "Off"
          : "No adapter"
        : null,
    },
    {
      panel: "audio",
      label: "Sound",
      icon: volumeIcon(output),
      status: output ? (output.muted ? "Muted" : `${output.volume}%`) : null,
    },
    { panel: "display", label: "Display", icon: Sun, status: null },
    { panel: "system", label: "System", icon: Gauge, status: null },
  ];
}

/**
 * Quick controls. Each opens an Islandy panel (inside the dashboard, using
 * the island's `focused` state) showing real Windows state, with native
 * controls where Windows supports them and Settings shortcuts otherwise.
 * Pressing the control of the open panel closes it again. The last button
 * opens Windows Settings itself (Windows only).
 */
export function QuickControls() {
  const activeWidget = useIsland((s) => s.activeWidget);
  const canOpen = useCanOpenSystemSettings();
  const controls = useControls();

  return (
    <div
      role="group"
      aria-label="Quick controls"
      className="flex items-center gap-0.5 rounded-full border border-hairline bg-surface-secondary p-0.5"
    >
      {controls.map(({ panel, label, icon, status }) => {
        const open = activeWidget === panel;
        return (
          <IconButton
            key={panel}
            size="sm"
            icon={icon}
            label={status ? `${label}: ${status}` : label}
            tooltip={open ? `Close ${label}` : status ? `${label} · ${status}` : label}
            pressed={open}
            onClick={() => {
              dispatchIsland(
                open ? { type: "CLOSE_WIDGET" } : { type: "OPEN_WIDGET", widget: panel },
              );
            }}
          />
        );
      })}
      <IconButton
        size="sm"
        icon={Settings}
        label={canOpen ? "Open Windows Settings" : "Windows Settings, available on Windows"}
        tooltip={canOpen ? "Windows Settings" : "Windows Settings (Windows only)"}
        tooltipAlign="end"
        disabled={!canOpen}
        onClick={() => {
          openSettings("home");
        }}
      />
    </div>
  );
}
