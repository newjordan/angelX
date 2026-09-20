//! T5 witness — the independence gate over *children*, at the git level.
//!
//! Transfer of arXiv 2608.25512 §3.1.1 (Def 42/44, Thm 43/45): a set of effects
//! whose footprints do not intersect commutes — they may run concurrently, and
//! their inverses may then be applied in **any** order, ending in `φ(γ) = γ₀`
//! (Thm 7). The paper's own caveat is the discipline here (§6.1: the runtime
//! checks no witness), so the check below is not an argument about the model: it
//! creates real delegated worktrees with the same helper the delegate uses, gives
//! each child a real commit, reverts them in the plan's reverse order with the same
//! inverse pair the delegate uses, and then asks git whether the repository is
//! *bit-identical* — same HEAD, same refs, empty status, no worktree residue.

use super::*;
use crate::agent::harness::independence::{self, ChildFootprint};
use crate::agent::harness::spawn::SpawnTool;
use std::path::Path;

fn git(root: &Path, args: &[&str]) -> String {
    run_git(root, args)
        .unwrap_or_else(|error| panic!("git {args:?} in {}: {error}", root.display()))
}

/// Distinct children with a checkout and a ref each are one wave, and reverting
/// them in that plan's reverse order restores the repository exactly.
#[test]
fn disjoint_worktree_children_revert_in_reverse_order_without_residue() {
    let root = scratch("independence_gate");
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("README.md"), "base\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "base"]);
    let base = git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    let refs_before = git(
        &repo,
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );

    // Worktrees live beside the repository, the shape `DelegateTool::worktree_base`
    // chooses so a delegated checkout never reads as user dirt.
    let worktrees = root.join(".angel-worktrees");
    std::fs::create_dir_all(&worktrees).unwrap();
    let children: Vec<ChildFootprint> = (0..3)
        .map(|index| {
            ChildFootprint::worktree_child(
                format!("child-{index}"),
                worktrees.join(format!("c{index}")),
                format!("angel/c{index}"),
            )
        })
        .collect();

    let plan = independence::plan(&children);
    assert!(plan.independent(), "disjoint checkouts commute: {plan:?}");
    assert!(independence::refusal(&children).is_none());
    let revert_order = plan.revert_order();
    assert_eq!(revert_order, vec![2, 1, 0]);

    // Real effects: each child gets its own checkout, off its own ref, with a
    // commit of its own — the delegate's `prepare_delegate_worktree` path.
    for index in 0..children.len() {
        let worktree = worktrees.join(format!("c{index}"));
        crate::agent::harness::orchestrator::prepare_delegate_worktree(
            &repo,
            &worktree,
            &format!("angel/c{index}"),
            &base,
            "off-limits",
            None,
        )
        .unwrap();
        std::fs::write(worktree.join("README.md"), format!("child {index}\n")).unwrap();
        git(&worktree, &["add", "-A"]);
        git(
            &worktree,
            &["commit", "-q", "-m", &format!("child {index}")],
        );
    }
    // Three children really ran: three refs, three checkouts, and the shared
    // checkout untouched.
    assert_eq!(git(&repo, &["worktree", "list"]).lines().count(), 4);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]).trim(), base);
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");

    // The inverse pair the delegate applies, in the order the plan gives.
    for index in revert_order {
        let worktree = worktrees.join(format!("c{index}"));
        git(
            &repo,
            &[
                "worktree",
                "remove",
                "--force",
                worktree.to_string_lossy().as_ref(),
            ],
        );
        git(&repo, &["branch", "-D", &format!("angel/c{index}")]);
    }

    // Bit-identical to the pre-spawn state (Thm 7's `φ(γ) = γ₀`), witnessed by git
    // rather than asserted in prose.
    assert_eq!(
        git(&repo, &["rev-parse", "HEAD"]).trim(),
        base,
        "HEAD moved"
    );
    assert_eq!(
        git(
            &repo,
            &["for-each-ref", "--format=%(refname) %(objectname)"]
        ),
        refs_before,
        "a ref survived"
    );
    assert_eq!(
        git(&repo, &["status", "--porcelain"]),
        "",
        "worktree is dirty"
    );
    assert_eq!(
        git(&repo, &["worktree", "list"]).lines().count(),
        1,
        "worktree metadata survived"
    );
    assert!(
        std::fs::read_dir(&worktrees).unwrap().next().is_none(),
        "checkout residue left on disk"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Overlap is *partial*: only the children that share a resource are serialized,
/// and the refusal names the resource for the caller.
#[test]
fn one_shared_ref_serializes_only_the_pair_that_shares_it() {
    let root = scratch("independence_partial");
    let children = vec![
        ChildFootprint::worktree_child("child-0", root.join("w0"), "angel/shared"),
        ChildFootprint::worktree_child("child-1", root.join("w1"), "angel/shared"),
        ChildFootprint::workspace_writer("seat-2", root.join("workspace")),
    ];
    let plan = independence::plan(&children);
    assert!(!plan.independent());
    assert_eq!(plan.waves, vec![vec![0, 2], vec![1]], "{plan:?}");
    assert_eq!(plan.revert_order(), vec![1, 2, 0]);

    let receipt = independence::refusal(&children).expect("a receipt");
    assert!(receipt.contains("angel/shared"), "names the ref: {receipt}");
    assert!(
        receipt.contains("child-0") && receipt.contains("child-1"),
        "names the pair: {receipt}"
    );
    assert!(
        !receipt.contains("seat-2"),
        "the child that only commutes is not accused: {receipt}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The admission gate on the production `spawn` path: `tools=code` seats are
/// serialized because they claim the one shared workspace — and the refusal says
/// which resource collided, instead of only which knob to turn.
#[test]
fn spawn_code_seats_are_serialized_by_their_shared_workspace_claim() {
    let workspace = scratch("independence_spawn");
    let tool = SpawnTool::new(workspace.clone(), None, Vec::new());

    let error = tool
        .call(&serde_json::json!({"task": "write", "n": 2, "tools": "code"}))
        .expect_err("two code seats must not be admitted");
    assert!(error.contains("tools=code needs n=1"), "{error}");
    assert!(
        error.contains("shared workspace"),
        "the collision is named: {error}"
    );
    assert!(
        error.contains(workspace.to_string_lossy().as_ref()),
        "the workspace is named: {error}"
    );

    // The gate is about concurrency, not about the grant: one seat is admitted and
    // fails later for want of a roster, which is a different error.
    let single = tool
        .call(&serde_json::json!({"task": "write", "n": 1, "tools": "code"}))
        .expect_err("no clubs are registered");
    assert!(!single.contains("needs n=1"), "{single}");

    // Read-only seats declare nothing exclusive, so a panel of them is admitted.
    let panel = tool.call(&serde_json::json!({
        "task": "read", "n": 3, "tools": "read_only", "formation": "panel"
    }));
    if let Err(error) = panel {
        assert!(!error.contains("needs n=1"), "{error}");
    }

    let _ = std::fs::remove_dir_all(&workspace);
}
