//! Bounded local persistence: `activity.json` in Islandy's per-user data
//! directory. It holds the activity settings and at most
//! [`MAX_EVENTS`](super::engine::MAX_EVENTS) events from the last 7 days, written
//! atomically. Sessions are not persisted (they are rebuilt from live hook
//! events), so nothing stale reappears after a restart.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::model::{ActivityEvent, ActivitySettings};
use crate::services::paths;

pub const FILE: &str = "activity.json";
const CORRUPT_FILE: &str = "activity.corrupt.json";
/// Far above what 300 small events need; anything bigger isn't ours.
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const VERSION: u32 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredRef<'a, I: Serialize> {
    version: u32,
    settings: &'a ActivitySettings,
    events: I,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    version: u32,
    #[serde(default)]
    settings: ActivitySettings,
    #[serde(default)]
    events: Vec<serde_json::Value>,
}

#[derive(Debug, Default)]
pub struct Loaded {
    pub settings: ActivitySettings,
    pub events: Vec<ActivityEvent>,
    /// The file existed but couldn't be used; it was moved aside.
    pub recovered: bool,
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// Loads history. A missing file is an empty history; an unreadable or
/// malformed one is moved to `activity.corrupt.json` and replaced. Single
/// malformed events are skipped rather than failing the whole file.
pub fn load(dir: &Path) -> Loaded {
    let file = path(dir);
    let Ok(meta) = std::fs::metadata(&file) else {
        return Loaded::default();
    };
    let parsed = (meta.len() <= MAX_FILE_BYTES)
        .then(|| std::fs::read(&file).ok())
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok())
        .filter(|s| s.version == VERSION);
    match parsed {
        Some(stored) => Loaded {
            settings: stored.settings,
            events: stored
                .events
                .into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect(),
            recovered: false,
        },
        None => {
            let _ = std::fs::rename(&file, dir.join(CORRUPT_FILE));
            Loaded {
                recovered: true,
                ..Loaded::default()
            }
        }
    }
}

pub fn save<'a>(
    dir: &Path,
    settings: &ActivitySettings,
    events: impl Iterator<Item = &'a ActivityEvent>,
) -> std::io::Result<()> {
    paths::ensure_private_dir(dir)?;
    let events: Vec<&ActivityEvent> = events.collect();
    let json = serde_json::to_vec(&StoredRef {
        version: VERSION,
        settings,
        events,
    })
    .map_err(std::io::Error::other)?;
    paths::write_atomic(&path(dir), &json)
}

/// Deletes the history file (used by "Clear history"; the next save writes
/// an empty one).
pub fn delete(dir: &Path) {
    let _ = std::fs::remove_file(path(dir));
    let _ = std::fs::remove_file(dir.join(CORRUPT_FILE));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::activity::model::{
        ActivitySource, EventKind, Lifecycle, Priority, Severity,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("islandy-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn event(id: &str) -> ActivityEvent {
        ActivityEvent {
            id: id.into(),
            source: ActivitySource::Developer,
            kind: EventKind::BuildFailed,
            session: None,
            project: Some("Islandy".into()),
            title: "Build failed".into(),
            description: None,
            severity: Severity::Error,
            priority: Priority::P1,
            lifecycle: Lifecycle::Resolved,
            timestamp: 1,
            expires_at: None,
            dedupe_key: None,
        }
    }

    #[test]
    fn round_trips_settings_and_events() {
        let dir = temp_dir("roundtrip");
        assert!(load(&dir).events.is_empty());
        let settings = ActivitySettings {
            quiet: true,
            sources: crate::services::activity::model::SourceToggles {
                developer: false,
                ..Default::default()
            },
            peek_duration: crate::services::activity::model::PeekDuration::Long,
            retention_days: 14,
            ..Default::default()
        };
        let events = [event("a1"), event("a2")];
        save(&dir, &settings, events.iter()).unwrap();
        let loaded = load(&dir);
        assert!(!loaded.recovered);
        assert_eq!(loaded.settings, settings);
        assert_eq!(loaded.events, events);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn skips_bad_events_and_recovers_from_corrupt_files() {
        let dir = temp_dir("corrupt");
        paths::ensure_private_dir(&dir).unwrap();
        let good = serde_json::to_value(event("a1")).unwrap();
        let json = serde_json::json!({
            "version": 1,
            "events": [good, { "id": "x", "prompt": "secret" }]
        });
        std::fs::write(path(&dir), serde_json::to_vec(&json).unwrap()).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.settings, ActivitySettings::default());

        std::fs::write(path(&dir), b"{ not json").unwrap();
        let loaded = load(&dir);
        assert!(loaded.recovered && loaded.events.is_empty());
        assert!(!path(&dir).exists());
        assert!(dir.join(CORRUPT_FILE).exists());
        delete(&dir);
        assert!(!dir.join(CORRUPT_FILE).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
