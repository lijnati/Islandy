import { Check, ChevronDown } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";

import { displayTitle } from "../../features/media/media-model";
import { useMedia } from "../../features/media/media-store";
import { focusRing } from "../ui/IconButton";
import { cx } from "../ui/cx";

/**
 * The source app of the selected session. With several sessions it becomes a
 * small menu to pick one (or return to automatic selection). The choice is
 * kept by Rust until that session disappears.
 */
export function SessionSwitcher({ align = "end" }: { align?: "start" | "end" }) {
  const snapshot = useMedia((s) => s.snapshot);
  const selecting = useMedia((s) => s.selecting);
  const select = useMedia((s) => s.select);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const menuId = useId();

  const sessions = snapshot?.sessions ?? [];
  const selected = sessions.find((s) => s.id === snapshot?.selectedSessionId) ?? null;
  const multiple = sessions.length > 1;
  const isOpen = open && multiple;

  useEffect(() => {
    if (!isOpen) return undefined;
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    rootRef.current?.querySelector<HTMLElement>("[aria-checked=true]")?.focus();
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [isOpen]);

  if (!selected) return null;
  if (!multiple) {
    return (
      <span className="max-w-[140px] truncate text-caption text-fg-subtle">
        {selected.sourceAppName}
      </span>
    );
  }

  const choose = (id: string | null) => {
    setOpen(false);
    void select(id);
  };
  const auto = snapshot?.selectionMode === "auto";

  return (
    <div
      ref={rootRef}
      className="relative"
      onKeyDown={(event) => {
        // Keep Escape inside the menu instead of collapsing the island.
        if (event.key === "Escape" && isOpen) {
          event.stopPropagation();
          event.preventDefault();
          setOpen(false);
          rootRef.current?.querySelector<HTMLElement>("button")?.focus();
        }
        if ((event.key === "ArrowDown" || event.key === "ArrowUp") && isOpen) {
          event.preventDefault();
          const items = [
            ...(rootRef.current?.querySelectorAll<HTMLElement>("[role=menuitemradio]") ?? []),
          ];
          const index = items.indexOf(document.activeElement as HTMLElement);
          const next = (index + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
          items[next]?.focus();
        }
      }}
    >
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-controls={isOpen ? menuId : undefined}
        aria-busy={selecting || undefined}
        aria-label={`Player: ${selected.sourceAppName}. ${sessions.length} players available`}
        onClick={() => {
          setOpen((v) => !v);
        }}
        className={cx(
          "-mr-1 inline-flex h-5 max-w-[150px] items-center gap-0.5 rounded-full px-1.5 text-caption font-medium text-fg-muted transition-colors duration-fast hover:bg-surface-hover hover:text-fg",
          isOpen && "bg-surface-hover text-fg",
          focusRing,
        )}
      >
        <span className="truncate">{selected.sourceAppName}</span>
        <span className="text-fg-subtle tabular-nums">·{sessions.length}</span>
        <ChevronDown size={12} strokeWidth={2} aria-hidden="true" className="shrink-0" />
      </button>
      {isOpen && (
        <div
          id={menuId}
          role="menu"
          aria-label="Choose a player"
          className={cx(
            "absolute top-full z-(--z-overlay) mt-1 flex max-h-[150px] w-[230px] flex-col overflow-y-auto rounded-[12px] border border-hairline-strong bg-surface-elevated p-1 shadow-control",
            align === "end" ? "right-0" : "left-0",
          )}
        >
          <MenuItem
            checked={auto}
            onSelect={() => {
              choose(null);
            }}
          >
            <span className="text-body text-fg">Automatic</span>
            <span className="text-caption text-fg-subtle">Follow what's playing</span>
          </MenuItem>
          <div aria-hidden="true" className="my-1 h-px shrink-0 bg-hairline" />
          {sessions.map((s) => (
            <MenuItem
              key={s.id}
              checked={!auto && s.id === selected.id}
              onSelect={() => {
                choose(s.id);
              }}
            >
              <span className="flex items-center gap-1.5 text-body text-fg">
                <span className="truncate">{s.sourceAppName}</span>
                {s.playbackStatus === "playing" && (
                  <span
                    aria-label="playing"
                    className="size-1.5 shrink-0 rounded-full bg-status-success"
                  />
                )}
              </span>
              <span className="truncate text-caption text-fg-subtle">{displayTitle(s)}</span>
            </MenuItem>
          ))}
        </div>
      )}
    </div>
  );
}

function MenuItem({
  checked,
  onSelect,
  children,
}: {
  checked: boolean;
  onSelect: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      role="menuitemradio"
      aria-checked={checked}
      tabIndex={-1}
      onClick={onSelect}
      className={cx(
        "flex w-full shrink-0 items-center gap-2 rounded-[8px] px-2 py-1 text-left transition-colors duration-fast hover:bg-surface-hover focus-visible:bg-surface-hover",
        "outline-hidden",
      )}
    >
      <span className="flex min-w-0 flex-1 flex-col">{children}</span>
      <Check
        size={13}
        strokeWidth={2}
        aria-hidden="true"
        className={cx("shrink-0 text-fg", !checked && "invisible")}
      />
    </button>
  );
}
