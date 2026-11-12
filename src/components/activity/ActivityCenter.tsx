import { Settings2, Trash2 } from "lucide-react";
import { useState } from "react";

import { filterEvents, type ActivityFilter } from "../../features/activity/activity-model";
import { useActivity, activityStore } from "../../features/activity/activity-store";
import { openClaudeSession, openIntegrations } from "../../features/activity/navigation";
import { useNow } from "../../hooks/useNow";
import type { ActivityEvent } from "../../services/types";
import { focusRing } from "../ui/IconButton";
import { cx } from "../ui/cx";
import { ActivityRow } from "./ActivityRow";

const NO_EVENTS: ActivityEvent[] = [];

const FILTERS: { id: ActivityFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "claudeCode", label: "Claude Code" },
  { id: "developer", label: "Builds & tests" },
  { id: "system", label: "System" },
];

const chip = (active: boolean) =>
  cx(
    "inline-flex h-7 shrink-0 items-center rounded-full px-2.5 text-caption font-medium transition-colors duration-fast ease-standard",
    focusRing,
    active
      ? "bg-fg text-surface"
      : "border border-hairline text-fg-muted hover:bg-surface-hover hover:text-fg",
  );

/** The activity center: recent events with filters, dismiss and clear. */
export function ActivityCenter() {
  const now = useNow().getTime();
  const events = useActivity((s) => s.snapshot?.events ?? NO_EVENTS);
  const sessions = useActivity((s) => s.snapshot?.sessions.length ?? 0);
  const filter = useActivity((s) => s.filter);
  const connection = useActivity((s) => s.connection);
  const historyError = useActivity((s) => s.snapshot?.historyError ?? false);
  const [confirmClear, setConfirmClear] = useState(false);
  const shown = filterEvents(events, filter);

  return (
    <div className="flex h-full flex-col gap-2">
      <div className="flex items-center gap-1.5">
        <div
          role="group"
          aria-label="Filter by source"
          className="flex min-w-0 items-center gap-1.5"
        >
          {FILTERS.map((f) => (
            <button
              key={f.id}
              type="button"
              aria-pressed={filter === f.id}
              className={chip(filter === f.id)}
              onClick={() => {
                activityStore.getState().setFilter(f.id);
              }}
            >
              {f.label}
            </button>
          ))}
        </div>
        <span className="flex-1" />
        {sessions > 0 && (
          <button
            type="button"
            className={chip(false)}
            onClick={() => {
              openClaudeSession(null);
            }}
          >
            {sessions === 1 ? "1 session" : `${sessions} sessions`}
          </button>
        )}
        {events.length > 0 &&
          (confirmClear ? (
            <>
              <button
                type="button"
                className={cx(chip(false), "text-status-attention")}
                onClick={() => {
                  activityStore.getState().clearHistory();
                  setConfirmClear(false);
                }}
              >
                Clear all history
              </button>
              <button
                type="button"
                className={chip(false)}
                onClick={() => {
                  setConfirmClear(false);
                }}
              >
                Cancel
              </button>
            </>
          ) : (
            <button
              type="button"
              aria-label="Clear history"
              title="Clear history"
              className={cx(chip(false), "px-2")}
              onClick={() => {
                setConfirmClear(true);
              }}
            >
              <Trash2 size={13} strokeWidth={2} aria-hidden="true" />
            </button>
          ))}
        <button
          type="button"
          aria-label="Integrations and sources"
          title="Integrations and sources"
          className={cx(chip(false), "px-2")}
          onClick={openIntegrations}
        >
          <Settings2 size={13} strokeWidth={2} aria-hidden="true" />
        </button>
      </div>

      {shown.length > 0 ? (
        <ul
          aria-label="Recent activity"
          className="-mx-2 min-h-0 flex-1 overflow-y-auto overscroll-contain"
        >
          {shown.map((e) => (
            <li key={e.id}>
              <ActivityRow event={e} now={now} dismissible />
            </li>
          ))}
        </ul>
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center gap-1 text-center">
          <p className="text-body font-medium text-fg">
            {filter === "all" ? "No recent activity" : "Nothing from this source"}
          </p>
          <p className="max-w-[420px] text-caption text-fg-muted">
            {connection === "unavailable"
              ? "Activity appears in the Islandy desktop app."
              : "Claude Code sessions, build and test results, and system alerts appear here. History is kept on this device for 7 days."}
          </p>
        </div>
      )}
      {historyError && (
        <p className="text-caption text-status-attention">
          History can't be saved right now; it's kept in memory until Islandy quits.
        </p>
      )}
    </div>
  );
}
