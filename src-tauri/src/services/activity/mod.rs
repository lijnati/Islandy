//! The smart activity engine and its local integrations.
//!
//! ```text
//!  Claude Code ──hook (async, exec form)──► `islandy hook` ─┐
//!  scripts ──────────────────────────────► `islandy notify` ─┤ reduced, validated events
//!                                                            ▼ (token, loopback TCP)
//!                                               receiver thread ──┐
//!  Phase 3 SystemHub alerts ──────────────────────────────────────┼─► activity worker
//!  Tauri commands (dismiss, quiet, sources) ──────────────────────┘   (ActivityEngine)
//!                                                                      │ snapshot / peek
//!                                                                      ▼
//!                                                 Channel<ActivityUpdate> ──► UI
//!                                                 activity.json (bounded, atomic)
//! ```
//!
//! - [`model`]           IPC and storage types (mirrored in `src/services/types.ts`)
//! - [`protocol`]        wire format, token file, client
//! - [`engine`]          the deterministic engine (priorities, dedupe, sessions)
//! - [`receiver`]        loopback listener (auth, limits)
//! - [`store`]           bounded persistence
//! - [`claude_settings`] opt-in hook installation in `~/.claude/settings.json`
//! - [`claude_desktop`]  Claude Desktop (chat) reply detection on Windows
//!
//! Everything is event-driven: the worker sleeps until the next message or
//! the engine's next deadline. Nothing polls Claude Code. Claude Desktop has
//! no hooks, so its watcher probes the Claude window (see that module).

pub mod claude_desktop;
pub mod claude_settings;
pub mod engine;
pub mod model;
pub mod protocol;
pub mod receiver;
pub mod store;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, MutexGuard,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use self::{
    engine::{ActivityEngine, Input, Outcome},
    model::{ActivitySettings, ActivitySnapshot, ActivitySource, ActivityUpdate, ReceiverStatus},
    protocol::{ClaudeWire, Decision, Rejection, WireEvent},
    receiver::{Receiver, Responder},
};
use crate::services::{paths, system::models::SystemAlert};

/// Coalesces bursts of state changes (e.g. tool calls) into one update.
const PUBLISH_DELAY: Duration = Duration::from_millis(120);
/// Debounces history writes.
const PERSIST_DELAY: Duration = Duration::from_millis(1500);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(800);

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// User actions on activity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityCommand {
    Dismiss(String),
    ClearHistory,
    SetQuiet(bool),
    SetSource(ActivitySource, bool),
    SetSettings(ActivitySettings),
    /// The user's answer to permission question `id` of a session.
    Answer {
        session: String,
        id: u64,
        decision: Decision,
    },
}

enum Message {
    Input(Input),
    /// A permission request whose bridge waits for the island's answer.
    Ask(ClaudeWire, Responder),
    Command(ActivityCommand),
    Receiver(ReceiverStatus),
    Shutdown,
}

type Sink = Box<dyn Fn(ActivityUpdate) + Send>;

struct Hub {
    sink: Option<Sink>,
    latest: ActivitySnapshot,
}

/// Holds the UI sink and the latest snapshot under one lock, so a UI that
/// connects can't miss an update published in between.
struct SharedHub(Mutex<Hub>);

impl SharedHub {
    fn lock(&self) -> MutexGuard<'_, Hub> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn publish(&self, snapshot: ActivitySnapshot) {
        let mut hub = self.lock();
        hub.latest = snapshot.clone();
        if let Some(sink) = &hub.sink {
            sink(ActivityUpdate::Snapshot { snapshot });
        }
    }
    fn peek(&self, update: ActivityUpdate) {
        if let Some(sink) = &self.lock().sink {
            sink(update);
        }
    }
}

/// Handle to the activity worker and receiver, managed as Tauri state.
pub struct ActivityService {
    tx: mpsc::Sender<Message>,
    hub: Arc<SharedHub>,
    worker: Mutex<Option<JoinHandle<()>>>,
    receiver: Mutex<Option<Receiver>>,
    /// The Claude source is on (the Claude Desktop watcher idles otherwise).
    claude_on: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
}

impl ActivityService {
    /// Loads history, starts the worker and the local receiver. A receiver
    /// failure (e.g. no data directory) is reported in the snapshot; the
    /// rest of Islandy keeps working.
    pub fn start() -> std::io::Result<Self> {
        let dir = paths::data_dir();
        Self::start_in(dir, true)
    }

