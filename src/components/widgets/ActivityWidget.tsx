import { DEMO_ACTIVITIES } from "../../features/activity/demo-activity";
import { dispatchIsland } from "../../features/island/island-store";
import { useNow } from "../../hooks/useNow";
import { WidgetSurface } from "../ui/WidgetSurface";
import { ActivityCard } from "./ActivityCard";

/** DEMO activity widget. Real activity (e.g. Claude Code hooks) is Phase 4. */
export function ActivityWidget({ className }: { className?: string | undefined }) {
  const now = useNow();
  const [primary, ...rest] = DEMO_ACTIVITIES;
  return (
    <WidgetSurface
      title="Activity"
      demo
      className={className}
      onOpen={() => {
        dispatchIsland({ type: "OPEN_WIDGET", widget: "activity" });
      }}
    >
      {primary && <ActivityCard activity={primary} now={now} />}
      <ul className="mt-auto flex flex-col gap-1.5 border-t border-hairline pt-2">
        {rest.map((a) => (
          <li key={a.id}>
            <ActivityCard activity={a} now={now} variant="compact" />
          </li>
        ))}
      </ul>
    </WidgetSurface>
  );
}

export function ActivityDetail() {
  const now = useNow();
  return (
    <ul
      className="grid h-full auto-cols-fr grid-flow-col gap-3"
      aria-label="Recent activity (demo)"
    >
      {DEMO_ACTIVITIES.map((a) => (
        <li
          key={a.id}
          className="flex min-w-0 flex-col justify-between rounded-tile border border-hairline bg-surface-secondary p-3 shadow-surface"
        >
          <ActivityCard activity={a} now={now} />
          <p className="text-caption text-fg-subtle">
            Demo data · live activity arrives in Phase 4
          </p>
        </li>
      ))}
    </ul>
  );
}
