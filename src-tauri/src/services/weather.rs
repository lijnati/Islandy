//! The weather widget's data: current conditions and a short forecast from
//! [Open-Meteo](https://open-meteo.com) (free, no account, no API key).
//!
//! Privacy: nothing is requested until the user picks a place in Settings ›
//! Widgets. Requests carry only that place's rounded coordinates (or, for the
//! search, the typed place name) and a generic user agent. Results are kept
//! in memory only. Requests happen on demand, when the dashboard shows the
//! widget and the cached forecast is older than [`FRESH_FOR`]; there is no
//! background polling. "Use my location" reads the device position once,
//! only when that button is clicked.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::{
    error::{IslandyError, Result},
    services::{
        activity::now_ms,
        settings::models::{TemperatureUnit, WeatherLocation},
    },
};

const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
/// A forecast younger than this is shown without a new request.
pub const FRESH_FOR: Duration = Duration::from_secs(30 * 60);
/// After a failed request, wait this long before trying again.
const RETRY_AFTER: Duration = Duration::from_secs(5 * 60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
pub const FORECAST_DAYS: usize = 4;
const MAX_RESULTS: usize = 6;
pub const QUERY_LEN: (usize, usize) = (2, 80);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentWeather {
    pub temperature: f64,
    pub feels_like: f64,
    /// WMO weather interpretation code (0 clear … 99 thunderstorm with hail).
    pub code: u8,
    pub is_day: bool,
    pub humidity: Option<f64>,
    /// km/h for Celsius, mph for Fahrenheit.
    pub wind_speed: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyWeather {
    /// Local date at the place, `YYYY-MM-DD`.
    pub date: String,
    pub code: u8,
    pub high: f64,
    pub low: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    pub current: CurrentWeather,
    pub days: Vec<DailyWeather>,
}

/// Mirrored by `WeatherSnapshot` in `src/services/types.ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum WeatherSnapshot {
    /// No place chosen: nothing is ever requested.
    Unconfigured,
    #[serde(rename_all = "camelCase")]
    Ready {
        location: WeatherLocation,
        unit: TemperatureUnit,
        forecast: Forecast,
        updated_at_ms: u64,
        /// The last refresh failed; this is the previous forecast.
        stale: bool,
    },
    /// The first forecast for this place is being fetched.
    #[serde(rename_all = "camelCase")]
    Loading { location: WeatherLocation },
    #[serde(rename_all = "camelCase")]
    Error {
        location: WeatherLocation,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceResult {
    pub name: String,
    pub detail: String,
    pub latitude: f64,
    pub longitude: f64,
}

/* ------------------------------------------------------------------------ */
/* Open-Meteo wire format                                                    */
/* ------------------------------------------------------------------------ */

#[derive(Deserialize)]
struct ForecastWire {
    current: CurrentWire,
    daily: DailyWire,
}

#[derive(Deserialize)]
struct CurrentWire {
    temperature_2m: f64,
    apparent_temperature: Option<f64>,
    weather_code: f64,
    is_day: Option<f64>,
    relative_humidity_2m: Option<f64>,
    wind_speed_10m: Option<f64>,
}

#[derive(Deserialize)]
struct DailyWire {
    time: Vec<String>,
    weather_code: Vec<Option<f64>>,
    temperature_2m_max: Vec<Option<f64>>,
    temperature_2m_min: Vec<Option<f64>>,
}

#[derive(Deserialize)]
struct GeocodingWire {
    #[serde(default)]
    results: Vec<PlaceWire>,
}

#[derive(Deserialize)]
struct PlaceWire {
    name: String,
    latitude: f64,
    longitude: f64,
    admin1: Option<String>,
    country: Option<String>,
}

fn wmo_code(value: f64) -> u8 {
    if value.is_finite() && (0.0..=99.0).contains(&value) {
        value as u8
    } else {
        0
    }
}

fn parse_forecast(body: &str) -> Option<Forecast> {
    let wire: ForecastWire = serde_json::from_str(body).ok()?;
    let c = wire.current;
    if !c.temperature_2m.is_finite() {
        return None;
    }
    let current = CurrentWeather {
        temperature: c.temperature_2m,
        feels_like: c
            .apparent_temperature
            .filter(|v| v.is_finite())
            .unwrap_or(c.temperature_2m),
        code: wmo_code(c.weather_code),
        is_day: c.is_day.map_or(true, |v| v >= 0.5),
        humidity: c.relative_humidity_2m.filter(|v| v.is_finite()),
        wind_speed: c.wind_speed_10m.filter(|v| v.is_finite()),
    };
    let d = wire.daily;
    let days = d
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, date)| {
            let high = (*d.temperature_2m_max.get(i)?)?;
            let low = (*d.temperature_2m_min.get(i)?)?;
            let code = (*d.weather_code.get(i)?)?;
            (date.len() == 10 && high.is_finite() && low.is_finite()).then(|| DailyWeather {
                date: date.clone(),
                code: wmo_code(code),
                high,
                low,
            })
        })
        .take(FORECAST_DAYS)
        .collect();
    Some(Forecast { current, days })
}

fn parse_places(body: &str) -> Option<Vec<PlaceResult>> {
    let wire: GeocodingWire = serde_json::from_str(body).ok()?;
    Some(
        wire.results
            .into_iter()
            .filter_map(|p| {
                let mut parts: Vec<String> = Vec::new();
                for part in [p.admin1, p.country].into_iter().flatten() {
                    if !part.is_empty() && part != p.name && !parts.contains(&part) {
                        parts.push(part);
                    }
                }
                let place = WeatherLocation {
                    name: p.name,
                    detail: parts.join(", "),
                    latitude: p.latitude,
                    longitude: p.longitude,
                }
                .sanitized()?;
                Some(PlaceResult {
                    name: place.name,
                    detail: place.detail,
                    latitude: place.latitude,
                    longitude: place.longitude,
                })
            })
            .take(MAX_RESULTS)
            .collect(),
    )
}

fn forecast_query(
    location: &WeatherLocation,
    unit: TemperatureUnit,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("latitude", format!("{:.2}", location.latitude)),
        ("longitude", format!("{:.2}", location.longitude)),
        (
            "current",
            "temperature_2m,apparent_temperature,weather_code,is_day,relative_humidity_2m,wind_speed_10m"
                .into(),
        ),
        ("daily", "weather_code,temperature_2m_max,temperature_2m_min".into()),
        ("timezone", "auto".into()),
        ("forecast_days", FORECAST_DAYS.to_string()),
    ];
    if unit == TemperatureUnit::Fahrenheit {
        query.push(("temperature_unit", "fahrenheit".into()));
        query.push(("wind_speed_unit", "mph".into()));
    }
    query
}

