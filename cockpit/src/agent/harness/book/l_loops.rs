//! ⠇ l — work that came back with the same outcome.
//!
//! Four detectors, one surface: the same batch straight back (anti-spin), one
//! call re-issued across a window (storm, opt-in), the same passive poll with
//! no intervening work (poll treadmill), and a short alternating cycle
//! (opt-in). The first to see a loop throws its route; while the loop lasts,
//! one `⠇` route is shown again every [`RESHOW_HOPS`] hops of it, and changed
//! bytes re-arm it at once. None of them stops a turn.
//!
//! Across `/loop` iterations the same surface names what the last iteration
//! repeated, under the loop's setback header (`⠳⠑`): a restated plan, a
//! repeated costly action, an unchanged submission or acceptance check
//! (`⠇⠑`, one page each), a verification blocked identically (`⠇⠋`). A
//! streak of actions that changed nothing is `⠇⠛`, its count as data.

use super::{Primary, Raise, Route, Sub};
use crate::agent::harness::*;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

pub(crate) const CELL: char = '⠇';
/// A loop route rides its own turn after this sign, not the result's tail.
pub(crate) const WARNING: char = '⛔';
pub(crate) const SAME_BATCH: Route = Route::new(CELL, '⠁');
pub(crate) const STORM: Route = Route::new(CELL, '⠃');
pub(crate) const POLL: Route = Route::new(CELL, '⠉');
pub(crate) const CYCLE: Route = Route::new(CELL, '⠙');
pub(crate) const REPEATS: Route = Route::new(CELL, '⠑');
pub(crate) const BLOCKED_REPEAT: Route = Route::new(CELL, '⠋');
pub(crate) const STREAK: Route = Route::new(CELL, '⠛');
pub(crate) const UNSTUCK: Route = Route::new(CELL, '⠓');
/// The ladder a lasting loop climbs, one page per turn. The first is 0.1.6's
/// redirect (the anti-spin's second stage, at the fourth identical batch, once
/// its stop was taken away); the rest are the ways out measured on DeepSeek.
const UNSTUCK_PAGES: &[&str] = &[
    "MANDATORY REDIRECTION: You have repeated the same tool call multiple times without making progress. You are caught in a deterministic loop. Break this loop immediately: you MUST NOT repeat this call or run another inspection. Step back and use `write_file` to rewrite the implementing file cleanly from first principles, or use `str_replace` to apply a completely different fix. State your new hypothesis and edit the code now.",
    "the call's answer will not change; name the assumption behind it and test the opposite",
    "what would make this output correct? change the code, not the probe",
    "take a different tool, or a file the loop has not touched",
    "if no way forward remains, give your best answer and name what is unresolved",
];

