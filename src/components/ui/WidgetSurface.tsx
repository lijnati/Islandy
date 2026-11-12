import { ChevronRight } from "lucide-react";
import { useId, type ReactNode } from "react";

import { focusRing } from "./IconButton";
import { cx } from "./cx";

interface WidgetSurfaceProps {
  title: string;
  /** Opens the widget's detail view. The header title becomes the button. */
  onOpen?: () => void;
  /** Marks the widget as showing local demo data. */
  demo?: boolean;
  /** Extra header content (right side). */
  accessory?: ReactNode;
  className?: string | undefined;
  children: ReactNode;
}

/** The card every dashboard widget sits in: secondary surface, hairline, header. */
export function WidgetSurface({
  title,
  onOpen,
  demo,
  accessory,
  className,
  children,
}: WidgetSurfaceProps) {
  const id = useId();
  return (
    <section
      aria-labelledby={id}
      className={cx(
        "flex min-w-0 flex-col rounded-tile border border-hairline bg-surface-secondary p-3 shadow-surface",
        className,
      )}
    >
      <header className="mb-2 flex h-4 items-center justify-between gap-2">
        {onOpen ? (
          <button
            type="button"
            id={id}
            onClick={onOpen}
            aria-label={`Open ${title}`}
            className={cx(
              "group/open -mx-1 inline-flex items-center gap-0.5 rounded-[6px] px-1 text-caption font-medium tracking-label text-fg-muted uppercase transition-colors duration-fast hover:text-fg",
              focusRing,
            )}
          >
            {title}
            <ChevronRight
              size={12}
              strokeWidth={2}
              aria-hidden="true"
              className="opacity-0 transition-opacity duration-fast group-hover/open:opacity-100 group-focus-visible/open:opacity-100"
            />
          </button>
        ) : (
          <h2 id={id} className="text-caption font-medium tracking-label text-fg-muted uppercase">
            {title}
          </h2>
        )}
        <div className="flex items-center gap-1.5">
          {accessory}
          {demo && <DemoBadge />}
        </div>
      </header>
      {children}
    </section>
  );
}

export function DemoBadge({
  title = "Sample data — not connected to Windows yet",
}: {
  title?: string;
}) {
  return (
    <span
      title={title}
      className="rounded-full border border-hairline px-1.5 text-[9px] leading-[14px] font-semibold tracking-label text-fg-subtle uppercase"
    >
      Demo
      <span className="sr-only">: {title}</span>
    </span>
  );
}
