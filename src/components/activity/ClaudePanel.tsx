import { useActivity, activityStore } from "../../features/activity/activity-store";
import {
  elapsedSince,
  eventsForSession,
  formatElapsed,
  isWorking,
  lastActivityLabel,
  needsAttention,
  sessionName,
  sessionStatusLabel,
  sessionTone,
} from "../../features/activity/activity-model";
import { openIntegrations } from "../../features/activity/navigation";
import { formatRelative } from "../../features/island/format";
import { useNow } from "../../hooks/useNow";
import { useSeconds } from "../../hooks/useSeconds";
import type { ClaudeSession } from "../../services/types";
import { Caption, Tile } from "../system/shared";
import { focusRing } from "../ui/IconButton";
import { StatusIndicator } from "../ui/StatusIndicator";
import { cx } from "../ui/cx";
import { ActivityRow } from "./ActivityRow";
import { ClaudeGlyph } from "./ClaudeGlyph";

const NO_SESSIONS: ClaudeSession[] = [];

const glyphState = (s: ClaudeSession) =>
  needsAttention(s) ? "attention" : isWorking(s) ? "working" : "idle";

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <Caption className="truncate whitespace-nowrap">{label}</Caption>
      <span className="truncate text-body text-fg tabular-nums">{children}</span>
    </div>
  );
}

/** Compact session switcher: one row per session, attention first. */
function SessionList({ sessions, selected }: { sessions: ClaudeSession[]; selected: string }) {
  return (
    <ul aria-label="Claude Code sessions" className="flex min-h-0 flex-col gap-1 overflow-y-auto">
      {sessions.map((s) => {
        const active = s.id === selected;
        return (
          <li key={s.id}>
            <button
              type="button"
              aria-current={active || undefined}
              onClick={() => {
                activityStore.getState().selectSession(s.id);
              }}
              className={cx(
                "flex h-9 w-full items-center gap-2 rounded-control px-2 text-left transition-colors duration-fast",
                focusRing,
                active ? "bg-surface-hover" : "hover:bg-surface-raised",
              )}
            >
              <ClaudeGlyph state={glyphState(s)} size={13} />
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-caption font-medium text-fg">{sessionName(s)}</span>
                <span className="truncate text-[10px] leading-[12px] text-fg-muted">
                  {sessionStatusLabel(s)}
                </span>
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

function SessionDetail({ session, now: minuteNow }: { session: ClaudeSession; now: number }) {
  const events = useActivity((s) => s.snapshot?.events);
  const since = elapsedSince(session);
  const activity = lastActivityLabel(session);
  const attentionEvent = events?.find((e) => e.id === session.attentionEvent);
  const recent = events ? eventsForSession(events, session.id).slice(0, 2) : [];
  const working = isWorking(session);
  // A per-second clock only while this view shows a running turn.
  const now = useSeconds(working && since !== null, minuteNow);

  return (
    <div className="flex min-w-0 flex-1 flex-col gap-2.5">
      <div className="flex min-w-0 items-center gap-2">
        <ClaudeGlyph state={glyphState(session)} size={16} />
        <h3 className="truncate text-title font-semibold tracking-tight text-fg">
          {sessionName(session)}
        </h3>
        <StatusIndicator
          className="ml-1 shrink-0"
          tone={sessionTone(session)}
          label={sessionStatusLabel(session)}
        />
      </div>

      {needsAttention(session) ? (
        <div
          role="status"
          className="flex items-center gap-3 rounded-control border border-status-waiting/30 bg-status-waiting/10 px-3 py-2"
        >
          <div className="flex min-w-0 flex-1 flex-col">
            <span className="text-body font-medium text-fg">
              {session.status === "waiting"
                ? "Claude Code is waiting for input"
                : `Claude Code needs attention${attentionEvent?.description ? ` · ${attentionEvent.description}` : ""}`}
            </span>
            <span className="text-caption text-fg-muted">
              Respond in the terminal where Claude Code is running.
            </span>
          </div>
          {session.attentionEvent && (
            <button
              type="button"
              onClick={() => {
                if (session.attentionEvent)
                  activityStore.getState().dismiss(session.attentionEvent);
              }}
              className={cx(
                "inline-flex h-7 shrink-0 items-center rounded-full border border-hairline px-2.5 text-caption font-medium text-fg-muted transition-colors hover:bg-surface-hover hover:text-fg",
                focusRing,
              )}
            >
              Dismiss
            </button>
          )}
        </div>
      ) : (
        <div className="grid grid-cols-3 gap-3">
          <Fact label="Last activity">
            {session.stale
              ? `No events for ${formatRelative(session.lastSeenAt, now).replace(" ago", "")}`
              : working
                ? (activity ?? "Working")
                : "—"}
          </Fact>
          <Fact label={working ? "This turn" : "Session"}>
            {since !== null ? formatElapsed(now - since) : "Started before Islandy"}
          </Fact>
          <Fact label="Last event">{formatRelative(session.lastSeenAt, now)}</Fact>
        </div>
      )}

      {session.subagents > 0 && (
        <p className="text-caption text-fg-muted">
          {session.subagents === 1
            ? "1 subagent running"
            : `${session.subagents} subagents running`}
        </p>
      )}

      {recent.length > 0 && (
        <ul className="mt-auto flex flex-col gap-1.5 border-t border-hairline pt-2">
          {recent.map((e) => (
            <li key={e.id}>
              <ActivityRow event={e} now={now} compact />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/**
 * Claude Code sessions as observed from hooks. Status is what the hooks
 * reported; there is no progress, estimate or remote control here.
 */
export function ClaudePanel() {
  const now = useNow().getTime();
  const sessions = useActivity((s) => s.snapshot?.sessions ?? NO_SESSIONS);
  const selectedId = useActivity((s) => s.selectedSession);
  const integration = useActivity((s) => s.integration.status?.state);
  const selected = sessions.find((s) => s.id === selectedId) ?? sessions[0];

  if (!selected) {
    return (
      <Tile title="Claude Code" className="h-full">
        <div className="flex flex-1 flex-col justify-center gap-1.5">
          <p className="text-body font-medium text-fg">No Claude Code sessions right now</p>
          <p className="max-w-[520px] text-caption text-fg-muted">
            {integration === "installed"
              ? "Sessions appear when Claude Code reports activity through Islandy's hooks. Sessions that started before the hooks were added appear after their next event."
              : "Enable the Claude Code integration to see sessions here. Islandy adds observational hooks to your user settings; nothing is sent anywhere else."}
          </p>
          {integration !== "installed" && (
            <button
              type="button"
              onClick={openIntegrations}
              className={cx(
                "mt-1 inline-flex h-7 items-center self-start rounded-full border border-hairline px-2.5 text-caption font-medium text-fg-muted transition-colors hover:bg-surface-hover hover:text-fg",
                focusRing,
              )}
            >
              Set up Claude Code
            </button>
          )}
        </div>
      </Tile>
    );
  }

  return (
    <div className="flex h-full gap-3">
      {sessions.length > 1 && (
        <div className="flex w-[34%] max-w-[188px] shrink-0 flex-col gap-1.5 rounded-tile border border-hairline bg-surface-secondary p-2 shadow-surface">
          <Caption className="px-1">{sessions.length} sessions</Caption>
          <SessionList sessions={sessions} selected={selected.id} />
        </div>
      )}
      <div className="flex min-w-0 flex-1 rounded-tile border border-hairline bg-surface-secondary p-3 shadow-surface">
        <SessionDetail session={selected} now={now} />
      </div>
    </div>
  );
}
