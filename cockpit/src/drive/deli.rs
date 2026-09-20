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
    EvidenceRegime, WORKER_SYS, curated_prompt, finding_claim, has_evidence_tag, normalize,
    numbered, parse_iteration_sections,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Stage prompts
// ---------------------------------------------------------------------------
//
// The worker prompt, the structural-pivot reframe, the curated-state prompt
// builder, finding parsing and dedup all live in [`crate::drive::iterate`] now, shared
// with the cross-turn loop controller. Only the synthesis prompt — deli's own
// final-answer stage — stays here.

/// Fuses the accumulated findings into the final answer for the user.
const DELI_SYNTH_SYS: &str = "You synthesize the accumulated findings of an autonomous work \
    loop into one final answer for the user. Integrate them, resolve contradictions, discard \
    dead ends, and answer the problem directly — never narrate the loop or the iteration \
    process.";

/// deli's worker is offered no tools ([`DeliClub::call_cancellable`] passes an empty tool
/// list and rejects a tool request outright), so it reasons over the problem
/// statement alone. Asking it for filesystem or benchmark citations it has no
/// way to obtain produces invented ones — see [`EvidenceRegime`].
const DELI_REGIME: EvidenceRegime = EvidenceRegime::Reasoning;

// ---------------------------------------------------------------------------
// Knobs
// ---------------------------------------------------------------------------

/// Every tunable for a deli run, built from env by [`DeliClub::from_env`].
#[derive(Clone)]
struct Knobs {
    /// Hard cap on iterations for one driver turn (≥1). The protocol's 15-round
    /// session cap, shrunk so an interactive turn still terminates.
    rounds: usize,
    /// `stale_count` at which the next iteration is forced to pivot structure.
    pivot: usize,
    /// `stale_count` at which the loop gives up early (0 = never; runs full rounds).
    stall_stop: usize,
    /// Stop early once this many distinct findings accumulate (0 = run all rounds).
    min_findings: usize,
    /// Directory for the protocol's `state/` files (empty = in-memory only).
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

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// One iteration's summary, mirroring the protocol's `iteration_log.jsonl` line.
#[derive(Clone)]
struct IterLog {
    iteration: usize,
    direction: String,
    new_findings: usize,
    stale_count: usize,
    /// Why this round surfaced nothing, when the cause was a failed call rather
    /// than a worker that simply broke no new ground. A round that produced
    /// nothing is ambiguous without this — "the model errored" and "the model
    /// had nothing to add" demand opposite responses from anything watching.
    error: Option<String>,
}

/// The accumulated state of a run — the in-memory form of the protocol's `state/`.
///
/// Findings and hypotheses are kept apart on purpose. Only an *evidenced* finding
/// is progress: it enters `findings` and resets the stall counter. Everything else
/// — a bullet the worker labeled a hypothesis, or one it called a finding without
/// citing a checkable source — lands in `hypotheses`, rides into the next curated
/// prompt for validation, and never resets stall detection. Demoting rather than
/// discarding keeps honest labeling free: a worker loses nothing by admitting an
/// idea is untested, so it has no incentive to dress one up as a fact.
#[derive(Default)]
struct DeliState {
    iteration: usize,
    findings: Vec<String>,
    hypotheses: Vec<String>,
    directions_tried: Vec<String>,
    stale_count: usize,
    log: Vec<IterLog>,
}

// ---------------------------------------------------------------------------
// DeliClub
// ---------------------------------------------------------------------------

/// A [`Club`] that answers by running a bounded Deli_AutoResearch loop over an
/// inner club, then synthesizing the findings.
pub struct DeliClub {
    name: String,
    inner: Arc<dyn Club>,
    k: Knobs,
    call_effort: Option<String>,
}

impl DeliClub {
    /// Build from env: `ANGEL_DELI_ROUNDS` (6, ≥1), `ANGEL_DELI_PIVOT` (2),
    /// `ANGEL_DELI_STALL_STOP` (0 = off), `ANGEL_DELI_MIN_FINDINGS` (0 = off),
    /// `ANGEL_DELI_STATE_DIR` (unset = in-memory).
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

    /// A model-selected side excursion. Its result returns to the calling
    /// turn; it must not overwrite an operator's long-running Deli state.
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

