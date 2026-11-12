import type { Track } from "../../features/media/demo-media";
import { cx } from "../ui/cx";

/**
 * Generated album artwork for demo tracks (no image files, no network):
 * a soft two-tone field with a sun disc and horizon line.
 */
export function Artwork({ track, className }: { track: Track; className?: string }) {
  const [a, b] = track.artwork;
  return (
    <div
      role="img"
      aria-label={`Artwork for ${track.album}`}
      className={cx("relative shrink-0 overflow-hidden rounded-art shadow-art", className)}
      style={{
        backgroundColor: a,
        backgroundImage: `linear-gradient(160deg, ${a} 15%, ${b} 120%)`,
      }}
    >
      <span
        aria-hidden="true"
        className="absolute top-[22%] left-[26%] size-[42%] rounded-full opacity-90"
        style={{ backgroundColor: b, boxShadow: `0 0 24px ${b}` }}
      />
      <span aria-hidden="true" className="absolute inset-x-0 top-[64%] bottom-0 bg-black/35" />
      <span
        aria-hidden="true"
        className="absolute inset-0 rounded-art ring-1 ring-white/10 ring-inset"
      />
    </div>
  );
}
