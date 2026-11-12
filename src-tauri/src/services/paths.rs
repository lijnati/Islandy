//! Islandy's per-user data directory, shared by the app and its command-line
//! modes (`islandy hook`, `islandy notify`), which run without Tauri.
//!
//! It matches Tauri's `app_local_data_dir()` for the bundle identifier:
//! `%LOCALAPPDATA%\com.islandy.desktop` on Windows,
//! `$XDG_DATA_HOME/com.islandy.desktop` (or `~/.local/share/…`) on Linux.
//! `%LOCALAPPDATA%` lives in the user's profile, whose default ACL grants
//! access only to that user, SYSTEM and administrators.

use std::path::PathBuf;

pub const APP_IDENTIFIER: &str = "com.islandy.desktop";

fn non_empty(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

/// The per-user local data root, or `None` if it can't be determined.
fn local_data_root() -> Option<PathBuf> {
    if cfg!(windows) {
        non_empty("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        non_empty("HOME").map(|h| h.join("Library").join("Application Support"))
    } else {
        non_empty("XDG_DATA_HOME").or_else(|| non_empty("HOME").map(|h| h.join(".local/share")))
    }
}

/// `<local data root>/com.islandy.desktop`.
pub fn data_dir() -> Option<PathBuf> {
    local_data_root().map(|root| root.join(APP_IDENTIFIER))
}

/// Creates `dir` (and parents) readable only by the current user where the
/// platform supports it (Unix mode 0700; on Windows the profile ACL applies).
pub fn ensure_private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir)
    }
}

/// Writes `bytes` to `path` atomically: a sibling temporary file is written,
/// flushed and renamed over the target (Unix files get mode 0600).
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("path has no file name"))?;
    let mut tmp_name = file_name.to_os_string();
    tmp_name.push(".islandy-tmp");
    let tmp = path.with_file_name(tmp_name);
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_the_file_and_leaves_no_temporary() {
        let dir = std::env::temp_dir().join(format!("islandy-paths-{}", std::process::id()));
        ensure_private_dir(&dir).unwrap();
        let path = dir.join("x.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
