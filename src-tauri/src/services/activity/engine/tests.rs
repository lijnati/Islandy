use super::*;
use crate::services::activity::{
    model::{PeekDuration, ToolCategory},
    protocol::{session_key, ClaudeHook as H},
};

const T0: u64 = 1_700_000_000_000;

fn engine() -> ActivityEngine {
    ActivityEngine::new(ActivitySettings::default(), T0)
}

fn claude(session: &str, event: H) -> ClaudeWire {
    ClaudeWire {
        event,
        session: session_key(session),
        project: Some("Islandy".into()),
        tool: None,
        notification: None,
        error: None,
        subagent: false,
    }
}

fn tool(session: &str, event: H, tool: ToolCategory) -> Input {
    Input::Claude(ClaudeWire {
        tool: Some(tool),
        ..claude(session, event)
    })
}

fn notify(session: &str, kind: NotificationKind) -> Input {
    Input::Claude(ClaudeWire {
        notification: Some(kind),
        ..claude(session, H::Notification)
    })
}

fn c(session: &str, event: H) -> Input {
    Input::Claude(claude(session, event))
}

fn dev(event: DeveloperEvent) -> DeveloperWire {
    DeveloperWire {
        event,
        project: Some("Islandy".into()),
        label: None,
        title: None,
        severity: None,
        at: None,
    }
}

fn session<'a>(snap: &'a ActivitySnapshot, name: &str) -> &'a ClaudeSession {
    let key = session_key(name);
    snap.sessions
        .iter()
        .find(|s| s.id == key)
        .expect("session listed")
}

fn snap(e: &ActivityEngine) -> ActivitySnapshot {
    e.snapshot(1, ReceiverStatus::Listening, false)
}

/* ---------------------------------------------------------------------- */
/* Claude Code lifecycle                                                   */
/* ---------------------------------------------------------------------- */

#[test]
fn a_full_turn_maps_conservatively() {
    let mut e = engine();
    let out = e.ingest(c("s1", H::SessionStart), T0);
    assert!(
        out.changed && out.peek.is_none(),
        "session start never peeks"
    );
    assert_eq!(session(&snap(&e), "s1").status, SessionStatus::Ready);
    assert_eq!(session(&snap(&e), "s1").started_at, Some(T0));

    e.ingest(c("s1", H::UserPromptSubmit), T0 + 1_000);
    let s = snap(&e);
    assert_eq!(session(&s, "s1").status, SessionStatus::Working);
    assert_eq!(session(&s, "s1").turn_started_at, Some(T0 + 1_000));

    // Tool activity updates the indicator but never peeks or records history.
    let before = e.history().count();
    let out = e.ingest(tool("s1", H::PreToolUse, ToolCategory::Editing), T0 + 2_000);
    assert!(out.peek.is_none() && !out.persist);
    assert_eq!(
        session(&snap(&e), "s1").last_tool,
        Some(ToolCategory::Editing)
    );
    // Identical tool events don't republish either.
    let out = e.ingest(
        tool("s1", H::PostToolUse, ToolCategory::Editing),
        T0 + 2_500,
    );
    assert!(!out.changed);
    assert_eq!(e.history().count(), before);

    let out = e.ingest(c("s1", H::Stop), T0 + 10_000);
    let peek = out.peek.expect("turn finished peeks");
    assert_eq!(peek.event.kind, EventKind::ClaudeTurnFinished);
    assert_eq!(peek.event.priority, Priority::P1);
    assert_eq!(peek.dwell_ms, 4_000);
    // "Finished responding", never "task complete".
    assert_eq!(peek.event.title, "Claude finished responding");
    let s = snap(&e);
    assert_eq!(session(&s, "s1").status, SessionStatus::Idle);
    assert_eq!(session(&s, "s1").turn_started_at, None);

    let out = e.ingest(c("s1", H::SessionEnd), T0 + 20_000);
    assert!(out.peek.is_none());
    assert_eq!(session(&snap(&e), "s1").status, SessionStatus::Ended);
    // Ended sessions are listed briefly, then dropped.
    e.tick(T0 + 20_000 + ENDED_LINGER_MS);
    assert!(snap(&e).sessions.is_empty());
}

