//! The typed preferences schema (`settings.json`).
//!
//! Mirrored by `src/features/settings/settings-schema.ts`. Every value is
//! bounded: deserialization is lenient per field (an invalid value falls back
//! to its default instead of discarding the whole file) and [`Settings::normalize`]
//! clamps numbers and repairs combinations that would make the island
//! impossible to open.
//!
//! Notification preferences live with the activity engine
//! (`activity.json`), which already owns quiet mode and sources; launch at
//! login is read from Windows itself, never stored here; and no secret is
//! ever stored in this file.

use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};

/// Current file format. Bump it and add a step to `storage::MIGRATIONS`
/// when a change can't be expressed with defaults alone.
pub const SETTINGS_VERSION: u32 = 2;

/// Deserializes a settings section field by field on top of its defaults:
/// each stored value that is valid is kept, each invalid one (wrong type,
/// unknown variant, out of range) falls back to that field's default, and
/// unknown keys are ignored. A section that isn't an object is all defaults.
fn section<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Serialize + DeserializeOwned + Default,
{
    use serde_json::Value;
    let incoming = Value::deserialize(deserializer)?;
    let Ok(Value::Object(mut merged)) = serde_json::to_value(T::default()) else {
        return Ok(T::default());
    };
    if let Value::Object(incoming) = incoming {
        for (key, value) in incoming {
            let Some(previous) = merged.get(&key).cloned() else {
                continue;
            };
            merged.insert(key.clone(), value);
            if serde_json::from_value::<T>(Value::Object(merged.clone())).is_err() {
                merged.insert(key, previous);
            }
        }
    }
    Ok(serde_json::from_value(Value::Object(merged)).unwrap_or_default())
}

macro_rules! choice {
    ($(#[$meta:meta])* $name:ident { $default:ident $(, $variant:ident)* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        // `Anchor::TopLeft`… mirror the UI wording.
        #[allow(clippy::enum_variant_names)]
        pub enum $name {
            #[default]
            $default,
            $($variant),*
        }
    };
}

choice!(
    /// Colour scheme of every Islandy surface.
    Theme { Midnight, Graphite, Frost }
);
choice!(
    /// A restrained set of accents (controls, selection, focus); status
    /// colours are not affected.
    Accent { White, Blue, Violet, Green, Amber, Coral }
);
choice!(
    /// Corner radius presets for the dashboard and its widgets.
    Corners { Standard, Compact, Round }
);
choice!(
    /// `full`: springs with a little overshoot. `subtle`: shorter, no overshoot.
    AnimationLevel { Full, Subtle }
);
choice!(
    /// `system` follows Windows' "Animation effects"; `on` always reduces
    /// motion; `off` always animates.
    ReducedMotion { System, On, Off }
);
choice!(
    /// Horizontal anchor of the island on its monitor.
    Anchor { TopCenter, TopLeft, TopRight }
);
choice!(
    /// Size presets of the collapsed pill (and, proportionally, peeks).
    SizePreset { Standard, Compact, Comfortable }
);
choice!(
    /// Temperature unit of the weather widget.
    TemperatureUnit { Celsius, Fahrenheit }
);

/// Dashboard widgets, in the registry's canonical (default) order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WidgetId {
    Media,
    Calendar,
    Activity,
    System,
    Audio,
    Weather,
}

impl WidgetId {
    pub const ALL: [WidgetId; 6] = [
        WidgetId::Media,
        WidgetId::Calendar,
        WidgetId::Activity,
        WidgetId::System,
        WidgetId::Audio,
        WidgetId::Weather,
    ];

