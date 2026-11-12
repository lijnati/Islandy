import { Check, Speaker } from "lucide-react";

import { useLevel, useSystem } from "../../features/system/system-store";
import type { AudioState } from "../../services/types";
import { IconButton } from "../ui/IconButton";
import { LevelSlider } from "../ui/LevelSlider";
import { cx } from "../ui/cx";
import { volumeIcon } from "./icons";
import { DomainMessage, SettingsLink, SystemNoticeLine, Tile } from "./shared";
import { useDomain } from "../../features/system/use-domain";

function VolumeControl({ audio }: { audio: AudioState }) {
  const level = useLevel("volume");
  const setVolume = useSystem((s) => s.setVolume);
  const toggleMute = useSystem((s) => s.toggleMute);
  const busy = useSystem((s) => Boolean(s.pending.mute));
  const output = audio.output;

  if (!output) {
    return (
      <p className="text-body text-fg-muted">
        No audio output device is active. Connect speakers or headphones.
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-1">
      <p title={output.name} className="truncate text-title font-semibold tracking-tight text-fg">
        {output.name}
      </p>
      <div className="flex items-center gap-3">
        <IconButton
          icon={volumeIcon(output, level)}
          label={output.muted ? "Unmute" : "Mute"}
          pressed={output.muted}
          busy={busy}
          onClick={() => void toggleMute()}
          tooltipSide="top"
          tooltipAlign="start"
        />
        <LevelSlider
          value={level}
          onChange={setVolume}
          label="Volume"
          muted={output.muted}
          className="min-w-0 flex-1"
        />
        <span
          aria-hidden="true"
          className={cx(
            "w-12 text-right text-title font-semibold tabular-nums",
            output.muted ? "text-fg-muted" : "text-fg",
          )}
        >
          {level ?? "–"}
          <span className="text-caption font-medium text-fg-muted">%</span>
        </span>
      </div>
      <p className="text-caption text-fg-muted">
        {output.muted ? "Muted" : "Same level as the Windows volume control"}
      </p>
      <SystemNoticeLine />
    </div>
  );
}

function DeviceList({ audio }: { audio: AudioState }) {
  if (audio.devices.length === 0) {
    return <p className="text-body text-fg-muted">No active output devices.</p>;
  }
  return (
    <ul aria-label="Output devices" className="-mx-1 flex min-h-0 flex-col overflow-y-auto">
      {audio.devices.map((device) => (
        <li
          key={device.id}
          className={cx(
            "flex h-7 shrink-0 items-center gap-2 rounded-[8px] px-1 text-body",
            device.isDefault ? "text-fg" : "text-fg-muted",
          )}
        >
          <Speaker size={14} strokeWidth={1.75} aria-hidden="true" className="shrink-0" />
          <span title={device.name} className="min-w-0 flex-1 truncate">
            {device.name}
          </span>
          {device.isDefault && (
            <>
              <Check size={14} strokeWidth={2.25} aria-hidden="true" className="shrink-0" />
              <span className="sr-only">(current output)</span>
            </>
          )}
        </li>
      ))}
    </ul>
  );
}

/** Master volume, mute, and the active output devices. */
export function AudioPanel() {
  const view = useDomain("audio");
  return (
    <div className="flex h-full gap-3">
      <Tile title="Volume" className="flex-[1.4]">
        {view.kind === "ready" ? (
          <VolumeControl audio={view.state} />
        ) : (
          <DomainMessage view={view} />
        )}
      </Tile>
      <Tile title="Output" className="flex-1">
        <div className="flex min-h-0 flex-1 flex-col justify-between gap-2">
          {view.kind === "ready" ? (
            <DeviceList audio={view.state} />
          ) : (
            <p className="text-body text-fg-muted">Output devices appear here.</p>
          )}
          {/* Windows has no documented API to change the default output. */}
          <SettingsLink page="sound">Change output in Sound settings</SettingsLink>
        </div>
      </Tile>
    </div>
  );
}
