import type { Availability, SystemDomain, SystemSnapshot } from "../../services/types";
import { useSystem } from "./system-store";

export type DomainView<T> =
  { kind: "ready"; state: T } | { kind: "loading" | "browser" | "unsupported" | "unavailable" };

/** Where a domain stands, from the connection and its own status. */
export function useDomain<D extends SystemDomain>(domain: D): DomainView<SystemSnapshot[D]> {
  const connection = useSystem((s) => s.connection);
  const state = useSystem((s) => s.snapshot?.[domain] ?? null);
  if (connection === "unavailable") return { kind: "browser" };
  if (!state) return { kind: "loading" };
  const status: Availability = state.status;
  if (status === "ready") return { kind: "ready", state };
  return { kind: status };
}
