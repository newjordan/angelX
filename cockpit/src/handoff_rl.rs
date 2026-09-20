//! Handoff RL controller — forced reinforcement-learning competition loop with
//! cockpit-owned **demand + clear + inject** restarts.
//!
//! Start options match the agent `/loop` workshop: duration, roll cap (iters),
//! and token budget (including the five-day podrace profile). After a submission
//! *result* is in, the cockpit demands handoff, wipes conversation history, and
//! prompt-injects a fresh starter that begins with `hit it chewy`.

use crate::loop_dialog::LoopLaunchSettings;
use serde::{Deserialize, Serialize};

pub(crate) const HANDOFF_RL_STARTER: &str = "hit it chewy";

pub(crate) const HANDOFF_RL_SEQUENCE_DIRECTIVE: &str = "I want you to compete for me using this cockpit and its agentic tools/resources at your disposal. \
    I want you to place victories on the board, remember to check the board before submitting. \
    - ALWAYS BE IMPROVING: the revolving door never stops. The current BEST goes up to bat now. \
    Sitting, polling, or waiting on a prepped submission is a failure. \
    - Once a submission is in play, immediately improve the next best on the newest winning baseline: \
    isolate the next hot-path hypothesis, price the phase, cross-compile locally for register/spill checks, \
    assert zero-fallback correctness, and submit that bat. The harness watcher owns in-flight status.";

