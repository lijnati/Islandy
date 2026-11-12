import { describe, expect, it } from "vitest";

import { systemFocusFor } from "../island/useNativeBridge";
import {
  alertKey,
  alertToPeek,
  applyUpdate,
  formatBytes,
  networkSummary,
  powerDescription,
  revisionsOf,
  systemErrorMessage,
} from "./system-model";
import { systemSnapshot } from "./system-test-utils";

describe("per-domain revisions", () => {
  it("applies newer updates to their own domain only", () => {
    const s = systemSnapshot({ revision: 5 });
    const r = revisionsOf(s);
    const muted = {
      ...s.audio,
      output: s.audio.output && { ...s.audio.output, muted: true },
    };
    const next = applyUpdate(s, r, { domain: "audio", revision: 6, state: muted });
    expect(next?.snapshot.audio.output?.muted).toBe(true);
    expect(next?.revisions).toMatchObject({ audio: 6, power: 5 });
    expect(next?.snapshot.power).toBe(s.power);
    if (!next) throw new Error("expected an update");
    // Stale for audio, but a *metrics* update at 6 is still new for metrics.
    expect(
      applyUpdate(next.snapshot, next.revisions, {
        domain: "audio",
        revision: 6,
        state: s.audio,
      }),
    ).toBeNull();
    expect(
      applyUpdate(next.snapshot, next.revisions, {
        domain: "metrics",
        revision: 6,
        state: s.metrics,
      }),
    ).not.toBeNull();
    expect(
      applyUpdate(s, r, { domain: "alert", revision: 9, alert: { type: "networkLost" } }),
    ).toBeNull();
  });
});

describe("power text", () => {
  it("never shows a desktop as a charged battery", () => {
    const desktop = {
      ...systemSnapshot().power,
      source: "ac" as const,
      battery: { presence: "absent" as const },
    };
    expect(powerDescription(desktop)).toBe("Runs on AC power");
    expect(
      powerDescription({
        ...desktop,
        battery: { presence: "present", percent: 100, state: "notCharging" },
      }),
    ).toBe("Fully charged");
    expect(
      powerDescription({
        ...desktop,
        battery: { presence: "present", percent: 80, state: "notCharging" },
      }),
    ).toBe("Plugged in, not charging");
    expect(powerDescription({ ...desktop, battery: { presence: "unknown" } })).toBe(
      "Battery status unknown",
    );
  });
});

describe("network text", () => {
  it("distinguishes a connection from internet access", () => {
    const base = systemSnapshot().network;
    expect(networkSummary(base)).toMatchObject({
      title: "Connected",
      detail: "Wi-Fi · Home",
      online: true,
    });
    expect(networkSummary({ ...base, connectivity: "local" })).toMatchObject({
      title: "No internet",
      connected: true,
      online: false,
    });
    expect(networkSummary({ ...base, connectivity: "constrained" }).title).toBe("Limited access");
    expect(networkSummary({ ...base, connectivity: "none", connection: null })).toMatchObject({
      title: "Offline",
      connected: false,
    });
  });
});

describe("alerts", () => {
  it("map to concise peeks", () => {
    expect(alertToPeek({ type: "batteryLow", percent: 15 })).toMatchObject({
      title: "Battery low — 15%",
      tone: "attention",
      icon: "batteryLow",
    });
    expect(alertToPeek({ type: "networkRestored", kind: "wifi", name: "Home" })).toMatchObject({
      title: "Connected to Wi-Fi",
      detail: "Home",
    });
    expect(alertToPeek({ type: "audioOutputChanged", name: "Headphones" })).toMatchObject({
      title: "Audio output changed",
      detail: "Headphones",
    });
    expect(alertToPeek({ type: "powerConnected", percent: null }).detail).toBe("Plugged in");
    expect(alertKey({ type: "audioOutputChanged", name: "A" })).not.toBe(
      alertKey({ type: "audioOutputChanged", name: "B" }),
    );
  });
});

describe("formatting and errors", () => {
  it("formats memory sizes", () => {
    expect(formatBytes(16 * 1024 ** 3)).toBe("16 GB");
    expect(formatBytes(6.25 * 1024 ** 3)).toBe("6.3 GB");
    expect(formatBytes(512 * 1024 ** 2)).toBe("512 MB");
  });

  it("uses neutral wording and never native text", () => {
    expect(systemErrorMessage("denied")).toBe("Windows didn't allow that change");
    expect(systemErrorMessage("platform")).toBe("Couldn't apply that change");
    expect(systemErrorMessage("unknown")).toBe("Couldn't apply that change");
  });
});

describe("sampling focus", () => {
  it("is on only while the system or display panel is open", () => {
    expect(systemFocusFor({ mode: "focused", activeWidget: "system" })).toBe("overview");
    expect(systemFocusFor({ mode: "focused", activeWidget: "display" })).toBe("display");
    expect(systemFocusFor({ mode: "focused", activeWidget: "audio" })).toBe("none");
    expect(systemFocusFor({ mode: "expanded", activeWidget: null })).toBe("none");
    expect(systemFocusFor({ mode: "collapsed", activeWidget: null })).toBe("none");
  });
});
