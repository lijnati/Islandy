import {
  Asterisk,
  Battery,
  BatteryLow,
  FlaskConical,
  Hammer,
  MessageSquare,
  Network,
  Plug,
  Server,
  Speaker,
  Wifi,
  WifiOff,
  type LucideIcon,
} from "lucide-react";

import type { PeekIcon } from "../../features/island/island-store";

/** Icons for peeks and activity rows. */
export const ACTIVITY_ICONS: Record<PeekIcon, LucideIcon> = {
  battery: Battery,
  batteryLow: BatteryLow,
  plug: Plug,
  wifi: Wifi,
  network: Network,
  offline: WifiOff,
  speaker: Speaker,
  // A generic asterisk (not Anthropic's mark) stands for Claude Code.
  claude: Asterisk,
  build: Hammer,
  tests: FlaskConical,
  server: Server,
  message: MessageSquare,
};