#[test]
fn permission_requests_raise_one_persistent_attention() {
    let mut e = engine();
    e.ingest(c("s1", H::UserPromptSubmit), T0);
    e.ingest(tool("s1", H::PreToolUse, ToolCategory::Running), T0 + 100);
    let out = e.ingest(
        Input::Claude(ClaudeWire {
            tool: Some(ToolCategory::Running),
            ..claude("s1", H::PermissionRequest)
        }),
        T0 + 200,
    );
    let peek = out.peek.expect("attention peeks");
    assert_eq!(peek.event.priority, Priority::P0);
    assert_eq!(peek.event.severity, Severity::Attention);
    assert_eq!(peek.event.lifecycle, Lifecycle::Active);
    assert_eq!(
        peek.event.description.as_deref(),
        Some("Permission requested")
    );
    assert_eq!(
        peek.event.expires_at, None,
        "attention never times out on its own"
    );
    let s = snap(&e);
    assert_eq!(session(&s, "s1").status, SessionStatus::Attention);
    assert_eq!(
        session(&s, "s1").attention_event.as_ref(),
        Some(&peek.event.id)
    );

    // The matching notification seconds later is the same request.
    let out = e.ingest(notify("s1", NotificationKind::PermissionPrompt), T0 + 6_200);
    assert_eq!(out, Outcome::default());
    // A parallel PreToolUse doesn't answer it either.
    e.ingest(tool("s1", H::PreToolUse, ToolCategory::Reading), T0 + 6_300);
    assert_eq!(session(&snap(&e), "s1").status, SessionStatus::Attention);

    // The tool ran: the request was answered.
    let out = e.ingest(
        tool("s1", H::PostToolUse, ToolCategory::Running),
        T0 + 9_000,
    );
    assert!(out.changed && out.persist);
    let s = snap(&e);
    assert_eq!(session(&s, "s1").status, SessionStatus::Working);
    assert_eq!(session(&s, "s1").attention_event, None);
    let event = s.events.iter().find(|x| x.id == peek.event.id).unwrap();
    assert_eq!(event.lifecycle, Lifecycle::Resolved);
}

#[test]
fn idle_prompts_mean_waiting_until_the_user_answers() {
    let mut e = engine();
    e.ingest(c("s1", H::Stop), T0);
    let out = e.ingest(notify("s1", NotificationKind::IdlePrompt), T0 + 60_000);
    assert_eq!(
        out.peek.unwrap().event.kind,
        EventKind::ClaudeWaitingForInput
    );
    assert_eq!(session(&snap(&e), "s1").status, SessionStatus::Waiting);
    e.ingest(c("s1", H::UserPromptSubmit), T0 + 90_000);
    let s = snap(&e);
    assert_eq!(session(&s, "s1").status, SessionStatus::Working);
    assert!(s.events.iter().all(|x| x.lifecycle != Lifecycle::Active));
}

#[test]
fn other_notifications_are_ignored() {
    let mut e = engine();
    e.ingest(c("s1", H::UserPromptSubmit), T0);
    let out = e.ingest(notify("s1", NotificationKind::Other), T0 + 1);
    assert!(!out.changed && out.peek.is_none());
    assert_eq!(session(&snap(&e), "s1").status, SessionStatus::Working);
}

#[test]
fn stop_failures_are_errors_with_a_safe_reason() {
    let mut e = engine();
    let out = e.ingest(
        Input::Claude(ClaudeWire {
            error: Some(StopError::RateLimit),
            ..claude("s1", H::StopFailure)
        }),
        T0,
    );
    let peek = out.peek.unwrap();
    assert_eq!(peek.event.severity, Severity::Error);
    assert_eq!(peek.dwell_ms, 6_000);
    assert_eq!(
        peek.event.description.as_deref(),
        Some("Rate limit reached")
    );
    assert_eq!(session(&snap(&e), "s1").status, SessionStatus::Error);
}

#[test]
fn sessions_are_tracked_independently_even_in_one_project() {
    let mut e = engine();
    e.ingest(c("a", H::UserPromptSubmit), T0);
    e.ingest(c("b", H::UserPromptSubmit), T0 + 1);
    e.ingest(c("b", H::PermissionRequest), T0 + 2);
    e.ingest(c("a", H::Stop), T0 + 3);
    let s = snap(&e);
    assert_eq!(s.sessions.len(), 2);
    assert_eq!(session(&s, "a").status, SessionStatus::Idle);
    assert_eq!(session(&s, "b").status, SessionStatus::Attention);
    // Attention is listed first.
    assert_eq!(s.sessions[0].id, session_key("b"));
    // Both share the project name, not an identity.
    assert_eq!(s.sessions[0].project, s.sessions[1].project);
}

