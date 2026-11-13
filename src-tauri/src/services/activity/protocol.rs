//! The local event protocol between `islandy hook` / `islandy notify` and the
//! running app's receiver.
//!
//! One connection carries one message:
//!
//! ```text
//! ISLANDY1 <64 hex token>\n<one line of JSON>\n      (client → Islandy, ≤ 4 KiB)
//! ok\n  |  error <code>\n                             (Islandy → client)
//! ```
//!
//! The JSON is a *reduced* event: the bridge has already dropped everything
//! except a fixed set of enum values and short names. The receiver accepts
//! only this shape (`deny_unknown_fields`, closed enums, length limits) and
//! assigns its own timestamps.
//!
//! The listener binds `127.0.0.1` on an OS-assigned port. The port and a
//! random 256-bit per-installation token are published in `receiver.json`
//! in Islandy's per-user data directory. A client must read that file to
//! connect, so other local users (and web pages, which can't read files or
//! speak this protocol) are rejected.

use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use super::model::{Severity, ToolCategory};

pub const MAGIC: &str = "ISLANDY1";
/// Upper bound for a whole message (header + JSON).
pub const MAX_MESSAGE: usize = 4096;
pub const TOKEN_LEN: usize = 64;
pub const INFO_FILE: &str = "receiver.json";

pub const MAX_PROJECT: usize = 64;
pub const MAX_LABEL: usize = 80;
pub const MAX_TITLE: usize = 100;
pub const MAX_SUMMARY: usize = 160;
/// How long `islandy hook` waits for an answer to a permission request
/// before handing it back to Claude Code's own prompt.
pub const ANSWER_WAIT: Duration = Duration::from_secs(30);

/* -------------------------------------------------------------------------- */
/* Events                                                                     */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WireEvent {
    Claude(ClaudeWire),
    Developer(DeveloperWire),
    /// Reachability check (`islandy status`); never reaches the engine.
    Ping,
}

/// Claude Code hook events Islandy listens to (and installs hooks for).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClaudeHook {
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    PermissionRequest,
    Notification,
    Stop,
    StopFailure,
    SubagentStart,
    SubagentStop,
}

impl ClaudeHook {
    pub const ALL: [ClaudeHook; 12] = [
        Self::SessionStart,
        Self::SessionEnd,
        Self::UserPromptSubmit,
        Self::PreToolUse,
        Self::PostToolUse,
        Self::PostToolUseFailure,
        Self::PermissionRequest,
        Self::Notification,
        Self::Stop,
        Self::StopFailure,
        Self::SubagentStart,
        Self::SubagentStop,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::PermissionRequest => "PermissionRequest",
            Self::Notification => "Notification",
            Self::Stop => "Stop",
            Self::StopFailure => "StopFailure",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|h| h.name() == name)
    }
}

/// `Notification.notification_type`, reduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationKind {
    /// `permission_prompt`
    PermissionPrompt,
    /// `idle_prompt`: Claude Code is waiting for the user's input.
    IdlePrompt,
    /// `elicitation_dialog`, `elicitation_url_dialog`, `agent_needs_input`
    InputRequest,
    /// Anything else (auth messages, plugin notifications, new types).
    Other,
}

impl NotificationKind {
    pub fn from_hook(value: &str) -> Self {
        match value {
            "permission_prompt" => Self::PermissionPrompt,
            "idle_prompt" => Self::IdlePrompt,
            "elicitation_dialog" | "elicitation_url_dialog" | "agent_needs_input" => {
                Self::InputRequest
            }
            _ => Self::Other,
        }
    }
}

/// `StopFailure.error`, reduced to what the UI can explain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopError {
    RateLimit,
    Overloaded,
    Authentication,
    Billing,
    InvalidRequest,
    Server,
    MaxOutputTokens,
    Unknown,
}

