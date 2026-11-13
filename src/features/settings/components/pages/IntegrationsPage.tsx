import { useState } from "react";

import { isNativeAvailable } from "../../../../services/native";
import type {
  ClaudeIntegration,
  IntegrationState,
  SystemSnapshot,
  Availability,
} from "../../../../services/types";
import {
  getActivitySnapshot,
  getClaudeIntegration,
  getMediaSnapshot,
  getSystemSnapshot,
  installClaudeIntegration,
  removeClaudeIntegration,
  testLocalReceiver,
} from "../../settings-service";
import { useNativeQuery } from "../../use-native-query";
import { formatAgo } from "../../format";
import { Button, Code, ConfirmButton, PageHeader, Row, Section, StatusText } from "../controls";

const CLAUDE_STATE: Record<
  IntegrationState,
  { text: string; tone: "neutral" | "success" | "warning" | "error" }
> = {
  notDetected: { text: "Claude Code not found", tone: "neutral" },
  notInstalled: { text: "Hooks not installed", tone: "neutral" },
  installed: { text: "Hooks installed", tone: "success" },
  needsUpdate: { text: "Hooks need repair", tone: "warning" },
  unreadable: { text: "Settings file can't be read safely", tone: "error" },
};

function errorText(cause: unknown) {
  return cause instanceof Error ? cause.message.replace(/^[a-z_]+: /, "") : String(cause);
}

function ClaudeSection() {
  const integration = useNativeQuery(getClaudeIntegration);
  const activity = useNativeQuery(getActivitySnapshot, { pollMs: 3000 });
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const status: ClaudeIntegration | null = integration.data;
  const state = status?.state;
  const last = activity.data?.lastReceived.claudeCode ?? null;
  const sessions = activity.data?.sessions.length ?? 0;

  const run = (action: () => Promise<ClaudeIntegration>, done: string) => {
    setBusy(true);
    action().then(
      () => {
        setBusy(false);
        setResult(done);
        integration.reload();
      },
      (cause: unknown) => {
        setBusy(false);
        setResult(errorText(cause));
      },
    );
  };

  return (
    <Section
      title="Claude Code"
      description="Observational hooks in your user settings. Islandy receives the event name, an anonymous session key, the project folder name and a tool category. Never prompts, responses, commands, file contents or paths."
    >
      <Row
        label="Status"
        description={
          status?.settingsPath ? (
            <span className="select-text">{status.settingsPath}</span>
          ) : undefined
        }
        control={
          isNativeAvailable() ? (
            state ? (
              <StatusText tone={CLAUDE_STATE[state].tone}>{CLAUDE_STATE[state].text}</StatusText>
            ) : (
              <StatusText tone="neutral">{integration.error ?? "Checking…"}</StatusText>
            )
          ) : (
            <StatusText tone="neutral">Desktop app only</StatusText>
          )
        }
      />
      <Row
        label="Last event received"
        description={
          sessions > 0
            ? `${sessions} session${sessions === 1 ? "" : "s"} visible on the island`
            : undefined
        }
        control={
          <span className="text-caption text-fg-muted">
            {last ? formatAgo(last) : "None since Islandy started"}
          </span>
        }
      />
      <Row
        label="Hooks"
        description={
          result ??
          "A backup of your settings is saved before every change. Other hooks and project settings are never touched."
        }
        control={
          <div className="flex items-center gap-2">
            {state === "installed" || state === "needsUpdate" ? (
              <>
                <Button
                  busy={busy}
                  onClick={() => {
                    run(installClaudeIntegration, "Hooks reinstalled.");
                  }}
                >
                  {state === "needsUpdate" ? "Repair" : "Reinstall"}
                </Button>
                <ConfirmButton
                  question="Remove Islandy's hooks?"
                  confirmLabel="Remove"
                  onConfirm={() => {
                    run(removeClaudeIntegration, "Hooks removed.");
                  }}
                >
                  Remove…
                </ConfirmButton>
              </>
            ) : (
              <Button
                variant="primary"
                busy={busy}
                disabled={
                  !isNativeAvailable() ||
                  !state ||
                  state === "notDetected" ||
                  state === "unreadable"
                }
                onClick={() => {
                  run(
                    installClaudeIntegration,
                    "Hooks installed. New Claude Code sessions report to Islandy.",
                  );
                }}
              >
                Install hooks
              </Button>
            )}
            <Button onClick={integration.reload}>Check again</Button>
          </div>
        }
      />
      <details className="group px-4 py-3">
        <summary className="cursor-pointer text-body font-medium text-fg">Troubleshooting</summary>
        <ul className="mt-2 flex list-disc flex-col gap-1.5 pl-5 text-caption text-fg-muted">
          <li>
            A Claude Code session that was already running may need a restart to load new hooks.
          </li>
          <li>
            Nothing arrives? Check “Local receiver” below and run <Code>islandy status</Code> in a
            terminal.
          </li>
          <li>Claude Code asks you to trust a folder before hooks run there.</li>
          <li>
            In <Code>claude -p</Code> runs, Claude Code may stop background hooks at exit, so the
            final “finished” event can be missing.
          </li>
          <li>
            “Can't be read safely”: the settings file isn't plain JSON. Islandy won't edit it; see
            docs/claude-code-integration.md for the manual snippet.
          </li>
        </ul>
      </details>
    </Section>
  );
}