#[test]
fn sessions_seen_mid_turn_have_an_unknown_start() {
    // Islandy (re)started while Claude Code kept running.
    let mut e = engine();
    e.ingest(tool("s1", H::PostToolUse, ToolCategory::Searching), T0);
    let s = snap(&e);
    assert_eq!(session(&s, "s1").status, SessionStatus::Working);
    assert_eq!(session(&s, "s1").started_at, None);
}

#[test]
fn silence_means_unknown_never_success() {
    let mut e = engine();
    e.ingest(c("s1", H::UserPromptSubmit), T0);
    assert_eq!(e.next_deadline(), Some(T0 + STALE_AFTER_MS));
    let out = e.tick(T0 + STALE_AFTER_MS);
    assert!(out.changed && out.peek.is_none());
    let s = snap(&e);
    assert!(session(&s, "s1").stale);
    assert_eq!(
        session(&s, "s1").status,
        SessionStatus::Working,
        "not 'completed'"
    );
    assert!(s
        .events
        .iter()
        .all(|x| x.kind != EventKind::ClaudeTurnFinished));
    // Any event clears staleness.
    e.ingest(
        tool("s1", H::PostToolUse, ToolCategory::Running),
        T0 + STALE_AFTER_MS + 5,
    );
    assert!(!session(&snap(&e), "s1").stale);
    // A long silence eventually removes the session without a verdict.
    e.tick(T0 + STALE_AFTER_MS + 5 + ACTIVE_LINGER_MS);
    assert!(snap(&e).sessions.is_empty());
    assert!(e.history().all(|x| x.kind != EventKind::ClaudeTurnFinished));
}

#[test]
fn abandoned_attention_expires_with_its_session() {
    let mut e = engine();
    e.ingest(c("s1", H::PermissionRequest), T0);
    let out = e.tick(T0 + ACTIVE_LINGER_MS);
    assert!(out.changed && out.persist);
    assert!(snap(&e).sessions.is_empty());
    assert_eq!(e.history().next().unwrap().lifecycle, Lifecycle::Expired);
}

#[test]
fn subagents_are_counted_and_reset() {
    let mut e = engine();
    e.ingest(c("s1", H::SubagentStart), T0);
    e.ingest(c("s1", H::SubagentStart), T0 + 1);
    assert_eq!(session(&snap(&e), "s1").subagents, 2);
    assert_eq!(session(&snap(&e), "s1").status, SessionStatus::Working);
    e.ingest(c("s1", H::SubagentStop), T0 + 2);
    e.ingest(c("s1", H::SubagentStop), T0 + 3);
    e.ingest(c("s1", H::SubagentStop), T0 + 4); // a missed start can't underflow
    assert_eq!(session(&snap(&e), "s1").subagents, 0);
}

#[test]
fn session_count_is_bounded() {
    let mut e = engine();
    for i in 0..(MAX_SESSIONS + 5) {
        e.ingest(c(&format!("s{i}"), H::Stop), T0 + i as u64 * 10_000);
    }
    let s = snap(&e);
    assert_eq!(s.sessions.len(), MAX_SESSIONS);
    // The newest are kept.
    assert!(s
        .sessions
        .iter()
        .any(|x| x.id == session_key(&format!("s{}", MAX_SESSIONS + 4))));
}

#[test]
fn repeated_stops_are_deduplicated_per_session() {
    let mut e = engine();
    assert!(e.ingest(c("a", H::Stop), T0).peek.is_some());
    assert!(e.ingest(c("a", H::Stop), T0 + 500).peek.is_none());
    // A different session is not a duplicate.
    assert!(e.ingest(c("b", H::Stop), T0 + 600).peek.is_some());
    // After the window, it is a new outcome.
    assert!(e.ingest(c("a", H::Stop), T0 + 5_000).peek.is_some());
    assert_eq!(e.history().count(), 3);
}

/* ---------------------------------------------------------------------- */
/* Developer events                                                        */
/* ---------------------------------------------------------------------- */

