use super::*;
use crate::agent::club::{ChatMsg, ClubReply, ToolCall, ToolDef};
use std::sync::Mutex;

const LIMITS: WatchdogLimits = WatchdogLimits {
    check_secs: 1800,
    review_secs: 3600,
};

fn aged(secs: u64) -> Instant {
    Instant::now()
        .checked_sub(Duration::from_secs(secs))
        .expect("monotonic clock supports backdating in tests")
}

#[test]
fn verdicts_come_from_the_last_verdict_line() {
    assert_eq!(
        parse_verdict("hung\nverdict: stop_call"),
        Some(Verdict::StopCall)
    );
    assert_eq!(
        parse_verdict("verdict: wait\nreconsidered\n**Verdict:** `restart_turn`."),
        Some(Verdict::RestartTurn)
    );
    assert_eq!(parse_verdict("VERDICT: handoff"), Some(Verdict::HandOff));
    assert_eq!(parse_verdict("verdict: hand-off"), Some(Verdict::HandOff));
    assert_eq!(parse_verdict("it should wait"), None);
    assert_eq!(parse_verdict("verdict: solve the task"), None);
    assert_eq!(parse_verdict(""), None);
}

#[test]
fn the_fixed_rule_hands_a_running_call_off_then_restarts() {
    assert_eq!(fallback_verdict(true, false), Verdict::HandOff);
    assert_eq!(
        fallback_verdict(true, true),
        Verdict::RestartTurn,
        "a release already failed to free the turn"
    );
    assert_eq!(fallback_verdict(false, false), Verdict::RestartTurn);
}

#[test]
fn the_check_comes_at_thirty_minutes_and_the_review_at_sixty() {
    let due = |quiet, checked, next, released| watchdog_due(LIMITS, quiet, checked, next, released);
    assert_eq!(due(1799, false, 3600, None), WatchdogDue::Nothing);
    assert_eq!(due(1800, false, 3600, None), WatchdogDue::Check);
    assert_eq!(due(3599, true, 3600, None), WatchdogDue::Nothing);
    assert_eq!(due(3600, true, 3600, None), WatchdogDue::Review);
    // A frame gap past both: the check still comes first.
    assert_eq!(due(7200, false, 3600, None), WatchdogDue::Check);
    // After a release, only the grace clock matters.
    assert_eq!(
        due(9000, true, 3600, Some(RELEASE_GRACE_SECS - 1)),
        WatchdogDue::Nothing
    );
    assert_eq!(
        due(9000, true, 3600, Some(RELEASE_GRACE_SECS)),
        WatchdogDue::Escalate
    );
    // No soft check configured.
    let no_check = WatchdogLimits {
        check_secs: 0,
        review_secs: 3600,
    };
    assert_eq!(
        watchdog_due(no_check, 3000, false, 3600, None),
        WatchdogDue::Nothing
    );
    assert_eq!(
        watchdog_due(no_check, 3600, false, 3600, None),
        WatchdogDue::Review
    );
}

#[test]
fn a_zero_review_limit_turns_the_watchdog_off() {
    let _lock = crate::tests::env_lock();
    let _review = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_WATCHDOG_REVIEW_SECS");
    let _check = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_WATCHDOG_CHECK_SECS");
    assert_eq!(watchdog_limits(), Some(LIMITS));
    let _off = crate::tests::TestEnvGuard::set("ANGEL_LOOP_WATCHDOG_REVIEW_SECS", "0");
    assert_eq!(watchdog_limits(), None);
}

/// A connected reviewer: reads the route its prompt opens with, keeps the
/// page and the facts, then answers with its scripted reply.
struct Seat {
    reply: &'static str,
    page: Mutex<Option<String>>,
    facts: Mutex<Option<String>>,
}

impl Seat {
    fn new(reply: &'static str) -> Arc<Self> {
        Arc::new(Self {
            reply,
            page: Mutex::new(None),
            facts: Mutex::new(None),
        })
    }
}

impl Club for Seat {
    fn label(&self) -> &str {
        "watchdog-seat"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("a connected seat is called through chat".into())
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if let Some(page) = messages
            .iter()
            .rev()
            .find(|m| m.tool_call_id.as_deref() == Some("route"))
        {
            *self.page.lock().unwrap() = Some(page.content.to_string());
            return Ok(ClubReply::Text(self.reply.to_string()));
        }
        let prompt = messages.last().unwrap().content.to_string();
        let route = prompt.lines().next().unwrap().to_string();
        *self.facts.lock().unwrap() = Some(prompt);
        Ok(ClubReply::Calls(vec![ToolCall {
            id: "route".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path": format!("ledger://{route}")}),
        }]))
    }
}

