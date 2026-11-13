import { MapPin, Search } from "lucide-react";
import { useId, useState } from "react";

import { focusRing } from "../../../components/ui/IconButton";
import { cx } from "../../../components/ui/cx";
import type { PlaceResult, TemperatureUnit } from "../../../services/types";
import { normalizeLocation } from "../settings-schema";
import { isNativeAvailable, searchWeatherPlaces } from "../settings-service";
import { useSettings, useSettingsStore } from "../settings-store";
import { Button, Row, Section, Segmented, StatusText } from "./controls";

type SearchState =
  | { kind: "idle" }
  | { kind: "searching" }
  | { kind: "results"; places: PlaceResult[] }
  | { kind: "error"; message: string };

const UNITS = [
  { value: "celsius", label: "°C" },
  { value: "fahrenheit", label: "°F" },
] as const satisfies readonly { value: TemperatureUnit; label: string }[];

/** Settings › Widgets › Weather: the place (search) and the unit. */
export function WeatherSettings() {
  const weather = useSettings((s) => s.weather);
  const enabled = useSettings((s) => s.widgets.items.some((w) => w.id === "weather" && w.enabled));
  const update = useSettingsStore((s) => s.update);
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState<SearchState>({ kind: "idle" });
  const inputId = useId();
  const native = isNativeAvailable();

  const setWeather = (next: Partial<typeof weather>) =>
    void update((s) => ({ ...s, weather: { ...s.weather, ...next } }));

  const choose = (place: PlaceResult) => {
    // Picking a place is a clear sign the widget is wanted: turn it on too.
    void update((s) => ({
      ...s,
      weather: { ...s.weather, location: normalizeLocation(place) },
      widgets: enabled
        ? s.widgets
        : {
            items: s.widgets.items.map((w) => (w.id === "weather" ? { ...w, enabled: true } : w)),
          },
    }));
    setSearch({ kind: "idle" });
    setQuery("");
  };

  const runSearch = () => {
    const text = query.trim();
    if (text.length < 2) return;
    setSearch({ kind: "searching" });
    searchWeatherPlaces(text).then(
      (places) => {
        setSearch({ kind: "results", places });
      },
      (cause: unknown) => {
        setSearch({
          kind: "error",
          message: cause instanceof Error ? cause.message.replace(/^[a-z_]+: /, "") : String(cause),
        });
      },
    );
  };

  return (
    <Section
      title="Weather"
      description="Forecasts come from Open-Meteo (no account or key). Islandy sends only the place's coordinates, rounded to about 1 km, and only while the dashboard shows the widget, at most every 30 minutes."
    >
      <Row
        label="Place"
        description={
          weather.location
            ? [
                weather.location.detail,
                `${weather.location.latitude}, ${weather.location.longitude}`,
              ]
                .filter(Boolean)
                .join(" · ")
            : "No place chosen: the widget asks for one and nothing is requested."
        }
        control={
          weather.location ? (
            <span className="flex items-center gap-3">
              <span className="flex items-center gap-1.5 text-body font-medium text-fg">
                <MapPin size={14} strokeWidth={2} aria-hidden="true" />
                {weather.location.name}
              </span>
              <Button
                onClick={() => {
                  setWeather({ location: null });
                }}
              >
                Remove
              </Button>
            </span>
          ) : undefined
        }
      />
      <Row
        label={weather.location ? "Change place" : "Find a place"}
        htmlFor={inputId}
        stacked
        disabled={!native}
        control={
          <div className="flex flex-col gap-2">
            <form
              onSubmit={(event) => {
                event.preventDefault();
                runSearch();
              }}
              className="flex gap-2"
              role="search"
            >
              <div
                className={cx(
                  "flex h-8 min-w-0 flex-1 items-center gap-2 rounded-[8px] border border-hairline bg-surface px-2.5",
                  "focus-within:border-accent",
                )}
              >
                <Search size={14} strokeWidth={2} aria-hidden="true" className="text-fg-subtle" />
                <input
                  id={inputId}
                  type="search"
                  value={query}
                  maxLength={80}
                  disabled={!native}
                  placeholder="City or town, e.g. Addis Ababa"
                  autoComplete="off"
                  spellCheck={false}
                  onChange={(e) => {
                    setQuery(e.target.value);
                  }}
                  className="min-w-0 flex-1 bg-transparent text-body text-fg outline-hidden placeholder:text-fg-subtle"
                />
              </div>
              <Button
                variant="primary"
                busy={search.kind === "searching"}
                disabled={!native || query.trim().length < 2}
                onClick={runSearch}
              >
                Search
              </Button>
            </form>
            {search.kind === "error" && <StatusText tone="error">{search.message}</StatusText>}
            {search.kind === "results" &&
              (search.places.length === 0 ? (
                <StatusText tone="neutral">No place found. Try another spelling.</StatusText>
              ) : (
                <ul
                  aria-label="Places found"
                  className="divide-y divide-hairline rounded-[8px] border border-hairline"
                >
                  {search.places.map((place) => (
                    <li key={`${place.latitude},${place.longitude},${place.name}`}>
                      <button
                        type="button"
                        onClick={() => {
                          choose(place);
                        }}
                        className={cx(
                          "flex w-full items-center justify-between gap-3 px-3 py-2 text-left transition-colors duration-fast hover:bg-surface-hover",
                          focusRing,
                        )}
                      >
                        <span className="flex min-w-0 flex-col">
                          <span className="truncate text-body font-medium text-fg">
                            {place.name}
                          </span>
                          <span className="truncate text-caption text-fg-muted">
                            {place.detail || "—"}
                          </span>
                        </span>
                        <span className="text-caption text-fg-subtle">Use</span>
                      </button>
                    </li>
                  ))}
                </ul>
              ))}
          </div>
        }
      />
      <Row
        label="Temperature unit"
        control={
          <Segmented
            label="Temperature unit"
            value={weather.unit}
            options={UNITS}
            onChange={(unit) => {
              setWeather({ unit });
            }}
          />
        }
      />
    </Section>
  );
}
