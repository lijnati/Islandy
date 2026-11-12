//! Command-line modes of the Islandy executable. They run *instead of* the
//! app (no window, no Tauri) and exit quickly:
//!
//! - `islandy hook`: the Claude Code hook bridge. Reads the hook's JSON from
//!   stdin, keeps a few allow-listed fields, forwards a reduced event to the
//!   running app and exits 0 — always, silently, within about a second.
//! - `islandy notify …`: developer workflow events from scripts.
//! - `islandy status`: checks whether the running app can be reached.
//!
//! None of them can make Islandy do anything but record an activity event.

use std::{
    ffi::OsString,
    io::Read,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use serde_json::Value;

use crate::services::{
    activity::{
        model::{Severity, ToolCategory},
        protocol::{
            self, clean_text, project_from_cwd, session_key, ClaudeHook, ClaudeWire,
            DeveloperEvent, DeveloperWire, NotificationKind, SendError, StopError, WireEvent,
            MAX_LABEL, MAX_PROJECT, MAX_TITLE,
        },
    },
    paths,
};

/// Hook payloads can be large (`tool_response` of a file read). Islandy
/// needs a few small fields; anything beyond this is drained, not parsed.
const MAX_HOOK_INPUT: usize = 8 * 1024 * 1024;

/// Runs a command-line mode if `args` asks for one; `None` means "start the
/// app". Returns the process exit code.
pub fn main(args: &[OsString]) -> Option<i32> {
    let mode = args.get(1)?.to_str()?;
    let rest: Vec<String> = args
        .iter()
        .skip(2)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    match mode {
        "hook" => Some(hook()),
        "notify" => Some(notify(&rest)),
        "status" => Some(status()),
        _ => None,
    }
}

/* -------------------------------------------------------------------------- */
/* Claude Code hook bridge                                                    */
/* -------------------------------------------------------------------------- */

/// The only fields read from a hook payload. Values are kept as loose JSON
/// so an unexpected type can't make the bridge fail; `tool_input`,
/// `tool_response`, `prompt`, `message`, `last_assistant_message`,
/// `transcript_path` and everything else are skipped by the parser.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HookInput {
    hook_event_name: Option<Value>,
    session_id: Option<Value>,
    cwd: Option<Value>,
    tool_name: Option<Value>,
    notification_type: Option<Value>,
    error: Option<Value>,
    agent_id: Option<Value>,
    stop_hook_active: Option<Value>,
}

fn str_field(v: &Option<Value>) -> Option<&str> {
    v.as_ref().and_then(Value::as_str)
}

/// Maps a Claude Code tool name to a coarse category (never the name itself
/// for MCP tools, which can reveal server names).
pub fn tool_category(name: &str) -> ToolCategory {
    if name.starts_with("mcp__") {
        return ToolCategory::Mcp;
    }
    match name {
        "Read" | "NotebookRead" | "LS" | "View" => ToolCategory::Reading,
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => ToolCategory::Editing,
        "Grep" | "Glob" | "ToolSearch" | "LSP" => ToolCategory::Searching,
        "Bash" | "PowerShell" | "BashOutput" | "KillShell" | "KillBash" | "Monitor" => {
            ToolCategory::Running
        }
        "WebFetch" | "WebSearch" => ToolCategory::Web,
        "Task" | "Agent" => ToolCategory::Delegating,
        "TodoWrite" | "TaskCreate" | "TaskUpdate" | "TaskList" | "EnterPlanMode"
        | "ExitPlanMode" => ToolCategory::Planning,
        _ => ToolCategory::Other,
    }
}

/// Reduces a raw hook payload to the wire event, or `None` to ignore it.
pub fn reduce_hook(raw: &[u8]) -> Option<WireEvent> {
    let input: HookInput = serde_json::from_slice(raw).ok()?;
    let event = ClaudeHook::from_name(str_field(&input.hook_event_name)?)?;
    let session_id = str_field(&input.session_id).filter(|s| !s.is_empty() && s.len() <= 256)?;
    // A Stop hook re-running because another Stop hook continued the turn.
    if event == ClaudeHook::Stop
        && input.stop_hook_active.as_ref().and_then(Value::as_bool) == Some(true)
    {
        return None;
    }
    let tool = matches!(
        event,
        ClaudeHook::PreToolUse
            | ClaudeHook::PostToolUse
            | ClaudeHook::PostToolUseFailure
            | ClaudeHook::PermissionRequest
    )
    .then(|| str_field(&input.tool_name).map(tool_category))
    .flatten();
    Some(WireEvent::Claude(ClaudeWire {
        event,
        session: session_key(session_id),
        project: str_field(&input.cwd).and_then(project_from_cwd),
        tool,
        notification: (event == ClaudeHook::Notification)
            .then(|| str_field(&input.notification_type).map(NotificationKind::from_hook))
            .flatten(),
        error: (event == ClaudeHook::StopFailure)
            .then(|| StopError::from_hook(str_field(&input.error).unwrap_or(""))),
        subagent: input.agent_id.is_some(),
    }))
}