struct Fixture {
    root: PathBuf,
    _file: crate::tests::TestEnvGuard,
    _mirror: crate::tests::TestEnvGuard,
    _review: crate::tests::TestEnvGuard,
    _check: crate::tests::TestEnvGuard,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn wedged_loop(tag: &str, quiet_secs: u64) -> (crate::App, Thinking, Fixture) {
    let root = std::env::temp_dir().join(format!("angel-watchdog-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let fixture = Fixture {
        _file: crate::tests::TestEnvGuard::set(
            "ANGEL_LOOP_FILE",
            root.join("loop.json").to_str().unwrap(),
        ),
        _mirror: crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR"),
        _review: crate::tests::TestEnvGuard::unset("ANGEL_LOOP_WATCHDOG_REVIEW_SECS"),
        _check: crate::tests::TestEnvGuard::unset("ANGEL_LOOP_WATCHDOG_CHECK_SECS"),
        root: root.clone(),
    };
    let mut app = crate::seed_preview_app();
    assert!(
        app.loop_local_club()
            .filter(|club| club.label() != "practice" && club.is_available())
            .is_none(),
        "the preview app offers no local reviewer; the turn's own seat reviews"
    );
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        task: "r31".into(),
        workspace: Some(root),
        awaiting_turn: true,
        iteration: 63,
        podrace: true,
        ..Default::default()
    };
    let mut thinking = Thinking::pending_for_test("practice");
    thinking.started = aged(quiet_secs + 30);
    thinking.last_stream_at = aged(quiet_secs);
    (app, thinking, fixture)
}

fn wedge_on_a_solver(app: &mut crate::App) {
    app.tool_strip.call_event(
        crate::agent::harness::ToolEventId("solve".into()),
        "shell",
        "gcc -O2 -o /tmp/r31_wordram_sat r31_wordram_sat.c && /tmp/r31_wordram_sat /tmp/r31-start.cnf",
    );
}

fn owner(thinking: &Thinking) -> usize {
    Arc::as_ptr(&thinking.cancel) as usize
}

/// Drive the watchdog until the review it dispatched has been harvested.
fn harvest(app: &mut crate::App, thinking: &Thinking) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let restart = app.loop_watchdog(thinking);
        if app.loop_watch.review.is_none() {
            return restart;
        }
        assert!(Instant::now() < deadline, "the review never landed");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn half_an_hour_quiet_is_a_soft_check_once_with_no_model() {
    let _lock = crate::tests::env_lock();
    let (mut app, thinking, _fixture) = wedged_loop("check", 31 * 60);
    wedge_on_a_solver(&mut app);

    assert_eq!(app.loop_watchdog(&thinking), None);
    assert!(
        app.loop_watch.review.is_none(),
        "no reviewer at the soft check"
    );
    assert_eq!(app.loop_ctl.watchdog.len(), 1);
    let check = &app.loop_ctl.watchdog[0];
    assert_eq!(check.kind, "check");
    assert_eq!(check.iteration, 63);
    assert!(check.quiet_secs >= 31 * 60);
    assert!(
        check.detail.contains("loop iteration 64 in flight"),
        "{}",
        check.detail
    );
    assert!(check.detail.contains("(podrace)"), "{}", check.detail);
    assert!(
        check.detail.contains("in flight: shell for"),
        "{}",
        check.detail
    );
    assert!(
        check.detail.contains("/tmp/r31_wordram_sat"),
        "{}",
        check.detail
    );
    assert!(app.messages.iter().any(|m| {
        m.text
            .contains("loop watchdog: no harness progress for 31m")
            && m.text.contains("soft check")
    }));
    assert_eq!(load().unwrap().watchdog, app.loop_ctl.watchdog, "persisted");

    // Once per quiet stretch.
    assert_eq!(app.loop_watchdog(&thinking), None);
    assert_eq!(app.loop_ctl.watchdog.len(), 1);
    assert!(!crate::agent::harness::release_pending(owner(&thinking)));
}

#[test]
fn an_hour_quiet_sends_a_reviewer_whose_stop_call_becomes_a_release() {
    let _lock = crate::tests::env_lock();
    let (mut app, mut thinking, _fixture) = wedged_loop("review", 61 * 60);
    wedge_on_a_solver(&mut app);
    let seat = Seat::new("a CPU-bound solver with no output for an hour\nverdict: stop_call");
    thinking.club = Some(seat.clone() as Arc<dyn Club>);

    assert_eq!(app.loop_watchdog(&thinking), None, "the soft check first");
    assert_eq!(
        app.loop_watchdog(&thinking),
        None,
        "then the review goes out"
    );
    assert!(app.loop_watch.review.is_some());
    assert_eq!(harvest(&mut app, &thinking), None);

    let page = seat
        .page
        .lock()
        .unwrap()
        .clone()
        .expect("the seat read its route");
    assert!(
        page.contains("only job is to keep the harness moving"),
        "{page}"
    );
    let facts = seat.facts.lock().unwrap().clone().unwrap();
    assert!(
        facts.starts_with(&st_connected::WATCHDOG.cells()),
        "{facts}"
    );
    assert!(facts.contains("/tmp/r31_wordram_sat"), "{facts}");
    assert!(
        facts.contains("earlier watchdog actions this stretch: none"),
        "{facts}"
    );

    let review = app.loop_ctl.watchdog.last().unwrap();
    assert_eq!(review.kind, "review");
    assert_eq!(review.verdict.as_deref(), Some("stop_call"));
    assert_eq!(review.by, "watchdog-seat");
    assert_eq!(
        review.detail,
        "a CPU-bound solver with no output for an hour"
    );
    assert!(crate::agent::harness::release_pending(owner(&thinking)));
    assert!(app.loop_watch.released_at.is_some());

    // The release never landed (nothing consumed it): past the grace the
    // turn is restarted and the request withdrawn.
    assert_eq!(app.loop_watchdog(&thinking), None, "inside the grace");
    app.loop_watch.released_at = Some(aged(RELEASE_GRACE_SECS));
    let restart = app
        .loop_watchdog(&thinking)
        .expect("escalates to a restart");
    assert!(restart.contains("turn restarted"), "{restart}");
    let escalate = app.loop_ctl.watchdog.last().unwrap();
    assert_eq!(escalate.kind, "escalate");
    assert_eq!(escalate.verdict.as_deref(), Some("restart_turn"));
    assert!(!crate::agent::harness::release_pending(owner(&thinking)));
}

#[test]
fn a_wait_verdict_reviews_again_half_an_hour_later() {
    let _lock = crate::tests::env_lock();
    let (mut app, mut thinking, _fixture) = wedged_loop("wait", 61 * 60);
    wedge_on_a_solver(&mut app);
    thinking.club = Some(Seat::new("fresh output a minute ago\nverdict: wait") as Arc<dyn Club>);
    app.loop_watchdog(&thinking);
    app.loop_watchdog(&thinking);
    assert_eq!(harvest(&mut app, &thinking), None);
    assert_eq!(
        app.loop_ctl.watchdog.last().unwrap().verdict.as_deref(),
        Some("wait")
    );
    assert!(!crate::agent::harness::release_pending(owner(&thinking)));
    assert!(app.loop_watch.next_review_secs >= 61 * 60 + LIMITS.check_secs);
    assert_eq!(app.loop_watchdog(&thinking), None);
    assert!(app.loop_watch.review.is_none(), "not again until then");
}

#[test]
fn a_reviewer_without_a_verdict_gives_way_to_the_fixed_rule() {
    let _lock = crate::tests::env_lock();
    let (mut app, mut thinking, _fixture) = wedged_loop("garbled", 61 * 60);
    wedge_on_a_solver(&mut app);
    thinking.club = Some(Seat::new("let me just finish the SAT solve for you") as Arc<dyn Club>);
    app.loop_watchdog(&thinking);
    app.loop_watchdog(&thinking);
    assert_eq!(harvest(&mut app, &thinking), None);
    let review = app.loop_ctl.watchdog.last().unwrap();
    assert_eq!(review.verdict.as_deref(), Some("handoff"));
    assert_eq!(review.by, "fixed rule");
    assert!(
        review.detail.contains("without a verdict"),
        "{}",
        review.detail
    );
    assert!(crate::agent::harness::release_pending(owner(&thinking)));
    crate::agent::harness::clear_release(owner(&thinking));
}

#[test]
fn with_no_call_running_and_no_reviewer_the_turn_restarts() {
    let _lock = crate::tests::env_lock();
    let (mut app, thinking, _fixture) = wedged_loop("no-seat", 61 * 60);
    assert!(thinking.club.is_none());
    app.loop_watchdog(&thinking);
    let restart = app
        .loop_watchdog(&thinking)
        .expect("a provider wedge with nobody to ask restarts the turn");
    assert!(restart.contains("no reviewer seat"), "{restart}");
    let review = app.loop_ctl.watchdog.last().unwrap();
    assert_eq!(review.verdict.as_deref(), Some("restart_turn"));
    assert_eq!(review.by, "fixed rule");
    let check = &app.loop_ctl.watchdog[0];
    assert!(
        check.detail.contains("in flight: no tool call"),
        "{}",
        check.detail
    );
}

#[test]
fn the_harness_moving_again_is_recorded_and_resets_the_watch() {
    let _lock = crate::tests::env_lock();
    let (mut app, mut thinking, _fixture) = wedged_loop("resume", 31 * 60);
    wedge_on_a_solver(&mut app);
    app.loop_watchdog(&thinking);
    assert!(app.loop_watch.checked);

    // A tool result lands: the turn's event clock moves.
    thinking.note_stream_progress();
    assert_eq!(app.loop_watchdog(&thinking), None);
    let resumed = app.loop_ctl.watchdog.last().unwrap();
    assert_eq!(resumed.kind, "resumed");
    assert!(resumed.quiet_secs >= 31 * 60);
    assert!(!app.loop_watch.checked);
    assert!(
        app.messages
            .iter()
            .any(|m| m.text.contains("harness moving again after 31m"))
    );

    // A quiet stretch nobody flagged leaves no record when it ends.
    let records = app.loop_ctl.watchdog.len();
    thinking.note_stream_progress();
    app.loop_watchdog(&thinking);
    assert_eq!(app.loop_ctl.watchdog.len(), records);
}

#[test]
fn the_watchdog_only_watches_a_running_iteration() {
    let _lock = crate::tests::env_lock();
    let (mut app, thinking, _fixture) = wedged_loop("paused", 3 * 3600);
    app.loop_ctl.status = LoopStatus::Paused;
    assert_eq!(app.loop_watchdog(&thinking), None);
    app.loop_ctl.status = LoopStatus::Running;
    app.loop_ctl.awaiting_turn = false;
    assert_eq!(app.loop_watchdog(&thinking), None);
    assert!(app.loop_ctl.watchdog.is_empty());
}

#[test]
fn records_are_capped() {
    let _lock = crate::tests::env_lock();
    let (mut app, _thinking, _fixture) = wedged_loop("cap", 60);
    for n in 0..(MAX_RECORDS as u64 + 5) {
        app.loop_watchdog_record("check", n, None, "", "");
    }
    assert_eq!(app.loop_ctl.watchdog.len(), MAX_RECORDS);
    assert_eq!(
        app.loop_ctl.watchdog[0].quiet_secs, 5,
        "oldest dropped first"
    );
}

#[test]
fn spans_read_like_the_transcript() {
    assert_eq!(span(42), "42s");
    assert_eq!(span(31 * 60 + 5), "31m05s");
    assert_eq!(span(20 * 3600 + 30 * 60), "20h30m");
    assert_eq!(ago(None), "never");
    assert_eq!(ago(Some(90)), "1m30s ago");
}

#[test]
fn after_two_waits_the_fixed_rule_decides_without_asking_again() {
    let _lock = crate::tests::env_lock();
    let (mut app, mut thinking, _fixture) = wedged_loop("waits", 121 * 60);
    wedge_on_a_solver(&mut app);
    let seat = Seat::new("still thinking about it\nverdict: wait");
    thinking.club = Some(seat.clone() as Arc<dyn Club>);
    app.loop_watchdog(&thinking);
    assert!(app.loop_watch.checked);
    app.loop_watch.actions = vec!["wait (watchdog-seat)".into(), "wait (watchdog-seat)".into()];
    app.loop_watch.next_review_secs = 120 * 60;

    assert_eq!(app.loop_watchdog(&thinking), None);
    assert!(app.loop_watch.review.is_none(), "no third review");
    assert!(
        seat.facts.lock().unwrap().is_none(),
        "the seat was not asked"
    );
    let review = app.loop_ctl.watchdog.last().unwrap();
    assert_eq!(review.verdict.as_deref(), Some("handoff"));
    assert_eq!(review.by, "fixed rule");
    assert_eq!(review.detail, "the reviewer said wait 2 times");
    assert!(crate::agent::harness::release_pending(owner(&thinking)));
    crate::agent::harness::clear_release(owner(&thinking));
}
