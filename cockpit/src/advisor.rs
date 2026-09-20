//! Advisor — the lightest MoA: one reviewer model reads the primary answer (or
//! a hop summary) and injects a concern or a hard blocker inline. Ported from
//! oh-my-pi's advisor role (github.com/can1357/oh-my-pi).
//!
//! ## Modes (`ANGEL_ADVISOR`)
//!
//! | Value | Behavior |
//! |---|---|
//! | unset / `0` / `off` / `false` | Off |
//! | `1` / `true` / `final` | Final-answer gate only (swarm + ordinary turns) |
//! | `hops` / `every` / `always` | Final gate **and** a short review after each tool hop |
//!
//! Failures are always swallowed: the advisor must never cost the answer.

/// System prompt for the final-answer advisor pass.
pub(crate) const ADVISOR_SYS: &str = "You are a terse senior reviewer. You are shown a TASK and a \
proposed ANSWER. Reply with exactly one verdict line and nothing else:\n\
- `CLEAR` if the answer is sound and complete.\n\
- `NOTE: <one sentence>` for a real but non-blocking concern.\n\
- `BLOCK: <one sentence>` if the answer is wrong, unsafe, or misses the task's core requirement.\n\
Judge substance, not style. Do not rewrite the answer; add nothing after the verdict line.";

/// System prompt for mid-turn hop reviews (tool-batch outcomes).
pub(crate) const ADVISOR_HOP_SYS: &str = "You are a terse senior reviewer watching an agent mid-task. \
You are shown a TASK and the latest TOOL HOP summary. Reply with exactly one verdict line:\n\
- `CLEAR` if the hop looks on track.\n\
- `NOTE: <one sentence>` for a real but non-blocking concern (missed check, risky edit, stale assumption).\n\
- `BLOCK: <one sentence>` if the hop is clearly wrong, destructive, or thrashing.\n\
Be sparse — most hops are CLEAR. Do not rewrite anything; one line only.";

/// How aggressively the advisor watches the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdvisorMode {
    Off,
    /// Review only when a final answer is about to land.
    Final,
    /// Final review plus a brief note after each tool hop.
    Hops,
}

impl AdvisorMode {
    pub(crate) fn from_env() -> Self {
        match std::env::var("ANGEL_ADVISOR") {
            Ok(v) => {
                let t = v.trim();
                if t.is_empty()
                    || t == "0"
                    || t.eq_ignore_ascii_case("false")
                    || t.eq_ignore_ascii_case("off")
                    || t.eq_ignore_ascii_case("no")
                {
                    Self::Off
                } else if t.eq_ignore_ascii_case("hops")
                    || t.eq_ignore_ascii_case("every")
                    || t.eq_ignore_ascii_case("always")
                    || t.eq_ignore_ascii_case("hop")
                {
                    Self::Hops
                } else {
                    // `1`, `true`, `final`, or any other truthy token → final only.
                    Self::Final
                }
            }
            Err(_) => Self::Off,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_on(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub(crate) fn watches_final(self) -> bool {
        matches!(self, Self::Final | Self::Hops)
    }

    pub(crate) fn watches_hops(self) -> bool {
        matches!(self, Self::Hops)
    }
}

pub(crate) struct AdvisorVerdict {
    pub(crate) blocker: Option<String>,
    pub(crate) note: Option<String>,
}

impl AdvisorVerdict {
    pub(crate) fn is_clear(&self) -> bool {
        self.blocker.is_none() && self.note.is_none()
    }
}

/// True when any advisor mode is enabled (final and/or hops).
#[allow(dead_code)]
pub(crate) fn enabled() -> bool {
    AdvisorMode::from_env().is_on()
}

/// True when mid-turn hop reviews are enabled.
pub(crate) fn hops_enabled() -> bool {
    AdvisorMode::from_env().watches_hops()
}

/// True when final-answer reviews are enabled.
pub(crate) fn final_enabled() -> bool {
    AdvisorMode::from_env().watches_final()
}

/// Build the advisor review prompt from the task and the proposed answer.
pub(crate) fn review_prompt(task: &str, answer: &str) -> String {
    format!("TASK:\n{task}\n\nANSWER:\n{answer}\n\nYour verdict line:")
}

/// Build a hop-review prompt from the task and a compact tool-hop summary.
pub(crate) fn hop_review_prompt(task: &str, hop_summary: &str) -> String {
    format!("TASK:\n{task}\n\nTOOL HOP:\n{hop_summary}\n\nYour verdict line:")
}

/// Rest after an ASCII-case-insensitive prefix, already trimmed. Allocation-free.
fn rest_after_ascii_prefix<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let n = prefix.len();
    if line.len() >= n && line.as_bytes()[..n].eq_ignore_ascii_case(prefix.as_bytes()) {
        Some(line[n..].trim())
    } else {
        None
    }
}

/// Parse the advisor's reply into a verdict: the first line starting (case-
/// insensitively) with `BLOCK:` or `NOTE:` wins; anything else (incl. `CLEAR`)
/// is clear.
pub(crate) fn parse_verdict(reply: &str) -> AdvisorVerdict {
    for line in reply.lines() {
        let t = line.trim();
        if let Some(msg) = rest_after_ascii_prefix(t, "BLOCK:")
            && !msg.is_empty()
        {
            return AdvisorVerdict {
                blocker: Some(msg.to_string()),
                note: None,
            };
        }
        if let Some(msg) = rest_after_ascii_prefix(t, "NOTE:")
            && !msg.is_empty()
        {
            return AdvisorVerdict {
                blocker: None,
                note: Some(msg.to_string()),
            };
        }
    }
    AdvisorVerdict {
        blocker: None,
        note: None,
    }
}

/// Prefer blocker over note. Shared by answer and hop annotation.
fn selected_verdict(verdict: &AdvisorVerdict) -> Option<(&str, bool)> {
    verdict
        .blocker
        .as_deref()
        .map(|b| (b, true))
        .or_else(|| verdict.note.as_deref().map(|n| (n, false)))
}

/// Render the verdict as an inline annotation to append to the answer, or `None`
/// when the advisor cleared it.
pub(crate) fn annotate(verdict: &AdvisorVerdict) -> Option<String> {
    match selected_verdict(verdict) {
        Some((b, true)) => Some(format!("\n\n> ⚠ advisor (blocker): {b}")),
        Some((n, false)) => Some(format!("\n\n> 💡 advisor: {n}")),
        None => None,
    }
}

/// Harness-message form for hop notes (injected mid-turn, not into the answer).
pub(crate) fn annotate_hop(verdict: &AdvisorVerdict) -> Option<String> {
    match selected_verdict(verdict) {
        Some((b, true)) => Some(format!("[advisor hop · blocker] {b}")),
        Some((n, false)) => Some(format!("[advisor hop · note] {n}")),
        None => None,
    }
}

/// True when `answer` already carries an advisor annotation (avoid double review).
pub(crate) fn already_annotated(answer: &str) -> bool {
    answer.contains("advisor (blocker)") || answer.contains("💡 advisor:")
}

/// Bound a hop summary so the advisor prompt stays cheap.
pub(crate) fn bound_hop_summary(raw: &str, max_chars: usize) -> String {
    let max = max_chars.max(200);
    let t = raw.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let mut out: String = t.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/advisor__tests.rs"]
mod tests;
