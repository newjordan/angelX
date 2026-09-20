//! Teacher-watch — graceful recovery for long local-model sessions.
//!
//! Local seats (turbo, spark, llama.cpp, vLLM) die after long calibration and
//! proving runs: KV/context overflow, empty 200s, transport reset, 503 while
//! reloading, or "model not available". The student turn must not crash the
//! campaign. This module classifies those faults, rolls context deterministically,
//! and optionally asks a cheap teacher (luna if reachable) for a one-line note.
//!
//! Offline-first: no teacher is required. A dark luna/local teacher is skipped.
//! `ANGEL_TEACHER_WATCH=0` disables the whole latch.

use super::{
    ChatMsg, Club, ToolDef, ToolRegistry, TurnEvent, env_flag, env_usize, estimate_tokens,
    estimate_tool_tokens, fit_tool_results_to_budget, is_context_overflow_error,
    is_empty_reply_error, maybe_compact_for_turn,
};
use crate::tools::consult::{find_in_roster, is_optional_local_label};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// Default teacher seat. Cheap access agent — never required to be up.
pub(crate) const DEFAULT_TEACHER_CLUB: &str = "luna";

const ASK_TIMEOUT: Duration = Duration::from_secs(6);
const ERROR_SNIP: usize = 240;
const TEACHER_PROMPT_CAP: usize = 1_200;
static TEACHER_ASK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

struct TeacherAskLease;

impl Drop for TeacherAskLease {
    fn drop(&mut self) {
        TEACHER_ASK_IN_FLIGHT.store(false, Ordering::Release);
    }
}

pub(crate) fn teacher_watch_enabled() -> bool {
    env_flag("ANGEL_TEACHER_WATCH", true)
}

pub(crate) fn teacher_watch_ask_enabled() -> bool {
    env_flag("ANGEL_TEACHER_WATCH_ASK", true)
}

/// Apply to local/student seats. Paid SOTA drivers keep the existing overflow
/// path only — they are not the long-session crash class.
pub(crate) fn teacher_watch_applies(club: &dyn Club) -> bool {
    teacher_watch_enabled() && !crate::club::is_sota_label(club.label())
}

/// Why a long local session needs the monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SessionFault {
    ContextOverflow,
    EmptyReply,
    TransportDead,
    ModelUnavailable,
    Timeout,
}

impl SessionFault {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ContextOverflow => "context-overflow",
            Self::EmptyReply => "empty-reply",
            Self::TransportDead => "transport-dead",
            Self::ModelUnavailable => "model-unavailable",
            Self::Timeout => "timeout",
        }
    }

    /// Retry the same student seat after a context roll. Transport death and
    /// a missing model cannot be fixed by sending the same request again.
    pub(crate) fn retries_same_club(self) -> bool {
        matches!(
            self,
            Self::ContextOverflow | Self::EmptyReply | Self::Timeout
        )
    }
}

/// Classify a provider/tool error as a long-session fault, or `None` if it is
/// an ordinary permanent/auth failure the monitor must not launder.
pub(crate) fn classify_session_fault(error: &str) -> Option<SessionFault> {
    if is_context_overflow_error(error) {
        return Some(SessionFault::ContextOverflow);
    }
    if is_empty_reply_error(error) {
        return Some(SessionFault::EmptyReply);
    }
    let lower = error.to_ascii_lowercase();
    if lower.starts_with("turn idle timeout") {
        return None;
    }
    if lower.contains("model not available")
        || lower.contains("not reachable")
        || lower.contains("not available right now")
        || (lower.contains("model") && lower.contains("not available"))
    {
        return Some(SessionFault::ModelUnavailable);
    }
    if lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("deadline exceeded")
    {
        return Some(SessionFault::Timeout);
    }
    if lower.contains("transport error")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("loading model")
        || (lower.contains("http 503") || lower.contains("http 502"))
    {
        return Some(SessionFault::TransportDead);
    }
    None
}

