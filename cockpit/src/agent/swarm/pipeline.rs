//! The MoA pipeline stages and the run() driver.

use super::*;

struct ProposalPool {
    drafts: Vec<String>,
    failure_summary: String,
    dissent: Option<f64>,
}

struct ProposalRequest<'a> {
    base_sys: &'a str,
    rest: &'a [ChatMsg],
    problem: &'a str,
    grounding_sys: &'a str,
}

/// Per-seat proposer outcome, printed only under `ANGEL_SWARM_DRAFT_DEBUG=1`.
///
/// The pipeline's own diagnostics report *counts* ("kept 0/2 usable drafts"),
/// which cannot distinguish a seat that erred from one that answered with a
/// stub — and a stub is invisible everywhere else, because a short draft is
/// dropped silently by [`collect_text`]. When a seat contributes nothing to
/// turn after turn while the same model answers the same prompt fine outside
/// the harness, this is the missing evidence: what that seat actually said.
///
/// The preview is bounded and only printed for drafts short enough to be
/// suspicious, so an enabled debug run does not mirror whole answers to stderr.
fn log_proposer_drafts(assigned: &[Arc<dyn Club>], results: &[Result<String, String>]) {
    if !env_flag_or("ANGEL_SWARM_DRAFT_DEBUG", false) {
        return;
    }
    const SUSPICIOUS_CHARS: usize = 400;
    const PREVIEW_CHARS: usize = 240;
    for (club, result) in assigned.iter().zip(results) {
        match result {
            Ok(text) => {
                let chars = text.chars().count();
                let trimmed = text.trim();
                let verdict = if trimmed.is_empty() {
                    " BLANK — dropped"
                } else if chars < SUSPICIOUS_CHARS {
                    " short"
                } else {
                    ""
                };
                if chars < SUSPICIOUS_CHARS {
                    let preview: String = trimmed.chars().take(PREVIEW_CHARS).collect();
                    eprintln!(
                        "[swarm:draft] {} → {chars} chars{verdict} | {}",
                        club.label(),
                        preview.replace('\n', "⏎")
                    );
                } else {
                    eprintln!("[swarm:draft] {} → {chars} chars", club.label());
                }
            }
            Err(err) => eprintln!(
                "[swarm:draft] {} → ERROR: {}",
                club.label(),
                compact_error(err, 220)
            ),
        }
    }
}

impl SwarmClub {
    /// Cheap meta-cognition pass: ask the model itself whether the request wants
    /// the swarm. Any failure defaults to `Open` — fanning out is the swarm's
    /// reason to exist, so we'd rather over-think than silently degrade.
    pub(crate) fn classify(&self, problem: &str) -> Mandate {
        match self.call(CLASSIFIER_SYS, &[ChatMsg::user(problem)]) {
            Ok(verdict) => {
                let v = verdict.to_ascii_uppercase();
                if v.contains("TIGHT") && !v.contains("OPEN") {
                    Mandate::Tight
                } else {
                    Mandate::Open
                }
            }
            Err(_) => Mandate::Open,
        }
    }