    fn enabled_by_default(self) -> bool {
        matches!(self, Self::Media | Self::Calendar | Self::Activity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneralSettings {
    /// Global toggle shortcut (Tauri accelerator syntax). Validated and
    /// registered by `shortcut.rs`; an invalid value falls back to the default.
    pub toggle_shortcut: String,
    /// When Windows starts Islandy at sign-in, keep the island hidden until
    /// it is shown from the tray or with the shortcut.
    pub start_hidden_at_login: bool,
    /// Hide the island while a fullscreen window covers its monitor.
    pub hide_in_fullscreen: bool,
    /// Write debug-level entries to the local log (off by default).
    pub verbose_logging: bool,
}

pub const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+Space";

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            toggle_shortcut: DEFAULT_SHORTCUT.to_owned(),
            start_hidden_at_login: false,
            hide_in_fullscreen: true,
            verbose_logging: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppearanceSettings {
    pub theme: Theme,
    pub accent: Accent,
    /// Island and settings background opacity, in percent.
    pub opacity: u8,
    pub corners: Corners,
    pub animation: AnimationLevel,
    pub reduced_motion: ReducedMotion,
}

pub const OPACITY_RANGE: (u8, u8) = (70, 100);

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            accent: Accent::default(),
            opacity: 100,
            corners: Corners::default(),
            animation: AnimationLevel::default(),
            reduced_motion: ReducedMotion::default(),
        }
    }
}

/// Where the island sits. `monitor` is `"primary"` or a monitor name as
/// reported by Windows (e.g. `\\.\DISPLAY2`); a monitor that is no longer
/// connected falls back to the primary one without losing the choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IslandSettings {
    pub anchor: Anchor,
    /// Horizontal offset from the anchor, logical px (clamped on screen).
    pub offset_x: i32,
    /// Gap between the top of the work area and the island, logical px.
    pub top_margin: u8,
    pub monitor: String,
    pub size: SizePreset,
    /// Alt + drag the collapsed pill to move it horizontally.
    pub drag_to_reposition: bool,
}

pub const PRIMARY_MONITOR: &str = "primary";
pub const OFFSET_LIMIT: i32 = 2000;
pub const TOP_MARGIN_RANGE: (u8, u8) = (0, 48);
const MAX_MONITOR_NAME: usize = 128;

impl Default for IslandSettings {
    fn default() -> Self {
        Self {
            anchor: Anchor::default(),
            offset_x: 0,
            top_margin: 8,
            monitor: PRIMARY_MONITOR.to_owned(),
            size: SizePreset::default(),
            drag_to_reposition: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InteractionSettings {
    pub open_on_hover: bool,
    pub hover_delay_ms: u16,
    pub open_on_click: bool,
    /// A click opens the dashboard pinned (stays until Esc / click outside).
    pub pin_on_click: bool,
    /// A *pinned* dashboard also collapses after the pointer has been away
    /// for [`PINNED_IDLE_MS`]. (An unpinned dashboard always collapses when
    /// the pointer leaves, after `collapse_delay_ms`: it has no keyboard
    /// focus, so nothing else could dismiss it.)
    pub auto_collapse: bool,
    pub collapse_delay_ms: u16,
}

pub const HOVER_DELAY_RANGE: (u16, u16) = (100, 500);
pub const COLLAPSE_DELAY_RANGE: (u16, u16) = (200, 1000);
/// Mirrored by `PINNED_IDLE_MS` in `src/features/island/island-store.ts`.
#[allow(dead_code)]
pub const PINNED_IDLE_MS: u32 = 15_000;

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            open_on_hover: true,
            hover_delay_ms: 150,
            open_on_click: true,
            pin_on_click: true,
            auto_collapse: false,
            collapse_delay_ms: 360,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetEntry {
    pub id: WidgetId,
    pub enabled: bool,
}

/// Dashboard widgets in display order. Always contains every known widget
/// exactly once (see [`Settings::normalize`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WidgetSettings {
    #[serde(deserialize_with = "lenient_entries")]
    pub items: Vec<WidgetEntry>,
}

/// Keeps every valid entry and drops unknown or malformed ones.
fn lenient_entries<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<WidgetEntry>, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .filter_map(|v| WidgetEntry::deserialize(v).ok())
            .collect(),
        _ => Vec::new(),
    })
}

impl Default for WidgetSettings {
    fn default() -> Self {
        Self {
            items: WidgetId::ALL
                .iter()
                .map(|&id| WidgetEntry {
                    id,
                    enabled: id.enabled_by_default(),
                })
                .collect(),
        }
    }
}