#[test]
fn builds_start_ambiently_and_finish_with_a_peek() {
    let mut e = engine();
    let out = e.ingest(Input::Developer(dev(DeveloperEvent::BuildStarted)), T0);
    assert!(out.changed && out.peek.is_none(), "started is ambient");
    let started = e.history().last().unwrap().clone();
    assert_eq!(started.lifecycle, Lifecycle::Active);
    assert_eq!(started.expires_at, Some(T0 + TASK_TIMEOUT_MS));

    let out = e.ingest(
        Input::Developer(dev(DeveloperEvent::BuildFailed)),
        T0 + 30_000,
    );
    let peek = out.peek.unwrap();
    assert_eq!((peek.event.priority, peek.dwell_ms), (Priority::P1, 6_000));
    assert_eq!(
        e.history().find(|x| x.id == started.id).unwrap().lifecycle,
        Lifecycle::Resolved
    );

    // Success is informational.
    e.ingest(
        Input::Developer(dev(DeveloperEvent::TestsStarted)),
        T0 + 40_000,
    );
    let out = e.ingest(
        Input::Developer(dev(DeveloperEvent::TestsPassed)),
        T0 + 50_000,
    );
    let peek = out.peek.unwrap();
    assert_eq!((peek.event.priority, peek.dwell_ms), (Priority::P2, 4_000));
}

#[test]
fn started_tasks_expire_instead_of_failing() {
    let mut e = engine();
    e.ingest(Input::Developer(dev(DeveloperEvent::BuildStarted)), T0);
    assert_eq!(e.next_deadline(), Some(T0 + TASK_TIMEOUT_MS));
    let out = e.tick(T0 + TASK_TIMEOUT_MS);
    assert!(out.changed && out.peek.is_none());
    assert_eq!(e.history().last().unwrap().lifecycle, Lifecycle::Expired);
}

#[test]
fn build_bursts_are_deduplicated_but_different_tasks_are_not() {
    let mut e = engine();
    let mut peeks = 0;
    for i in 0..50 {
        let out = e.ingest(
            Input::Developer(dev(DeveloperEvent::BuildSucceeded)),
            T0 + i * 20,
        );
        peeks += usize::from(out.peek.is_some());
    }
    assert_eq!(peeks, 1);
    assert_eq!(e.history().count(), 1);
    // Same title ("Build succeeded"), different project: not a duplicate.
    let other = DeveloperWire {
        project: Some("Other".into()),
        ..dev(DeveloperEvent::BuildSucceeded)
    };
    assert!(e.ingest(Input::Developer(other), T0 + 1_100).peek.is_some());
}

#[test]
fn out_of_order_cli_events_are_dropped() {
    let mut e = engine();
    let at = |event, at| {
        Input::Developer(DeveloperWire {
            at: Some(at),
            ..dev(event)
        })
    };
    e.ingest(at(DeveloperEvent::BuildSucceeded, T0 + 10), T0 + 20);
    // "started" ran first but arrived late: it must not reopen the build.
    let out = e.ingest(at(DeveloperEvent::BuildStarted, T0), T0 + 30);
    assert_eq!(out, Outcome::default());
    assert!(e.history().all(|x| x.lifecycle != Lifecycle::Active));
}

#[test]
fn messages_use_their_severity() {
    let mut e = engine();
    let msg = |title: &str, severity| {
        Input::Developer(DeveloperWire {
            title: Some(title.into()),
            severity,
            ..dev(DeveloperEvent::Message)
        })
    };
    let out = e.ingest(msg("Deploy blocked", Some(Severity::Warning)), T0);
    assert_eq!(out.peek.unwrap().event.priority, Priority::P1);
    let out = e.ingest(msg("Lint clean", None), T0 + 1);
    let peek = out.peek.unwrap();
    assert_eq!(
        (peek.event.severity, peek.dwell_ms),
        (Severity::Info, 3_000)
    );
    assert_eq!(peek.event.title, "Lint clean");
}

/* ---------------------------------------------------------------------- */
/* System alerts                                                           */
/* ---------------------------------------------------------------------- */

