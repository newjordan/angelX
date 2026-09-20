//! Shared atoms for over-time iteration drivers.
//!
//! These were lifted out of [`deli`](crate::deli) so two drivers can share one
//! implementation instead of duplicating it:
//! - the in-turn **deli** driver (`ANGEL_DRIVER=deli`), which runs a bounded loop
//!   inside a single turn, and
//! - the cross-turn **loop controller** ([`loop_ctl`](crate::loop_ctl)), which
//!   drives successive full agentic turns toward a goal.
//!
//! The protocol they share: each iteration sees only *curated state* (the problem,
//! the accumulated findings, the directions already tried) — never the raw history,
//! because context accumulation is the documented cause of cognitive loops. An
//! iteration must break new ground; findings are deduped (a paraphrase doesn't
//! count as new); and once the loop stalls, a *structural* pivot is forced instead
//! of tuning the same approach harder.

/// System prompt for one iteration of a long-horizon loop: it sees only curated
/// state, so it must break new ground rather than restate what's known.
pub(crate) const WORKER_SYS: &str = "You are a single iteration of a long-horizon autonomous work \
    loop. You see only curated state — the problem, the findings so far, and the directions \
    already tried — not the full history, so treat the findings list as the complete record. \
    Your one job this iteration is to BREAK NEW GROUND: open an angle the prior directions \
    missed and produce concrete, verifiable findings. A claim is not progress merely because it \
    is new: every factual finding must cite evidence you actually have, using one of the citation \
    kinds the output contract below offers you and no others. If you cannot honestly cite a claim, \
    label it a hypothesis — that is the correct move, not a failure. Never invent a citation to \
    satisfy the format: an unsupported finding is worse than an admitted unknown. Be terse and \
    specific; this is raw material a later step will synthesize, not a finished answer.";

