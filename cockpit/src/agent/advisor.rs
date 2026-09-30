//! Advisor — the lightest MoA: one reviewer model reads the primary answer and
//! annotates it with a concern or a hard blocker for the operator. Ported from
//! oh-my-pi's advisor role (github.com/can1357/oh-my-pi).
//!
//! ## Modes (`ANGEL_ADVISOR`)
//!
//! | Value | Behavior |
//! |---|---|
//! | unset / `0` / `off` / `false` | Off |
//! | any other value | Final-answer review (swarm + ordinary turns) |
//!
//! Failures are always swallowed: the advisor must never cost the answer.

/// The advisor's brief: `⠌⠊`, a connected seat's route (`book::st_connected`).
/// The reviewer is offered the ledger reader alone, so it can read it.
pub(crate) fn brief() -> String {
    crate::agent::harness::book::st_connected::ADVISOR.cells()
}

/// One connected review: the brief and the review prompt, on `club`, reading
/// the ledger of `workspace`.
pub(crate) fn review(
    club: &dyn crate::agent::club::Club,
    workspace: &std::path::Path,
    task: &str,
    answer: &str,
) -> Result<String, String> {
    let prompt = format!("{}\n\n{}", brief(), review_prompt(task, answer));
    crate::agent::harness::book::connect::respond(club, workspace, &prompt)
}

/// How aggressively the advisor watches the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdvisorMode {
    Off,
    /// Review only when a final answer is about to land.
    Final,
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
                } else {
                    // `1`, `true`, `final`, or any other truthy token. The old
                    // per-hop mode (`hops`) injected prose mid-turn; the harness
                    // now speaks only through the book, so it reviews finals.
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
        matches!(self, Self::Final)
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

/// True when the advisor is enabled.
#[allow(dead_code)]
pub(crate) fn enabled() -> bool {
    AdvisorMode::from_env().is_on()
}

/// True when final-answer reviews are enabled.
pub(crate) fn final_enabled() -> bool {
    AdvisorMode::from_env().watches_final()
}

/// Build the advisor review prompt from the task and the proposed answer: the
/// labels are the brief's pages (`⠌⠊⠋`, `⠌⠊⠛`, `⠌⠊⠓`), the task and the answer
/// ride beside them as data.
pub(crate) fn review_prompt(task: &str, answer: &str) -> String {
    use crate::agent::harness::book::{d3_roles::pages, st_connected::ADVISOR};
    format!(
        "{}\n{task}\n\n{}\n{answer}\n\n{}",
        pages(ADVISOR, [6]),
        pages(ADVISOR, [7]),
        pages(ADVISOR, [8])
    )
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

/// True when `answer` already carries an advisor annotation (avoid double review).
pub(crate) fn already_annotated(answer: &str) -> bool {
    answer.contains("advisor (blocker)") || answer.contains("💡 advisor:")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/advisor__tests.rs"]
mod tests;
