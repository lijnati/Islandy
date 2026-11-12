import { useAppInfo } from "../../hooks/useAppInfo";
import { useIslandStore } from "../../store/island";
import { Clock } from "./Clock";

/**
 * The island surface. It fills the native window exactly: the window is sized
 * by Rust to match the pill, so there is no invisible click-blocking margin.
 *
 * Phase 0 renders only the `collapsed` mode.
 */
export function Island() {
  const mode = useIslandStore((s) => s.mode);
  const appInfo = useAppInfo();

  const label =
    appInfo.status === "ready" ? `${appInfo.info.name} ${appInfo.info.version}` : "Islandy";

  return (
    <main
      data-mode={mode}
      data-ipc={appInfo.status}
      aria-label={label}
      title={label}
      className="flex h-full w-full items-center justify-center overflow-hidden rounded-island border border-hairline bg-surface px-4"
    >
      <Clock />
    </main>
  );
}
