import { Switch } from "../../../../components/ui/Switch";
import type {
  Anchor,
  InteractionSettings,
  IslandSettings,
  MonitorInfo,
  SizePreset,
} from "../../../../services/types";
import { listMonitors } from "../../settings-service";
import { RANGES } from "../../settings-schema";
import { useSettings, useSettingsStore } from "../../settings-store";
import { useNativeQuery } from "../../use-native-query";
import { Button, PageHeader, RangeField, Row, Section, Segmented, SelectField } from "../controls";

const SIZE_NAME: Record<SizePreset, string> = {
  compact: "Compact",
  standard: "Standard",
  comfortable: "Comfortable",
};

const SIZE_LABEL: Record<SizePreset, string> = {
  compact: "Compact · 200 × 40",
  standard: "Standard · 220 × 44",
  comfortable: "Comfortable · 260 × 48",
};

export function IslandPage() {
  const island = useSettings((s) => s.island);
  const interaction = useSettings((s) => s.interaction);
  const update = useSettingsStore((s) => s.update);
  const monitors = useNativeQuery(listMonitors, { pollMs: 5000 });

  const setIsland = <K extends keyof IslandSettings>(key: K, value: IslandSettings[K]) =>
    void update((s) => ({ ...s, island: { ...s.island, [key]: value } }));
  const setInteraction = <K extends keyof InteractionSettings>(
    key: K,
    value: InteractionSettings[K],
  ) => void update((s) => ({ ...s, interaction: { ...s.interaction, [key]: value } }));

  const list: MonitorInfo[] = monitors.data ?? [];
  const selectedMissing =
    island.monitor !== "primary" && !list.some((m) => m.id === island.monitor);
  const monitorOptions = [
    { value: "primary", label: "Primary display" },
    ...list.map((m) => ({ value: m.id, label: `${m.label}${m.primary ? " (primary)" : ""}` })),
    ...(selectedMissing
      ? [{ value: island.monitor, label: "Disconnected display (using primary)" }]
      : []),
  ];
  // Never let both ways of opening with the pointer be switched off.
  const onlyHover = interaction.openOnHover && !interaction.openOnClick;
  const onlyClick = interaction.openOnClick && !interaction.openOnHover;

  return (
    <div className="flex flex-col gap-7">
      <PageHeader
        title="Island"
        description="Where the island sits, how big it is and how it responds to the pointer. Changes apply immediately."
      />

      <Section
        title="Position"
        footer="Positions are kept inside the monitor's visible area. If the selected display is disconnected, the island moves to the primary display and returns when it's back."
      >
        <Row
          label="Display"
          htmlFor="monitor"
          control={
            <SelectField
              id="monitor"
              label="Display"
              value={island.monitor}
              options={monitorOptions}
              onChange={(v) => {
                setIsland("monitor", v);
              }}
            />
          }
        />
        <Row
          label="Alignment"
          control={
            <Segmented<Anchor>
              label="Alignment"
              value={island.anchor}
              onChange={(v) => {
                setIsland("anchor", v);
              }}
              options={[
                { value: "topLeft", label: "Top left" },
                { value: "topCenter", label: "Top center" },
                { value: "topRight", label: "Top right" },
              ]}
            />
          }
        />
        <Row
          label="Horizontal offset"
          description="Moves the island left or right of its alignment."
          control={
            <div className="flex items-center gap-2">
              <RangeField
                id="offset"
                label="Horizontal offset"
                value={Math.max(-600, Math.min(600, island.offsetX))}
                min={-600}
                max={600}
                step={10}
                format={(v) => `${v > 0 ? "+" : ""}${v} px`}
                onCommit={(v) => {
                  setIsland("offsetX", v);
                }}
              />
              <Button
                disabled={island.offsetX === 0}
                onClick={() => {
                  setIsland("offsetX", 0);
                }}
              >
                Reset
              </Button>
            </div>
          }
        />
        <Row
          label="Distance from top"
          control={
            <RangeField
              id="top"
              label="Distance from top"
              value={island.topMargin}
              {...RANGES.topMargin}
              format={(v) => `${v} px`}
              onCommit={(v) => {
                setIsland("topMargin", v);
              }}
            />
          }
        />
        <Row
          label="Alt + drag to move"
          description="Hold Alt and drag the collapsed island sideways. Plain clicks still open the dashboard."
          control={
            <Switch
              label="Alt + drag to move"
              checked={island.dragToReposition}
              onChange={(on) => {
                setIsland("dragToReposition", on);
              }}
            />
          }
        />
      </Section>

      <Section
        title="Size"
        footer="Logical pixels; Windows display scaling is applied on top. The dashboard keeps its size and fits smaller monitors."
      >
        <Row
          label="Island size"
          control={
            <Segmented<SizePreset>
              label="Island size"
              value={island.size}
              onChange={(v) => {
                setIsland("size", v);
              }}
              options={(["compact", "standard", "comfortable"] as const).map((v) => ({
                value: v,
                label: SIZE_NAME[v],
                title: SIZE_LABEL[v],
              }))}
            />
          }
        />
      </Section>

      <Section
        title="Pointer"
        footer="At least one way of opening with the pointer stays on. The shortcut and the tray icon always work."
      >
        <Row
          label="Open on hover"
          description={onlyHover ? "Turn on “Open on click” first to turn this off." : undefined}
          control={
            <Switch
              label="Open on hover"
              checked={interaction.openOnHover}
              disabled={onlyHover}
              onChange={(on) => {
                setInteraction("openOnHover", on);
              }}
            />
          }
        />
        <Row
          label="Hover delay"
          disabled={!interaction.openOnHover}
          control={
            <RangeField
              id="hover"
              label="Hover delay"
              value={interaction.hoverDelayMs}
              {...RANGES.hoverDelayMs}
              disabled={!interaction.openOnHover}
              format={(v) => `${v} ms`}
              onCommit={(v) => {
                setInteraction("hoverDelayMs", v);
              }}
            />
          }
        />
        <Row
          label="Open on click"
          description={onlyClick ? "Turn on “Open on hover” first to turn this off." : undefined}
          control={
            <Switch
              label="Open on click"
              checked={interaction.openOnClick}
              disabled={onlyClick}
              onChange={(on) => {
                setInteraction("openOnClick", on);
              }}
            />
          }
        />
        <Row
          label="Pin when clicked"
          description="A clicked dashboard stays open until Esc, a click elsewhere or the shortcut."
          disabled={!interaction.openOnClick}
          control={
            <Switch
              label="Pin when clicked"
              checked={interaction.pinOnClick}
              disabled={!interaction.openOnClick}
              onChange={(on) => {
                setInteraction("pinOnClick", on);
              }}
            />
          }
        />
        <Row
          label="Collapse delay"
          description="How long an unpinned dashboard waits after the pointer leaves."
          control={
            <RangeField
              id="collapse"
              label="Collapse delay"
              value={interaction.collapseDelayMs}
              {...RANGES.collapseDelayMs}
              format={(v) => `${v} ms`}
              onCommit={(v) => {
                setInteraction("collapseDelayMs", v);
              }}
            />
          }
        />
        <Row
          label="Auto-collapse pinned dashboard"
          description="Collapse a pinned dashboard after 15 seconds without the pointer on it."
          control={
            <Switch
              label="Auto-collapse pinned dashboard"
              checked={interaction.autoCollapse}
              onChange={(on) => {
                setInteraction("autoCollapse", on);
              }}
            />
          }
        />
      </Section>
    </div>
  );
}
