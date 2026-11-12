//! `settings.json` in Islandy's per-user data directory.
//!
//! - Writes are atomic (temporary file + rename, see `paths::write_atomic`).
//! - A file that isn't valid JSON (or is absurdly large) is moved to
//!   `settings.corrupt.json` and defaults are used; nothing is lost silently.
//! - Older formats are migrated step by step ([`MIGRATIONS`]).
//! - A file written by a *newer* Islandy is copied to
//!   `settings.v<N>.backup.json` before this version first overwrites it.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::models::{Settings, SETTINGS_VERSION};
use crate::services::paths;

pub const FILE: &str = "settings.json";
pub const CORRUPT_FILE: &str = "settings.corrupt.json";
const MAX_FILE_BYTES: u64 = 256 * 1024;

/// One migration step: rewrites a document of version `n` (the index + 1)
/// into version `n + 1`. Version 1 is the first released format, so the
/// list is empty for now.
type Migration = fn(&mut serde_json::Map<String, Value>);
pub const MIGRATIONS: &[Migration] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum LoadStatus {
    /// No file yet: defaults.
    Fresh,
    Loaded,
    Migrated {
        from: u32,
    },
    /// The file was unusable; it was moved to `settings.corrupt.json`.
    Recovered,
    /// Written by a newer Islandy; a backup was made before use.
    Newer {
        version: u32,
    },
}

#[derive(Debug)]
pub struct Loaded {
    pub settings: Settings,
    pub status: LoadStatus,
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

fn move_aside(dir: &Path) {
    let _ = std::fs::rename(path(dir), dir.join(CORRUPT_FILE));
}

/// Applies `migrations` to bring `doc` from `from` to the current version.
fn migrate(doc: &mut serde_json::Map<String, Value>, from: u32, migrations: &[Migration]) {
    for (index, step) in migrations.iter().enumerate() {
        let step_from = index as u32 + 1;
        if step_from >= from {
            step(doc);
        }
    }
    doc.insert("version".into(), Value::from(SETTINGS_VERSION));
}

fn load_with(dir: &Path, migrations: &[Migration]) -> Loaded {
    let file = path(dir);
    let Ok(meta) = std::fs::metadata(&file) else {
        return Loaded {
            settings: Settings::default(),
            status: LoadStatus::Fresh,
        };
    };
    let doc = (meta.len() <= MAX_FILE_BYTES)
        .then(|| std::fs::read(&file).ok())
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let Some(Value::Object(mut doc)) = doc else {
        move_aside(dir);
        return Loaded {
            settings: Settings::default(),
            status: LoadStatus::Recovered,
        };
    };
    // A missing version means the first format.
    let version = doc
        .get("version")
        .and_then(Value::as_u64)
        .map_or(1, |v| v.min(u64::from(u32::MAX)) as u32)
        .max(1);
    let status = if version > SETTINGS_VERSION {
        let backup = dir.join(format!("settings.v{version}.backup.json"));
        if !backup.exists() {
            let _ = std::fs::copy(&file, backup);
        }
        LoadStatus::Newer { version }
    } else if version < SETTINGS_VERSION {
        migrate(&mut doc, version, migrations);
        LoadStatus::Migrated { from: version }
    } else {
        LoadStatus::Loaded
    };
    match serde_json::from_value::<Settings>(Value::Object(doc)) {
        Ok(settings) => Loaded {
            settings: settings.normalize(),
            status,
        },
        Err(_) => {
            move_aside(dir);
            Loaded {
                settings: Settings::default(),
                status: LoadStatus::Recovered,
            }
        }
    }
}

pub fn load(dir: &Path) -> Loaded {
    load_with(dir, MIGRATIONS)
}

pub fn save(dir: &Path, settings: &Settings) -> std::io::Result<()> {
    paths::ensure_private_dir(dir)?;
    let bytes = serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?;
    paths::write_atomic(&path(dir), &bytes)
}

#[cfg(test)]
mod tests {
    use super::super::models::{Anchor, Theme};
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "islandy-settings-{name}-{}-{}",
            std::process::id(),
            crate::services::activity::now_ms()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = temp_dir("missing");
        let loaded = load(&dir);
        assert_eq!(loaded.status, LoadStatus::Fresh);
        assert_eq!(loaded.settings, Settings::default());
    }

