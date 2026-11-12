//! Activity types shared by the engine, the store and the UI.
//!
//! Everything here is mirrored in `src/services/types.ts`. Records hold only
//! display metadata produced by Islandy itself: a fixed event kind, a short
//! title, an optional project name and a severity. There is no free-form
//! metadata map, and no prompt, response, tool argument or tool output is
//! ever stored.

use serde::{Deserialize, Serialize};

/// Where an activity came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivitySource {
    ClaudeCode,
    Developer,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
    Attention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Lifecycle {
    /// Ongoing or unacknowledged (a running build, an open attention request).
    Active,
    /// Superseded by a later event (the build finished, the agent continued).
    Resolved,
    /// The user dismissed it.
    Dismissed,
    /// Its time ran out, or Islandy restarted while it was active.
    Expired,
}

/// Presentation priority (not execution priority). Lower is more important.
///
/// - `0` needs attention now (permission request, waiting for input)
/// - `1` important outcome (turn finished, build failed, critical warning)
/// - `2` informational (build succeeded, network restored)
/// - `3` ambient (working, started); never peeks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
}

impl From<Priority> for u8 {
    fn from(p: Priority) -> u8 {
        p as u8
    }
}

impl TryFrom<u8> for Priority {
    type Error = &'static str;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => Self::P0,
            1 => Self::P1,
            2 => Self::P2,
            3 => Self::P3,
            _ => return Err("priority must be 0–3"),
        })
    }
}

/// What happened, as a fixed set. The UI picks icons and wording from this;
/// the stored `title`/`description` are Islandy's own short strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    // Claude Code
    ClaudeSessionStarted,
    ClaudeAttention,
    ClaudeWaitingForInput,
    ClaudeTurnFinished,
    ClaudeError,
    ClaudeSessionEnded,
    // Developer workflow
    BuildStarted,
    BuildSucceeded,
    BuildFailed,
    TestsStarted,
    TestsPassed,
    TestsFailed,
    ServerStarted,
    ServerStopped,
    Message,
    // System (Phase 3 alerts)
    BatteryLow,
    BatteryCritical,
    PowerConnected,
    PowerDisconnected,
    NetworkLost,
    NetworkRestored,
    AudioOutputChanged,
}

/// One activity record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityEvent {
    /// Stable, unique and increasing (`a<hex>`), also across restarts.
    pub id: String,
    pub source: ActivitySource,
    pub kind: EventKind,
    /// Islandy's opaque Claude session key (`c<16 hex>`), for Claude events.
    pub session: Option<String>,
    /// A short project name (a folder name), never a full path.
    pub project: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub severity: Severity,
    pub priority: Priority,
    pub lifecycle: Lifecycle,
    /// Epoch ms, assigned by Islandy when the event was received.
    pub timestamp: u64,
    /// When an `active` event expires on its own (epoch ms).
    pub expires_at: Option<u64>,
    /// Source-specific de-duplication key; never derived from the title.
    pub dedupe_key: Option<String>,
}

/* -------------------------------------------------------------------------- */
/* Claude Code sessions                                                       */
/* -------------------------------------------------------------------------- */

/// What the hooks last told us. Deliberately coarse: there is no progress,
/// no estimate, and no claim about what Claude is "thinking".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    /// Session started; no turn observed yet.
    Ready,
    /// A turn is running (prompt submitted or tool activity observed).
    Working,
    /// Claude Code reported a permission prompt or another request.
    Attention,
    /// Claude Code reported it is waiting for input (idle prompt).
    Waiting,
    /// The last turn stopped (`Stop`). Not "task complete".
    Idle,
    /// The last turn ended with an API error (`StopFailure`).
    Error,
    /// `SessionEnd` was received.
    Ended,
}

/// Coarse tool categories. Tool names are mapped by the hook bridge; tool
/// arguments never leave Claude Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolCategory {
    Reading,
    Editing,
    Searching,
    Running,
    Web,
    Delegating,
    Planning,
    Mcp,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    /// Opaque key derived from Claude Code's `session_id` by the bridge.
    pub id: String,
    pub project: Option<String>,
    pub status: SessionStatus,
    /// When Islandy first saw the session start (`None` if it was already
    /// running when Islandy started).
    pub started_at: Option<u64>,
    /// Start of the current turn, while working.
    pub turn_started_at: Option<u64>,
    pub last_seen_at: u64,
    pub last_tool: Option<ToolCategory>,
    /// Subagents currently running (from SubagentStart/Stop).
    pub subagents: u8,
    /// The open attention request's event id, if any.
    pub attention_event: Option<String>,
    /// The user dismissed the attention indicator for this session.
    pub attention_dismissed: bool,
    pub ended_at: Option<u64>,
    /// No event for a long time while working/waiting: status unknown.
    pub stale: bool,
}