    fn start_in(dir: Option<PathBuf>, listen: bool) -> std::io::Result<Self> {
        let now = now_ms();
        let loaded = dir.as_deref().map(store::load).unwrap_or_default();
        let mut engine = ActivityEngine::new(loaded.settings, now);
        engine.restore(loaded.events, now);
        if loaded.recovered {
            crate::log_warn!(
                "activity",
                "activity history was unreadable; started a new one"
            );
        }
        let claude_on = Arc::new(AtomicBool::new(engine.settings().sources.claude_code));
        let initial = engine.snapshot(1, ReceiverStatus::Starting, dir.is_none());
        let hub = Arc::new(SharedHub(Mutex::new(Hub {
            sink: None,
            latest: initial,
        })));
        let (tx, rx) = mpsc::channel();
        let worker = {
            let hub = Arc::clone(&hub);
            let dir = dir.clone();
            std::thread::Builder::new()
                .name("islandy-activity".into())
                .spawn(move || Worker::new(engine, hub, dir).run(&rx))?
        };
        let service = Self {
            tx,
            hub,
            worker: Mutex::new(Some(worker)),
            receiver: Mutex::new(None),
            claude_on,
            stopping: Arc::new(AtomicBool::new(false)),
        };
        if listen {
            service.start_receiver(dir);
            service.start_claude_desktop_watch();
        }
        Ok(service)
    }

    fn start_receiver(&self, dir: Option<PathBuf>) {
        let tx = self.tx.clone();
        let sink: receiver::Sink = Box::new(move |event, responder| {
            let message = match (event, responder) {
                (WireEvent::Claude(e), Some(r)) => Message::Ask(e, r),
                (WireEvent::Claude(e), None) => Message::Input(Input::Claude(e)),
                (WireEvent::Developer(e), _) => Message::Input(Input::Developer(e)),
                (WireEvent::Ping, _) => return Ok(()),
            };
            tx.send(message).map_err(|_| Rejection::Unavailable)
        });
        let status = match dir.map(|d| Receiver::start(&d, sink)) {
            Some(Ok(receiver)) => {
                *self.receiver.lock().unwrap_or_else(|e| e.into_inner()) = Some(receiver);
                ReceiverStatus::Listening
            }
            Some(Err(e)) => {
                crate::log_warn!("activity", "activity receiver unavailable: {e}");
                ReceiverStatus::Unavailable
            }
            None => ReceiverStatus::Unavailable,
        };
        let _ = self.tx.send(Message::Receiver(status));
    }

    /// Watches Claude Desktop (Windows only; elsewhere nothing starts).
    fn start_claude_desktop_watch(&self) {
        // Not in tests: they must not probe the real desktop.
        if !cfg!(windows) || cfg!(test) {
            return;
        }
        let tx = self.tx.clone();
        let on = Arc::clone(&self.claude_on);
        let stopping = Arc::clone(&self.stopping);
        let spawned = std::thread::Builder::new()
            .name("islandy-claude-desktop".into())
            .spawn(move || {
                let Some(mut prober) = claude_desktop::Prober::new() else {
                    crate::log_warn!(
                        "activity",
                        "UI Automation unavailable; Claude Desktop isn't watched"
                    );
                    return;
                };
                let mut tracker = claude_desktop::Tracker::default();
                while !stopping.load(Ordering::Relaxed) {
                    let probe = if on.load(Ordering::Relaxed) {
                        prober.probe()
                    } else {
                        claude_desktop::Probe::NotRunning
                    };
                    if let Some(wire) = tracker.update(probe, now_ms()) {
                        if tx.send(Message::Input(Input::Claude(wire))).is_err() {
                            return;
                        }
                    }
                    std::thread::sleep(claude_desktop::POLL);
                }
            });
        if let Err(e) = spawned {
            crate::log_warn!("activity", "couldn't start the Claude Desktop watcher: {e}");
        }
    }

    /// Replaces the UI sink and returns the latest snapshot atomically.
    pub fn connect(&self, sink: impl Fn(ActivityUpdate) + Send + 'static) -> ActivitySnapshot {
        let mut hub = self.hub.lock();
        hub.sink = Some(Box::new(sink));
        hub.latest.clone()
    }

    pub fn snapshot(&self) -> ActivitySnapshot {
        self.hub.lock().latest.clone()
    }

    /// Fire-and-forget: the result arrives as a snapshot.
    pub fn command(&self, command: ActivityCommand) {
        match &command {
            ActivityCommand::SetSource(ActivitySource::ClaudeCode, on) => {
                self.claude_on.store(*on, Ordering::Relaxed);
            }
            ActivityCommand::SetSettings(s) => {
                self.claude_on
                    .store(s.sources.claude_code, Ordering::Relaxed);
            }
            _ => {}
        }
        let _ = self.tx.send(Message::Command(command));
    }

