import { Check } from "lucide-react";

import { cx } from "../../../../components/ui/cx";
import { focusRing } from "../../../../components/ui/IconButton";
import type { Accent, Theme } from "../../../../services/types";
import { RANGES } from "../../settings-schema";
import { useSettings, useSettingsStore } from "../../settings-store";
import { PageHeader, RangeField, Row, Section, Segmented } from "../controls";

const THEMES: { id: Theme; name: string; note: string; body: string; tile: string }[] = [
  { id: "midnight", name: "Midnight", note: "Near-black", body: "#080808", tile: "#171717" },
  { id: "graphite", name: "Graphite", note: "Charcoal", body: "#1b1b1d", tile: "#2a2a2d" },
  {
    id: "frost",
    name: "Frost",
    note: "Translucent",
    body: "rgb(30 32 38 / 0.8)",
    tile: "rgb(255 255 255 / 0.09)",
  },
];

const ACCENTS: { id: Accent; name: string; color: string }[] = [
  { id: "white", name: "White", color: "#f5f5f5" },
  { id: "blue", name: "Blue", color: "#6aa8ff" },
  { id: "violet", name: "Violet", color: "#ad95ff" },
  { id: "green", name: "Green", color: "#4fd6a8" },
  { id: "amber", name: "Amber", color: "#f5c56b" },
  { id: "coral", name: "Coral", color: "#ff9479" },
];

function ThemePicker({ value, onChange }: { value: Theme; onChange: (t: Theme) => void }) {
  return (
    <div role="radiogroup" aria-label="Theme" className="grid grid-cols-3 gap-3">
      {THEMES.map((t) => {
        const selected = t.id === value;
        return (
          <button
            key={t.id}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => {
              onChange(t.id);
            }}
            className={cx(
              "flex flex-col gap-2 rounded-[10px] border p-2.5 text-left transition-colors duration-fast",
              focusRing,
              selected ? "border-accent" : "border-hairline hover:border-hairline-strong",
            )}
          >
            <span
              aria-hidden="true"
              className="flex h-16 items-start justify-center rounded-[7px] bg-[linear-gradient(135deg,#3a3f4a,#1d2129_60%,#2b2533)] p-2"
            >
              <span
                className="flex h-7 w-24 items-center justify-between rounded-full border border-white/10 px-2.5"
                style={{ background: t.body }}
              >
                <span className="size-2 rounded-full" style={{ background: t.tile }} />
                <span className="text-[9px] font-semibold text-white">9:41</span>
              </span>
            </span>
            <span className="flex items-center justify-between">
              <span className="flex flex-col">
                <span className="text-body font-medium text-fg">{t.name}</span>
                <span className="text-caption text-fg-muted">{t.note}</span>
              </span>
              {selected && (
                <Check size={14} strokeWidth={2.5} aria-hidden="true" className="text-accent" />
              )}
            </span>
          </button>
        );
      })}
    </div>
  );
}

function AccentPicker({ value, onChange }: { value: Accent; onChange: (a: Accent) => void }) {
  return (
    <div role="radiogroup" aria-label="Accent color" className="flex items-center gap-2">
      {ACCENTS.map((a) => {
        const selected = a.id === value;
        return (
          <button
            key={a.id}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={a.name}
            title={a.name}
            onClick={() => {
              onChange(a.id);
            }}
            className={cx(
              "flex size-7 items-center justify-center rounded-full border-2 transition-transform duration-fast",
              focusRing,
              selected ? "border-fg" : "border-transparent hover:scale-110",
            )}
          >
            <span className="size-5 rounded-full" style={{ background: a.color }} />
          </button>
        );
      })}
    </div>
  );
}

export function AppearancePage() {
  const a = useSettings((s) => s.appearance);
  const update = useSettingsStore((s) => s.update);
  const set = <K extends keyof typeof a>(key: K, value: (typeof a)[K]) =>
    void update((s) => ({ ...s, appearance: { ...s.appearance, [key]: value } }));

  return (
    <div className="flex flex-col gap-7">
      <PageHeader
        title="Appearance"
        description="Changes apply to the island immediately. Status colors (success, waiting, problems) stay the same in every theme."
      />
      <Section title="Theme">
        <div className="p-3">
          <ThemePicker
            value={a.theme}
            onChange={(t) => {
              set("theme", t);
            }}
          />
        </div>
      </Section>
      <Section
        title="Color and surface"
        footer="Frost uses translucent surfaces; the desktop behind the island is not blurred."
      >
        <Row
          label="Accent"
          description="Switches, sliders, selection and focus."
          control={
            <AccentPicker
              value={a.accent}
              onChange={(v) => {
                set("accent", v);
              }}
            />
          }
        />
        <Row
          label="Background opacity"
          description="Lower values let the desktop show through slightly."
          control={
            <RangeField
              id="opacity"
              label="Background opacity"
              value={a.opacity}
              {...RANGES.opacity}
              format={(v) => `${v}%`}
              onCommit={(v) => {
                set("opacity", v);
              }}
            />
          }
        />
        <Row
          label="Corners"
          control={
            <Segmented
              label="Corner style"
              value={a.corners}
              onChange={(v) => {
                set("corners", v);
              }}
              options={[
                { value: "compact", label: "Compact" },
                { value: "standard", label: "Standard" },
                { value: "round", label: "Round" },
              ]}
            />
          }
        />
      </Section>
      <Section title="Motion">
        <Row
          label="Animation"
          description="Subtle uses shorter movements without overshoot."
          control={
            <Segmented
              label="Animation intensity"
              value={a.animation}
              onChange={(v) => {
                set("animation", v);
              }}
              options={[
                { value: "full", label: "Full" },
                { value: "subtle", label: "Subtle" },
              ]}
            />
          }
        />
        <Row
          label="Reduce motion"
          description="“System” follows Windows › Accessibility › Visual effects › Animation effects."
          control={
            <Segmented
              label="Reduce motion"
              value={a.reducedMotion}
              onChange={(v) => {
                set("reducedMotion", v);
              }}
              options={[
                { value: "system", label: "System" },
                { value: "on", label: "On" },
                { value: "off", label: "Off" },
              ]}
            />
          }
        />
      </Section>
    </div>
  );
}