/* ------------------------------------------------------------------------ */
/* Service                                                                   */
/* ------------------------------------------------------------------------ */

struct Cached {
    location: WeatherLocation,
    unit: TemperatureUnit,
    forecast: Forecast,
    fetched: Instant,
    updated_at_ms: u64,
}

#[derive(Default)]
struct State {
    cached: Option<Cached>,
    /// When the last request failed, and for which place/unit.
    failed: Option<(WeatherLocation, TemperatureUnit, Instant, String)>,
}

#[derive(Default)]
pub struct WeatherService {
    client: Mutex<Option<reqwest::Client>>,
    state: Mutex<State>,
    in_flight: AtomicBool,
}

fn network_error(e: &reqwest::Error) -> IslandyError {
    let text = if e.is_timeout() {
        "The weather service didn't answer in time."
    } else if e.is_connect() {
        "No connection to the weather service."
    } else if e.is_status() {
        "The weather service returned an error."
    } else {
        "The weather couldn't be loaded."
    };
    IslandyError::Network(text.into())
}

impl WeatherService {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn client(&self) -> Result<reqwest::Client> {
        let mut client = self.client.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = client.as_ref() {
            return Ok(c.clone());
        }
        let built = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("Islandy/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| network_error(&e))?;
        *client = Some(built.clone());
        Ok(built)
    }

    async fn get_text(&self, url: &str, query: &[(&str, String)]) -> Result<String> {
        let response = self
            .client()?
            .get(url)
            .query(query)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| network_error(&e))?;
        response.text().await.map_err(|e| network_error(&e))
    }

    fn ready(c: &Cached, stale: bool) -> WeatherSnapshot {
        WeatherSnapshot::Ready {
            location: c.location.clone(),
            unit: c.unit,
            forecast: c.forecast.clone(),
            updated_at_ms: c.updated_at_ms,
            stale,
        }
    }