impl StopError {
    pub fn from_hook(value: &str) -> Self {
        match value {
            "rate_limit" => Self::RateLimit,
            "overloaded" => Self::Overloaded,
            "authentication_failed"
            | "oauth_org_not_allowed"
            | "account_on_hold"
            | "cloud_credential_error" => Self::Authentication,
            "billing_error" => Self::Billing,
            "invalid_request" | "model_not_found" => Self::InvalidRequest,
            "server_error" => Self::Server,
            "max_output_tokens" => Self::MaxOutputTokens,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeWire {
    pub event: ClaudeHook,
    /// `c` + 16 hex: a hash of Claude Code's `session_id`.
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<ToolCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification: Option<NotificationKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<StopError>,
    /// The hook fired inside a subagent (`agent_id` was present).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub subagent: bool,
    /// `PermissionRequest` only: the bridge waits for an answer from the
    /// island (see [`ask`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask: Option<PermissionAsk>,
}

/// What a permission request asks for, shown on the island so it can be
/// answered there. Held in memory only while the question is open; never
/// written to history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionAsk {
    /// One line: the command, file path, URL or tool being requested.
    pub summary: String,
    /// `summary` shows the whole request. Otherwise Islandy offers no
    /// Allow (only Deny, or answering in Claude Code).
    pub complete: bool,
}

/// The island's answer to a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    Allow,
    Deny,
    /// No answer: Claude Code shows its own prompt as usual.
    Pass,
}

impl Decision {
    pub fn word(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Pass => "pass",
        }
    }

