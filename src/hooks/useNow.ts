import { createContext, useContext } from "react";

export const NowContext = createContext<Date | null>(null);

/** The current time, updated on every wall-clock minute (see NowProvider). */
export function useNow(): Date {
  const now = useContext(NowContext);
  if (!now) throw new Error("useNow must be used inside <NowProvider>");
  return now;
}
