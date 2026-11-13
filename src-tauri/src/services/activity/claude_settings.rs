//! Adds and removes Islandy's hooks in the *user-level* Claude Code settings
//! (`~/.claude/settings.json`, or `$CLAUDE_CONFIG_DIR/settings.json`).
//!
//! Rules:
//! - Only on an explicit user action (the integration panel's buttons).
//! - Project settings (`.claude/settings*.json`) are never touched.
//! - Only Islandy-managed handlers are added or removed. They are
//!   recognized exactly: `type: "command"`, `args: ["hook"]`, and a command
//!   whose file name is `islandy` / `islandy.exe`. Everything else (other
//!   hooks, matchers, unrelated settings, key order) is preserved.
//! - A malformed file, or a `hooks` value of an unexpected shape, is never
//!   modified; the user gets the manual snippet instead.
//! - Before writing, the original is copied to a timestamped backup, the
//!   file is re-read to detect a concurrent change, and the new content is
//!   written atomically (temporary file + rename).
//!
//! Handlers use the exec form (`command` + `args`, no shell), so paths with
//! spaces need no quoting under Git Bash or PowerShell, and `async: true` so
//! Claude Code never waits for Islandy — except `PermissionRequest`, which
//! runs synchronously so an answer given on the island can reach Claude
//! Code. It returns at once unless "Answer permission requests" is on, and
//! then waits at most `protocol::ANSWER_WAIT`.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Map, Value};

use super::protocol::ClaudeHook;
use crate::services::paths;

const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;
const BACKUP_PREFIX: &str = "settings.islandy-backup-";
const KEEP_BACKUPS: usize = 5;
/// Seconds; only meaningful if a Claude Code version ignores `async`.
const HOOK_TIMEOUT_SECS: u64 = 10;
/// Above `protocol::ANSWER_WAIT` (30 s), so the bridge always gives up first.
const PERMISSION_TIMEOUT_SECS: u64 = 45;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationState {
    /// No `~/.claude` directory: Claude Code doesn't seem to be set up.
    NotDetected,
    NotInstalled,
    Installed,
    /// Some events are missing, or the hooks point at another Islandy path.
    NeedsUpdate,
    /// The settings file can't be parsed or has an unexpected shape.
    Unreadable,
}

/// What the integration panel shows. Mirrored in `src/services/types.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeIntegration {
    pub state: IntegrationState,
    pub settings_path: Option<String>,
    /// Hook events Islandy installs.
    pub events: Vec<&'static str>,
    /// The exact JSON Islandy adds, for review or manual setup.
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntegrationError {
    #[error("Claude Code settings folder not found")]
    NotDetected,
    #[error("the Claude Code settings file is not valid JSON or has an unexpected shape")]
    Malformed,
    #[error("the settings file changed while Islandy was updating it")]
    Conflict,
    #[error("could not determine Islandy's own path")]
    NoExecutable,
    #[error("settings file error: {0}")]
    Io(String),
}

impl IntegrationError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NotDetected => "notFound",
            Self::Malformed => "invalid",
            Self::Conflict => "rejected",
            Self::NoExecutable | Self::Io(_) => "platform",
        }
    }
}

impl From<std::io::Error> for IntegrationError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.kind().to_string())
    }
}

/* -------------------------------------------------------------------------- */
/* Locations                                                                  */
/* -------------------------------------------------------------------------- */

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

/// Claude Code's user configuration directory.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = env_dir("CLAUDE_CONFIG_DIR") {
        return Some(dir);
    }
    let home = if cfg!(windows) {
        env_dir("USERPROFILE").or_else(|| env_dir("HOME"))
    } else {
        env_dir("HOME")
    };
    home.map(|h| h.join(".claude"))
}

/// The handler command: Islandy's own executable, with `/` separators on
/// Windows (accepted by Windows, Git Bash and PowerShell alike).
pub fn hook_command(exe: &Path) -> String {
    let text = exe.to_string_lossy().into_owned();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}

/* -------------------------------------------------------------------------- */
/* Pure JSON edits                                                            */
/* -------------------------------------------------------------------------- */

