import { describe, expect, it } from "vitest";

import {
  describeWeather,
  formatTemperature,
  unitSymbol,
  updatedAgo,
  weekdayOf,
  windUnit,
} from "./weather-model";

describe("weather model", () => {
  it("maps WMO codes to words and day/night icons", () => {
    expect(describeWeather(0)).toEqual({ label: "Clear", icon: "clear" });
    expect(describeWeather(0, false).icon).toBe("clearNight");
    expect(describeWeather(2, false).icon).toBe("partlyNight");
    expect(describeWeather(45).label).toBe("Fog");
    expect(describeWeather(53).icon).toBe("drizzle");
    expect(describeWeather(65).label).toBe("Heavy rain");
    expect(describeWeather(67).label).toBe("Freezing rain");
    expect(describeWeather(75).label).toBe("Heavy snow");
    expect(describeWeather(81).icon).toBe("showers");
    expect(describeWeather(86).icon).toBe("snow");
    expect(describeWeather(99).icon).toBe("thunder");
    expect(describeWeather(42).icon).toBe("cloudy");
  });

  it("formats temperatures without a negative zero", () => {
    expect(formatTemperature(12.8)).toBe("13°");
    expect(formatTemperature(-0.4)).toBe("0°");
    expect(formatTemperature(-3.6)).toBe("-4°");
    expect(unitSymbol("fahrenheit")).toBe("°F");
    expect(windUnit("celsius")).toBe("km/h");
  });

  it("names the place's calendar day regardless of the viewer's time zone", () => {
    expect(weekdayOf("2026-11-13", "en-US")).toBe("Fri");
    expect(weekdayOf("2026-11-16", "en-US")).toBe("Mon");
    expect(weekdayOf("garbage", "en-US")).toBe("");
  });

  it("says how old the forecast is", () => {
    const now = 10_000_000;
    expect(updatedAgo(now - 20_000, now)).toBe("Updated just now");
    expect(updatedAgo(now - 12 * 60_000, now)).toBe("Updated 12 min ago");
    expect(updatedAgo(now - 130 * 60_000, now)).toBe("Updated 2 h ago");
  });
});
