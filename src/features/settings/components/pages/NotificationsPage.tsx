import { Switch } from "../../../../components/ui/Switch";
import { isNativeAvailable } from "../../../../services/native";
import type {
  ActivitySettings,
  PeekDuration,
  PeekToggles,
  SourceToggles,
} from "../../../../services/types";
import { DEFAULT_ACTIVITY_SETTINGS } from "../../defaults";
import { useActivitySettings } from "../../use-activity-settings";
import { PageHeader, Row, Section, Segmented } from "../controls";

export function NotificationsPage() {
  const { settings, update, ready } = useActivitySettings();
  const s: ActivitySettings = settings ?? DEFAULT_ACTIVITY_SETTINGS;
  const disabled = !ready;
  const peek = (key: keyof PeekToggles, on: boolean) => {
    update({ ...s, peeks: { ...s.peeks, [key]: on } });
  };
  const source = (key: keyof SourceToggles, on: boolean) => {
    update({ ...s, sources: { ...s.sources, [key]: on } });
  };

  return (
    <div className="flex flex-col gap-7">
      <PageHeader
        title="Notifications"
        description="Peeks are the brief capsules the island shows on its own. Everything here is Islandy's; Windows notification settings and Do Not Disturb are never changed."
      />
      {!isNativeAvailable() && (
        <p className="text-caption text-fg-subtle">
          Shown with default values: notification settings live in the desktop app.
        </p>
      )}

      <Section title="Quiet mode">
        <Row
          label="Quiet mode"
          description="No automatic peeks. Events are still recorded, the activity list stays up to date, and requests for your attention keep their amber marker on the pill."
          control={
            <Switch
              label="Quiet mode"
              checked={s.quiet}
              disabled={disabled}
              onChange={(on) => {
                update({ ...s, quiet: on });
              }}
            />
          }
        />
      </Section>

      <Section
        title="Peek for"
        footer="Each source can peek at most 4 times a minute; further events are recorded silently. Requests for your attention are never limited."
      >
        <Row
          label="Claude Code"
          description="Finished turns, errors and requests for your attention."
          control={
            <Switch
              label="Peek for Claude Code"
              checked={s.peeks.claudeCode}
              disabled={disabled}
              onChange={(on) => {
                peek("claudeCode", on);
              }}
            />
          }
        />
        <Row
          label="Builds, tests and scripts"
          description="Events from islandy notify."
          control={
            <Switch
              label="Peek for builds, tests and scripts"
              checked={s.peeks.developer}
              disabled={disabled}
              onChange={(on) => {
                peek("developer", on);
              }}
            />
          }
        />
        <Row
          label="System alerts"
          description="Low battery, network lost or restored, power connected."
          control={
            <Switch
              label="Peek for system alerts"
              checked={s.peeks.system}
              disabled={disabled}
              onChange={(on) => {
                peek("system", on);
              }}
            />
          }
        />
        <Row
          label="Track changes"
          description="When a new song or video starts playing."
          control={
            <Switch
              label="Peek for track changes"
              checked={s.peeks.media}
              disabled={disabled}
              onChange={(on) => {
                peek("media", on);
              }}
            />
          }
        />
      </Section>

      <Section title="Outcomes">
        <Row
          label="Successes"
          description="Build passed, tests passed, Claude finished."
          control={
            <Switch
              label="Peek for successes"
              checked={s.peeks.success}
              disabled={disabled}
              onChange={(on) => {
                peek("success", on);
              }}
            />
          }
        />
        <Row
          label="Errors and warnings"
          description="Failures, API errors, low battery."
          control={
            <Switch
              label="Peek for errors and warnings"
              checked={s.peeks.errors}
              disabled={disabled}
              onChange={(on) => {
                peek("errors", on);
              }}
            />
          }
        />
        <Row
          label="Peek duration"
          description="Standard: 3–4 s for information, 6 s for problems, 8 s for attention requests."
          control={
            <Segmented<PeekDuration>
              label="Peek duration"
              value={s.peekDuration}
              disabled={disabled}
              onChange={(v) => {
                update({ ...s, peekDuration: v });
              }}
              options={[
                { value: "short", label: "Short" },
                { value: "standard", label: "Standard" },
                { value: "long", label: "Long" },
              ]}
            />
          }
        />
      </Section>

      <Section
        title="Record activity from"
        description="Turning a source off stops recording it entirely (no history, no indicator)."
      >
        <Row
          label="Claude Code"
          control={
            <Switch
              label="Record Claude Code"
              checked={s.sources.claudeCode}
              disabled={disabled}
              onChange={(on) => {
                source("claudeCode", on);
              }}
            />
          }
        />
        <Row
          label="Builds, tests and scripts"
          control={
            <Switch
              label="Record builds, tests and scripts"
              checked={s.sources.developer}
              disabled={disabled}
              onChange={(on) => {
                source("developer", on);
              }}
            />
          }
        />
        <Row
          label="System alerts"
          control={
            <Switch
              label="Record system alerts"
              checked={s.sources.system}
              disabled={disabled}
              onChange={(on) => {
                source("system", on);
              }}
            />
          }
        />
      </Section>
    </div>
  );
}