fn handler(command: &str, hook: ClaudeHook) -> Value {
    if hook == ClaudeHook::PermissionRequest {
        return json!({
            "type": "command",
            "command": command,
            "args": ["hook"],
            "timeout": PERMISSION_TIMEOUT_SECS,
        });
    }
    json!({
        "type": "command",
        "command": command,
        "args": ["hook"],
        "async": true,
        "timeout": HOOK_TIMEOUT_SECS,
    })
}

fn group(command: &str, hook: ClaudeHook) -> Value {
    json!({ "hooks": [handler(command, hook)] })
}

/// Whether a handler object is one Islandy installed.
pub fn is_managed(handler: &Value) -> bool {
    let Some(h) = handler.as_object() else {
        return false;
    };
    let is_command = h.get("type").and_then(Value::as_str) == Some("command");
    let args_hook = h
        .get("args")
        .and_then(Value::as_array)
        .is_some_and(|a| a.len() == 1 && a[0].as_str() == Some("hook"));
    let islandy = h
        .get("command")
        .and_then(Value::as_str)
        .and_then(|c| c.rsplit(['/', '\\']).next())
        .is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name == "islandy" || name == "islandy.exe"
        });
    is_command && args_hook && islandy
}

/// The JSON Islandy adds (shown to the user before enabling).
pub fn snippet(command: &str) -> String {
    let mut hooks = Map::new();
    for hook in ClaudeHook::ALL {
        hooks.insert(hook.name().to_owned(), json!([group(command, hook)]));
    }
    serde_json::to_string_pretty(&json!({ "hooks": hooks })).unwrap_or_default()
}

/// Validates the shape Islandy relies on, without changing anything:
/// a root object whose optional `hooks` is an object of arrays of objects,
/// each with an optional `hooks` array.
fn check_shape(root: &Value) -> Result<(), IntegrationError> {
    let root = root.as_object().ok_or(IntegrationError::Malformed)?;
    let Some(hooks) = root.get("hooks") else {
        return Ok(());
    };
    let hooks = hooks.as_object().ok_or(IntegrationError::Malformed)?;
    for groups in hooks.values() {
        for group in groups.as_array().ok_or(IntegrationError::Malformed)? {
            let group = group.as_object().ok_or(IntegrationError::Malformed)?;
            if let Some(handlers) = group.get("hooks") {
                handlers.as_array().ok_or(IntegrationError::Malformed)?;
            }
        }
    }
    Ok(())
}

/// Removes Islandy's handlers from one event's groups. Groups (and the
/// event) are dropped only when *this removal* emptied them.
fn strip_event(groups: &mut Vec<Value>) -> bool {
    let mut changed = false;
    groups.retain_mut(|group| {
        let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        let before = handlers.len();
        handlers.retain(|h| !is_managed(h));
        if handlers.len() == before {
            return true;
        }
        changed = true;
        !handlers.is_empty()
    });
    changed
}

/// Returns the settings with Islandy's hooks present exactly once per event
/// and pointing at `command`.
pub fn apply_install(mut root: Value, command: &str) -> Result<Value, IntegrationError> {
    check_shape(&root)?;
    let obj = root.as_object_mut().ok_or(IntegrationError::Malformed)?;
    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or(IntegrationError::Malformed)?;
    for hook in ClaudeHook::ALL {
        let groups = hooks
            .entry(hook.name())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or(IntegrationError::Malformed)?;
        let wanted = group(command, hook);
        let current = groups.iter().filter(|g| contains_managed(g)).count();
        if current == 1 && groups.contains(&wanted) {
            continue; // already exactly right: leave the array untouched
        }
        strip_event(groups);
        groups.push(wanted);
    }
    Ok(root)
}

fn contains_managed(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hs| hs.iter().any(is_managed))
}