    /// One blocking call to the inner club with a fresh system prompt in front of
    /// `msgs`. Deliberation rounds themselves are text-only; their action pass
    /// receives the full native tool set.
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
        match self.inner.chat_streaming_with_effort(
            &full,
            &[],
            self.call_effort.as_deref(),
            cancel,
            &mut |_| {},
        )? {
            ClubReply::Text(t) => Ok(t),
            ClubReply::Calls(_) => Err("deli worker requested a tool (none offered)".to_string()),
        }
    }

    /// Run the bounded loop, accumulating findings. Each round runs on a *fresh*
    /// context (only [`curated_prompt`](crate::drive::iterate::curated_prompt)), detects
    /// stalls, and forces a structural pivot once stalled. Returns the state.
    fn iterate(&self, problem: &str, base_sys: &str, cancel: &AtomicBool) -> DeliState {
        let mut st = DeliState::default();
        let mut seen: HashSet<String> = HashSet::new();
        let mut seen_hypotheses: HashSet<String> = HashSet::new();
        // Direction identity is retained once per invocation. The ordered
        // original strings still ride in prompts and persist files; membership
        // uses this set so each prior direction is not re-normalized on every
        // later round. Distinct from `seen` / `seen_hypotheses`, which key off
        // finding *claims* (evidence-stripped) rather than DIRECTION lines.
        let mut seen_directions: HashSet<String> = HashSet::new();
        let system = compose(WORKER_SYS, base_sys);
        for _ in 0..self.k.rounds {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let pivot = st.stale_count >= self.k.pivot;
            let prompt = curated_prompt(
                problem,
                &st.findings,
                &st.hypotheses,
                &st.directions_tried,
                pivot,
                DELI_REGIME,
            );
            let mut round_error = None;
            let (direction, reported, mut leads) =
                match self.call_cancellable(&system, &[ChatMsg::user(prompt)], cancel) {
                    Ok(out) => parse_iteration_sections(&out),
                    // A failed iteration counts as a stall, not a crash — the loop
                    // keeps going (bounded by rounds / stall_stop) rather than
                    // aborting. The reason is retained so a stalled run can say
                    // whether the worker was failing or merely unproductive.
                    Err(e) => {
                        round_error = Some(e);
                        (String::new(), Vec::new(), Vec::new())
                    }
                };
            // Only an evidenced finding is progress. An unevidenced one is a
            // lead, not a fact — it joins the hypotheses instead of inflating
            // the ledger and masking a stall. Dedup keys off the claim, so the
            // same assertion re-cited a second way is still not new ground.
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
            for lead in leads {
                let key = normalize(finding_claim(&lead));
                if !key.is_empty() && !seen.contains(&key) && seen_hypotheses.insert(key) {
                    st.hypotheses.push(lead);
                }
            }
            let new_findings = fresh.len();
            if new_findings == 0 {
                st.stale_count += 1;
            } else {
                st.stale_count = 0;
                st.findings.extend(fresh);
            }
            // A repeated DIRECTION must not grow the tried-list: it rides in
            // every later prompt, and a padded record misreports the search as
            // broader than it was. Insert the normalized key once; keep the
            // first original string in `directions_tried`.
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

    /// Fuse the accumulated findings into a single answer. With nothing surfaced
    /// (e.g. the worker was down), fall back to a direct pass so the user still
    /// gets an answer rather than an empty one.
    /// Open leads are handed to the synthesizer alongside the findings, marked
    /// unverified. A run can legitimately end with leads but no evidenced
    /// finding; discarding them there would throw away the entire loop and
    /// answer from a cold pass instead. Only a genuinely empty run — no findings
    /// *and* no leads, e.g. the worker was down — falls through to a direct pass.
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
        let system = compose(DELI_SYNTH_SYS, base_sys);
        if findings.is_empty() && hypotheses.is_empty() {
            return self.call_cancellable(&system, &[ChatMsg::user(problem)], cancel);
        }
        let findings_txt = if findings.is_empty() {
            "none established".to_string()
        } else {
            numbered(findings)
        };
        let leads_txt = if hypotheses.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nOpen leads (raised but NOT evidenced — do not present these as \
                 established; use them only where the answer must acknowledge an open \
                 question):\n{}",
                numbered(hypotheses)
            )
        };
        let msg = format!(
            "Problem:\n{problem}\n\nEvidenced findings from autonomous iteration:\n\
             {findings_txt}{leads_txt}\n\n\
             Synthesize these into a single, direct, well-organized answer to the problem. \
             Integrate them, resolve contradictions, drop dead ends, and answer the user \
             directly — no mention of the iteration process. Distinguish what is established \
             from what remains open; never state an open lead as fact.",
        );
        self.call_cancellable(&system, &[ChatMsg::user(msg)], cancel)
    }

    /// Best-effort write under `state_dir/<canonical-project-key>/state/`. A
    /// no-op when no dir or project-bound turn is configured; IO failures are
    /// swallowed so persistence can never break a run.
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
        let _ = std::fs::write(dir.join("progress.json"), progress.to_string());
        let _ = std::fs::write(
            dir.join("directions_tried.json"),
            serde_json::to_string(&st.directions_tried).unwrap_or_default(),
        );
        let findings = st
            .findings
            .iter()
            .map(|f| serde_json::json!({ "finding": f }).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(dir.join("findings.jsonl"), findings);
        let hypotheses = st
            .hypotheses
            .iter()
            .map(|h| serde_json::json!({ "hypothesis": h }).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(dir.join("hypotheses.jsonl"), hypotheses);
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
        let _ = std::fs::write(dir.join("iteration_log.jsonl"), log);
    }

    /// Drive a turn: iterate, then synthesize. Streams the final answer in one
    /// delta on the streaming path (the loop itself can't stream — it's many
    /// internal calls before the answer exists).
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

        // Nothing to work on → one direct pass through the inner club.
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
        let answer =
            self.synthesize_cancellable(&base_sys, &problem, &st.findings, &st.hypotheses, cancel)?;
        if stream {
            on_delta(StreamDelta::Content(&answer));
        }
        Ok(answer)
    }

    /// Deliberate once before the action phase, then keep the tool protocol
    /// intact. Subsequent tool-result hops must not restart all Deli rounds.
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
            "[Deli deliberation]\nFindings with cited support (check against actual tool evidence):\n{}\n\nOpen leads, unverified:\n{}\n\nUse the available tools to investigate, execute and verify the next useful action. RL campaigns and MoA remain available. Submit when you have a verified winner.",
            numbered(&state.findings), numbered(&state.hypotheses),
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

    /// Deli runs its rounds against the same inner model endpoint, so it's only
    /// reachable when that endpoint is — delegate the readiness probe.
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

