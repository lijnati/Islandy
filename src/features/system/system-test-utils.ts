import type { AudioState, SystemSnapshot } from "../../services/types";
import { LOADING_SNAPSHOT } from "./system-model";

/** Fixtures shaped exactly like Rust's serialized system state. */
export function audio(overrides: Partial<AudioState> = {}): AudioState {
  return {
    status: "ready",
    output: { deviceId: "d1", name: "Speakers (Realtek Audio)", volume: 40, muted: false },
    devices: [
      { id: "d1", name: "Speakers (Realtek Audio)", isDefault: true },
      { id: "d2", name: "Headphones", isDefault: false },
    ],
    ...overrides,
  };
}

export function systemSnapshot(overrides: Partial<SystemSnapshot> = {}): SystemSnapshot {
  return {
    ...LOADING_SNAPSHOT,
    revision: 1,
    audio: audio(),
    power: {
      status: "ready",
      source: "battery",
      battery: { presence: "present", percent: 86, state: "discharging" },
      saver: false,
    },
    network: {
      status: "ready",
      connectivity: "internet",
      connection: { kind: "wifi", name: "Home" },
    },
    radios: {
      status: "ready",
      wifi: { state: "on", canToggle: true },
      bluetooth: { state: "off", canToggle: false },
    },
    metrics: {
      status: "ready",
      cpuPercent: null,
      memory: null,
      sampledAtMs: 0,
      intervalMs: null,
    },
    display: {
      status: "ready",
      displays: [{ id: "i0", name: "Built-in display", kind: "internal", brightness: 70 }],
      unsupportedCount: 1,
    },
    ...overrides,
  };
}
