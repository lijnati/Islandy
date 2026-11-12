import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MotionGlobalConfig } from "motion/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { dispatchIsland, islandStore } from "../../features/island/island-store";
import { IslandRoot } from "./IslandRoot";

beforeAll(() => {
  MotionGlobalConfig.skipAnimations = true;
});

afterEach(() => {
  cleanup();
  act(() => {
    dispatchIsland({ type: "DISMISS" });
  });
  islandStore.dispose();
  vi.useRealTimers();
});

/** Lets the coordinator's (no-op, browser-mode) frame promise settle. */
async function settle() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("IslandRoot (browser mode, no native shell)", () => {
  it("renders the collapsed pill with the time and an accessible label", () => {
    render(<IslandRoot />);
    const pill = screen.getByRole("button", { name: /Islandy, .*Open dashboard/ });
    expect(pill.querySelector("time")).not.toBeNull();
  });

  it("click opens the pinned dashboard with all three widgets; Escape collapses", async () => {
    render(<IslandRoot />);
    fireEvent.click(screen.getByRole("button", { name: /Open dashboard/ }));
    await settle();
    expect(screen.getByRole("region", { name: "Islandy dashboard" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Open Now Playing" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Open Activity" })).toBeDefined();
    expect(screen.getByRole("table")).toBeDefined(); // calendar month grid
    expect(
      screen.getByRole("button", { name: "Unpin dashboard" }).getAttribute("aria-pressed"),
    ).toBe("true");
    expect(screen.getAllByText("Demo").length).toBeGreaterThanOrEqual(2);

    fireEvent.keyDown(window, { key: "Escape" });
    await settle();
    expect(islandStore.getState().mode).toBe("collapsed");
  });

  it("hover expands after the intent delay", () => {
    vi.useFakeTimers();
    const { container } = render(<IslandRoot />);
    const surface = container.querySelector("[data-mode]");
    if (!surface) throw new Error("surface missing");
    fireEvent.pointerEnter(surface);
    act(() => {
      vi.advanceTimersByTime(150);
    });
    expect(islandStore.getState().mode).toBe("expanded");
  });

  it("opens a widget detail view and returns with the back button", async () => {
    render(<IslandRoot />);
    fireEvent.click(screen.getByRole("button", { name: /Open dashboard/ }));
    await settle();
    fireEvent.click(
      screen.getByRole("button", {
        name: /^Open (January|February|March|April|May|June|July|August|September|October|November|December)$/,
      }),
    );
    await settle();
    expect(islandStore.getState()).toMatchObject({ mode: "focused", activeWidget: "calendar" });
    expect(screen.getByRole("button", { name: "Next month" })).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Back to dashboard" }));
    await settle();
    expect(islandStore.getState().mode).toBe("expanded");
  });

  it("quick controls are honest outside Windows: disabled, and say why", async () => {
    render(<IslandRoot />);
    fireEvent.click(screen.getByRole("button", { name: /Open dashboard/ }));
    await settle();
    const wifi = screen.getByRole("button", { name: "Wi-Fi settings, available on Windows" });
    expect(wifi.getAttribute("aria-disabled")).toBe("true");
    expect(
      screen
        .getByRole("button", { name: "Islandy settings (coming soon)" })
        .getAttribute("aria-disabled"),
    ).toBe("true");
  });

  it("demo play/pause only changes the demo state", async () => {
    render(<IslandRoot />);
    fireEvent.click(screen.getByRole("button", { name: /Open dashboard/ }));
    await settle();
    fireEvent.click(screen.getByRole("button", { name: "Play (demo)" }));
    expect(screen.getByRole("button", { name: "Pause (demo)" })).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Pause (demo)" }));
  });
});
