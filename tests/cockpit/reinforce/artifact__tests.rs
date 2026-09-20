use super::*;
use crate::reinforce::{TEST_VERIFIER_CONTRACT, TestReward};

#[test]
fn evaluator_artifact_roundtrip_replays_reward_and_rejects_tamper() {
    // This fixture spawns raw `git` (not the env-stripping pinned
    // command), so it holds the crate env lock: a concurrent test that
    // sets GIT_DIR/GIT_WORK_TREE (the workspace-evidence decoy) would
    // otherwise redirect the spawned git into a different work tree and
    // fail `git add` with "pathspec did not match" at random.
    let _guard = crate::tests::env_lock();
    let root =
        std::env::temp_dir().join(format!("angel-evaluator-artifact-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("workspace");
    let store = root.join("store");
    std::fs::create_dir_all(&workspace).unwrap();
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&workspace)
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::write(workspace.join("fixture.txt"), "frozen fixture").unwrap();
    let status = std::process::Command::new("git")
        .args(["add", "fixture.txt"])
        .current_dir(&workspace)
        .status()
        .unwrap();
    assert!(status.success());

    let evidence = EvaluatorEvidence::run_shell(
        "artifact-test",
        "printf '%s' 'test result: ok. 3 passed; 0 failed;'",
        &workspace,
        TEST_VERIFIER_CONTRACT,
        "artifact-subject",
    )
    .unwrap();
    let path = evidence.persist_append_only(&store).unwrap();
    assert_eq!(evidence.persist_append_only(&store).unwrap(), path);
    let writers = (0..8)
        .map(|_| {
            let evidence = evidence.clone();
            let store = store.clone();
            std::thread::spawn(move || evidence.persist_append_only(&store).unwrap())
        })
        .collect::<Vec<_>>();
    assert!(
        writers
            .into_iter()
            .all(|writer| writer.join().unwrap() == path)
    );
    assert!(std::fs::read_dir(&store).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let loaded = load_evaluator_artifact(&path).unwrap();
    assert_eq!(loaded.manifest_sha256(), evidence.manifest_sha256());
    assert_eq!(loaded.raw_output_sha256(), evidence.raw_output_sha256());
    assert_eq!(loaded.output(), evidence.output());
    assert_eq!(replay_evaluator_artifact(&path, &TestReward).unwrap(), 1.0);

    let original = std::fs::read(&path).unwrap();
    let mut tampered = original.clone();
    *tampered.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &tampered).unwrap();
    assert!(load_evaluator_artifact(&path).is_err());
    assert!(
        evidence
            .persist_append_only(&store)
            .unwrap_err()
            .contains("different bytes")
    );

    std::fs::write(&path, &original).unwrap();
    let wrong_name = store.join("wrong.evidence");
    std::fs::write(&wrong_name, &original).unwrap();
    assert!(
        load_evaluator_artifact(&wrong_name)
            .unwrap_err()
            .contains("filename")
    );

    #[cfg(unix)]
    {
        let link = store.join("link.evidence");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(
            load_evaluator_artifact(&link)
                .unwrap_err()
                .contains("non-symlink")
        );
    }
    let _ = std::fs::remove_dir_all(root);
}
