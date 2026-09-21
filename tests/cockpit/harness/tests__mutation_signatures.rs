//! Mutation edit signatures and core/peripheral path heuristics.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- mutation signatures suite ---

#[test]
fn mutation_edit_signatures_collapse_identical_payloads() {
    let a = ToolCall {
        id: "1".into(),
        name: "str_replace".into(),
        args: serde_json::json!({
            "path": "src/x.rs",
            "old": "return parse(\"%y\");",
            "new": "return parse(\"%y%Z\");",
        }),
    };
    let b = ToolCall {
        id: "2".into(),
        name: "str_replace".into(),
        args: serde_json::json!({
            "path": "src/x.rs",
            "old": "return parse(\"%y\");",
            "new": "return parse(\"%y%Z\");",
        }),
    };
    assert_eq!(mutation_edit_signatures(&a), mutation_edit_signatures(&b));
    assert_eq!(mutation_edit_signatures(&a).len(), 1);
}

#[test]
fn multi_edit_signatures_are_per_hunk() {
    let call = ToolCall {
        id: "m".into(),
        name: "multi_edit".into(),
        args: serde_json::json!({
            "path": "Crypto/src/X509Certificate.cpp",
            "edits": [
                {"old": "parse(\"%y\")", "new": "parse(\"%y%Z\")"},
                {"old": "parse(\"%Y\")", "new": "parse(\"%Y%Z\")"},
            ]
        }),
    };
    let sigs = mutation_edit_signatures(&call);
    assert_eq!(sigs.len(), 2);
    assert_ne!(sigs[0], sigs[1]);
}

#[test]
fn peripheral_and_core_path_heuristics() {
    assert!(is_peripheral_mutation_path("docs/book/src/foo/Makefile"));
    assert!(is_peripheral_mutation_path("testdata/project-v4/Makefile"));
    assert!(
        is_peripheral_mutation_path("DOCS/GUIDE.md"),
        "mixed-case DOCS/ still counts as peripheral"
    );
    assert!(
        is_peripheral_mutation_path("src\\FIXTURES\\out.bin"),
        "backslash /FIXTURES/ still counts as peripheral"
    );
    assert!(
        !is_peripheral_mutation_path("mydocs/foo.rs"),
        "mydocs is not docs/"
    );
    assert!(
        !is_peripheral_mutation_path("src/booking/foo.rs"),
        "/booking/ is not /book/"
    );
    assert!(!is_core_mutation_path("docs/book/src/foo/Makefile"));
    assert!(is_core_mutation_path("pkg/machinery/scaffold.go"));
    assert!(is_core_mutation_path("Crypto/src/X509Certificate.cpp"));
    assert!(
        is_core_mutation_path("SRC/kernel.cu"),
        "mixed-case SRC/ still counts as core"
    );
    assert!(
        is_core_mutation_path("Crypto\\SRC\\X509Certificate.cpp"),
        "backslash /SRC/ still counts as core"
    );
    assert!(
        !is_core_mutation_path("crates/foo.rs"),
        "root crates/ is not a mid-path /crates/ marker"
    );
    assert!(
        !is_core_mutation_path("scoring/foo.rs"),
        "scoring is not /src/"
    );
    assert!(looks_like_test_source_path("model_test.go"));
    assert!(looks_like_test_source_path("blp_test.go"));
    assert!(!looks_like_test_source_path("model/function.go"));
    assert!(
        looks_like_test_source_path("pkg/Foo_TEST.go"),
        "mixed-case _TEST.go still counts as invented-test"
    );
    assert!(
        looks_like_test_source_path("src/__TESTS__/bar.rs"),
        "mixed-case __TESTS__ still counts"
    );
    assert!(
        looks_like_test_source_path("src\\test\\kernel.cu"),
        "backslash /test/ still counts"
    );
    assert!(
        looks_like_test_source_path("TEST_helper.py"),
        "TEST_ prefix still counts"
    );
    assert!(
        !looks_like_test_source_path("src/testing/foo.rs"),
        "/testing/ is not /test/"
    );
    assert!(
        !looks_like_test_source_path("contest/foo.go"),
        "contest is not a test directory"
    );
}

#[test]
fn meta_note_path_matches_in_place() {
    assert!(is_meta_note_mutation_path("LIVING_HANDOFF.md"));
    assert!(
        is_meta_note_mutation_path("src\\LIVING-HANDOFF.md"),
        "backslash living-handoff still counts as board bookkeeping"
    );
    assert!(
        is_meta_note_mutation_path(".ANGELX\\NOTES\\session.txt"),
        "mixed-case .angelX/notes store still counts"
    );
    assert!(
        is_meta_note_mutation_path("workspace\\.angelX\\handoff\\tip.txt"),
        "backslash .angelX/handoff store still counts"
    );
    assert!(
        is_meta_note_mutation_path("NOTES.md"),
        "mixed-case notes.md basename still counts"
    );
    assert!(
        is_meta_note_mutation_path("src\\NOTES"),
        "backslash trailing /notes still counts"
    );
    assert!(
        is_meta_note_mutation_path("Notes"),
        "bare Notes still counts as the notes store"
    );
    for path in [
        ".angel/notes/session.txt",
        "prefix.angelX/notes/session.txt",
        ".angelX0/notes/session.txt",
        "workspace/.angelX/notes-extra/session.txt",
        "workspace/.angelX/handoff-extra/session.txt",
    ] {
        assert!(
            !is_meta_note_mutation_path(path),
            "{path}: store names require canonical component boundaries"
        );
    }
    assert!(!is_meta_note_mutation_path("src/living_handoff_parser.rs"));
    assert!(
        !is_meta_note_mutation_path("src\\footnotes.md"),
        "footnotes.md is not notes.md"
    );
    assert!(!is_meta_note_mutation_path("host-slot.json"));
    assert!(
        !is_meta_note_mutation_path("mydocs/foo.rs"),
        "mydocs is not the notes store"
    );
}