    /// Phase 3 alerts become activity (history + prioritized peeks).
    pub fn alert_observer(&self) -> impl Fn(&SystemAlert) + Send + 'static {
        let tx = Mutex::new(self.tx.clone());
        move |alert: &SystemAlert| {
            let tx = tx.lock().unwrap_or_else(|e| e.into_inner());
            let _ = tx.send(Message::Input(Input::System(alert.clone())));
        }
    }

    #[cfg(test)]
    fn submit(&self, input: Input) {
        let _ = self.tx.send(Message::Input(input));
    }

    /// Stops the receiver (removing its connection file) and the worker,
    /// which writes pending history first. Bounded.
    pub fn shutdown(&self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(mut receiver) = self
            .receiver
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            receiver.shutdown();
        }
        let _ = self.tx.send(Message::Shutdown);
        if let Some(worker) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
            while !worker.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}

#[cfg(debug_assertions)]
fn log_input(input: &Input) {
    match input {
        Input::Claude(e) => crate::log_debug!(
            "activity",
            "activity: claude {:?} {} tool={:?} notification={:?}",
            e.event,
            e.session,
            e.tool,
            e.notification
        ),
        Input::Developer(e) => crate::log_warn!("activity", "activity: developer {:?}", e.event),
        Input::System(_) => crate::log_warn!("activity", "activity: system alert"),
    }
}

struct Worker {
    engine: ActivityEngine,
    hub: Arc<SharedHub>,
    dir: Option<PathBuf>,
    revision: u64,
    receiver: ReceiverStatus,
    history_error: bool,
    publish_at: Option<Instant>,
    persist_at: Option<Instant>,
    /// Bridges waiting for an answer: session → (question id, connection).
    /// Kept in step with the engine's open questions; a dropped responder
    /// answers `pass`.
    waiting: HashMap<String, (u64, Responder)>,
}

impl Worker {
    fn new(engine: ActivityEngine, hub: Arc<SharedHub>, dir: Option<PathBuf>) -> Self {
        Self {
            engine,
            hub,
            history_error: dir.is_none(),
            dir,
            revision: 1,
            receiver: ReceiverStatus::Starting,
            publish_at: None,
            persist_at: None,
            waiting: HashMap::new(),
        }
    }

    fn run(mut self, rx: &mpsc::Receiver<Message>) {
        loop {
            let message = match self.wait() {
                Some(timeout) => match rx.recv_timeout(timeout) {
                    Ok(m) => Some(m),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                },
                None => match rx.recv() {
                    Ok(m) => Some(m),
                    Err(_) => break,
                },
            };
            match message {
                Some(Message::Input(input)) => {
                    // Debug builds only; event names and opaque keys, no content.
                    #[cfg(debug_assertions)]
                    log_input(&input);
                    let outcome = self.engine.ingest(input, now_ms());
                    self.apply(outcome, false);
                }
                Some(Message::Ask(e, responder)) => {
                    #[cfg(debug_assertions)]
                    log_input(&Input::Claude(e.clone()));
                    let session = e.session.clone();
                    let outcome = self.engine.ingest(Input::Claude(e), now_ms());
                    if let Some(id) = self.engine.question(&session) {
                        // Replaces (and so passes on) an older question.
                        self.waiting.insert(session, (id, responder));
                    }
                    self.apply(outcome, true);
                }
                Some(Message::Command(ActivityCommand::Answer {
                    session,
                    id,
                    decision,
                })) => {
                    let (outcome, open) = self.engine.answer(&session, id, decision);
                    if open {
                        if let Some((waiting_id, responder)) = self.waiting.remove(&session) {
                            if waiting_id == id {
                                responder.answer(decision);
                            }
                        }
                    }
                    self.apply(outcome, true);
                }
                Some(Message::Command(command)) => {
                    let outcome = match command {
                        ActivityCommand::Dismiss(id) => self.engine.dismiss(&id),
                        ActivityCommand::ClearHistory => {
                            if let Some(dir) = &self.dir {
                                store::delete(dir);
                            }
                            self.engine.clear_history()
                        }
                        ActivityCommand::SetQuiet(q) => self.engine.set_quiet(q),
                        ActivityCommand::SetSource(s, on) => self.engine.set_source(s, on),
                        ActivityCommand::SetSettings(settings) => {
                            self.engine.apply_settings(settings, now_ms())
                        }
                        ActivityCommand::Answer { .. } => Outcome::default(),
                    };
                    // User actions are reflected immediately.
                    self.apply(outcome, true);
                }
                Some(Message::Receiver(status)) => {
                    self.receiver = status;
                    self.publish();
                }
                Some(Message::Shutdown) => break,
                None => {}
            }
            self.run_due();
            // Questions that closed (answered in Claude Code, expired,
            // superseded, setting off) release their bridge.
            let engine = &self.engine;
            self.waiting
                .retain(|session, (id, _)| engine.question(session) == Some(*id));
        }
        self.persist();
    }

