//! Loop watchdog: keeps the harness moving, never the loop's task.
//!
//! The clock is the in-flight turn's last event (`Thinking::last_stream_at`):
//! a streamed token, a keep-alive, a tool call starting or finishing. The
//! loop's `updated_ms` is no use here, the heartbeat bumps it while the UI
//! runs, so a wedged iteration looks live (a hash loop sat 20h30m on one SAT
//! solve that way).
//!
//! - Quiet for `ANGEL_LOOP_WATCHDOG_CHECK_SECS` (default 30 min): the soft
//!   check. No model; the facts of what the iteration is blocked on go to the
//!   transcript and the loop file.
//! - Quiet for `ANGEL_LOOP_WATCHDOG_REVIEW_SECS` (default 60 min, `0` turns
//!   the watchdog off): a reviewer seat reads those facts (`⡌⠛`) and names one
//!   harness remedy. It is asked whether the harness is wedged, not how the
//!   task is going, and it acts on nothing itself: the watchdog carries out
//!   `wait`, `handoff`, `stop_call` or `restart_turn`. A reviewer that does
//!   not answer in time, or answers without a verdict, gives way to a fixed
//!   rule.
//! - After [`MAX_WAITS`] `wait` verdicts in one quiet stretch, the fixed rule
//!   decides instead of another review.
//! - A release (`handoff`, `stop_call`) that brings no event within
//!   [`RELEASE_GRACE_SECS`] means the turn is not blocked on a call the
//!   release can reach, and the turn is restarted.
//!
//! Every step is recorded on the loop (`LoopState::watchdog`), including the
//! harness moving again after one.

use super::*;
use crate::agent::harness::book::st_connected;
use crate::agent::harness::{Release, clear_release, request_release};
use std::sync::atomic::{AtomicBool, Ordering};

/// Quiet time before the soft check.
const CHECK_SECS: u64 = 30 * 60;
/// Quiet time before a reviewer is dispatched.
const REVIEW_SECS: u64 = 60 * 60;
/// A reviewer that has not answered by then is replaced by the fixed rule.
const REVIEW_DEADLINE_SECS: u64 = 5 * 60;
/// After a release, how long to wait for any harness event before the turn
/// is restarted.
pub(crate) const RELEASE_GRACE_SECS: u64 = 2 * 60;
/// `wait` verdicts a quiet stretch takes before the fixed rule decides: a
/// reviewer that keeps waiting must not rebuild the 20-hour wedge.
const MAX_WAITS: usize = 2;
/// Watchdog records kept on the loop.
const MAX_RECORDS: usize = 20;
/// The in-flight call's args, as the facts show them.
const FACT_ARGS_CHARS: usize = 400;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WatchdogLimits {
    /// `0` = no soft check.
    pub(crate) check_secs: u64,
    pub(crate) review_secs: u64,
}

/// The configured limits; `None` when the watchdog is off.
pub(crate) fn watchdog_limits() -> Option<WatchdogLimits> {
    let review_secs = env_u64("ANGEL_LOOP_WATCHDOG_REVIEW_SECS", REVIEW_SECS);
    (review_secs > 0).then(|| WatchdogLimits {
        check_secs: env_u64("ANGEL_LOOP_WATCHDOG_CHECK_SECS", CHECK_SECS),
        review_secs,
    })
}

/// One watchdog step, as the loop file keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchdogRecord {
    pub ts_ms: u64,
    /// Finished iterations when the step was taken.
    pub iteration: usize,
    /// `check`, `review`, `escalate` or `resumed`.
    pub kind: String,
    /// How long the harness had been quiet.
    pub quiet_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// Who chose the verdict: the reviewer's label, or `fixed rule`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub by: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

/// A harness remedy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    Wait,
    HandOff,
    StopCall,
    RestartTurn,
}

impl Verdict {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Verdict::Wait => "wait",
            Verdict::HandOff => "handoff",
            Verdict::StopCall => "stop_call",
            Verdict::RestartTurn => "restart_turn",
        }
    }
}

/// The verdict on the reviewer's last `verdict:` line, if it has one.
pub(crate) fn parse_verdict(text: &str) -> Option<Verdict> {
    let decorations: &[char] = &['`', '*', '_', '.', ' '];
    text.lines().rev().find_map(|line| {
        let line = line.trim().trim_matches(decorations).to_ascii_lowercase();
        let word = line
            .strip_prefix("verdict:")?
            .trim()
            .trim_matches(decorations);
        match word {
            "wait" => Some(Verdict::Wait),
            "handoff" | "hand_off" | "hand-off" => Some(Verdict::HandOff),
            "stop_call" | "stop" => Some(Verdict::StopCall),
            "restart_turn" | "restart" => Some(Verdict::RestartTurn),
            _ => None,
        }
    })
}

