//! Shared atoms for over-time iteration drivers.
//!
//! These were lifted out of [`deli`](crate::drive::deli) so two drivers can share one
//! implementation instead of duplicating it:
//! - the in-turn **deli** driver (`ANGEL_DRIVER=deli`), which runs a bounded loop
//!   inside a single turn, and
//! - the cross-turn **loop controller** ([`loop_ctl`](crate::drive::loop_ctl)), which
//!   drives successive full agentic turns toward a goal.
//!
//! The protocol they share: each iteration sees only *curated state* (the problem,
//! the accumulated findings, the directions already tried) — never the raw history,
//! because context accumulation is the documented cause of cognitive loops. An
//! iteration must break new ground; findings are deduped (a paraphrase doesn't
//! count as new); and once the loop stalls, a *structural* pivot is forced instead
//! of tuning the same approach harder.

pub(crate) fn numbered(items: &[String]) -> String {
    items
        .iter()
        .enumerate()
        .map(|(i, it)| format!("{}. {it}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn curated_prompt(
    problem: &str,
    findings: &[String],
    hypotheses: &[String],
    directions: &[String],
    pivot: bool,
    regime: EvidenceRegime,
) -> String {
    let findings_txt = if findings.is_empty() {
        none_yet()
    } else {
        numbered(findings)
    };
    let contract = match regime {
        EvidenceRegime::Grounded => crate::agent::harness::book::er_loop::CONTRACT,
        EvidenceRegime::Reasoning => crate::agent::harness::book::st_connected::REASONING_CONTRACT,
    };
    routed_prompt(
        problem,
        &findings_txt,
        findings.len(),
        hypotheses,
        directions,
        pivot,
        contract,
    )
}

fn none_yet() -> String {
    format!(
        "{}⠑",
        crate::agent::harness::book::d45_iteration::CURATED.cells()
    )
}

fn routed_prompt(
    problem: &str,
    findings_txt: &str,
    n: usize,
    hypotheses: &[String],
    directions: &[String],
    pivot: bool,
    contract: crate::agent::harness::book::Route,
) -> String {
    use crate::agent::harness::book::{d45_iteration, er_loop, sign_lines};
    let curated = d45_iteration::CURATED.cells();
    let tried = if directions.is_empty() {
        none_yet()
    } else {
        numbered(directions)
    };
    let mut routes = vec![er_loop::GROUND];
    let open_leads = if hypotheses.is_empty() {
        String::new()
    } else {
        routes.push(er_loop::OPEN_LEADS);
        format!(
            "\n\n{curated}⠉ n={}\n{}",
            hypotheses.len(),
            numbered(hypotheses)
        )
    };
    if pivot {
        routes.push(er_loop::PIVOT);
    }
    routes.push(contract);
    format!(
        "{curated}⠁\n{problem}\n\n{curated}⠃ n={n}\n{findings_txt}{open_leads}\n\n{curated}⠙\n\
         {tried}\n\n{}",
        sign_lines(&routes)
    )
}

pub(crate) fn parse_iteration_sections(text: &str) -> (String, Vec<String>, Vec<String>) {
    #[derive(Clone, Copy)]
    enum Section {
        Findings,
        Hypotheses,
    }

    let mut direction = String::new();
    let mut findings = Vec::new();
    let mut hypotheses = Vec::new();
    let mut section = Section::Findings;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(rest) = strip_prefix_ci(line, "DIRECTION:") {
            direction = rest.trim().to_string();
            continue;
        }
        if line.eq_ignore_ascii_case("FINDINGS:") {
            section = Section::Findings;
            continue;
        }
        if line.eq_ignore_ascii_case("HYPOTHESES:") {
            section = Section::Hypotheses;
            continue;
        }
        if let Some(item) = bullet(line) {
            match section {
                Section::Findings => findings.push(item),
                Section::Hypotheses => hypotheses.push(item),
            }
        }
    }
    (direction, findings, hypotheses)
}

fn strip_prefix_ci<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let mut chars = line.char_indices();
    for expected in prefix.chars() {
        let (_, actual) = chars.next()?;
        if !actual.eq_ignore_ascii_case(&expected) {
            return None;
        }
    }

    let rest_start = chars.next().map(|(idx, _)| idx).unwrap_or(line.len());
    Some(&line[rest_start..])
}

fn bullet(line: &str) -> Option<String> {
    for marker in ["- ", "* ", "• "] {
        if let Some(rest) = line.strip_prefix(marker) {
            let rest = rest.trim();
            return (!rest.is_empty()).then(|| rest.to_string());
        }
    }
    let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let after = &line[digits.len()..];
        if let Some(rest) = after
            .strip_prefix(". ")
            .or_else(|| after.strip_prefix(") "))
        {
            let rest = rest.trim();
            return (!rest.is_empty()).then(|| rest.to_string());
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvidenceRegime {
    Grounded,
    Reasoning,
}

pub(crate) const GROUNDED_EVIDENCE_KINDS: [&str; 7] = [
    "file:",
    "artifact:",
    "benchmark:",
    "test:",
    "url:",
    "command:",
    "tool:",
];

pub(crate) const REASONING_EVIDENCE_KINDS: [&str; 2] = ["premise:", "derivation:"];

impl EvidenceRegime {
    pub(crate) fn kinds(self) -> &'static [&'static str] {
        match self {
            EvidenceRegime::Grounded => &GROUNDED_EVIDENCE_KINDS,
            EvidenceRegime::Reasoning => &REASONING_EVIDENCE_KINDS,
        }
    }

    pub(crate) fn is_fabricated_kind(self, source: &str) -> bool {
        match self {
            EvidenceRegime::Grounded => false,
            EvidenceRegime::Reasoning => {
                let lower = source.to_ascii_lowercase();
                GROUNDED_EVIDENCE_KINDS
                    .iter()
                    .any(|prefix| lower.starts_with(prefix))
            }
        }
    }
}

const PLACEHOLDER_SOURCES: [&str; 5] = ["none", "n/a", "unknown", "todo", "<source>"];

pub(crate) fn finding_claim(finding: &str) -> &str {
    let lower = finding.to_ascii_lowercase();
    lower
        .find("[evidence:")
        .map(|idx| finding[..idx].trim())
        .unwrap_or_else(|| finding.trim())
}

pub(crate) fn evidence_source(finding: &str) -> Option<&str> {
    let lower = finding.to_ascii_lowercase();
    let start = lower.find("[evidence:")? + "[evidence:".len();
    let end = lower[start..].find(']')? + start;
    let source = finding[start..end].trim();
    (!source.is_empty()).then_some(source)
}

pub(crate) fn is_placeholder_evidence(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    PLACEHOLDER_SOURCES.iter().any(|p| lower == *p)
}

pub(crate) fn has_evidence_tag(finding: &str, regime: EvidenceRegime) -> bool {
    let Some(source) = evidence_source(finding) else {
        return false;
    };
    if is_placeholder_evidence(source) || regime.is_fabricated_kind(source) {
        return false;
    }
    let lower = source.to_ascii_lowercase();
    regime
        .kinds()
        .iter()
        .any(|prefix| lower.starts_with(prefix) && source[prefix.len()..].trim().len() >= 3)
}

pub(crate) fn normalize(s: &str) -> String {
    s.to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/iterate__tests.rs"]
mod tests;
