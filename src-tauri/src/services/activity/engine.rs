//! The activity engine: a deterministic state machine with no I/O and no
//! clock of its own (every call takes `now`, in epoch ms).
//!
//! It turns validated inputs (reduced Claude Code hook events, developer
//! events, Phase 3 system alerts) into:
//!
//! - **sessions**: one per Claude Code session id, with a coarse status;
//! - **events**: a bounded, de-duplicated history with lifecycles;
//! - **peek requests**: which events deserve a temporary peek, and for how
//!   long. Whether a peek is actually shown is the UI's decision (it knows
//!   whether the user is interacting with the island).
//!
//! Priority rules (presentation only):
//!
//! | Priority | Examples                                        | Peek      |
//! |----------|-------------------------------------------------|-----------|
//! | P0       | permission prompt, waiting for input            | 8 s, then the pill keeps an amber indicator |
//! | P1       | turn finished, API error, build/tests failed, battery/network warnings | 4 s success, 6 s error/warning |
//! | P2       | build succeeded, tests passed, network restored | 3 s info, 4 s success |
//! | P3       | session started/ended, build started, working   | never     |
//!
//! Quiet mode suppresses every peek; indicators and history continue.

use std::collections::{HashMap, VecDeque};

use super::{
    model::{
        ActivityEvent, ActivitySettings, ActivitySnapshot, ActivitySource, ClaudeSession,
        EventKind, LastReceived, Lifecycle, PeekRequest, Priority, ReceiverStatus, SessionStatus,
        Severity,
    },
    protocol::{
        ClaudeHook, ClaudeWire, DeveloperEvent, DeveloperWire, NotificationKind, StopError,
    },
};
use crate::services::system::models::{ConnectionKind, SystemAlert};

const SECOND: u64 = 1000;
const MINUTE: u64 = 60 * SECOND;
const HOUR: u64 = 60 * MINUTE;

/// History bounds.
pub const MAX_EVENTS: usize = 300;
/// Default retention (the user can choose 1–30 days).
#[cfg(test)]
pub const RETENTION_MS: u64 = 7 * DAY;
pub const DAY: u64 = 24 * HOUR;
/// At most this many peeks per source per [`PEEK_WINDOW_MS`] (P0 requests
/// for the user's attention are exempt), so one noisy integration can't
/// dominate the island. Events are still recorded.
pub const PEEK_BUDGET: usize = 4;
pub const PEEK_WINDOW_MS: u64 = 60_000;
/// Events included in a UI snapshot (newest first).
pub const SNAPSHOT_EVENTS: usize = 60;
pub const MAX_SESSIONS: usize = 12;

/// A working session with no event for this long shows "status unknown".
pub const STALE_AFTER_MS: u64 = 15 * MINUTE;
/// How long sessions stay listed without events, by state. Removal never
/// implies success: it only stops showing something we know nothing about.
pub const ENDED_LINGER_MS: u64 = 2 * MINUTE;
pub const IDLE_LINGER_MS: u64 = HOUR;
pub const ACTIVE_LINGER_MS: u64 = 3 * HOUR;

/// A started build/test run that never reports back expires (not "failed").
pub const TASK_TIMEOUT_MS: u64 = 2 * HOUR;
pub const SERVER_TIMEOUT_MS: u64 = 12 * HOUR;
pub const SYSTEM_TIMEOUT_MS: u64 = 6 * HOUR;

const DEDUPE_CAPACITY: usize = 256;
const DEDUPE_MAX_WINDOW: u64 = 5 * MINUTE;
/// Client clocks outside this range of ours are ignored for ordering.
const CLOCK_SKEW_MS: u64 = 10 * MINUTE;

/// The result of feeding an input or a command to the engine.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The snapshot changed and should be republished.
    pub changed: bool,
    /// The history changed and should be persisted.
    pub persist: bool,
    pub peek: Option<PeekRequest>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Claude(ClaudeWire),
    Developer(DeveloperWire),
    System(SystemAlert),
}

/// A not-yet-recorded event.
struct Draft {
    source: ActivitySource,
    kind: EventKind,
    session: Option<String>,
    project: Option<String>,
    title: String,
    description: Option<String>,
    severity: Severity,
    priority: Priority,
    lifecycle: Lifecycle,
    expires_at: Option<u64>,
    dedupe_key: Option<String>,
}