    pub fn from_word(word: &str) -> Self {
        match word {
            "allow" => Self::Allow,
            "deny" => Self::Deny,
            _ => Self::Pass,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeveloperEvent {
    BuildStarted,
    BuildSucceeded,
    BuildFailed,
    TestsStarted,
    TestsPassed,
    TestsFailed,
    ServerStarted,
    ServerStopped,
    Message,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeveloperWire {
    pub event: DeveloperEvent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A short label, e.g. "Release build" (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Required for `message`: its one-line title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// For `message` only; defaults to info.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    /// When the CLI ran (epoch ms). Used only to order events of one task
    /// that arrive out of order; never displayed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
}

/* -------------------------------------------------------------------------- */
/* Text and identifiers                                                       */
/* -------------------------------------------------------------------------- */

/// One line of display text: control characters dropped, whitespace runs
/// collapsed, trimmed and cut to `max` characters. `None` if nothing is left.
pub fn clean_text(value: &str, max: usize) -> Option<String> {
    let mut out = String::new();
    let mut space = false;
    for c in value.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if c.is_control() || is_bidi_control(c) {
            continue;
        }
        if out.chars().count() >= max {
            break;
        }
        if space {
            out.push(' ');
            space = false;
            if out.chars().count() >= max {
                break;
            }
        }
        out.push(c);
    }
    (!out.is_empty()).then_some(out)
}

/// Bidirectional overrides could make a name render misleadingly.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}')
}

fn is_clean(value: &str, max: usize) -> bool {
    clean_text(value, max).as_deref() == Some(value)
}

/// The last component of a working directory, as a project name.
pub fn project_from_cwd(cwd: &str) -> Option<String> {
    let trimmed = cwd.trim_end_matches(['/', '\\']);
    let name = trimmed.rsplit(['/', '\\']).next().unwrap_or("");
    // A drive root ("C:") is not a project name.
    if name.is_empty() || (name.len() == 2 && name.ends_with(':')) {
        return None;
    }
    clean_text(name, MAX_PROJECT)
}

/// Islandy's opaque session key: FNV-1a 64 of Claude Code's session id.
pub fn session_key(session_id: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in session_id.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("c{hash:016x}")
}

pub fn is_session_key(value: &str) -> bool {
    value.len() == 17
        && value.starts_with('c')
        && value[1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Strict validation of a decoded event (the receiver never trusts a client).
pub fn validate(event: &WireEvent) -> Result<(), &'static str> {
    let optional = |v: &Option<String>, max: usize| v.as_deref().map_or(true, |s| is_clean(s, max));
    match event {
        WireEvent::Ping => {}
        WireEvent::Claude(e) => {
            if !is_session_key(&e.session) {
                return Err("session");
            }
            if !optional(&e.project, MAX_PROJECT) {
                return Err("project");
            }
            if e.notification.is_some() && e.event != ClaudeHook::Notification {
                return Err("notification");
            }
            if e.error.is_some() && e.event != ClaudeHook::StopFailure {
                return Err("error");
            }
            if let Some(ask) = &e.ask {
                if e.event != ClaudeHook::PermissionRequest || !is_clean(&ask.summary, MAX_SUMMARY)
                {
                    return Err("ask");
                }
            }
        }
        WireEvent::Developer(e) => {
            if !optional(&e.project, MAX_PROJECT) || !optional(&e.label, MAX_LABEL) {
                return Err("text");
            }
            let message = e.event == DeveloperEvent::Message;
            match (&e.title, message) {
                (Some(t), true) if is_clean(t, MAX_TITLE) => {}
                (None, false) => {}
                _ => return Err("title"),
            }
            if e.severity.is_some() && !message {
                return Err("severity");
            }
            if matches!(e.severity, Some(Severity::Attention)) {
                // Attention is reserved for Claude Code requests.
                return Err("severity");
            }
        }
    }
    Ok(())
}

/* -------------------------------------------------------------------------- */
/* Connection file and token                                                  */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverInfo {
    pub version: u32,
    pub port: u16,
    pub token: String,
}

pub fn is_token(value: &str) -> bool {
    value.len() == TOKEN_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// 32 random bytes from the OS CSPRNG, hex encoded.
pub fn new_token() -> std::io::Result<String> {
    let mut bytes = [0u8; TOKEN_LEN / 2];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Constant-time comparison (no early exit on the first differing byte).
pub fn token_matches(expected: &str, given: &[u8]) -> bool {
    let expected = expected.as_bytes();
    if expected.len() != given.len() {
        return false;
    }
    expected
        .iter()
        .zip(given)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

pub fn info_path(dir: &Path) -> PathBuf {
    dir.join(INFO_FILE)
}

pub fn read_info(dir: &Path) -> Option<ReceiverInfo> {
    let bytes = std::fs::read(info_path(dir)).ok()?;
    if bytes.len() > 1024 {
        return None;
    }
    let info: ReceiverInfo = serde_json::from_slice(&bytes).ok()?;
    (info.version == 1 && info.port != 0 && is_token(&info.token)).then_some(info)
}

/* -------------------------------------------------------------------------- */
/* Framing                                                                    */
/* -------------------------------------------------------------------------- */

pub fn encode(token: &str, event: &WireEvent) -> Result<Vec<u8>, &'static str> {
    let json = serde_json::to_string(event).map_err(|_| "encode")?;
    let message = format!("{MAGIC} {token}\n{json}\n");
    if message.len() > MAX_MESSAGE {
        return Err("too large");
    }
    Ok(message.into_bytes())
}

/// Why a message was rejected; sent back as `error <code>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    TooLarge,
    Malformed,
    Unauthorized,
    Invalid,
    RateLimited,
    Unavailable,
}

impl Rejection {
    pub fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "size",
            Self::Malformed => "malformed",
            Self::Unauthorized => "auth",
            Self::Invalid => "invalid",
            Self::RateLimited => "rate",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Splits a complete message, checks the token *before* looking at the
/// JSON, then decodes and validates the event.
pub fn decode(message: &[u8], token: &str) -> Result<WireEvent, Rejection> {
    if message.len() > MAX_MESSAGE {
        return Err(Rejection::TooLarge);
    }
    let newline = message
        .iter()
        .position(|b| *b == b'\n')
        .ok_or(Rejection::Malformed)?;
    let header = &message[..newline];
    let prefix = format!("{MAGIC} ");
    let given = header
        .strip_prefix(prefix.as_bytes())
        .ok_or(Rejection::Malformed)?;
    if !token_matches(token, given) {
        return Err(Rejection::Unauthorized);
    }
    let body = &message[newline + 1..];
    let body = body.strip_suffix(b"\n").unwrap_or(body);
    if body.contains(&b'\n') {
        return Err(Rejection::Malformed);
    }
    let event: WireEvent = serde_json::from_slice(body).map_err(|_| Rejection::Invalid)?;
    validate(&event).map_err(|_| Rejection::Invalid)?;
    Ok(event)
}

/* -------------------------------------------------------------------------- */
/* Client                                                                     */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// No data directory, or no connection file: Islandy isn't running (or
    /// never ran).
    NotRunning,
    /// Connecting or writing failed / timed out.
    Unreachable,
    /// Islandy answered with an error code.
    Rejected(String),
    Encode,
}

pub const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
pub const IO_TIMEOUT: Duration = Duration::from_millis(700);

/// Sends one event to the running app. Bounded by the timeouts above.
pub fn send(dir: &Path, event: &WireEvent) -> Result<(), SendError> {
    deliver(dir, event).map(drop)
}

/// Sends a permission request (an event with `ask`) and waits up to
/// `wait` for the island's answer. Anything unexpected — Islandy not
/// running, an older Islandy that doesn't answer, a timeout — is `Pass`.
pub fn ask(dir: &Path, event: &WireEvent, wait: Duration) -> Decision {
    let Ok(mut stream) = deliver(dir, event) else {
        return Decision::Pass;
    };
    if stream.set_read_timeout(Some(wait)).is_err() {
        return Decision::Pass;
    }
    match read_line(&mut stream) {
        Ok(line) => Decision::from_word(&line),
        Err(()) => Decision::Pass,
    }
}

/// Sends one message and checks the `ok`; returns the open connection.
fn deliver(dir: &Path, event: &WireEvent) -> Result<TcpStream, SendError> {
    let info = read_info(dir).ok_or(SendError::NotRunning)?;
    let message = encode(&info.token, event).map_err(|_| SendError::Encode)?;
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, info.port));
    let mut stream =
        TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|_| SendError::Unreachable)?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .and_then(|_| stream.set_read_timeout(Some(IO_TIMEOUT)))
        .map_err(|_| SendError::Unreachable)?;
    stream
        .write_all(&message)
        .map_err(|_| SendError::Unreachable)?;
    let reply = read_line(&mut stream).map_err(|()| SendError::Unreachable)?;
    if reply == "ok" {
        Ok(stream)
    } else {
        Err(SendError::Rejected(
            reply.strip_prefix("error ").unwrap_or("unknown").to_owned(),
        ))
    }
}

/// Reads one short reply line byte by byte (so nothing after the newline is
/// consumed), trimmed. EOF before a newline returns what was read.
fn read_line(stream: &mut TcpStream) -> Result<String, ()> {
    let mut line = Vec::with_capacity(16);
    let mut byte = [0u8; 1];
    while line.len() < 64 {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => line.push(byte[0]),
            Err(_) => return Err(()),
        }
    }
    Ok(String::from_utf8_lossy(&line).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    fn stop() -> WireEvent {
        WireEvent::Claude(ClaudeWire {
            event: ClaudeHook::Stop,
            session: session_key("abc"),
            project: Some("Islandy".into()),
            tool: None,
            notification: None,
            error: None,
            subagent: false,
            ask: None,
        })
    }

    #[test]
    fn round_trips_an_event() {
        let bytes = encode(TOKEN, &stop()).unwrap();
        assert!(bytes.starts_with(b"ISLANDY1 0011"));
        assert_eq!(decode(&bytes, TOKEN).unwrap(), stop());
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains(r#"{"kind":"claude","event":"Stop","session":"c"#));
    }

    #[test]
    fn rejects_missing_or_wrong_tokens_before_parsing() {
        let bytes = encode(TOKEN, &stop()).unwrap();
        let other = TOKEN.replace("00", "11");
        assert_eq!(decode(&bytes, &other), Err(Rejection::Unauthorized));
        assert_eq!(
            decode(b"ISLANDY1 \n{}\n", TOKEN),
            Err(Rejection::Unauthorized)
        );
        // An HTTP request (e.g. from a browser) is not this protocol.
        let http = b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n{}";
        assert_eq!(decode(http, TOKEN), Err(Rejection::Malformed));
        // Garbage after a valid token is invalid, not accepted.
        let bad = format!("{MAGIC} {TOKEN}\nnot json\n");
        assert_eq!(decode(bad.as_bytes(), TOKEN), Err(Rejection::Invalid));
    }

    #[test]
    fn rejects_oversized_and_multi_line_messages() {
        let big = format!("{MAGIC} {TOKEN}\n{}\n", "x".repeat(MAX_MESSAGE));
        assert_eq!(decode(big.as_bytes(), TOKEN), Err(Rejection::TooLarge));
        let two = format!("{MAGIC} {TOKEN}\n{{}}\n{{}}\n");
        assert_eq!(decode(two.as_bytes(), TOKEN), Err(Rejection::Malformed));
        let mut long = stop();
        if let WireEvent::Claude(e) = &mut long {
            e.project = Some("p".repeat(5000));
        }
        assert!(encode(TOKEN, &long).is_err());
    }

    #[test]
    fn rejects_unknown_fields_and_values() {
        let decode_json =
            |json: &str| decode(format!("{MAGIC} {TOKEN}\n{json}\n").as_bytes(), TOKEN);
        let session = session_key("x");
        let ok = format!(r#"{{"kind":"claude","event":"Stop","session":"{session}"}}"#);
        assert!(decode_json(&ok).is_ok());
        for bad in [
            // Extra fields (e.g. a prompt) are refused, not ignored.
            format!(r#"{{"kind":"claude","event":"Stop","session":"{session}","prompt":"x"}}"#),
            format!(r#"{{"kind":"claude","event":"PreCompact","session":"{session}"}}"#),
            r#"{"kind":"claude","event":"Stop","session":"not-a-key"}"#.to_owned(),
            format!(
                r#"{{"kind":"claude","event":"Stop","session":"{session}","project":"a\u0007b"}}"#
            ),
            format!(
                r#"{{"kind":"claude","event":"Stop","session":"{session}","error":"rateLimit"}}"#
            ),
            r#"{"kind":"shell","command":"calc.exe"}"#.to_owned(),
            r#"{"kind":"developer","event":"message"}"#.to_owned(),
            r#"{"kind":"developer","event":"buildFailed","title":"x"}"#.to_owned(),
            r#"{"kind":"developer","event":"message","title":"x","severity":"attention"}"#
                .to_owned(),
        ] {
            assert_eq!(decode_json(&bad), Err(Rejection::Invalid), "{bad}");
        }
    }

    #[test]
    fn cleans_text_and_derives_projects() {
        assert_eq!(
            clean_text("  a\t\nb \u{7}c  ", 10).as_deref(),
            Some("a b c")
        );
        assert_eq!(clean_text("a\u{7}b", 10).as_deref(), Some("ab"));
        assert_eq!(clean_text("\u{202E}evil", 10).as_deref(), Some("evil"));
        assert_eq!(clean_text("   ", 10), None);
        assert_eq!(clean_text("abcdef", 3).as_deref(), Some("abc"));
        assert_eq!(clean_text("ab cd", 3).as_deref(), Some("ab "));
        assert_eq!(
            project_from_cwd("/home/u/Islandy").as_deref(),
            Some("Islandy")
        );
        assert_eq!(
            project_from_cwd(r"C:\Users\u\code\My App\").as_deref(),
            Some("My App")
        );
        assert_eq!(project_from_cwd(r"C:\"), None);
        assert_eq!(project_from_cwd("/"), None);
    }

    #[test]
    fn session_keys_are_stable_and_opaque() {
        let a = session_key("6f1c0b5e-2d7a-4a5e-9b1d-0c2f3e4a5b6c");
        assert_eq!(a, session_key("6f1c0b5e-2d7a-4a5e-9b1d-0c2f3e4a5b6c"));
        assert_ne!(a, session_key("6f1c0b5e-2d7a-4a5e-9b1d-0c2f3e4a5b6d"));
        assert!(is_session_key(&a));
        assert!(!a.contains("6f1c0b5e"));
    }

    #[test]
    fn maps_hook_values_conservatively() {
        assert_eq!(
            NotificationKind::from_hook("idle_prompt"),
            NotificationKind::IdlePrompt
        );
        assert_eq!(
            NotificationKind::from_hook("agent_needs_input"),
            NotificationKind::InputRequest
        );
        assert_eq!(
            NotificationKind::from_hook("auth_success"),
            NotificationKind::Other
        );
        assert_eq!(
            NotificationKind::from_hook("brand_new_type"),
            NotificationKind::Other
        );
        assert_eq!(
            StopError::from_hook("account_on_hold"),
            StopError::Authentication
        );
        assert_eq!(StopError::from_hook("???"), StopError::Unknown);
        assert_eq!(ClaudeHook::from_name("Stop"), Some(ClaudeHook::Stop));
        assert_eq!(ClaudeHook::from_name("PreCompact"), None);
    }

    #[test]
    fn tokens_are_random_hex_and_compared_exactly() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert!(is_token(&a) && is_token(&b));
        assert_ne!(a, b);
        assert!(token_matches(&a, a.as_bytes()));
        assert!(!token_matches(&a, b.as_bytes()));
        assert!(!token_matches(&a, &a.as_bytes()[1..]));
    }
}
