//! Parallel/serial tool batch scheduling and path-overlap policy.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.
//! Shared `tc` helper for batch fixtures lives here.

use super::*;

// --- schedule / path-overlap suite ---

#[test]
fn parallel_reads_still_parallelize() {
    let b = vec![
        tc("grep", serde_json::json!({"pattern":"x"})),
        tc("read_file", serde_json::json!({"path":"a.rs"})),
        tc("list_dir", serde_json::json!({"path":"."})),
    ];
    assert!(batch_parallelizable_with(&b, true));
    // Reads parallelize regardless of the write-extension flag.
    assert!(batch_parallelizable_with(&b, false));
}

#[test]
fn disjoint_writes_parallelize_unless_disabled() {
    let b = vec![
        tc(
            "write_file",
            serde_json::json!({"path":"a.rs","content":"1"}),
        ),
        tc("str_replace", serde_json::json!({"path":"b.rs"})),
        tc("multi_edit", serde_json::json!({"path":"dir/c.rs"})),
    ];
    assert!(
        batch_parallelizable_with(&b, true),
        "disjoint writes run together"
    );
    assert!(
        !batch_parallelizable_with(&b, false),
        "disabled → writes serialize"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_parent_aliases_serialize_while_known_disjoint_writes_stay_parallel() {
    use std::os::unix::fs::symlink;

    let root = scratch("footprint_parent_alias");
    std::fs::create_dir_all(root.join("real")).unwrap();
    std::fs::create_dir_all(root.join("other")).unwrap();
    symlink("real", root.join("alias")).unwrap();
    let boundary = WorkspaceBoundary::new(&root);

    let aliases = vec![
        tc(
            "write_file",
            serde_json::json!({"path":"real/x","content":"one"}),
        ),
        tc(
            "str_replace",
            serde_json::json!({"path":"alias/x","old":"one","new":"two"}),
        ),
    ];
    assert!(!batch_parallelizable_in_with(&boundary, &aliases, true));
    assert_eq!(
        batch_segments_in_with(&boundary, &aliases, true),
        vec![0..1, 1..2],
        "two aliases of one confined parent must form only serial segments"
    );

    let disjoint = vec![
        tc(
            "write_file",
            serde_json::json!({"path":"real/a","content":"one"}),
        ),
        tc(
            "write_file",
            serde_json::json!({"path":"other/b","content":"two"}),
        ),
    ];
    assert!(batch_parallelizable_in_with(&boundary, &disjoint, true));
    assert_eq!(
        batch_segments_in_with(&boundary, &disjoint, true),
        vec![0..2]
    );

    let unknown_parent = vec![
        tc("write_file", serde_json::json!({"path":"missing/a"})),
        tc("write_file", serde_json::json!({"path":"other/b"})),
    ];
    assert!(
        !batch_parallelizable_in_with(&boundary, &unknown_parent, true),
        "an unknown ancestor must serialize until confinement resolves it"
    );

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn overlapping_writes_serialize() {
    let b = vec![
        tc("write_file", serde_json::json!({"path":"a.rs"})),
        tc("str_replace", serde_json::json!({"path":"a.rs"})),
    ];
    assert!(!batch_parallelizable_with(&b, true));
}

#[test]
fn absolute_path_batches_fail_closed_to_serial_scheduling() {
    let writes = vec![
        tc("write_file", serde_json::json!({"path":"/workspace/a.rs"})),
        tc("str_replace", serde_json::json!({"path":"b.rs"})),
    ];
    assert!(
        !batch_parallelizable_with(&writes, true),
        "without the registry root, an absolute mutation must stay serial"
    );

    let read_write = vec![
        tc("read_file", serde_json::json!({"path":"/workspace/a.rs"})),
        tc("write_file", serde_json::json!({"path":"b.rs"})),
    ];
    assert!(
        !batch_parallelizable_with(&read_write, true),
        "an absolute read is broad for conflict purposes"
    );
}

#[test]
fn tool_repair_is_classified_mutating_not_parallel_safe() {
    // tool_repair mutates ~/.local/bin symlinks and global git config, so it must
    // NOT be read-only/parallel-safe — it forces the batch serial.
    assert!(!is_parallel_safe("tool_repair"));
    assert!(matches!(
        footprint("tool_repair", &serde_json::json!({}), true),
        Footprint::Effect
    ));
    let b = vec![
        tc("read_file", serde_json::json!({"path":"a.rs"})),
        tc("tool_repair", serde_json::json!({})),
    ];
    assert!(
        !batch_parallelizable_with(&b, true),
        "tool_repair must not run concurrently with other calls"
    );
}

#[test]
fn writes_to_same_file_via_dot_segments_serialize() {
    // `a/x` and `a/./x` lexically differ but resolve to the same file; the
    // footprint must collapse `.`/`..` (like safe_path) so they conflict and
    // do NOT run concurrently. Likewise `a/x` vs `a/b/../x`.
    let b = vec![
        tc(
            "write_file",
            serde_json::json!({"path":"a/x","content":"1"}),
        ),
        tc(
            "write_file",
            serde_json::json!({"path":"a/./x","content":"2"}),
        ),
    ];
    assert!(
        !batch_parallelizable_with(&b, true),
        "a/x and a/./x are the same file → must serialize"
    );
    let b2 = vec![
        tc(
            "write_file",
            serde_json::json!({"path":"a/x","content":"1"}),
        ),
        tc(
            "str_replace",
            serde_json::json!({"path":"a/b/../x","content":"2"}),
        ),
    ];
    assert!(
        !batch_parallelizable_with(&b2, true),
        "a/x and a/b/../x resolve equal → must serialize"
    );
}

#[test]
fn write_with_broad_read_serializes() {
    // grep scans the whole tree → unsafe to run during a write.
    let b = vec![
        tc("grep", serde_json::json!({"pattern":"x"})),
        tc("write_file", serde_json::json!({"path":"a.rs"})),
    ];
    assert!(!batch_parallelizable_with(&b, true));
}

/// Regression (found by scenario sweep): read_file on a BINARY file must
/// return a concise notice, not a wall of U+FFFD from from_utf8_lossy that
/// floods the model's context.

#[test]
fn read_file_vs_write_respects_path_overlap() {
    let ok = vec![
        tc("read_file", serde_json::json!({"path":"a.rs"})),
        tc("write_file", serde_json::json!({"path":"b.rs"})),
    ];
    assert!(batch_parallelizable_with(&ok, true));
    let bad = vec![
        tc("read_file", serde_json::json!({"path":"a.rs"})),
        tc("write_file", serde_json::json!({"path":"./a.rs"})),
    ];
    assert!(
        !batch_parallelizable_with(&bad, true),
        "same file (./ normalized) conflicts"
    );
}

#[test]
fn effectful_calls_force_serial() {
    let shell = vec![
        tc("read_file", serde_json::json!({"path":"a.rs"})),
        tc("shell", serde_json::json!({"cmd":"ls"})),
    ];
    assert!(!batch_parallelizable_with(&shell, true));
    // apply_patch can touch several files → treated as an effect, never parallel.
    let patch = vec![
        tc("write_file", serde_json::json!({"path":"a.rs"})),
        tc("apply_patch", serde_json::json!({"patch":"..."})),
    ];
    assert!(!batch_parallelizable_with(&patch, true));
}

#[test]
fn mixed_batches_segment_parallel_runs_around_effect_barriers() {
    let calls = vec![
        tc("read_file", serde_json::json!({"path":"a.rs"})),
        tc("read_file", serde_json::json!({"path":"b.rs"})),
        tc("shell", serde_json::json!({"cmd":"cargo metadata"})),
        tc("grep", serde_json::json!({"pattern":"App"})),
        tc("read_file", serde_json::json!({"path":"c.rs"})),
    ];
    assert_eq!(batch_segments_with(&calls, true), vec![0..2, 2..3, 3..5]);
}

#[test]
fn mixed_batch_segments_preserve_conflict_order_and_cover_every_call_once() {
    let calls = vec![
        tc("read_file", serde_json::json!({"path":"a.rs"})),
        tc(
            "write_file",
            serde_json::json!({"path":"a.rs","content":"next"}),
        ),
        tc("read_file", serde_json::json!({"path":"b.rs"})),
        tc(
            "write_file",
            serde_json::json!({"path":"c.rs","content":"next"}),
        ),
    ];
    let segments = batch_segments_with(&calls, true);
    assert_eq!(segments, vec![0..1, 1..4]);
    assert_eq!(
        segments.into_iter().flatten().collect::<Vec<_>>(),
        (0..calls.len()).collect::<Vec<_>>()
    );
}

#[test]
fn paths_overlap_is_component_wise() {
    assert!(paths_overlap(Path::new("a/b.rs"), Path::new("a/b.rs")));
    assert!(paths_overlap(Path::new("a"), Path::new("a/b.rs"))); // dir contains file
    assert!(!paths_overlap(Path::new("a/b.rs"), Path::new("a/bc.rs"))); // not a string prefix
    assert!(!paths_overlap(Path::new("a.rs"), Path::new("b.rs")));
}