/* -------------------------------------------------------------------------- */
/* Settings, snapshot, updates                                                */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SourceToggles {
    pub claude_code: bool,
    pub developer: bool,
    pub system: bool,
}

impl Default for SourceToggles {
    fn default() -> Self {
        Self {
            claude_code: true,
            developer: true,
            system: true,
        }
    }
}

impl SourceToggles {
    pub fn get(&self, source: ActivitySource) -> bool {
        match source {
            ActivitySource::ClaudeCode => self.claude_code,
            ActivitySource::Developer => self.developer,
            ActivitySource::System => self.system,
        }
    }
    pub fn set(&mut self, source: ActivitySource, on: bool) {
        match source {
            ActivitySource::ClaudeCode => self.claude_code = on,
            ActivitySource::Developer => self.developer = on,
            ActivitySource::System => self.system = on,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ActivitySettings {
    /// Islandy's own quiet mode (not Windows Do Not Disturb).
    pub quiet: bool,
    pub sources: SourceToggles,
}

/// The local receiver's state, for troubleshooting in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReceiverStatus {
    Starting,
    Listening,
    /// Couldn't bind or publish the connection file; events can't arrive.
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySnapshot {
    pub revision: u64,
    /// Visible sessions, most recently active first.
    pub sessions: Vec<ClaudeSession>,
    /// Recent events, newest first (bounded).
    pub events: Vec<ActivityEvent>,
    pub settings: ActivitySettings,
    pub receiver: ReceiverStatus,
    /// History couldn't be read or written; it's kept in memory only.
    pub history_error: bool,
}

/// A request to show an event as a peek. Never replayed on reconnect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeekRequest {
    pub event: ActivityEvent,
    /// How long the peek stays (the pointer resting on it pauses this).
    pub dwell_ms: u32,
}

/// Rust → UI push messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ActivityUpdate {
    Snapshot { snapshot: ActivitySnapshot },
    Peek { peek: PeekRequest },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_serializes_as_a_number() {
        assert_eq!(serde_json::to_value(Priority::P1).unwrap(), 1);
        assert_eq!(
            serde_json::from_value::<Priority>(3.into()).unwrap(),
            Priority::P3
        );
        assert!(serde_json::from_value::<Priority>(4.into()).is_err());
    }

    #[test]
    fn events_round_trip_and_reject_extra_fields() {
        let event = ActivityEvent {
            id: "a1".into(),
            source: ActivitySource::ClaudeCode,
            kind: EventKind::ClaudeTurnFinished,
            session: Some("c0123456789abcdef".into()),
            project: Some("Islandy".into()),
            title: "Claude finished responding".into(),
            description: None,
            severity: Severity::Success,
            priority: Priority::P1,
            lifecycle: Lifecycle::Resolved,
            timestamp: 5,
            expires_at: None,
            dedupe_key: None,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["source"], "claudeCode");
        assert_eq!(json["kind"], "claudeTurnFinished");
        assert_eq!(
            serde_json::from_value::<ActivityEvent>(json.clone()).unwrap(),
            event
        );
        let mut extra = json;
        extra["prompt"] = "secret".into();
        assert!(serde_json::from_value::<ActivityEvent>(extra).is_err());
    }

    #[test]
    fn updates_are_tagged() {
        let json = serde_json::to_value(ActivityUpdate::Snapshot {
            snapshot: ActivitySnapshot {
                revision: 1,
                sessions: vec![],
                events: vec![],
                settings: ActivitySettings::default(),
                receiver: ReceiverStatus::Listening,
                history_error: false,
            },
        })
        .unwrap();
        assert_eq!(json["type"], "snapshot");
        assert_eq!(json["snapshot"]["settings"]["sources"]["claudeCode"], true);
        assert_eq!(json["snapshot"]["receiver"], "listening");
    }
}