// `⠇⠑`: each page is sent alone, as its address, under the loop's setback.
pub(crate) const SAME_PLAN: &str = "⠇⠑⠁";
pub(crate) const COSTLY_REPEAT: &str = "⠇⠑⠃";
pub(crate) const UNCHANGED_SUBMISSION: &str = "⠇⠑⠉";
pub(crate) const UNCHANGED_RED: &str = "⠇⠑⠙";

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "loops",
    surface: "work came back with the same outcome",
    subs: &[
        Sub {
            route: SAME_BATCH,
            name: "same-batch",
            signal: "the same tool batch came straight back with the same outcome (batch below)",
            action: "",
            ideas: "- Name the assumption behind the call, then check its opposite.",
            pages: &[
                "You've repeated the same tool call several times with no new result — you're stuck in a loop, not converging.",
                "Break the pattern deliberately: (1) state the key assumption your current approach depends on, then test the OPPOSITE hypothesis; (2) if that doesn't fit, reframe the problem by analogy to a different domain and see what that suggests; (3) or attack it with a different tool entirely.",
                "Do not repeat the previous tool call.",
                "If you genuinely cannot make progress, give your best final answer and flag what's unresolved.",
            ],
        },
        Sub {
            route: STORM,
            name: "storm",
            signal: "one call was re-issued unchanged across several hops (call below)",
            action: "",
            ideas: "",
            pages: &[
                "You have issued this exact call several times with identical arguments in the observation window.",
                "Do not repeat this call.",
                "You must edit the code using write_file or str_replace to fix the issue, or run a different command.",
            ],
        },
        Sub {
            route: POLL,
            name: "poll",
            signal: "the same status/log poll kept returning with no intervening work",
            action: "",
            ideas: "- Changing timestamps are not new outcomes.",
            pages: &[
                "`proc_wait` blocks until a job ends; otherwise take the next real step.",
                "Status snapshots and shell sleeps are observations, not candidate progress.",
                "If a submission is in flight, the harness watcher already owns its status and will inject WATCHER NOTIFY.",
                "Mutate the candidate, run a local preflight/benchmark, submit the current best, or report a concrete blocker before requesting another status snapshot.",
            ],
        },
        Sub {
            pages: &[],
            route: CYCLE,
            name: "cycle",
            signal: "a short cycle of batches repeated with unchanged outcomes",
            action: "break the cycle with a different hypothesis or tool",
            ideas: "",
        },
        Sub {
            route: REPEATS,
            name: "repeats",
            signal: "the last loop iteration repeated itself (one page, its counts beside it)",
            action: "",
            ideas: "",
            pages: &[
                "the previous iteration restated the same plan verbatim; a repeated plan is not progress — execute it or change it, and report the concrete result",
                "the previous iteration repeated {duplicate_costly_actions} costly action(s); justify the replication and compare its result with the earlier artifact before another retry",
                "repeated competitive submissions of unchanged candidates are banned; inspect the existing result and change the candidate mechanism before another eligible submission",
                "the unchanged acceptance check remains FAILED without rerunning the process:",
            ],
        },
        Sub {
            route: BLOCKED_REPEAT,
            name: "blocked-repeat",
            signal: "verification was blocked identically several times (count beside the route, last failure below)",
            action: "",
            ideas: "",
            pages: &[
                "verification blocked {count}× identically — do not run that command again.",
                "Fix the error it printed or verify another way, then make a bounded measurement with an explicit result the next action.",
                "Last failure:",
            ],
        },
        Sub {
            route: STREAK,
            name: "streak",
            signal: "consecutive actions changed nothing (the count and the last verifier beside the page)",
            action: "",
            ideas: "",
            pages: &[
                "unproductive streak: {streak} consecutive actions added no new sources; deliver the answer with supporting citations or an explicit missing-evidence statement with NO citations",
                "unproductive streak: {streak} consecutive actions changed nothing verifiable; the verifier's last outcome was {last}; produce a verified candidate, run the verifier with an explicit result, or report the blocker as your answer. Scratch files outside the repository (e.g. in /tmp) do not count as progress; edit the target source file directly.",
                "MANDATORY PROGRESS REDIRECTION: escalated unproductive turn: {streak} consecutive unproductive hops with no progress since escalation; last verifier outcome: {last}. You must stop inspecting and stop running unchanged commands. You MUST edit the target source code using `write_file` or `str_replace` before executing any more tools. State your concrete fix and modify the file now.",
            ],
        },
        // A loop that outlasts its warning meets a new page each time: a
        // repeated cue becomes part of the loop, a new one re-routes (live on
        // DeepSeek, only the first of 22–26 identical `⛔⠇⠁` turns changed
        // the next call).
        Sub {
            route: UNSTUCK,
            name: "unstuck",
            signal: "the loop outlasted its warning (one way out per page)",
            action: "",
            ideas: "",
            pages: UNSTUCK_PAGES,
        },
    ],
};

/// A loop that keeps looping has its `⠇` route shown again every this many
/// hops of it: the cadence of the 0.1.6 spin guard (a nudge at the second
/// identical batch, a redirect at the fourth, then again). Shown once per
/// stretch, a DeepSeek polyglot run repeated one read-only call 30–55 times
/// to the hop cap on tasks the 0.1.x builds had solved.
pub(crate) const RESHOW_HOPS: usize = 2;

/// The four loop detectors and the latch that paces their route, for one turn.
pub(crate) struct Loops {
    last_batch: Option<u64>,
    spin: usize,
    storm: Option<ToolCallStorm>,
    poll: RepeatedPollGuard,
    poll_limit: usize,
    passive_sleep_max_secs: u64,
    cycle: Option<ToolBatchCycle>,
    cycle_repeats: usize,
    armed: bool,
    /// Hops the loop has lasted since its route was last shown.
    quiet: usize,
    /// Loop turns shown in this stretch without a workspace change.
    shown: usize,
    /// The last counted batch with its outcome, numbers masked, and how many
    /// hops in a row it came back.
    last_near: Option<u64>,
    near_spin: usize,
}

