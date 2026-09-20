//! Project-doc discovery, scope walk, and byte-cap coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- project docs suite -----------------------------------------------------

/// Write a realistic git-root marker (`.git/HEAD`) so `is_git_root` honors it
/// — a bare empty `.git` dir no longer counts.
fn mark_git_root(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(dir.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
}

#[test]
fn project_docs_concatenate_git_root_to_workspace() {
    let base = std::env::temp_dir().join(format!("angel_pdoc_chain_{}", std::process::id()));
    let sub = base.join("crate-a");
    std::fs::create_dir_all(&sub).unwrap();
    mark_git_root(&base); // real .git/HEAD marker
    std::fs::write(base.join("AGENTS.md"), "ROOT RULES").unwrap();
    std::fs::write(sub.join("AGENTS.md"), "SUB RULES").unwrap();
    let docs = discover_project_docs(&sub, 32 * 1024);
    let ri = docs.find("ROOT RULES").expect("root doc present");
    let si = docs.find("SUB RULES").expect("sub doc present");
    assert!(
        ri < si,
        "git-root doc must precede the workspace doc: {docs:?}"
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn project_docs_empty_when_none() {
    let base = std::env::temp_dir().join(format!("angel_pdoc_none_{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    mark_git_root(&base);
    assert_eq!(discover_project_docs(&base, 1024), "");
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn project_docs_respect_byte_cap() {
    let base = std::env::temp_dir().join(format!("angel_pdoc_cap_{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    mark_git_root(&base);
    std::fs::write(base.join("AGENTS.md"), "x".repeat(5000)).unwrap();
    let docs = discover_project_docs(&base, 1000);
    assert!(docs.len() <= 1000, "hard byte cap exceeded: {}", docs.len());
    assert!(docs.contains("truncated"));
    assert!(docs.starts_with("## AGENTS.md"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn project_doc_cap_preserves_root_and_higher_precedence_nested_edges() {
    let base = std::env::temp_dir().join(format!("angel_pdoc_scopes_{}", std::process::id()));
    let sub = base.join("crate-a");
    std::fs::create_dir_all(&sub).unwrap();
    mark_git_root(&base);
    std::fs::write(
        base.join("AGENTS.md"),
        format!("ROOT-HEAD\n{}\nROOT-TAIL", "r".repeat(4000)),
    )
    .unwrap();
    std::fs::write(
        sub.join("AGENTS.md"),
        format!("SUB-HEAD\n{}\nSUB-TAIL", "s".repeat(4000)),
    )
    .unwrap();

    let docs = discover_project_docs(&sub, 800);
    assert!(docs.len() <= 800, "hard byte cap exceeded: {}", docs.len());
    for expected in [
        "## AGENTS.md",
        "ROOT-HEAD",
        "ROOT-TAIL",
        "## crate-a/AGENTS.md",
        "SUB-HEAD",
        "SUB-TAIL",
    ] {
        assert!(docs.contains(expected), "missing {expected:?}: {docs}");
    }
    assert_eq!(docs.matches("scoped project doc truncated").count(), 2);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn project_doc_cap_reallocates_short_parent_budget_to_nested_scope_utf8_safely() {
    let base = std::env::temp_dir().join(format!("angel_pdoc_fair_{}", std::process::id()));
    let sub = base.join("crate-a");
    std::fs::create_dir_all(&sub).unwrap();
    mark_git_root(&base);
    std::fs::write(base.join("AGENTS.md"), "ROOT-SHORT").unwrap();
    std::fs::write(
        sub.join("AGENTS.md"),
        format!("SUB-HEAD\n{}\nSUB-TAIL", "🦀".repeat(2000)),
    )
    .unwrap();

    let docs = discover_project_docs(&sub, 700);
    assert!(docs.len() <= 700);
    assert!(docs.contains("ROOT-SHORT"));
    assert!(docs.contains("SUB-HEAD"));
    assert!(docs.contains("SUB-TAIL"));
    assert!(
        docs.len() > 600,
        "short parent budget was not reallocated: {}",
        docs.len()
    );
    assert!(discover_project_docs(&sub, 0).is_empty());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn project_docs_no_git_root_reads_only_start() {
    // No `.git` up the chain → only the start dir is read; the parent's doc is
    // ignored (we never walk the whole filesystem).
    let base = std::env::temp_dir().join(format!("angel_pdoc_nogit_{}", std::process::id()));
    let sub = base.join("inner");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(base.join("AGENTS.md"), "PARENT DOC").unwrap();
    std::fs::write(sub.join("AGENTS.md"), "INNER DOC").unwrap();
    let docs = discover_project_docs(&sub, 1024);
    assert!(docs.contains("INNER DOC"));
    assert!(
        !docs.contains("PARENT DOC"),
        "must not walk past start without a git root"
    );
    std::fs::remove_dir_all(&base).ok();
}
