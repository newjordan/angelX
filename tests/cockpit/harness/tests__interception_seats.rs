//! T3 — interception: policy as context metadata, consulted at invocation.
//!
//! §3.2.3 (Def 26/27) and §6.3: the same registered tool is permitted in one
//! context and denied in another, the provider is untouched, and a retune binds
//! the very next call with no re-registration. Real tools write real files here,
//! so a denied write is visible as an unchanged file — not as a passing
//! assertion about a counter.

use super::*;
use crate::harness::interception::Interception;
use crate::harness::registry::{ROOT_SEAT, SeatGrant};

/// A registry carrying the real file tools, rooted in a scratch directory.
fn registry_with_files(tag: &str) -> (ToolRegistry, PathBuf) {
    let root = scratch(tag);
    let mut registry = ToolRegistry::new();
    crate::harness::register_file_tools(&mut registry, root.clone());
    (registry, root)
}

#[test]
fn the_root_keeps_write_access_a_seat_does_not() {
    let (registry, root) = registry_with_files("interception-seat");
    let target = root.join("note.txt");
    registry
        .dispatch(
            "write_file",
            &serde_json::json!({"path": target, "content": "root wrote\n"}),
        )
        .expect("the root context writes");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "root wrote\n");

    // The child seat is attenuated; nothing about the tool or its registration moved.
    registry.grant_seat(
        "child",
        Interception::deny_tools(["write_file", "str_replace"]),
    );
    let denied = registry
        .dispatch_with_cancel_in_seat(
            "child",
            "write_file",
            &serde_json::json!({"path": target, "content": "child wrote\n"}),
            None,
        )
        .expect_err("a read-only seat cannot write");
    assert!(denied.contains("deny:tool:write_file"), "{denied}");
    assert!(denied.contains("seat child"), "{denied}");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "root wrote\n",
        "the refused write left no trace on disk"
    );

    // The same seat may read the same path: policy is per invocation, not per tool.
    let read = registry
        .dispatch_with_cancel_in_seat(
            "child",
            "read_file",
            &serde_json::json!({"path": target}),
            None,
        )
        .expect("reading is untouched by a write refusal");
    assert!(read.contains("root wrote"), "{read}");

    // And the root keeps working while the child is attenuated.
    registry
        .dispatch(
            "write_file",
            &serde_json::json!({"path": root.join("second.txt"), "content": "still\n"}),
        )
        .expect("the root is unaffected by a seat's grant");
}

#[test]
fn a_denial_is_a_receipt_that_names_its_policy_and_its_seat() {
    let (registry, root) = registry_with_files("interception-receipt");
    assert!(registry.policy_denials().is_empty(), "nothing denied yet");
    registry.grant_seat("child", Interception::deny_tool("write_file"));
    let _ = registry.dispatch_with_cancel_in_seat(
        "child",
        "write_file",
        &serde_json::json!({"path": root.join("nope.txt"), "content": "x"}),
        None,
    );
    let receipts = registry.policy_denials();
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    assert_eq!(receipts[0].tool, "write_file");
    assert_eq!(receipts[0].policy, "deny:tool:write_file");
    assert_eq!(receipts[0].seat.as_deref(), Some("child"));
    assert!(!root.join("nope.txt").exists());
}

#[test]
fn retuning_a_grant_binds_the_next_call_with_no_re_registration() {
    let (registry, root) = registry_with_files("interception-retune");
    let advertised: Vec<String> = registry.defs().iter().map(|d| d.name.clone()).collect();

    registry.grant_seat("child", Interception::deny_tool("write_file"));
    let denied = registry
        .dispatch_with_cancel_in_seat(
            "child",
            "write_file",
            &serde_json::json!({"path": root.join("a.txt"), "content": "a"}),
            None,
        )
        .expect_err("attenuated");
    assert!(denied.contains("deny:tool:write_file"), "{denied}");

    // Retune in place — the write is permitted on the next call. No reload, no
    // re-registration: the advertised set is exactly what it was.
    registry.grant_seat("child", Interception::empty());
    registry
        .dispatch_with_cancel_in_seat(
            "child",
            "write_file",
            &serde_json::json!({"path": root.join("a.txt"), "content": "a"}),
            None,
        )
        .expect("the retuned grant binds the next call");
    assert_eq!(std::fs::read_to_string(root.join("a.txt")).unwrap(), "a");
    let after: Vec<String> = registry.defs().iter().map(|d| d.name.clone()).collect();
    assert_eq!(advertised, after, "a policy change re-registers nothing");
    assert_eq!(registry.tracked_registrations(), 0);
}

#[test]
fn an_ancestor_denial_cannot_be_widened_by_a_seat() {
    let (registry, root) = registry_with_files("interception-narrowing");
    registry.grant_seat(ROOT_SEAT, Interception::deny_tool("write_file"));
    registry.grant_seat("child", Interception::empty());

    let error = registry
        .dispatch(
            "write_file",
            &serde_json::json!({"path": root.join("root.txt"), "content": "x"}),
        )
        .expect_err("the root context has no write_file either");
    assert!(error.contains("deny:tool:write_file"), "{error}");

    let error = registry
        .dispatch_with_cancel_in_seat(
            "child",
            "write_file",
            &serde_json::json!({"path": root.join("child.txt"), "content": "x"}),
            None,
        )
        .expect_err("a child grant cannot widen the ancestor");
    assert!(error.contains("deny:tool:write_file"), "{error}");
    assert!(!root.join("child.txt").exists());

    // Releasing the ancestor restores both contexts; the child grant survives.
    registry.revoke_seat(ROOT_SEAT);
    registry
        .dispatch_with_cancel_in_seat(
            "child",
            "write_file",
            &serde_json::json!({"path": root.join("child.txt"), "content": "x"}),
            None,
        )
        .expect("with the ancestor's denial gone the child may write again");
    assert!(root.join("child.txt").exists());
    assert!(!root.join("root.txt").exists());
}

/// The production shape: `code_mode` installs its seat's table for the duration of
/// a run with `SeatGrant::install` and lets the scope's end release it, so an
/// unwinding run cannot leave its own attenuation behind.
#[test]
fn a_seat_grant_is_released_when_its_scope_ends() {
    let (registry, root) = registry_with_files("interception-scope");
    let target = root.join("scoped.txt");
    {
        let _seat = SeatGrant::install(&registry, "child", Interception::deny_tool("write_file"));
        let denied = registry
            .dispatch_with_cancel_in_seat(
                "child",
                "write_file",
                &serde_json::json!({"path": target, "content": "x"}),
                None,
            )
            .expect_err("the scope's table refuses the write");
        assert!(denied.contains("deny:tool:write_file"), "{denied}");
        assert!(!target.exists());
    }
    // The scope ended: the grant is gone without anyone remembering to undo it.
    registry
        .dispatch_with_cancel_in_seat(
            "child",
            "write_file",
            &serde_json::json!({"path": target, "content": "x"}),
            None,
        )
        .expect("the release restored the seat's ordinary policy");
    assert!(target.exists());
}
