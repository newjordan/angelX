//! A loop started from the launch line, and a launch with nobody at the keyboard.
//!
//! `--goal TEXT` and `--loop TEXT` do at startup what `/goal TEXT` and
//! `/loop TEXT` do when typed. `--unattended` adds three things: the cockpit
//! runs on a host with no window (`remote::serve`, so `angel --attach NAME`
//! can still watch it), a pause that only waits for a person to type
//! `/loop resume` is resumed, and the process ends when the loop is over, with
//! the reason on stderr and in the exit status.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::agent::harness::TurnStopReason;
use crate::agent::harness::book::d45_iteration::TURN_STOPPED;
use crate::drive::loop_ctl::LoopStatus;
use crate::interactive_launch::InteractiveLaunch;

/// The loop finished, stopped itself, or reached a cap the launch gave it.
pub(crate) const ENDED: i32 = 0;
/// The loop failed, or the cockpit quit while it was still running.
pub(crate) const FAILED: i32 = 1;
/// `--loop TEXT` did not start a loop.
pub(crate) const NOT_STARTED: i32 = 2;
/// The loop waits for something only a person can give.
pub(crate) const NEEDS_A_PERSON: i32 = 3;
/// The loop kept pausing straight after being resumed.
pub(crate) const KEEPS_PAUSING: i32 = 4;

/// A pause this soon after a resume means the resumed turn did no work.
pub(crate) const QUICK_PAUSE: Duration = Duration::from_secs(60);
/// This many such pauses in a row end the run.
pub(crate) const QUICK_PAUSE_LIMIT: u32 = 5;
/// How long a loop may wait for a person's approval before the run ends.
pub(crate) const APPROVAL_WAIT: Duration = Duration::from_secs(120);

/// How an unattended launch ended: the exit status and the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ending {
    pub code: i32,
    pub why: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Wait,
    Resume,
    End(Ending),
}

/// What the loop's own saved state says.
pub(crate) struct Seen<'a> {
    pub status: LoopStatus,
    /// `LoopState::last_setback`.
    pub setback: Option<&'a str>,
    /// `LoopState::last_error`.
    pub error: Option<&'a str>,
    /// `LoopState::execution_blocker` is set.
    pub blocked: bool,
    /// `LoopState::paused_for_cap`.
    pub at_cap: bool,
}

/// The reason a stopped inner turn paused the loop, as the loop recorded it.
fn stopped_turn(setback: &str) -> Option<&str> {
    setback
        .strip_prefix(TURN_STOPPED)?
        .trim_start()
        .strip_prefix("reason=")
        .map(str::trim)
}

/// A turn that ran out its own time or hops, or lost its provider, leaves the
/// loop's work whole: the next iteration can go on from it. The other stops
/// (an interrupt, a blocked host, a refusal, a failed save) are not repaired by
/// trying again.
fn resumes_by_itself(reason: &str) -> bool {
    [
        TurnStopReason::IdleTimeout,
        TurnStopReason::Deadline,
        TurnStopReason::MaxHops,
        TurnStopReason::ProviderError,
    ]
    .iter()
    .any(|stop| stop.as_str() == reason)
}

fn at_cap(seen: &Seen<'_>) -> bool {
    seen.at_cap
        || seen.setback.is_some_and(|setback| {
            setback.starts_with("budget reached:") || setback.starts_with("operator cap reached:")
        })
}