    /// The forecast for `location`, from the cache while it is fresh.
    /// `force` skips the freshness check (but not the retry delay).
    pub async fn snapshot(
        &self,
        location: Option<WeatherLocation>,
        unit: TemperatureUnit,
        force: bool,
    ) -> WeatherSnapshot {
        let Some(location) = location else {
            *self.state() = State::default();
            return WeatherSnapshot::Unconfigured;
        };
        {
            let state = self.state();
            let cached = state
                .cached
                .as_ref()
                .filter(|c| c.location == location && c.unit == unit);
            if let Some(c) = cached {
                if !force && c.fetched.elapsed() < FRESH_FOR {
                    return Self::ready(c, false);
                }
            }
            let recent_failure = state
                .failed
                .as_ref()
                .filter(|(l, u, at, _)| *l == location && *u == unit && at.elapsed() < RETRY_AFTER);
            if let Some((_, _, _, message)) = recent_failure {
                return match cached {
                    Some(c) => Self::ready(c, true),
                    None => WeatherSnapshot::Error {
                        location,
                        message: message.clone(),
                    },
                };
            }
            // Another call is already fetching: answer with what we have.
            if self.in_flight.swap(true, Ordering::AcqRel) {
                return match cached {
                    Some(c) => Self::ready(c, false),
                    None => WeatherSnapshot::Loading { location },
                };
            }
        }

        let result = self
            .get_text(FORECAST_URL, &forecast_query(&location, unit))
            .await
            .and_then(|body| {
                parse_forecast(&body).ok_or_else(|| {
                    IslandyError::Network("The weather service sent an unexpected answer.".into())
                })
            });
        self.in_flight.store(false, Ordering::Release);

        let mut state = self.state();
        match result {
            Ok(forecast) => {
                state.failed = None;
                let cached = Cached {
                    location,
                    unit,
                    forecast,
                    fetched: Instant::now(),
                    updated_at_ms: now_ms(),
                };
                let snapshot = Self::ready(&cached, false);
                state.cached = Some(cached);
                snapshot
            }
            Err(e) => {
                let message = e.to_string();
                crate::log_warn!("weather", "forecast request failed: {message}");
                state.failed = Some((location.clone(), unit, Instant::now(), message.clone()));
                match state
                    .cached
                    .as_ref()
                    .filter(|c| c.location == location && c.unit == unit)
                {
                    Some(c) => Self::ready(c, true),
                    None => WeatherSnapshot::Error { location, message },
                }
            }
        }
    }

    /// Place search for Settings (Open-Meteo geocoding).
    pub async fn search(&self, query: &str) -> Result<Vec<PlaceResult>> {
        let query = query.trim();
        let len = query.chars().count();
        if len < QUERY_LEN.0 || len > QUERY_LEN.1 || query.chars().any(char::is_control) {
            return Err(IslandyError::InvalidArgument("place name"));
        }
        let body = self
            .get_text(
                GEOCODING_URL,
                &[
                    ("name", query.to_owned()),
                    ("count", MAX_RESULTS.to_string()),
                    ("language", "en".into()),
                    ("format", "json".into()),
                ],
            )
            .await?;
        parse_places(&body).ok_or_else(|| {
            IslandyError::Network("The place search sent an unexpected answer.".into())
        })
    }
}

/* ------------------------------------------------------------------------ */
/* "Use my location" (one-shot, only when the user asks)                     */
/* ------------------------------------------------------------------------ */

#[cfg(windows)]
const LOCATION_TIMEOUT: Duration = Duration::from_secs(15);

/// Why the one-shot location read failed (user-facing text).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LocationError {
    #[error(
        "Windows didn't allow Islandy to read your location. Turn on Location in Windows \
         Settings › Privacy & security › Location, including \"Let desktop apps access your \
         location\", then try again."
    )]
    #[cfg_attr(not(windows), allow(dead_code))] // only constructed on Windows
    Denied,
    #[error("Windows didn't answer with a location in time. Try again, or search for a place.")]
    #[cfg_attr(not(windows), allow(dead_code))]
    Timeout,
    #[error("Your location isn't available right now. Search for a place instead.")]
    #[cfg_attr(not(windows), allow(dead_code))]
    Unavailable,
    #[cfg_attr(windows, allow(dead_code))] // only constructed off Windows
    #[error("Location isn't available on this platform. Search for a place instead.")]
    Unsupported,
}

