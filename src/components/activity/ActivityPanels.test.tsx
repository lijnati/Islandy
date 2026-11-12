import { act, cleanup, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_ACTIVITY_SETTINGS } from "../../features/settings/defaults";

import { activityStore, initialActivityState } from "../../features/activity/activity-store";
import { event, session, snapshot, T0 } from "../../features/activity/activity-test-utils";
import { islandStore } from "../../features/island/island-store";
import type { ActivitySnapshot, ClaudeIntegration } from "../../services/types";
import { CollapsedIsland } from "../island/CollapsedIsland";
import { NowProvider } from "../island/NowProvider";
import { ActivityCenter } from "./ActivityCenter";
import { ActivityWidget } from "./ActivityWidget";
import { ClaudePanel } from "./ClaudePanel";
import { IntegrationsPanel } from "./IntegrationsPanel";

const render = (ui: ReactElement) => rtlRender(<NowProvider>{ui}</NowProvider>);

function load(s: ActivitySnapshot, integration: ClaudeIntegration["state"] = "installed") {
  act(() => {
    activityStore.setState({
      ...initialActivityState,
      connection: "connected",
      snapshot: s,
      integration: {
        status: {
          state: integration,
          settingsPath: "/home/u/.claude/settings.json",
          events: ["SessionStart", "Stop"],
          snippet: "{}",
        },
        busy: false,
        error: null,
      },
    });
  });
}

afterEach(() => {
  cleanup();
  activityStore.setState(initialActivityState);
  islandStore.setState({ mode: "collapsed", activeWidget: null, pinned: false });
  vi.restoreAllMocks();
});

describe("collapsed pill", () => {
  it("shows one working session as project · Working", () => {
    load(snapshot({ sessions: [session()] }));
    render(<CollapsedIsland />);
    const pill = screen.getByRole("button", { name: /Claude Code, Islandy: Working/ });
    expect(pill.textContent).toContain("Islandy · Working");
  });

  it("keeps a persistent attention indicator and counts sessions", () => {
    load(
      snapshot({
        sessions: [
          session({ id: "c1", project: "Web", status: "attention", attentionEvent: "a1" }),
          session({ id: "c2" }),
          session({ id: "c3", project: "Api" }),
        ],
      }),
    );
    render(<CollapsedIsland />);
    const pill = screen.getByRole("button", { name: /3 sessions, 1 needs you/ });
    expect(pill.textContent).toContain("3 sessions · 1 needs you");
  });

  it("marks quiet mode", () => {
    load(
      snapshot({
        settings: { ...DEFAULT_ACTIVITY_SETTINGS, quiet: true },
      }),
    );
    render(<CollapsedIsland />);
    expect(screen.getByRole("button", { name: /Quiet mode on/ })).toBeDefined();
  });

  it("is the plain pill without activity", () => {
    load(snapshot({ sessions: [session({ status: "idle" })] }));
    render(<CollapsedIsland />);
    expect(screen.getByRole("button").textContent).not.toContain("·");
  });
});

describe("activity center", () => {
  const events = [
    event({ id: "a1", title: "Claude finished responding" }),
    event({
      id: "a2",
      source: "developer",
      kind: "buildFailed",
      title: "Build failed",
      severity: "error",
      session: null,
    }),
    event({
      id: "a3",
      source: "system",
      kind: "networkLost",
      title: "Network disconnected",
      severity: "warning",
      session: null,
      project: null,
    }),
  ];

  it("filters by source", () => {
    load(snapshot({ events }));
    render(<ActivityCenter />);
    expect(screen.getAllByRole("listitem")).toHaveLength(3);
    fireEvent.click(screen.getByRole("button", { name: "Builds & tests" }));
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    expect(screen.getByText("Islandy · Build failed")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "System" }));
    expect(screen.getByText("Network disconnected")).toBeDefined();
  });

  it("dismisses single events and asks before clearing history", () => {
    load(snapshot({ events }));
    const dismiss = vi.fn();
    const clearHistory = vi.fn();
    act(() => {
      activityStore.setState({ dismiss, clearHistory });
    });
    render(<ActivityCenter />);
    fireEvent.click(screen.getByRole("button", { name: "Remove from history: Build failed" }));
    expect(dismiss).toHaveBeenCalledWith("a2");
    fireEvent.click(screen.getByRole("button", { name: "Clear history" }));
    expect(clearHistory).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Clear all history" }));
    expect(clearHistory).toHaveBeenCalledTimes(1);
  });

  it("has an empty state", () => {
    load(snapshot());
    render(<ActivityCenter />);
    expect(screen.getByText("No recent activity")).toBeDefined();
    expect(screen.getByText(/kept on this device for 7 days/)).toBeDefined();
  });
});

