use super::*;
use std::fs;

struct TempGitRepo(PathBuf);

impl TempGitRepo {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "angel-git-commit-split-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        run_git(&path, &["init", "-q"]).unwrap();
        run_git(&path, &["config", "commit.gpgsign", "false"]).unwrap();
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("src/lib.rs"), "pub fn value() -> u8 { 1 }\n").unwrap();
        run_git(&path, &["add", "--", "src/lib.rs"]).unwrap();
        run_git(&path, &["commit", "-q", "-m", "initial"]).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn committed_paths(repo: &Path, revision: &str) -> Vec<String> {
    run_git(repo, &["show", "--pretty=format:", "--name-only", revision])
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

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

#[test]
fn execute_split_preserves_prestaged_docs_until_docs_unit() {
    let repo = TempGitRepo::new();
    fs::write(repo.path().join("src/lib.rs"), "pub fn value() -> u8 { 2 }\n").unwrap();
    fs::create_dir_all(repo.path().join("docs")).unwrap();
    fs::write(repo.path().join("docs/change.md"), "Change notes.\n").unwrap();
    run_git(repo.path(), &["add", "--", "docs/change.md"]).unwrap();

    let tool = GitCommitTool {
        workspace: repo.path().to_path_buf(),
    };
    let result = tool
        .call(&serde_json::json!({ "execute": true, "split": true }))
        .unwrap();

    assert!(result.contains("2 unit(s)"), "{result}");
    assert_eq!(
        committed_paths(repo.path(), "HEAD~1"),
        vec!["src/lib.rs".to_string()]
    );
    assert_eq!(
        committed_paths(repo.path(), "HEAD"),
        vec!["docs/change.md".to_string()]
    );
    assert!(run_git(repo.path(), &["status", "--porcelain"])
        .unwrap()
        .trim()
        .is_empty());
}
