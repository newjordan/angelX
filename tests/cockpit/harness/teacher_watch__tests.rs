use super::*;
use crate::agent::club::{ChatMsg, ClubReply};
use std::time::Duration;

struct MockClub {
    label: String,
    available: bool,
    response: String,
}

struct BlockingClub {
    calls: Arc<std::sync::atomic::AtomicUsize>,
    release: Arc<AtomicBool>,
}

impl Club for BlockingClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        while !self.release.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok("CONTINUE: released".to_string())
    }

    fn label(&self) -> &str {
        "blocked-teacher"
    }
}

impl MockClub {
    fn up(label: &str, response: &str) -> Self {
        Self {
            label: label.to_string(),
            available: true,
            response: response.to_string(),
        }
    }
    fn down(label: &str) -> Self {
        Self {
            label: label.to_string(),
            available: false,
            response: String::new(),
        }
    }
}

impl Club for MockClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        if !self.available {
            return Err(format!("{} model not available", self.label));
        }
        Ok(self.response.clone())
    }
    fn label(&self) -> &str {
        &self.label
    }
    fn is_available(&self) -> bool {
        self.available
    }
    fn chat_streaming(
        &self,
        _messages: &[ChatMsg],
        _tools: &[ToolDef],
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
    ) -> Result<ClubReply, String> {
        self.respond("").map(ClubReply::Text)
    }
}

#[test]
fn classifies_long_session_faults() {
    assert_eq!(
        classify_session_fault("HTTP 413: context_length_exceeded"),
        Some(SessionFault::ContextOverflow)
    );
    assert_eq!(
        classify_session_fault("empty reply (no text and no tool calls)"),
        Some(SessionFault::EmptyReply)
    );
    assert_eq!(
        classify_session_fault("transport error after 3 attempt(s): Connection refused"),
        Some(SessionFault::TransportDead)
    );
    assert_eq!(
        classify_session_fault("HTTP 503: Loading model"),
        Some(SessionFault::TransportDead)
    );
    assert_eq!(
        classify_session_fault("turbo model not available"),
        Some(SessionFault::ModelUnavailable)
    );
    assert_eq!(
        classify_session_fault("consult_model (turbo): club 'turbo' is not reachable"),
        Some(SessionFault::ModelUnavailable)
    );
    assert_eq!(
        classify_session_fault("HTTP 408: request timed out"),
        Some(SessionFault::Timeout)
    );
    assert_eq!(
        classify_session_fault("turn idle timeout (600s with no stream progress)"),
        None,
        "the UI watchdog is a loop stall, not a provider session fault"
    );
    assert_eq!(
        classify_session_fault("HTTP 401: invalid api key"),
        None,
        "auth must not be laundered as a session fault"
    );
}

#[test]
fn pick_teacher_skips_down_luna_and_the_dying_student() {
    let luna: Arc<dyn Club> = Arc::new(MockClub::down("luna"));
    let turbo: Arc<dyn Club> = Arc::new(MockClub::up("turbo", "no"));
    let spark: Arc<dyn Club> = Arc::new(MockClub::up("spark", "ok"));
    let roster = vec![luna, turbo, spark];
    let teacher = pick_teacher(&roster, &[], "turbo").unwrap();
    assert_eq!(teacher.label(), "spark");
}

#[test]
fn pick_teacher_returns_none_when_every_seat_is_dark() {
    let luna: Arc<dyn Club> = Arc::new(MockClub::down("luna"));
    let turbo: Arc<dyn Club> = Arc::new(MockClub::down("turbo"));
    assert!(pick_teacher(&[luna, turbo], &[], "turbo").is_none());
}

#[test]
fn parse_teacher_lines() {
    assert_eq!(
        parse_teacher_line("CATCH: kv cache blew up after hop 40"),
        Some(TeacherLine::Catch("kv cache blew up after hop 40".into()))
    );
    assert_eq!(
        parse_teacher_line("ROLL: drop the tool tail\nextra"),
        Some(TeacherLine::Roll("drop the tool tail".into()))
    );
    assert!(parse_teacher_line("CLEAR").is_none());
}