impl Draft {
    fn new(source: ActivitySource, kind: EventKind, title: &str, severity: Severity) -> Self {
        Self {
            source,
            kind,
            session: None,
            project: None,
            title: title.to_owned(),
            description: None,
            severity,
            priority: Priority::P2,
            lifecycle: Lifecycle::Resolved,
            expires_at: None,
            dedupe_key: None,
        }
    }
    fn priority(mut self, p: Priority) -> Self {
        self.priority = p;
        self
    }
    fn active(mut self, expires_at: Option<u64>) -> Self {
        self.lifecycle = Lifecycle::Active;
        self.expires_at = expires_at;
        self
    }
    fn describe(mut self, d: Option<String>) -> Self {
        self.description = d;
        self
    }
    fn project(mut self, p: Option<String>) -> Self {
        self.project = p;
        self
    }
    fn session(mut self, s: &str) -> Self {
        self.session = Some(s.to_owned());
        self
    }
    fn key(mut self, k: String) -> Self {
        self.dedupe_key = Some(k);
        self
    }
}

/// How long a peek for this event stays, or `None` for no peek.
pub fn dwell_ms(priority: Priority, severity: Severity) -> Option<u32> {
    match (priority, severity) {
        (Priority::P3, _) => None,
        (Priority::P0, _) => Some(8_000),
        (_, Severity::Error | Severity::Warning) => Some(6_000),
        (_, Severity::Success) => Some(4_000),
        (Priority::P1, _) => Some(5_000),
        _ => Some(3_000),
    }
}

pub struct ActivityEngine {
    settings: ActivitySettings,
    sessions: Vec<ClaudeSession>,
    /// Oldest first.
    events: VecDeque<ActivityEvent>,
    /// De-duplication key → last time it was accepted.
    dedupe: HashMap<String, u64>,
    /// Developer task key → newest client timestamp seen (ordering).
    task_clock: HashMap<String, u64>,
    /// Recent peek times per source (rate limiting).
    peek_log: HashMap<ActivitySource, VecDeque<u64>>,
    last_received: LastReceived,
    next_id: u64,
}

impl ActivityEngine {
    pub fn new(settings: ActivitySettings, now: u64) -> Self {
        Self {
            settings: settings.normalized(),
            sessions: Vec::new(),
            events: VecDeque::new(),
            dedupe: HashMap::new(),
            task_clock: HashMap::new(),
            peek_log: HashMap::new(),
            last_received: LastReceived::default(),
            next_id: now << 10,
        }
    }

    /// Restores persisted history. Events that were active when Islandy
    /// stopped are marked expired: nothing stale resurfaces after a restart,
    /// and no peek is ever produced from history.
    pub fn restore(&mut self, mut events: Vec<ActivityEvent>, now: u64) {
        events.retain(|e| e.timestamp <= now.saturating_add(MINUTE));
        events.sort_by_key(|e| e.timestamp);
        for e in &mut events {
            if e.lifecycle == Lifecycle::Active {
                e.lifecycle = Lifecycle::Expired;
            }
            if let Some(n) = parse_id(&e.id) {
                self.next_id = self.next_id.max(n + 1);
            }
        }
        self.events = events.into();
        self.enforce_retention(now);
    }

    pub fn settings(&self) -> ActivitySettings {
        self.settings
    }

    /// All stored events, oldest first (for persistence).
    pub fn history(&self) -> impl Iterator<Item = &ActivityEvent> {
        self.events.iter()
    }

    pub fn snapshot(
        &self,
        revision: u64,
        receiver: ReceiverStatus,
        history_error: bool,
    ) -> ActivitySnapshot {
        let mut sessions = self.sessions.clone();
        sessions.sort_by(|a, b| {
            session_rank(a)
                .cmp(&session_rank(b))
                .then(b.last_seen_at.cmp(&a.last_seen_at))
        });
        ActivitySnapshot {
            revision,
            sessions,
            events: self
                .events
                .iter()
                .rev()
                .take(SNAPSHOT_EVENTS)
                .cloned()
                .collect(),
            settings: self.settings,
            receiver,
            history_error,
            last_received: self.last_received,
        }
    }

    pub fn ingest(&mut self, input: Input, now: u64) -> Outcome {
        let source = match &input {
            Input::Claude(_) => ActivitySource::ClaudeCode,
            Input::Developer(_) => ActivitySource::Developer,
            Input::System(_) => ActivitySource::System,
        };
        if !self.settings.sources.get(source) {
            return Outcome::default();
        }
        // Tool events change nothing visible but are worth showing as
        // "last event received" (no snapshot is published just for this).
        match source {
            ActivitySource::ClaudeCode => self.last_received.claude_code = Some(now),
            ActivitySource::Developer => self.last_received.developer = Some(now),
            ActivitySource::System => {}
        }
        let mut outcome = match input {
            Input::Claude(e) => self.claude(e, now),
            Input::Developer(e) => self.developer(e, now),
            Input::System(a) => self.system(&a, now),
        };
        if outcome.persist {
            self.enforce_retention(now);
        }
        outcome.peek = outcome.peek.and_then(|p| self.allow_peek(source, p, now));
        outcome
    }