#[test]
fn system_alerts_are_prioritized_and_resolve_each_other() {
    let mut e = engine();
    let out = e.ingest(Input::System(SystemAlert::NetworkLost), T0);
    let lost = out.peek.unwrap().event;
    assert_eq!(
        (lost.priority, lost.lifecycle),
        (Priority::P1, Lifecycle::Active)
    );
    // Repeats within the window are dropped.
    assert_eq!(
        e.ingest(Input::System(SystemAlert::NetworkLost), T0 + 1_000),
        Outcome::default()
    );
    let out = e.ingest(
        Input::System(SystemAlert::NetworkRestored {
            kind: ConnectionKind::Wifi,
            name: Some("Home".into()),
        }),
        T0 + 5_000,
    );
    let restored = out.peek.unwrap().event;
    assert_eq!(restored.priority, Priority::P2);
    assert_eq!(restored.title, "Connected to Wi-Fi");
    assert_eq!(e.history().next().unwrap().lifecycle, Lifecycle::Resolved);

    let out = e.ingest(Input::System(SystemAlert::BatteryLow { percent: 18 }), T0);
    assert_eq!(out.peek.unwrap().event.severity, Severity::Warning);
    e.ingest(
        Input::System(SystemAlert::PowerConnected { percent: Some(18) }),
        T0 + 2,
    );
    assert!(e.history().all(|x| x.lifecycle != Lifecycle::Active));
}

/* ---------------------------------------------------------------------- */
/* Settings, user actions, persistence                                     */
/* ---------------------------------------------------------------------- */

#[test]
fn quiet_mode_suppresses_peeks_but_keeps_state() {
    let mut e = engine();
    assert!(e.set_quiet(true).persist);
    let out = e.ingest(c("s1", H::PermissionRequest), T0);
    assert!(out.changed && out.peek.is_none());
    let s = snap(&e);
    assert_eq!(session(&s, "s1").status, SessionStatus::Attention);
    assert_eq!(s.events.len(), 1);
    assert!(s.settings.quiet);
}

#[test]
fn disabled_sources_record_nothing() {
    let mut e = engine();
    e.ingest(c("s1", H::UserPromptSubmit), T0);
    e.set_source(ActivitySource::ClaudeCode, false);
    assert!(snap(&e).sessions.is_empty());
    assert_eq!(e.ingest(c("s1", H::Stop), T0 + 1), Outcome::default());
    assert_eq!(e.history().count(), 0);
    e.set_source(ActivitySource::Developer, false);
    assert_eq!(
        e.ingest(Input::Developer(dev(DeveloperEvent::BuildFailed)), T0),
        Outcome::default()
    );
    // System still works.
    assert!(
        e.ingest(Input::System(SystemAlert::NetworkLost), T0)
            .changed
    );
}

#[test]
fn dismissing_attention_hides_the_indicator_until_a_new_request() {
    let mut e = engine();
    let id = e
        .ingest(c("s1", H::PermissionRequest), T0)
        .peek
        .unwrap()
        .event
        .id;
    assert!(e.dismiss(&id).changed);
    let s = snap(&e);
    assert!(session(&s, "s1").attention_dismissed);
    assert_eq!(s.events[0].lifecycle, Lifecycle::Dismissed);
    // The session's status doesn't pretend the request was answered.
    assert_eq!(session(&s, "s1").status, SessionStatus::Attention);
    // Dismissing a finished event removes it from history.
    assert!(e.dismiss(&id).changed);
    assert_eq!(e.history().count(), 0);
    assert_eq!(e.dismiss("a0"), Outcome::default());
}

#[test]
fn clearing_history_removes_everything() {
    let mut e = engine();
    e.ingest(c("s1", H::PermissionRequest), T0);
    e.ingest(Input::Developer(dev(DeveloperEvent::BuildFailed)), T0);
    e.clear_history();
    let s = snap(&e);
    assert!(s.events.is_empty());
    assert!(session(&s, "s1").attention_dismissed);
    assert_eq!(session(&s, "s1").attention_event, None);
}

#[test]
fn history_is_bounded_by_count_and_age() {
    let mut e = engine();
    for i in 0..(MAX_EVENTS as u64 + 40) {
        let msg = DeveloperWire {
            title: Some(format!("m{i}")),
            ..dev(DeveloperEvent::Message)
        };
        e.ingest(Input::Developer(msg), T0 + i);
    }
    assert_eq!(e.history().count(), MAX_EVENTS);
    assert_eq!(snap(&e).events.len(), SNAPSHOT_EVENTS);
    assert_eq!(snap(&e).events[0].title, format!("m{}", MAX_EVENTS + 39));
    let out = e.tick(T0 + RETENTION_MS + 100_000);
    assert!(out.persist);
    assert_eq!(e.history().count(), 0);
}