/// Returns the settings without any Islandy handler.
pub fn apply_remove(mut root: Value) -> Result<Value, IntegrationError> {
    check_shape(&root)?;
    let Some(obj) = root.as_object_mut() else {
        return Err(IntegrationError::Malformed);
    };
    let Some(hooks) = obj.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok(root);
    };
    let mut emptied = Vec::new();
    for (event, groups) in hooks.iter_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        if strip_event(groups) && groups.is_empty() {
            emptied.push(event.clone());
        }
    }
    let removed_any = !emptied.is_empty();
    for event in emptied {
        hooks.remove(&event);
    }
    if removed_any && hooks.is_empty() {
        obj.remove("hooks");
    }
    Ok(root)
}

/// Installation state of parsed settings for `command`.
pub fn inspect(root: &Value, command: &str) -> IntegrationState {
    if check_shape(root).is_err() {
        return IntegrationState::Unreadable;
    }
    let hooks = root.get("hooks").and_then(Value::as_object);
    let mut exact = 0;
    let mut any = 0;
    for hook in ClaudeHook::ALL {
        let groups = hooks
            .and_then(|h| h.get(hook.name()))
            .and_then(Value::as_array);
        let managed: Vec<&Value> = groups
            .into_iter()
            .flatten()
            .filter_map(|g| g.get("hooks").and_then(Value::as_array))
            .flatten()
            .filter(|h| is_managed(h))
            .collect();
        any += usize::from(!managed.is_empty());
        exact += usize::from(managed.len() == 1 && *managed[0] == handler(command, hook));
    }
    match (any, exact) {
        (0, _) => IntegrationState::NotInstalled,
        (_, e) if e == ClaudeHook::ALL.len() => IntegrationState::Installed,
        _ => IntegrationState::NeedsUpdate,
    }
}

/* -------------------------------------------------------------------------- */
/* Files                                                                      */
/* -------------------------------------------------------------------------- */

pub struct Location {
    pub dir: PathBuf,
    pub file: PathBuf,
}

impl Location {
    pub fn user() -> Option<Self> {
        config_dir().map(|dir| Self {
            file: dir.join("settings.json"),
            dir,
        })
    }
}

/// Reads the settings: `None` bytes when the file doesn't exist yet.
fn read(file: &Path) -> Result<(Option<Vec<u8>>, Value), IntegrationError> {
    match std::fs::metadata(file) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((None, Value::Object(Map::new())))
        }
        Err(e) => return Err(e.into()),
        Ok(meta) if meta.len() > MAX_SETTINGS_BYTES => return Err(IntegrationError::Malformed),
        Ok(_) => {}
    }
    let bytes = std::fs::read(file)?;
    // An empty file is treated like a missing one.
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok((Some(bytes), Value::Object(Map::new())));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| IntegrationError::Malformed)?;
    Ok((Some(bytes), value))
}

pub fn status(location: Option<&Location>, command: &str) -> ClaudeIntegration {
    let info = |state, path: Option<&Path>| ClaudeIntegration {
        state,
        settings_path: path.map(|p| p.to_string_lossy().into_owned()),
        events: ClaudeHook::ALL.iter().map(|h| h.name()).collect(),
        snippet: snippet(command),
    };
    let Some(loc) = location else {
        return info(IntegrationState::NotDetected, None);
    };
    if !loc.dir.is_dir() {
        return info(IntegrationState::NotDetected, Some(&loc.file));
    }
    let state = match read(&loc.file) {
        Ok((_, value)) => inspect(&value, command),
        Err(_) => IntegrationState::Unreadable,
    };
    info(state, Some(&loc.file))
}

fn write_with_backup(
    loc: &Location,
    original: Option<&[u8]>,
    value: &Value,
    now_ms: u64,
) -> Result<(), IntegrationError> {
    let mut text = serde_json::to_string_pretty(value).map_err(|_| IntegrationError::Malformed)?;
    text.push('\n');
    if let Some(bytes) = original {
        let backup = loc.dir.join(format!("{BACKUP_PREFIX}{now_ms}.json"));
        paths::write_atomic(&backup, bytes)?;
        prune_backups(&loc.dir);
    }
    // Abort if Claude Code (or the user) changed the file meanwhile.
    let current = match std::fs::read(&loc.file) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    if current.as_deref() != original {
        return Err(IntegrationError::Conflict);
    }
    #[cfg(unix)]
    let permissions = std::fs::metadata(&loc.file).ok().map(|m| m.permissions());
    paths::write_atomic(&loc.file, text.as_bytes())?;
    #[cfg(unix)]
    if let Some(p) = permissions {
        let _ = std::fs::set_permissions(&loc.file, p);
    }
    Ok(())
}