/** Claude Desktop (the chat app) has no hooks; Islandy watches its window. */
function ClaudeDesktopSection() {
  return (
    <Section
      title="Claude Desktop"
      description="Nothing to install. While Claude Desktop writes a reply, Islandy shows it on the island and tells you when it's done. It only checks whether Claude's “Stop response” button is on screen, using Windows UI Automation. It never reads your chats."
    >
      <Row
        label="Code tab"
        description="Claude Code sessions in Claude Desktop use the hooks above, like the terminal. Install them once, then start a new session."
      />
      <Row
        label="Turn off"
        description="Turn off the “Claude Code” source in Notifications to stop both."
      />
    </Section>
  );
}

function DeveloperSection() {
  const activity = useNativeQuery(getActivitySnapshot, { pollMs: 3000 });
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const receiver = activity.data?.receiver;
  const last = activity.data?.lastReceived.developer ?? null;
  return (
    <Section
      title="Developer events"
      description={
        <>
          Scripts report builds, tests and dev servers with the <Code>islandy</Code> command that
          ships with the app.
        </>
      }
    >
      <Row
        label="Local receiver"
        description="Loopback only (127.0.0.1), with a per-installation token. Events can only create activity entries."
        control={
          !isNativeAvailable() ? (
            <StatusText tone="neutral">Desktop app only</StatusText>
          ) : receiver === "listening" ? (
            <StatusText tone="success">Listening</StatusText>
          ) : receiver === "unavailable" ? (
            <StatusText tone="error">Unavailable</StatusText>
          ) : (
            <StatusText tone="neutral">Starting…</StatusText>
          )
        }
      />
      <Row
        label="Send a test event"
        description={
          test
            ? test.text
            : last
              ? `Last developer event: ${formatAgo(last)}`
              : "Shows a message peek through the same path scripts use."
        }
        control={
          <Button
            busy={busy}
            disabled={!isNativeAvailable()}
            onClick={() => {
              setBusy(true);
              testLocalReceiver().then(
                () => {
                  setBusy(false);
                  setTest({ ok: true, text: "Delivered. Look at the island." });
                  activity.reload();
                },
                (cause: unknown) => {
                  setBusy(false);
                  setTest({ ok: false, text: errorText(cause) });
                },
              );
            }}
          >
            Send test
          </Button>
        }
      />
      <div className="flex flex-col gap-2 px-4 py-3">
        <span className="text-body font-medium text-fg">Examples</span>
        <pre className="overflow-x-auto rounded-[8px] bg-surface p-3 font-mono text-[12px] leading-[18px] text-fg-muted select-text">
          {`islandy notify build started --project api
islandy notify build failed --project api --label "Release build"
islandy notify tests passed --project web
islandy notify server started
islandy notify message --title "Deploy finished" --severity success
islandy status`}
        </pre>
        <p className="text-caption text-fg-subtle">
          The command is <Code>islandy.exe</Code> in Islandy's install folder. Add that folder to
          PATH, or call it by its full path. Exit codes: 0 delivered, 2 usage, 3 Islandy not
          running, 4 rejected.
        </p>
      </div>
    </Section>
  );
}