#[test]
fn restore_expires_active_events_and_keeps_ids_unique() {
    let mut e = engine();
    e.ingest(c("s1", H::PermissionRequest), T0);
    e.ingest(Input::Developer(dev(DeveloperEvent::BuildStarted)), T0 + 1);
    let saved: Vec<_> = e.history().cloned().collect();

    // Restart: same clock (worst case for id collisions).
    let mut r = ActivityEngine::new(ActivitySettings::default(), T0);
    r.restore(saved.clone(), T0 + 10);
    assert!(r.history().all(|x| x.lifecycle == Lifecycle::Expired));
    assert!(snap(&r).sessions.is_empty(), "sessions are not persisted");
    let out = r.ingest(Input::Developer(dev(DeveloperEvent::BuildFailed)), T0 + 20);
    let new_id = out.peek.unwrap().event.id;
    assert!(saved.iter().all(|x| x.id != new_id));
    assert!(parse_id(&new_id).unwrap() > parse_id(&saved[1].id).unwrap());

    // Events older than the retention window are dropped on load.
    let mut old = ActivityEngine::new(ActivitySettings::default(), T0);
    old.restore(saved, T0 + RETENTION_MS + 10);
    assert_eq!(old.history().count(), 0);
}

#[test]
fn dedupe_memory_is_bounded_under_bursts() {
    let mut e = engine();
    for i in 0..2_000u64 {
        e.ingest(
            Input::System(SystemAlert::AudioOutputChanged {
                name: format!("Device {i}"),
            }),
            T0 + i,
        );
    }
    assert!(e.dedupe.len() <= DEDUPE_CAPACITY);
    assert_eq!(e.history().count(), MAX_EVENTS);
}

#[test]
fn deadlines_cover_every_timer() {
    let mut e = engine();
    assert_eq!(e.next_deadline(), None);
    e.ingest(c("s1", H::Stop), T0);
    // Idle session removal, and retention of its event.
    assert_eq!(e.next_deadline(), Some(T0 + IDLE_LINGER_MS));
    e.ingest(Input::Developer(dev(DeveloperEvent::BuildStarted)), T0 + 5);
    assert_eq!(e.next_deadline(), Some(T0 + IDLE_LINGER_MS));
    e.ingest(c("s2", H::UserPromptSubmit), T0 + 7);
    assert_eq!(e.next_deadline(), Some(T0 + 7 + STALE_AFTER_MS));
}

#[test]
fn dwell_times_follow_the_priority_table() {
    assert_eq!(dwell_ms(Priority::P0, Severity::Attention), Some(8_000));
    assert_eq!(dwell_ms(Priority::P1, Severity::Success), Some(4_000));
    assert_eq!(dwell_ms(Priority::P1, Severity::Error), Some(6_000));
    assert_eq!(dwell_ms(Priority::P2, Severity::Info), Some(3_000));
    assert_eq!(dwell_ms(Priority::P2, Severity::Success), Some(4_000));
    assert_eq!(dwell_ms(Priority::P3, Severity::Error), None);
}

/* ---------------------------------------------------------------------- */
/* Notification preferences (Phase 5)                                      */
/* ---------------------------------------------------------------------- */

fn message(title: &str, severity: Severity) -> Input {
    Input::Developer(DeveloperWire {
        title: Some(title.into()),
        severity: Some(severity),
        ..dev(DeveloperEvent::Message)
    })
}

fn with(f: impl FnOnce(&mut ActivitySettings)) -> ActivityEngine {
    let mut settings = ActivitySettings::default();
    f(&mut settings);
    ActivityEngine::new(settings, T0)
}

