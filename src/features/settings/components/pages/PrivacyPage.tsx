import { Copy, FolderOpen } from "lucide-react";
import { useState } from "react";

import { Switch } from "../../../../components/ui/Switch";
import { isNativeAvailable } from "../../../../services/native";
import { DEFAULT_ACTIVITY_SETTINGS } from "../../defaults";
import { formatSize } from "../../format";
import {
  clearActivityHistory,
  getDiagnostics,
  getStorageInfo,
  openIslandyFolder,
} from "../../settings-service";
import { useSettings, useSettingsStore } from "../../settings-store";
import { useActivitySettings } from "../../use-activity-settings";
import { useNativeQuery } from "../../use-native-query";
import { Button, ConfirmButton, PageHeader, Row, Section, Segmented } from "../controls";

const RETENTION = [1, 3, 7, 14, 30] as const;

function DiagnosticsRow() {
  const [text, setText] = useState<string | null>(null);
  const [copied, setCopied] = useState<"copied" | "failed" | null>(null);
  const load = () => {
    getDiagnostics().then(
      (summary) => {
        setText(summary);
        navigator.clipboard.writeText(summary).then(
          () => {
            setCopied("copied");
          },
          () => {
            setCopied("failed");
          },
        );
      },
      (cause: unknown) => {
        setText(String(cause));
      },
    );
  };
  return (
    <div className="flex flex-col gap-2 px-4 py-3">
      <div className="flex items-center justify-between gap-4">
        <div className="flex flex-col gap-0.5">
          <span className="text-body font-medium text-fg">Diagnostic summary</span>
          <span className="text-caption text-fg-muted">
            Version, Windows version, feature availability, integration status and recent warnings.
            No activity, tokens or file contents.
          </span>
        </div>
        <Button disabled={!isNativeAvailable()} onClick={load}>
          <Copy size={13} strokeWidth={2} aria-hidden="true" />
          Copy
        </Button>
      </div>
      {copied && (
        <p role="status" className="text-caption text-fg-muted">
          {copied === "copied"
            ? "Copied to the clipboard. Review it before sharing."
            : "Couldn't copy; select the text below."}
        </p>
      )}
      {text && (
        <pre className="max-h-48 overflow-auto rounded-[8px] bg-surface p-3 font-mono text-[11px] leading-[16px] whitespace-pre-wrap text-fg-muted select-text">
          {text}
        </pre>
      )}
    </div>
  );
}

export function PrivacyPage() {
  const activity = useActivitySettings();
  const storage = useNativeQuery(getStorageInfo, { pollMs: 5000 });
  const verbose = useSettings((s) => s.general.verboseLogging);
  const update = useSettingsStore((s) => s.update);
  const settings = activity.settings ?? DEFAULT_ACTIVITY_SETTINGS;
  const events = activity.snapshot?.events.length ?? 0;
  const native = isNativeAvailable();

  return (
    <div className="flex flex-col gap-7">
      <PageHeader
        title="Privacy"
        description="Islandy works on this PC. It goes online only to check for updates and, if you choose a place, to load the weather."
      />

      <section
        aria-label="What Islandy collects"
        className="rounded-field border border-hairline bg-surface-raised p-4"
      >
        <ul className="grid grid-cols-2 gap-x-6 gap-y-1.5 text-caption text-fg-muted">
          <li>• No account, no cloud, no telemetry or analytics.</li>
          <li>
            • Nothing about you is uploaded. Online: update checks (GitHub), weather (Open-Meteo).
          </li>
          <li>• Never collects prompts, responses or tool output.</li>
          <li>• Never reads source code, terminal history or environment variables.</li>
          <li>• Activity history: event names, project folder names, times.</li>
          <li>• Logs are local, bounded and never include hook payloads.</li>
        </ul>
      </section>

      <Section
        title="Activity history"
        description={
          native ? `${events} event${events === 1 ? "" : "s"} stored (at most 300).` : undefined
        }
      >
        <Row
          label="Keep history for"
          control={
            <Segmented
              label="Keep history for"
              value={String(settings.retentionDays)}
              disabled={!activity.ready}
              onChange={(v) => {
                activity.update({ ...settings, retentionDays: Number(v) });
              }}
              options={RETENTION.map((d) => ({
                value: String(d),
                label: d === 1 ? "1 day" : `${d} days`,
              }))}
            />
          }
        />
        <Row
          label="Clear activity history"
          description="Deletes every stored event now. Open sessions keep running."
          control={
            <ConfirmButton
              question="Delete all history?"
              confirmLabel="Clear"
              disabled={!native}
              onConfirm={() => {
                void clearActivityHistory().then(() => {
                  setTimeout(activity.reload, 150);
                });
              }}
            >
              Clear…
            </ConfirmButton>
          }
        />
      </Section>

      <Section
        title="Stored on this PC"
        description={
          storage.data?.dataDir ? (
            <span className="font-mono text-[11px] select-text">{storage.data.dataDir}</span>
          ) : undefined
        }
        footer="Claude Code hooks live in your Claude Code settings; remove them from Integrations. The receiver token is a per-user file protected by your Windows profile and is never shown or logged."
      >
        {(storage.data?.files ?? []).map((f) => (
          <Row
            key={f.name}
            label={<span className="font-mono text-[12px]">{f.name}</span>}
            description={f.purpose}
            control={
              <span className="text-caption text-fg-muted tabular-nums">{formatSize(f.bytes)}</span>
            }
          />
        ))}
        <Row
          label="Open folders"
          control={
            <div className="flex gap-2">
              <Button disabled={!native} onClick={() => void openIslandyFolder("data")}>
                <FolderOpen size={13} strokeWidth={2} aria-hidden="true" />
                Data
              </Button>
              <Button disabled={!native} onClick={() => void openIslandyFolder("logs")}>
                <FolderOpen size={13} strokeWidth={2} aria-hidden="true" />
                Logs
              </Button>
            </div>
          }
        />
      </Section>

      <Section title="Diagnostics">
        <Row
          label="Detailed logging"
          description="Adds debug entries to the local log. Leave off unless you're troubleshooting."
          control={
            <Switch
              label="Detailed logging"
              checked={verbose}
              onChange={(on) =>
                void update((s) => ({ ...s, general: { ...s.general, verboseLogging: on } }))
              }
            />
          }
        />
        <DiagnosticsRow />
      </Section>
    </div>
  );
}