/// What a batch looked like before dispatch, for the after-hop detectors.
pub(crate) struct Batch {
    pub(crate) counted: bool,
    fingerprint: Option<u64>,
    poll_fingerprint: Option<u64>,
}

impl Loops {
    pub(crate) fn from_env() -> Self {
        let cycle_max_period = env_usize("ANGEL_TOOL_CYCLE_MAX_PERIOD", 5).min(8);
        let cycle_repeats = env_usize("ANGEL_TOOL_CYCLE_REPEATS", 5).min(10);
        let poll_limit = env_usize("ANGEL_POLL_REPEAT_LIMIT", 8).min(32);
        Self {
            last_batch: None,
            spin: 0,
            storm: env_flag("ANGEL_TOOLCALL_STORM", false)
                .then(|| env_usize("ANGEL_TOOLCALL_STORM_WINDOW", 6))
                .filter(|window| *window > 0)
                .map(ToolCallStorm::new),
            poll: RepeatedPollGuard::new(poll_limit),
            poll_limit,
            passive_sleep_max_secs: env_usize("ANGEL_PASSIVE_SLEEP_MAX_SECS", 2) as u64,
            cycle: (std::env::var_os("ANGEL_TOOL_CYCLE_REPEATS").is_some()
                && cycle_max_period >= 2
                && cycle_repeats >= 2)
                .then(|| ToolBatchCycle::new(cycle_max_period, cycle_repeats)),
            cycle_repeats,
            armed: true,
            quiet: 0,
            shown: 0,
            last_near: None,
            near_spin: 0,
        }
    }

    /// Consecutive identical counted batches, for telemetry.
    pub(crate) fn spin(&self) -> usize {
        self.spin
    }