    /// Applies the notification preferences and the per-source rate limit
    /// to a peek request. The event itself is recorded either way.
    fn allow_peek(
        &mut self,
        source: ActivitySource,
        mut peek: PeekRequest,
        now: u64,
    ) -> Option<PeekRequest> {
        let s = self.settings;
        if s.quiet || !s.peeks.source(source) {
            return None;
        }
        match peek.event.severity {
            Severity::Success if !s.peeks.success => return None,
            Severity::Error | Severity::Warning if !s.peeks.errors => return None,
            _ => {}
        }
        if peek.event.priority != Priority::P0 {
            let log = self.peek_log.entry(source).or_default();
            while log
                .front()
                .is_some_and(|&t| now.saturating_sub(t) >= PEEK_WINDOW_MS)
            {
                log.pop_front();
            }
            if log.len() >= PEEK_BUDGET {
                return None;
            }
            log.push_back(now);
        }
        peek.dwell_ms = s.peek_duration.scale(peek.dwell_ms);
        Some(peek)
    }

    fn retention_ms(&self) -> u64 {
        u64::from(self.settings.retention_days.max(1)) * DAY
    }

    /* ------------------------------------------------------------------ */
    /* Recording                                                           */
    /* ------------------------------------------------------------------ */

    /// `true` if `key` was accepted within `window` (and is a duplicate now).
    fn duplicate(&mut self, key: &str, window: u64, now: u64) -> bool {
        if let Some(&at) = self.dedupe.get(key) {
            if now.saturating_sub(at) < window {
                return true;
            }
        }
        if self.dedupe.len() >= DEDUPE_CAPACITY {
            self.dedupe
                .retain(|_, at| now.saturating_sub(*at) < DEDUPE_MAX_WINDOW);
            if self.dedupe.len() >= DEDUPE_CAPACITY {
                if let Some(oldest) = self
                    .dedupe
                    .iter()
                    .min_by_key(|(_, at)| **at)
                    .map(|(k, _)| k.clone())
                {
                    self.dedupe.remove(&oldest);
                }
            }
        }
        self.dedupe.insert(key.to_owned(), now);
        false
    }

    fn record(&mut self, draft: Draft, now: u64) -> (ActivityEvent, Option<PeekRequest>) {
        let id = format!("a{:x}", self.next_id);
        self.next_id += 1;
        let event = ActivityEvent {
            id,
            source: draft.source,
            kind: draft.kind,
            session: draft.session,
            project: draft.project,
            title: draft.title,
            description: draft.description,
            severity: draft.severity,
            priority: draft.priority,
            lifecycle: draft.lifecycle,
            timestamp: now,
            expires_at: draft.expires_at,
            dedupe_key: draft.dedupe_key,
        };
        self.events.push_back(event.clone());
        let peek = dwell_ms(event.priority, event.severity).map(|dwell_ms| PeekRequest {
            event: event.clone(),
            dwell_ms,
        });
        (event, peek)
    }

    fn recorded(&mut self, draft: Draft, now: u64) -> Outcome {
        let (_, peek) = self.record(draft, now);
        Outcome {
            changed: true,
            persist: true,
            peek,
        }
    }

    /// Marks an active event resolved. Returns whether anything changed.
    fn resolve(&mut self, id: &str) -> bool {
        match self.events.iter_mut().find(|e| e.id == id) {
            Some(e) if e.lifecycle == Lifecycle::Active => {
                e.lifecycle = Lifecycle::Resolved;
                true
            }
            _ => false,
        }
    }

    /// Resolves every active event with this de-duplication key.
    fn resolve_key(&mut self, key: &str) -> bool {
        let mut changed = false;
        for e in self.events.iter_mut() {
            if e.lifecycle == Lifecycle::Active && e.dedupe_key.as_deref() == Some(key) {
                e.lifecycle = Lifecycle::Resolved;
                changed = true;
            }
        }
        changed
    }

