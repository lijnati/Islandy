import {
  Bluetooth,
  BluetoothOff,
  Volume,
  Volume1,
  Volume2,
  VolumeX,
  Wifi,
  WifiOff,
  type LucideIcon,
} from "lucide-react";

import type { AudioOutput, NetworkState, RadioInfo } from "../../services/types";

export function volumeIcon(
  output: AudioOutput | null | undefined,
  level?: number | null,
): LucideIcon {
  if (!output) return Volume2;
  const v = level ?? output.volume;
  if (output.muted) return VolumeX;
  if (v === 0) return Volume;
  return v < 50 ? Volume1 : Volume2;
}

export function networkIcon(network: NetworkState | null | undefined): LucideIcon {
  if (!network || network.status !== "ready") return Wifi;
  return network.connectivity === "none" ? WifiOff : Wifi;
}

export function bluetoothIcon(radio: RadioInfo | null | undefined): LucideIcon {
  return radio && radio.state !== "on" ? BluetoothOff : Bluetooth;
}
