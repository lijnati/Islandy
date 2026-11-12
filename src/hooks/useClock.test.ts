import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { msUntilNextMinute, useClock } from "./useClock";

describe("msUntilNextMinute", () => {
  it("aligns to the next minute boundary", () => {
    expect(msUntilNextMinute(new Date(2026, 0, 1, 9, 41, 0, 0))).toBe(60_000);
    expect(msUntilNextMinute(new Date(2026, 0, 1, 9, 41, 59, 999))).toBe(1);
    expect(msUntilNextMinute(new Date(2026, 0, 1, 9, 41, 30, 250))).toBe(29_750);
  });
});

describe("useClock", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 0, 1, 9, 41, 50, 0));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("updates once per minute, on the boundary", () => {
    const { result } = renderHook(() => useClock());
    expect(result.current.getMinutes()).toBe(41);

    act(() => void vi.advanceTimersByTime(9_999));
    expect(result.current.getMinutes()).toBe(41);

    act(() => void vi.advanceTimersByTime(1));
    expect(result.current.getMinutes()).toBe(42);

    act(() => void vi.advanceTimersByTime(60_000));
    expect(result.current.getMinutes()).toBe(43);
  });

  it("schedules exactly one pending timer", () => {
    renderHook(() => useClock());
    expect(vi.getTimerCount()).toBe(1);
    act(() => void vi.advanceTimersByTime(10_000));
    expect(vi.getTimerCount()).toBe(1);
  });
});