/// The fixed rule, when no reviewer verdict is to be had: hand a running call
/// to the background; with no call running, or after a release that did not
/// free the turn, restart it.
pub(crate) fn fallback_verdict(call_running: bool, released_before: bool) -> Verdict {
    if call_running && !released_before {
        Verdict::HandOff
    } else {
        Verdict::RestartTurn
    }
}

/// What the watchdog does this frame for a stretch quiet `quiet_secs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WatchdogDue {
    Nothing,
    Check,
    Review,
    Escalate,
}

pub(crate) fn watchdog_due(
    limits: WatchdogLimits,
    quiet_secs: u64,
    checked: bool,
    next_review_secs: u64,
    released_secs: Option<u64>,
) -> WatchdogDue {
    if let Some(released) = released_secs {
        return if released >= RELEASE_GRACE_SECS {
            WatchdogDue::Escalate
        } else {
            WatchdogDue::Nothing
        };
    }
    if limits.check_secs > 0 && !checked && quiet_secs >= limits.check_secs {
        return WatchdogDue::Check;
    }
    if quiet_secs >= next_review_secs {
        return WatchdogDue::Review;
    }
    WatchdogDue::Nothing
}

/// The watch over the in-flight iteration (runtime only).
#[derive(Default)]
pub(crate) struct LoopWatch {
    /// The quiet stretch being watched: the turn's last event.
    stretch: Option<Instant>,
    /// The turn's root owner, for its release requests.
    owner: usize,
    limits: WatchdogLimits,
    checked: bool,
    /// Quiet seconds at which the next review is due.
    next_review_secs: u64,
    released_at: Option<Instant>,
    /// Verdicts taken in this stretch, oldest first.
    actions: Vec<String>,
    review: Option<ReviewPending>,
}

pub(crate) struct ReviewPending {
    by: String,
    started: Instant,
    cancel: Arc<AtomicBool>,
    rx: Receiver<Result<String, String>>,
}

impl Drop for ReviewPending {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl crate::App {
    /// Watch the in-flight loop iteration. Returns the reason when the turn is
    /// to be restarted; the caller retires it and the loop retries.
    pub(crate) fn loop_watchdog(&mut self, thinking: &Thinking) -> Option<String> {
        if self.loop_ctl.status != LoopStatus::Running || !self.loop_ctl.awaiting_turn {
            return None;
        }
        let stretch = thinking.last_stream_at;
        if self.loop_watch.stretch != Some(stretch) {
            let limits = watchdog_limits()?;
            self.loop_watchdog_new_stretch(thinking, stretch, limits);
        }
        let quiet = stretch.elapsed().as_secs();
        if self.loop_watch.review.is_some() {
            return self.loop_watchdog_harvest_review(quiet);
        }
        let watch = &self.loop_watch;
        match watchdog_due(
            watch.limits,
            quiet,
            watch.checked,
            watch.next_review_secs,
            watch.released_at.map(|at| at.elapsed().as_secs()),
        ) {
            WatchdogDue::Nothing => None,
            WatchdogDue::Check => {
                self.loop_watch.checked = true;
                let facts = self.loop_watchdog_facts(thinking, quiet);
                self.messages.push(Message::new(
                    Role::Activity,
                    format!(
                        "⏱ loop watchdog: no harness progress for {}; soft check\n{facts}",
                        span(quiet)
                    ),
                ));
                self.loop_watchdog_record("check", quiet, None, "", &facts);
                None
            }
            WatchdogDue::Review => self.loop_watchdog_dispatch(thinking, quiet),
            WatchdogDue::Escalate => {
                clear_release(self.loop_watch.owner);
                let detail = format!(
                    "a release brought no harness event in {}",
                    span(RELEASE_GRACE_SECS)
                );
                self.loop_watchdog_record(
                    "escalate",
                    quiet,
                    Some(Verdict::RestartTurn),
                    "fixed rule",
                    &detail,
                );
                Some(format!(
                    "loop watchdog: no harness progress for {}; {detail}; turn restarted",
                    span(quiet)
                ))
            }
        }
    }

    fn loop_watchdog_new_stretch(
        &mut self,
        thinking: &Thinking,
        stretch: Instant,
        limits: WatchdogLimits,
    ) {
        let previous = std::mem::take(&mut self.loop_watch);
        clear_release(previous.owner);
        if let Some(before) = previous.stretch
            && (previous.checked || !previous.actions.is_empty())
        {
            let quiet = stretch.saturating_duration_since(before).as_secs();
            let detail = if previous.actions.is_empty() {
                String::new()
            } else {
                format!("after {}", previous.actions.join(", "))
            };
            self.messages.push(Message::new(
                Role::Activity,
                format!(
                    "⏱ loop watchdog: harness moving again after {} quiet {detail}",
                    span(quiet)
                ),
            ));
            self.loop_watchdog_record("resumed", quiet, None, "", &detail);
        }
        self.loop_watch = LoopWatch {
            stretch: Some(stretch),
            owner: Arc::as_ptr(&thinking.cancel) as usize,
            limits,
            next_review_secs: limits.review_secs,
            ..LoopWatch::default()
        };
    }

