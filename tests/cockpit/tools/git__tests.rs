use super::*;

#[test]
fn sealed_task_rejects_typed_commit_execution_before_touching_git() {
    let _lock = crate::tests::env_lock();
    let _sealed = crate::tests::TestEnvGuard::set("ANGEL_TASK_SHELL_PROTECT_GIT", "1");
    let tool = GitCommitTool {
        workspace: PathBuf::from("/definitely/not/a/repository"),
    };

    let error = tool
        .call(&serde_json::json!({ "execute": true }))
        .unwrap_err();
    assert!(
        error.contains("protects repository control state"),
        "{error}"
    );
}