impl LocationError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Denied => "denied",
            Self::Timeout => "timeout",
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Name shown for a place found by location (Open-Meteo has no reverse
/// geocoding, and none is added).
#[cfg_attr(not(windows), allow(dead_code))] // only used by the Windows reader
pub const CURRENT_LOCATION_NAME: &str = "Current location";

/// A [`PlaceResult`] for coordinates read from the device: rounded to the
/// forecast's precision, named "Current location", with the coordinates as
/// the detail. `None` for non-finite or out-of-range values.
#[cfg_attr(not(windows), allow(dead_code))] // only used by the Windows reader
pub fn place_from_coordinates(latitude: f64, longitude: f64) -> Option<PlaceResult> {
    let place = WeatherLocation {
        name: CURRENT_LOCATION_NAME.into(),
        detail: String::new(),
        latitude,
        longitude,
    }
    .sanitized()?;
    Some(PlaceResult {
        detail: format!("{:.2}, {:.2}", place.latitude, place.longitude),
        name: place.name,
        latitude: place.latitude,
        longitude: place.longitude,
    })
}

#[cfg(windows)]
fn read_position() -> std::result::Result<(f64, f64), LocationError> {
    use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator};

    use crate::services::winrt::{wait, with_mta, WaitError};

    const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;
    let map = |e: WaitError| match e {
        WaitError::Timeout => LocationError::Timeout,
        WaitError::Failed(e) if e.code().0 == E_ACCESSDENIED => LocationError::Denied,
        WaitError::Failed(_) => LocationError::Unavailable,
    };
    with_mta(|| {
        // Asking is what lets Windows show its own prompt, if it needs one.
        let access = wait(Geolocator::RequestAccessAsync(), LOCATION_TIMEOUT).map_err(map)?;
        match access {
            GeolocationAccessStatus::Allowed => {}
            GeolocationAccessStatus::Denied => return Err(LocationError::Denied),
            _ => return Err(LocationError::Unavailable),
        }
        // The default accuracy is the coarse one: no GPS is forced on.
        let locator = Geolocator::new().map_err(|_| LocationError::Unavailable)?;
        let position = wait(locator.GetGeopositionAsync(), LOCATION_TIMEOUT).map_err(map)?;
        let point = position
            .Coordinate()
            .and_then(|c| c.Point())
            .and_then(|p| p.Position())
            .map_err(|_| LocationError::Unavailable)?;
        Ok((point.Latitude, point.Longitude))
    })
}

