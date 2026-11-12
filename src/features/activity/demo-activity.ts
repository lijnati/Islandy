/**
 * The tray's "Preview peek (demo)" sample. Static text: nothing is read from
 * Claude Code or any other app. Real activity comes from the activity
 * engine (see activity-store.ts).
 */
import type { PeekContent } from "../island/island-store";

export const DEMO_PEEK: PeekContent = {
  source: "Claude Code",
  title: "Claude finished responding",
  detail: "Demo · Islandy",
  tone: "success",
  icon: "claude",
};
