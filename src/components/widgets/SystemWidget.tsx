import { BatteryCharging, BatteryFull, Bluetooth, Plug } from "lucide-react";

import { dispatchIsland } from "../../features/island/island-store";
import {
  batteryPercent,
  networkSummary,
  powerDescription,
} from "../../features/system/system-model";
import { useDomain } from "../../features/system/use-domain";
import { networkIcon } from "../system/icons";
import { WidgetSurface } from "../ui/WidgetSurface";
import type { LucideIcon } from "lucide-react";

function Row({ icon: Icon, title, detail }: { icon: LucideIcon; title: string; detail: string }) {
  return (
    <li className="flex min-w-0 items-center gap-2.5">
      <span className="flex size-7 shrink-0 items-center justify-center rounded-full bg-surface-raised">
        <Icon size={14} strokeWidth={2} aria-hidden="true" className="text-fg" />
      </span>
      <span className="flex min-w-0 flex-col">
        <span className="truncate text-body font-medium text-fg">{title}</span>
        <span className="truncate text-caption text-fg-muted">{detail}</span>
      </span>
    </li>
  );
}

const MESSAGES = {
  browser: "Available in the desktop app",
  unsupported: "Not supported here",
  unavailable: "Couldn't be read",
  loading: "Loading…",
} as const;

/** Battery, network and Bluetooth at a glance. Opens the System panel. */
export function SystemWidget({ className }: { className?: string }) {
  const power = useDomain("power");
  const network = useDomain("network");
  const radios = useDomain("radios");

  const powerRow =
    power.kind === "ready"
      ? (() => {
          const percent = batteryPercent(power.state.battery);
          const charging =
            power.state.battery.presence === "present" && power.state.source === "ac";
          return {
            icon:
              power.state.battery.presence === "absent"
                ? Plug
                : charging
                  ? BatteryCharging
                  : BatteryFull,
            title: percent === null ? "Power" : `${percent}%`,
            detail: powerDescription(power.state),
          };
        })()
      : { icon: BatteryFull, title: "Power", detail: MESSAGES[power.kind] };

  const net = network.kind === "ready" ? networkSummary(network.state) : null;
  const bt = radios.kind === "ready" ? radios.state.bluetooth : null;

  return (
    <WidgetSurface
      title="System"
      onOpen={() => {
        dispatchIsland({ type: "OPEN_WIDGET", widget: "system" });
      }}
      className={className}
    >
      <ul className="flex flex-1 flex-col justify-center gap-2.5">
        <Row {...powerRow} />
        <Row
          icon={networkIcon(network.kind === "ready" ? network.state : null)}
          title={net?.title ?? "Network"}
          detail={
            net ? net.detail || "—" : MESSAGES[network.kind === "ready" ? "loading" : network.kind]
          }
        />
        <Row
          icon={Bluetooth}
          title="Bluetooth"
          detail={
            radios.kind !== "ready"
              ? MESSAGES[radios.kind]
              : !bt
                ? "No adapter"
                : bt.state === "on"
                  ? "On"
                  : bt.state === "off"
                    ? "Off"
                    : bt.state === "disabled"
                      ? "Adapter disabled"
                      : "Unknown"
          }
        />
      </ul>
    </WidgetSurface>
  );
}
