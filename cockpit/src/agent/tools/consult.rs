//! `consult_model` / `code_review` — agent tools for calling in other models,
//! expert consultations, peer reviews, and code help mid-turn.
//!
//! Configured remote models are available unless ANGEL_ALLOW_SOTA_CONSULT=0
//! or solo mode restricts consultation. Unknown model names fail explicitly;
//! they never select an unrelated provider.

use crate::agent::club::{ChatMsg, Club, ClubReply, ToolDef};
use crate::agent::harness::{HandleKind, Tool, maybe_offload_root_body};
use crate::agent::tools::solo::{deny_sota_if_blocked_except, solo_mode_active};
use serde_json::Value;
use std::sync::Arc;

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/consult__deli_tests.rs"]
mod deli_tests;

/// Resolve an operator-typed seat name against a roster: exact label first,
/// then a label or live-model-id substring. The one place this matching rule
/// lives — `consult_model` and the needs-pro escalation seat both go through it,
/// so a name that works for one works for the other.
pub(crate) fn find_in_roster(roster: &[Arc<dyn Club>], wanted: &str) -> Option<Arc<dyn Club>> {
    let lower = wanted.trim().to_ascii_lowercase();
    roster
        .iter()
        .find(|club| {
            club.label().eq_ignore_ascii_case(wanted.trim())
                || club.label().to_ascii_lowercase().contains(&lower)
                || club
                    .live_model_name()
                    .is_some_and(|m| m.to_ascii_lowercase().contains(&lower))
        })
        .map(Arc::clone)
}

/// Local fleet seats (turbo, spark, atlas, gemma, …) are optional. A turn must
/// not depend on any of them being up; only use one while `Club::is_available`.
pub(crate) fn is_optional_local_label(label: &str) -> bool {
    let label = label.trim();
    !label.is_empty()
        && !label.eq_ignore_ascii_case("practice")
        && !crate::agent::club::is_sota_label(label)
}

/// Live optional-local labels — used in skip receipts so the model can pick a
/// seat that is actually up instead of retrying the corpse.
pub(crate) fn live_optional_local_labels(roster: &[Arc<dyn Club>]) -> Vec<String> {
    roster
        .iter()
        .filter(|club| is_optional_local_label(club.label()) && club.is_available())
        .map(|club| club.label().to_string())
        .collect()
}

pub(crate) fn skip_down_local_message(tool: &str, wanted: &str, live: &[String]) -> String {
    let live = if live.is_empty() {
        "none".to_string()
    } else {
        live.join(", ")
    };
    format!(
        "[{tool} skipped] {wanted} is not reachable right now. Live local seats: {live}. \
         Do the work yourself; do not retry {wanted}."
    )
}

/// What to do with a resolved seat that may be a down local box.
pub(crate) enum LocalSeat {
    Use {
        club: Arc<dyn Club>,
        notice: Option<String>,
    },
    Skip {
        wanted: String,
    },
}

/// If `wanted` is an optional local and currently down, use another live local
/// or `self_club`. If nothing is up, skip — never fail the turn for a dark LAN
/// box.
pub(crate) fn take_local_seat(
    wanted: Arc<dyn Club>,
    roster: &[Arc<dyn Club>],
    self_club: Option<&Arc<dyn Club>>,
) -> LocalSeat {
    if !is_optional_local_label(wanted.label()) || wanted.is_available() {
        return LocalSeat::Use {
            club: wanted,
            notice: None,
        };
    }
    let from = wanted.label().to_string();
    if let Some(live) = roster.iter().find(|club| {
        is_optional_local_label(club.label())
            && !club.label().eq_ignore_ascii_case(&from)
            && club.is_available()
    }) {
        return LocalSeat::Use {
            club: Arc::clone(live),
            notice: Some(format!("{from} is not reachable; using {}", live.label())),
        };
    }
    if let Some(self_club) = self_club {
        return LocalSeat::Use {
            club: Arc::clone(self_club),
            notice: Some(format!(
                "{from} is not reachable; using self ({})",
                self_club.label()
            )),
        };
    }
    LocalSeat::Skip { wanted: from }
}

