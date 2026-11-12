import { getAppInfo } from "../../settings-service";
import { useNativeQuery } from "../../use-native-query";
import { IslandyMark } from "../IslandyMark";
import { PageHeader, Row, Section } from "../controls";

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

      <Section title="Updates">
        <Row
          label="Automatic updates are off"
          description="This release has no update server or signing key, so Islandy never downloads anything. Install a newer version by running its installer; your settings and history are kept."
        />
      </Section>

      <p className="px-1 text-caption text-fg-subtle">
        Islandy is an independent project inspired by the Dynamic Island idea. It is not affiliated
        with or endorsed by Apple Inc. or Microsoft.
      </p>
    </div>
  );
}