/// Evidence that the host should demand a forced handoff restart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HandoffDemand {
    /// Compact receipt for the note (submit + result fingerprints).
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct VictoryEntry {
    pub candidate_id: String,
    pub score: f64,
    pub hypothesis: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct HandoffRlState {
    pub active: bool,
    /// Operator task / campaign brief (mirrors `/loop` task).
    pub task: String,
    pub winning_baseline: Option<String>,
    pub winning_score: Option<f64>,
    pub current_hypothesis: Option<String>,
    pub victory_board: Vec<VictoryEntry>,
    pub handoff_count: usize,
    pub last_handoff_note: Option<String>,
    /// Successful submit observed since the last forced roll. Cleared on roll.
    pub pending_submit: bool,
    /// Last successful submit fingerprint(s), for the injection note.
    pub last_submit_note: Option<String>,

    // --- budgets (0 = off / endless), same contract as LoopState ---
    /// Max forced rolls (handoff injections). 0 = endless.
    pub max_iters: usize,
    /// Wall-clock deadline from `started_ms`. 0 = endless.
    pub deadline_secs: u64,
    /// Cumulative rough token budget across rolls. 0 = endless.
    pub token_budget: usize,
    pub tokens_spent: usize,
    pub started_ms: u64,
    /// Optional inter-roll cadence hint (workshop/interval parity with `/loop`).
    pub interval_secs: u64,
    /// Five-day competition profile: unlimited rolls/tokens, fixed deadline.
    pub podrace: bool,
    /// Why the campaign last stopped (budget / operator).
    pub last_stop_reason: Option<String>,
}

impl HandoffRlState {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Arm the campaign with optional hypothesis/task text (budgets applied separately).
    pub fn start(&mut self, hypothesis: Option<&str>) -> String {
        self.active = true;
        self.last_stop_reason = None;
        if self.started_ms == 0 {
            self.started_ms = now_ms();
        }
        if let Some(hyp) = hypothesis {
            let hyp = hyp.trim();
            if !hyp.is_empty() {
                self.current_hypothesis = Some(hyp.to_string());
                if self.task.is_empty() {
                    self.task = hyp.to_string();
                }
            }
        }
        format!(
            "handoff RL started · forced clear/inject loop armed (starter '{}'). \
             Handoff is demanded only after a submission result is in.",
            HANDOFF_RL_STARTER
        )
    }

    /// Apply the same workshop settings the agent loop uses.
    pub fn apply_launch_settings(&mut self, settings: &LoopLaunchSettings) {
        self.deadline_secs = settings.deadline_secs;
        self.max_iters = settings.max_iters;
        self.token_budget = settings.token_budget;
        self.podrace = settings.podrace;
    }

    /// Stop handoff RL mode (does not wipe the victory board or budget counters).
    pub fn stop(&mut self) -> String {
        self.active = false;
        self.pending_submit = false;
        self.last_stop_reason = Some("stopped by user".to_string());
        "handoff RL stopped".to_string()
    }

    pub(crate) fn stop_for_guard(&mut self, reason: &str) -> String {
        self.active = false;
        self.pending_submit = false;
        self.last_stop_reason = Some(reason.to_string());
        format!("handoff RL paused: inner turn stopped ({reason}); explicit restart required")
    }

    /// Pause/stop because a budget was hit.
    pub fn stop_for_budget(&mut self, why: &str) -> String {
        self.active = false;
        self.pending_submit = false;
        self.last_stop_reason = Some(why.to_string());
        format!(
            "handoff RL operator cap reached: {why} — raise the cap (`/handoff-rl max N`, \
             `/handoff-rl endless`) or `/handoff-rl start` again"
        )
    }

    /// Which budget (if any) is exhausted — same contract as the agent loop.
    pub fn budget_tripped(&self) -> Option<String> {
        if self.max_iters > 0 && self.handoff_count >= self.max_iters {
            return Some(format!("max rolls (/handoff-rl max {})", self.max_iters));
        }
        if self.token_budget > 0 && self.tokens_spent >= self.token_budget {
            return Some(format!(
                "token budget (/handoff-rl tokens {})",
                self.token_budget
            ));
        }
        if self.deadline_secs > 0 && self.started_ms > 0 {
            let elapsed = now_ms().saturating_sub(self.started_ms) / 1000;
            if elapsed >= self.deadline_secs {
                return Some(format!(
                    "deadline (/handoff-rl duration {}s)",
                    self.deadline_secs
                ));
            }
        }
        None
    }

    /// True if another forced roll is allowed under current budgets.
    #[cfg(test)]
    pub fn can_force_roll(&self) -> Result<(), String> {
        if !self.active {
            return Err("handoff RL is not active".to_string());
        }
        if let Some(why) = self.budget_tripped() {
            return Err(why);
        }
        Ok(())
    }

    /// Record a victory on the board with candidate ID, score, and hypothesis.
    pub fn record_victory(&mut self, candidate_id: &str, score: f64, hypothesis: &str) -> String {
        let entry = VictoryEntry {
            candidate_id: candidate_id.trim().to_string(),
            score,
            hypothesis: hypothesis.trim().to_string(),
            timestamp: chrono_now_stamp(),
        };
        let is_new_best = match self.winning_score {
            Some(best) => score > best,
            None => true,
        };

        if is_new_best {
            self.winning_score = Some(score);
            self.winning_baseline = Some(entry.candidate_id.clone());
        }
        self.current_hypothesis = Some(entry.hypothesis.clone());
        self.victory_board.push(entry);
        // A scored victory is an explicit submission-result receipt.
        self.pending_submit = true;
        self.last_submit_note = Some(format!("victory {} @ {:.4}", candidate_id.trim(), score));

        let hyp_suffix = if hypothesis.is_empty() {
            String::new()
        } else {
            format!(" · hypothesis: {hypothesis}")
        };
        format!(
            "victory placed on board · candidate: {} (score: {:.4}){}{} · handoff demanded",
            candidate_id,
            score,
            if is_new_best {
                " [NEW WINNING BASELINE]"
            } else {
                ""
            },
            hyp_suffix
        )
    }

    /// True after an operator-recorded victory that has not yet been rolled.
    #[cfg(test)]
    pub fn victory_demands_handoff(&self) -> bool {
        self.active
            && self.pending_submit
            && self
                .last_submit_note
                .as_deref()
                .is_some_and(|n| n.starts_with("victory "))
    }

    /// Observe one completed turn's tool receipts. Returns a demand when a
    /// successful submit has been seen (this turn or earlier) **and** a
    /// score/status/result receipt lands. Prose alone never triggers.
    pub fn observe_turn(
        &mut self,
        outcome_actions: &[String],
        _reply: &str,
    ) -> Option<HandoffDemand> {
        if !self.active {
            return None;
        }
        if self.budget_tripped().is_some() {
            return None;
        }

        let mut submit_bits = Vec::new();
        let mut result_bits = Vec::new();
        for action in outcome_actions {
            if is_submit_action(action) {
                submit_bits.push(action.clone());
            } else if is_result_action(action) {
                result_bits.push(action.clone());
            }
        }

        if !submit_bits.is_empty() {
            self.pending_submit = true;
            self.last_submit_note = Some(submit_bits.join("; "));
        }

        if self.pending_submit && !result_bits.is_empty() {
            let summary = format!(
                "submit: {} · result: {}",
                self.last_submit_note.as_deref().unwrap_or("pending"),
                result_bits.join("; ")
            );
            return Some(HandoffDemand { summary });
        }

        None
    }

    /// Charge rough token usage (same ~chars/4 estimate as the agent loop).
    pub fn charge_tokens(&mut self, text: &str) {
        self.tokens_spent = self.tokens_spent.saturating_add(est_tokens(text));
    }

    /// Status text for `/handoff-rl status`.
    pub fn status_text(&self) -> String {
        let status_str = if self.active { "ACTIVE" } else { "INACTIVE" };
        let winning_base = self.winning_baseline.as_deref().unwrap_or("none");
        let winning_sc = self
            .winning_score
            .map(|s| format!("{s:.4}"))
            .unwrap_or_else(|| "none".to_string());
        let hyp = self.current_hypothesis.as_deref().unwrap_or("none");
        let victories_cnt = self.victory_board.len();
        let pending = if self.pending_submit {
            format!(
                "YES · {}",
                self.last_submit_note.as_deref().unwrap_or("unlabeled")
            )
        } else {
            "no".to_string()
        };
        let rolls_budget = if self.max_iters == 0 {
            "∞".to_string()
        } else {
            self.max_iters.to_string()
        };
        let tok_budget = if self.token_budget == 0 {
            "∞".to_string()
        } else {
            self.token_budget.to_string()
        };
        let deadline = if self.deadline_secs == 0 {
            "∞".to_string()
        } else {
            format!("{}s", self.deadline_secs)
        };
        let elapsed = if self.started_ms > 0 {
            format!("{}s", now_ms().saturating_sub(self.started_ms) / 1000)
        } else {
            "—".to_string()
        };
        let profile = if self.podrace { "podrace" } else { "standard" };
        let task = if self.task.trim().is_empty() {
            "(none)"
        } else {
            self.task.trim()
        };

        let mut lines = Vec::new();
        lines.push(format!("handoff RL status: {status_str} · {profile}"));
        lines.push(format!("  task: {task}"));
        lines.push(format!("  starter: {}", HANDOFF_RL_STARTER));
        lines.push(format!(
            "  rolls: {} / {rolls_budget} · tokens ~{} / {tok_budget} · elapsed {elapsed} / {deadline}",
            self.handoff_count, self.tokens_spent
        ));
        if self.interval_secs > 0 {
            lines.push(format!("  interval hint: {}s", self.interval_secs));
        }
        lines.push(format!(
            "  pending submit (awaiting result→demand): {pending}"
        ));
        lines.push(format!(
            "  winning baseline: {winning_base} (score: {winning_sc})"
        ));
        lines.push(format!("  current hot-path hypothesis: {hyp}"));
        lines.push(format!("  victory board entries: {victories_cnt}"));
        if let Some(why) = &self.last_stop_reason {
            lines.push(format!("  last stop: {why}"));
        }
        if let Some(why) = self.budget_tripped() {
            lines.push(format!("  budget: TRIPPED · {why}"));
        }

        if !self.victory_board.is_empty() {
            lines.push("  recent victories:".to_string());
            for (idx, v) in self.victory_board.iter().rev().take(5).enumerate() {
                lines.push(format!(
                    "    [{}] {} · score: {:.4} · hyp: {}",
                    idx + 1,
                    v.candidate_id,
                    v.score,
                    v.hypothesis
                ));
            }
        }

        lines.join("\n")
    }

    /// Build the forced prompt-injection payload. Increments the roll counter
    /// and clears the pending-submit latch (consume on build).
    pub fn build_forced_injection(&mut self, candidate_summary: Option<&str>) -> String {
        self.handoff_count += 1;
        self.pending_submit = false;

        let roll_num = self.handoff_count;
        let winning_base = self.winning_baseline.as_deref().unwrap_or("baseline-v0");
        let score_str = self
            .winning_score
            .map(|s| format!("{s:.4}"))
            .unwrap_or_else(|| "unscored".to_string());
        let hyp = self
            .current_hypothesis
            .as_deref()
            .unwrap_or("isolate hot-path bottlenecks and optimize execution speed/accuracy");

        let victories_summary = if self.victory_board.is_empty() {
            "no victories recorded yet".to_string()
        } else {
            format!("{} victories recorded on board", self.victory_board.len())
        };

        let summary_text = candidate_summary
            .map(|s| format!("\n  latest submission evidence: {}", s.trim()))
            .unwrap_or_default();

        let task_line = if self.task.trim().is_empty() {
            String::new()
        } else {
            format!("\n  Campaign task: {}", self.task.trim())
        };

        let budget_line =
            if self.max_iters == 0 && self.token_budget == 0 && self.deadline_secs == 0 {
                format!("\n  iteration {} · no cap", self.handoff_count)
            } else {
                format!(
                    "\n  Budget: rolls {}/{} · tokens ~{}/{} · deadline {}",
                    self.handoff_count,
                    if self.max_iters == 0 {
                        "∞".to_string()
                    } else {
                        self.max_iters.to_string()
                    },
                    self.tokens_spent,
                    if self.token_budget == 0 {
                        "∞".to_string()
                    } else {
                        self.token_budget.to_string()
                    },
                    if self.deadline_secs == 0 {
                        "∞".to_string()
                    } else {
                        format!("{}s", self.deadline_secs)
                    },
                )
            };

        // Host-enforced prompt injection: not a suggestion. The cockpit wiped
        // prior turns; this message is the sole user-facing restart payload.
        let note = format!(
            "{starter}\n\n\
            [FORCED HANDOFF — CONTEXT WIPED BY COCKPIT · roll #{roll}]\n\
            This is a host-enforced context restart (prompt injection procedure). \
            Prior conversation history has been erased. Do not renegotiate, summarize \
            the wipe, or ask whether to continue. Obey the sequence directive.\n\
            - Winning Baseline: {base} (score: {score})\n\
            - Board Status: {board}\n\
            - Current Hot-Path Hypothesis: {hyp}\
            {task_line}\
            {budget_line}\
            {summary}\n\n\
            [forced sequence directive]\n\
            {directive}\n\
            [/forced sequence directive]\n\n\
            BEGIN IMMEDIATELY. Compete. Place victories on the board. After the next \
            submission result is in, the cockpit will demand handoff again.\n\
            [/FORCED HANDOFF]",
            starter = HANDOFF_RL_STARTER,
            roll = roll_num,
            base = winning_base,
            score = score_str,
            board = victories_summary,
            hyp = hyp,
            task_line = task_line,
            budget_line = budget_line,
            summary = summary_text,
            directive = HANDOFF_RL_SEQUENCE_DIRECTIVE
        );

        self.charge_tokens(&note);
        self.last_handoff_note = Some(note.clone());
        note
    }
}

/// Successful submit fingerprint (outcome action or costly fingerprint text).
pub(crate) fn is_submit_action(action: &str) -> bool {
    let l = action.to_ascii_lowercase();
    l.contains("submit")
}

/// Score / status / board poll — a *result* receipt, not the submit itself.
pub(crate) fn is_result_action(action: &str) -> bool {
    let l = action.to_ascii_lowercase();
    if is_submit_action(&l) {
        return false;
    }
    l.contains("score")
        || l.contains("status")
        || l.contains("submissions")
        || l.contains("leaderboard")
        || l.contains("rank")
}

fn est_tokens(s: &str) -> usize {
    // Same rough proxy the agent loop uses: ~4 chars per token.
    s.len().div_ceil(4)
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn chrono_now_stamp() -> String {
    format!("{}", now_ms() / 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endless_command_clears_caps_and_rearms_cap_stopped_campaign() {
        let _lock = crate::tests::env_lock();
        let mut app = crate::seed_preview_app();
        app.handoff_rl.start(None);
        app.handoff_rl.max_iters = 2;
        app.handoff_rl.handoff_count = 2;
        app.handoff_rl.deadline_secs = 900;
        app.handoff_rl.token_budget = 123;
        let reason = app.handoff_rl.budget_tripped().unwrap();
        // L01 names the operator's own command in the notice: "max rolls (/handoff-rl max 2)".
        assert!(
            reason.starts_with("max rolls (") && reason.contains('2'),
            "{reason}"
        );
        app.handoff_rl.stop_for_budget(&reason);
        let reply = app.handoff_rl_command(Some("endless"));
        assert!(reply.contains("resumed"), "{reply}");
        assert!(app.handoff_rl.active);
        assert_eq!(
            (
                app.handoff_rl.max_iters,
                app.handoff_rl.deadline_secs,
                app.handoff_rl.token_budget
            ),
            (0, 0, 0)
        );
        assert!(app.handoff_rl.budget_tripped().is_none());
    }

    #[test]
    fn default_injection_has_no_budget_language() {
        let mut st = HandoffRlState::default();
        st.start(None);
        st.tokens_spent = usize::MAX;
        st.handoff_count = 100_000;
        assert!(st.budget_tripped().is_none());
        let prompt = st.build_forced_injection(None);
        assert!(prompt.contains("iteration 100001 · no cap"));
        assert!(!prompt.to_lowercase().contains("budget"));
        assert!(!prompt.to_lowercase().contains("deadline"));
    }

    #[test]
    fn l01_handoff_defaults_pass_old_limits_and_explicit_cap_names_value() {
        let mut state = HandoffRlState::default();
        state.apply_launch_settings(
            &crate::loop_dialog::LoopLaunchDialog::handoff_rl("fixture", 0).settings(),
        );
        state.handoff_count = 151;
        state.tokens_spent = 2_000_001;
        state.started_ms = now_ms().saturating_sub(3_601_000);
        assert_eq!(state.budget_tripped(), None);
        state.deadline_secs = 3600;
        assert!(
            state
                .budget_tripped()
                .unwrap()
                .contains("/handoff-rl duration 3600s")
        );
    }

    #[test]
    fn handoff_rl_initial_state() {
        let st = HandoffRlState::new();
        assert!(!st.active);
        assert_eq!(st.handoff_count, 0);
        assert!(!st.pending_submit);
        assert!(st.winning_baseline.is_none());
        assert!(st.winning_score.is_none());
        assert_eq!(st.max_iters, 0);
        assert_eq!(st.token_budget, 0);
        assert_eq!(st.deadline_secs, 0);
    }

    #[test]
    fn handoff_rl_start_stop() {
        let mut st = HandoffRlState::new();
        let res = st.start(Some("optimize search query throughput"));
        assert!(st.active);
        assert!(res.contains("hit it chewy"));
        assert_eq!(
            st.current_hypothesis.as_deref(),
            Some("optimize search query throughput")
        );

        let stop_res = st.stop();
        assert!(!st.active);
        assert!(!st.pending_submit);
        assert_eq!(stop_res, "handoff RL stopped");
    }

    #[test]
    fn apply_launch_settings_matches_loop_workshop() {
        let mut st = HandoffRlState::new();
        st.apply_launch_settings(&LoopLaunchSettings {
            deadline_secs: 3600,
            max_iters: 25,
            token_budget: 2_000_000,
            podrace: false,
        });
        assert_eq!(st.deadline_secs, 3600);
        assert_eq!(st.max_iters, 25);
        assert_eq!(st.token_budget, 2_000_000);
        assert!(!st.podrace);

        st.apply_launch_settings(&LoopLaunchSettings {
            deadline_secs: 5 * 24 * 3600,
            max_iters: 0,
            token_budget: 0,
            podrace: true,
        });
        assert!(st.podrace);
        assert_eq!(st.max_iters, 0);
        assert_eq!(st.token_budget, 0);
    }

    #[test]
    fn budget_tripped_on_max_rolls() {
        let mut st = HandoffRlState::new();
        st.start(None);
        st.max_iters = 2;
        st.build_forced_injection(None);
        assert!(st.budget_tripped().is_none());
        st.build_forced_injection(None);
        assert!(st.budget_tripped().unwrap().contains("max rolls"));
        assert!(st.can_force_roll().is_err());
    }

    #[test]
    fn handoff_rl_record_victories() {
        let mut st = HandoffRlState::new();
        st.start(None);

        let msg1 = st.record_victory("cand-1", 85.5, "inline hot path cache");
        assert!(msg1.contains("NEW WINNING BASELINE"));
        assert!(msg1.contains("handoff demanded"));
        assert_eq!(st.winning_baseline.as_deref(), Some("cand-1"));
        assert_eq!(st.winning_score, Some(85.5));
        assert!(st.pending_submit);
        assert!(st.victory_demands_handoff());

        let msg2 = st.record_victory("cand-2", 82.0, "slower variant");
        assert!(!msg2.contains("NEW WINNING BASELINE"));
        assert_eq!(st.winning_baseline.as_deref(), Some("cand-1"));

        let msg3 = st.record_victory("cand-3", 92.3, "vectorized loop");
        assert!(msg3.contains("NEW WINNING BASELINE"));
        assert_eq!(st.winning_baseline.as_deref(), Some("cand-3"));
        assert_eq!(st.winning_score, Some(92.3));
        assert_eq!(st.victory_board.len(), 3);
    }

    #[test]
    fn observe_turn_requires_submit_then_result() {
        let mut st = HandoffRlState::new();
        st.start(None);

        let d1 = st.observe_turn(
            &["shell:popcorn submit --mode benchmark".into()],
            "submitted cand",
        );
        assert!(d1.is_none());
        assert!(st.pending_submit);

        let d2 = st.observe_turn(&[], "SCORE: 12.3 looking good");
        assert!(d2.is_none());
        assert!(st.pending_submit);

        let d3 = st.observe_turn(
            &["outcome:shell:hilbert submissions --all".into()],
            "polled board",
        );
        assert!(d3.is_some());
        let demand = d3.unwrap();
        assert!(demand.summary.contains("submit:"));
        assert!(demand.summary.contains("result:"));
    }

    #[test]
    fn observe_turn_same_turn_submit_and_result() {
        let mut st = HandoffRlState::new();
        st.start(None);
        let d = st.observe_turn(
            &[
                "shell:hilbert submit".into(),
                "outcome:shell:hilbert status abc123".into(),
            ],
            "done",
        );
        assert!(d.is_some());
    }

    #[test]
    fn observe_turn_result_without_prior_submit_is_silent() {
        let mut st = HandoffRlState::new();
        st.start(None);
        let d = st.observe_turn(
            &["outcome:shell:hilbert submissions --all".into()],
            "just checking board",
        );
        assert!(d.is_none());
        assert!(!st.pending_submit);
    }

    #[test]
    fn forced_injection_starts_with_hit_it_chewy_and_wipes_pending() {
        let mut st = HandoffRlState::new();
        st.start(Some("hypothesis A"));
        st.record_victory("cand-win", 99.1, "hypothesis A");
        assert!(st.pending_submit);

        let note = st.build_forced_injection(Some("promoted cand-win after 100 tests passed"));
        assert!(note.starts_with("hit it chewy"));
        assert!(note.contains("[FORCED HANDOFF — CONTEXT WIPED BY COCKPIT · roll #1]"));
        assert!(note.contains("cand-win"));
        assert!(note.contains("99.1000"));
        assert!(note.contains(HANDOFF_RL_SEQUENCE_DIRECTIVE));
        assert!(note.contains("prompt injection procedure"));
        assert_eq!(st.handoff_count, 1);
        assert!(!st.pending_submit);
        assert!(!st.victory_demands_handoff());
        assert!(st.tokens_spent > 0);
    }

    #[test]
    fn classify_submit_vs_result_actions() {
        assert!(is_submit_action("shell:popcorn submit --mode benchmark"));
        assert!(is_submit_action("shell:hilbert submit"));
        assert!(!is_result_action("shell:popcorn submit --mode benchmark"));
        assert!(is_result_action("outcome:shell:hilbert submissions --all"));
        assert!(is_result_action("outcome:shell:popcorn status"));
        assert!(is_result_action("shell:leaderboard check"));
        assert!(!is_submit_action("outcome:shell:hilbert status"));
    }

    /// The operator loop: force inject → (work) → submit → result → force inject…
    #[test]
    fn handoff_cycle_submit_result_then_force_ad_infinitum() {
        let mut st = HandoffRlState::new();
        st.start(Some("compete on board"));
        st.max_iters = 0; // endless
        st.deadline_secs = 0;
        st.token_budget = 0;

        for roll in 1..=8 {
            // First inject (or re-inject after result).
            let note = st.build_forced_injection(Some(&format!("roll-{roll} start")));
            assert!(
                note.starts_with("hit it chewy"),
                "roll {roll}: injection must start with hit it chewy"
            );
            assert!(note.contains("[FORCED HANDOFF"));
            assert_eq!(st.handoff_count, roll);
            assert!(!st.pending_submit);

            // Work happens; submit alone does not demand.
            assert!(
                st.observe_turn(&["shell:hilbert submit cand".into()], "submitted")
                    .is_none()
            );
            assert!(st.pending_submit);

            // Result lands → demand.
            let demand = st
                .observe_turn(
                    &["outcome:shell:hilbert status abc".into()],
                    "terminal score 1.0",
                )
                .expect("result after submit must demand handoff");
            assert!(demand.summary.contains("submit:"));
            assert!(demand.summary.contains("result:"));
            // Pending stays latched until the next build_forced_injection.
            assert!(st.pending_submit);
        }
        assert_eq!(st.handoff_count, 8);
        assert!(st.can_force_roll().is_ok());
    }
}