/// A place chosen in Settings › Widgets › Weather (from Open-Meteo's
/// geocoding search). Coordinates are stored rounded to two decimals
/// (about 1 km): enough for a forecast, no more precise than needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherLocation {
    pub name: String,
    /// Region and/or country, for display ("Bavaria, Germany").
    pub detail: String,
    pub latitude: f64,
    pub longitude: f64,
}

// Coordinates are always finite and in range (see `normalize`).
impl Eq for WeatherLocation {}

const MAX_PLACE_TEXT: usize = 80;

impl WeatherLocation {
    pub fn round(value: f64) -> f64 {
        (value * 100.0).round() / 100.0
    }

    /// In range, finite and with displayable names; `None` otherwise.
    pub fn sanitized(mut self) -> Option<Self> {
        let clean = |text: &str| -> String {
            text.chars()
                .filter(|c| !c.is_control())
                .take(MAX_PLACE_TEXT)
                .collect::<String>()
                .trim()
                .to_owned()
        };
        self.name = clean(&self.name);
        self.detail = clean(&self.detail);
        let valid = !self.name.is_empty()
            && self.latitude.is_finite()
            && self.longitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && (-180.0..=180.0).contains(&self.longitude);
        valid.then(|| {
            self.latitude = Self::round(self.latitude);
            self.longitude = Self::round(self.longitude);
            self
        })
    }
}

/// The weather widget. No location means no request is ever made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct WeatherSettings {
    pub location: Option<WeatherLocation>,
    pub unit: TemperatureUnit,
}

/// Automatic updates (signed releases from the project's GitHub Releases).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateSettings {
    /// Look for a new version shortly after start and then every few hours.
    pub auto_check: bool,
    /// Download a found update and install it while the PC is idle (the
    /// island collapsed, no input for a few minutes). Off: Islandy only
    /// tells you and waits for "Restart to update".
    pub auto_install: bool,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            auto_check: true,
            auto_install: true,
        }
    }
}

/// First-run state. A fresh install starts with `welcomed: false` and shows
/// the welcome page once; every file written before this section existed
/// (version 1) is migrated to `true` so upgrades never see it
/// (`storage::MIGRATIONS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OnboardingSettings {
    /// The welcome page was finished or skipped.
    pub welcomed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    #[serde(deserialize_with = "section")]
    pub general: GeneralSettings,
    #[serde(deserialize_with = "section")]
    pub appearance: AppearanceSettings,
    #[serde(deserialize_with = "section")]
    pub island: IslandSettings,
    #[serde(deserialize_with = "section")]
    pub interaction: InteractionSettings,
    #[serde(deserialize_with = "section")]
    pub widgets: WidgetSettings,
    #[serde(deserialize_with = "section")]
    pub weather: WeatherSettings,
    #[serde(deserialize_with = "section")]
    pub updates: UpdateSettings,
    #[serde(deserialize_with = "section")]
    pub onboarding: OnboardingSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            general: GeneralSettings::default(),
            appearance: AppearanceSettings::default(),
            island: IslandSettings::default(),
            interaction: InteractionSettings::default(),
            widgets: WidgetSettings::default(),
            weather: WeatherSettings::default(),
            updates: UpdateSettings::default(),
            onboarding: OnboardingSettings::default(),
        }
    }
}

fn clamp<T: Ord>(v: T, (lo, hi): (T, T)) -> T {
    v.clamp(lo, hi)
}

fn valid_monitor_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_MONITOR_NAME && !name.chars().any(|c| c.is_control())
}

