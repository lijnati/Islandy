/**
 * Activity state fed by the Rust activity engine.
 *
 * - Snapshots arrive over one channel (`connectActivity`) and replace the
 *   previous one only if their revision is newer.
 * - Peek requests are handed to listeners (the presentation coordinator);
 *   they are never replayed after a reconnect.
 * - Commands are fire-and-forget on the Rust side: the result arrives as a
 *   snapshot. Toggles show `pending` until it does (never optimistic).
 * - The Claude Code integration status is read on demand; enabling or
 *   removing it only happens through `installIntegration`/`removeIntegration`,
 *   which the UI calls after an explicit click.
 */
import { createStore, useStore } from "zustand";

import * as native from "../../services/native";
import { NativeError } from "../../services/native";
import type {
  ActivitySnapshot,
  ActivitySource,
  ActivityUpdate,
  ClaudeIntegration,
  PeekRequest,
  PermissionDecision,
} from "../../services/types";
import type { ActivityFilter } from "./activity-model";

export type ActivityConnection = "idle" | "connecting" | "connected" | "unavailable";

export interface IntegrationView {
  status: ClaudeIntegration | null;
  busy: boolean;
  error: string | null;
}

export interface ActivityStoreState {
  connection: ActivityConnection;
  snapshot: ActivitySnapshot | null;
  filter: ActivityFilter;
  /** The Claude session shown in the Claude Code view. */
  selectedSession: string | null;
  /** In-flight setting changes: "quiet", "source:claudeCode", … */
  pending: Readonly<Record<string, true>>;
  integration: IntegrationView;
}

export interface ActivityStoreActions {
  start: () => void;
  /** Returns false when stale. */
  applySnapshot: (snapshot: ActivitySnapshot) => boolean;
  /** Dispatches a channel message (snapshot or peek). */
  receive: (update: ActivityUpdate) => void;
  setFilter: (filter: ActivityFilter) => void;
  selectSession: (id: string | null) => void;
  dismiss: (id: string) => void;
  /** Answers a permission request shown on the island (once). */
  answer: (session: string, question: number, decision: PermissionDecision) => void;
  clearHistory: () => void;
  setQuiet: (quiet: boolean) => void;
  setSource: (source: ActivitySource, enabled: boolean) => void;
  refreshIntegration: () => Promise<void>;
  installIntegration: () => Promise<void>;
  removeIntegration: () => Promise<void>;
  /** Subscribes to peek requests; returns an unsubscribe function. */
  onPeek: (listener: (peek: PeekRequest) => void) => () => void;
}

export type ActivityStore = ActivityStoreState & ActivityStoreActions;

export interface ActivityDeps {
  available: () => boolean;
  connectActivity: typeof native.connectActivity;
  dismissActivity: typeof native.dismissActivity;
  answerPermission: typeof native.answerPermission;
  clearActivityHistory: typeof native.clearActivityHistory;
  setQuietMode: typeof native.setQuietMode;
  setActivitySource: typeof native.setActivitySource;
  getClaudeIntegration: typeof native.getClaudeIntegration;
  installClaudeIntegration: typeof native.installClaudeIntegration;
  removeClaudeIntegration: typeof native.removeClaudeIntegration;
}

const nativeDeps: ActivityDeps = {
  available: native.isNativeAvailable,
  connectActivity: native.connectActivity,
  dismissActivity: native.dismissActivity,
  answerPermission: native.answerPermission,
  clearActivityHistory: native.clearActivityHistory,
  setQuietMode: native.setQuietMode,
  setActivitySource: native.setActivitySource,
  getClaudeIntegration: native.getClaudeIntegration,
  installClaudeIntegration: native.installClaudeIntegration,
  removeClaudeIntegration: native.removeClaudeIntegration,
};

export const initialActivityState: ActivityStoreState = {
  connection: "idle",
  snapshot: null,
  filter: "all",
  selectedSession: null,
  pending: {},
  integration: { status: null, busy: false, error: null },
};

