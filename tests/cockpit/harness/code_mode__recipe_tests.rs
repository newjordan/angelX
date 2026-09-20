use super::*;

#[test]
fn code_mode_repo_recon_query_is_encoded_as_data() {
    let script =
        repo_recon_script("needle'); return shell({command:'touch escaped'}); // NeedleWidget")
            .unwrap();
    assert!(!script.contains("shell("));
    assert!(!script.contains("touch escaped"));
    assert!(script.contains("needlewidget"));
    assert!(script.len() < 16 * 1024);
}

#[test]
fn code_mode_repo_recon_terms_are_bounded_and_deterministic() {
    let query = "repair cockpit/src/agent/harness/turn.rs repeated read search telemetry telemetry";
    let first = repo_recon_terms(query);
    assert_eq!(first, repo_recon_terms(query));
    assert!(first.len() <= REPO_RECON_MAX_TERMS);
    assert!(first.contains(&"cockpit/src/agent/harness/turn.rs".to_string()));
}

#[test]
fn code_mode_repo_recon_drops_task_fluff_stopwords() {
    let terms = repo_recon_terms(
        "please help implement improve task only just something MutationThrashNudge",
    );
    for stop in [
        "please",
        "help",
        "implement",
        "improve",
        "task",
        "only",
        "just",
        "something",
    ] {
        assert!(
            !terms.iter().any(|t| t == stop),
            "stopword {stop} should not be a recon term, got {terms:?}"
        );
    }
    assert!(
        terms
            .iter()
            .any(|t| t.contains("mutation") || t.contains("thrash") || t.contains("nudge")),
        "identifier should survive fluff strip: {terms:?}"
    );
}

#[test]
fn code_mode_repo_recon_program_prefers_impl_paths() {
    let script = repo_recon_script("MutationThrashNudge thrash").unwrap();
    assert!(
        script.contains("implPathBonus"),
        "recon program should rank implementing libraries above docs/testdata"
    );
    assert!(script.contains("docs|testdata|fixtures"));
    assert!(script.contains("src|lib|pkg|core"));
}
