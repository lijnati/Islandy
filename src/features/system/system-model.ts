/**
 * Pure helpers for system state: revision handling, display text, alert →
 * peek mapping and error wording. No IPC here (see system-store.ts).
 */
import type { NativeError } from "../../services/native";
import type {
  BatteryInfo,
  NetworkState,
  PowerState,
  SystemAlert,
  SystemDomain,
  SystemSnapshot,
  SystemUpdate,
} from "../../services/types";
import type { PeekContent } from "../island/island-store";

export const SYSTEM_DOMAINS: readonly SystemDomain[] = [
  "audio",
  "power",
  "network",
  "radios",
  "metrics",
  "display",
];

/** Every domain "loading": the state before the first snapshot. */
export const LOADING_SNAPSHOT: SystemSnapshot = {
  revision: 0,
  audio: { status: "loading", output: null, devices: [] },
  power: { status: "loading", source: "unknown", battery: { presence: "unknown" }, saver: null },
  network: { status: "loading", connectivity: "none", connection: null },
  radios: { status: "loading", wifi: null, bluetooth: null },
  metrics: {
    status: "loading",
    cpuPercent: null,
    memory: null,
    sampledAtMs: 0,
    intervalMs: null,
  },
  display: { status: "loading", displays: [], unsupportedCount: 0 },
};

export type DomainRevisions = Readonly<Record<SystemDomain, number>>;

export function revisionsOf(snapshot: SystemSnapshot): DomainRevisions {
  const r = snapshot.revision;
  return { audio: r, power: r, network: r, radios: r, metrics: r, display: r };
}

/**
 * Applies one domain update if it is newer than what we hold for that
 * domain. Returns `null` when stale (or an alert, which carries no state).
 */
export function applyUpdate(
  snapshot: SystemSnapshot,
  revisions: DomainRevisions,
  update: SystemUpdate,
): { snapshot: SystemSnapshot; revisions: DomainRevisions } | null {
  if (update.domain === "alert") return null;
  if (update.revision <= revisions[update.domain]) return null;
  return {
    snapshot: {
      ...snapshot,
      revision: Math.max(snapshot.revision, update.revision),
      [update.domain]: update.state,
    },
    revisions: { ...revisions, [update.domain]: update.revision },
  };
}

/* ------------------------------------------------------------------------ */
/* Display text                                                              */
/* ------------------------------------------------------------------------ */

export function formatBytes(bytes: number): string {
  const gib = bytes / 1024 ** 3;
  if (gib >= 10) return `${Math.round(gib)} GB`;
  if (gib >= 1) return `${gib.toFixed(1)} GB`;
  return `${Math.round(bytes / 1024 ** 2)} MB`;
}

export function batteryPercent(battery: BatteryInfo): number | null {
  return battery.presence === "present" ? battery.percent : null;
}

/** A short, honest description of the power state. */
export function powerDescription(power: PowerState): string {
  const { battery } = power;
  if (battery.presence === "absent") return "Runs on AC power";
  if (battery.presence === "unknown") return "Battery status unknown";
  switch (battery.state) {
    case "charging":
      return "Charging";
    case "discharging":
      return power.saver ? "On battery · saver on" : "On battery";
    case "notCharging":
      return battery.percent === 100 ? "Fully charged" : "Plugged in, not charging";
    case "unknown":
      return "Battery";
  }
}

export interface NetworkSummary {
  title: string;
  detail: string;
  /** Any network connection at all (not necessarily internet). */
  connected: boolean;
  online: boolean;
}

const KIND_LABEL = {
  wifi: "Wi-Fi",
  ethernet: "Ethernet",
  cellular: "Cellular",
  other: "Network",
} as const;

export function networkSummary(network: NetworkState): NetworkSummary {
  const via = network.connection
    ? [KIND_LABEL[network.connection.kind], network.connection.name].filter(Boolean).join(" · ")
    : "";
  switch (network.connectivity) {
    case "internet":
      return { title: "Connected", detail: via, connected: true, online: true };
    case "constrained":
      return {
        title: "Limited access",
        detail: via ? `${via} · sign-in may be required` : "Sign-in may be required",
        connected: true,
        online: true,
      };
    case "local":
      return {
        title: "No internet",
        detail: via || "Connected, without internet",
        connected: true,
        online: false,
      };
    case "none":
      return {
        title: "Offline",
        detail: "Not connected to a network",
        connected: false,
        online: false,
      };
  }
}

/* ------------------------------------------------------------------------ */
/* Alerts → peeks                                                            */
/* ------------------------------------------------------------------------ */

/** A stable key for de-duplicating repeated alerts. */
export function alertKey(alert: SystemAlert): string {
  switch (alert.type) {
    case "audioOutputChanged":
      return `${alert.type}:${alert.name}`;
    case "networkRestored":
      return `${alert.type}:${alert.kind}:${alert.name ?? ""}`;
    default:
      return alert.type;
  }
}

export function alertToPeek(alert: SystemAlert): PeekContent {
  switch (alert.type) {
    case "batteryLow":
      return {
        source: "Battery",
        title: `Battery low — ${alert.percent}%`,
        detail: "Plug in soon",
        tone: "attention",
        icon: "batteryLow",
      };
    case "batteryCritical":
      return {
        source: "Battery",
        title: `Battery critically low — ${alert.percent}%`,
        detail: "Plug in now",
        tone: "attention",
        icon: "batteryLow",
      };
    case "powerConnected":
      return {
        source: "Power",
        title: "Power connected",
        detail: alert.percent === null ? "Plugged in" : `${alert.percent}%`,
        tone: "success",
        icon: "plug",
      };
    case "powerDisconnected":
      return {
        source: "Power",
        title: "On battery",
        detail: alert.percent === null ? "Unplugged" : `${alert.percent}% remaining`,
        tone: "neutral",
        icon: "battery",
      };
    case "networkLost":
      return {
        source: "Network",
        title: "Network disconnected",
        detail: "No internet connection",
        tone: "attention",
        icon: "offline",
      };
    case "networkRestored":
      return {
        source: "Network",
        title:
          alert.kind === "wifi"
            ? "Connected to Wi-Fi"
            : alert.kind === "ethernet"
              ? "Connected to Ethernet"
              : "Back online",
        detail: alert.name ?? "Internet restored",
        tone: "success",
        icon: alert.kind === "wifi" ? "wifi" : "network",
      };
    case "audioOutputChanged":
      return {
        source: "Sound",
        title: "Audio output changed",
        detail: alert.name,
        tone: "neutral",
        icon: "speaker",
      };
  }
}

/* ------------------------------------------------------------------------ */
/* Errors                                                                    */
/* ------------------------------------------------------------------------ */

/** Neutral wording for a failed system command (never raw native text). */
export function systemErrorMessage(kind: NativeError["kind"]): string {
  switch (kind) {
    case "denied":
      return "Windows didn't allow that change";
    case "notFound":
      return "That device is no longer available";
    case "unsupported":
      return "Not supported on this device";
    case "timeout":
      return "The device didn't respond";
    case "unavailable":
      return "Not available right now";
    default:
      return "Couldn't apply that change";
  }
}
