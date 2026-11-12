import { useId, useState, type ReactNode } from "react";

import { focusRing } from "../../../components/ui/IconButton";
import { cx } from "../../../components/ui/cx";

/** A titled group of rows (hairline-separated, on a faint surface). */
export function Section({
  title,
  description,
  children,
  footer,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
}) {
  const id = useId();
  return (
    <section aria-labelledby={id} className="flex flex-col gap-2">
      <div className="flex flex-col gap-0.5 px-1">
        <h2 id={id} className="text-caption font-semibold tracking-label text-fg-muted uppercase">
          {title}
        </h2>
        {description && <p className="text-caption text-fg-subtle">{description}</p>}
      </div>
      <div className="divide-y divide-hairline rounded-field border border-hairline bg-surface-raised">
        {children}
      </div>
      {footer && <div className="px-1 text-caption text-fg-subtle">{footer}</div>}
    </section>
  );
}

/** One setting: label and description on the left, its control on the right. */
export function Row({
  label,
  description,
  control,
  htmlFor,
  stacked = false,
  disabled = false,
}: {
  label: ReactNode;
  description?: ReactNode;
  control?: ReactNode;
  htmlFor?: string;
  /** Put the control under the text (wide controls). */
  stacked?: boolean;
  disabled?: boolean;
}) {
  const text = (
    <div className={cx("flex min-w-0 flex-col gap-0.5", disabled && "opacity-50")}>
      {htmlFor ? (
        <label htmlFor={htmlFor} className="text-body font-medium text-fg">
          {label}
        </label>
      ) : (
        <span className="text-body font-medium text-fg">{label}</span>
      )}
      {description && <p className="text-caption text-fg-muted">{description}</p>}
    </div>
  );
  return (
    <div
      className={cx(
        "flex gap-4 px-4 py-3",
        stacked ? "flex-col" : "min-h-[52px] items-center justify-between",
      )}
    >
      {text}
      {control && <div className={cx(stacked ? "" : "flex shrink-0 items-center")}>{control}</div>}
    </div>
  );
}

