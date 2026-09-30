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

// The worker's frame (`⠻⠁`) and the pivot option (`⠻⠙`) are ledger pages;
// see `book::er_loop`. The curated state's labels are the pages of `⠘⠋`.

/// Number a list `1. … 2. …` for a prompt.
pub(crate) fn numbered(items: &[String]) -> String {
    items
        .iter()
        .enumerate()
        .map(|(i, it)| format!("{}. {it}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The curated state injected into one iteration — the only thing it sees. With
/// `pivot` set, the structural-pivot reframe is appended. The output contract
/// (`DIRECTION:` / `FINDINGS:` bullets) is exactly what
/// [`parse_iteration_sections`] reads.
///
/// `hypotheses` are the open leads: claims a prior iteration raised but could not
/// evidence. They are shown separately from findings and explicitly marked
/// unverified, so an iteration can spend itself confirming or killing one rather
/// than re-deriving it — and so an unevidenced claim never reads as settled fact.
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
    // Both workers read the ledger — the grounded one through `read_file`, the
    // text-only one through the ledger reader — so both contracts are routes.
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

/// `⠘⠋⠑`: an empty findings or directions list.
fn none_yet() -> String {
    format!(
        "{}⠑",
        crate::agent::harness::book::d45_iteration::CURATED.cells()
    )
}

/// An iteration's directions are routes (`⠻`): the ground to break, the open
/// leads, the pivot and the regime's output contract are the ledger pages; the
/// problem, findings, leads and directions ride as data, each after its label's
/// page address (`⠘⠋`).
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

/// Parse an iteration's output into `(direction, findings, hypotheses)`.
///
/// The direction is the text after a `DIRECTION:` line; findings and hypotheses
/// are the bullet / numbered lines under their respective headers (a header line
/// isn't a bullet, so it's naturally skipped). Lenient: any bullet counts, so a
/// slightly off-format reply still yields its content rather than nothing.
///
/// Untested ideas stay separate so a later iteration can validate them without
/// their inflating the evidence ledger or resetting stall detection. Both
/// drivers read this; neither wants findings and hypotheses merged.
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

/// Strip a case-insensitive prefix, returning the remainder.
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

/// If `line` is a bullet (`-`, `*`, `•`) or a numbered item (`1.`, `1)`), return
/// its content; otherwise `None`.
fn bullet(line: &str) -> Option<String> {
    for marker in ["- ", "* ", "• "] {
        if let Some(rest) = line.strip_prefix(marker) {
            let rest = rest.trim();
            return (!rest.is_empty()).then(|| rest.to_string());
        }
    }
    // Numbered: leading digits then `.` or `)` then a space.
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

/// What kind of evidence an iteration is actually *able* to produce.
///
/// This is not a strictness dial — it is a statement of fact about the worker,
/// and getting it wrong manufactures fabrication. Measured across
/// nemotron-3-super-120b, gpt-oss-20b and ling-3.0-flash: when a text-only
/// worker is asked for `file:`/`benchmark:` citations it cannot obtain, all
/// three reason their way to inventing them rather than declining. One said the
/// quiet part outright — *"the system might not actually check the existence of
/// the file; it's just a format, so we can produce plausible evidence tags"* —
/// and another emitted a fabricated file path, a fabricated benchmark artifact,
/// and an invented latency measurement in the same reply.
///
/// So the regime must match the worker's real capability. Ask a grounded worker
/// for grounded evidence; ask a reasoning-only worker for reasoning-only
/// evidence, and treat a filesystem citation from it as the fabrication signal
/// it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvidenceRegime {
    /// The worker has tools: it can read files, run commands, fetch URLs. Its
    /// citations name artifacts that either resolve or don't.
    Grounded,
    /// Text-only deep-think: no filesystem, no execution, no network. It can
    /// honestly cite only the problem statement it was given and its own
    /// derivations from prior admitted findings.
    Reasoning,
}

/// Evidence kinds a **grounded** worker can cite: each names an artifact that
/// can be resolved and checked.
pub(crate) const GROUNDED_EVIDENCE_KINDS: [&str; 7] = [
    "file:",
    "artifact:",
    "benchmark:",
    "test:",
    "url:",
    "command:",
    "tool:",
];

/// Evidence kinds a **reasoning-only** worker can cite honestly. `premise:`
/// points at the problem statement it was handed; `derivation:` at a logical
/// step from a premise or an already-admitted finding. Nothing else is within
/// its reach.
pub(crate) const REASONING_EVIDENCE_KINDS: [&str; 2] = ["premise:", "derivation:"];

impl EvidenceRegime {
    /// The citation kinds admissible under this regime.
    pub(crate) fn kinds(self) -> &'static [&'static str] {
        match self {
            EvidenceRegime::Grounded => &GROUNDED_EVIDENCE_KINDS,
            EvidenceRegime::Reasoning => &REASONING_EVIDENCE_KINDS,
        }
    }

    /// Whether a citation of this kind is *impossible* for the worker to have
    /// obtained — i.e. affirmative evidence of fabrication rather than merely
    /// unrecognized. A reasoning-only worker citing `file:src/x.rs:42` did not
    /// read that file; it invented the reference to satisfy the format.
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

/// Sources that name no real evidence — a model filling in the tag shape without
/// actually citing anything.
const PLACEHOLDER_SOURCES: [&str; 5] = ["none", "n/a", "unknown", "todo", "<source>"];

/// The claim part of a finding, with any trailing `[evidence: …]` tag stripped.
/// Dedup keys off this so the same claim with two different citations collapses.
pub(crate) fn finding_claim(finding: &str) -> &str {
    let lower = finding.to_ascii_lowercase();
    lower
        .find("[evidence:")
        .map(|idx| finding[..idx].trim())
        .unwrap_or_else(|| finding.trim())
}

/// The contents of a finding's `[evidence: …]` tag, if it has one.
pub(crate) fn evidence_source(finding: &str) -> Option<&str> {
    let lower = finding.to_ascii_lowercase();
    let start = lower.find("[evidence:")? + "[evidence:".len();
    let end = lower[start..].find(']')? + start;
    let source = finding[start..end].trim();
    (!source.is_empty()).then_some(source)
}

/// Whether an evidence source is one of the do-nothing placeholders.
pub(crate) fn is_placeholder_evidence(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    PLACEHOLDER_SOURCES.iter().any(|p| lower == *p)
}

/// Whether a finding carries a citation admissible **under its regime**.
///
/// Tag-shape only: it confirms the worker named a source of a kind it could
/// actually have, not that the source says what the claim says. A grounded
/// driver layers a real resolution check on top (see
/// [`loop_ctl`](crate::drive::loop_ctl)); a reasoning-only driver has nothing to
/// resolve against and must not pretend otherwise.
///
/// Under [`EvidenceRegime::Reasoning`] a filesystem-style citation is not merely
/// unrecognized, it is rejected as fabricated — see [`EvidenceRegime`] for the
/// measurements that motivated that.
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

/// Normalize a finding for dedup: lowercased, whitespace-collapsed, trimmed of
/// surrounding punctuation. Two findings that differ only cosmetically collapse to
/// the same key, so a paraphrase doesn't count as "new".
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