/// What an unattended launch does about a loop that is not running.
/// `quick_pauses` counts the pauses in a row that came within `QUICK_PAUSE` of
/// a resume, this one included.
pub(crate) fn next_step(seen: &Seen<'_>, quick_pauses: u32) -> Step {
    let said = || seen.error.or(seen.setback).unwrap_or("no reason recorded");
    let end = |code, why: String| Step::End(Ending { code, why });
    // A metering gateway's explicit `run_action=stop` ends the run cleanly at
    // once regardless of whether the loop landed in Paused, Failed, or Running.
    if seen
        .error
        .is_some_and(crate::agent::club::error_indicates_gateway_stop)
    {
        return end(ENDED, format!("gateway stopped the run: {}", said()));
    }
    match seen.status {
        LoopStatus::Baselining
        | LoopStatus::Running
        | LoopStatus::Verifying
        | LoopStatus::AwaitingApproval => Step::Wait,
        LoopStatus::Done => end(ENDED, "the loop is done".to_string()),
        LoopStatus::Stopped => end(ENDED, format!("the loop stopped: {}", said())),
        LoopStatus::Failed => end(FAILED, format!("the loop failed: {}", said())),
        LoopStatus::Idle => end(FAILED, "no loop is running".to_string()),
        LoopStatus::Paused if at_cap(seen) => {
            end(ENDED, format!("the loop reached its cap: {}", said()))
        }
        LoopStatus::Paused if seen.blocked => end(
            NEEDS_A_PERSON,
            format!("the loop is blocked: {}", said()),
        ),
        LoopStatus::Paused => match seen.setback.and_then(stopped_turn) {
            Some(reason) if resumes_by_itself(reason) => {
                if quick_pauses >= QUICK_PAUSE_LIMIT {
                    end(
                        KEEPS_PAUSING,
                        format!(
                            "the loop paused {quick_pauses} times in a row straight after a resume ({reason}): {}",
                            said()
                        ),
                    )
                } else {
                    Step::Resume
                }
            }
            _ => end(
                NEEDS_A_PERSON,
                format!("the loop paused and waits for a person: {}", said()),
            ),
        },
    }
}

/// How long to leave a pause before resuming it: nothing after a turn that
/// worked, then 10 s doubling to 160 s after turns that did not.
pub(crate) fn resume_delay(quick_pauses: u32) -> Duration {
    match quick_pauses {
        0 => Duration::ZERO,
        n => Duration::from_secs(10 << (n - 1).min(4)),
    }
}

static ENDING: OnceLock<Ending> = OnceLock::new();

/// The launch line's goal and loop, and the unattended run's own bookkeeping.
pub(crate) struct LaunchLoop {
    goal: Option<String>,
    start: Option<String>,
    unattended: bool,
    started: bool,
    ended: bool,
    resumed_at: Option<Instant>,
    resume_due: Option<Instant>,
    quick_pauses: u32,
    pause_counted: bool,
    awaiting_since: Option<Instant>,
}

impl LaunchLoop {
    pub(crate) fn from_launch(launch: &InteractiveLaunch) -> Option<Self> {
        if launch.goal.is_none() && launch.loop_start.is_none() {
            return None;
        }
        Some(Self {
            goal: launch.goal.clone(),
            start: launch.loop_start.clone(),
            unattended: launch.unattended,
            started: false,
            ended: false,
            resumed_at: None,
            resume_due: None,
            quick_pauses: 0,
            pause_counted: false,
            awaiting_since: None,
        })
    }

