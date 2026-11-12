import { useState } from "react";

import type { ActivitySettings } from "../../services/types";
import { getActivitySnapshot, isNativeAvailable, setActivitySettings } from "./settings-service";
import { useNativeQuery } from "./use-native-query";

/**
 * Activity (notification) settings for the settings window. Rust's activity
 * engine owns them; this reads its snapshot and sends whole updates, showing
 * the requested value until the engine confirms it.
 */
export function useActivitySettings() {
  const snapshot = useNativeQuery(getActivitySnapshot, { pollMs: 3000 });
  const [pending, setPending] = useState<ActivitySettings | null>(null);
  const confirmed = snapshot.data?.settings ?? null;
  const settings = pending ?? confirmed;

  const update = (next: ActivitySettings) => {
    if (!isNativeAvailable()) return;
    setPending(next);
    setActivitySettings(next).then(
      () => {
        // The engine applies commands asynchronously: re-read shortly after,
        // and keep showing the requested value until that read has landed.
        setTimeout(snapshot.reload, 150);
        setTimeout(() => {
          setPending((p) => (p === next ? null : p));
        }, 1000);
      },
      () => {
        setPending(null);
      },
    );
  };

  return {
    settings,
    snapshot: snapshot.data,
    reload: snapshot.reload,
    update,
    ready: isNativeAvailable() && confirmed !== null,
  };
}
