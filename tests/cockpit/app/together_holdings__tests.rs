//! The holdings as the cockpit feeds them (the herald's reading of a tool call) and saves them with its realm.

use crate::drive::together_shooter::holdings::*;
use crate::drive::together_shooter::holdings::testing::*;
use super::op_of_call;
use crate::drive::together_realm::Spoil;

#[test]
fn the_herald_tells_commits_tests_compactions_and_subagents() {
    assert_eq!(op_of_call("shell", "command=git commit -m \"x\""), Some((Op::Commits, false)));
    assert_eq!(op_of_call("shell", "command=cargo test --bin angel"), Some((Op::Tests, false)));
    assert_eq!(op_of_call("run_tests", ""), Some((Op::Tests, false)));
    assert_eq!(op_of_call("spawn", "task=look"), Some((Op::Subagents, true)));
    assert_eq!(op_of_call("memory_compact", ""), Some((Op::Compactions, false)));
    assert_eq!(op_of_call("read_file", "path=a.rs"), None);
    assert_eq!(op_of_call("shell", "command=ls"), None);
    let mut ops = Ops::default();
    for op in [Op::Tests, Op::Commits, Op::Compactions, Op::Subagents, Op::Loops, Op::Errors] {
        ops.note(op);
    }
    assert_eq!((ops.tests, ops.commits, ops.compactions, ops.subagents, ops.loops, ops.errors), (1, 1, 1, 1, 1, 1));
}

#[test]
fn holdings_are_saved_with_the_realm_and_a_bad_record_resets_alone() {
    use crate::drive::together_realm::Realm;
    let dir = std::env::temp_dir().join(format!("holdings-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rewards = dir.join("island.json");
    let path = rewards.with_extension("realm.json");
    // An older realm: no holdings at all.
    std::fs::write(&path, r#"{"treasury":{"gold":5},"home":{"deepest":3}}"#).unwrap();
    let mut realm = Realm::beside(Some(&rewards));
    assert_eq!(realm.home.holdings, Holdings::default());
    let (h, _) = held();
    realm.home.holdings = h.clone();
    realm.save().unwrap();
    let again = Realm::beside(Some(&rewards));
    assert_eq!(again.home.holdings, h);
    // A broken holdings record costs the holdings, never the realm.
    std::fs::write(
        &path,
        r#"{"treasury":{"gold":5},"home":{"deepest":3,"holdings":{"owned":7,"stores":"lots"}}}"#,
    )
    .unwrap();
    let realm = Realm::beside(Some(&rewards));
    assert_eq!(realm.treasury.get(Spoil::Gold), 5);
    assert_eq!(realm.home.deepest, 3);
    assert_eq!(realm.home.holdings, Holdings::default());
    std::fs::remove_dir_all(dir).unwrap();
}
