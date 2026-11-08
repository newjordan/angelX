use super::*;

fn prefetched_for(workspace: &Path) -> bool {
    PREFETCHED_RUNTIMES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .is_some_and(|(begun_for, _)| begun_for == workspace)
}

#[test]
fn a_started_capture_is_taken_once_by_the_registry_for_its_workspace() {
    let _env = crate::tests::env_lock();
    let workspace = crate::tests::TestGitWorkspace::new("prefetch-pins");
    let other = crate::tests::TestGitWorkspace::new("prefetch-pins-other");

    prefetch_verifier_runtimes(workspace.path());
    assert!(prefetched_for(workspace.path()));

    // A registry for another workspace neither takes nor discards it.
    let _ = capture_verifier_runtimes(other.path());
    assert!(prefetched_for(workspace.path()));

    // The registry for its workspace takes it, exactly once...
    let (cargo, _native) = capture_verifier_runtimes(workspace.path());
    assert!(!prefetched_for(workspace.path()));
    let fresh = capture_verifier_runtimes_now(workspace.path());
    // ...and what it takes is the same answer a capture made on the spot gives.
    assert_eq!(cargo.executable().is_ok(), fresh.0.executable().is_ok());
    if let (Ok(taken), Ok(now)) = (cargo.executable(), fresh.0.executable()) {
        assert_eq!(taken.cargo.path, now.cargo.path);
        assert_eq!(taken.rustc.path, now.rustc.path);
    }

    // With nothing started, a registry simply captures.
    let _ = capture_verifier_runtimes(workspace.path());
    assert!(!prefetched_for(workspace.path()));
}