/// Folded into the iteration prompt once the loop has stalled: change the frame,
/// not the parameters.
pub(crate) const PIVOT_NOTE: &str = "PIVOT — the recent directions stalled. Do not tune the same \
    approach harder. Change a STRUCTURAL constraint of the approach: a different mechanism, \
    decomposition, or measurement — a genuinely different frame, not a parameter tweak. Finish the \
    experiment already in flight and record its measurement first; the goal itself never changes.";

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
        "none yet".to_string()
    } else {
        numbered(findings)
    };
    let open_leads = if hypotheses.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nOpen leads ({n}) — raised but NOT yet evidenced. Treat these as unverified: \
             confirming or killing one with concrete evidence counts as breaking new \
             ground.\n{list}",
            n = hypotheses.len(),
            list = numbered(hypotheses)
        )
    };
    let tried = if directions.is_empty() {
        "none yet".to_string()
    } else {
        directions
            .iter()
            .enumerate()
            .map(|(i, d)| format!("{}. {d}", i + 1))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let pivot_note = if pivot {
        format!("\n\n{PIVOT_NOTE}")
    } else {
        String::new()
    };
    let contract = match regime {
        EvidenceRegime::Grounded => {
            "Output exactly this shape and nothing else:\n\
             DIRECTION: <one short line naming the angle>\nFINDINGS:\n\
             - <factual claim> [evidence: file:<path>:<line>]\n\
             - <measured claim> [evidence: benchmark:<artifact path>]\n\
             HYPOTHESES:\n- <untested idea, if any>\n\nOnly FINDINGS with a concrete, checkable \
             evidence tag are admitted as progress. Cite only sources you actually opened, ran, or \
             fetched this iteration, and never one that does not directly support the claim."
        }
        // A text-only worker has no repository and no benchmark harness. Asking
        // it for `file:`/`benchmark:` citations reliably produces invented ones,
        // so the contract offers only what it can honestly supply and names the
        // fabrication failure mode explicitly rather than trusting it to infer
        // the boundary.
        EvidenceRegime::Reasoning => {
            "You are reasoning only. You have NO repository, NO filesystem, NO shell, and NO \
             network this iteration: you cannot open a file, run a command or benchmark, or fetch \
             a URL. Therefore you must NOT emit file:, benchmark:, test:, command:, tool: or url: \
             citations — a citation of that kind would be fabricated, and a fabricated citation is \
             worse than no finding at all. It will be rejected and counted against progress.\n\n\
             Any claim about specific code, concrete file contents, or a measured number is a \
             HYPOTHESIS here, not a finding — no matter how confident you are.\n\n\
             Output exactly this shape and nothing else:\n\
             DIRECTION: <one short line naming the angle>\nFINDINGS:\n\
             - <claim that follows from the problem statement as given> [evidence: premise:<the \
             part of the problem it rests on>]\n\
             - <claim that follows logically from a premise or an earlier finding> \
             [evidence: derivation:<the step>]\n\
             HYPOTHESES:\n\
             - <anything needing code, measurement, or an external source to settle>\n\n\
             Only FINDINGS carrying a premise: or derivation: tag are admitted as progress. \
             Putting a real uncertainty under HYPOTHESES costs you nothing and is the correct \
             move; dressing one up as a finding is the one thing that fails."
        }
    };
    format!(
        "Problem:\n{problem}\n\nFindings so far ({n}):\n{findings_txt}{open_leads}\n\nDirections \
         already tried:\n{tried}\n\nTake a NEW direction, materially distinct from every one already \
         tried, and surface concrete, verifiable findings the directions above missed. Do not \
         restate known findings.{pivot_note}\n\n{contract}",
        n = findings.len()
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
/// [`loop_ctl`](crate::loop_ctl)); a reasoning-only driver has nothing to
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
mod tests {
    use super::*;

    #[test]
    fn parse_iteration_extracts_direction_and_findings() {
        let out = "DIRECTION: attack the cache layer\nFINDINGS:\n- the TTL is unbounded\n\
                   * a second key collides\n2) eviction never fires";
        let (dir, found, _) = parse_iteration_sections(out);
        assert_eq!(dir, "attack the cache layer");
        assert_eq!(
            found,
            vec![
                "the TTL is unbounded".to_string(),
                "a second key collides".to_string(),
                "eviction never fires".to_string(),
            ]
        );
    }

    #[test]
    fn parse_iteration_sections_keeps_hypotheses_out_of_findings() {
        let out = "DIRECTION: measure it\nFINDINGS:\n- measured 3 ms [evidence: benchmark:/tmp/a.txt]\n\
                   HYPOTHESES:\n- fusion may help";
        let (dir, findings, hypotheses) = parse_iteration_sections(out);
        assert_eq!(dir, "measure it");
        assert_eq!(
            findings,
            vec!["measured 3 ms [evidence: benchmark:/tmp/a.txt]"]
        );
        assert_eq!(hypotheses, vec!["fusion may help"]);
    }

    #[test]
    fn normalize_collapses_cosmetic_differences() {
        assert_eq!(
            normalize("  The  TTL is Unbounded. "),
            normalize("the ttl is unbounded")
        );
    }

    #[test]
    fn bullet_recognizes_every_marker_and_rejects_non_bullets() {
        // Dash, star, and the unicode bullet all yield their trimmed content.
        assert_eq!(bullet("- dash item").as_deref(), Some("dash item"));
        assert_eq!(bullet("* star item").as_deref(), Some("star item"));
        assert_eq!(bullet("• unicode item").as_deref(), Some("unicode item"));
        // Numbered with `.` or `)` separators.
        assert_eq!(bullet("1. first").as_deref(), Some("first"));
        assert_eq!(bullet("42) forty-two").as_deref(), Some("forty-two"));
        // An empty bullet (marker but no content) is not a finding.
        assert_eq!(bullet("- "), None);
        assert_eq!(bullet("3. "), None);
        // A bare number with no separator, and plain prose, are not bullets.
        assert_eq!(bullet("3 apples"), None);
        assert_eq!(bullet("just a sentence"), None);
        // A dash with no trailing space is not a bullet marker.
        assert_eq!(bullet("-no-space"), None);
    }

    #[test]
    fn parse_iteration_is_case_insensitive_on_direction_and_lenient_on_findings() {
        // Lowercase `direction:` header still parses (strip_prefix_ci), and a
        // FINDINGS header line is never itself captured as a finding.
        let out = "direction: lower header\nFINDINGS:\n1. one\n2) two";
        let (dir, found, _) = parse_iteration_sections(out);
        assert_eq!(dir, "lower header");
        assert_eq!(found, vec!["one".to_string(), "two".to_string()]);
        // No DIRECTION line at all → empty direction, findings still collected.
        let (dir2, found2, _) = parse_iteration_sections("- only a finding");
        assert_eq!(dir2, "");
        assert_eq!(found2, vec!["only a finding".to_string()]);
    }

    #[test]
    fn parse_iteration_does_not_panic_on_non_ascii_leading_text() {
        let out = "ធ្វើការ inspect workspace quickly for HTML5 game assets/code.DIRECTION: Existing project surface scan\n\
                   FINDINGS:\n- still parses bullets after unicode prose";
        let (dir, found, _) = parse_iteration_sections(out);
        assert_eq!(dir, "");
        assert_eq!(
            found,
            vec!["still parses bullets after unicode prose".to_string()]
        );
    }

    #[test]
    fn normalize_strips_punctuation_and_empties_punctuation_only() {
        // Surrounding punctuation is trimmed, case folded, and whitespace collapsed.
        assert_eq!(normalize("  ...Foo   Bar...  "), "foo bar");
        assert_eq!(normalize("**Bold**"), "bold");
        // A single punctuation-only token collapses to the empty key (no finding).
        // (trim_matches only strips the ends, so use a token with no inner space.)
        assert_eq!(normalize("!!!---"), "");
        assert_eq!(normalize("   "), "");
    }

    #[test]
    fn finding_claim_strips_the_evidence_tag_for_dedup() {
        assert_eq!(
            finding_claim("the TTL is unbounded [evidence: file:src/a.rs:3]"),
            "the TTL is unbounded"
        );
        // No tag → the whole trimmed line is the claim.
        assert_eq!(finding_claim("  bare claim  "), "bare claim");
        // The same claim with two different citations dedups to one key.
        assert_eq!(
            normalize(finding_claim("same claim [evidence: file:a.rs:1]")),
            normalize(finding_claim("Same claim. [evidence: file:b.rs:9]"))
        );
    }

    #[test]
    fn evidence_source_extracts_tag_contents_case_insensitively() {
        assert_eq!(
            evidence_source("c [evidence: file:src/a.rs:3]").unwrap(),
            "file:src/a.rs:3"
        );
        // The header match is case-insensitive but the returned source keeps
        // its original casing (paths are case-sensitive on Linux).
        assert_eq!(
            evidence_source("c [EVIDENCE: File:Src/A.rs]").unwrap(),
            "File:Src/A.rs"
        );
        assert_eq!(evidence_source("c [evidence: ]"), None, "empty tag");
        assert_eq!(evidence_source("no tag at all"), None);
        assert_eq!(evidence_source("c [evidence: unclosed"), None);
    }

    #[test]
    fn has_evidence_tag_admits_named_kinds_and_rejects_the_rest() {
        // Every enumerated kind with real content is admissible.
        for kind in GROUNDED_EVIDENCE_KINDS {
            let finding = format!("claim [evidence: {kind}some/real/source]");
            assert!(
                has_evidence_tag(&finding, EvidenceRegime::Grounded),
                "{kind} with content must be admissible"
            );
        }
        // Placeholders are not evidence.
        for placeholder in ["none", "N/A", "unknown", "TODO", "<source>"] {
            assert!(
                !has_evidence_tag(
                    &format!("claim [evidence: {placeholder}]"),
                    EvidenceRegime::Grounded
                ),
                "{placeholder} must not count as evidence"
            );
        }
        // A tag naming no recognized kind is not a checkable citation.
        assert!(!has_evidence_tag(
            "claim [evidence: I am fairly confident]",
            EvidenceRegime::Grounded
        ));
        // A recognized kind with nothing meaningful after it is not either.
        assert!(!has_evidence_tag(
            "claim [evidence: file:]",
            EvidenceRegime::Grounded
        ));
        assert!(!has_evidence_tag(
            "claim [evidence: file:ab]",
            EvidenceRegime::Grounded
        ));
        // A bare assertion with no tag at all.
        assert!(!has_evidence_tag(
            "the cache is slow",
            EvidenceRegime::Grounded
        ));
    }

    #[test]
    fn numbered_one_indexes_each_item() {
        assert_eq!(numbered(&[]), "");
        assert_eq!(
            numbered(&["a".to_string(), "b".to_string(), "c".to_string()]),
            "1. a\n2. b\n3. c"
        );
    }

    #[test]
    fn curated_prompt_says_none_yet_when_empty() {
        let p = curated_prompt("solve", &[], &[], &[], false, EvidenceRegime::Grounded);
        assert!(p.contains("Findings so far (0)"));
        // Both the findings and directions sections fall back to "none yet".
        assert_eq!(p.matches("none yet").count(), 2);
        assert!(p.contains("Problem:\nsolve"));
        // With no open leads the section is omitted entirely, not shown empty.
        assert!(!p.contains("Open leads"));
    }

    #[test]
    fn curated_prompt_shows_open_leads_as_explicitly_unverified() {
        let p = curated_prompt(
            "solve",
            &["settled [evidence: file:a.rs:1]".into()],
            &["the allocator may be hot".into()],
            &[],
            false,
            EvidenceRegime::Grounded,
        );
        assert!(p.contains("Open leads (1)"));
        assert!(p.contains("1. the allocator may be hot"));
        assert!(
            p.contains("NOT yet evidenced"),
            "a lead must never read as an established finding"
        );
        // Confirming a lead is framed as progress, so an iteration can spend
        // itself validating rather than only chasing untouched ground.
        assert!(p.contains("breaking new ground"));
    }

    #[test]
    fn curated_prompt_injects_pivot_when_flagged() {
        let no_pivot = curated_prompt(
            "solve it",
            &["f1".into()],
            &[],
            &["a".into()],
            false,
            EvidenceRegime::Grounded,
        );
        assert!(!no_pivot.contains("PIVOT"), "no pivot unless flagged");
        assert!(no_pivot.contains("Findings so far (1)"));
        assert!(no_pivot.contains("1. f1"));
        let pivot = curated_prompt("solve it", &[], &[], &[], true, EvidenceRegime::Grounded);
        assert!(
            pivot.contains("PIVOT"),
            "pivot reframe injected when flagged"
        );
        assert!(pivot.contains("none yet"));
    }
}
