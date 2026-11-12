import { Laptop, Monitor } from "lucide-react";

import { useLevel, useSystem } from "../../features/system/system-store";
import type { DisplayInfo, DisplayState } from "../../services/types";
import { LevelSlider } from "../ui/LevelSlider";
import { DomainMessage, SettingsLink, SystemNoticeLine, Tile } from "./shared";
import { useDomain } from "../../features/system/use-domain";

function DisplayRow({ display }: { display: DisplayInfo }) {
  const level = useLevel(`brightness:${display.id}`);
  const setBrightness = useSystem((s) => s.setBrightness);
  const Icon = display.kind === "internal" ? Laptop : Monitor;
  return (
    <li className="flex flex-col gap-0.5">
      <div className="flex items-center gap-2 text-body text-fg">
        <Icon size={14} strokeWidth={1.75} aria-hidden="true" className="shrink-0 text-fg-muted" />
        <span title={display.name} className="min-w-0 flex-1 truncate">
          {display.name}
        </span>
        <span className="text-caption text-fg-muted tabular-nums">
          {display.brightness === null ? "Not responding" : `${level ?? display.brightness}%`}
        </span>
      </div>
      <LevelSlider
        value={display.brightness === null ? null : level}
        onChange={(v) => {
          setBrightness(display.id, v);
        }}
        label={`Brightness, ${display.name}`}
      />
    </li>
  );
}

function unsupportedNote(display: DisplayState): string | null {
  const n = display.unsupportedCount;
  if (n === 0) return null;
  return n === 1
    ? "1 display doesn't support brightness control here."
    : `${n} displays don't support brightness control here.`;
}

/** Brightness for displays that support it; Settings for the rest. */
export function DisplayPanel() {
  const view = useDomain("display");
  return (
    <div className="flex h-full gap-3">
      <Tile title="Brightness" className="flex-[1.4]">
        {view.kind === "ready" ? (
          view.state.displays.length > 0 ? (
            <ul aria-label="Displays" className="flex min-h-0 flex-col gap-2 overflow-y-auto pr-1">
              {view.state.displays.map((d) => (
                <DisplayRow key={d.id} display={d} />
              ))}
            </ul>
          ) : (
            <div className="flex flex-col gap-1">
              <p className="text-title font-semibold tracking-tight text-fg">Not supported</p>
              <p className="text-body text-fg-muted">
                No connected display offers brightness control to apps. Use the monitor&apos;s own
                buttons or Display settings.
              </p>
            </div>
          )
        ) : (
          <DomainMessage
            view={view}
            messages={{ unavailable: "Couldn't reach the displays. Trying again when reopened." }}
          />
        )}
        <SystemNoticeLine className="mt-auto" />
      </Tile>
      <Tile title="Displays" className="flex-1">
        <div className="flex min-h-0 flex-1 flex-col justify-between gap-2">
          <p className="text-body text-fg-muted">
            {(view.kind === "ready" ? unsupportedNote(view.state) : null) ??
              "Built-in screens and monitors with DDC/CI can be adjusted here."}
          </p>
          <SettingsLink page="display">Display settings</SettingsLink>
        </div>
      </Tile>
    </div>
  );
}