    #[test]
    fn round_trips() {
        let dir = temp_dir("roundtrip");
        let mut s = Settings::default();
        s.appearance.theme = Theme::Frost;
        s.island.anchor = Anchor::TopRight;
        s.island.offset_x = -40;
        save(&dir, &s).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.status, LoadStatus::Loaded);
        assert_eq!(loaded.settings, s);
        // No temporary file is left behind.
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from(FILE)]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(path(&dir)).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn corrupt_files_are_moved_aside_and_recovered() {
        for (name, content) in [
            ("garbage", &b"{ not json"[..]),
            ("array", &b"[1, 2, 3]"[..]),
            ("empty", &b""[..]),
        ] {
            let dir = temp_dir(name);
            std::fs::write(path(&dir), content).unwrap();
            let loaded = load(&dir);
            assert_eq!(loaded.status, LoadStatus::Recovered, "{name}");
            assert_eq!(loaded.settings, Settings::default());
            assert!(!path(&dir).exists());
            assert_eq!(std::fs::read(dir.join(CORRUPT_FILE)).unwrap(), content);
        }
    }

    #[test]
    fn oversized_files_are_not_parsed() {
        let dir = temp_dir("huge");
        let big = format!("{{\"pad\":\"{}\"}}", "x".repeat(MAX_FILE_BYTES as usize));
        std::fs::write(path(&dir), big).unwrap();
        assert_eq!(load(&dir).status, LoadStatus::Recovered);
    }

    #[test]
    fn partially_invalid_files_keep_their_valid_values() {
        let dir = temp_dir("partial");
        std::fs::write(
            path(&dir),
            r#"{"version":1,"appearance":{"theme":"graphite","opacity":5},"island":{"anchor":"sideways"}}"#,
        )
        .unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.status, LoadStatus::Loaded);
        assert_eq!(loaded.settings.appearance.theme, Theme::Graphite);
        assert_eq!(loaded.settings.appearance.opacity, 70);
        assert_eq!(loaded.settings.island.anchor, Anchor::TopCenter);
    }

    #[test]
    fn migrations_run_in_order_from_the_stored_version() {
        fn v1_to_v2(doc: &mut serde_json::Map<String, Value>) {
            // e.g. a renamed key
            if let Some(v) = doc.remove("look") {
                doc.insert("appearance".into(), v);
            }
        }
        fn v2_to_v3(doc: &mut serde_json::Map<String, Value>) {
            doc.entry("island")
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .unwrap()
                .insert("anchor".into(), "topRight".into());
        }
        let dir = temp_dir("migrate");
        std::fs::write(path(&dir), r#"{"look":{"theme":"frost"}}"#).unwrap();
        let mut doc = serde_json::from_slice::<Value>(&std::fs::read(path(&dir)).unwrap())
            .unwrap()
            .as_object()
            .unwrap()
            .clone();
        migrate(&mut doc, 1, &[v1_to_v2, v2_to_v3]);
        let s: Settings = serde_json::from_value(Value::Object(doc.clone())).unwrap();
        assert_eq!(s.appearance.theme, Theme::Frost);
        assert_eq!(s.island.anchor, Anchor::TopRight);
        // Starting at version 2 skips the first step.
        let mut doc = serde_json::json!({"look": {"theme": "frost"}})
            .as_object()
            .unwrap()
            .clone();
        migrate(&mut doc, 2, &[v1_to_v2, v2_to_v3]);
        assert!(doc.contains_key("look"));
        assert_eq!(doc["version"], SETTINGS_VERSION);
    }

    #[test]
    fn files_from_newer_versions_are_backed_up_once() {
        let dir = temp_dir("newer");
        let original = r#"{"version":7,"appearance":{"theme":"graphite"},"future":{"x":1}}"#;
        std::fs::write(path(&dir), original).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.status, LoadStatus::Newer { version: 7 });
        assert_eq!(loaded.settings.appearance.theme, Theme::Graphite);
        let backup = dir.join("settings.v7.backup.json");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
        save(&dir, &loaded.settings).unwrap();
        // A second load (now version 1) doesn't touch the backup.
        assert_eq!(load(&dir).status, LoadStatus::Loaded);
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
    }
}
