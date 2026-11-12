import { ChevronRight } from "lucide-react";

import {
  elapsedSince,
  formatMinutes,
  isWorking,
  lastActivityLabel,
  needsAttention,
  sessionName,
  sessionStatusLabel,
  sessionTone,
} from "../../features/activity/activity-model";
import { useActivity } from "../../features/activity/activity-store";
import { openClaudeSession, openIntegrations } from "../../features/activity/navigation";
import { dispatchIsland } from "../../features/island/island-store";
import { useNow } from "../../hooks/useNow";
import type { ClaudeSession } from "../../services/types";
import { focusRing } from "../ui/IconButton";
import { StatusIndicator } from "../ui/StatusIndicator";
import { WidgetSurface } from "../ui/WidgetSurface";
import { cx } from "../ui/cx";
import { ActivityRow } from "./ActivityRow";
import { ClaudeGlyph } from "./ClaudeGlyph";

function SessionSummary({
  session,
  more,
  now,
}: {
  session: ClaudeSession;
  more: number;
  now: number;
}) {
  const since = elapsedSince(session);
  const activity = lastActivityLabel(session);
  const state = needsAttention(session) ? "attention" : isWorking(session) ? "working" : "idle";
  return (
    <button
      type="button"
      onClick={() => {
        openClaudeSession(session.id);
      }}
      aria-label={`Claude Code, ${sessionName(session)}: ${sessionStatusLabel(session)}. Open details`}
      className={cx(
        "group/s -mx-1 flex min-w-0 items-start gap-2.5 rounded-control px-1 py-0.5 text-left transition-colors duration-fast hover:bg-surface-raised",
        focusRing,
      )}
    >
      <span className="flex size-8 shrink-0 items-center justify-center rounded-[10px] border border-hairline bg-surface-elevated">
        <ClaudeGlyph state={state} size={16} />
      </span>
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-caption font-medium text-fg-muted">
          Claude Code{more > 0 ? ` · +${more} more` : ""}
        </span>
        <span className="truncate text-body font-semibold tracking-tight text-fg">
          {sessionName(session)}
        </span>
        <span className="mt-1 flex min-w-0 items-center gap-2">
          <StatusIndicator tone={sessionTone(session)} label={sessionStatusLabel(session)} />
          {since !== null && session.status === "working" && !session.stale && (
            <span className="text-caption text-fg-subtle tabular-nums">
              {formatMinutes(now - since)}
            </span>
          )}
        </span>
        {activity && session.status === "working" && !session.stale && (
          <span className="mt-0.5 truncate text-caption text-fg-subtle">{activity}</span>
        )}
      </span>
      <ChevronRight
        size={14}
        aria-hidden="true"
        className="mt-1 shrink-0 text-fg-subtle opacity-0 transition-opacity group-hover/s:opacity-100"
      />
    </button>
  );
}

/** Dashboard card: the most relevant Claude Code session and recent events. */
export function ActivityWidget({ className }: { className?: string | undefined }) {
  const now = useNow().getTime();
  const snapshot = useActivity((s) => s.snapshot);
  const connection = useActivity((s) => s.connection);
  const integration = useActivity((s) => s.integration.status?.state);
  const sessions = snapshot?.sessions.filter((s) => s.status !== "ended") ?? [];
  const primary = sessions[0];
  const events = snapshot?.events ?? [];
  const recent = events.filter((e) => e.priority < 3).slice(0, primary ? 2 : 4);

  return (
    <WidgetSurface
      title="Activity"
      className={className}
      onOpen={() => {
        dispatchIsland({ type: "OPEN_WIDGET", widget: "activity" });
      }}
    >
      {primary && <SessionSummary session={primary} more={sessions.length - 1} now={now} />}
      {recent.length > 0 ? (
        <ul
          className={cx(
            "flex flex-col gap-1.5",
            primary && "mt-auto border-t border-hairline pt-2",
          )}
        >
          {recent.map((e) => (
            <li key={e.id}>
              <ActivityRow event={e} now={now} compact />
            </li>
          ))}
        </ul>
      ) : (
        !primary && (
          <div className="flex flex-1 flex-col justify-center gap-1.5">
            <p className="text-body font-medium text-fg">No recent activity</p>
            <p className="text-caption text-fg-muted">
              {connection === "unavailable"
                ? "Activity appears in the Islandy desktop app."
                : integration === "installed"
                  ? "Claude Code sessions and build events will appear here."
                  : "Connect Claude Code or send build events to see them here."}
            </p>
            {connection !== "unavailable" && integration !== "installed" && (
              <button
                type="button"
                onClick={openIntegrations}
                className={cx(
                  "mt-1 inline-flex h-7 items-center self-start rounded-full border border-hairline px-2.5 text-caption font-medium text-fg-muted transition-colors duration-fast hover:bg-surface-hover hover:text-fg",
                  focusRing,
                )}
              >
                Set up integrations
              </button>
            )}
          </div>
        )
      )}
    </WidgetSurface>
  );
}