describe("Claude Code view", () => {
  it("switches between sessions independently", () => {
    load(
      snapshot({
        sessions: [
          session({ id: "c1", project: "Islandy", lastTool: "editing" }),
          session({ id: "c2", project: "Islandy", status: "idle", lastTool: null }),
        ],
      }),
    );
    render(<ClaudePanel />);
    const list = screen.getByRole("list", { name: "Claude Code sessions" });
    expect(list.querySelectorAll("li")).toHaveLength(2);
    expect(screen.getByText("Editing files")).toBeDefined();
    fireEvent.click(screen.getAllByRole("button", { name: /Islandy/ })[1] as HTMLElement);
    expect(activityStore.getState().selectedSession).toBe("c2");
    expect(screen.getAllByText("Finished responding").length).toBeGreaterThan(0);
  });

  it("shows attention with dismiss only — no remote approval", () => {
    const attention = event({
      id: "a9",
      kind: "claudeAttention",
      title: "Claude Code needs attention",
      description: "Permission requested",
      severity: "attention",
      priority: 0,
      lifecycle: "active",
    });
    load(
      snapshot({
        sessions: [session({ status: "attention", attentionEvent: "a9" })],
        events: [attention],
      }),
    );
    const dismiss = vi.fn();
    act(() => {
      activityStore.setState({ dismiss });
    });
    render(<ClaudePanel />);
    expect(screen.getByText(/needs attention · Permission requested/)).toBeDefined();
    expect(screen.getByText(/Respond in the terminal/)).toBeDefined();
    expect(screen.queryByRole("button", { name: /approve|allow|accept/i })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(dismiss).toHaveBeenCalledWith("a9");
  });

  it("says when a session started before Islandy saw it", () => {
    vi.useFakeTimers();
    vi.setSystemTime(T0 + 60_000);
    load(snapshot({ sessions: [session({ status: "idle", startedAt: null })] }));
    render(<ClaudePanel />);
    expect(screen.getByText("Started before Islandy")).toBeDefined();
    vi.useRealTimers();
  });

  it("explains how to connect when there are no sessions", () => {
    load(snapshot(), "notInstalled");
    render(<ClaudePanel />);
    expect(screen.getByRole("button", { name: "Set up Claude Code" })).toBeDefined();
  });
});

describe("integrations", () => {
  it("enables only after an explicit confirmation and explains the data", () => {
    load(snapshot(), "notInstalled");
    const installIntegration = vi.fn(() => Promise.resolve());
    act(() => {
      activityStore.setState({ installIntegration });
    });
    render(<IntegrationsPanel />);
    expect(
      screen.getByText(/Never prompts, responses, commands, file contents or paths/),
    ).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Enable" }));
    expect(installIntegration).not.toHaveBeenCalled();
    expect(screen.getByText(/A backup is saved first/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Add hooks" }));
    expect(installIntegration).toHaveBeenCalledTimes(1);
  });

  it("offers removal when installed and shows per-source switches", () => {
    load(snapshot());
    const setSource = vi.fn();
    act(() => {
      activityStore.setState({ setSource });
    });
    render(<IntegrationsPanel />);
    expect(screen.getByRole("button", { name: "Remove hooks" })).toBeDefined();
    fireEvent.click(screen.getByRole("switch", { name: "Builds, tests & scripts" }));
    expect(setSource).toHaveBeenCalledWith("developer", false);
  });

  it("refuses to enable when the settings file is unreadable", () => {
    load(snapshot(), "unreadable");
    render(<IntegrationsPanel />);
    expect(screen.getByRole("button", { name: "Enable" }).hasAttribute("disabled")).toBe(true);
  });
});

describe("dashboard card", () => {
  it("summarizes the most relevant session and recent outcomes", () => {
    load(
      snapshot({
        sessions: [session({ status: "waiting", attentionEvent: "a1" })],
        events: [event({ title: "Build failed", source: "developer", priority: 1, session: null })],
      }),
    );
    render(<ActivityWidget />);
    expect(
      screen.getByRole("button", { name: "Claude Code, Islandy: Waiting for input. Open details" }),
    ).toBeDefined();
    expect(screen.getByText(/Build failed/)).toBeDefined();
  });
});
