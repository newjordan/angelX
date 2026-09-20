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
