import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { DEFAULT_SETTINGS } from "../defaults";
import { settingsStore } from "../settings-store";
import { SettingsApp } from "./SettingsApp";

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

const open = (name: string) => {
  fireEvent.click(
    within(screen.getByRole("navigation", { name: "Settings" })).getByRole("button", { name }),
  );
};

describe("SettingsApp (browser mode)", () => {
  it("navigates between all pages", () => {
    render(<SettingsApp />);
    for (const name of [
      "General",
      "Appearance",
      "Island",
      "Widgets",
      "Integrations",
      "Notifications",
      "Privacy",
      "About",
    ]) {
      open(name);
      expect(screen.getByRole("heading", { level: 1, name })).toBeTruthy();
      expect(
        within(screen.getByRole("navigation", { name: "Settings" }))
          .getByRole("button", { name })
          .getAttribute("aria-current"),
      ).toBe("page");
    }
  });

  it("opens the page named in the URL", () => {
    window.location.hash = "#about";
    render(<SettingsApp />);
    expect(screen.getByRole("heading", { level: 1, name: "About" })).toBeTruthy();
    expect(screen.getByText(/not affiliated with or endorsed by Apple/)).toBeTruthy();
  });

  it("switches the theme and accent immediately", () => {
    render(<SettingsApp />);
    open("Appearance");
    fireEvent.click(screen.getByRole("radio", { name: /Graphite/ }));
    fireEvent.click(screen.getByRole("radio", { name: "Violet" }));
    expect(settingsStore.getState().state.settings.appearance).toMatchObject({
      theme: "graphite",
      accent: "violet",
    });
    expect(document.documentElement.dataset.theme).toBe("graphite");
    expect(document.documentElement.dataset.accent).toBe("violet");
  });

  it("reorders, hides and restores widgets", () => {
    render(<SettingsApp />);
    open("Widgets");
    fireEvent.click(screen.getByRole("button", { name: "Move Activity up" }));
    fireEvent.click(screen.getByRole("switch", { name: "Show Sound" }));
    let items = settingsStore.getState().state.settings.widgets.items;
    expect(items.map((w) => w.id)).toEqual([
      "media",
      "activity",
      "calendar",
      "system",
      "audio",
      "weather",
    ]);
    expect(items.find((w) => w.id === "audio")?.enabled).toBe(true);
    // Four widgets don't fit: the last enabled one is flagged.
    expect(screen.getByText("Doesn't fit")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Restore default layout" }));
    items = settingsStore.getState().state.settings.widgets.items;
    expect(items).toEqual(DEFAULT_SETTINGS.widgets.items);
  });

  it("never lets both pointer openers be turned off", () => {
    render(<SettingsApp />);
    open("Island");
    fireEvent.click(screen.getByRole("switch", { name: "Open on hover" }));
    expect(settingsStore.getState().state.settings.interaction.openOnHover).toBe(false);
    const click = screen.getByRole("switch", { name: "Open on click" });
    expect(click.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(click);
    expect(settingsStore.getState().state.settings.interaction.openOnClick).toBe(true);
  });

  it("changes position and size", () => {
    render(<SettingsApp />);
    open("Island");
    fireEvent.click(screen.getByRole("radio", { name: "Top right" }));
    fireEvent.click(screen.getByRole("radio", { name: "Compact" }));
    expect(settingsStore.getState().state.settings.island).toMatchObject({
      anchor: "topRight",
      size: "compact",
    });
  });

  it("shows unavailable integrations honestly outside the desktop app", () => {
    render(<SettingsApp />);
    open("Integrations");
    expect(screen.getAllByText("Desktop app only").length).toBeGreaterThan(2);
    expect(screen.getByRole("button", { name: "Install hooks" }).hasAttribute("disabled")).toBe(
      true,
    );
    expect(screen.getByRole("button", { name: "Send test" }).hasAttribute("disabled")).toBe(true);
  });

  it("keeps notification switches read-only without the activity engine", () => {
    render(<SettingsApp />);
    open("Notifications");
    expect(screen.getByRole("switch", { name: "Quiet mode" }).getAttribute("aria-disabled")).toBe(
      "true",
    );
    expect(screen.getByText(/Do Not Disturb are never changed/)).toBeTruthy();
  });

  it("shows a banner when settings were recovered", () => {
    act(() => {
      settingsStore.setState({
        state: { settings: DEFAULT_SETTINGS, loadStatus: { kind: "recovered" }, saveError: false },
      });
    });
    render(<SettingsApp />);
    expect(screen.getByRole("alert").textContent).toMatch(/settings.corrupt.json/);
  });

  it("records a shortcut (browser mode explains it needs the app)", () => {
    render(<SettingsApp />);
    fireEvent.click(screen.getByRole("button", { name: "Change" }));
    fireEvent.keyDown(window, { code: "KeyK", key: "k", ctrlKey: true, altKey: true });
    expect(screen.getByRole("alert").textContent).toMatch(/desktop app/);
  });
});