    /// Time until the earliest pending deadline (`None`: sleep until a message).
    fn wait(&self) -> Option<Duration> {
        let now = Instant::now();
        let engine = self
            .engine
            .next_deadline()
            .map(|at| now + Duration::from_millis(at.saturating_sub(now_ms())));
        [engine, self.publish_at, self.persist_at]
            .into_iter()
            .flatten()
            .min()
            .map(|at| at.saturating_duration_since(now))
    }

    fn run_due(&mut self) {
        if self.engine.next_deadline().is_some_and(|at| at <= now_ms()) {
            let outcome = self.engine.tick(now_ms());
            self.apply(outcome, false);
        }
        let now = Instant::now();
        if self.publish_at.is_some_and(|at| at <= now) {
            self.publish();
        }
        if self.persist_at.is_some_and(|at| at <= now) {
            self.persist();
        }
    }

    fn apply(&mut self, outcome: Outcome, immediate: bool) {
        if outcome.persist && self.persist_at.is_none() {
            self.persist_at = Some(Instant::now() + PERSIST_DELAY);
        }
        if outcome.peek.is_some() || (immediate && outcome.changed) {
            // The UI gets the state the peek belongs to first.
            self.publish();
        } else if outcome.changed && self.publish_at.is_none() {
            self.publish_at = Some(Instant::now() + PUBLISH_DELAY);
        }
        if let Some(peek) = outcome.peek {
            self.hub.peek(ActivityUpdate::Peek { peek });
        }
    }

    fn publish(&mut self) {
        self.publish_at = None;
        self.revision += 1;
        self.hub.publish(
            self.engine
                .snapshot(self.revision, self.receiver, self.history_error),
        );
    }

    fn persist(&mut self) {
        let Some(at) = self.persist_at.take() else {
            return;
        };
        let _ = at;
        let Some(dir) = &self.dir else { return };
        let ok = store::save(dir, &self.engine.settings(), self.engine.history()).is_ok();
        if ok == self.history_error {
            self.history_error = !ok;
            self.publish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::activity::{
        model::{EventKind, Lifecycle},
        protocol::{session_key, ClaudeHook, ClaudeWire, DeveloperEvent, DeveloperWire},
    };

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("islandy-activity-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn collect() -> (
        Arc<Mutex<Vec<ActivityUpdate>>>,
        impl Fn(ActivityUpdate) + Send + 'static,
    ) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let seen = Arc::clone(&seen);
            move |u| seen.lock().unwrap().push(u)
        };
        (seen, sink)
    }

    fn wait_for(seen: &Mutex<Vec<ActivityUpdate>>, f: impl Fn(&[ActivityUpdate]) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if f(&seen.lock().unwrap()) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("condition not met: {:?}", seen.lock().unwrap());
    }

    fn stop(session: &str) -> Input {
        Input::Claude(ClaudeWire {
            event: ClaudeHook::Stop,
            session: session_key(session),
            project: Some("Islandy".into()),
            tool: None,
            notification: None,
            error: None,
            subagent: false,
            ask: None,
        })
    }