    /// Before dispatch: anti-spin and the storm window see the batch as issued,
    /// and raise on every hop their loop holds (the latch paces what is shown).
    /// `counted` is false for a pure competition board wait, whose outcome can
    /// change under the same call.
    pub(crate) fn before_dispatch(
        &mut self,
        calls: &[ToolCall],
        counted: bool,
        raises: &mut Vec<Raise>,
    ) -> Batch {
        let fingerprint = counted.then(|| anti_spin_batch_fingerprint(calls));
        if let Some(fingerprint) = fingerprint {
            if self.last_batch == Some(fingerprint) {
                self.spin += 1;
            } else {
                self.last_batch = Some(fingerprint);
                self.spin = 1;
            }
        }
        if counted && self.spin >= 2 {
            let batch = calls
                .iter()
                .map(|call| {
                    toolcall_storm_signature(call)
                        .chars()
                        .take(240)
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            raises.push(Raise::new(SAME_BATCH, batch));
        }
        if let Some(counts) = self.storm.as_mut().map(|storm| storm.observe(calls))
            && let Some((call, count)) = calls
                .iter()
                .zip(counts)
                .find(|(_, count)| *count >= TOOLCALL_STORM_THRESHOLD)
        {
            let signature = toolcall_storm_signature(call);
            raises.push(Raise::new(
                STORM,
                format!(
                    "×{count} in the window: {}",
                    signature.chars().take(240).collect::<String>()
                ),
            ));
        }
        let passive = passive_poll_only_batch(calls, self.passive_sleep_max_secs);
        let poll_fingerprint = (self.poll_limit > 0 && passive && !poll_batch_advances_work(calls))
            .then(|| anti_spin_batch_fingerprint(calls));
        Batch {
            counted,
            fingerprint,
            poll_fingerprint,
        }
    }

    /// The cycle detector's observation of a finished batch, when it is armed.
    pub(crate) fn cycle_observation(
        &self,
        batch: &Batch,
        calls: &[ToolCall],
        results: &[(String, Option<Duration>, ToolOutcome)],
    ) -> Option<u64> {
        self.cycle.as_ref().map(|_| {
            tool_batch_cycle_observation_from_calls_fp(
                batch
                    .fingerprint
                    .unwrap_or_else(|| anti_spin_batch_fingerprint(calls)),
                results,
            )
        })
    }

    /// With its results: the batch and its outcome with every run of digits
    /// masked, so a repeat that only renumbers (a fresh `/tmp/o25.txt`, a
    /// thread id in a panic, a timing) reads as the repeat it is. Paging a
    /// file changes the output, so it never matches.
    pub(crate) fn observe_outcome(
        &mut self,
        batch: &Batch,
        calls: &[ToolCall],
        results: &[(String, Option<Duration>, ToolOutcome)],
    ) {
        if !batch.counted {
            return;
        }
        let mut hasher = DefaultHasher::new();
        calls.len().hash(&mut hasher);
        for call in calls {
            mask_digits(&toolcall_storm_signature(call)).hash(&mut hasher);
        }
        for (result, _, outcome) in results {
            mask_digits(result).hash(&mut hasher);
            outcome.execution.as_str().hash(&mut hasher);
            outcome.verification.as_str().hash(&mut hasher);
        }
        let fingerprint = hasher.finish();
        if self.last_near == Some(fingerprint) {
            self.near_spin += 1;
        } else {
            self.last_near = Some(fingerprint);
            self.near_spin = 1;
        }
    }

    /// After the batch is paired with its results: a renumbered repeat, the
    /// poll treadmill and the cycle detector. A live background job is a
    /// wait, not a loop.
    pub(crate) fn after_hop(
        &mut self,
        batch: &Batch,
        cycle_observation: Option<u64>,
        mutated: bool,
        state_changed: bool,
        raises: &mut Vec<Raise>,
    ) {
        if mutated && let Some(storm) = self.storm.as_mut() {
            storm.workspace_changed();
        }
        if mutated {
            self.last_near = None;
            self.near_spin = 0;
        } else if self.near_spin >= NEAR_REPEATS
            && self.spin < 2
            && crate::agent::tools::proc::live_job_count() == 0
        {
            // An exact repeat is anti-spin's, raised before dispatch; a live
            // job's changing numbers are a wait.
            raises.push(Raise::new(
                SAME_BATCH,
                format!(
                    "×{} with only numbers changed, same outcome",
                    self.near_spin
                ),
            ));
        }
        if self.poll.observe(batch.poll_fingerprint, mutated) {
            self.poll.reset();
            if crate::agent::tools::proc::live_job_count() == 0 {
                raises.push(Raise::new(
                    POLL,
                    format!(
                        "the same poll {}× with no intervening work",
                        self.poll_limit
                    ),
                ));
            }
        }
        if let Some(detector) = self.cycle.as_mut() {
            if state_changed || !batch.counted {
                detector.clear();
            } else if let Some(period) = cycle_observation.and_then(|value| detector.observe(value))
            {
                detector.clear();
                raises.push(Raise::new(
                    CYCLE,
                    format!(
                        "a {period}-batch cycle ×{}, outcomes unchanged",
                        self.cycle_repeats
                    ),
                ));
            }
        }
    }

    /// One `⠇` route per hop, whichever detector saw the loop first: shown when
    /// a loop starts, then again every [`RESHOW_HOPS`] hops while it lasts;
    /// changed bytes re-arm it at once.
    pub(crate) fn latch(&mut self, raises: &mut Vec<Raise>, mutated: bool) {
        if raises.iter().any(|raise| raise.route.primary == CELL) {
            self.quiet += 1;
            if self.armed || self.quiet >= RESHOW_HOPS {
                self.armed = false;
                self.quiet = 0;
                self.shown += 1;
                let mut kept = false;
                raises.retain(|raise| {
                    if raise.route.primary != CELL {
                        return true;
                    }
                    let keep = !kept;
                    kept = true;
                    keep
                });
            } else {
                raises.retain(|raise| raise.route.primary != CELL);
            }
        }
        if mutated {
            self.armed = true;
            self.shown = 0;
        }
    }

    /// The cells of this hop's loop turn: the detector's route when a loop
    /// starts, then the next page of `⠇⠓`, so each turn is new to the model;
    /// the last page stands once the pages run out.
    pub(crate) fn turn_cells(&self, route_cells: String) -> String {
        let pages = UNSTUCK_PAGES.len();
        match self.shown {
            0 | 1 => route_cells,
            shown => format!(
                "{}{}",
                UNSTUCK.cells(),
                super::DIGITS[(shown - 2).min(pages - 1)]
            ),
        }
    }
}

/// A batch that comes back with only its numbers changed and the same
/// outcome is a loop at its third sighting in a row; an exact repeat is one at
/// its second. Renumbering is weaker evidence: a scan of numbered items can
/// meet the same empty answer twice.
pub(crate) const NEAR_REPEATS: usize = 3;

/// Text with each run of ASCII digits replaced by one `#`.
pub(crate) fn mask_digits(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_digits = false;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            if !in_digits {
                out.push('#');
            }
            in_digits = true;
        } else {
            out.push(ch);
            in_digits = false;
        }
    }
    out
}

