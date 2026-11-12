import { ChevronLeft, ChevronUp, Moon, Pin, PinOff } from "lucide-react";

import { activityStore, useActivity } from "../../features/activity/activity-store";

import { dispatchIsland, useIsland, type WidgetId } from "../../features/island/island-store";
import { formatShortDate, formatTime } from "../../features/island/format";
import { useNativeStatus } from "../../features/island/native-status";
import { useNow } from "../../hooks/useNow";
import { IconButton } from "../ui/IconButton";
import { QuickControls } from "./QuickControls";

const WIDGET_TITLES: Record<WidgetId, string> = {
  media: "Now Playing",
  calendar: "Calendar",
  activity: "Activity",
  claude: "Claude Code",
  integrations: "Integrations",
  audio: "Sound",
  system: "System",
  network: "Network",
  bluetooth: "Bluetooth",
  display: "Display",
};

export function IslandToolbar() {
  const now = useNow();
  const pinned = useIsland((s) => s.pinned);
  const activeWidget = useIsland((s) => s.activeWidget);
  const shortcut = useNativeStatus((s) => s.connection?.shortcut);
  const quiet = useActivity((s) => s.snapshot?.settings.quiet ?? false);
  const quietReady = useActivity((s) => s.connection === "connected");
  const quietBusy = useActivity((s) => Boolean(s.pending.quiet));
  const shortcutHint = shortcut?.registered
    ? ` (${shortcut.accelerator.replace(/\+/g, " + ")})`
    : "";

  return (
    <div className="flex h-8 items-center justify-between gap-3 pl-1">
      {activeWidget ? (
        <div className="flex min-w-0 items-center gap-1.5">
          <IconButton
            icon={ChevronLeft}
            label="Back to dashboard"
            tooltip="Back (Esc)"
            tooltipAlign="start"
            onClick={() => {
              dispatchIsland({ type: "CLOSE_WIDGET" });
            }}
          />
          <h2 className="truncate text-title font-semibold tracking-tight text-fg">
            {WIDGET_TITLES[activeWidget]}
          </h2>
        </div>
      ) : (
        <div className="flex min-w-0 items-baseline gap-2 pl-2">
          <time
            dateTime={now.toISOString()}
            className="text-title font-semibold tracking-tight text-fg tabular-nums"
          >
            {formatTime(now)}
          </time>
          <span className="truncate text-body text-fg-muted">{formatShortDate(now)}</span>
        </div>
      )}

      <div className="flex shrink-0 items-center gap-2">
        <QuickControls />
        <span aria-hidden="true" className="h-4 w-px bg-hairline" />
        <IconButton
          icon={Moon}
          label={quiet ? "Turn off quiet mode" : "Turn on quiet mode"}
          tooltip={
            quietReady
              ? quiet
                ? "Quiet mode on: no automatic peeks"
                : "Quiet mode: no automatic peeks"
              : "Quiet mode (desktop app)"
          }
          pressed={quiet}
          busy={quietBusy}
          disabled={!quietReady}
          onClick={() => {
            activityStore.getState().setQuiet(!quiet);
          }}
        />
        <IconButton
          icon={pinned ? PinOff : Pin}
          label={pinned ? "Unpin dashboard" : "Pin dashboard open"}
          pressed={pinned}
          tooltipAlign="end"
          onClick={() => {
            dispatchIsland({ type: "TOGGLE_PIN" });
          }}
        />
        <IconButton
          icon={ChevronUp}
          label="Collapse"
          tooltip={`Collapse (Esc)${shortcutHint ? ` · toggle${shortcutHint}` : ""}`}
          tooltipAlign="end"
          onClick={() => {
            dispatchIsland({ type: "DISMISS" });
          }}
        />
      </div>
    </div>
  );
}