fn prune_backups(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(BACKUP_PREFIX))
        })
        .collect();
    // Timestamps have the same width for centuries: lexical order is age.
    backups.sort();
    let excess = backups.len().saturating_sub(KEEP_BACKUPS);
    for old in backups.into_iter().take(excess) {
        let _ = std::fs::remove_file(old);
    }
}

/// Installs (or repairs) Islandy's hooks. Returns the new status.
pub fn install(
    loc: &Location,
    command: &str,
    now_ms: u64,
) -> Result<ClaudeIntegration, IntegrationError> {
    if !loc.dir.is_dir() {
        return Err(IntegrationError::NotDetected);
    }
    let (original, value) = read(&loc.file)?;
    let updated = apply_install(value.clone(), command)?;
    if updated != value || original.is_none() {
        write_with_backup(loc, original.as_deref(), &updated, now_ms)?;
    }
    Ok(status(Some(loc), command))
}

/// Removes Islandy's hooks (and nothing else). Returns the new status.
pub fn remove(
    loc: &Location,
    command: &str,
    now_ms: u64,
) -> Result<ClaudeIntegration, IntegrationError> {
    if !loc.file.exists() {
        return Ok(status(Some(loc), command));
    }
    let (original, value) = read(&loc.file)?;
    let updated = apply_remove(value.clone())?;
    if updated != value {
        write_with_backup(loc, original.as_deref(), &updated, now_ms)?;
    }
    Ok(status(Some(loc), command))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = "C:/Users/Ada Lovelace/AppData/Local/Islandy/islandy.exe";

    fn user_settings() -> Value {
        json!({
            "model": "opus",
            "permissions": { "allow": ["Bash(npm test)"] },
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Write|Edit",
                        "hooks": [{ "type": "command", "command": "prettier --write", "timeout": 30 }]
                    }
                ],
                "PreCompact": [{ "hooks": [{ "type": "command", "command": "echo hi" }] }]
            },
            "statusLine": { "type": "command", "command": "~/.claude/status.sh" }
        })
    }

    #[test]
    fn install_adds_one_handler_per_event_and_preserves_everything_else() {
        let installed = apply_install(user_settings(), CMD).unwrap();
        assert_eq!(inspect(&installed, CMD), IntegrationState::Installed);
        // Unrelated settings and hooks are untouched, in order.
        let keys: Vec<_> = installed.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["model", "permissions", "hooks", "statusLine"]);
        assert_eq!(installed["permissions"], user_settings()["permissions"]);
        let post = installed["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(post[0], user_settings()["hooks"]["PostToolUse"][0]);
        assert_eq!(post.len(), 2);
        assert_eq!(
            installed["hooks"]["PreCompact"],
            user_settings()["hooks"]["PreCompact"]
        );
        // Exec form, async, no matcher (all tools).
        let ours = &post[1]["hooks"][0];
        assert_eq!(ours["command"], CMD);
        assert_eq!(ours["args"], json!(["hook"]));
        assert_eq!(ours["async"], true);
        assert!(post[1].get("matcher").is_none());
        for hook in ClaudeHook::ALL {
            assert!(
                installed["hooks"][hook.name()].is_array(),
                "{}",
                hook.name()
            );
        }
    }

    #[test]
    fn install_is_idempotent_and_repairs_a_moved_executable() {
        let once = apply_install(user_settings(), CMD).unwrap();
        let twice = apply_install(once.clone(), CMD).unwrap();
        assert_eq!(once, twice);

        let moved = "D:/Apps/Islandy/islandy.exe";
        assert_eq!(inspect(&once, moved), IntegrationState::NeedsUpdate);
        let repaired = apply_install(once, moved).unwrap();
        assert_eq!(inspect(&repaired, moved), IntegrationState::Installed);
        let post = repaired["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(post.len(), 2, "the old handler is replaced, not duplicated");
        assert_eq!(post[1]["hooks"][0]["command"], moved);
    }

    #[test]
    fn removal_restores_the_original_exactly() {
        let installed = apply_install(user_settings(), CMD).unwrap();
        let removed = apply_remove(installed).unwrap();
        assert_eq!(removed, user_settings());
        assert_eq!(inspect(&removed, CMD), IntegrationState::NotInstalled);
        // From a file without hooks: the hooks key disappears again.
        let bare = json!({ "model": "opus" });
        let removed = apply_remove(apply_install(bare.clone(), CMD).unwrap()).unwrap();
        assert_eq!(removed, bare);
    }

    #[test]
    fn removal_keeps_user_handlers_that_share_a_group() {
        let mut settings = apply_install(json!({}), CMD).unwrap();
        settings["hooks"]["Stop"][0]["hooks"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "type": "command", "command": "say done" }));
        let removed = apply_remove(settings).unwrap();
        assert_eq!(
            removed["hooks"]["Stop"],
            json!([{ "hooks": [{ "type": "command", "command": "say done" }] }])
        );
        assert!(removed["hooks"].get("PreToolUse").is_none());
    }

    #[test]
    fn similar_looking_handlers_are_not_ours() {
        for handler in [
            json!({ "type": "command", "command": "islandy hook" }),
            json!({ "type": "command", "command": "/opt/notislandy", "args": ["hook"] }),
            json!({ "type": "command", "command": "/opt/islandy", "args": ["hook", "x"] }),
            json!({ "type": "http", "command": "/opt/islandy", "args": ["hook"] }),
        ] {
            assert!(!is_managed(&handler), "{handler}");
        }
        assert!(is_managed(&handler(
            r"C:\Program Files\Islandy\Islandy.exe",
            ClaudeHook::Stop
        )));
        assert!(is_managed(&handler("/usr/bin/islandy", ClaudeHook::Stop)));
    }

    #[test]
    fn unexpected_shapes_are_never_modified() {
        for bad in [
            json!([]),
            json!({ "hooks": [] }),
            json!({ "hooks": { "Stop": {} } }),
            json!({ "hooks": { "Stop": ["x"] } }),
            json!({ "hooks": { "Stop": [{ "hooks": "x" }] } }),
        ] {
            assert_eq!(
                apply_install(bad.clone(), CMD),
                Err(IntegrationError::Malformed)
            );
            assert_eq!(apply_remove(bad.clone()), Err(IntegrationError::Malformed));
            assert_eq!(inspect(&bad, CMD), IntegrationState::Unreadable);
        }
    }

    #[test]
    fn snippet_lists_every_event() {
        let snippet: Value = serde_json::from_str(&snippet(CMD)).unwrap();
        let events = snippet["hooks"].as_object().unwrap();
        assert_eq!(events.len(), ClaudeHook::ALL.len());
        assert!(events.contains_key("PermissionRequest"));
    }

    fn temp(name: &str) -> Location {
        let dir =
            std::env::temp_dir().join(format!("islandy-claude-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Location {
            file: dir.join("settings.json"),
            dir,
        }
    }

    fn backups(dir: &Path) -> Vec<PathBuf> {
        let mut v: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().contains(BACKUP_PREFIX))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn files_are_backed_up_written_atomically_and_round_trip() {
        let loc = temp("files");
        let original = serde_json::to_string_pretty(&user_settings()).unwrap();
        std::fs::write(&loc.file, &original).unwrap();

        let status = install(&loc, CMD, 1).unwrap();
        assert_eq!(status.state, IntegrationState::Installed);
        assert_eq!(backups(&loc.dir).len(), 1);
        assert_eq!(
            std::fs::read_to_string(&backups(&loc.dir)[0]).unwrap(),
            original
        );
        // Installing again changes nothing (no new backup).
        install(&loc, CMD, 2).unwrap();
        assert_eq!(backups(&loc.dir).len(), 1);

        let status = remove(&loc, CMD, 3).unwrap();
        assert_eq!(status.state, IntegrationState::NotInstalled);
        let after: Value = serde_json::from_slice(&std::fs::read(&loc.file).unwrap()).unwrap();
        assert_eq!(after, user_settings());
        let leftovers: Vec<_> = std::fs::read_dir(&loc.dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with("islandy-tmp"))
            .collect();
        assert!(leftovers.is_empty());
        std::fs::remove_dir_all(&loc.dir).unwrap();
    }

    #[test]
    fn malformed_files_are_left_alone() {
        let loc = temp("malformed");
        std::fs::write(&loc.file, "{ \"hooks\": { // comment\n }").unwrap();
        assert_eq!(status(Some(&loc), CMD).state, IntegrationState::Unreadable);
        assert_eq!(
            install(&loc, CMD, 1).unwrap_err(),
            IntegrationError::Malformed
        );
        assert_eq!(
            remove(&loc, CMD, 1).unwrap_err(),
            IntegrationError::Malformed
        );
        assert_eq!(
            std::fs::read_to_string(&loc.file).unwrap(),
            "{ \"hooks\": { // comment\n }"
        );
        assert!(backups(&loc.dir).is_empty());
        std::fs::remove_dir_all(&loc.dir).unwrap();
    }

    #[test]
    fn missing_files_and_folders() {
        let loc = temp("missing");
        // Folder exists, file doesn't: install creates it (no backup).
        assert_eq!(
            status(Some(&loc), CMD).state,
            IntegrationState::NotInstalled
        );
        install(&loc, CMD, 1).unwrap();
        assert!(loc.file.exists() && backups(&loc.dir).is_empty());
        std::fs::remove_dir_all(&loc.dir).unwrap();
        // No Claude Code folder: nothing is created.
        assert_eq!(status(Some(&loc), CMD).state, IntegrationState::NotDetected);
        assert_eq!(
            install(&loc, CMD, 1).unwrap_err(),
            IntegrationError::NotDetected
        );
        assert!(!loc.dir.exists());
        assert_eq!(status(None, CMD).state, IntegrationState::NotDetected);
    }

    #[test]
    fn concurrent_changes_abort_the_write() {
        let loc = temp("conflict");
        std::fs::write(&loc.file, "{}").unwrap();
        let err = write_with_backup(&loc, Some(b"{ }"), &json!({ "x": 1 }), 1).unwrap_err();
        assert_eq!(err, IntegrationError::Conflict);
        assert_eq!(std::fs::read_to_string(&loc.file).unwrap(), "{}");
        std::fs::remove_dir_all(&loc.dir).unwrap();
    }

    #[test]
    fn backups_are_pruned() {
        let loc = temp("prune");
        std::fs::write(&loc.file, "{}").unwrap();
        for i in 0..8u64 {
            install(&loc, CMD, 1_000 + i).unwrap();
            remove(&loc, CMD, 2_000 + i).unwrap();
        }
        assert_eq!(backups(&loc.dir).len(), KEEP_BACKUPS);
        std::fs::remove_dir_all(&loc.dir).unwrap();
    }

    #[test]
    fn only_the_permission_hook_waits() {
        let root = apply_install(json!({}), CMD).unwrap();
        let permission = &root["hooks"]["PermissionRequest"][0]["hooks"][0];
        assert!(permission.get("async").is_none());
        assert_eq!(permission["timeout"], PERMISSION_TIMEOUT_SECS);
        assert!(PERMISSION_TIMEOUT_SECS > super::super::protocol::ANSWER_WAIT.as_secs());
        assert_eq!(root["hooks"]["Stop"][0]["hooks"][0]["async"], true);
        assert_eq!(inspect(&root, CMD), IntegrationState::Installed);
        // A 0.2 install (all async) needs an update.
        let mut old = root.clone();
        old["hooks"]["PermissionRequest"][0]["hooks"][0]["async"] = json!(true);
        old["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] = json!(HOOK_TIMEOUT_SECS);
        assert_eq!(inspect(&old, CMD), IntegrationState::NeedsUpdate);
    }
}