    /// Advisor gate (`ANGEL_ADVISOR`): one reviewer pass over the *final* answer
    /// — the lightest MoA. On a NOTE/BLOCK verdict the concern is appended inline
    /// (and streamed) so the reader, and the next agent turn, see what the doer
    /// rushed past. A review failure is swallowed: the advisor must never cost
    /// the answer. Distinct from judge/verify, which score/revise same-turn drafts.
    pub(crate) fn maybe_advise(
        &self,
        messages: &[ChatMsg],
        answer: String,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> String {
        if !crate::agent::advisor::final_enabled() || answer.trim().is_empty() {
            return answer;
        }
        if crate::agent::advisor::already_annotated(&answer) {
            return answer;
        }
        let task = messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let prompt = crate::agent::advisor::review_prompt(&task, &answer);
        let verdict = match self.call(crate::agent::advisor::ADVISOR_SYS, &[ChatMsg::user(prompt)])
        {
            Ok(reply) => crate::agent::advisor::parse_verdict(&reply),
            Err(_) => return answer,
        };
        if verdict.is_clear() {
            return answer;
        }
        match crate::agent::advisor::annotate(&verdict) {
            Some(annotation) => {
                on_delta(StreamDelta::Content(&annotation));
                format!("{answer}{annotation}")
            }
            None => answer,
        }
    }

    /// The tight-directive path (and the all-workers-failed fallback): one direct
    /// pass through the inner club, streamed when the caller is streaming.
    pub(crate) fn single_pass(
        &self,
        history: &[ChatMsg],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        stream: bool,
    ) -> Result<String, String> {
        crate::ui::viz::agentviz::stage("direct", vec!["solo".to_string()]);
        if stream {
            self.checked_stream_text_reply(&*self.clubs.aggregate, history, cancel, on_delta)
        } else {
            Self::text_reply(
                self.chat_with_failover(&*self.clubs.aggregate, history),
                self.clubs.aggregate.label(),
            )
        }
    }

    // --- autoresearch stages -------------------------------------------------

    /// `RESEARCH`: best-effort live research to ground the proposers. Returns a
    /// formatted source list, or `None` on any failure (so the swarm degrades to
    /// ungrounded rather than erroring). Grok is an additive scout: it never
    /// replaces proposer/judge/verify/aggregate roles.
    fn research_block(&self, problem: &str, cancel: &AtomicBool) -> Option<String> {
        // Grok scout (up to ~120s) and SearXNG (8s) are independent network
        // calls that used to run back-to-back *before* any proposer launched —
        // pure serial dead time at the front of every research-routed turn.
        // Two scoped threads make the grounding stage cost max(), not sum().
        //
        // Honor a cancel already requested (Esc during an earlier stage) before
        // launching the expensive scout, and thread `cancel` into the scout
        // itself: the Grok CLI subprocess is torn down promptly on a mid-scout
        // cancel via `Club::respond_cancellable`, so Esc no longer waits out the
        // ~120s scout timeout.
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let want_grok = self.should_run_grok_research(problem) && self.clubs.research.is_some();
        let want_searx = self.k.research;
        if !want_grok && !want_searx {
            return None;
        }
        let budget = crate::agent::harness::formation_budget::current();
        let (grok, searx) = std::thread::scope(|s| {
            let grok = want_grok.then(|| {
                s.spawn(|| {
                    let _budget_scope =
                        crate::agent::harness::formation_budget::enter(budget.clone());
                    let research = self.clubs.research.as_ref()?;
                    match research.respond_cancellable(&Self::grok_research_prompt(problem), cancel)
                    {
                        Ok(text) if !text.trim().is_empty() => {
                            Some(format!("### Grok research scout\n{}", text.trim()))
                        }
                        Ok(_) => None,
                        Err(err) => {
                            eprintln!(
                                "[{}] {} research scout failed: {err}",
                                self.name,
                                research.label()
                            );
                            None
                        }
                    }
                })
            });
            let searx = want_searx.then(|| {
                s.spawn(|| {
                    self.searx_research_block(problem)
                        .map(|text| format!("### SearXNG web search\n{text}"))
                })
            });
            (
                grok.and_then(|h| h.join().unwrap_or(None)),
                searx.and_then(|h| h.join().unwrap_or(None)),
            )
        });
        let blocks: Vec<String> = [grok, searx].into_iter().flatten().collect();
        (!blocks.is_empty()).then(|| blocks.join("\n\n"))
    }

    fn should_run_research(&self, problem: &str) -> bool {
        self.k.research || self.should_run_grok_research(problem)
    }

    pub(crate) fn should_research_route(&self, problem: &str) -> bool {
        wants_live_research(problem) && (self.k.research || self.clubs.research.is_some())
    }

    fn should_run_grok_research(&self, problem: &str) -> bool {
        self.clubs.research.is_some()
            && (self.k.research
                || self.sota_moa_grok_research_default()
                || crate::agent::club::grok_research_always()
                || wants_live_research(problem))
    }

    fn sota_moa_grok_research_default(&self) -> bool {
        // Formation decks persist an explicit scout choice. The automatic
        // SOTA preset should not insert a CLI subprocess in front of every
        // deliberative coding turn; live-research prompts still engage through
        // `wants_live_research`, and operators can opt in globally here.
        self.name == "sota-moa" && env_flag_or("ANGEL_SOTA_MOA_GROK_RESEARCH", false)
    }

    fn searx_research_block(&self, problem: &str) -> Option<String> {
        let q: String = problem.chars().take(240).collect();
        let resp = ureq::get(&self.k.search_url)
            .query("q", &q)
            .query("format", "json")
            .timeout(std::time::Duration::from_secs(8))
            .call()
            .ok()?;
        let v: serde_json::Value = resp.into_json().ok()?;
        let results = v.get("results")?.as_array()?;
        let mut lines = Vec::new();
        for r in results.iter().take(6) {
            let title = r.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
            let content = r
                .get("content")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            let url = r.get("url").and_then(|x| x.as_str()).unwrap_or("").trim();
            if title.is_empty() && content.is_empty() {
                continue;
            }
            lines.push(format!("- {title} — {content} ({url})"));
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    fn grok_research_prompt(problem: &str) -> String {
        format!(
            "You are Grok, the live research scout for an angel0 mixture-of-agents \
             panel. Bring fresh web/X context back to the team without solving the \
             whole task. Focus on current, latest, trending, or online facts. Return \
             concise bullets with dates when available, include source URLs, and flag \
             uncertainty.\n\nUser request:\n{problem}"
        )
    }

    fn propose_waves(
        &self,
        request: ProposalRequest<'_>,
        stream: bool,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        trace: &mut ledger::TurnTrace,
    ) -> ProposalPool {
        let _seat_role = crate::agent::harness::formation_budget::enter_role("propose");
        let ProposalRequest {
            base_sys,
            rest,
            problem,
            grounding_sys,
        } = request;
        let max_waves = self.k.max_waves.max(1);
        let base_width = self.k.width.clamp(1, roster_len());
        let max_width = self.k.max_width.max(base_width).min(roster_len());
        let hard = max_waves > 1 && matches!(estimate_difficulty(problem), Difficulty::Hard);
        let mut wave_size = if hard {
            base_width.saturating_mul(self.k.hard_factor.max(1))
        } else {
            base_width
        }
        .clamp(1, max_width);
        let mut launched = 0usize;
        let mut pool: Vec<String> = Vec::new();
        let mut failures: Vec<String> = Vec::new();
        let mut prev_dissent = None;
        let mut final_dissent = None;

        self.emit_progress(
            stream,
            on_delta,
            "proposers",
            &format!("up to {max_width} proposer(s) across {max_waves} wave(s)"),
        );
        for wave_index in 0..max_waves {
            // A user interrupt during the proposer stage used to burn every
            // remaining wave before the pipeline noticed — check per wave, and
            // let the fan-out below stop collecting mid-wave too.
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let remaining = max_width.saturating_sub(launched);
            if remaining == 0 {
                break;
            }
            let n = wave_size.min(remaining);
            let labels = (0..n).map(|i| angle(launched + i).key).collect::<Vec<_>>();
            crate::ui::viz::agentviz::stage(format!("proposer wave {}", wave_index + 1), labels);
            self.emit_progress(
                stream,
                on_delta,
                "wave",
                &format!(
                    "{}/{} launching {} proposer(s)",
                    wave_index + 1,
                    max_waves,
                    n
                ),
            );
            let frontier = if wave_index == 0 {
                String::new()
            } else {
                contention_summary(&pool)
            };
            let started = std::time::Instant::now();
            let proposer_pool = self.available_proposers();
            let assigned = (0..n)
                .map(|i| Arc::clone(&proposer_pool[(launched + i) % proposer_pool.len()]))
                .collect::<Vec<_>>();
            let results = self.fan_out_across_on_cancel(
                &assigned,
                Some(cancel),
                self.k.seat_efforts.propose.as_deref(),
                |i| {
                    let a = angle(launched + i);
                    let deleg = if self.k.delegate && is_delegator(&a.key) {
                        format!("\n\n{DELEGATOR_INSTRUCTION}")
                    } else {
                        String::new()
                    };
                    let frontier = if frontier.trim().is_empty() {
                        String::new()
                    } else {
                        format!(
                            "\n\nEarlier drafts exposed this disagreement frontier:\n{frontier}\n\
                         Add a materially new angle or sharpen the unresolved split."
                        )
                    };
                    let stage = format!(
                        "{}\n\n{}{}{}{}",
                        a.sys, PROPOSER_BASE, grounding_sys, deleg, frontier
                    );
                    let stage = answer_stage_with_compaction_prep(&stage);
                    stage_parts(&stage, base_sys, rest, None)
                },
            );
            log_proposer_drafts(&assigned, &results);
            let set = collect_text(results);
            if set.drafts.len() < n {
                failures.push(set.failure_summary());
            }
            let novel = count_novel_against(&pool, &set.drafts, dedup_similarity());
            let returned = set.drafts.len();
            pool.extend(set.drafts);
            launched += n;
            final_dissent = draft_dissent(&pool);
            let decision = decide_wave(WaveInputs {
                wave_index,
                max_waves,
                launched_total: launched,
                max_width,
                returned,
                novel,
                wave_size: n,
                dissent: final_dissent,
                prev_dissent,
                novelty_floor: self.k.novelty_floor,
                dissent_lo: dissent_lo(),
                dissent_epsilon: self.k.dissent_epsilon,
                growth: self.k.wave_growth,
            });
            trace.waves.push(ledger::WaveTrace {
                launched: n,
                returned,
                novel,
                dissent: final_dissent,
                decision: decision.label(),
                ms: started.elapsed().as_millis(),
            });
            self.emit_progress(
                stream,
                on_delta,
                "wave",
                &format!(
                    "{}/{} returned {} · novel {} · dissent {} · {}",
                    wave_index + 1,
                    max_waves,
                    returned,
                    novel,
                    final_dissent
                        .map(|d| format!("{d:.2}"))
                        .unwrap_or_else(|| "n/a".to_string()),
                    trace
                        .waves
                        .last()
                        .map(|w| w.decision.as_str())
                        .unwrap_or("stop")
                ),
            );
            match decision {
                Decision::Stop(_) => break,
                Decision::Expand { next_wave } if next_wave > 0 => {
                    prev_dissent = final_dissent;
                    wave_size = next_wave;
                }
                Decision::Expand { .. } => break,
            }
        }

        ProposalPool {
            drafts: pool,
            failure_summary: if failures.is_empty() {
                "No proposer error payload was reported.".to_string()
            } else {
                failures.join("; ")
            },
            dissent: final_dissent,
        }
    }

    /// `REFLECT`: critique the current drafts so the next layer can fix the gaps.
    /// Score-only stages see the bounded [`aux_context`] tail by default; `0`
    /// restores the full history.
    fn critique(&self, rest: &[ChatMsg], drafts: &[String]) -> Option<String> {
        let ctx = aux_context(rest);
        let cap =
            per_draft_cap_for(&*self.clubs.aggregate, drafts.len()).unwrap_or_else(draft_min_chars);
        let task = format!(
            "Critique these {} drafts answering the problem above. List the most important \
             weaknesses, errors, contradictions, and unexplored angles across them — the \
             specific things the next revision must fix or add. Terse bullet points only.\n\n{}",
            drafts.len(),
            numbered_bounded(drafts, "Draft", cap)
        );
        let (system, msgs) = stage_parts(CRITIC_SYS, "", &ctx, Some(task));
        self.call(&system, &msgs)
            .ok()
            .filter(|t| !t.trim().is_empty())
    }

    /// `JUDGE`: score the drafts and keep the top-k (Pareto-style pruning). A
    /// single reviewer (`judge_panel == 1`) is the calibrated-judge default; a
    /// larger panel votes — every reviewer scores independently and concurrently
    /// (no anchoring; diversity from the inner model's sampling), and the per-draft
    /// *median* ranks, so no lone outlier decides a draft's fate. On any
    /// parse/scoring failure, keep all drafts rather than dropping work.
    #[cfg(test)]
    pub(crate) fn judge_select(
        &self,
        base_sys: &str,
        rest: &[ChatMsg],
        drafts: Vec<String>,
    ) -> Vec<String> {
        let _seat_role = crate::agent::harness::formation_budget::enter_role("judge");
        if drafts.len() <= 1 {
            return drafts;
        }
        // A panel that can't prune is pure spend: when every draft would be kept
        // anyway (keep ≥ len), skip the reviewer round-trips entirely. The only
        // thing lost is the strongest-first reordering, which no downstream
        // stage depends on.
        let keep = if self.k.keep > 0 {
            self.k.keep
        } else {
            drafts.len().div_ceil(2).max(2)
        };
        if keep >= drafts.len() {
            return drafts;
        }
        let cap =
            per_draft_cap_for(&*self.clubs.judge, drafts.len()).unwrap_or_else(draft_min_chars);
        let Some(ranked) = self.judge_rank(base_sys, rest, &drafts, self.k.judge_panel, cap, None)
        else {
            return drafts;
        };
        ranked
            .into_iter()
            .take(keep)
            .map(|idx| drafts[idx].clone())
            .collect()
    }

    fn judge_rank(
        &self,
        base_sys: &str,
        rest: &[ChatMsg],
        drafts: &[String],
        panel: usize,
        per_draft_chars: usize,
        weights: Option<&[usize]>,
    ) -> Option<Vec<usize>> {
        let _seat_role = crate::agent::harness::formation_budget::enter_role("judge");
        // Single-score (default) vs. per-dimension scoring: pick the prompt and the
        // matching parser together so the requested format and the parse can't drift.
        type ScoreParser = fn(&str, usize) -> Vec<(usize, f64)>;
        let weighted;
        let blocks = if let Some(weights) =
            weights.filter(|w| self.k.judge_weights && w.len() == drafts.len())
        {
            weighted = drafts
                .iter()
                .cloned()
                .zip(weights.iter().copied())
                .collect::<Vec<_>>();
            numbered_weighted_bounded(&weighted, "Response", per_draft_chars, true)
        } else {
            numbered_bounded(drafts, "Response", per_draft_chars)
        };
        let (prompt, parse): (String, ScoreParser) = if self.k.dims {
            let dims = JUDGE_DIMS
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>()
                .join(", ");
            (
                format!(
                    "Score each of the {} responses below on these dimensions, each 0 to 10: \
                     {dims}. Output one line per response as 'N: a b c d' where a b c d are the \
                     scores for {dims} in that exact order (e.g. '1: 8 6 7 5'), nothing else.\n\n{}",
                    drafts.len(),
                    blocks
                ),
                parse_dim_scores,
            )
        } else {
            (
                format!(
                    "Score each of the {} responses below from 0 to 10 for combined correctness, \
                     insight, and rigor. Output one line per response as 'N: score' (e.g. '1: 7'), \
                     nothing else.\n\n{}",
                    drafts.len(),
                    blocks
                ),
                parse_scores,
            )
        };
        // Score-only stage: bounded context, cache-aligned shape. The judge
        // keeps the conversation's own `base_sys` in the system slot so its
        // calls share the byte-identical `[system][conversation]` prefix with
        // the propose/aggregate seats of the same turn.
        let ctx = aux_context(rest);
        let (system, msgs) = stage_parts(JUDGE_SYS, base_sys, &ctx, Some(prompt));
        // Each reviewer scores blind; a single judge is just a panel of one.
        let panel = panel.max(1);
        let panel = panel.min(self.k.judge_fanout.max(1));
        let judge_pool = self.available_judges();
        let assigned = (0..panel)
            .map(|index| Arc::clone(&judge_pool[index % judge_pool.len()]))
            .collect::<Vec<_>>();
        let reviews: Vec<Vec<(usize, f64)>> = self
            .fan_out_across_on_cancel(
                &assigned,
                None,
                self.k.seat_efforts.judge.as_deref(),
                |_| (system.clone(), msgs.clone()),
            )
            .into_iter()
            .filter_map(Result::ok)
            .map(|t| parse(&t, drafts.len()))
            .filter(|r| !r.is_empty())
            .collect();
        let ranked = median_rank(&reviews);
        if ranked.is_empty() {
            return None;
        }
        Some(ranked.into_iter().map(|(idx, _)| idx).collect())
    }

    fn judge_select_scaled(
        &self,
        base_sys: &str,
        rest: &[ChatMsg],
        drafts: Vec<String>,
        weights: Option<Vec<usize>>,
    ) -> (Vec<String>, usize) {
        if drafts.len() <= 1 {
            return (drafts, 0);
        }
        crate::ui::viz::agentviz::stage(
            "judge panel",
            (0..self.k.judge_panel.max(1))
                .map(|i| format!("judge-{}", i + 1))
                .collect(),
        );
        let keep = if self.k.keep > 0 {
            self.k.keep.min(drafts.len())
        } else {
            drafts.len().div_ceil(2).max(2).min(drafts.len())
        };
        let cap = per_draft_cap_for(&*self.clubs.judge, drafts.len());
        if drafts.len() <= self.k.pod_size && cap.is_some() {
            if keep >= drafts.len() {
                return (drafts, 0);
            }
            let ranked = self.judge_rank(
                base_sys,
                rest,
                &drafts,
                self.k.judge_panel,
                cap.unwrap_or_else(draft_min_chars),
                weights.as_deref(),
            );
            return match ranked {
                Some(ranked) => (
                    ranked
                        .into_iter()
                        .take(keep)
                        .map(|idx| drafts[idx].clone())
                        .collect(),
                    1,
                ),
                None => (drafts, 1),
            };
        }

        let pods = seed_pods(drafts.len(), self.k.pod_size);
        // Each pod's rank is an independent judge round-trip; they used to run
        // one after another (6 serial rounds at 32 drafts / pod_size 6) even
        // though only the panel *inside* a rank was concurrent. Run the pods on
        // scoped threads and fold the outcomes in pod order — same winners,
        // wall-clock of one round instead of pods.len().
        struct PodOutcome {
            winners: Vec<String>,
            weights: Vec<usize>,
            judged: bool,
        }
        let budget = crate::agent::harness::formation_budget::current();
        let outcomes: Vec<PodOutcome> = std::thread::scope(|s| {
            let drafts = &drafts;
            let weights = &weights;
            let handles: Vec<_> = pods
                .into_iter()
                .map(|pod| {
                    let budget = budget.clone();
                    s.spawn(move || {
                        let _budget_scope = crate::agent::harness::formation_budget::enter(budget);
                        let pod_drafts = pod
                            .iter()
                            .map(|&idx| drafts[idx].clone())
                            .collect::<Vec<_>>();
                        let pod_weights = weights
                            .as_ref()
                            .map(|w| pod.iter().map(|&idx| w[idx]).collect::<Vec<_>>());
                        let pod_keep = self.k.pod_keep.min(pod_drafts.len()).max(1);
                        if pod_keep >= pod_drafts.len() {
                            let weights = pod_weights.unwrap_or_else(|| vec![1; pod_keep]);
                            return PodOutcome {
                                winners: pod_drafts,
                                weights,
                                judged: false,
                            };
                        }
                        let cap = per_draft_cap_for(&*self.clubs.judge, pod_drafts.len())
                            .unwrap_or_else(draft_min_chars);
                        match self.judge_rank(
                            base_sys,
                            rest,
                            &pod_drafts,
                            self.k.judge_panel,
                            cap,
                            pod_weights.as_deref(),
                        ) {
                            Some(ranked) => {
                                let mut winners = Vec::new();
                                let mut kept_weights = Vec::new();
                                for idx in ranked.into_iter().take(pod_keep) {
                                    winners.push(pod_drafts[idx].clone());
                                    kept_weights
                                        .push(pod_weights.as_ref().map(|w| w[idx]).unwrap_or(1));
                                }
                                PodOutcome {
                                    winners,
                                    weights: kept_weights,
                                    judged: true,
                                }
                            }
                            None => {
                                let weights =
                                    pod_weights.unwrap_or_else(|| vec![1; pod_drafts.len()]);
                                PodOutcome {
                                    winners: pod_drafts,
                                    weights,
                                    judged: true,
                                }
                            }
                        }
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join().unwrap_or(PodOutcome {
                        winners: Vec::new(),
                        weights: Vec::new(),
                        judged: false,
                    })
                })
                .collect()
        });
        let mut winners: Vec<String> = Vec::new();
        let mut winner_weights: Vec<usize> = Vec::new();
        let mut rounds = 0usize;
        for outcome in outcomes {
            if outcome.judged {
                rounds += 1;
            }
            winners.extend(outcome.winners);
            winner_weights.extend(outcome.weights);
        }

        if winners.len() <= 1 {
            return (winners, rounds);
        }
        let final_keep = keep.min(winners.len());
        let cap =
            per_draft_cap_for(&*self.clubs.judge, winners.len()).unwrap_or_else(draft_min_chars);
        rounds += 1;
        match self.judge_rank(
            base_sys,
            rest,
            &winners,
            self.k.judge_panel,
            cap,
            Some(&winner_weights),
        ) {
            Some(ranked) => (
                ranked
                    .into_iter()
                    .take(final_keep)
                    .map(|idx| winners[idx].clone())
                    .collect(),
                rounds,
            ),
            None => (winners, rounds),
        }
    }

    /// Draw `samples` final syntheses concurrently (self-consistency). Diversity
    /// comes from the inner model's sampling; the prompt is identical.
    fn synthesize(
        &self,
        base_sys: &str,
        rest: &[ChatMsg],
        drafts: &[String],
        weights: Option<&[usize]>,
    ) -> Vec<String> {
        let _seat_role = crate::agent::harness::formation_budget::enter_role("aggregate");
        let cap =
            per_draft_cap_for(&*self.clubs.aggregate, drafts.len()).unwrap_or_else(draft_min_chars);
        let task = if let Some(weights) = weights.filter(|w| w.len() == drafts.len()) {
            let weighted = drafts
                .iter()
                .cloned()
                .zip(weights.iter().copied())
                .collect::<Vec<_>>();
            agg_task_weighted(&weighted, false, cap)
        } else {
            agg_task_with_cap(drafts, false, cap)
        };
        let stage = answer_stage_with_compaction_prep(AGGREGATOR_SYS);
        let (system, messages) = stage_parts(&stage, base_sys, rest, Some(task));
        let results = self.fan_out(self.k.samples.max(1), |_| {
            (system.clone(), messages.clone())
        });
        keep_text(results)
    }

    fn reduce_drafts(
        &self,
        base_sys: &str,
        rest: &[ChatMsg],
        drafts: Vec<String>,
        weights: Option<Vec<usize>>,
    ) -> (Vec<String>, Option<Vec<usize>>, usize) {
        let _seat_role = crate::agent::harness::formation_budget::enter_role("reduce");
        let mut current = drafts
            .into_iter()
            .zip(
                weights
                    .unwrap_or_default()
                    .into_iter()
                    .chain(std::iter::repeat(1)),
            )
            .collect::<Vec<(String, usize)>>();
        let mut rounds = 0usize;
        // Belt-and-suspenders against a runaway reduction. A context-pressure
        // round may deliberately map a small pool one draft at a time to make
        // each result dense before the final reduce, so strict pool shrinkage
        // is not guaranteed on that one round.
        const MAX_REDUCE_ROUNDS: usize = 32;
        loop {
            let configured_fanin = self.k.agg_fanin.max(2);
            let fits =
                per_draft_cap_for(&*self.clubs.aggregate, current.len()).is_some_and(|cap| {
                    cap == 0
                        || current
                            .iter()
                            .all(|(draft, _)| draft.chars().count() <= cap)
                });
            let must_split = current.len() > 1
                && (current.len() > configured_fanin || !fits)
                && rounds < MAX_REDUCE_ROUNDS;
            if !must_split {
                let out_weights = current.iter().map(|(_, w)| *w).collect::<Vec<_>>();
                let out_drafts = current.into_iter().map(|(d, _)| d).collect::<Vec<_>>();
                return (out_drafts, Some(out_weights), rounds);
            }

            // Under context pressure, spend a dense map round instead of
            // clipping every draft to an arbitrary product ceiling. For a pool
            // already no wider than the configured fan-in, halve the fan-in;
            // two oversized drafts therefore get independent compression calls.
            let fanin = if fits || current.len() > configured_fanin {
                configured_fanin
            } else {
                current.len().div_ceil(2).max(1)
            };
            let pods = current
                .chunks(fanin)
                .map(|c| c.to_vec())
                .collect::<Vec<Vec<(String, usize)>>>();
            let pod_weights = pods
                .iter()
                .map(|pod| pod.iter().map(|(_, w)| *w).sum::<usize>())
                .collect::<Vec<_>>();
            let tasks = pods
                .iter()
                .map(|pod| {
                    let cap = per_draft_cap_for(&*self.clubs.aggregate, pod.len())
                        .unwrap_or_else(draft_min_chars);
                    let stage = answer_stage_with_compaction_prep(AGGREGATOR_SYS);
                    let (system, messages) = stage_parts(
                        &stage,
                        base_sys,
                        rest,
                        Some(agg_task_weighted(pod, true, cap)),
                    );
                    (system, messages)
                })
                .collect::<Vec<_>>();
            rounds += 1;
            let fused = self.fan_out(tasks.len(), |i| tasks[i].clone());
            // Pair each pod's fused draft with ITS OWN pod weight *before*
            // dropping failed/blank pods. `fan_out` returns one index-aligned
            // result per pod; compacting first (the old `collect_text(fused)`)
            // and only then zipping against `pod_weights` positionally paired
            // every survivor after a dropped middle pod with the wrong weight
            // and silently discarded the last one — corrupting the convergence
            // counts fed to the weighted synthesis. Zipping first, filtering
            // second, keeps every survivor on its true weight.
            let next: Vec<(String, usize)> = fused
                .into_iter()
                .zip(pod_weights)
                .filter_map(|(res, w)| match res {
                    Ok(text) if !text.trim().is_empty() => Some((text, w)),
                    _ => None,
                })
                .collect();
            if next.is_empty() {
                let out_weights = current.iter().map(|(_, w)| *w).collect::<Vec<_>>();
                let out_drafts = current.into_iter().map(|(d, _)| d).collect::<Vec<_>>();
                return (out_drafts, Some(out_weights), rounds);
            }
            current = next;
        }
    }

    /// `SAMPLES`: pick the strongest of several candidate answers. Score-only
    /// stages see the bounded [`aux_context`] tail (full history at `0`).
    fn choose_best(&self, rest: &[ChatMsg], candidates: &[String]) -> usize {
        if candidates.len() <= 1 {
            return 0;
        }
        // Self-consistency samples often converge on the same answer; when every
        // candidate is near-identical, any pick is the same pick — skip the
        // chooser round-trip (local similarity check, zero model calls).
        if all_near_identical(candidates) {
            return 0;
        }
        let ctx = aux_context(rest);
        let cap = per_draft_cap_for(&*self.clubs.aggregate, candidates.len())
            .unwrap_or_else(draft_min_chars);
        let task = format!(
            "Below are {} candidate final answers to the problem above. Reply with ONLY the \
             number (1-{}) of the single best — most correct, complete, and clear — answer.\n\n{}",
            candidates.len(),
            candidates.len(),
            numbered_bounded(candidates, "Candidate", cap)
        );
        let (system, msgs) = stage_parts(CHOOSER_SYS, "", &ctx, Some(task));
        match self.call(&system, &msgs) {
            Ok(t) => first_int(&t)
                .filter(|n| *n >= 1 && *n <= candidates.len())
                .map(|n| n - 1)
                .unwrap_or(0),
            Err(_) => 0,
        }
    }

    /// `VERIFY`: adversarially check the answer; if flawed, revise. Loop up to
    /// `verify` rounds or until the verifier is satisfied.
    pub(crate) fn verify_revise(
        &self,
        base_sys: &str,
        rest: &[ChatMsg],
        answer: String,
        cancel: &AtomicBool,
    ) -> String {
        let _seat_role = crate::agent::harness::formation_budget::enter_role("verify");
        crate::ui::viz::agentviz::stage(
            "verify",
            (0..self.k.verify.max(1))
                .map(|i| format!("verifier-{}", i + 1))
                .collect(),
        );
        // `current` is what each round verifies/revises from (so it keeps
        // exploring); `best` is what we return. Without the guard they move
        // together — `best` is just the latest revision, exactly as before. With
        // the guard on, a revision only becomes `best` if it out-scores the prior
        // best, so a round can never hand back something worse than it started.
        let mut current = answer.clone();
        let mut best = answer;
        // The verifier sees the bounded aux-context tail by default; `0`
        // restores the full-history scoring view.
        let ctx = aux_context(rest);
        let verifier_pool = self.available_verifiers();
        for pass in 0..self.k.verify {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let task = format!(
                "Check the answer below against the problem above for concrete errors, \
                 unsupported claims, logical gaps, or missing considerations. If it is \
                 genuinely solid, reply with exactly OK. Otherwise list the specific problems \
                 to fix, terse.\n\nAnswer:\n{current}"
            );
            // Score-only, but cache-aligned with the conversation's own system
            // slot: the verifier's calls share the same `[system][conversation]`
            // prefix as every other seat of the turn.
            let (system, msgs) = stage_parts(VERIFIER_SYS, base_sys, &ctx, Some(task));
            let verifier = &verifier_pool[pass % verifier_pool.len()];
            match self.call_on_with_effort(
                &**verifier,
                &system,
                &msgs,
                self.k.seat_efforts.verify.as_deref(),
            ) {
                Ok(v) if is_ok(&v) => break,
                Ok(v) => match self.revise(base_sys, rest, &current, &v) {
                    Ok(r) if !r.trim().is_empty() => {
                        // Convergence check (local, no model call): a revision
                        // that barely changed the text means the reviser had
                        // nothing real to fix — another round would re-verify
                        // the same answer and burn 2-3 more calls for nothing.
                        let converged = near_identical(&r, &current);
                        if self.k.verify_guard {
                            // Keep the better of (best so far, this revision) —
                            // skipping the score call when they're the same text.
                            if near_identical(&best, &r)
                                || self.choose_best(rest, &[best.clone(), r.clone()]) == 1
                            {
                                best = r.clone();
                            }
                        } else {
                            best = r.clone();
                        }
                        current = r;
                        if converged {
                            break;
                        }
                    }
                    _ => break,
                },
                Err(_) => break,
            }
        }
        best
    }

    /// Rewrite an answer to fix the verifier's findings, keeping what was correct.
    fn revise(
        &self,
        base_sys: &str,
        rest: &[ChatMsg],
        answer: &str,
        issues: &str,
    ) -> Result<String, String> {
        let _seat_role = crate::agent::harness::formation_budget::enter_role("revise");
        let task = format!(
            "Revise the answer below to fix these problems, keeping everything already correct. \
             Output the full corrected answer directly to the user — no preamble, no mention of \
             the revision.\n\nProblems:\n{issues}\n\nAnswer:\n{answer}"
        );
        let stage = answer_stage_with_compaction_prep(AGGREGATOR_SYS);
        let (system, msgs) = stage_parts(&stage, base_sys, rest, Some(task));
        self.call(&system, &msgs)
    }

    /// `CITE`: cross-check the final answer against the sources `research` gathered.
    /// Every nontrivial factual claim must be either supported by a listed source
    /// (attributed inline) or hedged/dropped — never back-filled with an invented
    /// citation. One pass; any failure or empty rewrite keeps the answer unchanged.
    pub(crate) fn cite_check(
        &self,
        base_sys: &str,
        rest: &[ChatMsg],
        answer: String,
        sources: &str,
    ) -> String {
        let task = format!(
            "Revise the answer below so every nontrivial factual claim is either supported by \
             one of the sources listed here — attribute it inline (e.g. 'per [source]') — or \
             explicitly hedged or removed when the sources don't support it. Never invent a \
             citation or cite a source not in this list. Keep everything already correct and \
             output the full answer directly, no preamble, no meta-commentary.\n\n\
             Sources:\n{sources}\n\nAnswer:\n{answer}"
        );
        let stage = answer_stage_with_compaction_prep(AGGREGATOR_SYS);
        let (system, msgs) = stage_parts(&stage, base_sys, rest, Some(task));
        match self.call(&system, &msgs) {
            Ok(r) if !r.trim().is_empty() => r,
            _ => answer,
        }
    }

    // --- the pipeline --------------------------------------------------------

    /// The mixture-of-agents itself. Times the turn, snapshots per-club token
    /// counters around it, and appends one line to the MoA ledger — so every
    /// turn leaves a record of what it cost and what the dissent gate decided
    /// (`/moa` reads it back). The pipeline lives in [`Self::run_inner`].
    pub(crate) fn run(
        &self,
        history: &[ChatMsg],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        stream: bool,
    ) -> Result<String, String> {
        self.run_seeded(history, cancel, on_delta, stream, None)
    }

    /// [`Self::run`] with an optional pre-computed draft folded into the pool —
    /// the tool-capable driver hop already produced a full answer before
    /// deciding to amplify, and discarding that generation wastes it. The seed
    /// rides as one more anonymous draft: the dissent gate / judge / synthesis
    /// weigh it like any proposer's, it never counts toward the SOTA-MOA
    /// proposer quorum (that wants independent drafts), and it rescues the
    /// all-proposers-failed case without a fresh single pass.
    pub(crate) fn run_seeded(
        &self,
        history: &[ChatMsg],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        stream: bool,
        seed_draft: Option<&str>,
    ) -> Result<String, String> {
        self.announce_active_mixture();
        let started = std::time::Instant::now();
        let before = ledger::usage_by_club(&self.clubs, &self.fallbacks);
        let mut trace = ledger::TurnTrace {
            route: "direct",
            formation: engaged_formation_name(),
            ..Default::default()
        };
        let result = self.run_inner(history, cancel, on_delta, stream, seed_draft, &mut trace);
        let after = ledger::usage_by_club(&self.clubs, &self.fallbacks);
        let delta = ledger::usage_delta(&before, &after);
        ledger::record_turn(
            &self.name,
            &trace,
            started.elapsed().as_millis(),
            result.is_ok(),
            &delta,
        );
        // Experience ledger: the config-keyed, reflex-facing twin of the MoA
        // ledger line above. The legacy line stays byte-identical (a public
        // contract); this one adds the reflex view — the dissent/gate decision,
        // judge/verify facts, and any text-stage failovers the pipeline drained
        // this turn (the LongCat raw-markup brownout lands here). Best-effort and
        // test-silent, so it can never fail a turn.
        let (judge_on, verify_rounds) = trace
            .effective
            .map(|(_, j, v, _)| (j, v))
            .unwrap_or((self.k.judge, self.k.verify));
        crate::knowledge::experience::record_moa(&crate::knowledge::experience::MoaExperience {
            driver: &self.name,
            route: trace.route,
            ok: result.is_ok(),
            latency_ms: started.elapsed().as_millis(),
            dissent: trace.dissent,
            gate: trace.gate,
            proposed: trace.proposed,
            kept: trace.kept,
            effective: trace.effective,
            formation: trace.formation.clone(),
            judge: judge_on.then_some(crate::knowledge::experience::JudgeFacts {
                panel: self.k.judge_panel.max(1),
                kept: trace.kept,
            }),
            verify: (verify_rounds > 0).then_some(crate::knowledge::experience::VerifyFacts {
                rounds: verify_rounds,
                passed: None,
            }),
            tokens: delta,
            failovers: crate::knowledge::experience::drain_failovers(),
        });
        result
    }

    /// The pipeline: the adaptive route gate, optional research grounding, the
    /// layered fan-out (with optional reflection), the dissent gate, optional
    /// judge pruning, the final synthesis (with optional self-consistency), and
    /// an optional verify/revise loop. Token-streams the answer on the base path.
    fn run_inner(
        &self,
        history: &[ChatMsg],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        stream: bool,
        seed_draft: Option<&str>,
        trace: &mut ledger::TurnTrace,
    ) -> Result<String, String> {
        // The persona/aggregator prompts replace the conversation's own system
        // prompt, but we fold the original into `base_sys` so the worker still
        // inherits angel's voice and standing instructions.
        let (base_sys, mut rest) = split_system(history);
        // HEDGE: fold the calibrated-claims ladder into the base voice so it rides
        // through every worker (proposers → synthesis → verify) via `compose`.
        // Default off, so the swarm's prose is unchanged.
        let base_sys = if !self.k.hedge {
            base_sys
        } else if base_sys.trim().is_empty() {
            HEDGE_LADDER.to_string()
        } else {
            format!("{base_sys}\n\n{HEDGE_LADDER}")
        };
        let problem = rest
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Nothing to attack, or a tight directive → one direct pass.
        if problem.trim().is_empty() {
            return self.single_pass(history, cancel, on_delta, stream);
        }
        // Router. Default = SPEED: a fast heuristic decides single-pass vs the full
        // swarm with NO routing round-trip — one fewer model call on *every* turn.
        // ANGEL_SWARM_SMART_ROUTE=1 restores the adaptive model classifier (trivial
        // turns still skip it via the fast path). ANGEL_SWARM_ALWAYS forces the swarm.
        let research_route = self.should_research_route(&problem);
        let deliberate = if smart_route_enabled() {
            research_route
                || !(fastpath_enabled() && obviously_tight(&problem))
                    && matches!(self.classify(&problem), Mandate::Open)
        } else {
            research_route || wants_deliberation(&problem)
        };
        if !self.k.always && !deliberate {
            return self.single_pass(history, cancel, on_delta, stream);
        }
        trace.route = "deliberate";
        self.emit_progress(
            stream,
            on_delta,
            "engaged",
            &format!(
                "width {}→≤{} · waves ≤{} · layers {} · propose {} · judge {} · verify {} · aggregate {}",
                self.k.width,
                self.k.max_width,
                self.k.max_waves,
                self.k.layers,
                self.available_proposers()
                    .iter()
                    .map(|club| club.label())
                    .collect::<Vec<_>>()
                    .join(","),
                self.clubs.judge.label(),
                self.clubs.verify.label(),
                self.clubs.aggregate.label()
            ),
        );

        // RESEARCH: ground the proposers in live sources. Keep the raw source list
        // around so the optional CITE pass can check the final answer against it.
        let sources = if self.should_run_research(&problem) {
            self.emit_progress(stream, on_delta, "research", "grounding proposer context");
            self.research_block(&problem, cancel)
        } else {
            None
        };
        let grounding_sys = sources
            .as_ref()
            .map(|g| {
                format!(
                    "\n\nResearch scout context — use what is relevant, ignore the \
                     rest, and never fabricate citations:\n{g}"
                )
            })
            .unwrap_or_default();

        // Layer 0: progressive waves of diverse proposers. Delegators
        // (empiricist/red-team base angle) also get the test-request instruction
        // when delegation is on.
        let proposal_set = self.propose_waves(
            ProposalRequest {
                base_sys: &base_sys,
                rest: &rest,
                problem: &problem,
                grounding_sys: &grounding_sys,
            },
            stream,
            cancel,
            on_delta,
            trace,
        );
        let proposal_failure_summary = proposal_set.failure_summary;
        let raw_dissent = proposal_set.dissent;
        let mut drafts = proposal_set.drafts;
        trace.proposed = drafts.len();
        if self.requires_sota_moa_quorum() && drafts.len() < self.sota_moa_min_proposer_drafts() {
            return Err(format!(
                "SOTA-MOA proposer quorum failed: kept {}/{} usable draft(s), need at least {} \
                 before synthesis. {} Set ANGEL_SOTA_MOA_ALLOW_DEGRADED=1 to allow direct/single-draft fallback.",
                drafts.len(),
                self.k.width,
                self.sota_moa_min_proposer_drafts(),
                proposal_failure_summary
            ));
        }
        // Fold the driver's pre-computed answer in as one more draft (see
        // `run_seeded`) — after the quorum check, so it never masquerades as an
        // independent proposer, but before the empty-pool fallback, which it
        // makes unnecessary.
        if let Some(seed) = seed_draft.filter(|s| !s.trim().is_empty()) {
            drafts.insert(0, seed.to_string());
        }
        if drafts.is_empty() {
            // The whole swarm failed (gemma down / overloaded). Still answer.
            trace.route = "fallback";
            return self.single_pass(history, cancel, on_delta, stream);
        }

        // DELEGATE: extract the test requests the delegators emitted, let angel
        // approve + route + run them, strip the raw blocks, and fold the executed
        // evidence into the conversation so every downstream stage reasons from
        // real results rather than the drafts' bare assertions.
        if self.k.delegate {
            let mut requests = Vec::new();
            for d in &drafts {
                requests.extend(parse_requests(d));
            }
            requests = dedupe_requests(requests, self.k.max_tests);
            if !requests.is_empty() {
                let router = Router::from_env();
                // Approval gate: with many agents you can't prompt per-call, so a
                // non-local action (phone a SOTA / remote / peer) asks once per
                // *kind*; approve-all covers the rest of the turn. Local sandboxed
                // tests run unprompted. Off unless ANGEL_SWARM_APPROVE is set, so
                // existing behavior is unchanged. The prompt is a UI round-trip —
                // no tokens, and the decision never enters any agent's context.
                let approve = std::env::var_os("ANGEL_SWARM_APPROVE").is_some();
                // Approval prompts stay serial (one modal at a time), but the
                // approved tests run concurrently: each is an independent
                // sandboxed subprocess or fleet call with a 60-180s budget that
                // used to accumulate back-to-back inside the turn.
                let denials: Vec<Option<crate::agent::swarm_delegate::TestResult>> = requests
                    .iter()
                    .map(|r| {
                        if approve && r.placement.needs_approval() {
                            let claim: String = r.claim.chars().take(80).collect();
                            let prompt = format!("swarm → {} · {claim}", r.placement.label());
                            if crate::agent::approval::ask(r.placement.approval_scope(), &prompt)
                                == crate::agent::approval::Decision::Deny
                            {
                                return Some(crate::agent::swarm_delegate::TestResult {
                                    request: r.clone(),
                                    verdict: "denied",
                                    detail: "skipped — approval denied".to_string(),
                                });
                            }
                        }
                        None
                    })
                    .collect();
                let results: Vec<_> = std::thread::scope(|s| {
                    let router = &router;
                    let handles: Vec<_> = requests
                        .iter()
                        .zip(denials)
                        .map(|(r, denied)| {
                            s.spawn(move || match denied {
                                Some(result) => result,
                                None => router.run(r),
                            })
                        })
                        .collect();
                    handles
                        .into_iter()
                        .zip(&requests)
                        .map(|(h, r)| {
                            h.join()
                                .unwrap_or_else(|_| crate::agent::swarm_delegate::TestResult {
                                    request: r.clone(),
                                    verdict: "error",
                                    detail: "test worker thread panicked".to_string(),
                                })
                        })
                        .collect()
                });
                self.emit_progress(
                    stream,
                    on_delta,
                    "delegate",
                    &format!("folding {} test result(s)", results.len()),
                );
                if std::env::var_os("ANGEL_SWARM_DEBUG").is_some() {
                    eprintln!("[swarm] {}", evidence_block(&results));
                }
                for d in drafts.iter_mut() {
                    *d = strip_blocks(d);
                }
                drafts.retain(|d| !d.trim().is_empty());
                if drafts.is_empty() {
                    trace.route = "fallback";
                    return self.single_pass(history, cancel, on_delta, stream);
                }
                rest.push(ChatMsg::harness(evidence_block(&results)));
            }
        }

        // SCALE: above the flat fan-in, cluster locally and carry one weighted
        // representative per idea family. The cluster signal gates before mean
        // pairwise dissent because large pools compress that mean toward the
        // middle even when the draft set is plainly split.
        let mut draft_weights: Option<Vec<usize>> = None;
        let mut scale_signal = None;
        if self.k.cluster && drafts.len() > self.k.scale_threshold {
            let clusters = cluster_drafts(&drafts, self.k.cluster_sim);
            scale_signal = scale_signal_at(
                &clusters,
                drafts.len(),
                self.k.dominant_relax,
                self.k.eclusters_hi,
            );
            let reps = cluster_representatives(
                &drafts,
                &clusters,
                RepMode::from_env_value(&self.k.rep_mode),
            );
            let before = drafts.len();
            draft_weights = Some(reps.iter().map(|(_, weight)| *weight).collect());
            drafts = reps.into_iter().map(|(draft, _)| draft).collect();
            if let Some(signal) = &scale_signal {
                trace.scale = Some(ledger::ScaleTrace {
                    clusters: signal.clusters,
                    dominant: signal.dominant,
                    eclusters: signal.eclusters,
                    gate_src: "cluster",
                    ..Default::default()
                });
                self.emit_progress(
                    stream,
                    on_delta,
                    "scale",
                    &format!(
                        "{} drafts → {} cluster rep(s) · dominant {:.2} · eclusters {:.2}",
                        before, signal.clusters, signal.dominant, signal.eclusters
                    ),
                );
            }
        }

        // DISSENT GATE: spend according to measured disagreement. Agreement is
        // the pipeline's own evidence the answer is easy; divergence buys judge
        // and verify scrutiny. `turn` is this turn's view of the swarm under the
        // gated knobs; every stage below runs on it.
        let dissent = if self.k.dissent_gate {
            raw_dissent.or_else(|| draft_dissent(&drafts))
        } else {
            None
        };
        let (gated_knobs, gate) = if self.k.dissent_gate && scale_signal.is_some() {
            gate_knobs_scaled_at(&self.k, scale_signal.as_ref(), dissent_verify_rounds())
        } else {
            gate_knobs(&self.k, dissent)
        };
        let turn = self.gated(gated_knobs);
        let k = &turn.k;
        trace.dissent = dissent;
        trace.gate = gate.map(GateAction::label);
        trace.effective = Some((k.layers, k.judge, k.verify, k.samples));
        if let (Some(d), Some(g)) = (dissent, gate) {
            self.emit_progress(
                stream,
                on_delta,
                "dissent",
                &format!(
                    "{d:.2} → {} · layers {} · judge {} · verify {} · samples {}",
                    g.label(),
                    k.layers,
                    if k.judge { "on" } else { "off" },
                    k.verify,
                    k.samples
                ),
            );
        }

        // Local near-duplicate collapse (zero model calls): proposers that
        // converged on the same answer add nothing to the mixture, but every
        // surviving copy costs real tokens in each downstream stage — and a
        // collapse to a single draft lets the refine layers and judge skip
        // entirely. Final synthesis still always runs: internal panel spend is
        // preserved while one dedicated stage shapes the user-facing output.
        if draft_weights.is_none() {
            let before = drafts.len();
            drafts = collapse_near_duplicates(drafts);
            if drafts.len() < before {
                self.emit_progress(
                    stream,
                    on_delta,
                    "dedup",
                    &format!("collapsed {before} → {} convergent draft(s)", drafts.len()),
                );
            }
        }
        trace.kept = drafts.len();

        // Layers 1..L: optional reflection, then persona-emphasized re-synthesis.
        // A layer that wholly fails leaves the previous drafts standing.
        for layer in 1..k.layers {
            let _seat_role = crate::agent::harness::formation_budget::enter_role("refine");
            if drafts.len() < 2 || cancel.load(Ordering::Relaxed) {
                break;
            }
            let refine_width = k.refine_width.clamp(1, roster_len());
            let agent_labels = (0..refine_width).map(|i| angle(i).key).collect::<Vec<_>>();
            crate::ui::viz::agentviz::stage(
                format!("layer {}/{}", layer, k.layers.saturating_sub(1)),
                agent_labels.clone(),
            );
            self.emit_progress(
                stream,
                on_delta,
                "layer",
                &format!("{layer}/{} refining drafts", k.layers.saturating_sub(1)),
            );
            let crit_suffix = if k.reflect {
                self.emit_progress(stream, on_delta, "reflect", "critiquing draft weaknesses");
                turn.critique(&rest, &drafts)
                    .map(|c| {
                        format!("\n\nKnown weaknesses in these drafts — fix or address them:\n{c}")
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let prev = drafts.clone();
            let prev_weights = draft_weights.clone();
            let refined = turn.fan_out(refine_width, |i| {
                let a = angle(i);
                let lens = format!(
                    "{AGGREGATOR_SYS}\n\nWork the synthesis with this emphasis: {}{crit_suffix}",
                    a.sys
                );
                let lens = answer_stage_with_compaction_prep(&lens);
                let cap = per_draft_cap_for(&*turn.clubs.aggregate, prev.len())
                    .unwrap_or_else(draft_min_chars);
                let task = if let Some(weights) = prev_weights.as_ref() {
                    let weighted = prev
                        .iter()
                        .cloned()
                        .zip(weights.iter().copied())
                        .collect::<Vec<_>>();
                    agg_task_weighted(&weighted, true, cap)
                } else {
                    agg_task_with_cap(&prev, true, cap)
                };
                stage_parts(&lens, &base_sys, &rest, Some(task))
            });
            let next = keep_text(refined);
            if !next.is_empty() {
                // Refinement pulls drafts toward each other; collapse the ones
                // that met (locally, no calls) so a converged layer ends the
                // ladder early instead of paying `width` more calls per layer.
                drafts = collapse_near_duplicates(next);
                draft_weights = None;
            }
        }

        // JUDGE: prune to the strongest drafts before the expensive synthesis.
        if k.judge {
            let n = k.judge_panel.max(1);
            crate::ui::viz::agentviz::stage(
                "judge",
                (1..=n).map(|i| format!("reviewer {i}")).collect(),
            );
            self.emit_progress(
                stream,
                on_delta,
                "judge",
                &format!("{n} reviewer(s) scoring drafts"),
            );
            let (judged, rounds) =
                turn.judge_select_scaled(&base_sys, &rest, drafts, draft_weights.take());
            drafts = judged;
            if let Some(scale) = trace.scale.as_mut() {
                scale.judge_rounds += rounds;
            }
        }

        let needs_reduce = drafts.len() > k.agg_fanin
            || !drafts_fit_untruncated_for(&*turn.clubs.aggregate, &drafts);
        if needs_reduce && drafts.len() > 1 {
            self.emit_progress(
                stream,
                on_delta,
                "reduce",
                &format!("map-reducing {} draft(s) before synthesis", drafts.len()),
            );
            let (reduced, reduced_weights, rounds) =
                turn.reduce_drafts(&base_sys, &rest, drafts, draft_weights.take());
            drafts = reduced;
            draft_weights = reduced_weights;
            if rounds > 0 {
                trace
                    .scale
                    .get_or_insert_with(|| ledger::ScaleTrace {
                        gate_src: "dissent",
                        ..Default::default()
                    })
                    .reduce_rounds += rounds;
            }
        }

        // Final synthesis. The base path (no self-consistency, no verify, no cite)
        // keeps token streaming; the heavier paths compute the answer then emit it
        // whole — they post-process after synthesis, so they can't stream live.
        let stream_final =
            stream && k.samples <= 1 && k.verify == 0 && !(k.cite && sources.is_some());

        crate::ui::viz::agentviz::stage("synthesis", vec!["aggregator".to_string()]);
        self.emit_progress(
            stream,
            on_delta,
            "synthesis",
            &format!("aggregating {} draft(s)", drafts.len()),
        );
        let _seat_role = crate::agent::harness::formation_budget::enter_role("aggregate");
        let answer = if stream_final {
            let cap = per_draft_cap_for(&*turn.clubs.aggregate, drafts.len())
                .unwrap_or_else(draft_min_chars);
            let task =
                if let Some(weights) = draft_weights.as_ref().filter(|w| w.len() == drafts.len()) {
                    let weighted = drafts
                        .iter()
                        .cloned()
                        .zip(weights.iter().copied())
                        .collect::<Vec<_>>();
                    agg_task_weighted(&weighted, false, cap)
                } else {
                    agg_task_with_cap(&drafts, false, cap)
                };
            let stage = answer_stage_with_compaction_prep(AGGREGATOR_SYS);
            let (system, messages) = stage_parts(&stage, &base_sys, &rest, Some(task));
            let mut full = Vec::with_capacity(messages.len() + 1);
            if !system.trim().is_empty() {
                full.push(ChatMsg::system(system));
            }
            full.extend(messages);
            let synthesized =
                turn.checked_stream_text_reply(&*turn.clubs.aggregate, &full, cancel, on_delta)?;
            if synthesized.trim().is_empty() {
                // The aggregator returned a successful-but-empty completion. The
                // buffered-stream path swallows live Content deltas and emits the
                // (empty) text only at the end, so the user has seen nothing yet —
                // mirror the non-streaming branch below and recover a real answer
                // via single_pass instead of returning a silent empty report.
                trace.route = "fallback";
                return self.single_pass(history, cancel, on_delta, stream);
            }
            return Ok(synthesized);
        } else {
            let candidates = turn.synthesize(&base_sys, &rest, &drafts, draft_weights.as_deref());
            if candidates.is_empty() {
                trace.route = "fallback";
                return self.single_pass(history, cancel, on_delta, stream);
            }
            let pick = turn.choose_best(&rest, &candidates);
            candidates.into_iter().nth(pick).unwrap_or_default()
        };

        // VERIFY: adversarial check → revise.
        let answer = if k.verify > 0 {
            crate::ui::viz::agentviz::stage(
                "verify",
                (1..=k.verify).map(|i| format!("checker {i}")).collect(),
            );
            self.emit_progress(
                stream,
                on_delta,
                "verify",
                &format!("{} adversarial pass(es)", k.verify),
            );
            turn.verify_revise(&base_sys, &rest, answer, cancel)
        } else {
            answer
        };

        // CITE: when research-grounded, hold the final answer's claims to the
        // gathered sources. No-op without sources, so an offline search degrades
        // to the unchecked answer rather than blocking.
        let answer = match (k.cite, &sources) {
            (true, Some(src)) => {
                self.emit_progress(stream, on_delta, "cite", "checking sourced claims");
                turn.cite_check(&base_sys, &rest, answer, src)
            }
            _ => answer,
        };

        // Non-streamed paths emit the finished answer in one delta.
        if stream {
            self.emit_progress(stream, on_delta, "done", "delivering final answer");
            on_delta(StreamDelta::Content(&answer));
        }
        Ok(answer)
    }

    pub(crate) fn emit_progress(
        &self,
        stream: bool,
        on_delta: &mut dyn FnMut(StreamDelta),
        stage: &str,
        detail: &str,
    ) {
        if !stream {
            return;
        }
        let line = format!("MOA {stage}: {detail}\n");
        on_delta(StreamDelta::Reasoning(&line));
    }

    fn requires_sota_moa_quorum(&self) -> bool {
        self.name.eq_ignore_ascii_case("sota-moa")
            && (env_flag_or("ANGEL_SOTA_MOA_REQUIRE_FULL_WIDTH", false)
                || !env_flag_or("ANGEL_SOTA_MOA_ALLOW_DEGRADED", false))
    }

    fn sota_moa_min_proposer_drafts(&self) -> usize {
        if env_flag_or("ANGEL_SOTA_MOA_REQUIRE_FULL_WIDTH", false) {
            self.k.width
        } else {
            self.k.width.clamp(1, 2)
        }
    }

    #[cfg(test)]
    pub(crate) fn with_config(
        name: impl Into<String>,
        inner: Arc<dyn Club>,
        width: usize,
        layers: usize,
        always: bool,
    ) -> Self {
        Self::with_knobs(
            name,
            inner,
            Knobs {
                width: width.clamp(2, roster_len()),
                max_width: width.clamp(2, roster_len()),
                refine_width: width.clamp(2, roster_len()),
                layers: layers.max(1),
                always,
                ..Knobs::default()
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn with_knobs(name: impl Into<String>, inner: Arc<dyn Club>, k: Knobs) -> Self {
        let clubs = RoleClubs {
            propose: Arc::clone(&inner),
            propose_extra: Vec::new(),
            judge: Arc::clone(&inner),
            judge_extra: Vec::new(),
            verify: Arc::clone(&inner),
            verify_extra: Vec::new(),
            aggregate: inner,
            aggregate_extra: Vec::new(),
            research: None,
        };
        Self {
            name: name.into(),
            clubs,
            k,
            fallbacks: Vec::new(),
            tool_preflight: Default::default(),
        }
    }
}