#[test]
fn compose_note_keeps_offline_floor_and_appends_catch() {
    let note = compose_recovery_note(
        SessionFault::TransportDead,
        Some(&TeacherLine::Catch("llama-server reset after 32k".into())),
    );
    assert!(note.contains("went dark"), "{note}");
    assert!(note.contains("llama-server reset after 32k"), "{note}");
    assert!(note.contains("Do not retry"), "{note}");
}

#[test]
fn transport_and_unavailable_do_not_retry_the_same_club() {
    assert!(!SessionFault::TransportDead.retries_same_club());
    assert!(!SessionFault::ModelUnavailable.retries_same_club());
    assert!(SessionFault::ContextOverflow.retries_same_club());
    assert!(SessionFault::EmptyReply.retries_same_club());
}

#[test]
fn a_slow_teacher_is_waited_for_never_timed_out() {
    TEACHER_ASK_IN_FLIGHT.store(false, Ordering::Release);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let club: Arc<dyn Club> = Arc::new(BlockingClub {
        calls: Arc::clone(&calls),
        release: Arc::clone(&release),
    });
    // The teacher answers only after a pause; the ask waits for it instead
    // of giving up on a clock.
    let releaser = {
        let release = Arc::clone(&release);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            release.store(true, Ordering::Release);
        })
    };
    let answer = ask_teacher(club, std::path::Path::new("."), "first".to_string());
    releaser.join().unwrap();
    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert!(answer.is_some(), "a slow teacher's answer is kept");
    assert!(!TEACHER_ASK_IN_FLIGHT.load(Ordering::Acquire));

    let fast: Arc<dyn Club> = Arc::new(MockClub::up("luna", "CONTINUE: next"));
    assert_eq!(
        ask_teacher(fast, std::path::Path::new("."), "second".to_string()).as_deref(),
        Some("CONTINUE: next")
    );
}

/// The notes a later prompt carries are page addresses on `⠟⠓` / `⠟⠊`; the
/// operator and the ledger read them recited, the teacher's annotation after.
#[test]
fn recovery_notes_are_routes_recited_for_the_operator() {
    use crate::agent::harness::book::{connect::recite, ledger};
    for (fault, original) in [
        (
            SessionFault::ContextOverflow,
            "teacher-watch: context overflow — rolled the tail into a ledger. Continue from the current workspace and the compact note. Do not re-read the whole transcript.",
        ),
        (
            SessionFault::EmptyReply,
            "teacher-watch: local seat returned empty — context rolled. Answer or tool-call now; do not replay the same empty hop.",
        ),
        (
            SessionFault::TransportDead,
            "teacher-watch: local seat went dark (transport). Context rolled. Do not retry the same dead endpoint this hop; continue from the ledger on the next iteration.",
        ),
        (
            SessionFault::ModelUnavailable,
            "teacher-watch: named local is not reachable. Skipped. Continue yourself from the ledger; do not retry the dark seat.",
        ),
        (
            SessionFault::Timeout,
            "teacher-watch: local seat timed out after a long generation. Context rolled. Continue with a smaller next action.",
        ),
    ] {
        let note = deterministic_recovery_note(fault);
        assert!(ledger::is_warpath_line(&note), "{note}");
        assert_eq!(recite(&note), original);
        assert_eq!(compose_recovery_note(fault, None), original);
    }
}

/// The teacher's brief is `⠟⠚`; the reply shape and the fault are inline.
#[test]
fn the_teacher_is_asked_in_routes() {
    let prompt = teacher_ask_prompt(SessionFault::Timeout, "HTTP 408: request timed out", 7);
    let teacher = crate::agent::harness::book::q_stop::TEACHER;
    assert!(
        prompt.starts_with(&format!("{}\n", teacher.cells())),
        "{prompt}"
    );
    assert!(prompt.contains("ROLL: <why> | RETRY: <why> | CATCH: <bug> | CONTINUE: <next action>"));
    assert!(
        prompt.contains("Fault: timeout\nHop: 7\nError: HTTP 408"),
        "{prompt}"
    );
    assert!(!prompt.contains("teacher-monitor"), "{prompt}");
    assert!(teacher.sub().pages[0].contains("teacher-monitor"));
}
