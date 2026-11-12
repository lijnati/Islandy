import {
  Battery,
  BatteryLow,
  Check,
  Network,
  Plug,
  Sparkle,
  Speaker,
  Wifi,
  WifiOff,
  type LucideIcon,
} from "lucide-react";

import {
  dispatchIsland,
  type PeekContent,
  type PeekIcon,
} from "../../features/island/island-store";
import { MediaArtwork } from "../widgets/Artwork";
import { ActivityBars } from "./ActivityBars";
import { focusRing } from "../ui/IconButton";
import { StatusIndicator } from "../ui/StatusIndicator";
import { cx } from "../ui/cx";

const PEEK_ICONS: Record<PeekIcon, LucideIcon> = {
  battery: Battery,
  batteryLow: BatteryLow,
  plug: Plug,
  wifi: Wifi,
  network: Network,
  offline: WifiOff,
  speaker: Speaker,
};

function PeekGlyph({ peek }: { peek: PeekContent }) {
  const Icon = peek.icon ? PEEK_ICONS[peek.icon] : peek.tone === "success" ? Check : Sparkle;
  return (
    <span
      aria-hidden="true"
      className={cx(
        "flex size-8 shrink-0 items-center justify-center rounded-full bg-surface-elevated",
        peek.tone === "attention" ? "text-status-attention" : "text-fg",
      )}
    >
      <Icon size={15} strokeWidth={2} />
    </span>
  );
}

/** A temporary contextual capsule. Announced politely to screen readers. */
export function PeekIsland({ peek }: { peek: PeekContent }) {
  return (
    <button
      type="button"
      onClick={() => {
        dispatchIsland({ type: "PRESS" });
      }}
      aria-label={`${peek.source}: ${peek.title}, ${peek.detail}. Open dashboard`}
      className={cx("flex h-full w-full items-center gap-3 rounded-full pr-4 pl-2", focusRing)}
    >
      {peek.artworkKey !== undefined ? (
        <MediaArtwork
          artworkKey={peek.artworkKey}
          label=""
          iconSize={14}
          rounded="rounded-[9px]"
          className="size-8 shadow-none"
        />
      ) : (
        <PeekGlyph peek={peek} />
      )}
      <span role="status" className="flex min-w-0 flex-1 flex-col items-start text-left">
        <span className="text-caption font-medium text-fg-muted">{peek.source}</span>
        <span className="w-full truncate text-body font-semibold tracking-tight text-fg">
          {peek.title}
          <span className="font-regular text-fg-muted"> · {peek.detail}</span>
        </span>
      </span>
      {peek.artworkKey !== undefined ? (
        <ActivityBars active className="mr-0.5" />
      ) : (
        <StatusIndicator tone={peek.tone} />
      )}
    </button>
  );
}
