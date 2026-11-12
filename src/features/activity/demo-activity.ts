/**
 * DEMO activity data for Phase 1.
 *
 * Static sample entries; nothing is read from Claude Code or any other app,
 * and no events are generated in the background. Real activity arrives in
 * Phase 4 through a local, authenticated channel (see
 * docs/windows-integration.md → Claude Code hooks).
 */
import type { StatusTone } from "../island/island-store";

export type ActivityStatus = "working" | "waiting" | "done" | "failed";

export interface Activity {
  id: string;
  app: string;
  /** Lucide icon key; see ActivityCard. */
  icon: "terminal" | "hammer" | "download";
  title: string;
  status: ActivityStatus;
  /** Epoch ms of the last status change. */
  since: number;
}

export const statusTone: Record<ActivityStatus, StatusTone> = {
  working: "working",
  waiting: "attention",
  done: "success",
  failed: "attention",
};

export const statusLabel: Record<ActivityStatus, string> = {
  working: "Working",
  waiting: "Needs input",
  done: "Done",
  failed: "Failed",
};

const loadedAt = Date.now();
const minutes = (n: number) => loadedAt - n * 60_000;

export const DEMO_ACTIVITIES: readonly Activity[] = [
  {
    id: "a1",
    app: "Claude Code",
    icon: "terminal",
    title: "Building Islandy",
    status: "working",
    since: minutes(4),
  },
  {
    id: "a2",
    app: "Cargo",
    icon: "hammer",
    title: "Release build",
    status: "done",
    since: minutes(12),
  },
  {
    id: "a3",
    app: "Downloads",
    icon: "download",
    title: "WebView2 runtime",
    status: "done",
    since: minutes(47),
  },
];

/** A sample peek, used by the tray's "Preview peek (demo)". */
export const DEMO_PEEK = {
  source: "Claude Code",
  title: "Task finished",
  detail: "Islandy · 2m 14s",
  tone: "success",
} as const;
