use super::*;

#[test]
fn worktree_top_finds_the_git_link() {
    let base = std::env::temp_dir().join(format!("angel_wt_top_{}", std::process::id()));
    let top = base.join("self-1");
    let nested = top.join("cockpit/src");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(top.join(".git"), "gitdir: elsewhere").unwrap();
    assert_eq!(worktree_top(&nested), Some(top.clone()));
    // No .git anywhere up the chain → None (a plain dir isn't a worktree).
    let stray = base.join("not-a-worktree");
    std::fs::create_dir_all(&stray).unwrap();
    let found = worktree_top(&stray);
    assert!(
        found.is_none() || found.is_some_and(|p| !p.starts_with(&base)),
        "a dir without .git resolves to nothing inside the scratch base"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn truncate_is_char_safe() {
    assert_eq!(truncate("short", 60), "short");
    let long = "x".repeat(80);
    let t = truncate(&long, 60);
    assert_eq!(t.chars().count(), 61); // 60 + ellipsis
    assert!(t.ends_with('…'));
    assert_eq!(truncate("日本語テスト", 3), "日本語…");
}

#[test]
fn self_modification_requires_the_current_canonical_project() {
    let base = std::env::temp_dir().join(format!("angel_self_scope_{}", std::process::id()));
    let alpha = base.join("alpha");
    let beta = base.join("beta");
    std::fs::create_dir_all(&alpha).unwrap();
    std::fs::create_dir_all(&beta).unwrap();
    assert!(same_canonical_project(&alpha, &alpha));
    assert!(!same_canonical_project(&alpha, &beta));
    let _ = std::fs::remove_dir_all(base);
}