function integrationErrorMessage(error: unknown): string {
  const kind = error instanceof NativeError ? error.kind : "unknown";
  switch (kind) {
    case "notFound":
      return "Claude Code's settings folder wasn't found. Run Claude Code once, then try again.";
    case "invalid":
      return "Claude Code's settings file couldn't be read safely, so it wasn't changed. Use the manual setup instead.";
    case "rejected":
      return "The settings file changed while Islandy was updating it. Nothing was written; try again.";
    case "unavailable":
      return "Available in the Islandy desktop app.";
    default:
      return "Couldn't update Claude Code's settings. Nothing was changed.";
  }
}

export function createActivityStore(deps: ActivityDeps = nativeDeps) {
  const peekListeners = new Set<(peek: PeekRequest) => void>();
  let disposed = false;

  const store = createStore<ActivityStore>()((set, get) => {
    const withoutPending = (key: string) =>
      Object.fromEntries(Object.entries(get().pending).filter(([k]) => k !== key)) as Record<
        string,
        true
      >;
    /** Sends a fire-and-forget command; `pending` clears with the next snapshot or on failure. */
    const command = (key: string | null, run: () => Promise<void>) => {
      if (key) set({ pending: { ...get().pending, [key]: true } });
      run().catch((error: unknown) => {
        console.warn("[islandy] activity command failed:", error);
        if (key) set({ pending: withoutPending(key) });
      });
    };

    const integrationCall = async (run: () => Promise<ClaudeIntegration>) => {
      if (get().integration.busy) return;
      set({ integration: { ...get().integration, busy: true, error: null } });
      try {
        const status = await run();
        if (!disposed) set({ integration: { status, busy: false, error: null } });
      } catch (error) {
        if (!disposed) {
          set({
            integration: {
              ...get().integration,
              busy: false,
              error: integrationErrorMessage(error),
            },
          });
        }
      }
    };

    return {
      ...initialActivityState,

      start: () => {
        if (get().connection !== "idle") return;
        if (!deps.available()) {
          set({ connection: "unavailable" });
          return;
        }
        set({ connection: "connecting" });
        deps
          .connectActivity((update) => {
            get().receive(update);
          })
          .then(
            (snapshot) => {
              if (disposed) return;
              get().applySnapshot(snapshot);
              set({ connection: "connected" });
            },
            (error: unknown) => {
              console.warn("[islandy] connect_activity failed:", error);
              if (!disposed) set({ connection: "unavailable" });
            },
          );
        void get().refreshIntegration();
      },

      applySnapshot: (snapshot) => {
        const current = get().snapshot;
        if (current && snapshot.revision <= current.revision) return false;
        // Settings changes confirmed by Rust clear their pending flags.
        set({ snapshot, pending: {} });
        return true;
      },

      receive: (update) => {
        if (disposed) return;
        if (update.type === "snapshot") {
          get().applySnapshot(update.snapshot);
        } else {
          for (const listener of [...peekListeners]) listener(update.peek);
        }
      },

      setFilter: (filter) => {
        set({ filter });
      },

      selectSession: (id) => {
        set({ selectedSession: id });
      },

      dismiss: (id) => {
        command(null, () => deps.dismissActivity(id));
      },

      answer: (session, question, decision) => {
        const key = `answer:${session}:${question}`;
        if (get().pending[key]) return;
        command(key, () => deps.answerPermission(session, question, decision));
      },

      clearHistory: () => {
        command("clear", () => deps.clearActivityHistory());
      },

      setQuiet: (quiet) => {
        if (get().pending.quiet) return;
        command("quiet", () => deps.setQuietMode(quiet));
      },

      setSource: (source, enabled) => {
        const key = `source:${source}`;
        if (get().pending[key]) return;
        command(key, () => deps.setActivitySource(source, enabled));
      },

      refreshIntegration: () => integrationCall(() => deps.getClaudeIntegration()),
      installIntegration: () => integrationCall(() => deps.installClaudeIntegration()),
      removeIntegration: () => integrationCall(() => deps.removeClaudeIntegration()),

      onPeek: (listener) => {
        peekListeners.add(listener);
        return () => {
          peekListeners.delete(listener);
        };
      },
    };
  });

  const dispose = () => {
    disposed = true;
    peekListeners.clear();
  };

  return Object.assign(store, { dispose });
}

export const activityStore = createActivityStore();

export function useActivity<T>(selector: (state: ActivityStore) => T): T {
  return useStore(activityStore, selector);
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    activityStore.dispose();
  });
}