/// Extract display text from a [`ClubReply`].
fn reply_text(reply: ClubReply) -> String {
    match reply {
        ClubReply::Text(text) => text,
        ClubReply::Calls(calls) => format!("[requested {} tool call(s)]", calls.len()),
    }
}

/// Call another model or club (luna-on-high, deepseek-v4-flash, sota-swarm, etc.)
/// directly mid-turn for code help or second opinions.
pub(crate) struct ConsultModelTool {
    roster: Vec<Arc<dyn Club>>,
    fallback_club: Option<Arc<dyn Club>>,
}

impl ConsultModelTool {
    pub(crate) fn new(roster: Vec<Arc<dyn Club>>, fallback_club: Option<Arc<dyn Club>>) -> Self {
        Self {
            roster,
            fallback_club,
        }
    }

    fn find_in_roster(&self, wanted: &str) -> Option<Arc<dyn Club>> {
        find_in_roster(&self.roster, wanted)
    }

    /// Resolve a club plus an optional per-call reasoning effort (pinned only
    /// by the smart-escalation alias).
    fn resolve_club(&self, name: &str) -> Result<(Arc<dyn Club>, Option<String>), String> {
        let trimmed = name.trim();
        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("auto")
            || trimmed.eq_ignore_ascii_case("self")
        {
            // auto/self → in-hand only (never a random roster head).
            return self
                .fallback_club
                .clone()
                .map(|club| (club, None))
                .ok_or_else(|| "consult_model: no in-hand club for auto/self".to_string());
        }
        // "smart"/"sota" = the ONE designated escalation seat (Sol@max by
        // default, Kimi-K3@high alternative; TUI operators get the popup
        // choice). If the chosen seat is absent from this roster (e.g. no
        // ChatGPT OAuth token), the alternative is tried before erroring.
        if trimmed.eq_ignore_ascii_case("smart") || trimmed.eq_ignore_ascii_case("sota") {
            let seat = crate::agent::club::smart_seat();
            let (seat, club) = match self.find_in_roster(&seat.club) {
                Some(club) => (seat, club),
                None => {
                    let alt = crate::agent::club::smart_seat_alt();
                    let club = self.find_in_roster(&alt.club).ok_or_else(|| {
                        format!(
                            "consult_model: neither smart seat '{}' nor alternative '{}' is in \
                             the roster",
                            seat.club, alt.club
                        )
                    })?;
                    (alt, club)
                }
            };
            let self_label = self.fallback_club.as_ref().map(|c| c.label());
            deny_sota_if_blocked_except(club.label(), self_label)?;
            return Ok((club, Some(seat.effort)));
        }
        if let Some(club) = self.find_in_roster(trimmed) {
            let self_label = self.fallback_club.as_ref().map(|c| c.label());
            deny_sota_if_blocked_except(club.label(), self_label)?;
            return Ok((club, None));
        }
        // Fail closed — do NOT fall through to roster[0] (was openai/codex).
        Err(format!(
            "consult_model: unknown club '{trimmed}'. Available: self, auto, smart, {}",
            self.roster
                .iter()
                .map(|c| c.label())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

impl Tool for ConsultModelTool {
    fn name(&self) -> &str {
        "consult_model"
    }

    fn def(&self) -> ToolDef {
        let labels: Vec<&str> = self.roster.iter().map(|c| c.label()).collect();
        let policy = if solo_mode_active() {
            "SOLO MODE: only self/auto (in-hand). Paid SOTA/codex consults are blocked."
        } else if !crate::agent::tools::solo::allow_sota_consult() {
            "Remote consults are disabled by ANGEL_ALLOW_SOTA_CONSULT; set 1 to allow them."
        } else {
            "Configured remote models are available; their provider charges apply."
        };
        ToolDef {
            name: "consult_model".to_string(),
            description: format!(
                "Optional second opinion from another club mid-turn. {policy} \
                 Default is self/auto (your own seat). Local fleet seats \
                 (turbo/spark/atlas/…) are optional — only used while reachable; \
                 a down local is skipped or rerouted, never a failed turn. \
                 method=deli optionally runs fresh-context Deli deliberation and returns its \
                 findings to this turn; choose rounds when useful, then continue ordinary work. \
                 Deli reasons over the supplied material; use normal tools or RL campaigns \
                 to measure its proposals. Available: self, auto, {}.",
                labels.join(", ")
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "club": {
                        "type": "string",
                        "description": "club or model name (default auto=self). Prefer local fleet; paid SOTA needs operator allow."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "the task, code snippet, or question to consult on"
                    },
                    "system": {
                        "type": "string",
                        "description": "optional role instructions for the consulted model"
                    },
                    "method": {
                        "type": "string", "enum": ["direct", "deli"],
                        "description": "direct (default): one consultation; deli: iterative deliberation, then return to this task"
                    },
                    "rounds": {
                        "type": "integer", "minimum": 1,
                        "description": "Deli rounds for this call; defaults to the operator's ANGEL_DELI_ROUNDS (6)"
                    }
                },
                "required": ["prompt"],
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        let uncancelled = std::sync::atomic::AtomicBool::new(false);
        let cancel = cancel.unwrap_or(&uncancelled);
        if cancel.load(std::sync::atomic::Ordering::Acquire) {
            return Err("consult_model cancelled before dispatch".into());
        }
        let method = args["method"].as_str().unwrap_or("direct");
        if !matches!(method, "direct" | "deli") {
            return Err("consult_model method must be direct or deli".into());
        }
        let rounds = args
            .get("rounds")
            .map(|value| {
                if method != "deli" {
                    return Err("rounds applies to method=deli".to_string());
                }
                value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .filter(|value| *value > 0)
                    .ok_or_else(|| "rounds must be a positive integer".to_string())
            })
            .transpose()?;
        let prompt = args["prompt"].as_str().ok_or("missing 'prompt'")?.trim();
        if prompt.is_empty() {
            return Err("'prompt' cannot be empty".to_string());
        }
        let club_name = args.get("club").and_then(|v| v.as_str()).unwrap_or("auto");
        let (club, pinned_effort) = self.resolve_club(club_name)?;
        let (club, reroute) = match take_local_seat(club, &self.roster, self.fallback_club.as_ref())
        {
            LocalSeat::Use { club, notice } => (club, notice),
            LocalSeat::Skip { wanted } => {
                return Ok(skip_down_local_message(
                    "consult_model",
                    &wanted,
                    &live_optional_local_labels(&self.roster),
                ));
            }
        };
        let self_label = self.fallback_club.as_ref().map(|c| c.label());
        deny_sota_if_blocked_except(club.label(), self_label)?;

        let mut messages = Vec::new();
        if let Some(sys) = args
            .get("system")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            messages.push(ChatMsg::system(sys));
        }
        messages.push(ChatMsg::user(prompt));

        let club: Arc<dyn Club> = if method == "deli" {
            Arc::new(crate::drive::deli::DeliClub::for_consult(
                club,
                rounds,
                pinned_effort.clone(),
            ))
        } else {
            club
        };
        // A smart-escalation consult pins its effort per-call (Sol@max /
        // Kimi@high) without mutating the seat's route state.
        let reply = club
            .chat_streaming_with_effort(
                &messages,
                &[],
                pinned_effort.as_deref(),
                cancel,
                &mut |_| {},
            )
            .map_err(|e| format!("consult_model ({club_name}): {e}"))?;
        let text = reply_text(reply);

        let label = club.label();
        let mut header = match reroute {
            Some(note) => format!("[consult_model club={label}] {note}"),
            None => format!("[consult_model club={label}]"),
        };
        if method == "deli" {
            header.push_str(" [method=deli; proposals require actual checks]");
        }
        if let Some(receipt) = maybe_offload_root_body(
            &text,
            HandleKind::Subcall,
            "consult_model",
            &format!("consult_model|{label}"),
            4096,
        ) {
            Ok(format!("{header}\n{receipt}"))
        } else {
            Ok(format!("{header}\n{text}"))
        }
    }
}

/// Request an adversarial code review on a file, diff, or raw code snippet using
/// a cheap SOTA model or swarm panel.
pub(crate) struct CodeReviewTool {
    roster: Vec<Arc<dyn Club>>,
    fallback_club: Option<Arc<dyn Club>>,
}

impl CodeReviewTool {
    pub(crate) fn new(roster: Vec<Arc<dyn Club>>, fallback_club: Option<Arc<dyn Club>>) -> Self {
        Self {
            roster,
            fallback_club,
        }
    }

    fn resolve_club(&self, name: Option<&str>) -> Result<Arc<dyn Club>, String> {
        // Default: self (in-hand) — self-review / self-test, not silent SOTA.
        let target_name = name.unwrap_or("self");
        if target_name.eq_ignore_ascii_case("self")
            || target_name.eq_ignore_ascii_case("auto")
            || target_name.is_empty()
        {
            return self
                .fallback_club
                .clone()
                .ok_or_else(|| "code_review: no in-hand club for self".to_string());
        }
        let lower = target_name.to_ascii_lowercase();
        for club in &self.roster {
            if club.label().eq_ignore_ascii_case(target_name)
                || club.label().to_ascii_lowercase().contains(&lower)
            {
                let self_label = self.fallback_club.as_ref().map(|c| c.label());
                deny_sota_if_blocked_except(club.label(), self_label)?;
                return Ok(Arc::clone(club));
            }
        }
        Err(format!(
            "code_review: unknown club '{target_name}'. Use self or a listed local club."
        ))
    }
}

impl Tool for CodeReviewTool {
    fn name(&self) -> &str {
        "code_review"
    }

    fn def(&self) -> ToolDef {
        let policy = if solo_mode_active() {
            "SOLO: runs on self only."
        } else if !crate::agent::tools::solo::allow_sota_consult() {
            "Default self (in-hand). Remote reviews are disabled by ANGEL_ALLOW_SOTA_CONSULT."
        } else {
            "Default self (in-hand). Choose a configured model for an external review."
        };
        ToolDef {
            name: "code_review".to_string(),
            description: format!(
                "Adversarial review of code/diffs/files (correctness, safety, performance). {policy} \
                 Local fleet seats are optional and only used while reachable."
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "file path, git diff, or raw code snippet to review"
                    },
                    "focus": {
                        "type": "string",
                        "enum": ["correctness", "security", "performance", "logic", "general"],
                        "default": "general",
                        "description": "review focus area"
                    },
                    "club": {
                        "type": "string",
                        "description": "optional club (default: self / in-hand)"
                    }
                },
                "required": ["target"],
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let target = args["target"].as_str().ok_or("missing 'target'")?.trim();
        if target.is_empty() {
            return Err("'target' cannot be empty".to_string());
        }
        let focus = args
            .get("focus")
            .and_then(|v| v.as_str())
            .unwrap_or("general");
        let club = self.resolve_club(args.get("club").and_then(|v| v.as_str()))?;
        let (club, reroute) = match take_local_seat(club, &self.roster, self.fallback_club.as_ref())
        {
            LocalSeat::Use { club, notice } => (club, notice),
            LocalSeat::Skip { wanted } => {
                return Ok(skip_down_local_message(
                    "code_review",
                    &wanted,
                    &live_optional_local_labels(&self.roster),
                ));
            }
        };
        let self_label = self.fallback_club.as_ref().map(|c| c.label());
        deny_sota_if_blocked_except(club.label(), self_label)?;

        let code_body = if !target.contains('\n') && std::path::Path::new(target).is_file() {
            std::fs::read_to_string(target)
                .map(|content| format!("=== File: {target} ===\n{content}"))
                .unwrap_or_else(|_| target.to_string())
        } else {
            target.to_string()
        };

        let system_prompt = format!(
            "You are an expert adversarial code reviewer focusing on {focus}.\n\
             Review the provided code carefully and structure your response with:\n\
             1. **Summary & Verdict**: Clean / Issues Found / Critical Bugs\n\
             2. **Key Findings**: Specific lines, invariant breaks, logic bugs, or security flaws\n\
             3. **Actionable Fixes**: Precise replacement snippets or refactor advice."
        );

        let messages = vec![
            ChatMsg::system(system_prompt),
            ChatMsg::user(format!(
                "Please review the following code ({focus} focus):\n\n{code_body}"
            )),
        ];

        let cancel = std::sync::atomic::AtomicBool::new(false);
        let review = club
            .chat_streaming(&messages, &[], &cancel, &mut |_| {})
            .map_err(|e| format!("code_review: {e}"))?;
        let text = reply_text(review);

        let label = club.label();
        let header = match reroute {
            Some(note) => format!("[code_review focus={focus} club={label}] {note}"),
            None => format!("[code_review focus={focus} club={label}]"),
        };
        if let Some(receipt) = maybe_offload_root_body(
            &text,
            HandleKind::Subcall,
            "code_review",
            &format!("code_review|{focus}"),
            4096,
        ) {
            Ok(format!("{header}\n{receipt}"))
        } else {
            Ok(format!("{header}\n{text}"))
        }
    }
}

/// Leanstral is a send-to specialist, not a conversation seat: GLM/Sol/Grok
/// (or any in-hand driver) ship a lemma, formula, or kernel-check snippet here
/// instead of putting Leanstral on the MoA roster.
pub(crate) struct LeanstralTool {
    roster: Vec<Arc<dyn Club>>,
}

const LEANSTRAL_SYSTEM: &str = "You are Leanstral, a Lean/math specialist. \
The caller is sending a snippet or a precise ask — not the conversation. \
Check, complete, or kernel-style-reason about only what they sent. Be terse. \
Do not try to reconstruct the broader task.";

impl LeanstralTool {
    pub(crate) fn new(roster: Vec<Arc<dyn Club>>) -> Self {
        Self { roster }
    }
}

impl Tool for LeanstralTool {
    fn name(&self) -> &str {
        "leanstral"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "leanstral".to_string(),
            description: "Lean/math specialist on Spark. Send a lemma, tactic, formula, or \
                          kernel-check question. Keep the payload small — a snippet or a precise \
                          ask, not the full conversation. This is a tool, not a reader of the \
                          thread. Skip it if you can do the work yourself; use it when you need \
                          a Leanstral pass on concrete math/Lean."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "the lemma, formula, tactic, or precise math/Lean ask"
                    }
                },
                "required": ["prompt"],
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let prompt = args["prompt"].as_str().ok_or("missing 'prompt'")?.trim();
        if prompt.is_empty() {
            return Err("'prompt' cannot be empty".to_string());
        }
        let Some(club) = find_in_roster(&self.roster, "leanstral") else {
            return Ok(skip_down_local_message(
                "leanstral",
                "leanstral",
                &live_optional_local_labels(&self.roster),
            ));
        };
        // Do not substitute turbo/self when Leanstral is down — this is a
        // specialist send-to, not a generic consult.
        if !club.is_available() {
            return Ok(skip_down_local_message(
                "leanstral",
                club.label(),
                &live_optional_local_labels(&self.roster),
            ));
        }
        let messages = vec![ChatMsg::system(LEANSTRAL_SYSTEM), ChatMsg::user(prompt)];
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let reply = club
            .chat_streaming(&messages, &[], &cancel, &mut |_| {})
            .map_err(|e| format!("leanstral: {e}"))?;
        let text = reply_text(reply);
        let label = club.label();
        let header = format!("[leanstral club={label}]");
        if let Some(receipt) = maybe_offload_root_body(
            &text,
            HandleKind::Subcall,
            "leanstral",
            &format!("leanstral|{label}"),
            4096,
        ) {
            Ok(format!("{header}\n{receipt}"))
        } else {
            Ok(format!("{header}\n{text}"))
        }
    }
}

pub(crate) fn maybe_register_leanstral(
    r: &mut crate::agent::harness::ToolRegistry,
    roster: &[Arc<dyn Club>],
) {
    if find_in_roster(roster, "leanstral").is_none() {
        return;
    }
    r.register(Box::new(LeanstralTool::new(roster.to_vec())));
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/consult__tests.rs"]
mod tests;