    fn enforce_retention(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.retention_ms());
        while self.events.front().is_some_and(|e| e.timestamp < cutoff) {
            self.events.pop_front();
        }
        while self.events.len() > MAX_EVENTS {
            self.events.pop_front();
        }
    }

    /* ------------------------------------------------------------------ */
    /* Claude Code                                                         */
    /* ------------------------------------------------------------------ */

    fn session_index(&mut self, e: &ClaudeWire, now: u64) -> usize {
        if let Some(i) = self.sessions.iter().position(|s| s.id == e.session) {
            return i;
        }
        if self.sessions.len() >= MAX_SESSIONS {
            // Evict the least relevant: ended/idle before working, then oldest.
            if let Some(victim) = (0..self.sessions.len()).max_by(|&a, &b| {
                let (sa, sb) = (&self.sessions[a], &self.sessions[b]);
                eviction_rank(sa)
                    .cmp(&eviction_rank(sb))
                    .then(sb.last_seen_at.cmp(&sa.last_seen_at))
            }) {
                let removed = self.sessions.remove(victim);
                if let Some(id) = removed.attention_event {
                    self.resolve(&id);
                }
            }
        }
        self.sessions.push(ClaudeSession {
            id: e.session.clone(),
            project: e.project.clone(),
            status: SessionStatus::Ready,
            started_at: None,
            turn_started_at: None,
            last_seen_at: now,
            last_tool: None,
            subagents: 0,
            attention_event: None,
            attention_dismissed: false,
            ended_at: None,
            stale: false,
        });
        self.sessions.len() - 1
    }

    /// Closes the session's open attention request, if any.
    fn clear_attention(&mut self, i: usize) -> bool {
        let s = &mut self.sessions[i];
        s.attention_dismissed = false;
        match s.attention_event.take() {
            Some(id) => {
                self.resolve(&id);
                true
            }
            None => false,
        }
    }

    fn claude(&mut self, e: ClaudeWire, now: u64) -> Outcome {
        let i = self.session_index(&e, now);
        let before = self.sessions[i].clone();
        {
            let s = &mut self.sessions[i];
            s.last_seen_at = now;
            s.stale = false;
            if e.project.is_some() {
                s.project.clone_from(&e.project);
            }
        }
        let key = |suffix: &str| format!("claude:{}:{suffix}", e.session);
        let mut outcome = Outcome::default();
        let mut persist = false;

        match e.event {
            ClaudeHook::SessionStart => {
                persist |= self.clear_attention(i);
                let s = &mut self.sessions[i];
                let fresh = s.started_at.is_none() || s.status == SessionStatus::Ended;
                if fresh {
                    s.started_at = Some(now);
                }
                s.status = SessionStatus::Ready;
                s.turn_started_at = None;
                s.subagents = 0;
                s.ended_at = None;
                s.last_tool = None;
                if fresh && !self.duplicate(&key("start"), 10 * SECOND, now) {
                    let draft = Draft::new(
                        ActivitySource::ClaudeCode,
                        EventKind::ClaudeSessionStarted,
                        "Claude Code session started",
                        Severity::Info,
                    )
                    .priority(Priority::P3)
                    .session(&e.session)
                    .project(e.project.clone())
                    .key(key("start"));
                    outcome = self.recorded(draft, now);
                }
            }
            ClaudeHook::UserPromptSubmit => {
                persist |= self.clear_attention(i);
                let s = &mut self.sessions[i];
                s.status = SessionStatus::Working;
                s.turn_started_at = Some(now);
                s.last_tool = None;
            }
            ClaudeHook::PreToolUse => {
                // A tool about to run doesn't answer a *permission* prompt
                // (parallel calls can precede it), but it does mean an
                // input request was answered.
                if self.sessions[i].status == SessionStatus::Waiting {
                    persist |= self.clear_attention(i);
                }
                let s = &mut self.sessions[i];
                if s.status != SessionStatus::Attention {
                    s.status = SessionStatus::Working;
                }
                s.turn_started_at.get_or_insert(now);
                if e.tool.is_some() {
                    s.last_tool = e.tool;
                }
            }
            ClaudeHook::PostToolUse | ClaudeHook::PostToolUseFailure => {
                persist |= self.clear_attention(i);
                let s = &mut self.sessions[i];
                s.status = SessionStatus::Working;
                s.turn_started_at.get_or_insert(now);
                if e.tool.is_some() {
                    s.last_tool = e.tool;
                }
            }
            ClaudeHook::PermissionRequest => {
                outcome = self.attention(i, SessionStatus::Attention, &e, now);
            }
            ClaudeHook::Notification => match e.notification {
                Some(NotificationKind::PermissionPrompt) | Some(NotificationKind::InputRequest) => {
                    outcome = self.attention(i, SessionStatus::Attention, &e, now);
                }
                Some(NotificationKind::IdlePrompt) => {
                    outcome = self.attention(i, SessionStatus::Waiting, &e, now);
                }
                // Other notifications carry nothing Islandy shows.
                Some(NotificationKind::Other) | None => {}
            },
            ClaudeHook::Stop => {
                persist |= self.clear_attention(i);
                let s = &mut self.sessions[i];
                s.status = SessionStatus::Idle;
                s.turn_started_at = None;
                s.subagents = 0;
                if !self.duplicate(&key("stop"), 3 * SECOND, now) {
                    let draft = Draft::new(
                        ActivitySource::ClaudeCode,
                        EventKind::ClaudeTurnFinished,
                        "Claude finished responding",
                        Severity::Success,
                    )
                    .priority(Priority::P1)
                    .session(&e.session)
                    .project(e.project.clone());
                    outcome = self.recorded(draft, now);
                }
            }
            ClaudeHook::StopFailure => {
                persist |= self.clear_attention(i);
                let s = &mut self.sessions[i];
                s.status = SessionStatus::Error;
                s.turn_started_at = None;
                s.subagents = 0;
                if !self.duplicate(&key("error"), 10 * SECOND, now) {
                    let draft = Draft::new(
                        ActivitySource::ClaudeCode,
                        EventKind::ClaudeError,
                        "Claude Code stopped with an error",
                        Severity::Error,
                    )
                    .priority(Priority::P1)
                    .session(&e.session)
                    .project(e.project.clone())
                    .describe(Some(
                        stop_error_text(e.error.unwrap_or(StopError::Unknown)).into(),
                    ));
                    outcome = self.recorded(draft, now);
                }
            }
            ClaudeHook::SubagentStart => {
                let s = &mut self.sessions[i];
                s.subagents = s.subagents.saturating_add(1).min(99);
                if matches!(s.status, SessionStatus::Ready | SessionStatus::Idle) {
                    s.status = SessionStatus::Working;
                    s.turn_started_at.get_or_insert(now);
                }
            }
            ClaudeHook::SubagentStop => {
                let s = &mut self.sessions[i];
                s.subagents = s.subagents.saturating_sub(1);
            }
            ClaudeHook::SessionEnd => {
                persist |= self.clear_attention(i);
                let s = &mut self.sessions[i];
                s.status = SessionStatus::Ended;
                s.ended_at = Some(now);
                s.turn_started_at = None;
                s.subagents = 0;
                if !self.duplicate(&key("end"), 10 * SECOND, now) {
                    let draft = Draft::new(
                        ActivitySource::ClaudeCode,
                        EventKind::ClaudeSessionEnded,
                        "Claude Code session ended",
                        Severity::Info,
                    )
                    .priority(Priority::P3)
                    .session(&e.session)
                    .project(e.project.clone());
                    outcome = self.recorded(draft, now);
                }
            }
        }

        // `last_seen_at` alone doesn't republish (it is read when something
        // visible changes); everything else does.
        let after = &self.sessions[i];
        let visible_change = after.status != before.status
            || after.project != before.project
            || after.last_tool != before.last_tool
            || after.subagents != before.subagents
            || after.attention_event != before.attention_event
            || after.attention_dismissed != before.attention_dismissed
            || after.started_at != before.started_at
            || after.turn_started_at != before.turn_started_at
            || after.stale != before.stale;
        outcome.changed |= visible_change || persist;
        outcome.persist |= persist;
        outcome
    }

    /// Opens (or keeps) an attention request for session `i`.
    fn attention(&mut self, i: usize, status: SessionStatus, e: &ClaudeWire, now: u64) -> Outcome {
        let open = {
            let s = &self.sessions[i];
            s.attention_event.is_some() && s.status == status
        };
        if open {
            // Same request reported twice (PermissionRequest followed by a
            // permission_prompt notification): nothing new to show.
            return Outcome::default();
        }
        // A different request supersedes the previous one.
        self.clear_attention(i);
        self.sessions[i].status = status;
        let (kind, title, detail) = match (status, e.event, e.notification) {
            (SessionStatus::Waiting, _, _) => (
                EventKind::ClaudeWaitingForInput,
                "Claude Code is waiting for input",
                None,
            ),
            (_, ClaudeHook::PermissionRequest, _)
            | (_, _, Some(NotificationKind::PermissionPrompt)) => (
                EventKind::ClaudeAttention,
                "Claude Code needs attention",
                Some("Permission requested"),
            ),
            _ => (
                EventKind::ClaudeAttention,
                "Claude Code needs attention",
                Some("Input requested"),
            ),
        };
        let attention_key = format!("claude:{}:attention", e.session);
        // Also bounded in time: a flapping hook can't spam peeks.
        let repeat = self.duplicate(&format!("{attention_key}:{status:?}"), 5 * SECOND, now);
        let draft = Draft::new(ActivitySource::ClaudeCode, kind, title, Severity::Attention)
            .priority(Priority::P0)
            .active(None)
            .session(&e.session)
            .project(
                e.project
                    .clone()
                    .or_else(|| self.sessions[i].project.clone()),
            )
            .describe(detail.map(str::to_owned))
            .key(attention_key);
        let (event, peek) = self.record(draft, now);
        self.sessions[i].attention_event = Some(event.id);
        Outcome {
            changed: true,
            persist: true,
            peek: if repeat { None } else { peek },
        }
    }

    /* ------------------------------------------------------------------ */
    /* Developer events                                                    */
    /* ------------------------------------------------------------------ */

    fn developer(&mut self, e: DeveloperWire, now: u64) -> Outcome {
        use DeveloperEvent as D;
        let family = match e.event {
            D::BuildStarted | D::BuildSucceeded | D::BuildFailed => "build",
            D::TestsStarted | D::TestsPassed | D::TestsFailed => "tests",
            D::ServerStarted | D::ServerStopped => "server",
            D::Message => "message",
        };
        let task = format!(
            "dev:{family}:{}:{}",
            e.project.as_deref().unwrap_or("").to_lowercase(),
            e.label.as_deref().unwrap_or("").to_lowercase()
        );

        // Two CLI processes can race (PowerShell doesn't wait for GUI
        // programs): drop an event older than one already seen for the task.
        if let Some(at) = e.at.filter(|at| at.abs_diff(now) <= CLOCK_SKEW_MS) {
            if e.event != D::Message {
                match self.task_clock.get(&task) {
                    Some(&seen) if at < seen => return Outcome::default(),
                    _ => {
                        if self.task_clock.len() >= DEDUPE_CAPACITY {
                            self.task_clock.clear();
                        }
                        self.task_clock.insert(task.clone(), at);
                    }
                }
            }
        }

        let running = format!("{task}:running");
        let base = |kind, title: &str, severity| {
            Draft::new(ActivitySource::Developer, kind, title, severity)
                .project(e.project.clone())
                .describe(e.label.clone())
        };
        let (draft, window) = match e.event {
            D::BuildStarted | D::TestsStarted => {
                let (kind, title) = if e.event == D::BuildStarted {
                    (EventKind::BuildStarted, "Build started")
                } else {
                    (EventKind::TestsStarted, "Tests started")
                };
                (
                    base(kind, title, Severity::Info)
                        .priority(Priority::P3)
                        .active(Some(now + TASK_TIMEOUT_MS))
                        .key(running.clone()),
                    2 * SECOND,
                )
            }
            D::BuildSucceeded => (
                base(
                    EventKind::BuildSucceeded,
                    "Build succeeded",
                    Severity::Success,
                ),
                5 * SECOND,
            ),
            D::TestsPassed => (
                base(EventKind::TestsPassed, "Tests passed", Severity::Success),
                5 * SECOND,
            ),
            D::BuildFailed => (
                base(EventKind::BuildFailed, "Build failed", Severity::Error)
                    .priority(Priority::P1),
                5 * SECOND,
            ),
            D::TestsFailed => (
                base(EventKind::TestsFailed, "Tests failed", Severity::Error)
                    .priority(Priority::P1),
                5 * SECOND,
            ),
            D::ServerStarted => (
                base(
                    EventKind::ServerStarted,
                    "Dev server started",
                    Severity::Info,
                )
                .active(Some(now + SERVER_TIMEOUT_MS))
                .key(running.clone()),
                5 * SECOND,
            ),
            D::ServerStopped => (
                base(
                    EventKind::ServerStopped,
                    "Dev server stopped",
                    Severity::Info,
                )
                .priority(Priority::P3),
                5 * SECOND,
            ),
            D::Message => {
                let severity = e.severity.unwrap_or(Severity::Info);
                let priority = match severity {
                    Severity::Error | Severity::Warning => Priority::P1,
                    _ => Priority::P2,
                };
                let title = e.title.clone().unwrap_or_default();
                (
                    base(EventKind::Message, &title, severity).priority(priority),
                    5 * SECOND,
                )
            }
        };

        // Identical outcomes for the same task within the window are one
        // event (e.g. a watcher reporting the same build twice). The title
        // is part of the key only for free-form messages of the same task.
        let dedupe = match e.event {
            D::Message => format!("{task}:message:{}", draft.title.to_lowercase()),
            other => format!("{task}:{other:?}"),
        };
        if self.duplicate(&dedupe, window, now) {
            return Outcome::default();
        }
        // A new run, outcome or stop supersedes the running one.
        let resolved = if e.event == D::Message {
            false
        } else {
            self.resolve_key(&running)
        };
        let mut outcome = self.recorded(draft, now);
        outcome.changed |= resolved;
        outcome
    }

    /* ------------------------------------------------------------------ */
    /* System alerts (Phase 3)                                             */
    /* ------------------------------------------------------------------ */

    fn system(&mut self, alert: &SystemAlert, now: u64) -> Outcome {
        let ongoing = Some(now + SYSTEM_TIMEOUT_MS);
        let draft =
            |kind, title: &str, severity| Draft::new(ActivitySource::System, kind, title, severity);
        let (draft, key, window, resolves): (Draft, String, u64, Option<&str>) = match alert {
            SystemAlert::BatteryLow { percent } => (
                draft(EventKind::BatteryLow, "Battery low", Severity::Warning)
                    .priority(Priority::P1)
                    .describe(Some(format!("{percent}% remaining")))
                    .active(ongoing),
                "system:battery".into(),
                MINUTE,
                None,
            ),
            SystemAlert::BatteryCritical { percent } => (
                draft(
                    EventKind::BatteryCritical,
                    "Battery critically low",
                    Severity::Error,
                )
                .priority(Priority::P1)
                .describe(Some(format!("{percent}% remaining")))
                .active(ongoing),
                "system:battery-critical".into(),
                MINUTE,
                Some("system:battery"),
            ),
            SystemAlert::PowerConnected { percent } => (
                draft(EventKind::PowerConnected, "Power connected", Severity::Info)
                    .describe(percent.map(|p| format!("{p}%"))),
                "system:power-connected".into(),
                10 * SECOND,
                Some("system:battery"),
            ),
            SystemAlert::PowerDisconnected { percent } => (
                draft(EventKind::PowerDisconnected, "On battery", Severity::Info)
                    .describe(percent.map(|p| format!("{p}% remaining"))),
                "system:power-disconnected".into(),
                10 * SECOND,
                None,
            ),
            SystemAlert::NetworkLost => (
                draft(
                    EventKind::NetworkLost,
                    "Network disconnected",
                    Severity::Warning,
                )
                .priority(Priority::P1)
                .describe(Some("No internet connection".into()))
                .active(ongoing),
                "system:network".into(),
                30 * SECOND,
                None,
            ),
            SystemAlert::NetworkRestored { kind, name } => (
                draft(
                    EventKind::NetworkRestored,
                    match kind {
                        ConnectionKind::Wifi => "Connected to Wi-Fi",
                        ConnectionKind::Ethernet => "Connected to Ethernet",
                        _ => "Back online",
                    },
                    Severity::Success,
                )
                .describe(name.clone()),
                "system:network-restored".into(),
                30 * SECOND,
                Some("system:network"),
            ),
            SystemAlert::AudioOutputChanged { name } => (
                draft(
                    EventKind::AudioOutputChanged,
                    "Audio output changed",
                    Severity::Info,
                )
                .describe(Some(name.clone())),
                format!("system:audio:{name}"),
                10 * SECOND,
                None,
            ),
        };
        if self.duplicate(&key, window, now) {
            return Outcome::default();
        }
        let mut resolved = false;
        if let Some(k) = resolves {
            resolved |= self.resolve_key(k);
            if k == "system:battery" {
                resolved |= self.resolve_key("system:battery-critical");
            }
        }
        let draft = draft.key(key);
        let mut outcome = self.recorded(draft, now);
        outcome.changed |= resolved;
        outcome
    }

    /* ------------------------------------------------------------------ */
    /* Time                                                                */
    /* ------------------------------------------------------------------ */

    /// Expires events, marks sessions stale and drops sessions that have
    /// been silent too long. Call at [`next_deadline`](Self::next_deadline).
    pub fn tick(&mut self, now: u64) -> Outcome {
        let mut outcome = Outcome::default();
        for e in self.events.iter_mut() {
            if e.lifecycle == Lifecycle::Active && e.expires_at.is_some_and(|t| t <= now) {
                e.lifecycle = Lifecycle::Expired;
                outcome.changed = true;
                outcome.persist = true;
            }
        }
        let len = self.events.len();
        self.enforce_retention(now);
        if self.events.len() != len {
            outcome.changed = true;
            outcome.persist = true;
        }
        for s in self.sessions.iter_mut() {
            let stale = s.status == SessionStatus::Working
                && now.saturating_sub(s.last_seen_at) >= STALE_AFTER_MS;
            if stale != s.stale {
                s.stale = stale;
                outcome.changed = true;
            }
        }
        let mut expired = Vec::new();
        let listed = self.sessions.len();
        self.sessions.retain(|s| {
            let keep = removal_time(s).map_or(true, |t| t > now);
            if !keep {
                if let Some(id) = &s.attention_event {
                    expired.push(id.clone());
                }
            }
            keep
        });
        if self.sessions.len() != listed {
            outcome.changed = true;
        }
        for id in expired {
            if let Some(e) = self.events.iter_mut().find(|e| e.id == id) {
                if e.lifecycle == Lifecycle::Active {
                    e.lifecycle = Lifecycle::Expired;
                    outcome.persist = true;
                }
            }
        }
        outcome
    }

    /// The next time [`tick`](Self::tick) has something to do.
    pub fn next_deadline(&self) -> Option<u64> {
        let events = self.events.iter().filter_map(|e| match e.lifecycle {
            Lifecycle::Active => e.expires_at,
            _ => None,
        });
        let retention = self
            .events
            .front()
            .map(|e| e.timestamp + self.retention_ms() + 1);
        let sessions = self.sessions.iter().flat_map(|s| {
            let stale = (s.status == SessionStatus::Working && !s.stale)
                .then_some(s.last_seen_at + STALE_AFTER_MS);
            [stale, removal_time(s)]
        });
        events.chain(retention).chain(sessions.flatten()).min()
    }

    /* ------------------------------------------------------------------ */
    /* User actions                                                        */
    /* ------------------------------------------------------------------ */

    /// Dismisses an active event (it stays in history, marked dismissed) or
    /// removes a finished one from history.
    pub fn dismiss(&mut self, id: &str) -> Outcome {
        let Some(index) = self.events.iter().position(|e| e.id == id) else {
            return Outcome::default();
        };
        if self.events[index].lifecycle == Lifecycle::Active {
            self.events[index].lifecycle = Lifecycle::Dismissed;
            for s in self.sessions.iter_mut() {
                if s.attention_event.as_deref() == Some(id) {
                    s.attention_dismissed = true;
                }
            }
        } else {
            self.events.remove(index);
        }
        Outcome {
            changed: true,
            persist: true,
            peek: None,
        }
    }

    /// Deletes all history. Open attention indicators are dismissed too.
    pub fn clear_history(&mut self) -> Outcome {
        self.events.clear();
        for s in self.sessions.iter_mut() {
            if s.attention_event.take().is_some() {
                s.attention_dismissed = true;
            }
        }
        Outcome {
            changed: true,
            persist: true,
            peek: None,
        }
    }

    pub fn set_quiet(&mut self, quiet: bool) -> Outcome {
        let changed = self.settings.quiet != quiet;
        self.settings.quiet = quiet;
        Outcome {
            changed,
            persist: changed,
            peek: None,
        }
    }

    /// Replaces all activity settings at once (settings window). Sources
    /// that are switched off behave like [`Self::set_source`]; a shorter
    /// retention applies immediately.
    pub fn apply_settings(&mut self, next: ActivitySettings, now: u64) -> Outcome {
        let next = next.normalized();
        let previous = self.settings;
        if previous == next {
            return Outcome::default();
        }
        for source in [
            ActivitySource::ClaudeCode,
            ActivitySource::Developer,
            ActivitySource::System,
        ] {
            if previous.sources.get(source) != next.sources.get(source) {
                self.set_source(source, next.sources.get(source));
            }
        }
        self.settings = next;
        self.enforce_retention(now);
        Outcome {
            changed: true,
            persist: true,
            peek: None,
        }
    }

    /// Disabling a source stops recording it; Claude sessions are forgotten.
    pub fn set_source(&mut self, source: ActivitySource, on: bool) -> Outcome {
        let changed = self.settings.sources.get(source) != on;
        self.settings.sources.set(source, on);
        if !on && source == ActivitySource::ClaudeCode {
            self.sessions.clear();
        }
        if !on {
            for e in self.events.iter_mut() {
                if e.source == source && e.lifecycle == Lifecycle::Active {
                    e.lifecycle = Lifecycle::Resolved;
                }
            }
        }
        Outcome {
            changed,
            persist: changed,
            peek: None,
        }
    }
}