/// Cheap teacher: named pin (default luna) if reachable, else another live
/// cheap seat that is not the dying student. Never invents a down local.
pub(crate) fn pick_teacher(
    roster: &[Arc<dyn Club>],
    aux: &[Arc<dyn Club>],
    student_label: &str,
) -> Option<Arc<dyn Club>> {
    let wanted = std::env::var("ANGEL_TEACHER_WATCH_CLUB")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_TEACHER_CLUB.to_string());
    if let Some(club) = find_in_roster(roster, &wanted)
        && club.is_available()
        && !club.label().eq_ignore_ascii_case(student_label)
    {
        return Some(club);
    }
    let prefer = ["luna", "spark", "gemma", "atlas"];
    for name in prefer {
        if name.eq_ignore_ascii_case(student_label) {
            continue;
        }
        if let Some(club) = find_in_roster(roster, name)
            && club.is_available()
        {
            return Some(club);
        }
    }
    roster
        .iter()
        .chain(aux.iter())
        .find(|club| {
            let label = club.label();
            !label.eq_ignore_ascii_case(student_label)
                && club.is_available()
                && (is_optional_local_label(label) || label.to_ascii_lowercase().contains("luna"))
        })
        .cloned()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TeacherLine {
    Roll(String),
    Retry(String),
    Catch(String),
    Continue(String),
}

pub(crate) fn parse_teacher_line(reply: &str) -> Option<TeacherLine> {
    for line in reply.lines() {
        let t = line.trim();
        let upper = t.to_ascii_uppercase();
        let take = |prefix: &str| {
            t.get(prefix.len()..)
                .map(str::trim)
                .unwrap_or("")
                .to_string()
        };
        if upper.starts_with("CATCH:") {
            let msg = take("CATCH:");
            if !msg.is_empty() {
                return Some(TeacherLine::Catch(msg));
            }
        }
        if upper.starts_with("ROLL:") {
            let msg = take("ROLL:");
            if !msg.is_empty() {
                return Some(TeacherLine::Roll(msg));
            }
        }
        if upper.starts_with("RETRY:") {
            let msg = take("RETRY:");
            if !msg.is_empty() {
                return Some(TeacherLine::Retry(msg));
            }
        }
        if upper.starts_with("CONTINUE:") {
            let msg = take("CONTINUE:");
            if !msg.is_empty() {
                return Some(TeacherLine::Continue(msg));
            }
        }
    }
    None
}

pub(crate) fn deterministic_recovery_note(fault: SessionFault) -> String {
    match fault {
        SessionFault::ContextOverflow => {
            "teacher-watch: context overflow — rolled the tail into a ledger. \
             Continue from the current workspace and the compact note. Do not \
             re-read the whole transcript."
                .to_string()
        }
        SessionFault::EmptyReply => {
            "teacher-watch: local seat returned empty — context rolled. Answer \
             or tool-call now; do not replay the same empty hop."
                .to_string()
        }
        SessionFault::TransportDead => {
            "teacher-watch: local seat went dark (transport). Context rolled. \
             Do not retry the same dead endpoint this hop; continue from the \
             ledger on the next iteration."
                .to_string()
        }
        SessionFault::ModelUnavailable => {
            "teacher-watch: named local is not reachable. Skipped. Continue \
             yourself from the ledger; do not retry the dark seat."
                .to_string()
        }
        SessionFault::Timeout => "teacher-watch: local seat timed out after a long generation. \
             Context rolled. Continue with a smaller next action."
            .to_string(),
    }
}

pub(crate) fn teacher_ask_prompt(fault: SessionFault, error: &str, hop: usize) -> String {
    let snip: String = error.chars().take(ERROR_SNIP).collect();
    let mut prompt = format!(
        "You are a cheap teacher-monitor for a long local-model coding session. \
         One fault just happened during a calibration/proving run. \
         Reply with exactly one line:\n\
         ROLL: <why> | RETRY: <why> | CATCH: <bug> | CONTINUE: <next action>\n\
         Fault: {}\nHop: {hop}\nError: {snip}\n",
        fault.as_str()
    );
    if prompt.len() > TEACHER_PROMPT_CAP {
        prompt.truncate(TEACHER_PROMPT_CAP);
    }
    prompt
}

/// Bounded, fail-open teacher ask. A dark or slow teacher is skipped.
pub(crate) fn ask_teacher(club: Arc<dyn Club>, prompt: String) -> Option<String> {
    ask_teacher_with_timeout(club, prompt, ASK_TIMEOUT)
}