export function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
  disabled = false,
}: {
  label: string;
  value: T;
  options: readonly { value: T; label: string; title?: string }[];
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="inline-flex rounded-[9px] border border-hairline bg-surface p-0.5"
    >
      {options.map((o) => {
        const selected = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={selected}
            title={o.title}
            disabled={disabled}
            onClick={() => {
              if (!selected) onChange(o.value);
            }}
            className={cx(
              "h-7 rounded-[7px] px-3 text-caption font-medium whitespace-nowrap transition-colors duration-fast ease-standard disabled:cursor-not-allowed disabled:opacity-50",
              focusRing,
              selected
                ? "bg-surface-elevated text-fg shadow-control"
                : "text-fg-muted hover:text-fg",
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

/**
 * A bounded slider. It follows the pointer locally and commits on release
 * (or key press), so dragging doesn't write the settings file per pixel.
 */
export function RangeField({
  id,
  label,
  value,
  min,
  max,
  step,
  format,
  onCommit,
  disabled = false,
}: {
  id?: string;
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  format: (value: number) => string;
  onCommit: (value: number) => void;
  disabled?: boolean;
}) {
  const [draft, setDraft] = useState<number | null>(null);
  const shown = draft ?? value;
  const commit = () => {
    if (draft === null) return;
    if (draft !== value) onCommit(draft);
    setDraft(null);
  };
  return (
    <div className="flex items-center gap-3">
      <input
        id={id}
        type="range"
        aria-label={label}
        aria-valuetext={format(shown)}
        min={min}
        max={max}
        step={step}
        value={shown}
        disabled={disabled}
        onChange={(e) => {
          setDraft(Number(e.currentTarget.value));
        }}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
        className={cx(
          "h-1.5 w-44 cursor-pointer accent-(--color-accent) disabled:cursor-not-allowed disabled:opacity-50",
          focusRing,
        )}
      />
      <output
        htmlFor={id}
        className="w-16 text-right text-caption font-medium text-fg-muted tabular-nums"
      >
        {format(shown)}
      </output>
    </div>
  );
}

export function SelectField({
  id,
  label,
  value,
  options,
  onChange,
  disabled = false,
}: {
  id?: string;
  label: string;
  value: string;
  options: readonly { value: string; label: string }[];
  onChange: (value: string) => void;
  disabled?: boolean;
}) {
  return (
    <select
      id={id}
      aria-label={label}
      value={value}
      disabled={disabled}
      onChange={(e) => {
        onChange(e.currentTarget.value);
      }}
      className={cx(
        "h-8 max-w-64 min-w-48 rounded-[8px] border border-hairline bg-surface px-2.5 text-caption font-medium text-fg disabled:opacity-50",
        focusRing,
      )}
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function Button({
  children,
  onClick,
  variant = "secondary",
  disabled = false,
  busy = false,
  title,
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "primary" | "secondary" | "danger";
  disabled?: boolean;
  busy?: boolean;
  title?: string;
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      onClick={onClick}
      className={cx(
        "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-[8px] px-3 text-caption font-medium whitespace-nowrap transition-colors duration-fast ease-standard disabled:cursor-not-allowed disabled:opacity-50",
        focusRing,
        variant === "primary" && "bg-accent text-on-accent hover:bg-accent/90",
        variant === "secondary" &&
          "border border-hairline bg-surface-raised text-fg hover:bg-surface-hover",
        variant === "danger" &&
          "border border-status-attention/40 text-status-attention hover:bg-status-attention/10",
      )}
    >
      {children}
    </button>
  );
}

/** A destructive action that asks once more, inline (no modal). */
export function ConfirmButton({
  children,
  confirmLabel,
  question,
  onConfirm,
  disabled = false,
}: {
  children: ReactNode;
  confirmLabel: string;
  question: string;
  onConfirm: () => void;
  disabled?: boolean;
}) {
  const [asking, setAsking] = useState(false);
  if (!asking) {
    return (
      <Button
        variant="secondary"
        disabled={disabled}
        onClick={() => {
          setAsking(true);
        }}
      >
        {children}
      </Button>
    );
  }
  return (
    <div role="group" aria-label={question} className="flex items-center gap-2">
      <span className="text-caption text-fg-muted">{question}</span>
      <Button
        variant="danger"
        onClick={() => {
          setAsking(false);
          onConfirm();
        }}
      >
        {confirmLabel}
      </Button>
      <Button
        onClick={() => {
          setAsking(false);
        }}
      >
        Cancel
      </Button>
    </div>
  );
}

export function StatusText({
  tone,
  children,
}: {
  tone: "neutral" | "success" | "warning" | "error";
  children: ReactNode;
}) {
  return (
    <span
      className={cx(
        "inline-flex items-center gap-1.5 text-caption font-medium",
        tone === "neutral" && "text-fg-muted",
        tone === "success" && "text-status-success",
        tone === "warning" && "text-status-waiting",
        tone === "error" && "text-status-attention",
      )}
    >
      <span
        aria-hidden="true"
        className={cx(
          "size-1.5 rounded-full",
          tone === "neutral" && "bg-status-neutral",
          tone === "success" && "bg-status-success",
          tone === "warning" && "bg-status-waiting",
          tone === "error" && "bg-status-attention",
        )}
      />
      {children}
    </span>
  );
}

export function Code({ children }: { children: string }) {
  return (
    <code className="rounded-[5px] bg-surface px-1.5 py-0.5 font-mono text-[12px] text-fg select-text">
      {children}
    </code>
  );
}

export function PageHeader({ title, description }: { title: string; description?: ReactNode }) {
  return (
    <header className="flex flex-col gap-1">
      <h1 className="text-heading font-semibold tracking-tight text-fg">{title}</h1>
      {description && <p className="max-w-[560px] text-body text-fg-muted">{description}</p>}
    </header>
  );
}