    fn loop_watchdog_dispatch(&mut self, thinking: &Thinking, quiet: u64) -> Option<String> {
        let waits = self
            .loop_watch
            .actions
            .iter()
            .filter(|action| action.starts_with(Verdict::Wait.as_str()))
            .count();
        if waits >= MAX_WAITS {
            return self.loop_watchdog_apply(
                quiet,
                None,
                &format!("the reviewer said wait {waits} times"),
            );
        }
        let facts = self.loop_watchdog_facts(thinking, quiet);
        let club = self
            .loop_local_club()
            .filter(|club| club.label() != "practice" && club.is_available())
            .or_else(|| thinking.club.clone());
        let Some(club) = club else {
            return self.loop_watchdog_apply(quiet, None, "no reviewer seat");
        };
        let by = club.label().to_string();
        self.messages.push(Message::new(
            Role::Activity,
            format!(
                "⏱ loop watchdog: no harness progress for {}; {by} reviews whether the harness is wedged",
                span(quiet)
            ),
        ));
        let workspace = self.tools.current_workspace().to_path_buf();
        let prompt = format!("{}\n\n{facts}", st_connected::WATCHDOG.cells());
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let (send, rx) = std::sync::mpsc::channel();
        // A thread that cannot spawn drops `send`; that reads as no answer.
        let _ = std::thread::Builder::new()
            .name("loop-watchdog-review".into())
            .spawn(move || {
                let answer = crate::agent::harness::book::connect::chat(
                    &*club,
                    &workspace,
                    &[ChatMsg::user(prompt)],
                    None,
                    &worker_cancel,
                );
                let _ = send.send(answer);
            });
        self.loop_watch.review = Some(ReviewPending {
            by,
            started: Instant::now(),
            cancel,
            rx,
        });
        None
    }