/// Reads the device location once, on a blocking thread. Only called from
/// the "Use my location" button; nothing is stored or refreshed here.
pub async fn locate() -> Result<PlaceResult> {
    #[cfg(windows)]
    {
        let (latitude, longitude) = tauri::async_runtime::spawn_blocking(read_position)
            .await
            .map_err(|_| IslandyError::from(LocationError::Unavailable))?
            .map_err(IslandyError::from)?;
        place_from_coordinates(latitude, longitude).ok_or(LocationError::Unavailable.into())
    }
    #[cfg(not(windows))]
    {
        Err(LocationError::Unsupported.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_rounds_a_located_place() {
        let p = place_from_coordinates(9.031_234, 38.768_9).unwrap();
        assert_eq!(p.name, "Current location");
        assert_eq!((p.latitude, p.longitude), (9.03, 38.77));
        assert_eq!(p.detail, "9.03, 38.77");
        assert!(place_from_coordinates(f64::NAN, 0.0).is_none());
        assert!(place_from_coordinates(91.0, 0.0).is_none());
    }

    #[test]
    fn location_errors_have_frontend_kinds() {
        assert_eq!(LocationError::Denied.kind(), "denied");
        assert!(LocationError::Denied
            .to_string()
            .contains("Let desktop apps"));
        assert_eq!(LocationError::Timeout.kind(), "timeout");
        assert_eq!(LocationError::Unsupported.kind(), "unsupported");
    }

    // Recorded from api.open-meteo.com (trimmed).
    const FORECAST: &str = r#"{"latitude":9.03,"longitude":38.77,"timezone":"Africa/Addis_Ababa",
        "current":{"time":"2026-11-13T07:45","interval":900,"temperature_2m":12.8,
          "apparent_temperature":11.2,"weather_code":45,"is_day":1,
          "relative_humidity_2m":81,"wind_speed_10m":7.4},
        "daily":{"time":["2026-11-13","2026-11-14","2026-11-15","2026-11-16"],
          "weather_code":[51,45,1,3],"temperature_2m_max":[20.1,20.7,21.7,21.7],
          "temperature_2m_min":[10.7,6.2,8.5,8.5]}}"#;

    #[test]
    fn parses_a_forecast() {
        let f = parse_forecast(FORECAST).unwrap();
        assert_eq!(f.current.temperature, 12.8);
        assert_eq!(f.current.feels_like, 11.2);
        assert_eq!(f.current.code, 45);
        assert!(f.current.is_day);
        assert_eq!(f.current.humidity, Some(81.0));
        assert_eq!(f.days.len(), 4);
        assert_eq!(
            f.days[0],
            DailyWeather {
                date: "2026-11-13".into(),
                code: 51,
                high: 20.1,
                low: 10.7
            }
        );
    }

    #[test]
    fn tolerates_missing_and_null_values() {
        let body = r#"{"current":{"temperature_2m":-3.5,"weather_code":71,"is_day":0},
            "daily":{"time":["2026-01-01","2026-01-02"],"weather_code":[71,null],
              "temperature_2m_max":[0.5,1.0],"temperature_2m_min":[-5.0,-4.0]}}"#;
        let f = parse_forecast(body).unwrap();
        assert_eq!(f.current.feels_like, -3.5);
        assert!(!f.current.is_day);
        assert_eq!(f.current.wind_speed, None);
        assert_eq!(f.days.len(), 1, "a day with a null value is skipped");
        assert!(parse_forecast("{}").is_none());
        assert!(parse_forecast("not json").is_none());
        assert_eq!(wmo_code(250.0), 0);
        assert_eq!(wmo_code(f64::NAN), 0);
    }

    #[test]
    fn parses_places_with_a_short_detail() {
        let body = r#"{"results":[
            {"id":344979,"name":"Addis Ababa","latitude":9.02497,"longitude":38.74689,
             "country":"Ethiopia","admin1":"Addis Ababa"},
            {"id":1,"name":"Springfield","latitude":39.80172,"longitude":-89.64371,
             "country":"United States","admin1":"Illinois"},
            {"id":2,"name":"Nowhere","latitude":95.0,"longitude":0.0}]}"#;
        let places = parse_places(body).unwrap();
        assert_eq!(places.len(), 2, "out-of-range coordinates are dropped");
        assert_eq!(
            places[0].detail, "Ethiopia",
            "region equal to the name is omitted"
        );
        assert_eq!((places[0].latitude, places[0].longitude), (9.02, 38.75));
        assert_eq!(places[1].detail, "Illinois, United States");
        assert_eq!(parse_places(r#"{"generationtime_ms":0.1}"#).unwrap(), []);
    }

    #[test]
    fn requests_use_rounded_coordinates_and_the_unit() {
        let place = WeatherLocation {
            name: "X".into(),
            detail: String::new(),
            latitude: 9.02,
            longitude: -38.75,
        };
        let q = forecast_query(&place, TemperatureUnit::Fahrenheit);
        assert!(q.contains(&("latitude", "9.02".into())));
        assert!(q.contains(&("longitude", "-38.75".into())));
        assert!(q.contains(&("temperature_unit", "fahrenheit".into())));
        let q = forecast_query(&place, TemperatureUnit::Celsius);
        assert!(!q.iter().any(|(k, _)| *k == "temperature_unit"));
    }

    #[test]
    fn nothing_is_requested_without_a_place() {
        let service = WeatherService::default();
        let snapshot =
            tauri::async_runtime::block_on(service.snapshot(None, TemperatureUnit::Celsius, true));
        assert_eq!(snapshot, WeatherSnapshot::Unconfigured);
        assert!(
            service.client.lock().unwrap().is_none(),
            "no client was even built"
        );
    }

    #[test]
    fn rejects_bad_search_queries() {
        let service = WeatherService::default();
        for q in ["", " a ", "\u{7}bell", &"x".repeat(81)] {
            assert!(tauri::async_runtime::block_on(service.search(q)).is_err());
        }
    }
}
