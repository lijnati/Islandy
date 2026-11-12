import {
  getShortcutStatus,
  getStartupStatus,
  setIslandVisible,
  setLaunchAtLogin,
} from "../../settings-service";
import { isNativeAvailable } from "../../../../services/native";
import { Switch } from "../../../../components/ui/Switch";
import { useSettings, useSettingsStore } from "../../settings-store";
import { useNativeQuery } from "../../use-native-query";
import { Button, ConfirmButton, PageHeader, Row, Section } from "../controls";
import { ShortcutRecorder } from "../ShortcutRecorder";
import { useState } from "react";

export function GeneralPage() {
  const general = useSettings((s) => s.general);
  const update = useSettingsStore((s) => s.update);
  const reset = useSettingsStore((s) => s.reset);
  const startup = useNativeQuery(getStartupStatus);
  const shortcut = useNativeQuery(getShortcutStatus);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [startupBusy, setStartupBusy] = useState(false);
  const native = isNativeAvailable();

  const toggleStartup = (on: boolean) => {
    setStartupBusy(true);
    setLaunchAtLogin(on).then(
      () => {
        setStartupBusy(false);
        setStartupError(null);
        startup.reload();
      },
      (cause: unknown) => {
        setStartupBusy(false);
        setStartupError(
          cause instanceof Error ? cause.message.replace(/^[a-z_]+: /, "") : String(cause),
        );
      },
    );
  };

  return (
    <div className="flex flex-col gap-7">
      <PageHeader
        title="General"
        description="Startup, the keyboard shortcut and how Islandy behaves around other apps."
      />

      <Section title="Startup">
        <Row
          label="Launch at startup"
          description={
            startupError ??
            (startup.data?.disabledBySystem
              ? "Turned off in Task Manager › Startup apps. Turn it on here to start Islandy at sign-in again."
              : "Start Islandy when you sign in to Windows. Per-user, no administrator rights.")
          }
          control={
            <Switch
              label="Launch at startup"
              checked={startup.data?.enabled ?? false}
              disabled={!native || !startup.data?.supported}
              busy={startupBusy}
              onChange={toggleStartup}
            />
          }
        />
        <Row
          label="Start hidden"
          description="When Windows starts Islandy, keep the island hidden until you open it from the tray or with the shortcut."
          control={
            <Switch
              label="Start hidden at sign-in"
              checked={general.startHiddenAtLogin}
              onChange={(on) =>
                void update((s) => ({ ...s, general: { ...s.general, startHiddenAtLogin: on } }))
              }
            />
          }
        />
      </Section>

      <Section
        title="Keyboard"
        footer="The tray icon always opens Islandy, even without a working shortcut."
      >
        <Row
          label="Show or hide the dashboard"
          description="A global shortcut. Combinations used by Windows or another app are refused, never taken over."
          control={
            <ShortcutRecorder
              accelerator={shortcut.data?.accelerator ?? general.toggleShortcut}
              registered={shortcut.data?.registered ?? true}
              registrationError={shortcut.data?.error ?? null}
            />
          }
        />
      </Section>

      <Section title="Behavior">
        <Row
          label="Hide during fullscreen apps"
          description="Hide the island while a game, video or presentation fills its monitor. Maximized windows don't count."
          control={
            <Switch
              label="Hide during fullscreen apps"
              checked={general.hideInFullscreen}
              onChange={(on) =>
                void update((s) => ({ ...s, general: { ...s.general, hideInFullscreen: on } }))
              }
            />
          }
        />
        <Row
          label="Hide the island"
          description="Islandy keeps running in the notification area. Show it again from the tray icon or with the shortcut."
          control={
            <Button disabled={!native} onClick={() => void setIslandVisible(false)}>
              Hide island
            </Button>
          }
        />
      </Section>

      <Section title="Reset">
        <Row
          label="Reset all settings"
          description="Restores appearance, position, widgets, interaction and notification settings. History, hooks and startup are kept."
          control={
            <ConfirmButton
              question="Reset everything?"
              confirmLabel="Reset"
              onConfirm={() => void reset()}
            >
              Reset…
            </ConfirmButton>
          }
        />
      </Section>
    </div>
  );
}
