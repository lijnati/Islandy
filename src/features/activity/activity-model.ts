/**
 * Pure helpers for activity: wording, tones, the ambient indicator, peeks
 * and filtering. No IPC here (see activity-store.ts).
 *
 * Wording is deliberately conservative: Islandy reports what Claude Code's
 * hooks said, never progress, estimates or what Claude is "thinking".
 */
import type {
  ActivityEvent,
  ActivityKind,
  ActivitySeverity,
  ActivitySnapshot,
  ActivitySource,
  ClaudeSession,
  PeekRequest,
  ToolCategory,
} from "../../services/types";
import type { PeekContent, PeekIcon, StatusTone } from "../island/island-store";
import { DEFAULT_ACTIVITY_SETTINGS } from "../settings/defaults";

export const EMPTY_ACTIVITY: ActivitySnapshot = {
  revision: 0,
  sessions: [],
  events: [],
  settings: DEFAULT_ACTIVITY_SETTINGS,
  receiver: "starting",
  historyError: false,
  lastReceived: { claudeCode: null, developer: null },
  usage: { turns: 0, workedMs: 0, neededYou: 0, partial: true },
};

/* ------------------------------------------------------------------------ */
/* Sessions                                                                  */
/* ------------------------------------------------------------------------ */

/** Whether a session currently asks for the user (and wasn't dismissed). */
export function needsAttention(s: ClaudeSession): boolean {
  return (s.status === "attention" || s.status === "waiting") && !s.attentionDismissed;
}

export function isWorking(s: ClaudeSession): boolean {
  return s.status === "working" && !s.stale;
}

export function sessionStatusLabel(s: ClaudeSession): string {
  if (s.stale) return "Status unknown";
  switch (s.status) {
    case "ready":
      return "Ready";
    case "working":
      return "Working";
    case "attention":
      return s.attentionDismissed ? "Waiting (dismissed)" : "Needs attention";
    case "waiting":
      return s.attentionDismissed ? "Waiting (dismissed)" : "Waiting for input";
    case "idle":
      return "Finished responding";
    case "error":
      return "Stopped with an error";
    case "ended":
      return "Session ended";
  }
}

export function sessionTone(s: ClaudeSession): StatusTone {
  if (s.stale) return "neutral";
  switch (s.status) {
    case "attention":
    case "waiting":
      return s.attentionDismissed ? "neutral" : "waiting";
    case "working":
      return "neutral";
    case "idle":
      return "success";
    case "error":
      return "attention";
    default:
      return "neutral";
  }
}

const TOOL_LABEL: Record<ToolCategory, string> = {
  reading: "Reading files",
  editing: "Editing files",
  searching: "Searching",
  running: "Running a command",
  web: "Using the web",
  delegating: "Running a subagent",
  planning: "Planning",
  mcp: "Using an MCP tool",
  other: "Using a tool",
};

/** "Last activity" as observed from hooks (the latest tool category). */
export function lastActivityLabel(s: ClaudeSession): string | null {
  return s.lastTool ? TOOL_LABEL[s.lastTool] : null;
}

export function sessionName(s: ClaudeSession): string {
  return s.project ?? "Claude Code";
}

/** `mm:ss`, or `h:mm:ss` past an hour. */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
}

