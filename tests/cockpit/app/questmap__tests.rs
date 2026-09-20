use super::*;

#[test]
fn classify_reads_openings() {
    assert_eq!(
        classify("But that ignores the condition"),
        StepKind::Rejection
    );
    assert_eq!(
        classify("Wait, actually the condition matters"),
        StepKind::Reversal
    );
    assert_eq!(classify("Actually, I was wrong"), StepKind::Reversal);
    assert_eq!(
        classify("Alternatively, use symmetry"),
        StepKind::Alternative
    );
    assert_eq!(classify("Hmm, is that right?"), StepKind::Doubt);
    assert_eq!(classify("So the answer is 13/27"), StepKind::Conclusion);
    assert_eq!(classify("Enumerate the cases."), StepKind::Step);
    // A "wait, but" is a reversal — most decisive class wins.
    assert_eq!(classify("Wait, but that's circular"), StepKind::Reversal);
}

#[test]
fn segments_keep_decimals_whole() {
    let (segs, truncated) = segments("The value is 0.5 exactly. Next step.\nAnd a line.");
    assert!(!truncated);
    assert_eq!(
        segs,
        vec!["The value is 0.5 exactly.", "Next step.", "And a line."]
    );
}

#[test]
fn sample_trace_charts_two_expeditions() {
    let tree = parse(SAMPLE_TRACE);
    let kids = tree.children();
    let leaves: Vec<usize> = (0..tree.nodes.len())
        .filter(|&i| kids[i].is_empty())
        .collect();
    assert_eq!(leaves.len(), 2, "one felled instinct + one victor");
    let felled = leaves
        .iter()
        .filter(|&&l| tree.nodes[l].fell.is_some())
        .count();
    assert_eq!(felled, 1);
    assert!(tree.resolved(), "the sample reaches 13/27");
}

#[test]
fn alternatives_fork_without_abandoning() {
    let trace = "Try the direct route. Compute the sum. But the sum diverges. \
                     Alternatively, use symmetry. The pairs cancel. So the answer is 0.";
    let tree = parse(trace);
    let kids = tree.children();
    let leaves: Vec<usize> = (0..tree.nodes.len())
        .filter(|&i| kids[i].is_empty())
        .collect();
    assert_eq!(leaves.len(), 2);
    assert!(tree.resolved());
    assert!(
        tree.nodes.iter().any(|n| n.kind == StepKind::Alternative),
        "the deliberate fork is recorded"
    );
}

#[test]
fn render_is_deterministic_and_clamped() {
    let a = render_trace("nex2-mini", SAMPLE_TRACE, None, 72);
    let b = render_trace("nex2-mini", SAMPLE_TRACE, None, 72);
    assert_eq!(a, b);
    assert!(a.lines().all(|l| l.chars().count() <= 72));
    assert!(a.contains("QUEST MAP"));
}

#[test]
fn theme_override_and_stability() {
    assert_eq!(theme_for("anything", Some("dungeon")).name, "dungeon");
    assert_eq!(theme_for("anything", Some("VOYAGE")).name, "voyage");
    let t1 = theme_for("turbo·nex2-mini", None).name;
    let t2 = theme_for("turbo·nex2-mini", None).name;
    assert_eq!(t1, t2, "theme is a stable hash of the label");
}

#[test]
fn blank_trace_degrades_gracefully() {
    assert!(render_trace("m", "   ", None, 72).contains("blank"));
    // A single sentence is a quest with one unresolved expedition.
    let one = render_trace("m", "Just one thought here.", None, 72);
    assert!(one.contains("1 expedition ·"));
}

#[test]
fn classify_explain_reports_the_marker() {
    // The class and the marker that fired, so a real trace can be eyeballed.
    assert_eq!(
        classify_explain("But that ignores the condition"),
        (StepKind::Rejection, Some("but "))
    );
    // Precedence: a "Wait, but…" fires a Reversal marker, not a Rejection one.
    let (kind, marker) = classify_explain("Wait, but that's circular");
    assert_eq!(kind, StepKind::Reversal);
    assert_eq!(marker, Some("wait"));
    // A plain step matches nothing.
    assert_eq!(
        classify_explain("Enumerate the cases."),
        (StepKind::Step, None)
    );
    // `classify` is exactly the class half of `classify_explain`.
    for seg in [
        "Hmm, is that right?",
        "So the answer is 3",
        "Alternatively, x",
    ] {
        assert_eq!(classify(seg), classify_explain(seg).0);
    }
}

#[test]
fn classify_handles_markdown_and_answer_prefixes_from_real_traces() {
    // Real deepseek lines M0 misclassified as plain steps: markdown emphasis
    // and list bullets defeated the marker match, and "Answer:" wasn't a
    // conclusion marker at all. (M2 tuning against captured traces.)
    assert_eq!(classify("**Answer: 258 rolls.**"), StepKind::Conclusion);
    assert_eq!(classify("Answer: 2/3."), StepKind::Conclusion);
    // A bulleted rejection classifies once the "- " bullet is stripped.
    assert_eq!(
        classify("- But that double-counts the diagonal"),
        StepKind::Rejection
    );
    // Stripping is classification-only: a bulleted data line is still a step,
    // and decimals in the body are untouched.
    assert_eq!(
        classify("- Roll a 6 (prob 1/6): go to state 2"),
        StepKind::Step
    );
    assert_eq!(classify("The value is 0.5 here."), StepKind::Step);
}

#[test]
fn lexicon_view_annotates_and_tallies() {
    let view = render_lexicon("nex2-mini", SAMPLE_TRACE, 72);
    assert!(view.contains("⚙ LEXICON · nex2-mini"));
    // The felled instinct is annotated with the marker that classified it.
    assert!(view.contains("reject ← \"but \""));
    // The claimed answer is annotated too.
    assert!(view.contains("answer ← "));
    // Footer tallies the non-plain classes; every line honors the clamp.
    assert!(view.contains("steps ·"));
    assert!(view.lines().all(|l| l.chars().count() <= 72));
    // Degrades on an empty trace.
    assert!(render_lexicon("m", "   ", 72).contains("nothing to read"));
}

#[test]
fn party_chronicles_every_label() {
    let traces = vec![
        ("gemma4".to_string(), SAMPLE_TRACE.to_string()),
        ("qwen3".to_string(), SAMPLE_TRACE.to_string()),
    ];
    let map = render_party(&traces, 72);
    assert!(map.contains("THE GAUNTLET"));
    assert!(map.contains("gemma4"));
    assert!(map.contains("qwen3"));
}