/// Whether a hop's tool batch should advance the anti-spin counter.
///
/// A8: pure competition board wait/poll (no mutation, no submit/score outcome,
/// no free-form recon) must not count — the same `head`/`read_file` of the
/// living handoff can become a new outcome when the shared slot flips.
/// Repeated submit/score (outcome) and free-form recon still count.
pub(crate) fn anti_spin_counts_batch(
    mutation: bool,
    outcome: bool,
    wait_or_progress: bool,
    burns: bool,
) -> bool {
    if mutation || outcome || burns {
        return true;
    }
    // Pure wait/poll: wait_or_progress && !burns && !mutation && !outcome
    !wait_or_progress
}

fn parse_sleep_duration_secs(token: &str) -> Option<u64> {
    let token = token.trim_matches(['\'', '"', ')', '}', ',']);
    if token.eq_ignore_ascii_case("infinity") || token.eq_ignore_ascii_case("inf") {
        return Some(u64::MAX);
    }
    let (number, multiplier) = match token.as_bytes().last().copied() {
        Some(b's') | Some(b'S') => (&token[..token.len().saturating_sub(1)], 1.0),
        Some(b'm') | Some(b'M') => (&token[..token.len().saturating_sub(1)], 60.0),
        Some(b'h') | Some(b'H') => (&token[..token.len().saturating_sub(1)], 3600.0),
        Some(b'd') | Some(b'D') => (&token[..token.len().saturating_sub(1)], 86_400.0),
        _ => (token, 1.0),
    };
    let seconds = number.parse::<f64>().ok()? * multiplier;
    (seconds.is_finite() && seconds >= 0.0).then(|| seconds.ceil() as u64)
}

/// Return the longest direct `sleep DURATION` command in a shell chain.
/// Splitting only at shell control separators avoids treating prose such as
/// `echo sleep is bad` as a wait. Unknown/variable durations fail closed as an
/// unbounded wait because the harness cannot prove they are short.
pub(crate) fn shell_passive_sleep_secs(call: &ToolCall) -> Option<u64> {
    if call.name != "shell" {
        return None;
    }
    let command = crate::agent::tools::shell::shell_command_arg(&call.args)?;
    command
        .split([';', '|', '&', '\n'])
        .filter_map(|segment| {
            let mut words = segment
                .trim_start_matches(|c: char| c.is_ascii_whitespace() || matches!(c, '(' | '{'))
                .split_whitespace();
            let program = words.next()?.trim_matches(['\'', '"', '(', '{']);
            let base = program.rsplit('/').next().unwrap_or(program);
            if !base.eq_ignore_ascii_case("sleep") {
                return None;
            }
            Some(
                words
                    .next()
                    .and_then(parse_sleep_duration_secs)
                    .unwrap_or(u64::MAX),
            )
        })
        .max()
}

pub(crate) fn poll_batch_advances_work(calls: &[ToolCall]) -> bool {
    calls.iter().any(|call| {
        is_product_mutation_call(call)
            || is_local_preflight_call(call)
            || classify_tool_lane(call) == ToolLane::RunnerDispatch
            || matches!(call.name.as_str(), "proc_run" | "proc_stop")
    })
}

pub(crate) fn passive_poll_only_batch(calls: &[ToolCall], max_sleep_secs: u64) -> bool {
    !calls.is_empty()
        && calls.iter().all(|call| {
            is_passive_status_call(call)
                || shell_passive_sleep_secs(call).is_some_and(|secs| secs > max_sleep_secs)
        })
}