/// Reads at most `limit` bytes, then drains (and discards) the rest so the
/// writer never blocks on a full pipe. Returns `None` if input was too big.
fn read_bounded(mut reader: impl Read, limit: usize) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    let mut overflow = false;
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if !overflow && buffer.len() + n <= limit {
                    buffer.extend_from_slice(&chunk[..n]);
                } else {
                    overflow = true;
                    buffer = Vec::new();
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    (!overflow).then_some(buffer)
}

/// Observational only: always exits 0 and prints nothing, so it can never
/// block, alter or clutter a Claude Code session.
fn hook() -> i32 {
    let Some(raw) = read_bounded(std::io::stdin().lock(), MAX_HOOK_INPUT) else {
        return 0;
    };
    if let (Some(event), Some(dir)) = (reduce_hook(&raw), paths::data_dir()) {
        let _ = protocol::send(&dir, &event);
    }
    0
}

/* -------------------------------------------------------------------------- */
/* Developer events                                                           */
/* -------------------------------------------------------------------------- */

pub const NOTIFY_USAGE: &str = "\
usage:
  islandy notify build  started|succeeded|failed  [--project NAME] [--label TEXT]
  islandy notify tests  started|passed|failed     [--project NAME] [--label TEXT]
  islandy notify server started|stopped           [--project NAME] [--label TEXT]
  islandy notify message --title TEXT [--severity info|success|warning|error] [--project NAME]
  islandy notify --title TEXT [--type info|success|warning|error]

The project defaults to the current folder's name. Exit codes: 0 delivered,
2 usage error, 3 Islandy is not running, 4 rejected by Islandy.";

#[derive(Debug, PartialEq, Eq)]
pub struct UsageError(pub String);

fn severity(value: &str) -> Result<Severity, UsageError> {
    match value {
        "info" => Ok(Severity::Info),
        "success" => Ok(Severity::Success),
        "warning" => Ok(Severity::Warning),
        "error" => Ok(Severity::Error),
        other => Err(UsageError(format!("unknown severity `{other}`"))),
    }
}

/// Parses `notify` arguments into a developer event.
pub fn parse_notify(args: &[String], cwd: Option<&Path>) -> Result<DeveloperWire, UsageError> {
    let (positional, flags): (Vec<&String>, Vec<&String>) = {
        let split = args
            .iter()
            .position(|a| a.starts_with("--"))
            .unwrap_or(args.len());
        (
            args[..split].iter().collect(),
            args[split..].iter().collect(),
        )
    };
    let mut project = None;
    let mut label = None;
    let mut title = None;
    let mut sev = None;
    let mut iter = flags.into_iter();
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| UsageError(format!("{flag} needs a value")))?;
        let slot = match flag.as_str() {
            "--project" => &mut project,
            "--label" => &mut label,
            "--title" => &mut title,
            "--severity" | "--type" => &mut sev,
            other => return Err(UsageError(format!("unknown option `{other}`"))),
        };
        if slot.replace(value.clone()).is_some() {
            return Err(UsageError(format!("{flag} given twice")));
        }
    }
    let text = |v: Option<String>, max, name: &str| -> Result<Option<String>, UsageError> {
        v.map(|v| clean_text(&v, max).ok_or_else(|| UsageError(format!("{name} is empty"))))
            .transpose()
    };
    let project = text(project, MAX_PROJECT, "--project")?
        .or_else(|| cwd.and_then(|d| d.to_str()).and_then(project_from_cwd));
    let label = text(label, MAX_LABEL, "--label")?;
    let title = text(title, MAX_TITLE, "--title")?;

    use DeveloperEvent as D;
    let words: Vec<&str> = positional.iter().map(|s| s.as_str()).collect();
    let event = match words.as_slice() {
        ["build", "started"] => D::BuildStarted,
        ["build", "succeeded" | "passed"] => D::BuildSucceeded,
        ["build", "failed"] => D::BuildFailed,
        ["tests" | "test", "started"] => D::TestsStarted,
        ["tests" | "test", "passed" | "succeeded"] => D::TestsPassed,
        ["tests" | "test", "failed"] => D::TestsFailed,
        ["server", "started"] => D::ServerStarted,
        ["server", "stopped"] => D::ServerStopped,
        ["message"] | [] => D::Message,
        _ => return Err(UsageError(format!("unknown event `{}`", words.join(" ")))),
    };
    let message = event == D::Message;
    if message && title.is_none() {
        return Err(UsageError("a message needs --title".into()));
    }
    if !message && (title.is_some() || sev.is_some()) {
        return Err(UsageError("--title/--severity are for messages".into()));
    }
    Ok(DeveloperWire {
        event,
        project,
        label,
        title,
        severity: sev.as_deref().map(severity).transpose()?,
        at: Some(epoch_ms()),
    })
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(0))
}

