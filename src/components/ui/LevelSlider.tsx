import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";

import { cx } from "./cx";

interface LevelSliderProps {
  /** 0–100; `null` when unknown (renders an empty, disabled track). */
  value: number | null;
  onChange: (value: number) => void;
  label: string;
  disabled?: boolean;
  /** Dimmed fill, e.g. while muted. */
  muted?: boolean;
  className?: string;
}

const STEP = 2;
const PAGE = 10;

/**
 * A 0–100 slider with pointer drag and full keyboard support. It reports
 * every movement; callers (the system store) coalesce what reaches Windows.
 * The value shown is whatever the caller passes back.
 */
export function LevelSlider({
  value,
  onChange,
  label,
  disabled = false,
  muted = false,
  className,
}: LevelSliderProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);
  const inactive = disabled || value === null;
  const shown = value ?? 0;

  const fromPointer = (event: PointerEvent<HTMLDivElement>) => {
    const rect = trackRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return shown;
    return Math.round(Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)) * 100);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const next = {
      ArrowRight: shown + STEP,
      ArrowUp: shown + STEP,
      ArrowLeft: shown - STEP,
      ArrowDown: shown - STEP,
      PageUp: shown + PAGE,
      PageDown: shown - PAGE,
      Home: 0,
      End: 100,
    }[event.key];
    if (next === undefined) return;
    event.preventDefault();
    // Don't let arrows reach the island's own key handling.
    event.stopPropagation();
    onChange(Math.min(100, Math.max(0, next)));
  };

  return (
    <div
      role="slider"
      tabIndex={inactive ? -1 : 0}
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value ?? undefined}
      aria-valuetext={value === null ? "Unknown" : `${value}%`}
      aria-disabled={inactive || undefined}
      onPointerDown={
        inactive
          ? undefined
          : (event) => {
              event.currentTarget.setPointerCapture(event.pointerId);
              setDragging(true);
              onChange(fromPointer(event));
            }
      }
      onPointerMove={(event) => {
        if (dragging) onChange(fromPointer(event));
      }}
      onPointerUp={() => {
        setDragging(false);
      }}
      onPointerCancel={() => {
        setDragging(false);
      }}
      onKeyDown={inactive ? undefined : onKeyDown}
      className={cx(
        "group/level relative flex h-6 items-center outline-hidden",
        inactive ? "cursor-not-allowed" : "cursor-pointer touch-none",
        className,
      )}
    >
      <div
        ref={trackRef}
        className={cx(
          "relative h-1.5 w-full rounded-full bg-surface-hover",
          "group-focus-visible/level:outline-2 group-focus-visible/level:outline-offset-4 group-focus-visible/level:outline-focus group-focus-visible/level:outline-solid",
        )}
      >
        <div
          className={cx(
            "absolute inset-y-0 left-0 rounded-full",
            inactive ? "bg-fg-subtle" : muted ? "bg-fg-muted" : "bg-accent",
            !dragging && "transition-[width] duration-base ease-standard",
          )}
          style={{ width: `${shown}%` }}
        />
        {!inactive && (
          <span
            aria-hidden="true"
            className={cx(
              "absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-fg shadow-control ease-standard group-hover/level:scale-110",
              dragging
                ? "scale-110 transition-transform duration-fast"
                : "scale-100 transition-[left,scale] duration-base",
            )}
            style={{ left: `${shown}%` }}
          />
        )}
      </div>
    </div>
  );
}
