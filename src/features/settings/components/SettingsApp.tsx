import {
  Bell,
  Info,
  LayoutGrid,
  Palette,
  PanelTop,
  Plug,
  Shield,
  SlidersHorizontal,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { cx } from "../../../components/ui/cx";
import { focusRing } from "../../../components/ui/IconButton";
import type { SettingsPageId } from "../../../services/types";
import { useApplyAppearance } from "../appearance";
import { isSettingsPage, SETTINGS_PAGES } from "../settings-schema";
import { settingsStore, useSettingsStore } from "../settings-store";
import { IslandyMark } from "./IslandyMark";
import { AboutPage } from "./pages/AboutPage";
import { AppearancePage } from "./pages/AppearancePage";
import { GeneralPage } from "./pages/GeneralPage";
import { IntegrationsPage } from "./pages/IntegrationsPage";
import { IslandPage } from "./pages/IslandPage";
import { NotificationsPage } from "./pages/NotificationsPage";
import { PrivacyPage } from "./pages/PrivacyPage";
import { WelcomePage } from "./pages/WelcomePage";
import { WidgetsPage } from "./pages/WidgetsPage";

const ICONS: Record<Exclude<SettingsPageId, "welcome">, LucideIcon> = {
  general: SlidersHorizontal,
  appearance: Palette,
  island: PanelTop,
  widgets: LayoutGrid,
  integrations: Plug,
  notifications: Bell,
  privacy: Shield,
  about: Info,
};

const PAGES: Record<Exclude<SettingsPageId, "welcome">, () => React.JSX.Element> = {
  general: GeneralPage,
  appearance: AppearancePage,
  island: IslandPage,
  widgets: WidgetsPage,
  integrations: IntegrationsPage,
  notifications: NotificationsPage,
  privacy: PrivacyPage,
  about: AboutPage,
};

function pageFromHash(): SettingsPageId {
  const hash = window.location.hash.replace(/^#/, "");
  return isSettingsPage(hash) ? hash : "general";
}

function StatusBanner() {
  const loadStatus = useSettingsStore((s) => s.state.loadStatus);
  const saveError = useSettingsStore((s) => s.state.saveError);
  const error = useSettingsStore((s) => s.error);
  const message =
    error ??
    (saveError
      ? "Settings couldn't be saved to disk. Changes apply until Islandy quits."
      : loadStatus.kind === "recovered"
        ? "Your settings file was unreadable, so defaults are in use. The old file was kept as settings.corrupt.json."
        : loadStatus.kind === "newer"
          ? "These settings were written by a newer Islandy. A backup was made before this version saves them."
          : null);
  if (!message) return null;
  return (
    <p
      role="alert"
      className="rounded-field border border-status-waiting/30 bg-status-waiting/10 px-3 py-2 text-caption text-fg"
    >
      {message}
    </p>
  );
}

/** The settings window: sidebar navigation and one page at a time. */
export function SettingsApp() {
  useApplyAppearance();
  const [page, setPage] = useState<SettingsPageId>(pageFromHash);
  const main = useRef<HTMLElement>(null);

  useEffect(() => {
    settingsStore.getState().start();
    const unsubscribe = settingsStore.getState().onNavigate(setPage);
    const onHash = () => {
      setPage(pageFromHash());
    };
    window.addEventListener("hashchange", onHash);
    return () => {
      unsubscribe();
      window.removeEventListener("hashchange", onHash);
    };
  }, []);

  useEffect(() => {
    if (main.current) main.current.scrollTop = 0;
    if (window.location.hash !== `#${page}`) history.replaceState(null, "", `#${page}`);
  }, [page]);

  const Page = page === "welcome" ? null : PAGES[page];
  return (
    <div className="flex h-full bg-settings text-fg">
      <nav
        aria-label="Settings"
        className="flex w-[212px] shrink-0 flex-col gap-1 border-r border-hairline px-3 py-5"
      >
        <div className="mb-4 flex items-center gap-2.5 px-2">
          <IslandyMark size={26} />
          <span className="text-title font-semibold tracking-tight">Islandy</span>
        </div>
        <ul className="flex flex-col gap-0.5">
          {SETTINGS_PAGES.map(({ id, label }) => {
            const Icon = ICONS[id];
            const current = id === page;
            return (
              <li key={id}>
                <button
                  type="button"
                  aria-current={current ? "page" : undefined}
                  onClick={() => {
                    setPage(id);
                  }}
                  className={cx(
                    "relative flex h-8 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-body transition-colors duration-fast ease-standard",
                    focusRing,
                    current
                      ? "bg-surface-hover font-medium text-fg"
                      : "text-fg-muted hover:bg-surface-raised hover:text-fg",
                  )}
                >
                  {current && (
                    <span
                      aria-hidden="true"
                      className="absolute top-2 bottom-2 left-0 w-[3px] rounded-full bg-accent"
                    />
                  )}
                  <Icon size={15} strokeWidth={1.9} aria-hidden="true" />
                  {label}
                </button>
              </li>
            );
          })}
        </ul>
      </nav>
      <main ref={main} className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex max-w-[680px] flex-col gap-4 px-10 py-8">
          <StatusBanner />
          {Page ? <Page /> : <WelcomePage onNavigate={setPage} />}
        </div>
      </main>
    </div>
  );
}
