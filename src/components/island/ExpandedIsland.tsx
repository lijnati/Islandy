import { AnimatePresence, motion as m } from "motion/react";
import { useEffect, useRef, type PointerEvent } from "react";

import { breakpoints, motion } from "../../design/tokens";
import { dispatchIsland, useIsland, type WidgetId } from "../../features/island/island-store";
import { AudioPanel } from "../system/AudioPanel";
import { BluetoothPanel } from "../system/BluetoothPanel";
import { DisplayPanel } from "../system/DisplayPanel";
import { NetworkPanel } from "../system/NetworkPanel";
import { SystemPanel } from "../system/SystemPanel";
import { ActivityDetail, ActivityWidget } from "../widgets/ActivityWidget";
import { CalendarDetail, CalendarWidget } from "../widgets/CalendarWidget";
import { MediaDetail, MediaWidget } from "../widgets/MediaWidget";
import { IslandToolbar } from "./IslandToolbar";

const DETAILS: Record<WidgetId, () => React.JSX.Element> = {
  media: MediaDetail,
  calendar: CalendarDetail,
  activity: ActivityDetail,
  // System panels, opened from the quick controls.
  audio: AudioPanel,
  system: SystemPanel,
  network: NetworkPanel,
  bluetooth: BluetoothPanel,
  display: DisplayPanel,
};

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

  const showActivity = width >= breakpoints.hideActivity;
  const showCalendar = width >= breakpoints.hideCalendar;
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
              <m.div className="flex min-w-0 flex-[1.6]" {...enter(1, reduceMotion)}>
                <MediaWidget className="flex-1" />
              </m.div>
              {showCalendar && (
                <m.div className="flex w-[204px] shrink-0" {...enter(2, reduceMotion)}>
                  <CalendarWidget className="flex-1" />
                </m.div>
              )}
              {showActivity && (
                <m.div className="flex min-w-0 flex-1" {...enter(3, reduceMotion)}>
                  <ActivityWidget className="flex-1" />
                </m.div>
              )}
            </m.div>
          )}
        </AnimatePresence>
      </div>
    </section>
  );
}