    #[test]
    fn delivers_state_before_peeks_and_persists_on_shutdown() {
        let dir = temp_dir("service");
        let service = ActivityService::start_in(Some(dir.clone()), false).unwrap();
        let (seen, sink) = collect();
        let initial = service.connect(sink);
        assert!(initial.events.is_empty());
        service.submit(stop("s1"));
        wait_for(&seen, |u| {
            u.iter().any(|u| matches!(u, ActivityUpdate::Peek { .. }))
        });
        {
            let seen = seen.lock().unwrap();
            let peek_at = seen
                .iter()
                .position(|u| matches!(u, ActivityUpdate::Peek { .. }))
                .unwrap();
            let ActivityUpdate::Snapshot { snapshot } = &seen[peek_at - 1] else {
                panic!("a snapshot precedes the peek");
            };
            assert_eq!(snapshot.events[0].kind, EventKind::ClaudeTurnFinished);
        }
        service.command(ActivityCommand::SetQuiet(true));
        wait_for(
            &seen,
            |u| matches!(u.last(), Some(ActivityUpdate::Snapshot { snapshot }) if snapshot.settings.quiet),
        );
        service.shutdown();

        // A new instance restores history and settings, without peeks.
        let restarted = ActivityService::start_in(Some(dir.clone()), false).unwrap();
        let snapshot = restarted.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert!(snapshot.settings.quiet);
        assert!(snapshot.sessions.is_empty());
        restarted.shutdown();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn coalesces_bursts_into_few_snapshots() {
        let service = ActivityService::start_in(None, false).unwrap();
        let (seen, sink) = collect();
        service.connect(sink);
        for i in 0..40 {
            service.submit(Input::Claude(ClaudeWire {
                event: if i % 2 == 0 {
                    ClaudeHook::PreToolUse
                } else {
                    ClaudeHook::PostToolUse
                },
                session: session_key("s1"),
                project: Some("Islandy".into()),
                tool: Some(if i % 4 < 2 {
                    model::ToolCategory::Reading
                } else {
                    model::ToolCategory::Editing
                }),
                notification: None,
                error: None,
                subagent: false,
                ask: None,
            }));
        }
        std::thread::sleep(PUBLISH_DELAY * 3);
        let count = seen.lock().unwrap().len();
        assert!(count <= 4, "{count} snapshots for 40 tool events");
        assert!(count >= 1);
        service.shutdown();
    }

    #[test]
    fn end_to_end_through_the_receiver() {
        let dir = temp_dir("e2e");
        let service = ActivityService::start_in(Some(dir.clone()), true).unwrap();
        let (seen, sink) = collect();
        service.connect(sink);
        // The "listening" snapshot may be published before `connect`, so
        // poll the latest snapshot rather than the updates seen since.
        let deadline = Instant::now() + Duration::from_secs(5);
        while service.snapshot().receiver != ReceiverStatus::Listening {
            assert!(Instant::now() < deadline, "the receiver never listened");
            std::thread::sleep(Duration::from_millis(10));
        }
        let build = |event| {
            WireEvent::Developer(DeveloperWire {
                event,
                project: Some("Islandy".into()),
                label: None,
                title: None,
                severity: None,
                at: None,
            })
        };
        protocol::send(&dir, &build(DeveloperEvent::BuildStarted)).unwrap();
        protocol::send(&dir, &build(DeveloperEvent::BuildFailed)).unwrap();
        wait_for(&seen, |u| {
            u.iter().any(|u| matches!(u, ActivityUpdate::Peek { peek } if peek.event.kind == EventKind::BuildFailed))
        });
        let snapshot = service.snapshot();
        assert_eq!(snapshot.events[1].lifecycle, Lifecycle::Resolved);
        service.shutdown();
        assert_eq!(
            protocol::send(&dir, &build(DeveloperEvent::BuildStarted)),
            Err(protocol::SendError::NotRunning)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_permission_request_is_answered_from_the_island() {
        let dir = temp_dir("answer");
        let service = ActivityService::start_in(Some(dir.clone()), true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while service.snapshot().receiver != ReceiverStatus::Listening {
            assert!(Instant::now() < deadline, "the receiver never listened");
            std::thread::sleep(Duration::from_millis(10));
        }
        service.command(ActivityCommand::SetSettings(ActivitySettings {
            answer_permissions: true,
            ..ActivitySettings::default()
        }));
        let ask = WireEvent::Claude(ClaudeWire {
            event: ClaudeHook::PermissionRequest,
            session: session_key("s"),
            project: Some("Islandy".into()),
            tool: None,
            notification: None,
            error: None,
            subagent: false,
            ask: Some(protocol::PermissionAsk {
                summary: "cargo test".into(),
                complete: true,
            }),
        });
        let bridge = {
            let dir = dir.clone();
            std::thread::spawn(move || protocol::ask(&dir, &ask, Duration::from_secs(10)))
        };
        let question = loop {
            let snapshot = service.snapshot();
            if let Some(q) = snapshot.sessions.first().and_then(|s| s.question.clone()) {
                break q;
            }
            assert!(
                Instant::now() < deadline + Duration::from_secs(5),
                "no question"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(question.summary, "cargo test");
        service.command(ActivityCommand::Answer {
            session: session_key("s"),
            id: question.id,
            decision: Decision::Deny,
        });
        assert_eq!(bridge.join().unwrap(), Decision::Deny);
        service.shutdown();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