// ---------------------------------------------------------------------------
// Live calibration
// ---------------------------------------------------------------------------

/// What one live deli run produced, scored against the regime it ran under.
///
/// The number that matters is `fabricated`: findings citing a source kind the
/// worker had no way to obtain. Under [`EvidenceRegime::Reasoning`] that count
/// must be zero — a tool-less worker citing `file:` or `benchmark:` invented the
/// reference. It is measured rather than assumed because the failure is a
/// property of the *model*, not of this code, and models change under us.
#[derive(Default, Clone)]
struct CalibrationRun {
    worker: String,
    problem: String,
    iterations: usize,
    /// Findings admitted as progress (carried an in-regime citation).
    admitted: usize,
    /// Claims kept as unverified open leads (labeled hypotheses + demotions).
    leads: usize,
    /// Bullets whose citation names a kind this worker cannot obtain. Must be 0.
    fabricated: usize,
    /// Bullets carrying no `[evidence: …]` tag at all.
    untagged: usize,
    stale_count: usize,
    directions: usize,
    error: Option<String>,
}

impl CalibrationRun {
    /// Share of emitted bullets that carried a well-formed in-regime citation.
    fn compliance(&self) -> f64 {
        let total = self.admitted + self.leads;
        if total == 0 {
            return 0.0;
        }
        self.admitted as f64 / total as f64
    }
}

/// Score every bullet a run produced against `regime`, without re-calling the
/// model: the state carries the admitted findings, and the open leads carry
/// everything that was demoted or self-labeled.
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
        // A run that surfaced nothing is only interpretable alongside why: a
        // failing worker and an unproductive one look identical in the counts.
        error: st
            .log
            .iter()
            .find_map(|l| l.error.clone())
            .map(|e| e.chars().take(60).collect()),
        ..Default::default()
    };
    // A demoted lead still carries whatever citation the worker attached, so
    // this is where fabrication shows up: the gate rejected it, and we count
    // *why* it was rejected.
    for lead in &st.hypotheses {
        match crate::drive::iterate::evidence_source(lead) {
            None => out.untagged += 1,
            Some(source) if regime.is_fabricated_kind(source) => out.fabricated += 1,
            Some(_) => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Split a conversation into (merged system text, non-system messages).
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

/// Prepend `primary`, then fold in the conversation's own system prompt (if any).
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "../../../tests/cockpit/app/deli__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/deli_usage_tests.rs"]
mod usage_tests;
