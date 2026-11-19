//! The deli driver: a single-session run of the Deli_AutoResearch loop.
//! Tool-enabled callers receive an action pass after deliberation, with the
//! original native schemas. Tool-result hops continue that action phase directly.
//!
//! Where the [swarm](crate::agent::swarm) attacks a problem *in parallel* (a mixture of
//! agents in one shot), deli attacks it *over time* — a bounded sequence of
//! iterations that accumulate findings, modeled on the Deli_AutoResearch protocol
//! for long-horizon autonomous work. The protocol's full multi-day form is an
//! external orchestrator (a `/loop` + durable cron + heartbeat watchdog); this is
//! its in-cockpit, single-turn manifestation, capped by `rounds` so one driver
//! turn still terminates.
//!
//! Each round embeds the protocol's core mechanisms:
//! - **Fresh context** — an iteration sees only *curated state* (the problem, the
//!   accumulated findings, the open leads, the directions already tried), never
//!   the raw history. Context accumulation is the documented cause of cognitive
//!   loops, so state is injected via the prompt, not carried forward.
//! - **Direction diversity** — each iteration must open an angle materially
//!   distinct from every one already tried. Repeating a direction does not extend
//!   the tried-list, so the record never overstates how broad the search was.
//! - **Evidence gating** — only a finding citing a source the worker could
//!   actually have is admitted as progress. deli's worker is tool-less, so that
//!   means `premise:` (the problem statement it was handed) and `derivation:`
//!   (a logical step from a premise or an earlier finding) — and *only* those.
//!   A `file:`/`benchmark:` citation here is rejected as fabricated rather than
//!   merely unrecognized: the worker has no filesystem, so it cannot have read
//!   one. Anything not admitted is demoted to an open lead — kept, shown to the
//!   next iteration as explicitly unverified, never counted. Contrast
//!   [`loop_ctl`](crate::drive::loop_ctl), whose worker does have tools and whose
//!   citations are resolved against the real working tree.
//! - **Stall detection** — an iteration that surfaces no *new evidenced* finding
//!   (deduped on the claim, so re-citing the same assertion isn't new) raises
//!   `stale_count`.
//! - **Forced structural pivot** — once `stale_count` crosses `pivot`, the next
//!   iteration is told to change a *structural* constraint (invert a framing
//!   assumption, switch domain by analogy), not tune the same approach harder.
//! - **File-based state** (optional) — with `ANGEL_DELI_STATE_DIR` set, the
//!   protocol's `state/` files (progress / findings / hypotheses / directions /
//!   iteration log) are written each round, so an external watchdog can read
//!   progress and audit the evidence behind it.
//!
//! Reached through `/loop deli`, which wraps the loop's selected club. The
//! historical `ANGEL_DRIVER=deli` preference currently aliases `swarm` in the
//! bag; it does not select this wrapper. Tuned via `ANGEL_DELI_ROUNDS`,
//! `ANGEL_DELI_PIVOT`, `ANGEL_DELI_STALL_STOP`, `ANGEL_DELI_MIN_FINDINGS`, and
//! `ANGEL_DELI_STATE_DIR`. Wraps any [`Club`] and is testable over a mock.

