import { useState } from "react";

import { activityStore, useActivity } from "../../features/activity/activity-store";
import type { ActivitySource, IntegrationState } from "../../services/types";
import { Tile } from "../system/shared";
import { focusRing } from "../ui/IconButton";
import { StatusIndicator } from "../ui/StatusIndicator";
import { Switch } from "../ui/Switch";
import { cx } from "../ui/cx";

const STATE_TEXT: Record<
  IntegrationState,
  { label: string; tone: "neutral" | "success" | "waiting" | "attention" }
> = {
  notDetected: { label: "Claude Code not found", tone: "neutral" },
  notInstalled: { label: "Not enabled", tone: "neutral" },
  installed: { label: "Enabled", tone: "success" },
  needsUpdate: { label: "Needs repair", tone: "waiting" },
  unreadable: { label: "Settings file can't be read safely", tone: "attention" },
};

const button = (primary = false) =>
  cx(
    "inline-flex h-7 shrink-0 items-center rounded-full px-3 text-caption font-medium transition-colors duration-fast ease-standard disabled:cursor-not-allowed disabled:opacity-50",
    focusRing,
    primary
      ? "bg-accent text-on-accent hover:bg-accent/90"
      : "border border-hairline text-fg-muted hover:bg-surface-hover hover:text-fg",
  );

function ClaudeIntegrationTile() {
  const { status, busy, error } = useActivity((s) => s.integration);
  const connection = useActivity((s) => s.connection);
  const [confirm, setConfirm] = useState<"install" | "remove" | null>(null);
  const state = status?.state;
  const text = state ? STATE_TEXT[state] : null;
  const desktop = connection !== "unavailable";

  return (
    <Tile
      title="Claude Code"
      className="min-w-0 flex-[1.45]"
      accessory={text && <StatusIndicator tone={text.tone} label={text.label} />}
    >
      {confirm ? (
        <div className="flex flex-1 flex-col gap-2">
          <p className="text-body text-fg">
            {confirm === "install"
              ? `Add ${status?.events.length ?? 12} observational hooks to your user settings?`
              : "Remove Islandy's hooks from your user settings?"}
          </p>
          <p className="truncate text-caption text-fg-muted" title={status?.settingsPath ?? ""}>
            {status?.settingsPath ?? "~/.claude/settings.json"}
          </p>
          <p className="text-caption text-fg-muted">
            {confirm === "install"
              ? "A backup is saved first. Other hooks and settings stay as they are. Project settings are not touched."
              : "A backup is saved first. Only Islandy's hooks are removed."}
          </p>
          <div className="mt-auto flex gap-2">
            <button
              type="button"
              className={button(true)}
              disabled={busy}
              onClick={() => {
                const store = activityStore.getState();
                void (confirm === "install"
                  ? store.installIntegration()
                  : store.removeIntegration());
                setConfirm(null);
              }}
            >
              {confirm === "install" ? "Add hooks" : "Remove hooks"}
            </button>
            <button
              type="button"
              className={button()}
              onClick={() => {
                setConfirm(null);
              }}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div className="flex flex-1 flex-col gap-1.5">
          <p className="text-caption leading-[16px] text-fg-muted">
            Islandy receives the hook event name, an anonymous session key, the project folder name
            and a tool category (for example “editing files”). Never prompts, responses, commands,
            file contents or paths. Everything stays on this device.
          </p>
          {error ? (
            <p role="status" className="text-caption text-status-attention">
              {error}
            </p>
          ) : state === "notDetected" ? (
            <p className="text-caption text-fg-muted">Run Claude Code once, then check again.</p>
          ) : state === "installed" ? (
            <p className="text-caption text-fg-muted">
              New Claude Code sessions report to Islandy. A running session may need a restart.
            </p>
          ) : null}
          <div className="mt-auto flex flex-wrap gap-2">
            {!desktop ? (
              <p className="text-caption text-fg-subtle">Available in the Islandy desktop app.</p>
            ) : state === "installed" ? (
              <button
                type="button"
                className={button()}
                disabled={busy}
                onClick={() => {
                  setConfirm("remove");
                }}
              >
                Remove hooks
              </button>
            ) : (
              <button
                type="button"
                className={button(true)}
                disabled={busy || !state || state === "notDetected" || state === "unreadable"}
                onClick={() => {
                  setConfirm("install");
                }}
              >
                {state === "needsUpdate" ? "Repair hooks" : "Enable"}
              </button>
            )}
            {state === "needsUpdate" && (
              <button
                type="button"
                className={button()}
                disabled={busy}
                onClick={() => {
                  setConfirm("remove");
                }}
              >
                Remove hooks
              </button>
            )}
            {desktop && (
              <button
                type="button"
                className={button()}
                disabled={busy}
                onClick={() => {
                  void activityStore.getState().refreshIntegration();
                }}
              >
                Check again
              </button>
            )}
          </div>
        </div>
      )}
    </Tile>
  );
}

const SOURCES: { source: ActivitySource; label: string }[] = [
  { source: "claudeCode", label: "Claude Code" },
  { source: "developer", label: "Builds, tests & scripts" },
  { source: "system", label: "System alerts" },
];

function SourcesTile() {
  const settings = useActivity((s) => s.snapshot?.settings);
  const pending = useActivity((s) => s.pending);
  const receiver = useActivity((s) => s.snapshot?.receiver);
  const connection = useActivity((s) => s.connection);
  const disabled = connection !== "connected" || !settings;
  return (
    <Tile title="Sources" className="w-[268px] shrink-0">
      <ul className="flex flex-col gap-1.5">
        {SOURCES.map(({ source, label }) => (
          <li key={source} className="flex items-center justify-between gap-2">
            <span className="truncate text-body text-fg">{label}</span>
            <Switch
              label={label}
              checked={settings?.sources[source] ?? false}
              disabled={disabled}
              busy={Boolean(pending[`source:${source}`])}
              onChange={(on) => {
                activityStore.getState().setSource(source, on);
              }}
            />
          </li>
        ))}
        <li className="flex items-center justify-between gap-2 border-t border-hairline pt-1.5">
          <span className="truncate text-body text-fg">Quiet mode</span>
          <Switch
            label="Quiet mode"
            checked={settings?.quiet ?? false}
            disabled={disabled}
            busy={Boolean(pending.quiet)}
            onChange={(on) => {
              activityStore.getState().setQuiet(on);
            }}
          />
        </li>
      </ul>
      <p className="mt-auto truncate text-caption text-fg-subtle">
        {receiver === "listening"
          ? "Local receiver ready for hooks and scripts"
          : receiver === "unavailable"
            ? "Local receiver unavailable: events can't arrive"
            : "Local events: desktop app only"}
      </p>
    </Tile>
  );
}

/** Opt-in integrations and per-source switches. */
export function IntegrationsPanel() {
  return (
    <div className="flex h-full gap-3">
      <ClaudeIntegrationTile />
      <SourcesTile />
    </div>
  );
}