/// Lower ranks are listed first.
fn session_rank(s: &ClaudeSession) -> u8 {
    match s.status {
        SessionStatus::Attention | SessionStatus::Waiting if !s.attention_dismissed => 0,
        SessionStatus::Working if !s.stale => 1,
        SessionStatus::Ended => 3,
        _ => 2,
    }
}

/// Higher ranks are evicted first.
fn eviction_rank(s: &ClaudeSession) -> u8 {
    match s.status {
        SessionStatus::Ended => 3,
        SessionStatus::Idle | SessionStatus::Ready | SessionStatus::Error => 2,
        SessionStatus::Working if s.stale => 1,
        _ => 0,
    }
}

/// When a silent session stops being listed.
fn removal_time(s: &ClaudeSession) -> Option<u64> {
    Some(match s.status {
        SessionStatus::Ended => s.ended_at.unwrap_or(s.last_seen_at) + ENDED_LINGER_MS,
        SessionStatus::Ready | SessionStatus::Idle | SessionStatus::Error => {
            s.last_seen_at + IDLE_LINGER_MS
        }
        SessionStatus::Working | SessionStatus::Attention | SessionStatus::Waiting => {
            s.last_seen_at + ACTIVE_LINGER_MS
        }
    })
}

fn parse_id(id: &str) -> Option<u64> {
    id.strip_prefix('a')
        .and_then(|hex| u64::from_str_radix(hex, 16).ok())
}

fn stop_error_text(error: StopError) -> &'static str {
    match error {
        StopError::RateLimit => "Rate limit reached",
        StopError::Overloaded => "Service overloaded",
        StopError::Authentication => "Authentication problem",
        StopError::Billing => "Billing problem",
        StopError::InvalidRequest => "Request was rejected",
        StopError::Server => "Server error",
        StopError::MaxOutputTokens => "Output limit reached",
        StopError::Unknown => "API error",
    }
}

#[cfg(test)]
mod tests;