#[test]
fn peek_switches_filter_by_source_and_outcome_but_still_record() {
    let mut e = with(|s| s.peeks.developer = false);
    let out = e.ingest(Input::Developer(dev(DeveloperEvent::BuildFailed)), T0);
    assert!(out.changed && out.peek.is_none());
    assert_eq!(snap(&e).events.len(), 1, "recorded anyway");

    let mut e = with(|s| s.peeks.success = false);
    assert!(e
        .ingest(message("ok", Severity::Success), T0)
        .peek
        .is_none());
    assert!(e.ingest(message("bad", Severity::Error), T0).peek.is_some());

    let mut e = with(|s| s.peeks.errors = false);
    assert!(e.ingest(message("bad", Severity::Error), T0).peek.is_none());
    assert!(e
        .ingest(message("warn", Severity::Warning), T0)
        .peek
        .is_none());
    assert!(e
        .ingest(message("ok", Severity::Success), T0)
        .peek
        .is_some());

    // Claude attention respects its source switch (the pill indicator stays).
    let mut e = with(|s| s.peeks.claude_code = false);
    let out = e.ingest(c("s1", H::PermissionRequest), T0);
    assert!(out.peek.is_none());
    assert_eq!(session(&snap(&e), "s1").status, SessionStatus::Attention);
}

#[test]
fn peek_duration_scales_dwell_times() {
    let mut short = with(|s| s.peek_duration = PeekDuration::Short);
    let mut long = with(|s| s.peek_duration = PeekDuration::Long);
    let failed = || Input::Developer(dev(DeveloperEvent::BuildFailed));
    assert_eq!(short.ingest(failed(), T0).peek.unwrap().dwell_ms, 4_500);
    assert_eq!(long.ingest(failed(), T0).peek.unwrap().dwell_ms, 9_000);
}

#[test]
fn one_noisy_source_cannot_dominate_the_island() {
    let mut e = engine();
    let mut peeks = 0;
    for i in 0..20 {
        let title = format!("message {i}");
        if e.ingest(message(&title, Severity::Info), T0 + i * 1000)
            .peek
            .is_some()
        {
            peeks += 1;
        }
    }
    assert_eq!(peeks, PEEK_BUDGET, "rate limited within a minute");
    assert_eq!(snap(&e).events.len(), 20, "every event is still recorded");
    // Another source is unaffected.
    let out = e.ingest(c("s1", H::Stop), T0 + 21_000);
    assert!(out.peek.is_some());
    // P0 attention is never rate limited.
    for i in 0..6 {
        let s = format!("p{i}");
        assert!(e
            .ingest(c(&s, H::PermissionRequest), T0 + 22_000 + i)
            .peek
            .is_some());
    }
    // The budget refills after the window.
    let out = e.ingest(
        message("later", Severity::Info),
        T0 + 20_000 + PEEK_WINDOW_MS,
    );
    assert!(out.peek.is_some());
}

#[test]
fn retention_follows_the_setting() {
    let mut e = with(|s| s.retention_days = 1);
    e.ingest(message("old", Severity::Info), T0);
    e.ingest(message("new", Severity::Info), T0 + 20 * HOUR);
    e.tick(T0 + DAY + HOUR);
    let titles: Vec<_> = snap(&e).events.iter().map(|e| e.title.clone()).collect();
    assert_eq!(titles, ["new"]);
    assert_eq!(e.next_deadline(), Some(T0 + 20 * HOUR + DAY + 1));
}

#[test]
fn applying_settings_switches_sources_and_shortens_retention() {
    let mut e = engine();
    e.ingest(c("s1", H::SessionStart), T0);
    e.ingest(message("a", Severity::Info), T0);
    let mut next = ActivitySettings::default();
    next.sources.claude_code = false;
    next.retention_days = 200; // clamped to 30
    let out = e.apply_settings(next, T0 + 5 * DAY);
    assert!(out.changed && out.persist);
    assert!(snap(&e).sessions.is_empty(), "Claude sessions forgotten");
    assert_eq!(e.settings().retention_days, 30);
    assert!(!e.apply_settings(e.settings(), T0).changed, "no-op");
    next.retention_days = 1;
    e.apply_settings(next, T0 + 5 * DAY);
    assert!(
        snap(&e).events.is_empty(),
        "older than a day: removed at once"
    );
}

#[test]
fn last_received_tracks_unrecorded_tool_events() {
    let mut e = engine();
    assert_eq!(snap(&e).last_received, LastReceived::default());
    e.ingest(tool("s1", H::PreToolUse, ToolCategory::Reading), T0 + 5);
    assert_eq!(snap(&e).last_received.claude_code, Some(T0 + 5));
    assert_eq!(snap(&e).last_received.developer, None);
}
