import { useEffect, useRef } from "react";

import type { SettingsPageId } from "../../../../services/types";
import { useSettings, useSettingsStore } from "../../settings-store";
import { displayShortcut } from "../../shortcut-keys";
import { Button, Code, Row, Section } from "../controls";

/**
 * First-run welcome: three pointers into the rest of Settings. It is a page
 * of the settings window (`#welcome`, not in the sidebar), opened by the app
 * until it is finished or skipped, and reopened from Settings › About.
 */
export function WelcomePage({ onNavigate }: { onNavigate: (page: SettingsPageId) => void }) {
  const shortcut = useSettings((s) => s.general.toggleShortcut);
  const update = useSettingsStore((s) => s.update);
  const heading = useRef<HTMLHeadingElement>(null);

  // Move keyboard/screen-reader focus to the page title when it appears.
  useEffect(() => {
    heading.current?.focus({ preventScroll: true });
  }, []);

  const finish = () => {
    void update((s) => ({ ...s, onboarding: { ...s.onboarding, welcomed: true } }));
    onNavigate("general");
  };

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-1">
        <h1
          ref={heading}
          tabIndex={-1}
          className="text-heading font-semibold tracking-tight text-fg outline-none"
        >
          Welcome to Islandy
        </h1>
        <p className="max-w-[560px] text-body text-fg-muted">
          Your island is at the top of the screen. Three quick things make it yours. Each takes a
          minute and you can do any of them later from Settings.
        </p>
      </header>

      <Section title="Get set up">
        <Row
          label="Connect Claude Code"
          description="Install the Claude Code hooks so the island shows your sessions and asks when Claude needs you."
          control={
            <Button
              variant="primary"
              onClick={() => {
                onNavigate("integrations");
              }}
            >
              Open Integrations
            </Button>
          }
        />
        <Row
          label="Choose a place for the weather"
          description="Pick a city to turn on the weather widget. Until you do, Islandy makes no weather requests."
          control={
            <Button
              onClick={() => {
                onNavigate("widgets");
              }}
            >
              Open Widgets
            </Button>
          }
        />
        <Row
          label="Open the island from the keyboard"
          description={
            <>
              Press <Code>{displayShortcut(shortcut)}</Code> to show or hide the island from
              anywhere.
            </>
          }
          control={
            <Button
              onClick={() => {
                onNavigate("general");
              }}
            >
              Change shortcut
            </Button>
          }
        />
      </Section>

      <div className="flex justify-end">
        <Button variant="primary" onClick={finish}>
          Done
        </Button>
      </div>
    </div>
  );
}