    fn loop_watchdog_harvest_review(&mut self, quiet: u64) -> Option<String> {
        let review = self.loop_watch.review.as_ref()?;
        let answer: Result<String, String> = match review.rx.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Disconnected) => Err("the reviewer ended without an answer".into()),
            Err(TryRecvError::Empty)
                if review.started.elapsed().as_secs() >= REVIEW_DEADLINE_SECS =>
            {
                Err(format!(
                    "the reviewer gave no answer in {}",
                    span(REVIEW_DEADLINE_SECS)
                ))
            }
            Err(TryRecvError::Empty) => return None,
        };
        let review = self.loop_watch.review.take()?;
        match answer {
            Ok(text) => match parse_verdict(&text) {
                Some(verdict) => self.loop_watchdog_apply(
                    quiet,
                    Some((verdict, &review.by)),
                    &reason_line(&text),
                ),
                None => self.loop_watchdog_apply(
                    quiet,
                    None,
                    &format!("{} answered without a verdict", review.by),
                ),
            },
            Err(error) => self.loop_watchdog_apply(quiet, None, &error),
        }
    }

    /// Carry out `chosen` (a verdict and who gave it), or the fixed rule when
    /// there is none.
    fn loop_watchdog_apply(
        &mut self,
        quiet: u64,
        chosen: Option<(Verdict, &str)>,
        reason: &str,
    ) -> Option<String> {
        let (verdict, by) = chosen.unwrap_or_else(|| {
            let released_before = self
                .loop_watch
                .actions
                .iter()
                .any(|action| !action.starts_with(Verdict::Wait.as_str()));
            (
                fallback_verdict(self.tool_strip.has_running_calls(), released_before),
                "fixed rule",
            )
        });
        self.loop_watchdog_record("review", quiet, Some(verdict), by, reason);
        self.loop_watch
            .actions
            .push(format!("{} ({by})", verdict.as_str()));
        let owner = self.loop_watch.owner;
        let release = match verdict {
            Verdict::Wait => {
                let again = match self.loop_watch.limits.check_secs {
                    0 => self.loop_watch.limits.review_secs,
                    check => check,
                };
                self.loop_watch.next_review_secs = quiet.saturating_add(again);
                self.messages.push(Message::new(
                    Role::Activity,
                    format!(
                        "⏱ loop watchdog: wait ({by}: {reason}); next review in {}",
                        span(again)
                    ),
                ));
                return None;
            }
            Verdict::RestartTurn => {
                clear_release(owner);
                return Some(format!(
                    "loop watchdog: no harness progress for {}; turn restarted ({by}: {reason})",
                    span(quiet)
                ));
            }
            Verdict::HandOff => Release::HandOff,
            Verdict::StopCall => Release::Stop,
        };
        request_release(owner, release);
        self.loop_watch.released_at = Some(Instant::now());
        self.messages.push(Message::new(
            Role::Activity,
            format!(
                "⏱ loop watchdog: {} the call the turn is wedged on ({by}: {reason})",
                verdict.as_str()
            ),
        ));
        self.request_terminal_attention();
        None
    }

    /// What the iteration is blocked on, as data for the transcript, the loop
    /// file and the reviewer.
    fn loop_watchdog_facts(&self, thinking: &Thinking, quiet: u64) -> String {
        let st = &self.loop_ctl;
        let owner = Arc::as_ptr(&thinking.cancel) as usize;
        let mut facts = vec![format!(
            "loop iteration {} in flight on {}{}; iteration running {}; no harness event for {}",
            st.iteration + 1,
            crate::ui::views::turn_event_view::route_label(&thinking.requested_route),
            if st.podrace { " (podrace)" } else { "" },
            span(cycle_elapsed_secs(st).unwrap_or_else(|| thinking.started.elapsed().as_secs())),
            span(quiet),
        )];
        match self.tool_strip.oldest_running() {
            Some((name, args, age)) => {
                let mut args = args.trim().to_string();
                crate::agent::harness::truncate_to_char_boundary(&mut args, FACT_ARGS_CHARS);
                facts.push(format!(
                    "in flight: {name} for {}: {args}",
                    span(age.as_secs())
                ));
            }
            None => facts.push(
                "in flight: no tool call (waiting on the model or inside the harness)".into(),
            ),
        }
        if let Some(child) = crate::agent::harness::owned_child_snapshot(owner) {
            facts.push(format!(
                "process: {}, {} worker(s), running {}, last output {}, last CPU {}{}",
                child.program,
                child.workers,
                span(child.elapsed_secs),
                ago(child.output_age_secs),
                ago(child.cpu_age_secs),
                if child.setting_up {
                    ", still setting up"
                } else {
                    ""
                },
            ));
        }
        if let Some(delegate) = crate::agent::harness::owned_delegate_snapshot(owner) {
            facts.push(format!(
                "delegate: {} ({}), {} call(s), running {}, last event {}, last progress {}",
                delegate.label,
                delegate.phase,
                delegate.calls,
                span(delegate.elapsed_secs),
                ago(delegate.event_age_secs),
                ago(delegate.progress_age_secs),
            ));
        }
        facts.push(format!(
            "release pending: {}",
            if crate::agent::harness::release_pending(owner) {
                "yes"
            } else {
                "no"
            }
        ));
        facts.push(format!(
            "earlier watchdog actions this stretch: {}",
            if self.loop_watch.actions.is_empty() {
                "none".to_string()
            } else {
                self.loop_watch.actions.join(", ")
            }
        ));
        facts.join("\n")
    }

    fn loop_watchdog_record(
        &mut self,
        kind: &str,
        quiet: u64,
        verdict: Option<Verdict>,
        by: &str,
        detail: &str,
    ) {
        let records = &mut self.loop_ctl.watchdog;
        records.push(WatchdogRecord {
            ts_ms: now_ms(),
            iteration: self.loop_ctl.iteration,
            kind: kind.to_string(),
            quiet_secs: quiet,
            verdict: verdict.map(|verdict| verdict.as_str().to_string()),
            by: by.to_string(),
            detail: detail.to_string(),
        });
        let overflow = records.len().saturating_sub(MAX_RECORDS);
        records.drain(..overflow);
        // Like the heartbeat: publish the live tool-call count in the
        // persisted copy only.
        let settled = self.loop_ctl.tool_calls_total;
        self.loop_ctl.tool_calls_total = settled.saturating_add(self.tool_strip.snapshot().calls);
        save(&self.loop_ctl);
        self.loop_ctl.tool_calls_total = settled;
    }
}

/// The reviewer's reason: its first non-empty line that is not the verdict.
fn reason_line(text: &str) -> String {
    let mut reason = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.to_ascii_lowercase().contains("verdict:"))
        .unwrap_or("no reason given")
        .to_string();
    crate::agent::harness::truncate_to_char_boundary(&mut reason, 300);
    reason
}

fn span(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m{:02}s", secs / 60, secs % 60),
        _ => format!("{}h{:02}m", secs / 3600, secs % 3600 / 60),
    }
}

fn ago(age: Option<u64>) -> String {
    age.map_or_else(|| "never".to_string(), |age| format!("{} ago", span(age)))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/loop_ctl/watchdog__tests.rs"]
mod tests;