function availabilityText(
  a: Availability | undefined,
  ready: string,
): { text: string; tone: "neutral" | "success" | "warning" | "error" } {
  switch (a) {
    case "ready":
      return { text: ready, tone: "success" };
    case "unavailable":
      return { text: "Couldn't be read", tone: "warning" };
    case "unsupported":
      return { text: "Not supported", tone: "neutral" };
    default:
      return { text: isNativeAvailable() ? "Checking…" : "Desktop app only", tone: "neutral" };
  }
}

function NativeSection() {
  const media = useNativeQuery(getMediaSnapshot, { pollMs: 5000 });
  const system = useNativeQuery(getSystemSnapshot, { pollMs: 5000 });
  const s: SystemSnapshot | null = system.data;
  const mediaStatus = media.data?.status;
  const rows: {
    label: string;
    description: string;
    status: ReturnType<typeof availabilityText>;
  }[] = [
    {
      label: "Media sessions",
      description: "Windows media controls (browsers, Spotify, …).",
      status:
        mediaStatus === "ready"
          ? { text: `${media.data?.sessions.length ?? 0} active`, tone: "success" }
          : mediaStatus === "unsupported"
            ? { text: "Not supported", tone: "neutral" }
            : mediaStatus === "unavailable"
              ? { text: "Unavailable", tone: "warning" }
              : availabilityText(undefined, ""),
    },
    {
      label: "Volume",
      description: "Core Audio master volume and mute.",
      status: availabilityText(s?.audio.status, s?.audio.output ? "Available" : "No output device"),
    },
    {
      label: "Battery",
      description: "Charge and power source.",
      status:
        s?.power.status === "ready" && s.power.battery.presence === "absent"
          ? { text: "No battery (desktop)", tone: "neutral" }
          : availabilityText(s?.power.status, "Available"),
    },
    {
      label: "Network",
      description: "Windows' connectivity status.",
      status: availabilityText(s?.network.status, "Available"),
    },
    {
      label: "Wi-Fi and Bluetooth",
      description: "Switching needs Windows' radio permission; otherwise Islandy opens Settings.",
      status:
        s?.radios.status === "ready"
          ? {
              text: [
                s.radios.wifi ? "Wi-Fi" : null,
                s.radios.bluetooth ? "Bluetooth" : "No Bluetooth adapter",
              ]
                .filter(Boolean)
                .join(" · "),
              tone: "success",
            }
          : availabilityText(s?.radios.status, ""),
    },
    {
      label: "Brightness",
      description: "Built-in panels (WMI) and DDC/CI monitors.",
      status:
        s?.display.status === "ready" && s.display.displays.length === 0
          ? { text: "Unsupported on these displays", tone: "neutral" }
          : availabilityText(s?.display.status, `${s?.display.displays.length ?? 0} controllable`),
    },
  ];
  return (
    <Section
      title="Windows"
      description="What this PC offers. Unsupported features stay hidden or read-only in the dashboard."
    >
      {rows.map((r) => (
        <Row
          key={r.label}
          label={r.label}
          description={r.description}
          control={<StatusText tone={r.status.tone}>{r.status.text}</StatusText>}
        />
      ))}
    </Section>
  );
}

export function IntegrationsPage() {
  return (
    <div className="flex flex-col gap-7">
      <PageHeader
        title="Integrations"
        description="Everything Islandy connects to, and whether it works on this PC. Nothing here leaves your device."
      />
      <ClaudeSection />
      <ClaudeDesktopSection />
      <DeveloperSection />
      <NativeSection />
    </div>
  );
}
