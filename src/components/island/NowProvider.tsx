import type { ReactNode } from "react";

import { useClock } from "../../hooks/useClock";
import { NowContext } from "../../hooks/useNow";

/** Runs one minute-aligned clock for the whole island. */
export function NowProvider({ children }: { children: ReactNode }) {
  const now = useClock();
  return <NowContext value={now}>{children}</NowContext>;
}
