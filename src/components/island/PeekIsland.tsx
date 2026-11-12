import { Check, Sparkle } from "lucide-react";

import { dispatchIsland, type PeekContent } from "../../features/island/island-store";
import { focusRing } from "../ui/IconButton";
import { StatusIndicator } from "../ui/StatusIndicator";
import { cx } from "../ui/cx";

/** A temporary contextual capsule. Announced politely to screen readers. */
export function PeekIsland({ peek }: { peek: PeekContent }) {
  return (
    <button
      type="button"
      onClick={() => {
        dispatchIsland({ type: "PRESS" });
      }}
      aria-label={`${peek.source}: ${peek.title}, ${peek.detail}. Open dashboard`}
      className={cx("flex h-full w-full items-center gap-3 rounded-full pr-4 pl-2", focusRing)}
    >
      <span
        aria-hidden="true"
        className="flex size-8 shrink-0 items-center justify-center rounded-full bg-surface-elevated text-fg"
      >
        {peek.tone === "success" ? (
          <Check size={15} strokeWidth={2.25} />
        ) : (
          <Sparkle size={14} strokeWidth={2} />
        )}
      </span>
      <span role="status" className="flex min-w-0 flex-1 flex-col items-start text-left">
        <span className="text-caption font-medium text-fg-muted">{peek.source}</span>
        <span className="w-full truncate text-body font-semibold tracking-tight text-fg">
          {peek.title}
          <span className="font-regular text-fg-muted"> · {peek.detail}</span>
        </span>
      </span>
      <StatusIndicator tone={peek.tone} />
    </button>
  );
}
