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
pub(crate) const WARNING: char = '⛔';
pub(crate) const SAME_BATCH: Route = Route::new(CELL, '⠁');
pub(crate) const STORM: Route = Route::new(CELL, '⠃');
pub(crate) const POLL: Route = Route::new(CELL, '⠉');
pub(crate) const CYCLE: Route = Route::new(CELL, '⠙');
pub(crate) const REPEATS: Route = Route::new(CELL, '⠑');
pub(crate) const BLOCKED_REPEAT: Route = Route::new(CELL, '⠋');
pub(crate) const STREAK: Route = Route::new(CELL, '⠛');
pub(crate) const UNSTUCK: Route = Route::new(CELL, '⠓');
const UNSTUCK_PAGES: &[&str] = &[
    "Recent calls returned the same outcome. Review that evidence and choose the next useful step: inspect missing context, test a different hypothesis, wait for an external change, or edit when the evidence supports it.",
    "name the assumption behind the repeated call and check what could change its answer",
    "check whether the problem is in the implementation, the probe, or an external dependency before changing anything",
    "take a different tool, or a file the loop has not touched",
    "if no way forward remains, give your best answer and name what is unresolved",
];

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
                "Repeat a call when a relevant input or external condition has changed, or when another observation can inform the next decision.",
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
                "Check whether the repeated call is still useful before issuing it again.",
                "Choose the next read, experiment, edit, or wait from the evidence; a repeated observation alone does not justify rewriting code.",
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
                "A submission without a terminal receipt still needs an explicit result check; never assume a watcher will deliver it.",
                "Use the next status check when it can inform a decision; avoid tight repeated polls with no intervening work.",
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
                "Recent activity has not produced a new verified outcome after escalation: {streak} hops; last verifier outcome: {last}. Reassess the evidence and choose a useful inspection, experiment, edit, or wait. If an external prerequisite blocks progress, report it clearly.",
            ],
        },
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

pub(crate) const RESHOW_HOPS: usize = 2;

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
    quiet: usize,
    shown: usize,
    last_near: Option<u64>,
    near_spin: usize,
}

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

    pub(crate) fn spin(&self) -> usize {
        self.spin
    }

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

pub(crate) const NEAR_REPEATS: usize = 3;

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

pub(crate) fn anti_spin_counts_batch(
    mutation: bool,
    outcome: bool,
    wait_or_progress: bool,
    burns: bool,
) -> bool {
    if mutation || outcome || burns {
        return true;
    }
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

    pub(crate) fn reset(&mut self) {
        self.recent.clear();
    }

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

#[cfg(test)]
pub(crate) fn anti_spin_batch_signature(calls: &[ToolCall]) -> String {
    calls
        .iter()
        .map(toolcall_storm_signature)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn anti_spin_batch_fingerprint(calls: &[ToolCall]) -> u64 {
    let mut hasher = DefaultHasher::new();
    calls.len().hash(&mut hasher);
    for call in calls {
        call.name.hash(&mut hasher);
        feed_payload_value(&mut hasher, &call.args);
    }
    hasher.finish()
}

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

#[cfg(test)]
pub(crate) fn tool_batch_cycle_observation(
    calls: &[ToolCall],
    results: &[(String, Option<Duration>, ToolOutcome)],
) -> u64 {
    tool_batch_cycle_observation_from_calls_fp(anti_spin_batch_fingerprint(calls), results)
}

pub(crate) const TOOLCALL_STORM_THRESHOLD: usize = 3;

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

    pub(crate) fn observe(&mut self, calls: &[ToolCall]) -> Vec<usize> {
        let observed_at = std::time::Instant::now();
        let mut counts = Vec::with_capacity(calls.len());
        let mut batch: Vec<(String, std::time::Instant)> = Vec::with_capacity(calls.len());
        let mut after_edit: Option<usize> = None;
        for call in calls {
            let signature = toolcall_storm_signature(call);
            let repeats = |(prior, seen_at): &&(String, std::time::Instant)| {
                prior == &signature
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

    pub(crate) fn workspace_changed(&mut self) {
        self.hops.clear();
    }
}

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

pub(crate) const LOOP_COLLAPSE_ENV: &str = "ANGEL_LOOP_COLLAPSE";

pub(crate) const COLLAPSE_AT: usize = 3;

pub(crate) fn collapse_repeats(
    messages: &[crate::agent::club::ChatMsg],
) -> std::borrow::Cow<'_, [crate::agent::club::ChatMsg]> {
    use crate::agent::club::ChatRole;
    if !env_flag(LOOP_COLLAPSE_ENV, false) {
        return std::borrow::Cow::Borrowed(messages);
    }
    let mut signatures: Vec<Option<u64>> = vec![None; messages.len()];
    let mut counts: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    for (index, message) in messages.iter().enumerate() {
        if message.role != ChatRole::Assistant || message.tool_calls.is_empty() {
            continue;
        }
        let mut hasher = DefaultHasher::new();
        for call in message.tool_calls.iter() {
            mask_digits(&toolcall_storm_signature(call)).hash(&mut hasher);
        }
        for result in messages[index + 1..]
            .iter()
            .take_while(|next| next.role == ChatRole::Tool)
        {
            mask_digits(&result.content).hash(&mut hasher);
        }
        let signature = hasher.finish();
        signatures[index] = Some(signature);
        *counts.entry(signature).or_default() += 1;
    }
    if !counts.values().any(|count| *count >= COLLAPSE_AT) {
        return std::borrow::Cow::Borrowed(messages);
    }
    let mut kept = Vec::with_capacity(messages.len());
    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut dropping = false;
    for (index, message) in messages.iter().enumerate() {
        match signatures[index] {
            Some(signature) if counts[&signature] >= COLLAPSE_AT => {
                dropping = !seen.insert(signature);
                if !dropping {
                    kept.push(message.clone());
                }
            }
            Some(_) => {
                dropping = false;
                kept.push(message.clone());
            }
            None => {
                let rides_a_copy = dropping
                    && (message.role == ChatRole::Tool
                        || (matches!(message.role, ChatRole::Harness | ChatRole::User)
                            && message.content.starts_with(WARNING)));
                if !rides_a_copy {
                    dropping = false;
                    kept.push(message.clone());
                }
            }
        }
    }
    if let Some(signature) = signatures.iter().rev().flatten().next()
        && counts[signature] >= COLLAPSE_AT
    {
        kept.push(crate::agent::club::ChatMsg::harness(rotation_turn(
            counts[signature],
        )));
    }
    std::borrow::Cow::Owned(kept)
}

fn rotation_turn(repeats: usize) -> String {
    const ORDER: [usize; 5] = [1, 2, 3, 4, 0];
    let step = (repeats - COLLAPSE_AT) % (ORDER.len() + 1);
    if step == 0 {
        let page = PRIMARY
            .subs
            .iter()
            .find(|sub| sub.name == "same-batch")
            .and_then(|sub| sub.pages.first())
            .copied()
            .unwrap_or_default();
        let cells = SAME_BATCH.cells();
        return format!("{WARNING}{cells} ×{repeats}\n\n{cells} {page}");
    }
    let page = ORDER[step - 1];
    let cells = format!("{}{}", UNSTUCK.cells(), super::DIGITS[page]);
    format!("{WARNING}{cells}\n\n{cells} {}", UNSTUCK_PAGES[page])
}
