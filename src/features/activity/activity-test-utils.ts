import type {
  ActivityEvent,
  ActivitySnapshot,
  ClaudeSession,
  PeekRequest,
} from "../../services/types";
import { EMPTY_ACTIVITY } from "./activity-model";

export const T0 = 1_700_000_000_000;

export function session(overrides: Partial<ClaudeSession> = {}): ClaudeSession {
  return {
    id: "c0000000000000001",
    project: "Islandy",
    status: "working",
    startedAt: T0,
    turnStartedAt: T0,
    lastSeenAt: T0,
    lastTool: "editing",
    subagents: 0,
    attentionEvent: null,
    attentionDismissed: false,
    endedAt: null,
    stale: false,
    ...overrides,
  };
}

let next = 1;
export function event(overrides: Partial<ActivityEvent> = {}): ActivityEvent {
  return {
    id: `a${(next++).toString(16)}`,
    source: "claudeCode",
    kind: "claudeTurnFinished",
    session: "c0000000000000001",
    project: "Islandy",
    title: "Claude finished responding",
    description: null,
    severity: "success",
    priority: 1,
    lifecycle: "resolved",
    timestamp: T0,
    expiresAt: null,
    dedupeKey: null,
    ...overrides,
  };
}

export function snapshot(overrides: Partial<ActivitySnapshot> = {}): ActivitySnapshot {
  return { ...EMPTY_ACTIVITY, revision: 1, receiver: "listening", ...overrides };
}

export function peekRequest(e: ActivityEvent, dwellMs = 4000): PeekRequest {
  return { event: e, dwellMs };
}
