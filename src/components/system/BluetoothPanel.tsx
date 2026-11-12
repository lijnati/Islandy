import { Bluetooth } from "lucide-react";

import { RadioControl } from "./NetworkPanel";
import { DomainMessage, SettingsLink, SystemNoticeLine, Tile } from "./shared";
import { useDomain } from "../../features/system/use-domain";

/** The Bluetooth radio. Paired devices are deliberately not listed. */
export function BluetoothPanel() {
  const radios = useDomain("radios");
  return (
    <div className="flex h-full gap-3">
      <Tile title="Bluetooth" className="flex-[1.4]">
        <div className="flex min-h-0 flex-1 flex-col justify-between gap-2">
          {radios.kind === "ready" ? (
            <RadioControl radio={radios.state.bluetooth} kind="bluetooth" name="Bluetooth" />
          ) : (
            <DomainMessage view={radios} />
          )}
          <SystemNoticeLine />
        </div>
      </Tile>
      <Tile title="Devices" className="flex-1">
        <div className="flex min-h-0 flex-1 flex-col justify-between gap-2">
          <div className="flex items-start gap-2 text-body text-fg-muted">
            <Bluetooth size={16} strokeWidth={1.75} aria-hidden="true" className="mt-px shrink-0" />
            <p>Pair, connect and manage devices in Windows Settings.</p>
          </div>
          <SettingsLink page="bluetooth">Bluetooth settings</SettingsLink>
        </div>
      </Tile>
    </div>
  );
}