/** Minute-granular elapsed time for surfaces on the minute clock: "4 min", "1 h 05 min". */
export function formatMinutes(ms: number): string {
  const minutes = Math.max(0, Math.floor(ms / 60_000));
  if (minutes < 1) return "< 1 min";
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${(minutes % 60).toString().padStart(2, "0")} min`;
}

/** What the elapsed timer measures: the current turn, else the session. */
export function elapsedSince(s: ClaudeSession): number | null {
  if (s.status === "working" && s.turnStartedAt !== null) return s.turnStartedAt;
  if (s.status === "ended") return null;
  return s.startedAt;
}

/* ------------------------------------------------------------------------ */
/* Ambient indicator (collapsed pill)                                        */
/* ------------------------------------------------------------------------ */

export type Ambient =
  | {
      kind: "claude";
      /** The session to describe (attention first, then working). */
      session: ClaudeSession;
      /** Sessions that are working or need attention. */
      active: number;
      attention: number;
    }
  | { kind: "task"; event: ActivityEvent };

/**
 * What the collapsed pill shows besides the time, if anything: Claude Code
 * sessions that need attention or are working, else a running build/test.
 * Idle, ended and stale sessions don't occupy the pill.
 */
export function ambientOf(snapshot: ActivitySnapshot | null): Ambient | null {
  if (!snapshot) return null;
  const attention = snapshot.sessions.filter(needsAttention);
  const working = snapshot.sessions.filter(isWorking);
  const first = attention[0] ?? working[0];
  if (first) {
    return {
      kind: "claude",
      session: first,
      active: attention.length + working.length,
      attention: attention.length,
    };
  }
  const task = snapshot.events.find(
    (e) => e.lifecycle === "active" && (e.kind === "buildStarted" || e.kind === "testsStarted"),
  );
  return task ? { kind: "task", event: task } : null;
}

/** Short text for the pill, e.g. "Islandy · Working" or "3 sessions · 1 needs you". */
export function ambientLabel(a: Ambient): { primary: string; secondary: string } {
  if (a.kind === "task") {
    return {
      primary: a.event.project ?? (a.event.kind === "buildStarted" ? "Build" : "Tests"),
      secondary: a.event.kind === "buildStarted" ? "Building" : "Testing",
    };
  }
  if (a.active > 1) {
    return {
      primary: `${a.active} sessions`,
      secondary:
        a.attention > 0 ? `${a.attention} need${a.attention === 1 ? "s" : ""} you` : "Working",
    };
  }
  return {
    primary: sessionName(a.session),
    secondary: needsAttention(a.session)
      ? a.session.status === "waiting"
        ? "Waiting"
        : "Needs attention"
      : "Working",
  };
}

/* ------------------------------------------------------------------------ */
/* Events                                                                    */
/* ------------------------------------------------------------------------ */

export function severityTone(severity: ActivitySeverity): StatusTone {
  switch (severity) {
    case "success":
      return "success";
    case "warning":
    case "error":
      return "attention";
    case "attention":
      return "waiting";
    case "info":
      return "neutral";
  }
}

export function eventIcon(kind: ActivityKind): PeekIcon {
  switch (kind) {
    case "claudeSessionStarted":
    case "claudeAttention":
    case "claudeWaitingForInput":
    case "claudeTurnFinished":
    case "claudeError":
    case "claudeSessionEnded":
      return "claude";
    case "buildStarted":
    case "buildSucceeded":
    case "buildFailed":
      return "build";
    case "testsStarted":
    case "testsPassed":
    case "testsFailed":
      return "tests";
    case "serverStarted":
    case "serverStopped":
      return "server";
    case "message":
      return "message";
    case "batteryLow":
    case "batteryCritical":
      return "batteryLow";
    case "powerConnected":
      return "plug";
    case "powerDisconnected":
      return "battery";
    case "networkLost":
      return "offline";
    case "networkRestored":
      return "wifi";
    case "audioOutputChanged":
      return "speaker";
    case "updateAvailable":
      return "update";
  }
}

export const SOURCE_LABEL: Record<ActivitySource, string> = {
  claudeCode: "Claude Code",
  developer: "Developer",
  system: "System",
};

function sourceTitle(e: ActivityEvent): string {
  if (e.source === "claudeCode") return "Claude Code";
  if (e.source === "system") {
    switch (e.kind) {
      case "batteryLow":
      case "batteryCritical":
        return "Battery";
      case "powerConnected":
      case "powerDisconnected":
        return "Power";
      case "audioOutputChanged":
        return "Sound";
      case "updateAvailable":
        return "Islandy";
      default:
        return "Network";
    }
  }
  return e.project ?? "Developer";
}

/**
 * A shorter title where the source is already shown (peeks, compact rows):
 * "Claude Code needs attention" → "Needs attention".
 */
export function shortTitle(e: ActivityEvent): string {
  if (e.source !== "claudeCode" || !e.title.startsWith("Claude Code ")) return e.title;
  const rest = e.title.slice("Claude Code ".length);
  if (rest.startsWith("is ")) return rest.charAt(3).toUpperCase() + rest.slice(4);
  return rest.charAt(0).toUpperCase() + rest.slice(1);
}

/** One line of secondary text for an event. */
export function eventDetail(e: ActivityEvent): string {
  const parts =
    e.source === "developer"
      ? [e.description]
      : e.source === "claudeCode"
        ? [e.project, e.description]
        : [e.description];
  return parts.filter((p): p is string => Boolean(p)).join(" · ");
}

export function peekForActivity(request: PeekRequest): PeekContent {
  const { event } = request;
  return {
    source: sourceTitle(event),
    title: shortTitle(event),
    detail: eventDetail(event) || (event.source === "claudeCode" ? "Claude Code" : ""),
    tone: severityTone(event.severity),
    icon: eventIcon(event.kind),
    dwellMs: request.dwellMs,
  };
}

/* ------------------------------------------------------------------------ */
/* Filtering                                                                 */
/* ------------------------------------------------------------------------ */

export type ActivityFilter = "all" | ActivitySource;

export function filterEvents(
  events: readonly ActivityEvent[],
  filter: ActivityFilter,
): ActivityEvent[] {
  return filter === "all" ? [...events] : events.filter((e) => e.source === filter);
}

export function eventsForSession(
  events: readonly ActivityEvent[],
  session: string,
): ActivityEvent[] {
  return events.filter((e) => e.session === session);
}

/** Active attention events can be dismissed; others are removed from history. */
export function dismissLabel(e: ActivityEvent): string {
  return e.lifecycle === "active" ? "Dismiss" : "Remove from history";
}