    /// One pass, after the cockpit has advanced.
    pub(crate) fn step(&mut self, app: &mut crate::App) {
        if self.ended {
            return;
        }
        if !self.started {
            if app.flight_owner().holds_turn_slot() || app.pending_approval.is_some() {
                return;
            }
            self.started = true;
            app.refresh_goal_from_disk();
            if let Some(goal) = self.goal.take() {
                let text = app.update_goal(Some(goal));
                app.system_msg(text);
            }
            if let Some(start) = self.start.take() {
                let text = app.loop_command(Some(start));
                let started = app.loop_active();
                app.system_msg(text.clone());
                if self.unattended && !started {
                    self.end(
                        app,
                        Ending {
                            code: NOT_STARTED,
                            why: format!("--loop did not start a loop: {text}"),
                        },
                    );
                }
            }
            return;
        }
        if !self.unattended {
            return;
        }
        if crate::agent::harness::deadline_epoch_remaining_secs().is_some_and(|rem| rem == 0) {
            if app.loop_active() {
                let _ = app.loop_command(Some("stop".to_string()));
            }
            self.end(
                app,
                Ending {
                    code: ENDED,
                    why: "deadline reached (ANGEL_DEADLINE_EPOCH)".to_string(),
                },
            );
            return;
        }
        if app
            .loop_ctl
            .last_error
            .as_deref()
            .is_some_and(crate::agent::club::error_indicates_gateway_stop)
        {
            if app.loop_active() {
                let _ = app.loop_command(Some("stop".to_string()));
            }
            let detail = app
                .loop_ctl
                .last_error
                .clone()
                .unwrap_or_else(|| "gateway requested stop".to_string());
            self.end(
                app,
                Ending {
                    code: ENDED,
                    why: format!("gateway stopped the run: {detail}"),
                },
            );
            return;
        }
        if app.loop_ctl.status == LoopStatus::AwaitingApproval {
            let since = *self.awaiting_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= APPROVAL_WAIT {
                self.end(
                    app,
                    Ending {
                        code: NEEDS_A_PERSON,
                        why: "the loop waits for a person to approve an escalation".to_string(),
                    },
                );
            }
            return;
        }
        self.awaiting_since = None;
        if app.loop_active() {
            self.pause_counted = false;
            self.resume_due = None;
            return;
        }
        if app.loop_ctl.status == LoopStatus::Paused && !self.pause_counted {
            self.pause_counted = true;
            self.quick_pauses = match self.resumed_at {
                Some(at) if at.elapsed() < QUICK_PAUSE => self.quick_pauses + 1,
                _ => 0,
            };
        }
        let step = next_step(
            &Seen {
                status: app.loop_ctl.status,
                setback: app.loop_ctl.last_setback.as_deref(),
                error: app.loop_ctl.last_error.as_deref(),
                blocked: app.loop_ctl.execution_blocker.is_some(),
                at_cap: app.loop_ctl.paused_for_cap,
            },
            self.quick_pauses,
        );
        match step {
            Step::Wait => {}
            Step::Resume => {
                let delay = resume_delay(self.quick_pauses);
                let due = *self.resume_due.get_or_insert_with(|| Instant::now() + delay);
                if Instant::now() < due {
                    return;
                }
                self.resume_due = None;
                let text = app.loop_command(Some("resume".to_string()));
                if app.loop_active() {
                    self.resumed_at = Some(Instant::now());
                    self.pause_counted = false;
                    app.system_msg(format!("unattended · {text}"));
                } else {
                    self.end(
                        app,
                        Ending {
                            code: NEEDS_A_PERSON,
                            why: format!("the loop would not resume: {text}"),
                        },
                    );
                }
            }
            Step::End(ending) => self.end(app, ending),
        }
    }

    fn end(&mut self, app: &mut crate::App, ending: Ending) {
        self.ended = true;
        app.system_msg(format!("unattended · ending: {}", ending.why));
        let _ = ENDING.set(ending);
        app.should_quit = true;
    }
}

/// The cockpit with no terminal, until its loop is over.
#[cfg(feature = "glass")]
pub(crate) fn run(launch: InteractiveLaunch, workspace: std::path::PathBuf) -> std::io::Result<()> {
    let name = crate::remote::serve_requested()
        .unwrap_or_else(|| format!("loop-{}", std::process::id()));
    eprintln!("angel --unattended: session {name}; angel --attach {name} watches it");
    crate::remote::serve(&name, launch, workspace)?;
    let ending = ENDING.get().cloned().unwrap_or(Ending {
        code: FAILED,
        why: "the cockpit quit before the loop ended".to_string(),
    });
    eprintln!("angel --unattended: {}", ending.why);
    if ending.code != ENDED {
        std::process::exit(ending.code);
    }
    Ok(())
}

#[cfg(not(feature = "glass"))]
pub(crate) fn run(_launch: InteractiveLaunch, _workspace: std::path::PathBuf) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "--unattended needs a build with the glass feature (its host with no window)",
    ))
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/unattended__tests.rs"]
mod tests;
