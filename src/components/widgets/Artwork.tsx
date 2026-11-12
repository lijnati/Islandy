import { Music2 } from "lucide-react";

import { useMedia } from "../../features/media/media-store";
import { cx } from "../ui/cx";

interface MediaArtworkProps {
  /** Content key from the media snapshot; `null` shows the placeholder. */
  artworkKey: string | null;
  /** Accessible description, e.g. the album or track name. */
  label: string;
  className?: string;
  /** Corner radius utility (kept separate so it can't conflict with `className`). */
  rounded?: string;
  iconSize?: number;
}

/**
 * Album artwork from the media session (a local, size-limited data URL
 * produced by the Rust side), or a neutral placeholder while it loads or
 * when the app provides none. Only artwork whose key matches the *current*
 * track is ever shown, so switching tracks or sessions can't show stale art.
 */
export function MediaArtwork({
  artworkKey,
  label,
  className,
  rounded = "rounded-art",
  iconSize = 22,
}: MediaArtworkProps) {
  const url = useMedia((s) => (artworkKey ? s.artwork[artworkKey] : undefined));
  return (
    <div
      role="img"
      aria-label={label}
      className={cx(
        "relative shrink-0 overflow-hidden bg-surface-elevated shadow-art",
        rounded,
        className,
      )}
    >
      {url ? (
        <img
          src={url}
          alt=""
          draggable={false}
          decoding="async"
          className="absolute inset-0 size-full object-cover"
        />
      ) : (
        <span
          aria-hidden="true"
          className="absolute inset-0 flex items-center justify-center bg-linear-to-br from-surface-elevated to-surface-secondary text-fg-subtle"
        >
          <Music2 size={iconSize} strokeWidth={1.5} />
        </span>
      )}
      <span
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 rounded-[inherit] ring-1 ring-hairline-strong ring-inset"
      />
    </div>
  );
}