impl Settings {
    /// Brings every value into range and repairs invalid combinations.
    /// Idempotent.
    pub fn normalize(mut self) -> Self {
        self.version = SETTINGS_VERSION;

        let g = &mut self.general;
        let shortcut = g.toggle_shortcut.trim();
        g.toggle_shortcut = if shortcut.is_empty() || shortcut.len() > 64 {
            DEFAULT_SHORTCUT.to_owned()
        } else {
            shortcut.to_owned()
        };

        let a = &mut self.appearance;
        a.opacity = clamp(a.opacity, OPACITY_RANGE);

        let i = &mut self.island;
        i.offset_x = i.offset_x.clamp(-OFFSET_LIMIT, OFFSET_LIMIT);
        i.top_margin = clamp(i.top_margin, TOP_MARGIN_RANGE);
        if !valid_monitor_name(&i.monitor) {
            i.monitor = PRIMARY_MONITOR.to_owned();
        }

        let x = &mut self.interaction;
        x.hover_delay_ms = clamp(x.hover_delay_ms, HOVER_DELAY_RANGE);
        x.collapse_delay_ms = clamp(x.collapse_delay_ms, COLLAPSE_DELAY_RANGE);
        // The pointer must be able to open the island one way or another.
        if !x.open_on_hover && !x.open_on_click {
            x.open_on_click = true;
        }

        // Each known widget exactly once: keep the first occurrence, then
        // append any missing widget (e.g. one added in a newer version).
        let mut items: Vec<WidgetEntry> = Vec::with_capacity(WidgetId::ALL.len());
        for entry in self.widgets.items.drain(..) {
            if !items.iter().any(|e| e.id == entry.id) {
                items.push(entry);
            }
        }
        for id in WidgetId::ALL {
            if !items.iter().any(|e| e.id == id) {
                items.push(WidgetEntry {
                    id,
                    enabled: id.enabled_by_default(),
                });
            }
        }
        self.widgets.items = items;

        self.weather.location = self
            .weather
            .location
            .take()
            .and_then(WeatherLocation::sanitized);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_are_normalized() {
        let d = Settings::default();
        assert_eq!(d.clone().normalize(), d);
        assert_eq!(d.general.toggle_shortcut, "Ctrl+Alt+Space");
        assert_eq!(d.widgets.items.len(), WidgetId::ALL.len());
        assert!(d.interaction.open_on_hover && d.interaction.open_on_click);
    }

    /// The TypeScript defaults (`src/features/settings/defaults.ts`) are
    /// checked against the same fixture, so both sides can't drift apart.
    #[test]
    fn defaults_match_the_shared_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../src/features/settings/__fixtures__/default-settings.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(Settings::default()).unwrap(), fixture);
    }

    #[test]
    fn serializes_with_camel_case_keys() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(v["version"], 2);
        assert_eq!(v["onboarding"]["welcomed"], false);
        assert_eq!(v["general"]["toggleShortcut"], "Ctrl+Alt+Space");
        assert_eq!(v["island"]["anchor"], "topCenter");
        assert_eq!(v["island"]["offsetX"], 0);
        assert_eq!(v["interaction"]["hoverDelayMs"], 150);
        assert_eq!(v["appearance"]["reducedMotion"], "system");
        assert_eq!(
            v["widgets"]["items"][0],
            json!({"id": "media", "enabled": true})
        );
    }

    #[test]
    fn invalid_fields_fall_back_individually() {
        let v = json!({
            "version": 2,
            "appearance": { "theme": "neon", "opacity": "lots", "accent": "blue" },
            "island": { "anchor": "topLeft", "topMargin": -4, "offsetX": 1e12 },
            "interaction": { "hoverDelayMs": 5000, "openOnHover": "yes" },
            "general": 42,
        });
        let s: Settings = serde_json::from_value::<Settings>(v).unwrap().normalize();
        assert_eq!(
            s.appearance.theme,
            Theme::Midnight,
            "unknown variant → default"
        );
        assert_eq!(s.appearance.accent, Accent::Blue, "valid sibling kept");
        assert_eq!(s.appearance.opacity, 100, "wrong type → default");
        assert_eq!(s.island.anchor, Anchor::TopLeft);
        assert_eq!(s.island.top_margin, 8, "negative u8 → default");
        assert_eq!(s.island.offset_x, 0, "out of i32 range → default");
        assert_eq!(s.interaction.hover_delay_ms, 500, "clamped");
        assert!(s.interaction.open_on_hover, "wrong type → default");
        assert_eq!(
            s.general,
            GeneralSettings::default(),
            "wrong section type → default"
        );
    }

    #[test]
    fn a_fresh_install_is_not_welcomed_yet() {
        assert!(!Settings::default().onboarding.welcomed);
        // A section that isn't an object, or a wrong type, is "not welcomed".
        let s: Settings =
            serde_json::from_value(json!({"onboarding": {"welcomed": "yes"}})).unwrap();
        assert!(!s.onboarding.welcomed);
        let s: Settings =
            serde_json::from_value(json!({"onboarding": {"welcomed": true}})).unwrap();
        assert!(s.onboarding.welcomed);
    }

    #[test]
    fn numbers_are_clamped() {
        let mut s = Settings::default();
        s.appearance.opacity = 10;
        s.island.top_margin = 200;
        s.island.offset_x = -99_999;
        s.interaction.hover_delay_ms = 0;
        s.interaction.collapse_delay_ms = 60_000;
        let s = s.normalize();
        assert_eq!(s.appearance.opacity, 70);
        assert_eq!(s.island.top_margin, 48);
        assert_eq!(s.island.offset_x, -OFFSET_LIMIT);
        assert_eq!(s.interaction.hover_delay_ms, 100);
        assert_eq!(s.interaction.collapse_delay_ms, 1000);
    }

    #[test]
    fn the_island_always_stays_openable_with_the_pointer() {
        let mut s = Settings::default();
        s.interaction.open_on_hover = false;
        s.interaction.open_on_click = false;
        let s = s.normalize();
        assert!(s.interaction.open_on_click);
        assert!(!s.interaction.open_on_hover);
    }

    #[test]
    fn widgets_are_deduplicated_and_completed() {
        let v = json!({ "widgets": { "items": [
            { "id": "audio", "enabled": true },
            { "id": "radar", "enabled": true },
            { "id": "audio", "enabled": false },
            { "id": "media", "enabled": false },
            "garbage",
        ]}});
        let s = serde_json::from_value::<Settings>(v).unwrap().normalize();
        let ids: Vec<_> = s.widgets.items.iter().map(|e| (e.id, e.enabled)).collect();
        assert_eq!(
            ids,
            [
                (WidgetId::Audio, true),
                (WidgetId::Media, false),
                (WidgetId::Calendar, true),
                (WidgetId::Activity, true),
                (WidgetId::System, false),
                (WidgetId::Weather, false),
            ]
        );
    }

    #[test]
    fn weather_locations_are_validated_and_rounded() {
        let mut s = Settings::default();
        s.weather.location = Some(WeatherLocation {
            name: " Addis\u{7}Ababa ".into(),
            detail: "Ethiopia".into(),
            latitude: 9.024_97,
            longitude: 38.746_89,
        });
        let s = s.normalize();
        let loc = s.weather.location.clone().unwrap();
        assert_eq!(loc.name, "AddisAbaba");
        assert_eq!((loc.latitude, loc.longitude), (9.02, 38.75));
        assert_eq!(s.clone().normalize(), s, "idempotent");

        for (lat, lon) in [(91.0, 0.0), (0.0, -181.0), (f64::NAN, 0.0)] {
            let mut s = Settings::default();
            s.weather.location = Some(WeatherLocation {
                name: "X".into(),
                detail: String::new(),
                latitude: lat,
                longitude: lon,
            });
            assert_eq!(s.normalize().weather.location, None);
        }
        // A malformed stored location falls back to "none", keeping the unit.
        let v = json!({ "weather": { "location": { "name": 3 }, "unit": "fahrenheit" } });
        let s = serde_json::from_value::<Settings>(v).unwrap().normalize();
        assert_eq!(s.weather.location, None);
        assert_eq!(s.weather.unit, TemperatureUnit::Fahrenheit);
    }

    #[test]
    fn bad_shortcut_and_monitor_strings_are_reset() {
        let mut s = Settings::default();
        s.general.toggle_shortcut = "   ".into();
        s.island.monitor = "\u{7}evil".into();
        let s = s.normalize();
        assert_eq!(s.general.toggle_shortcut, DEFAULT_SHORTCUT);
        assert_eq!(s.island.monitor, PRIMARY_MONITOR);
        let mut s = Settings::default();
        s.island.monitor = r"\\.\DISPLAY2".into();
        assert_eq!(s.clone().normalize(), s);
    }
}
