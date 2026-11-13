import {
  Cloud,
  CloudDrizzle,
  CloudFog,
  CloudLightning,
  CloudMoon,
  CloudRain,
  CloudSun,
  CloudSunRain,
  Droplets,
  MapPin,
  Moon,
  RotateCw,
  Snowflake,
  Sun,
  Wind,
  type LucideIcon,
} from "lucide-react";

import {
  describeWeather,
  formatTemperature,
  weekdayOf,
  windUnit,
  type WeatherIconName,
} from "../../features/weather/weather-model";
import { useWeather } from "../../features/weather/use-weather";
import { isNativeAvailable, openSettingsWindow } from "../../services/native";
import { IconButton } from "../ui/IconButton";
import { WidgetSurface } from "../ui/WidgetSurface";

const WEATHER_ICONS: Record<WeatherIconName, LucideIcon> = {
  clear: Sun,
  clearNight: Moon,
  partly: CloudSun,
  partlyNight: CloudMoon,
  cloudy: Cloud,
  fog: CloudFog,
  drizzle: CloudDrizzle,
  rain: CloudRain,
  snow: Snowflake,
  showers: CloudSunRain,
  thunder: CloudLightning,
};

const chip =
  "rounded-full border border-hairline px-3 py-1 text-caption font-medium text-fg-muted transition-colors duration-fast hover:bg-surface-hover hover:text-fg";

function Message({ text, action }: { text: string; action?: React.ReactNode }) {
  return (
    <div className="flex flex-1 flex-col items-start justify-center gap-2">
      <p className="text-body text-fg-muted">{text}</p>
      {action}
    </div>
  );
}

/**
 * Current conditions and a three-day outlook for the place chosen in
 * Settings › Widgets (Open-Meteo; nothing is requested without a place).
 */
export function WeatherWidget({ className }: { className?: string }) {
  const { weather, retry } = useWeather();
  const chooseButton = isNativeAvailable() ? (
    <button type="button" className={chip} onClick={() => void openSettingsWindow("widgets")}>
      Choose a place
    </button>
  ) : null;

  if (!weather || weather.status === "loading") {
    return (
      <WidgetSurface title="Weather" className={className}>
        <Message text="Loading…" />
      </WidgetSurface>
    );
  }
  if (weather.status === "unconfigured") {
    return (
      <WidgetSurface title="Weather" className={className}>
        <Message text="Pick a place to see its weather." action={chooseButton} />
      </WidgetSurface>
    );
  }

  const place = (
    <span className="flex max-w-[120px] items-center gap-1 text-caption text-fg-muted">
      <MapPin size={11} strokeWidth={2} aria-hidden="true" className="shrink-0" />
      <span className="truncate">{weather.location.name}</span>
    </span>
  );

  if (weather.status === "error") {
    return (
      <WidgetSurface title="Weather" className={className} accessory={place}>
        <Message
          text={weather.message}
          action={
            <button type="button" className={chip} onClick={retry}>
              Try again
            </button>
          }
        />
      </WidgetSurface>
    );
  }

  const { current, days } = weather.forecast;
  const condition = describeWeather(current.code, current.isDay);
  const Icon = WEATHER_ICONS[condition.icon];
  const today = days[0];
  const later = days.slice(1, 4);

  return (
    <WidgetSurface title="Weather" className={className} accessory={place}>
      <div className="flex min-h-0 flex-1 flex-col justify-between gap-2">
        <div className="flex items-center gap-3">
          <Icon size={34} strokeWidth={1.6} aria-hidden="true" className="shrink-0 text-fg" />
          <div className="flex min-w-0 flex-col">
            <span className="text-[26px] leading-none font-semibold tracking-tight text-fg tabular-nums">
              {formatTemperature(current.temperature)}
            </span>
            <span className="truncate text-caption text-fg-muted">
              {condition.label}
              {today
                ? ` · H ${formatTemperature(today.high)} L ${formatTemperature(today.low)}`
                : ""}
            </span>
          </div>
          {weather.stale && (
            <span className="ml-auto">
              <IconButton
                icon={RotateCw}
                size="sm"
                label="Couldn't refresh. Try again"
                onClick={retry}
              />
            </span>
          )}
        </div>
        <p className="flex gap-3 text-caption text-fg-subtle tabular-nums">
          <span className="flex items-center gap-1">
            Feels {formatTemperature(current.feelsLike)}
          </span>
          {current.humidity !== null && (
            <span className="flex items-center gap-1">
              <Droplets size={11} strokeWidth={2} aria-hidden="true" />
              {Math.round(current.humidity)}%
            </span>
          )}
          {current.windSpeed !== null && (
            <span className="flex items-center gap-1">
              <Wind size={11} strokeWidth={2} aria-hidden="true" />
              {Math.round(current.windSpeed)} {windUnit(weather.unit)}
            </span>
          )}
        </p>
        {later.length > 0 && (
          <ul
            aria-label="Next days"
            className="grid grid-cols-3 gap-1 border-t border-hairline pt-2"
          >
            {later.map((day) => {
              const c = describeWeather(day.code);
              const DayIcon = WEATHER_ICONS[c.icon];
              return (
                <li
                  key={day.date}
                  className="flex flex-col items-center gap-0.5"
                  aria-label={`${weekdayOf(day.date)}: ${c.label}, high ${formatTemperature(day.high)}, low ${formatTemperature(day.low)}`}
                >
                  <span className="text-caption text-fg-muted">{weekdayOf(day.date)}</span>
                  <DayIcon size={15} strokeWidth={1.8} aria-hidden="true" className="text-fg" />
                  <span className="text-caption text-fg tabular-nums">
                    {formatTemperature(day.high)}
                    <span className="text-fg-subtle"> {formatTemperature(day.low)}</span>
                  </span>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </WidgetSurface>
  );
}
