use super::*;
use crate::club::{ChatMsg, ClubReply};

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
        _on_delta: &mut dyn FnMut(crate::club::StreamDelta),
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
fn timed_out_uninterruptible_teacher_stays_single_flight_until_return() {
    struct ReleaseOnDrop(Arc<AtomicBool>);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    TEACHER_ASK_IN_FLIGHT.store(false, Ordering::Release);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let _release_on_drop = ReleaseOnDrop(Arc::clone(&release));
    let club: Arc<dyn Club> = Arc::new(BlockingClub {
        calls: Arc::clone(&calls),
        release: Arc::clone(&release),
    });

    assert!(
        ask_teacher_with_timeout(
            Arc::clone(&club),
            "first".to_string(),
            Duration::from_millis(20)
        )
        .is_none()
    );
    let started_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while calls.load(Ordering::Acquire) == 0 && std::time::Instant::now() < started_deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(calls.load(Ordering::Acquire), 1);

    assert!(
        ask_teacher_with_timeout(
            club,
            "must not spawn".to_string(),
            Duration::from_millis(20)
        )
        .is_none()
    );
    assert_eq!(calls.load(Ordering::Acquire), 1);

    release.store(true, Ordering::Release);
    let released_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while TEACHER_ASK_IN_FLIGHT.load(Ordering::Acquire)
        && std::time::Instant::now() < released_deadline
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!TEACHER_ASK_IN_FLIGHT.load(Ordering::Acquire));

    let fast: Arc<dyn Club> = Arc::new(MockClub::up("luna", "CONTINUE: next"));
    assert_eq!(
        ask_teacher_with_timeout(fast, "third".to_string(), Duration::from_millis(100)).as_deref(),
        Some("CONTINUE: next")
    );
}
