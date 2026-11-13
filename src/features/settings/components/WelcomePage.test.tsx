import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { DEFAULT_SETTINGS } from "../defaults";
import { settingsStore } from "../settings-store";
import { SettingsApp } from "./SettingsApp";
import { WelcomePage } from "./pages/WelcomePage";

afterEach(() => {
  cleanup();
  act(() => {
    settingsStore.setState({
      state: { settings: DEFAULT_SETTINGS, loadStatus: { kind: "fresh" }, saveError: false },
      error: null,
    });
  });
  window.location.hash = "";
});

describe("WelcomePage", () => {
  it("has a heading that takes focus and shows the current shortcut", () => {
    act(() => {
      settingsStore.setState({
        state: {
          settings: {
            ...DEFAULT_SETTINGS,
            general: { ...DEFAULT_SETTINGS.general, toggleShortcut: "Ctrl+Shift+K" },
          },
          loadStatus: { kind: "loaded" },
          saveError: false,
        },
      });
    });
    render(<WelcomePage onNavigate={() => undefined} />);
    const h1 = screen.getByRole("heading", { level: 1, name: "Welcome to Islandy" });
    expect(document.activeElement).toBe(h1);
    expect(screen.getByText("Ctrl + Shift + K")).toBeTruthy();
  });

  it("points to the right pages", () => {
    const visited: string[] = [];
    render(
      <WelcomePage
        onNavigate={(page) => {
          visited.push(page);
        }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Open Integrations" }));
    fireEvent.click(screen.getByRole("button", { name: "Open Widgets" }));
    fireEvent.click(screen.getByRole("button", { name: "Change shortcut" }));
    expect(visited).toEqual(["integrations", "widgets", "general"]);
    expect(settingsStore.getState().state.settings.onboarding.welcomed).toBe(false);
  });

  it("Done marks the welcome as seen and leaves the page", async () => {
    window.location.hash = "#welcome";
    render(<SettingsApp />);
    fireEvent.click(screen.getByRole("button", { name: "Open Widgets" }));
    expect(screen.getByRole("heading", { level: 1, name: "Widgets" })).toBeTruthy();
    // Back to the welcome page and finish it.
    act(() => {
      window.location.hash = "#welcome";
    });
    fireEvent.click(await screen.findByRole("button", { name: "Done" }));
    expect(settingsStore.getState().state.settings.onboarding.welcomed).toBe(true);
    expect(screen.getByRole("heading", { level: 1, name: "General" })).toBeTruthy();
  });

  it("can be reopened from About", async () => {
    window.location.hash = "#about";
    render(<SettingsApp />);
    fireEvent.click(screen.getByRole("button", { name: "Show welcome" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "Welcome to Islandy" }),
    ).toBeTruthy();
  });
});
