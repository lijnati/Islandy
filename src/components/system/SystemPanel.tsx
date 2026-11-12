import { batteryPercent, formatBytes, powerDescription } from "../../features/system/system-model";
import { useSystem } from "../../features/system/system-store";
import type { MetricsState, PowerState } from "../../services/types";
import { MeterBar } from "../ui/MeterBar";
import { DomainMessage, SettingsLink, Tile } from "./shared";
import { useDomain } from "../../features/system/use-domain";

function BigNumber({ value, unit = "%" }: { value: number | null; unit?: string }) {
  return (
    <p className="flex items-baseline gap-0.5 text-display font-semibold tracking-display text-fg tabular-nums">
      {value === null ? <span className="text-fg-subtle">–</span> : value}
      <span className="text-title font-medium text-fg-muted">{unit}</span>
    </p>
  );
}

function liveNote(metrics: MetricsState): string {
  return metrics.intervalMs === null
    ? "Paused"
    : `Updates every ${Math.round(metrics.intervalMs / 1000)} s`;
}

function CpuTile() {
  const view = useDomain("metrics");
  const metrics = view.kind === "ready" ? view.state : null;
  return (
    <Tile title="CPU" className="flex-1">
      {metrics || view.kind === "ready" ? (
        <div className="flex flex-1 flex-col justify-end gap-2">
          <BigNumber value={metrics?.cpuPercent ?? null} />
          <MeterBar value={metrics?.cpuPercent ?? null} label="CPU usage" />
          <p className="truncate text-caption text-fg-muted">
            {metrics?.cpuPercent === null ? "Measuring…" : metrics ? liveNote(metrics) : ""}
          </p>
        </div>
      ) : (
        <DomainMessage view={view} />
      )}
    </Tile>
  );
}

function MemoryTile() {
  const view = useDomain("metrics");
  const memory = view.kind === "ready" ? view.state.memory : null;
  return (
    <Tile title="Memory" className="flex-1">
      {view.kind === "ready" ? (
        <div className="flex flex-1 flex-col justify-end gap-2">
          <BigNumber value={memory?.usedPercent ?? null} />
          <MeterBar value={memory?.usedPercent ?? null} label="Memory in use" />
          <p className="truncate text-caption text-fg-muted">
            {memory
              ? `${formatBytes(memory.totalBytes - memory.availableBytes)} of ${formatBytes(memory.totalBytes)} in use`
              : "Measuring…"}
          </p>
        </div>
      ) : (
        <DomainMessage view={view} />
      )}
    </Tile>
  );
}

function batteryTone(power: PowerState): "neutral" | "attention" | "success" {
  const percent = batteryPercent(power.battery);
  if (power.battery.presence === "present" && power.battery.state === "charging") return "success";
  if (percent !== null && percent <= 20 && power.source === "battery") return "attention";
  return "neutral";
}

function BatteryTile() {
  const view = useDomain("power");
  return (
    <Tile title="Battery" className="flex-1">
      {view.kind === "ready" ? (
        view.state.battery.presence === "present" ? (
          <div className="flex flex-1 flex-col justify-end gap-2">
            <BigNumber value={view.state.battery.percent} />
            <MeterBar
              value={view.state.battery.percent}
              label="Battery level"
              tone={batteryTone(view.state)}
            />
            <p className="truncate text-caption text-fg-muted">{powerDescription(view.state)}</p>
          </div>
        ) : (
          <div className="flex flex-1 flex-col justify-end gap-2">
            {/* Never a fake "100%": desktops simply have no battery. */}
            <p className="text-title font-semibold tracking-tight text-fg">
              {view.state.battery.presence === "absent" ? "No battery" : "Unknown"}
            </p>
            <p className="text-caption text-fg-muted">{powerDescription(view.state)}</p>
          </div>
        )
      ) : (
        <DomainMessage view={view} />
      )}
    </Tile>
  );
}

/** The system overview: CPU, memory and battery at a glance. */
export function SystemPanel() {
  const hasBattery = useSystem((s) => s.snapshot?.power.battery.presence === "present");
  return (
    <div className="flex h-full flex-col gap-2">
      <div className="flex min-h-0 flex-1 gap-3">
        <CpuTile />
        <MemoryTile />
        <BatteryTile />
      </div>
      {hasBattery && (
        <div className="flex justify-end">
          <SettingsLink page="battery">Power &amp; battery settings</SettingsLink>
        </div>
      )}
    </div>
  );
}
