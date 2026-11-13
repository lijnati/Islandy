import { AnimatePresence, motion as m } from "motion/react";
import { useEffect, useRef, type PointerEvent } from "react";

import { motion } from "../../design/tokens";
import { dispatchIsland, useIsland, type WidgetId } from "../../features/island/island-store";
import { openSettingsWindow, isNativeAvailable } from "../../services/native";
import type { DashboardWidgetId } from "../../services/types";
import { useSettings } from "../../features/settings/settings-store";
import { fitWidgets, WIDGETS } from "../../features/widgets/registry";
import { AudioPanel } from "../system/AudioPanel";
import { BluetoothPanel } from "../system/BluetoothPanel";
import { DisplayPanel } from "../system/DisplayPanel";
import { NetworkPanel } from "../system/NetworkPanel";
import { SystemPanel } from "../system/SystemPanel";
import { ActivityCenter } from "../activity/ActivityCenter";
import { ActivityWidget } from "../activity/ActivityWidget";
import { ClaudePanel } from "../activity/ClaudePanel";
import { IntegrationsPanel } from "../activity/IntegrationsPanel";
import { AudioWidget } from "../widgets/AudioWidget";
import { CalendarDetail, CalendarWidget } from "../widgets/CalendarWidget";
import { MediaDetail, MediaWidget } from "../widgets/MediaWidget";
import { SystemWidget } from "../widgets/SystemWidget";
import { WeatherWidget } from "../widgets/WeatherWidget";
import { IslandToolbar } from "./IslandToolbar";

const DETAILS: Record<WidgetId, () => React.JSX.Element> = {
  media: MediaDetail,
  calendar: CalendarDetail,
  activity: ActivityCenter,
  claude: ClaudePanel,
  integrations: IntegrationsPanel,
  // System panels, opened from the quick controls.
  audio: AudioPanel,
  system: SystemPanel,
  network: NetworkPanel,
  bluetooth: BluetoothPanel,
  display: DisplayPanel,
};

/** Dashboard widget components, by registry id. */
const WIDGET_COMPONENTS: Record<
  DashboardWidgetId,
  (props: { className?: string }) => React.JSX.Element
> = {
  media: MediaWidget,
  calendar: CalendarWidget,
  activity: ActivityWidget,
  system: SystemWidget,
  audio: AudioWidget,
  weather: WeatherWidget,
};

function NoWidgets() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 rounded-tile border border-dashed border-hairline text-center">
      <p className="text-body text-fg-muted">No widgets are turned on.</p>
      {isNativeAvailable() && (
        <button
          type="button"
          onClick={() => void openSettingsWindow("widgets")}
          className="rounded-full border border-hairline px-3 py-1 text-caption font-medium text-fg-muted transition-colors duration-fast hover:bg-surface-hover hover:text-fg"
        >
          Choose widgets
        </button>
      )}
    </div>
  );
}

/** Clicking empty dashboard space (not a control) pins it open. */
function pinOnBackgroundPress(event: PointerEvent<HTMLElement>) {
  const target = event.target as HTMLElement;
  if (!target.closest("button, a, [role=button]")) dispatchIsland({ type: "PRESS" });
}

const enter = (i: number, reduce: boolean) =>
  reduce
    ? { initial: false as const }
    : {
        initial: { opacity: 0, y: 6, scale: 0.985 },
        animate: { opacity: 1, y: 0, scale: 1 },
        transition: { ...motion.contentIn, delay: 0.06 + i * motion.stagger },
      };

/**
 * The dashboard: toolbar + widgets, or a widget's detail view (`focused`).
 * `width` is the fitted dashboard width; widgets drop out on narrow monitors.
 */
export function ExpandedIsland({ width, reduceMotion }: { width: number; reduceMotion: boolean }) {
  const activeWidget = useIsland((s) => s.activeWidget);
  const pinned = useIsland((s) => s.pinned);
  const ref = useRef<HTMLElement>(null);

  // When the island has taken keyboard focus (click, shortcut), move focus
  // into the dashboard so Tab reaches its controls and Escape works.
  useEffect(() => {
    if (pinned && document.hasFocus() && !ref.current?.contains(document.activeElement)) {
      ref.current?.focus({ preventScroll: true });
    }
  }, [pinned]);

  const items = useSettings((s) => s.widgets.items);
  const shown = fitWidgets(items, width);
  const Detail = activeWidget ? DETAILS[activeWidget] : null;

  return (
    <section
      ref={ref}
      tabIndex={-1}
      aria-label="Islandy dashboard"
      onPointerDown={pinned ? undefined : pinOnBackgroundPress}
      className="flex h-full flex-col gap-3 p-4 outline-hidden"
    >
      <m.div {...enter(0, reduceMotion)}>
        <IslandToolbar />
      </m.div>
      <div className="relative min-h-0 flex-1">
        <AnimatePresence initial={false} mode="popLayout">
          {Detail ? (
            <m.div
              key={`detail-${activeWidget ?? ""}`}
              className="absolute inset-0"
              initial={reduceMotion ? false : { opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, transition: motion.contentOut }}
              transition={motion.contentIn}
            >
              <Detail />
            </m.div>
          ) : (
            <m.div
              key="widgets"
              className="absolute inset-0 flex gap-3"
              initial={reduceMotion ? false : { opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0, transition: motion.contentOut }}
              transition={motion.contentIn}
            >
              {shown.length === 0 ? (
                <m.div className="flex-1" {...enter(1, reduceMotion)}>
                  <NoWidgets />
                </m.div>
              ) : (
                shown.map((id, i) => {
                  const def = WIDGETS[id];
                  const Widget = WIDGET_COMPONENTS[id];
                  return (
                    <m.div
                      key={id}
                      className={def.layout.kind === "fixed" ? "flex shrink-0" : "flex min-w-0"}
                      style={
                        def.layout.kind === "fixed"
                          ? { width: def.minWidth }
                          : { flex: `${def.layout.grow} 1 0`, minWidth: def.minWidth }
                      }
                      {...enter(i + 1, reduceMotion)}
                    >
                      <Widget className="flex-1" />
                    </m.div>
                  );
                })
              )}
            </m.div>
          )}
        </AnimatePresence>
      </div>
    </section>
  );
}
