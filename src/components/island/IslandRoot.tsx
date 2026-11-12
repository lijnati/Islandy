import { AnimatePresence, motion as m, type Transition } from "motion/react";
import { motion, radius } from "../../design/tokens";
import { dispatchIsland, useIsland, type IslandMode } from "../../features/island/island-store";
import { useIslandFrame } from "../../features/island/useIslandFrame";
import { useNativeBridge } from "../../features/island/useNativeBridge";
import { usePrefersReducedMotion } from "../../hooks/usePrefersReducedMotion";
import { NowProvider } from "./NowProvider";
import { cx } from "../ui/cx";
import { CollapsedIsland } from "./CollapsedIsland";
import { ExpandedIsland } from "./ExpandedIsland";
import { PeekIsland } from "./PeekIsland";

const isDashboard = (mode: IslandMode) => mode === "expanded" || mode === "focused";

function surfaceTransition(from: IslandMode, to: IslandMode): Transition {
  if (isDashboard(to)) return motion.expand;
  if (isDashboard(from)) return motion.collapse;
  return motion.peek;
}

/**
 * The island: one animated surface centered at the top of the window.
 *
 * The surface animates its own width/height/radius; each view inside is laid
 * out at its *final* size and clipped by the surface, so nothing reflows
 * during the animation. `useIslandFrame` keeps the native window's frame
 * ahead of growth and behind shrinkage (see window-coordinator.ts).
 */
export function IslandRoot() {
  useNativeBridge();
  const { mode, from, size, onAnimationComplete } = useIslandFrame();
  const hovering = useIsland((s) => s.hovering);
  const peek = useIsland((s) => s.peek);
  const peekSeq = useIsland((s) => s.peekSeq);
  const reduce = usePrefersReducedMotion();

  const transition = reduce ? { duration: 0 } : surfaceTransition(from, mode);

  const dashboard = isDashboard(mode);
  const borderRadius = dashboard ? radius.panel : radius.pill(size.height);
  const viewKey = dashboard ? "dashboard" : mode === "peek" ? `peek-${peekSeq}` : "collapsed";

  return (
    <NowProvider>
      <div className="flex h-full w-full justify-center">
        <m.div
          data-mode={mode}
          initial={false}
          animate={{ width: size.width, height: size.height, borderRadius }}
          transition={transition}
          onAnimationComplete={onAnimationComplete}
          onPointerEnter={() => {
            dispatchIsland({ type: "POINTER_ENTER" });
          }}
          onPointerLeave={() => {
            dispatchIsland({ type: "POINTER_LEAVE" });
          }}
          className={cx(
            "relative shrink-0 overflow-hidden border bg-surface shadow-surface transition-colors duration-fast",
            hovering && !dashboard ? "border-hairline-strong bg-[#0e0e0e]" : "border-hairline",
          )}
          style={{ width: size.width, height: size.height, borderRadius }}
        >
          <AnimatePresence initial={false}>
            <m.div
              key={viewKey}
              className="absolute top-0 left-1/2"
              style={{ width: size.width, height: size.height, marginLeft: -size.width / 2 }}
              initial={reduce ? false : { opacity: 0 }}
              animate={{
                opacity: 1,
                transition: { ...motion.contentIn, delay: dashboard ? 0.04 : 0.08 },
              }}
              exit={{ opacity: 0, transition: motion.contentOut }}
            >
              {dashboard ? (
                <ExpandedIsland width={size.width} reduceMotion={reduce} />
              ) : mode === "peek" && peek ? (
                <PeekIsland peek={peek} />
              ) : (
                <CollapsedIsland />
              )}
            </m.div>
          </AnimatePresence>
        </m.div>
      </div>
    </NowProvider>
  );
}
