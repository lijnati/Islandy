import { Keyboard } from "lucide-react";
import { useEffect, useState } from "react";

import { cx } from "../../../components/ui/cx";
import { isNativeAvailable, setToggleShortcut } from "../settings-service";
import { displayShortcut, recordKey } from "../shortcut-keys";
import { Button } from "./controls";

/**
 * Shows the toggle shortcut and records a new one. The new combination is
 * registered by Rust before it is saved; if Windows or another app owns
 * it, the old shortcut keeps working and the reason is shown.
 */
export function ShortcutRecorder({
  accelerator,
  registered,
  registrationError,
}: {
  accelerator: string;
  registered: boolean;
  registrationError: string | null;
}) {
  const [recording, setRecording] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!recording) return undefined;
    const onKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      const result = recordKey(event);
      switch (result.kind) {
        case "pending":
          setPreview(result.preview);
          break;
        case "cancel":
          setRecording(false);
          setPreview(null);
          break;
        case "invalid":
          setError(result.reason);
          break;
        case "done":
          setRecording(false);
          setPreview(null);
          if (!isNativeAvailable()) {
            setError("Shortcuts can be changed in the desktop app.");
            return;
          }
          setBusy(true);
          setToggleShortcut(result.accelerator).then(
            () => {
              setBusy(false);
              setError(null);
            },
            (cause: unknown) => {
              setBusy(false);
              setError(
                cause instanceof Error
                  ? cause.message.replace(/^set_toggle_shortcut: /, "")
                  : String(cause),
              );
            },
          );
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
    };
  }, [recording]);

  const shown = recording ? (preview ?? "Press a key combination…") : displayShortcut(accelerator);
  const problem = error ?? (!registered && registrationError ? registrationError : null);

  return (
    <div className="flex flex-col items-end gap-1.5">
      <div className="flex items-center gap-2">
        <kbd
          aria-live="polite"
          className={cx(
            "inline-flex h-8 min-w-36 items-center justify-center gap-1.5 rounded-[8px] border px-3 font-sans text-caption font-semibold",
            recording ? "border-accent text-fg" : "border-hairline bg-surface text-fg",
          )}
        >
          <Keyboard size={13} strokeWidth={2} aria-hidden="true" className="text-fg-muted" />
          {shown}
        </kbd>
        <Button
          busy={busy}
          onClick={() => {
            setError(null);
            setRecording((r) => !r);
          }}
        >
          {recording ? "Cancel" : "Change"}
        </Button>
      </div>
      {problem ? (
        <p role="alert" className="max-w-72 text-right text-caption text-status-attention">
          {problem}
          {!registered && " The tray icon still opens Islandy."}
        </p>
      ) : recording ? (
        <p className="text-caption text-fg-subtle">Esc to cancel</p>
      ) : null}
    </div>
  );
}
