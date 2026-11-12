import { cx } from "./cx";
import { focusRing } from "./IconButton";

interface SwitchProps {
  /** The state Windows reports. */
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
  /** A change is in flight: further presses are ignored by the caller. */
  busy?: boolean;
}

/**
 * An on/off switch that only ever shows the *reported* state: pressing it
 * asks for the change, and the knob moves when Windows confirms it.
 */
export function Switch({ checked, onChange, label, disabled = false, busy = false }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      aria-disabled={disabled || undefined}
      aria-busy={busy || undefined}
      onClick={
        disabled || busy
          ? undefined
          : () => {
              onChange(!checked);
            }
      }
      className={cx(
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors duration-fast ease-standard",
        focusRing,
        checked ? "border-transparent bg-fg" : "border-hairline-strong bg-surface-hover",
        busy && "opacity-60",
        disabled ? "cursor-not-allowed opacity-40" : "cursor-pointer",
      )}
    >
      <span
        aria-hidden="true"
        className={cx(
          "absolute size-3.5 rounded-full shadow-control transition-transform duration-base ease-standard",
          checked ? "translate-x-[18px] bg-surface" : "translate-x-[2px] bg-fg",
        )}
      />
    </button>
  );
}
