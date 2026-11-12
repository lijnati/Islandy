import { dispatchIsland } from "../island/island-store";
import { activityStore } from "./activity-store";

/** Opens the Claude Code view on a session (or the most relevant one). */
export function openClaudeSession(id: string | null) {
  activityStore.getState().selectSession(id);
  dispatchIsland({ type: "OPEN_WIDGET", widget: "claude" });
}

export function openIntegrations() {
  dispatchIsland({ type: "OPEN_WIDGET", widget: "integrations" });
}