use crate::agent::club::{ChatMsg, ChatRole, Club, ClubReply, StreamDelta, ToolDef};
use crate::drive::iterate::{
    EvidenceRegime, curated_prompt, finding_claim, has_evidence_tag, normalize, numbered,
    parse_iteration_sections,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const DELI_SYNTHESIS: crate::agent::harness::book::Route =
    crate::agent::harness::book::ing_drivers::DELI_SYNTHESIS;

const DELI_REGIME: EvidenceRegime = EvidenceRegime::Reasoning;

#[derive(Clone)]
struct Knobs {
    rounds: usize,
    pivot: usize,
    stall_stop: usize,
    min_findings: usize,
    state_dir: String,
}

impl Default for Knobs {
    fn default() -> Self {
        Self {
            rounds: 6,
            pivot: 2,
            stall_stop: 0,
            min_findings: 0,
            state_dir: String::new(),
        }
    }
}

#[derive(Clone)]
struct IterLog {
    iteration: usize,
    direction: String,
    new_findings: usize,
    stale_count: usize,
    error: Option<String>,
}

const EMPTY_ROUND: &str = "round returned no text (output spent before an answer)";

/// Tells whoever asked that part of the deliberation never answered, so a thin result is
/// not mistaken for a thorough one.
fn empty_rounds_note(st: &DeliState) -> Option<String> {
    let empty = st
        .log
        .iter()
        .filter(|round| round.error.as_deref() == Some(EMPTY_ROUND))
        .count();
    (empty > 0).then(|| {
        format!(
            "\n\n[deli] {empty} of {} rounds returned no text: the consulted model spent its \
             output allowance before answering. The answer above rests on the other rounds.",
            st.log.len()
        )
    })
}

#[derive(Default)]
struct DeliState {
    iteration: usize,
    findings: Vec<String>,
    hypotheses: Vec<String>,
    directions_tried: Vec<String>,
    stale_count: usize,
    log: Vec<IterLog>,
}

pub struct DeliClub {
    name: String,
    inner: Arc<dyn Club>,
    k: Knobs,
    call_effort: Option<String>,
}

impl DeliClub {
    pub fn from_env(name: impl Into<String>, inner: Arc<dyn Club>) -> Self {
        let k = Knobs {
            rounds: env_usize("ANGEL_DELI_ROUNDS", 6).max(1),
            pivot: env_usize("ANGEL_DELI_PIVOT", 2),
            stall_stop: env_usize("ANGEL_DELI_STALL_STOP", 0),
            min_findings: env_usize("ANGEL_DELI_MIN_FINDINGS", 0),
            state_dir: std::env::var("ANGEL_DELI_STATE_DIR").unwrap_or_default(),
        };
        Self {
            name: name.into(),
            inner,
            k,
            call_effort: None,
        }
    }

    pub(crate) fn for_consult(
        inner: Arc<dyn Club>,
        rounds: Option<usize>,
        effort: Option<String>,
    ) -> Self {
        let mut deli = Self::from_env(inner.label().to_string(), inner);
        if let Some(rounds) = rounds {
            deli.k.rounds = rounds;
        }
        deli.k.state_dir.clear();
        deli.call_effort = effort;
        deli
    }

    fn call_cancellable(
        &self,
        system: &str,
        msgs: &[ChatMsg],
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        if cancel.load(Ordering::Acquire) {
            return Err("deli cancelled".into());
        }
        let mut full = Vec::with_capacity(msgs.len() + 1);
        full.push(ChatMsg::system(system));
        full.extend_from_slice(msgs);
        crate::agent::harness::book::connect::chat(
            &*self.inner,
            &crate::agent::harness::book::connect::workspace(),
            &full,
            self.call_effort.as_deref(),
            cancel,
        )
    }

    fn iterate(&self, problem: &str, base_sys: &str, cancel: &AtomicBool) -> DeliState {
        let mut st = DeliState::default();
        let mut seen: HashSet<String> = HashSet::new();
        let mut seen_hypotheses: HashSet<String> = HashSet::new();
        let mut seen_directions: HashSet<String> = HashSet::new();
        let system = compose(
            &crate::agent::harness::book::er_loop::WORKER.cells(),
            base_sys,
        );
        for _ in 0..self.k.rounds {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let pivot = st.stale_count >= self.k.pivot;
            let mut prompt = curated_prompt(
                problem,
                &st.findings,
                &st.hypotheses,
                &st.directions_tried,
                pivot,
                DELI_REGIME,
            );
            let workspace = crate::knowledge::experience::current_turn_workspace();
            if let Some(workspace) = &workspace {
                prompt.push_str(&crate::drive::labyrinth::context(workspace, problem));
            }
            let mut round_error = None;
            let (direction, reported, mut leads) =
                match self.call_cancellable(&system, &[ChatMsg::user(prompt)], cancel) {
                    // A round that spends its whole output allowance before answering
                    // comes back empty. Recorded as a fault: silence would read as
                    // "looked and found nothing".
                    Ok(out) if out.trim().is_empty() => {
                        round_error = Some(EMPTY_ROUND.to_string());
                        (String::new(), Vec::new(), Vec::new())
                    }
                    Ok(out) => parse_iteration_sections(&out),
                    Err(e) => {
                        round_error = Some(e);
                        (String::new(), Vec::new(), Vec::new())
                    }
                };
            let mut fresh = Vec::new();
            for f in reported {
                if !has_evidence_tag(&f, DELI_REGIME) {
                    leads.push(f);
                    continue;
                }
                let key = normalize(finding_claim(&f));
                if !key.is_empty() && seen.insert(key) {
                    fresh.push(f);
                }
            }
            let mut fresh_leads = Vec::new();
            for lead in leads {
                let key = normalize(finding_claim(&lead));
                if !key.is_empty() && !seen.contains(&key) && seen_hypotheses.insert(key) {
                    fresh_leads.push(lead.clone());
                    st.hypotheses.push(lead);
                }
            }
            let new_findings = fresh.len();
            if let Some(workspace) = &workspace
                && let Err(error) = crate::drive::labyrinth::observe_iteration(
                    workspace,
                    "deli",
                    problem,
                    &fresh,
                    &fresh_leads,
                )
            {
                eprintln!("[labyrinth] Deli observations unavailable: {error}");
            }
            if new_findings == 0 {
                st.stale_count += 1;
            } else {
                st.stale_count = 0;
                st.findings.extend(fresh);
            }
            let direction = direction.trim();
            if !direction.is_empty() && seen_directions.insert(normalize(direction)) {
                st.directions_tried.push(direction.to_string());
            }
            st.iteration += 1;
            st.log.push(IterLog {
                iteration: st.iteration,
                direction: direction.trim().to_string(),
                new_findings,
                stale_count: st.stale_count,
                error: round_error,
            });
            self.persist(&st);
            if self.k.stall_stop > 0 && st.stale_count >= self.k.stall_stop {
                break;
            }
            if self.k.min_findings > 0 && st.findings.len() >= self.k.min_findings {
                break;
            }
        }
        st
    }

    #[cfg(test)]
    fn synthesize(
        &self,
        base_sys: &str,
        problem: &str,
        findings: &[String],
        hypotheses: &[String],
    ) -> Result<String, String> {
        self.synthesize_cancellable(
            base_sys,
            problem,
            findings,
            hypotheses,
            &AtomicBool::new(false),
        )
    }

    fn synthesize_cancellable(
        &self,
        base_sys: &str,
        problem: &str,
        findings: &[String],
        hypotheses: &[String],
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        use crate::agent::harness::book::d3_roles::pages;
        if findings.is_empty() && hypotheses.is_empty() {
            let system = compose(&pages(DELI_SYNTHESIS, [1, 2]), base_sys);
            return self.call_cancellable(&system, &[ChatMsg::user(problem)], cancel);
        }
        let system = compose(&DELI_SYNTHESIS.cells(), base_sys);
        let findings_txt = if findings.is_empty() {
            "none established".to_string()
        } else {
            numbered(findings)
        };
        let leads_txt = if hypotheses.is_empty() {
            String::new()
        } else {
            format!("\n\nopen leads:\n{}", numbered(hypotheses))
        };
        let msg = format!("Problem:\n{problem}\n\nfindings:\n{findings_txt}{leads_txt}");
        self.call_cancellable(&system, &[ChatMsg::user(msg)], cancel)
    }

    fn persist(&self, st: &DeliState) {
        if self.k.state_dir.is_empty() {
            return;
        }
        let Some(workspace) = crate::knowledge::experience::current_turn_workspace() else {
            return;
        };
        let identity = crate::platform::workspace_store::repo_identity(&workspace);
        let dir = std::path::Path::new(&self.k.state_dir)
            .join(identity.key)
            .join("state");
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let status = if self.k.stall_stop > 0 && st.stale_count >= self.k.stall_stop {
            "stuck"
        } else {
            "running"
        };
        let progress = serde_json::json!({
            "iteration": st.iteration,
            "total_findings": st.findings.len(),
            "open_leads": st.hypotheses.len(),
            "stale_count": st.stale_count,
            "status": status,
        });
        let _ = crate::platform::workspace_store::write_atomic(
            &dir.join("progress.json"),
            progress.to_string().as_bytes(),
        );
        let _ = crate::platform::workspace_store::write_atomic(
            &dir.join("directions_tried.json"),
            serde_json::to_string(&st.directions_tried)
                .unwrap_or_default()
                .as_bytes(),
        );
        let findings = st
            .findings
            .iter()
            .map(|f| serde_json::json!({ "finding": f }).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = crate::platform::workspace_store::write_atomic(
            &dir.join("findings.jsonl"),
            findings.as_bytes(),
        );
        let hypotheses = st
            .hypotheses
            .iter()
            .map(|h| serde_json::json!({ "hypothesis": h }).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = crate::platform::workspace_store::write_atomic(
            &dir.join("hypotheses.jsonl"),
            hypotheses.as_bytes(),
        );
        let log = st
            .log
            .iter()
            .map(|l| {
                serde_json::json!({
                    "iteration": l.iteration,
                    "direction": l.direction,
                    "new_findings": l.new_findings,
                    "stale_count": l.stale_count,
                    "error": l.error,
                })
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let _ = crate::platform::workspace_store::write_atomic(
            &dir.join("iteration_log.jsonl"),
            log.as_bytes(),
        );
    }

    fn run(
        &self,
        history: &[ChatMsg],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        stream: bool,
    ) -> Result<String, String> {
        if cancel.load(Ordering::Acquire) {
            return Err("deli cancelled".into());
        }
        let (base_sys, rest) = split_system(history);
        let problem = rest
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        if problem.trim().is_empty() {
            let reply = if stream {
                self.inner.chat_streaming(history, &[], cancel, on_delta)?
            } else {
                self.inner.chat(history, &[])?
            };
            return match reply {
                ClubReply::Text(t) => Ok(t),
                ClubReply::Calls(_) => Err("deli pass requested a tool (none offered)".to_string()),
            };
        }

        let st = self.iterate(&problem, &base_sys, cancel);
        let mut answer =
            self.synthesize_cancellable(&base_sys, &problem, &st.findings, &st.hypotheses, cancel)?;
        if let Some(note) = empty_rounds_note(&st) {
            answer.push_str(&note);
        }
        if stream {
            on_delta(StreamDelta::Content(&answer));
        }
        Ok(answer)
    }

    fn run_with_tools(
        &self,
        history: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if cancel.load(Ordering::Acquire) {
            return Err("deli cancelled".into());
        }
        let last_user = history
            .iter()
            .rposition(|message| message.role == ChatRole::User);
        let action_started = history
            .iter()
            .skip(last_user.map_or(0, |index| index + 1))
            .any(|message| message.role == ChatRole::Tool || !message.tool_calls.is_empty());
        if action_started {
            return self
                .inner
                .chat_streaming_with_effort(history, tools, effort, cancel, on_delta);
        }
        let (base_sys, _) = split_system(history);
        let problem = last_user
            .map(|index| history[index].content.as_ref())
            .unwrap_or("");
        if problem.trim().is_empty() {
            return self
                .inner
                .chat_streaming_with_effort(history, tools, effort, cancel, on_delta);
        }
        let state = self.iterate(problem, &base_sys, cancel);
        if cancel.load(Ordering::Acquire) {
            return Err("deli cancelled".into());
        }
        let mut action = history.to_vec();
        action.push(ChatMsg::harness(format!(
            "{}\nfindings:\n{}\n\nopen leads:\n{}",
            crate::agent::harness::book::ing_drivers::DELI.cells(),
            numbered(&state.findings),
            numbered(&state.hypotheses),
        )));
        self.inner
            .chat_streaming_with_effort(&action, tools, effort, cancel, on_delta)
    }

    #[cfg(test)]
    fn with_knobs(name: impl Into<String>, inner: Arc<dyn Club>, k: Knobs) -> Self {
        Self {
            name: name.into(),
            inner,
            k,
            call_effort: None,
        }
    }
}

impl Club for DeliClub {
    fn label(&self) -> &str {
        &self.name
    }

    fn usage_accounting(&self) -> crate::agent::club::AccountingView {
        self.inner.usage_accounting()
    }

    fn is_available(&self) -> bool {
        self.inner.is_available()
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        let history = [ChatMsg::user(prompt)];
        self.run(&history, &AtomicBool::new(false), &mut |_| {}, false)
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        if !tools.is_empty() {
            return self.run_with_tools(
                messages,
                tools,
                None,
                &AtomicBool::new(false),
                &mut |_| {},
            );
        }
        self.run(messages, &AtomicBool::new(false), &mut |_| {}, false)
            .map(ClubReply::Text)
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if !tools.is_empty() {
            return self.run_with_tools(messages, tools, None, cancel, on_delta);
        }
        self.run(messages, cancel, on_delta, true)
            .map(ClubReply::Text)
    }

    fn chat_streaming_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if tools.is_empty() {
            return self.chat_streaming(messages, tools, cancel, on_delta);
        }
        self.run_with_tools(messages, tools, effort, cancel, on_delta)
    }

    fn metadata(&self) -> Option<crate::agent::club::Metadata> {
        self.inner.metadata()
    }
    fn metadata_cached(&self) -> Option<crate::agent::club::Metadata> {
        self.inner.metadata_cached()
    }
    fn model_identity(&self) -> Option<String> {
        self.inner.model_identity()
    }
    fn route_identity(&self) -> crate::agent::club::RouteIdentity {
        self.inner.route_identity()
    }
    fn reasoning_effort(&self) -> Option<String> {
        self.inner.reasoning_effort()
    }
    fn reasoning_levels(&self) -> &[String] {
        self.inner.reasoning_levels()
    }
}

#[derive(Default, Clone)]
struct CalibrationRun {
    worker: String,
    problem: String,
    iterations: usize,
    admitted: usize,
    leads: usize,
    fabricated: usize,
    untagged: usize,
    stale_count: usize,
    directions: usize,
    error: Option<String>,
}

impl CalibrationRun {
    fn compliance(&self) -> f64 {
        let total = self.admitted + self.leads;
        if total == 0 {
            return 0.0;
        }
        self.admitted as f64 / total as f64
    }
}

fn score_run(
    worker: &str,
    problem: &str,
    st: &DeliState,
    regime: EvidenceRegime,
) -> CalibrationRun {
    let mut out = CalibrationRun {
        worker: worker.to_string(),
        problem: problem.chars().take(48).collect(),
        iterations: st.iteration,
        admitted: st.findings.len(),
        leads: st.hypotheses.len(),
        stale_count: st.stale_count,
        directions: st.directions_tried.len(),
        error: st
            .log
            .iter()
            .find_map(|l| l.error.clone())
            .map(|e| e.chars().take(60).collect()),
        ..Default::default()
    };
    for lead in &st.hypotheses {
        match crate::drive::iterate::evidence_source(lead) {
            None => out.untagged += 1,
            Some(source) if regime.is_fabricated_kind(source) => out.fabricated += 1,
            Some(_) => {}
        }
    }
    out
}

fn split_system(history: &[ChatMsg]) -> (String, Vec<ChatMsg>) {
    let mut sys = String::new();
    let mut rest = Vec::new();
    for m in history {
        if m.role == ChatRole::System {
            if !sys.is_empty() {
                sys.push_str("\n\n");
            }
            sys.push_str(&m.content);
        } else {
            rest.push(m.clone());
        }
    }
    (sys, rest)
}

fn compose(primary: &str, base_sys: &str) -> String {
    if base_sys.trim().is_empty() {
        primary.to_string()
    } else {
        format!("{primary}\n\n{base_sys}")
    }
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DeliCostEstimate {
    pub rounds: usize,
    pub est_tokens: usize,
    pub min_tokens_runway: usize,
    pub est_secs: u64,
    pub est_credits: f64,
    pub min_credits_runway: f64,
}

impl DeliCostEstimate {
    pub(crate) fn for_rounds(rounds: usize) -> Self {
        let r = rounds.max(1);
        let est_tokens = 12_000usize.saturating_mul(r).saturating_add(8_000);
        let est_secs = 75u64.saturating_mul(r as u64).saturating_add(45);
        let est_credits = 0.30 * (r as f64 + 1.0);
        Self {
            rounds: r,
            est_tokens,
            min_tokens_runway: est_tokens.saturating_mul(5) / 2,
            est_secs,
            est_credits,
            min_credits_runway: est_credits * 2.5,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LoopRunwaySnapshot {
    pub started_ms: u64,
    pub deadline_secs: u64,
    pub token_budget: usize,
    pub tokens_spent: usize,
    pub max_iters: usize,
    pub iteration: usize,
}

static ACTIVE_LOOP_RUNWAY: std::sync::Mutex<Option<LoopRunwaySnapshot>> =
    std::sync::Mutex::new(None);

pub(crate) fn set_active_loop_runway(snapshot: Option<LoopRunwaySnapshot>) {
    if let Ok(mut guard) = ACTIVE_LOOP_RUNWAY.lock() {
        *guard = snapshot;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DeliDisengaged {
    pub est: DeliCostEstimate,
    pub reason: String,
}

impl DeliDisengaged {
    pub(crate) fn consult_notice(&self, label: &str) -> String {
        format!(
            "[consult_model club={label}] [method=deli disengaged — endgame runway]\n             Deli (rounds={}, est. ~{} tok / ~${:.2}) does not engage at the end of a bounded timer or budget ({}).\n             Spend remaining runway shipping direct, bounded, verifiable increments immediately.",
            self.est.rounds, self.est.est_tokens, self.est.est_credits, self.reason
        )
    }
}

fn endgame_time_floor_secs(total_secs: u64, est_secs: u64) -> u64 {
    let ten_pct = total_secs / 10;
    if total_secs >= 2_400 {
        900u64.max(ten_pct)
    } else if total_secs > 0 {
        (total_secs * 12 / 100).max(est_secs.min(total_secs / 2))
    } else {
        900
    }
}

pub(crate) fn check_deli_runway_with(
    rounds: usize,
    now_epoch_ms: u64,
    loop_snap: Option<LoopRunwaySnapshot>,
    env_deadline_epoch: Option<u64>,
    env_total_secs: Option<u64>,
    credits_remaining_and_cap: Option<(f64, Option<f64>)>,
) -> Result<DeliCostEstimate, DeliDisengaged> {
    let est = DeliCostEstimate::for_rounds(rounds);

    if let Some(snap) = loop_snap {
        if snap.deadline_secs > 0 && snap.started_ms > 0 {
            let elapsed_s = now_epoch_ms.saturating_sub(snap.started_ms) / 1_000;
            let rem_s = snap.deadline_secs.saturating_sub(elapsed_s);
            let floor_s = endgame_time_floor_secs(snap.deadline_secs, est.est_secs);
            if rem_s < floor_s {
                return Err(DeliDisengaged {
                    est,
                    reason: format!("{}m {}s left on {}m loop timer (< {}m endgame cutoff)", rem_s / 60, rem_s % 60, snap.deadline_secs / 60, floor_s / 60),
                });
            }
        }
        if snap.token_budget > 0 {
            let rem_tok = snap.token_budget.saturating_sub(snap.tokens_spent);
            let floor_tok = (snap.token_budget / 10).max(est.min_tokens_runway.min(snap.token_budget / 2));
            if rem_tok < floor_tok {
                return Err(DeliDisengaged {
                    est,
                    reason: format!("{rem_tok} tokens left of {} token budget (< {floor_tok} endgame cutoff)", snap.token_budget),
                });
            }
        }
        if snap.max_iters > 0 {
            let rem_iters = snap.max_iters.saturating_sub(snap.iteration);
            let floor_iters = (snap.max_iters / 10).max(1);
            if rem_iters <= floor_iters && snap.max_iters > 1 {
                return Err(DeliDisengaged {
                    est,
                    reason: format!("{rem_iters} iteration(s) left of {} max_iters", snap.max_iters),
                });
            }
        }
    }

    if let Some(dl_epoch) = env_deadline_epoch.filter(|&d| d > 0) {
        let now_s = now_epoch_ms / 1_000;
        let rem_s = dl_epoch.saturating_sub(now_s);
        let floor_s = endgame_time_floor_secs(env_total_secs.unwrap_or(0), est.est_secs);
        if rem_s < floor_s {
            return Err(DeliDisengaged {
                est,
                reason: format!("{}m left before run deadline (< {}m endgame cutoff)", rem_s / 60, floor_s / 60),
            });
        }
    }

    if let Some((left, cap)) = credits_remaining_and_cap {
        let pct_floor = cap.filter(|&c| c > 0.0).map(|c| c * 0.10).unwrap_or(0.0);
        let floor = est.min_credits_runway.max(pct_floor);
        if left < floor {
            return Err(DeliDisengaged {
                est,
                reason: format!("{left:.2} credits left (< {floor:.2} endgame runway floor)"),
            });
        }
    }

    Ok(est)
}

fn probe_gateway_credits() -> Option<(f64, Option<f64>)> {
    if let Ok(rem) = std::env::var("ANGEL_BUDGET_REMAINING_USD")
        && let Ok(left) = rem.trim().parse::<f64>()
        && left.is_finite()
    {
        let cap = std::env::var("ANGEL_BUDGET_MAX_USD")
            .ok()
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0);
        return Some((left, cap));
    }
    let url = std::env::var("OPENRSI_GATEWAY_STATUS_URL")
        .or_else(|_| std::env::var("ANGEL_GATEWAY_STATUS_URL"))
        .ok()?;
    if url.trim().is_empty() {
        return None;
    }
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_millis(150))
        .build();
    let val: serde_json::Value = agent.get(url.trim()).call().ok()?.into_json().ok()?;
    let left = val.pointer("/remaining/normalized_usd").and_then(|v| v.as_f64())?;
    let cap = val.pointer("/limits/max_model_credits").and_then(|v| v.as_f64());
    Some((left, cap))
}

pub(crate) fn check_deli_runway(rounds: Option<usize>) -> Result<DeliCostEstimate, DeliDisengaged> {
    let default_rounds = env_usize("ANGEL_DELI_ROUNDS", 3);
    let r = rounds.unwrap_or(default_rounds);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let snap = ACTIVE_LOOP_RUNWAY.lock().ok().and_then(|g| *g);
    let dl = std::env::var("ANGEL_DEADLINE_EPOCH")
        .or_else(|_| std::env::var("DUNGENESS_AGENT_DEADLINE_EPOCH"))
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok());
    let total_s = std::env::var("ANGEL_TOTAL_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok());
    check_deli_runway_with(r, now_ms, snap, dl, total_s, probe_gateway_credits())
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/deli__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/deli_usage_tests.rs"]
mod usage_tests;