fn send_exit_code(result: Result<(), SendError>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(SendError::NotRunning | SendError::Unreachable) => {
            eprintln!("islandy: Islandy is not running (or can't be reached)");
            3
        }
        Err(SendError::Rejected(code)) => {
            eprintln!("islandy: rejected by Islandy ({code})");
            4
        }
        Err(SendError::Encode) => {
            eprintln!("islandy: event too large");
            2
        }
    }
}

fn notify(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{NOTIFY_USAGE}");
        return 0;
    }
    let cwd = std::env::current_dir().ok();
    let event = match parse_notify(args, cwd.as_deref()) {
        Ok(e) => e,
        Err(UsageError(message)) => {
            eprintln!("islandy: {message}\n\n{NOTIFY_USAGE}");
            return 2;
        }
    };
    let Some(dir) = paths::data_dir() else {
        return send_exit_code(Err(SendError::NotRunning));
    };
    send_exit_code(protocol::send(&dir, &WireEvent::Developer(event)))
}

fn status() -> i32 {
    let Some(dir) = paths::data_dir() else {
        return send_exit_code(Err(SendError::NotRunning));
    };
    let code = send_exit_code(protocol::send(&dir, &WireEvent::Ping));
    if code == 0 {
        println!("Islandy is running and accepting local events.");
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reduce(v: Value) -> Option<WireEvent> {
        reduce_hook(&serde_json::to_vec(&v).unwrap())
    }

    fn claude(v: Value) -> ClaudeWire {
        match reduce(v) {
            Some(WireEvent::Claude(c)) => c,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn reduces_a_tool_event_to_allow_listed_fields() {
        let secret = "sk-ant-THIS-MUST-NOT-LEAK";
        let payload = json!({
            "session_id": "abc-123",
            "transcript_path": "/home/u/.claude/projects/x/abc.jsonl",
            "cwd": "/home/u/code/Islandy",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": format!("curl -H 'x-api-key: {secret}'") },
            "tool_use_id": "toolu_1",
            "prompt": secret,
        });
        let wire = WireEvent::Claude(claude(payload));
        let text = serde_json::to_string(&wire).unwrap();
        assert!(!text.contains(secret));
        assert!(!text.contains("transcript"));
        assert!(!text.contains("/home"));
        assert!(!text.contains("abc-123"));
        assert_eq!(
            text,
            format!(
                r#"{{"kind":"claude","event":"PreToolUse","session":"{}","project":"Islandy","tool":"running"}}"#,
                session_key("abc-123")
            )
        );
        assert!(protocol::validate(&wire).is_ok());
    }

    #[test]
    fn maps_each_event_conservatively() {
        let base = |name: &str| json!({ "session_id": "s", "cwd": "C:\\code\\App", "hook_event_name": name });
        let mut n = base("Notification");
        n["notification_type"] = "idle_prompt".into();
        n["message"] = "Claude is waiting for your input".into();
        assert_eq!(claude(n).notification, Some(NotificationKind::IdlePrompt));

        let mut f = base("StopFailure");
        f["error"] = "rate_limit".into();
        f["error_details"] = "429 secret details".into();
        let c = claude(f);
        assert_eq!(c.error, Some(StopError::RateLimit));
        assert_eq!(c.project.as_deref(), Some("App"));

        // StopFailure without an error still says "unknown", not success.
        assert_eq!(claude(base("StopFailure")).error, Some(StopError::Unknown));
        // Tool names are categories; MCP tools don't leak server names.
        let mut m = base("PostToolUse");
        m["tool_name"] = "mcp__private-server__query".into();
        assert_eq!(claude(m).tool, Some(ToolCategory::Mcp));
        // `error` on a tool failure is not a StopFailure reason.
        let mut t = base("PostToolUseFailure");
        t["error"] = "exit 1".into();
        assert_eq!(claude(t).error, None);
        // Subagent context is a flag only.
        let mut s = base("PreToolUse");
        s["agent_id"] = "agent-7".into();
        assert!(claude(s).subagent);
    }

    #[test]
    fn ignores_what_islandy_doesnt_handle() {
        assert!(reduce(json!({ "hook_event_name": "PreCompact", "session_id": "s" })).is_none());
        assert!(reduce(json!({ "hook_event_name": "Stop" })).is_none());
        assert!(reduce(json!({ "hook_event_name": "Stop", "session_id": 5 })).is_none());
        assert!(reduce(
            json!({ "hook_event_name": "Stop", "session_id": "s", "stop_hook_active": true })
        )
        .is_none());
        assert!(reduce_hook(b"not json").is_none());
        assert!(reduce_hook(b"").is_none());
        // Unexpected types in optional fields don't break the bridge.
        let odd =
            json!({ "hook_event_name": "Stop", "session_id": "s", "cwd": 42, "error": {"x": 1} });
        assert_eq!(claude(odd).project, None);
    }

    #[test]
    fn bounded_reads_drain_oversized_input() {
        let small = [b'a'; 10];
        assert_eq!(read_bounded(&small[..], 10).unwrap().len(), 10);
        let big = vec![b'a'; 200_000];
        let mut cursor = std::io::Cursor::new(big);
        assert!(read_bounded(&mut cursor, 100_000).is_none());
        assert_eq!(cursor.position(), 200_000, "drained to the end");
    }

    #[test]
    fn categorizes_tools() {
        assert_eq!(tool_category("Edit"), ToolCategory::Editing);
        assert_eq!(tool_category("Grep"), ToolCategory::Searching);
        assert_eq!(tool_category("WebFetch"), ToolCategory::Web);
        assert_eq!(tool_category("Agent"), ToolCategory::Delegating);
        assert_eq!(tool_category("SomethingNew"), ToolCategory::Other);
    }

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn parses_notify_commands() {
        let cwd = Path::new("/home/u/code/Islandy");
        let e = parse_notify(&args("build failed --label Release"), Some(cwd)).unwrap();
        assert_eq!(e.event, DeveloperEvent::BuildFailed);
        assert_eq!(e.project.as_deref(), Some("Islandy"));
        assert_eq!(e.label.as_deref(), Some("Release"));
        assert!(e.at.is_some());

        let e = parse_notify(&args("tests passed --project Web"), Some(cwd)).unwrap();
        assert_eq!(
            (e.event, e.project.as_deref()),
            (DeveloperEvent::TestsPassed, Some("Web"))
        );

        // The spec's shorthand.
        let e = parse_notify(&args("--title Deployed --type success"), None).unwrap();
        assert_eq!(e.event, DeveloperEvent::Message);
        assert_eq!(e.title.as_deref(), Some("Deployed"));
        assert_eq!(e.severity, Some(Severity::Success));
        assert_eq!(e.project, None);

        for bad in [
            "build exploded",
            "server passed",
            "message",
            "build failed --title x",
            "--title",
            "--title a --title b",
            "--title x --severity attention",
            "build failed --rm -rf",
        ] {
            assert!(parse_notify(&args(bad), Some(cwd)).is_err(), "{bad}");
        }
        assert!(protocol::validate(&WireEvent::Developer(
            parse_notify(&args("server started"), Some(cwd)).unwrap()
        ))
        .is_ok());
    }

    #[test]
    fn non_cli_arguments_start_the_app() {
        let os = |v: &[&str]| v.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(main(&os(&["islandy"])), None);
        assert_eq!(main(&os(&["islandy", "--some-flag"])), None);
        assert_eq!(main(&os(&["islandy", "notify", "bogus"])), Some(2));
    }
}
