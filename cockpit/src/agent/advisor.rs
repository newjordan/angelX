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

pub(crate) fn brief() -> String {
    crate::agent::harness::book::st_connected::ADVISOR.cells()
}

pub(crate) fn review(
    club: &dyn crate::agent::club::Club,
    workspace: &std::path::Path,
    task: &str,
    answer: &str,
) -> Result<String, String> {
    let prompt = format!("{}\n\n{}", brief(), review_prompt(task, answer));
    crate::agent::harness::book::connect::respond(club, workspace, &prompt)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdvisorMode {
    Off,
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
                    Self::Final
                }
            }
            Err(_) => Self::Off,
        }
    }

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

#[allow(dead_code)]
pub(crate) fn enabled() -> bool {
    AdvisorMode::from_env().is_on()
}

pub(crate) fn final_enabled() -> bool {
    AdvisorMode::from_env().watches_final()
}

pub(crate) fn review_prompt(task: &str, answer: &str) -> String {
    use crate::agent::harness::book::{d3_roles::pages, st_connected::ADVISOR};
    format!(
        "{}\n{task}\n\n{}\n{answer}\n\n{}",
        pages(ADVISOR, [6]),
        pages(ADVISOR, [7]),
        pages(ADVISOR, [8])
    )
}

fn rest_after_ascii_prefix<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let n = prefix.len();
    if line.len() >= n && line.as_bytes()[..n].eq_ignore_ascii_case(prefix.as_bytes()) {
        Some(line[n..].trim())
    } else {
        None
    }
}

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

fn selected_verdict(verdict: &AdvisorVerdict) -> Option<(&str, bool)> {
    verdict
        .blocker
        .as_deref()
        .map(|b| (b, true))
        .or_else(|| verdict.note.as_deref().map(|n| (n, false)))
}

pub(crate) fn annotate(verdict: &AdvisorVerdict) -> Option<String> {
    match selected_verdict(verdict) {
        Some((b, true)) => Some(format!("\n\n> ⚠ advisor (blocker): {b}")),
        Some((n, false)) => Some(format!("\n\n> 💡 advisor: {n}")),
        None => None,
    }
}

pub(crate) fn already_annotated(answer: &str) -> bool {
    answer.contains("advisor (blocker)") || answer.contains("💡 advisor:")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/advisor__tests.rs"]
mod tests;
