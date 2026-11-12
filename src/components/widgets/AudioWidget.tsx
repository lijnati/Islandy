import { dispatchIsland } from "../../features/island/island-store";
import { useLevel, useSystem } from "../../features/system/system-store";
import { useDomain } from "../../features/system/use-domain";
import { volumeIcon } from "../system/icons";
import { DomainMessage } from "../system/shared";
import { IconButton } from "../ui/IconButton";
import { LevelSlider } from "../ui/LevelSlider";
import { WidgetSurface } from "../ui/WidgetSurface";
import { cx } from "../ui/cx";

/** Output device, volume and mute. Opens the Sound panel. */
export function AudioWidget({ className }: { className?: string }) {
  const view = useDomain("audio");
  const level = useLevel("volume");
  const setVolume = useSystem((s) => s.setVolume);
  const toggleMute = useSystem((s) => s.toggleMute);
  const busy = useSystem((s) => Boolean(s.pending.mute));
  const output = view.kind === "ready" ? view.state.output : null;

  return (
    <WidgetSurface
      title="Sound"
      onOpen={() => {
        dispatchIsland({ type: "OPEN_WIDGET", widget: "audio" });
      }}
      className={className}
    >
      {view.kind !== "ready" ? (
        <DomainMessage view={view} />
      ) : !output ? (
        <p className="text-body text-fg-muted">No active output device.</p>
      ) : (
        <div className="flex flex-1 flex-col justify-center gap-2">
          <p title={output.name} className="truncate text-body font-medium text-fg">
            {output.name}
          </p>
          <div className="flex items-center gap-2">
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
          </div>
          <p
            className={cx(
              "text-caption tabular-nums",
              output.muted ? "text-fg-subtle" : "text-fg-muted",
            )}
          >
            {output.muted ? "Muted" : `${level ?? "–"}%`}
          </p>
        </div>
      )}
    </WidgetSurface>
  );
}