fn ask_teacher_with_timeout(
    club: Arc<dyn Club>,
    prompt: String,
    timeout: Duration,
) -> Option<String> {
    // Most Club implementations inherit an uninterruptible cancellation
    // default. If one ignores the timeout signal, keep exactly that detached
    // worker instead of spawning another stuck teacher on every later fault.
    if TEACHER_ASK_IN_FLIGHT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return None;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_w = Arc::clone(&cancel);
    let (tx, rx) = mpsc::channel();
    if std::thread::Builder::new()
        .name("angel-teacher-watch".to_string())
        .spawn(move || {
            let _lease = TeacherAskLease;
            let _ = tx.send(club.respond_cancellable(&prompt, &cancel_w));
        })
        .is_err()
    {
        TEACHER_ASK_IN_FLIGHT.store(false, Ordering::Release);
        return None;
    }
    match rx.recv_timeout(timeout) {
        Ok(Ok(text)) if !text.trim().is_empty() => Some(text),
        _ => {
            cancel.store(true, Ordering::Relaxed);
            None
        }
    }
}

/// Roll the live tail with the ordinary model-free compactor. Never waits on
/// a teacher inference endpoint.
#[allow(clippy::too_many_arguments)]
pub(crate) fn roll_session_history(
    club: &dyn Club,
    history: &mut Vec<ChatMsg>,
    budget: usize,
    keep_recent: usize,
    keep_recent_tokens: usize,
    tools: &[ToolDef],
    registry: &ToolRegistry,
    events: &mpsc::Sender<TurnEvent>,
) -> bool {
    if budget == 0 {
        return false;
    }
    let before = estimate_tokens(history) + estimate_tool_tokens(tools);
    let compacted = maybe_compact_for_turn(
        club,
        history,
        budget,
        keep_recent,
        keep_recent_tokens,
        tools,
        registry,
        events,
    );
    let fitted = fit_tool_results_to_budget(history, budget, tools);
    let after = estimate_tokens(history) + estimate_tool_tokens(tools);
    compacted || fitted > 0 || after < before
}

/// Compose the operator/model-facing recovery note. Teacher text is an
/// annotation; the deterministic floor is always present.
pub(crate) fn compose_recovery_note(fault: SessionFault, teacher: Option<&TeacherLine>) -> String {
    let base = deterministic_recovery_note(fault);
    match teacher {
        Some(TeacherLine::Catch(bug)) => format!("{base} Caught: {bug}"),
        Some(TeacherLine::Roll(why) | TeacherLine::Retry(why) | TeacherLine::Continue(why)) => {
            format!("{base} Teacher: {why}")
        }
        None => base,
    }
}

/// One recovery attempt: roll history, optionally ask the teacher, return the
/// note the hop should inject. `None` when the latch is off or the error is
/// not a session fault.
#[allow(clippy::too_many_arguments)]
pub(crate) fn recover_session(
    club: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    error: &str,
    hop: usize,
    budget: usize,
    keep_recent: usize,
    keep_recent_tokens: usize,
    tools: &[ToolDef],
    events: &mpsc::Sender<TurnEvent>,
) -> Option<(SessionFault, String)> {
    if !teacher_watch_applies(club) {
        return None;
    }
    let fault = classify_session_fault(error)?;
    let _ = roll_session_history(
        club,
        history,
        budget,
        keep_recent,
        keep_recent_tokens,
        tools,
        registry,
        events,
    );
    let teacher_line = if teacher_watch_ask_enabled() {
        pick_teacher(registry.roster(), &registry.aux_clubs, club.label()).and_then(|teacher| {
            let _ = events.send(TurnEvent::Notice(format!(
                "teacher-watch: asking {} ({})",
                teacher.label(),
                fault.as_str()
            )));
            registry.auxiliary.utility_entered("teacher_watch");
            ask_teacher(teacher, teacher_ask_prompt(fault, error, hop))
                .and_then(|text| parse_teacher_line(&text))
        })
    } else {
        None
    };
    Some((fault, compose_recovery_note(fault, teacher_line.as_ref())))
}

pub(crate) fn max_session_recoveries() -> usize {
    env_usize("ANGEL_TEACHER_WATCH_RECOVERIES", 2).min(4)
}

#[cfg(test)]
mod tests {
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
            ask_teacher_with_timeout(fast, "third".to_string(), Duration::from_millis(100))
                .as_deref(),
            Some("CONTINUE: next")
        );
    }
}
