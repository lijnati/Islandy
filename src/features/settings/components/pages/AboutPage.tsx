import { useState } from "react";

import { Switch } from "../../../../components/ui/Switch";
import type { UpdateStatus } from "../../../../services/types";
import {
  checkForUpdates,
  getAppInfo,
  getUpdateStatus,
  installUpdate,
  isNativeAvailable,
} from "../../settings-service";
import { useSettings, useSettingsStore } from "../../settings-store";
import { useNativeQuery } from "../../use-native-query";
import { IslandyMark } from "../IslandyMark";
import { Button, PageHeader, Row, Section, StatusText } from "../controls";

const BUSY = new Set<UpdateStatus["phase"]>(["checking", "downloading", "installing"]);

function checkedAgo(ms: number | null): string {
  if (ms === null) return "";
  const minutes = Math.floor((Date.now() - ms) / 60_000);
  if (minutes < 1) return " · checked just now";
  if (minutes < 60) return ` · checked ${minutes} min ago`;
  return ` · checked ${Math.floor(minutes / 60)} h ago`;
}

function UpdateState({ status }: { status: UpdateStatus | null }) {
  if (!status) return <StatusText tone="neutral">—</StatusText>;
  const v = status.version ?? "";
  switch (status.phase) {
    case "idle":
      return <StatusText tone="neutral">Not checked yet</StatusText>;
    case "checking":
      return <StatusText tone="neutral">Checking…</StatusText>;
    case "upToDate":
      return <StatusText tone="success">Up to date{checkedAgo(status.lastCheckedMs)}</StatusText>;
    case "available":
      return <StatusText tone="warning">Version {v} is available</StatusText>;
    case "downloading":
      return (
        <StatusText tone="neutral">
          Downloading {v}
          {status.progress !== null ? ` · ${status.progress}%` : "…"}
        </StatusText>
      );
    case "ready":
      return <StatusText tone="success">Version {v} is ready to install</StatusText>;
    case "installing":
      return <StatusText tone="neutral">Installing {v}…</StatusText>;
    case "failed":
      return <StatusText tone="error">{status.error ?? "The update failed."}</StatusText>;
  }
}

function UpdatesSection() {
  const prefs = useSettings((s) => s.updates);
  const update = useSettingsStore((s) => s.update);
  const query = useNativeQuery(getUpdateStatus, { pollMs: 1000 });
  const [acting, setActing] = useState(false);
  const status = query.data;
  const native = isNativeAvailable();
  const busy = acting || (status ? BUSY.has(status.phase) : false);
  const installable = status?.phase === "ready" || status?.phase === "available";

  const set = (next: Partial<typeof prefs>) =>
    void update((s) => ({ ...s, updates: { ...s.updates, ...next } }));
  const run = (action: () => Promise<unknown>) => {
    setActing(true);
    action()
      .catch(() => undefined)
      .finally(() => {
        setActing(false);
        query.reload();
      });
  };

  return (
    <Section
      title="Updates"
      description="Islandy looks for signed releases on the project's GitHub page. A check sends no information about you or this PC, and an update is installed only if its signature matches the key built into Islandy."
      footer={
        status?.notes && installable ? (
          <details>
            <summary className="cursor-pointer text-fg-muted">
              What's new in {status.version}
            </summary>
            <pre className="mt-2 max-h-48 overflow-auto rounded-[8px] border border-hairline bg-surface p-3 font-sans text-caption whitespace-pre-wrap text-fg-muted select-text">
              {status.notes}
            </pre>
          </details>
        ) : undefined
      }
    >
      <Row
        label="Status"
        description={`You have version ${status?.currentVersion ?? __APP_VERSION__}.`}
        control={
          <span className="flex items-center gap-3">
            <UpdateState status={status} />
            {installable ? (
              <Button
                variant="primary"
                busy={busy}
                disabled={!native}
                onClick={() => {
                  run(installUpdate);
                }}
              >
                Restart to update
              </Button>
            ) : (
              <Button
                busy={busy}
                disabled={!native}
                onClick={() => {
                  run(checkForUpdates);
                }}
              >
                Check now
              </Button>
            )}
          </span>
        }
      />
      <Row
        label="Check for updates automatically"
        description="Shortly after Islandy starts, then every 6 hours."
        control={
          <Switch
            label="Check for updates automatically"
            checked={prefs.autoCheck}
            onChange={(on) => {
              set({ autoCheck: on });
            }}
          />
        }
      />
      <Row
        label="Install updates automatically"
        description="Download a new version in the background and install it while you're away from the PC (Islandy restarts in a few seconds; settings and history are kept). When off, Islandy tells you and waits for “Restart to update”."
        control={
          <Switch
            label="Install updates automatically"
            checked={prefs.autoInstall}
            onChange={(on) => {
              set({ autoInstall: on });
            }}
          />
        }
      />
    </Section>
  );
}

export function AboutPage() {
  const info = useNativeQuery(getAppInfo);
  const version = info.data?.version ?? __APP_VERSION__;
  return (
    <div className="flex flex-col gap-7">
      <PageHeader title="About" />
      <div className="flex items-center gap-4 px-1">
        <IslandyMark size={56} />
        <div className="flex flex-col gap-0.5">
          <span className="text-heading font-semibold tracking-tight text-fg">Islandy</span>
          <span className="text-body text-fg-muted">
            A beautiful, context-aware Dynamic Island for Windows.
          </span>
          <span className="text-caption text-fg-subtle">
            Version {version}
            {info.data?.debug ? " (development build)" : ""}
          </span>
        </div>
      </div>

      <Section title="This build">
        <Row
          label="Version"
          control={
            <span className="text-caption text-fg-muted tabular-nums select-text">{version}</span>
          }
        />
        <Row
          label="Runtime"
          control={
            <span className="text-caption text-fg-muted select-text">
              {info.data
                ? `Tauri ${info.data.tauriVersion} · WebView ${info.data.webviewVersion ?? "unknown"}`
                : "—"}
            </span>
          }
        />
        <Row
          label="Platform"
          control={
            <span className="text-caption text-fg-muted select-text">
              {info.data ? `${info.data.os} · ${info.data.arch}` : "—"}
            </span>
          }
        />
      </Section>

      <UpdatesSection />

      <p className="px-1 text-caption text-fg-subtle">
        Islandy is an independent project inspired by the Dynamic Island idea. It is not affiliated
        with or endorsed by Apple Inc. or Microsoft.
      </p>
    </div>
  );
}
