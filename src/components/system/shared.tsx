import { ExternalLink } from "lucide-react";
import type { ReactNode } from "react";

import { useCanOpenSystemSettings } from "../../features/island/native-status";
import { openSettings } from "../../features/system/open-settings";
import { useSystem } from "../../features/system/system-store";
import type { SettingsPage } from "../../services/types";
import { focusRing } from "../ui/IconButton";
import { cx } from "../ui/cx";

/** A quiet pill button that opens a Windows Settings page. */
export function SettingsLink({ page, children }: { page: SettingsPage; children: ReactNode }) {
  const canOpen = useCanOpenSystemSettings();
  return (
    <button
      type="button"
      onClick={
        canOpen
          ? () => {
              openSettings(page);
            }
          : undefined
      }
      aria-disabled={!canOpen || undefined}
      title={canOpen ? "Opens Windows Settings" : "Available in Islandy on Windows"}
      className={cx(
        "inline-flex h-7 max-w-full shrink-0 items-center gap-1.5 self-start rounded-full border border-hairline px-2.5 text-caption font-medium transition-colors duration-fast ease-standard",
        focusRing,
        canOpen
          ? "text-fg-muted hover:bg-surface-hover hover:text-fg"
          : "cursor-not-allowed text-fg-subtle",
      )}
    >
      <span className="truncate">{children}</span>
      <ExternalLink size={12} strokeWidth={2} aria-hidden="true" className="shrink-0" />
    </button>
  );
}

/** A small uppercase label, like the widget headers. */
export function Caption({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span
      className={cx("text-caption font-medium tracking-label text-fg-muted uppercase", className)}
    >
      {children}
    </span>
  );
}

/** A panel card (same surface as the dashboard widgets). */
export function Tile({
  title,
  accessory,
  className,
  children,
}: {
  title: string;
  accessory?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section
      aria-label={title}
      className={cx(
        "flex min-w-0 flex-col rounded-tile border border-hairline bg-surface-secondary p-3 shadow-surface",
        className,
      )}
    >
      <header className="mb-2 flex h-4 items-center justify-between gap-2">
        <Caption>{title}</Caption>
        {accessory}
      </header>
      {children}
    </section>
  );
}

const DEFAULT_MESSAGES = {
  browser: "Available in the Islandy desktop app on Windows.",
  unsupported: "Not available on this system.",
  unavailable: "Couldn't read this from Windows. Trying again shortly.",
} as const;

/** The non-ready states of a domain, as a short message. */
export function DomainMessage({
  view,
  messages = {},
}: {
  view: { kind: "loading" | "browser" | "unsupported" | "unavailable" };
  messages?: Partial<Record<"browser" | "unsupported" | "unavailable", string>>;
}) {
  if (view.kind === "loading") {
    return (
      <div aria-label="Loading" className="flex flex-col gap-2 py-1">
        <span className="h-3.5 w-2/3 animate-pulse rounded-full bg-surface-hover motion-reduce:animate-none" />
        <span className="h-3 w-1/3 animate-pulse rounded-full bg-surface-hover motion-reduce:animate-none" />
      </div>
    );
  }
  return (
    <p className="text-body text-fg-muted">{messages[view.kind] ?? DEFAULT_MESSAGES[view.kind]}</p>
  );
}

/** Transient error from a failed system command. */
export function SystemNoticeLine({ className }: { className?: string }) {
  const notice = useSystem((s) => s.notice);
  return (
    <p
      role="status"
      className={cx("min-h-[14px] truncate text-caption text-status-attention", className)}
    >
      {notice?.message ?? ""}
    </p>
  );
}
