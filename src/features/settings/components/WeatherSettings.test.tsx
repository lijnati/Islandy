import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_SETTINGS } from "../defaults";
import { settingsStore } from "../settings-store";

const locate = vi.hoisted(() => vi.fn());

vi.mock("../settings-service", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../settings-service")>()),
  isNativeAvailable: () => true,
  weatherLocate: locate,
}));

import { WeatherSettings } from "./WeatherSettings";

beforeEach(() => {
  locate.mockReset();
});

afterEach(() => {
  cleanup();
  act(() => {
    settingsStore.setState({
      state: { settings: DEFAULT_SETTINGS, loadStatus: { kind: "fresh" }, saveError: false },
      error: null,
    });
  });
});

describe("WeatherSettings › Use my location", () => {
  it("asks only when clicked, then saves the place like a search result", async () => {
    locate.mockResolvedValue({
      name: "Current location",
      detail: "9.03, 38.77",
      latitude: 9.03,
      longitude: 38.77,
    });
    render(<WeatherSettings />);
    expect(locate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Use my location" }));
    expect(locate).toHaveBeenCalledTimes(1);
    await waitFor(() => {
      const { weather, widgets } = settingsStore.getState().state.settings;
      expect(weather.location).toMatchObject({
        name: "Current location",
        latitude: 9.03,
        longitude: 38.77,
      });
      expect(widgets.items.find((w) => w.id === "weather")?.enabled).toBe(true);
    });
  });

  it("shows the reason when location is denied and saves nothing", async () => {
    locate.mockRejectedValue(new Error("denied: Turn on Location in Windows Settings"));
    render(<WeatherSettings />);
    fireEvent.click(screen.getByRole("button", { name: "Use my location" }));
    expect(await screen.findByText("Turn on Location in Windows Settings")).toBeTruthy();
    expect(settingsStore.getState().state.settings.weather.location).toBeNull();
  });
});
