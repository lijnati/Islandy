import { ArrowDown, ArrowUp } from "lucide-react";

import { IconButton } from "../../../../components/ui/IconButton";
import { Switch } from "../../../../components/ui/Switch";
import { cx } from "../../../../components/ui/cx";
import { designLayouts } from "../../../../design/tokens";
import { DEFAULT_SETTINGS } from "../../defaults";
import { moveWidget } from "../../settings-schema";
import { useSettings, useSettingsStore } from "../../settings-store";
import { fitWidgets, WIDGETS } from "../../../widgets/registry";
import { Button, PageHeader, Section } from "../controls";

export function WidgetsPage() {
  const items = useSettings((s) => s.widgets.items);
  const update = useSettingsStore((s) => s.update);
  const shown = new Set(fitWidgets(items, designLayouts.expanded.width));
  const setItems = (next: typeof items) => void update((s) => ({ ...s, widgets: { items: next } }));
  const isDefault = JSON.stringify(items) === JSON.stringify(DEFAULT_SETTINGS.widgets.items);

  return (
    <div className="flex flex-col gap-7">
      <PageHeader
        title="Widgets"
        description="Choose the dashboard's widgets and their order. Islandy shows the turned-on widgets that fit the dashboard's width, left to right."
      />
      <Section
        title="Dashboard"
        footer={
          <span className="flex items-center justify-between gap-4">
            <span>
              Widgets marked “Doesn't fit” are on, but there's no room for them next to the ones
              before.
            </span>
            <Button
              disabled={isDefault}
              onClick={() => {
                setItems(DEFAULT_SETTINGS.widgets.items);
              }}
            >
              Restore default layout
            </Button>
          </span>
        }
      >
        <ol aria-label="Dashboard widgets in order">
          {items.map((item, index) => {
            const def = WIDGETS[item.id];
            const fits = shown.has(item.id);
            return (
              <li
                key={item.id}
                className={cx(
                  "flex min-h-[60px] items-center gap-3 px-4 py-2.5",
                  index > 0 && "border-t border-hairline",
                )}
              >
                <span
                  aria-hidden="true"
                  className="w-4 text-caption font-semibold text-fg-subtle tabular-nums"
                >
                  {index + 1}
                </span>
                <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="flex items-center gap-2">
                    <span className="text-body font-medium text-fg">{def.name}</span>
                    {item.enabled && !fits && (
                      <span className="rounded-full border border-hairline px-1.5 text-[10px] leading-[16px] text-fg-muted">
                        Doesn't fit
                      </span>
                    )}
                  </span>
                  <span className="truncate text-caption text-fg-muted">{def.description}</span>
                </div>
                <IconButton
                  icon={ArrowUp}
                  size="sm"
                  label={`Move ${def.name} up`}
                  disabled={index === 0}
                  onClick={() => {
                    setItems(moveWidget(items, index, index - 1));
                  }}
                />
                <IconButton
                  icon={ArrowDown}
                  size="sm"
                  label={`Move ${def.name} down`}
                  disabled={index === items.length - 1}
                  onClick={() => {
                    setItems(moveWidget(items, index, index + 1));
                  }}
                />
                <Switch
                  label={`Show ${def.name}`}
                  checked={item.enabled}
                  onChange={(on) => {
                    setItems(items.map((e) => (e.id === item.id ? { ...e, enabled: on } : e)));
                  }}
                />
              </li>
            );
          })}
        </ol>
      </Section>
    </div>
  );
}