/// Repeated model-owned polling, even when timestamps/counters change in the
/// output. Distinct inspections remain allowed; other work clears the window.
pub(crate) struct RepeatedPollGuard {
    limit: usize,
    recent: std::collections::VecDeque<u64>,
}

impl RepeatedPollGuard {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            limit: limit.min(32),
            recent: std::collections::VecDeque::new(),
        }
    }

    /// Drop all retained observations without firing.
    pub(crate) fn reset(&mut self) {
        self.recent.clear();
    }

    /// Called only after the complete batch is paired with its results. A real
    /// workspace change or non-poll batch retires all prior observations.
    pub(crate) fn observe(&mut self, fingerprint: Option<u64>, mutated: bool) -> bool {
        let Some(fingerprint) = fingerprint.filter(|_| !mutated && self.limit > 0) else {
            self.recent.clear();
            return false;
        };
        if self.recent.len() == 32 {
            self.recent.pop_front();
        }
        self.recent.push_back(fingerprint);
        self.recent
            .iter()
            .filter(|&&value| value == fingerprint)
            .count()
            >= self.limit
    }
}

/// Stable anti-spin fingerprint for a tool batch (canonical JSON args so key
/// order cannot dodge it — same identity as storm signatures).
#[cfg(test)]
pub(crate) fn anti_spin_batch_signature(calls: &[ToolCall]) -> String {
    calls
        .iter()
        .map(toolcall_storm_signature)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Anti-spin identity. Hashes names + args in place so default hops do not
/// build or join storm strings. Key order is sorted; mutation bodies hash raw
/// (same uniqueness as [`payload_fingerprint`]).
pub(crate) fn anti_spin_batch_fingerprint(calls: &[ToolCall]) -> u64 {
    let mut hasher = DefaultHasher::new();
    calls.len().hash(&mut hasher);
    for call in calls {
        call.name.hash(&mut hasher);
        feed_payload_value(&mut hasher, &call.args);
    }
    hasher.finish()
}

/// Bounded detector for short alternating tool cycles (A→B→A→B, A→B→C→…).
/// Anti-spin catches only identical adjacent batches; this retains at most
/// `max_period * repeats` outcome-aware observations and looks for periods
/// 2..=max_period.
pub(crate) struct ToolBatchCycle {
    observations: Vec<u64>,
    max_period: usize,
    repeats: usize,
}

impl ToolBatchCycle {
    pub(crate) fn new(max_period: usize, repeats: usize) -> Self {
        Self {
            observations: Vec::with_capacity(max_period.saturating_mul(repeats)),
            max_period,
            repeats,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.observations.clear();
    }

    #[cfg(test)]
    pub(crate) fn retained(&self) -> usize {
        self.observations.len()
    }

    /// Record one fully paired tool batch and return the detected cycle period.
    pub(crate) fn observe(&mut self, observation: u64) -> Option<usize> {
        let capacity = self.max_period.saturating_mul(self.repeats);
        if capacity == 0 {
            return None;
        }
        if self.observations.len() == capacity {
            self.observations.remove(0);
        }
        self.observations.push(observation);
        let len = self.observations.len();
        (2..=self.max_period).find(|period| {
            let needed = period.saturating_mul(self.repeats);
            if len < needed {
                return false;
            }
            let suffix = &self.observations[len - needed..];
            suffix
                .iter()
                .enumerate()
                .all(|(index, value)| *value == suffix[index % period])
        })
    }
}

/// Cycle identity from an already-hashed batch: hop results fold in here
/// instead of walking write payloads twice.
pub(crate) fn tool_batch_cycle_observation_from_calls_fp(
    calls_fp: u64,
    results: &[(String, Option<Duration>, ToolOutcome)],
) -> u64 {
    let mut hasher = DefaultHasher::new();
    calls_fp.hash(&mut hasher);
    for (result, _, outcome) in results {
        result.hash(&mut hasher);
        outcome.execution.as_str().hash(&mut hasher);
        outcome.verification.as_str().hash(&mut hasher);
    }
    hasher.finish()
}

/// Hash calls together with their actual pre-cap results and typed outcomes.
/// A board/read whose output changes is progress, not a cycle, even when the
/// model repeats the same arguments. Elapsed time is deliberately excluded.
#[cfg(test)]
pub(crate) fn tool_batch_cycle_observation(
    calls: &[ToolCall],
    results: &[(String, Option<Duration>, ToolOutcome)],
) -> u64 {
    tool_batch_cycle_observation_from_calls_fp(anti_spin_batch_fingerprint(calls), results)
}

/// Occurrence at which the opt-in storm detector throws `⠇⠃`.
pub(crate) const TOOLCALL_STORM_THRESHOLD: usize = 3;

/// Sliding-window duplicate-call tracker (opt-in, `ANGEL_TOOLCALL_STORM`).
/// Distinct from anti-spin, which compares the whole batch with the one just
/// before: this matches individual calls across a window of hops.
///
/// A duplicate is the same call against the same workspace. A call's result
/// depends on the files it reads, so an edit starts a fresh window: an earlier
/// sighting described code that no longer exists (polyglot-v1: 35 of 37 storm
/// hits came right after an edit before this rule).
pub(crate) struct ToolCallStorm {
    hops: std::collections::VecDeque<Vec<(String, std::time::Instant)>>,
    window: usize,
}

impl ToolCallStorm {
    pub(crate) fn new(window: usize) -> Self {
        Self {
            hops: std::collections::VecDeque::new(),
            window: window.max(1),
        }
    }

    /// Fold one hop's batch into the window and return each call's occurrence
    /// count (1 = first sighting). Earlier duplicates in the same batch count,
    /// so a model that repeats a call inside one batch is caught too. A call
    /// that follows an edit in the same batch is compared only with what came
    /// after that edit.
    pub(crate) fn observe(&mut self, calls: &[ToolCall]) -> Vec<usize> {
        let observed_at = std::time::Instant::now();
        let mut counts = Vec::with_capacity(calls.len());
        let mut batch: Vec<(String, std::time::Instant)> = Vec::with_capacity(calls.len());
        let mut after_edit: Option<usize> = None;
        for call in calls {
            let signature = toolcall_storm_signature(call);
            let repeats = |(prior, seen_at): &&(String, std::time::Instant)| {
                prior == &signature
                    // A process snapshot is time-dependent: age these sightings.
                    && (!matches!(call.name.as_str(), "proc_status" | "proc_wait")
                        || observed_at.saturating_duration_since(*seen_at).as_secs() < 30)
            };
            let seen = match after_edit {
                Some(start) => batch[start..].iter().filter(repeats).count(),
                None => self
                    .hops
                    .iter()
                    .flatten()
                    .chain(batch.iter())
                    .filter(repeats)
                    .count(),
            };
            counts.push(seen + 1);
            batch.push((signature, observed_at));
            // An edit earlier in the batch counts as a change before it runs.
            if is_mutation_call(call) {
                after_edit = Some(batch.len());
            }
        }
        self.hops.push_back(batch);
        while self.hops.len() > self.window {
            self.hops.pop_front();
        }
        counts
    }

    /// The workspace changed during the last hop, through a direct edit or an
    /// opaque one such as a shell `sed -i`. Every earlier sighting described
    /// files that are gone. An identical rewrite that changed no bytes does not
    /// get here, so a storm of no-op writes is still caught.
    pub(crate) fn workspace_changed(&mut self) {
        self.hops.clear();
    }
}

/// Identity of a call for storm comparison: the tool name plus its arguments in
/// canonical form, so key order and whitespace cannot dodge it. Mutation
/// payloads are hashed so ordinary write hops do not allocate megabyte strings.
pub(crate) fn toolcall_storm_signature(call: &ToolCall) -> String {
    let mut args = String::new();
    write_canonical_json(&call.name, &call.args, &mut args);
    format!("{}|{args}", call.name)
}

fn write_canonical_json(tool: &str, value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_default());
                out.push(':');
                if is_competition_payload_key(tool, key) {
                    out.push('"');
                    out.push('#');
                    out.push_str(&payload_fingerprint(&map[key]));
                    out.push('"');
                } else {
                    write_canonical_json(tool, &map[key], out);
                }
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical_json(tool, item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}
