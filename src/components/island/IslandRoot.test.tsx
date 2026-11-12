import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MotionGlobalConfig } from "motion/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { dispatchIsland, islandStore } from "../../features/island/island-store";
import { initialMediaState, mediaStore } from "../../features/media/media-store";
import { session, snapshot } from "../../features/media/media-test-utils";
import { initialSystemState, systemStore } from "../../features/system/system-store";
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
  mediaStore.setState(initialMediaState);
  systemStore.setState(initialSystemState);
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
    expect(screen.getAllByText("Demo")).toHaveLength(1); // activity only; media is real

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

  it("quick controls open system panels in place and close them again", async () => {
    render(<IslandRoot />);
    fireEvent.click(screen.getByRole("button", { name: /Open dashboard/ }));
    await settle();
    const sound = screen.getByRole("button", { name: "Sound" });
    fireEvent.click(sound);
    await settle();
    expect(islandStore.getState()).toMatchObject({ mode: "focused", activeWidget: "audio" });
    expect(screen.getByRole("heading", { name: "Sound" })).toBeDefined();
    // Outside the desktop app the panel says so instead of showing fake values.
    expect(screen.getAllByText(/Available in the Islandy desktop app on Windows/).length).toBe(1);
    expect(screen.getByRole("button", { name: "Sound" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Sound" }));
    await settle();
    expect(islandStore.getState().mode).toBe("expanded");

    // Windows Settings can only be opened on Windows; the button says so.
    const settings = screen.getByRole("button", {
      name: "Windows Settings, available on Windows",
    });
    expect(settings.getAttribute("aria-disabled")).toBe("true");
  });

  it("media widget says media is unavailable in a plain browser", async () => {
    render(<IslandRoot />);
    fireEvent.click(screen.getByRole("button", { name: /Open dashboard/ }));
    await settle();
    expect(screen.getByText("Media unavailable")).toBeDefined();
  });

  it("shows real session data in the pill and the widget, and an empty state", async () => {
    mediaStore.setState({ connection: "connected" });
    mediaStore.getState().applySnapshot(
      snapshot({
        revision: 1,
        sessions: [
          session({ title: "Ünïcode Song", artist: "Band" }),
          session({
            id: "s2",
            sourceAppName: "Browser",
            title: null,
            artist: null,
            playbackStatus: "paused",
          }),
        ],
      }),
    );
    render(<IslandRoot />);
    expect(
      screen.getByRole("button", { name: /playing Ünïcode Song by Band\. Open dashboard/ }),
    ).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Open dashboard/ }));
    await settle();
    expect(screen.getByText("Ünïcode Song")).toBeDefined();
    expect(screen.getByRole("button", { name: "Pause" })).toBeDefined();
    expect(screen.getByRole("slider", { name: "Seek" })).toBeDefined();

    // Session switcher lists both players.
    fireEvent.click(screen.getByRole("button", { name: /Player: Player\. 2 players available/ }));
    expect(screen.getAllByRole("menuitemradio")).toHaveLength(3);
    expect(screen.getByText("Unknown track")).toBeDefined();

    act(() => {
      mediaStore.getState().applySnapshot(snapshot({ revision: 2, sessions: [] }));
    });
    expect(screen.getByText("Nothing playing")).toBeDefined();
  });

  it("unsupported controls are disabled and say so", async () => {
    mediaStore.setState({ connection: "connected" });
    const controls = {
      play: true,
      pause: true,
      togglePlayPause: true,
      next: false,
      previous: false,
      seek: false,
    };
    mediaStore
      .getState()
      .applySnapshot(snapshot({ revision: 1, sessions: [session({ controls })] }));
    render(<IslandRoot />);
    fireEvent.click(screen.getByRole("button", { name: /Open dashboard/ }));
    await settle();
    const next = screen.getByRole("button", { name: "Next track (not supported by Player)" });
    expect(next.getAttribute("aria-disabled")).toBe("true");
    expect(screen.queryByRole("slider")).toBeNull();
    expect(screen.getByRole("progressbar", { name: "Playback position" })).toBeDefined();
  });
});
