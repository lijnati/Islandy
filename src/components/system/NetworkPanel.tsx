import { networkSummary } from "../../features/system/system-model";
import { useSystem } from "../../features/system/system-store";
import type { RadioInfo } from "../../services/types";
import { StatusIndicator } from "../ui/StatusIndicator";
import { Switch } from "../ui/Switch";
import { DomainMessage, SettingsLink, SystemNoticeLine, Tile } from "./shared";
import { useDomain } from "../../features/system/use-domain";

const RADIO_TEXT: Record<RadioInfo["state"], string> = {
  on: "On",
  off: "Off",
  disabled: "Turned off in Windows or by a hardware switch",
  unknown: "State unknown",
};

/** A radio (Wi-Fi or Bluetooth): its real state, and a switch only if Windows allows one. */
export function RadioControl({
  radio,
  kind,
  name,
}: {
  radio: RadioInfo | null;
  kind: "wifi" | "bluetooth";
  name: string;
}) {
  const setRadio = useSystem((s) => s.setRadio);
  const busy = useSystem((s) => Boolean(s.pending[`radio:${kind}`]));
  if (!radio) {
    return <p className="text-body text-fg-muted">No {name} adapter found.</p>;
  }
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="min-w-0">
        <p className="text-title font-semibold tracking-tight text-fg">
          {radio.state === "on" ? "On" : radio.state === "off" ? "Off" : "Unavailable"}
        </p>
        <p className="truncate text-caption text-fg-muted">
          {radio.canToggle
            ? `${name} radio`
            : radio.state === "on" || radio.state === "off"
              ? "Change it in Settings"
              : RADIO_TEXT[radio.state]}
        </p>
      </div>
      {radio.canToggle && (
        <Switch
          checked={radio.state === "on"}
          busy={busy}
          label={`${name} radio`}
          onChange={(on) => void setRadio(kind, on)}
        />
      )}
    </div>
  );
}

/** Internet connectivity (as Windows assesses it) and the Wi-Fi radio. */
export function NetworkPanel() {
  const network = useDomain("network");
  const radios = useDomain("radios");
  const summary = network.kind === "ready" ? networkSummary(network.state) : null;
  return (
    <div className="flex h-full gap-3">
      <Tile
        title="Internet"
        className="flex-[1.4]"
        accessory={
          summary && (
            <StatusIndicator
              tone={summary.online ? "success" : summary.connected ? "working" : "attention"}
            />
          )
        }
      >
        <div className="flex min-h-0 flex-1 flex-col justify-between gap-2">
          {network.kind === "ready" ? (
            <div className="flex min-w-0 flex-col gap-1">
              <p className="text-heading font-semibold tracking-tight text-fg">{summary?.title}</p>
              <p title={summary?.detail} className="truncate text-body text-fg-muted">
                {summary?.detail || "Connected"}
              </p>
            </div>
          ) : (
            <DomainMessage view={network} />
          )}
          <div className="flex items-center justify-between gap-2">
            <p className="truncate text-caption text-fg-subtle">
              {network.kind === "ready" ? "Status reported by Windows" : ""}
            </p>
            <SettingsLink page="network">Network settings</SettingsLink>
          </div>
        </div>
      </Tile>
      <Tile title="Wi-Fi" className="flex-1">
        <div className="flex min-h-0 flex-1 flex-col justify-between gap-2">
          {radios.kind === "ready" ? (
            <RadioControl radio={radios.state.wifi} kind="wifi" name="Wi-Fi" />
          ) : (
            <DomainMessage view={radios} />
          )}
          <SystemNoticeLine />
          <SettingsLink page="wifi">Wi-Fi settings</SettingsLink>
        </div>
      </Tile>
    </div>
  );
}
