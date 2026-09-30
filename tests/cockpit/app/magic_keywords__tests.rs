use super::*;

#[test]
fn detects_standalone_keywords() {
    let e = scan("please ultrathink about this, then orchestrate the fix");
    assert!(e.ultrathink && e.orchestrate && !e.workflowz);
    assert!(
        e.steer_note()
            .unwrap()
            .contains(&crate::agent::harness::book::r_relentless::ULTRATHINK.cells())
    );
}

#[test]
fn ignores_code_spans_and_fences() {
    let e = scan("use `ultrathink` carefully\n```\norchestrate\n```\nok");
    assert!(!e.ultrathink);
    assert!(!e.orchestrate);
}

#[test]
fn ignores_paths_and_urls() {
    let e = scan("see path/to/orchestrate.rs and https://example.com/ultrathink");
    assert!(!e.orchestrate);
    assert!(!e.ultrathink);
}

#[test]
fn case_insensitive_whole_word() {
    let e = scan("UltraThink now; Workflowz please");
    assert!(e.ultrathink && e.workflowz);
    // Partial match must not fire.
    let e2 = scan("ultrathinking is not enough");
    assert!(!e2.ultrathink);
}

#[test]
fn detects_handoff_rl_keyword() {
    let e = scan("run handoff-rl on candidate suite");
    assert!(e.handoff_rl);
    let note = e.steer_note().unwrap();
    let route = crate::agent::harness::book::r_relentless::HANDOFF_RL;
    assert_eq!(note, route.cells(), "the keyword rides as its route");
    assert!(route.sub().pages.join(" ").contains("hit it chewy"));
}
