import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { systemSnapshot } from "../../features/system/system-test-utils";
import { initialSystemState, systemStore } from "../../features/system/system-store";
import { revisionsOf } from "../../features/system/system-model";
import type { SystemSnapshot } from "../../services/types";
import { AudioPanel } from "./AudioPanel";
import { BluetoothPanel } from "./BluetoothPanel";
import { DisplayPanel } from "./DisplayPanel";
import { NetworkPanel } from "./NetworkPanel";
import { SystemPanel } from "./SystemPanel";

const native = vi.hoisted(() => ({
  setMasterVolume: vi.fn(() => Promise.resolve()),
  setAudioMuted: vi.fn(() => Promise.resolve()),
  setRadioState: vi.fn(() => Promise.resolve()),
  setDisplayBrightness: vi.fn(() => Promise.resolve()),
}));

vi.mock("../../services/native", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/native")>()),
  ...native,
}));

function load(snapshot: SystemSnapshot) {
  act(() => {
    systemStore.setState({
      ...initialSystemState,
      connection: "connected",
      snapshot,
      revisions: revisionsOf(snapshot),
    });
  });
}

afterEach(() => {
  cleanup();
  systemStore.setState(initialSystemState);
  vi.clearAllMocks();
});

describe("AudioPanel", () => {
  it("shows the real output and sends slider and mute changes without faking them", async () => {
    load(systemSnapshot());
    render(<AudioPanel />);
    expect(screen.getAllByText("Speakers (Realtek Audio)").length).toBeGreaterThan(0);
    const slider = screen.getByRole("slider", { name: "Volume" });
    expect(slider.getAttribute("aria-valuenow")).toBe("40");
    fireEvent.keyDown(slider, { key: "ArrowRight" });
    expect(native.setMasterVolume).toHaveBeenCalledWith(42);
    expect(slider.getAttribute("aria-valuenow")).toBe("42"); // the user's value, while settling

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Mute" }));
      await Promise.resolve();
    });
    expect(native.setAudioMuted).toHaveBeenCalledWith(true);
    // Still unmuted: only a native update may change that.
    expect(screen.getByRole("button", { name: "Mute" })).toBeDefined();
    expect(screen.getByText("(current output)")).toBeDefined();
  });

  it("explains a missing device and an unreadable audio service", () => {
    load(systemSnapshot({ audio: { status: "ready", output: null, devices: [] } }));
    const { unmount } = render(<AudioPanel />);
    expect(screen.getByText(/No audio output device is active/)).toBeDefined();
    unmount();
    load(systemSnapshot({ audio: { status: "unavailable", output: null, devices: [] } }));
    render(<AudioPanel />);
    expect(screen.getByText(/Couldn't read this from Windows/)).toBeDefined();
  });
});

describe("SystemPanel", () => {
  it("shows the battery on laptops and never a fake percentage on desktops", () => {
    load(systemSnapshot());
    const { unmount } = render(<SystemPanel />);
    expect(screen.getByRole("meter", { name: "Battery level" }).getAttribute("aria-valuenow")).toBe(
      "86",
    );
    expect(screen.getByText("On battery")).toBeDefined();
    unmount();

    const base = systemSnapshot();
    load(
      systemSnapshot({
        power: { ...base.power, source: "ac", battery: { presence: "absent" } },
        metrics: {
          status: "ready",
          cpuPercent: 18,
          memory: { totalBytes: 16 * 1024 ** 3, availableBytes: 8 * 1024 ** 3, usedPercent: 50 },
          sampledAtMs: 1,
          intervalMs: 2000,
        },
      }),
    );
    render(<SystemPanel />);
    expect(screen.getByText("No battery")).toBeDefined();
    expect(screen.queryByRole("meter", { name: "Battery level" })).toBeNull();
    expect(screen.queryByText("100")).toBeNull();
    expect(screen.getByRole("meter", { name: "CPU usage" }).getAttribute("aria-valuenow")).toBe(
      "18",
    );
    expect(screen.getByText("8.0 GB of 16 GB in use")).toBeDefined();
    expect(screen.getByText("Updates every 2 s")).toBeDefined();
  });

  it("waits for a second CPU sample instead of guessing", () => {
    load(systemSnapshot());
    render(<SystemPanel />);
    expect(screen.getAllByText("Measuring…").length).toBe(2);
  });
});

describe("NetworkPanel and BluetoothPanel", () => {
  it("reports connected-without-internet honestly and toggles a switchable radio", async () => {
    const base = systemSnapshot();
    load(systemSnapshot({ network: { ...base.network, connectivity: "local" } }));
    render(<NetworkPanel />);
    expect(screen.getByText("No internet")).toBeDefined();
    const wifi = screen.getByRole("switch", { name: "Wi-Fi radio" });
    expect(wifi.getAttribute("aria-checked")).toBe("true");
    await act(async () => {
      fireEvent.click(wifi);
      await Promise.resolve();
    });
    expect(native.setRadioState).toHaveBeenCalledWith("wifi", false);
    // Not flipped locally.
    expect(wifi.getAttribute("aria-checked")).toBe("true");
  });

  it("offers Settings instead of a switch when Windows doesn't allow toggling", () => {
    load(systemSnapshot());
    render(<BluetoothPanel />);
    expect(screen.queryByRole("switch")).toBeNull();
    expect(screen.getByText("Change it in Settings")).toBeDefined();
    expect(screen.getByRole("button", { name: /Bluetooth settings/ })).toBeDefined();
  });
});

describe("DisplayPanel", () => {
  it("adjusts supported displays and explains unsupported ones", () => {
    load(systemSnapshot());
    const { unmount } = render(<DisplayPanel />);
    const slider = screen.getByRole("slider", { name: "Brightness, Built-in display" });
    fireEvent.keyDown(slider, { key: "PageDown" });
    expect(native.setDisplayBrightness).toHaveBeenCalledWith("i0", 60);
    expect(screen.getByText("1 display doesn't support brightness control here.")).toBeDefined();
    unmount();

    load(systemSnapshot({ display: { status: "ready", displays: [], unsupportedCount: 1 } }));
    render(<DisplayPanel />);
    expect(screen.getByText("Not supported")).toBeDefined();
    expect(screen.queryByRole("slider")).toBeNull();
  });

  it("separates a temporary failure from unsupported hardware", () => {
    load(systemSnapshot({ display: { status: "unavailable", displays: [], unsupportedCount: 0 } }));
    render(<DisplayPanel />);
    expect(screen.getByText(/Couldn't reach the displays/)).toBeDefined();
    expect(screen.queryByText("Not supported")).toBeNull();
  });
});
