import { useMemo } from "react";

import { ambientOf, type Ambient } from "./activity-model";
import { useActivity } from "./activity-store";

/** The collapsed pill's ambient activity, or `null` (plain pill). */
export function useAmbient(): Ambient | null {
  const snapshot = useActivity((s) => s.snapshot);
  return useMemo(() => ambientOf(snapshot), [snapshot]);
}

export function useQuiet(): boolean {
  return useActivity((s) => s.snapshot?.settings.quiet ?? false);
}
