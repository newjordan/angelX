use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

/// A workspace spelled relative to the working directory, as the polyglot
/// cell's `--workspace ws/<name>` is: `label` is that spelling.
struct LaunchDir {
    base: PathBuf,
    label: PathBuf,
}

impl LaunchDir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = PathBuf::from(format!(
            ".angel-launch-rel-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let label = base.join("ws").join(name);
        std::fs::create_dir_all(label.join("src")).unwrap();
        std::fs::write(label.join("bowling.py"), "class BowlingGame:\n    pass\n").unwrap();
        Self { base, label }
    }
}

impl Drop for LaunchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn a_path_spelled_from_the_launch_directory_names_the_workspace_file() {
    let dir = LaunchDir::new("bowling");
    let spelled = dir.label.join("bowling.py");
    assert_eq!(
        launch_relative_in_workspace(&dir.label, &spelled),
        Some(PathBuf::from("bowling.py"))
    );
    assert_eq!(
        launch_relative_in_workspace(&dir.label, &dir.label),
        Some(PathBuf::new())
    );
    // Every file tool resolves through `safe_path`: the read lands on the
    // workspace file, and a write does not create `ws/bowling/ws/bowling/`.
    let canonical = dir.label.canonicalize().unwrap();
    assert_eq!(
        safe_path(&dir.label, &spelled.to_string_lossy()).unwrap(),
        canonical.join("bowling.py")
    );
    assert_eq!(
        safe_path(&dir.label, &dir.label.join("src/new.rs").to_string_lossy()).unwrap(),
        canonical.join("src/new.rs")
    );
    assert_eq!(
        safe_path(&dir.label, "bowling.py").unwrap(),
        canonical.join("bowling.py")
    );
}

#[test]
fn a_real_entry_of_that_first_name_keeps_the_literal_path() {
    let dir = LaunchDir::new("bowling");
    let first = dir.base.clone();
    std::fs::create_dir_all(dir.label.join(&first)).unwrap();
    assert_eq!(
        launch_relative_in_workspace(&dir.label, &dir.label.join("bowling.py")),
        None
    );
}

#[test]
fn absolute_workspaces_absolute_paths_and_ordinary_paths_are_untouched() {
    let dir = LaunchDir::new("bowling");
    let canonical = dir.label.canonicalize().unwrap();
    assert_eq!(
        launch_relative_in_workspace(&canonical, &canonical.join("bowling.py")),
        None
    );
    assert_eq!(
        launch_relative_in_workspace(&dir.label, &canonical.join("bowling.py")),
        None
    );
    for plain in ["bowling.py", "src/lib.rs", "ws/other/x.py", "../x"] {
        assert_eq!(
            launch_relative_in_workspace(&dir.label, Path::new(plain)),
            None,
            "{plain}"
        );
    }
    assert_eq!(
        launch_relative_in_workspace(Path::new(""), Path::new("x")),
        None
    );
}

#[test]
fn a_leading_cd_to_the_launch_spelling_runs_in_the_workspace() {
    use crate::agent::tools::shell::resolve_launch_relative_cd;
    let dir = LaunchDir::new("bowling");
    let canonical = dir.label.canonicalize().unwrap();
    let label = dir.label.display();
    assert_eq!(
        resolve_launch_relative_cd(
            &format!("cd {label} && python3 -m pytest -q 2>&1 | grep FAILED"),
            Some(&dir.label)
        )
        .as_deref(),
        Some(
            format!(
                "cd '{}' && python3 -m pytest -q 2>&1 | grep FAILED",
                canonical.display()
            )
            .as_str()
        )
    );
    assert_eq!(
        resolve_launch_relative_cd(&format!("  cd {label}/src; ls"), Some(&dir.label)).as_deref(),
        Some(format!("  cd '{}'; ls", canonical.join("src").display()).as_str())
    );
    // Only a leading, plainly spelled cd the workspace cannot already resolve.
    for untouched in [
        "cd src && ls".to_string(),
        "cd . && ls".to_string(),
        format!("ls; cd {label}"),
        format!("cd \"{label}\" && ls"),
        format!("cd $HOME/{label}"),
        format!("cdx {label}"),
        "pwd".to_string(),
    ] {
        assert_eq!(
            resolve_launch_relative_cd(&untouched, Some(&dir.label)),
            None,
            "{untouched}"
        );
    }
    assert_eq!(
        resolve_launch_relative_cd(&format!("cd {label}"), Some(&canonical)),
        None
    );
}

#[test]
fn a_routed_verifier_takes_the_launch_spelling_as_the_workspace() {
    let dir = LaunchDir::new("bowling");
    let route = crate::agent::harness::shell_verifier::plan(
        "shell",
        &json!({"command": format!("cd {} && python3 -m pytest -q", dir.label.display())}),
        &dir.label,
    )
    .unwrap();
    assert_eq!(route.call.name, "run_tests");
    assert_eq!(route.call.args["dir"], ".");
    assert_eq!(route.call.args["entrypoint"], "pytest");
}

#[test]
fn a_written_rust_path_spelled_from_the_launch_directory_is_recorded_where_it_landed() {
    let dir = LaunchDir::new("acronym");
    let targets = crate::agent::tools::build::MutationTargets::default();
    targets.record(
        &dir.label,
        "write_file",
        &json!({"path": dir.label.join("src/lib.rs").to_string_lossy(), "content": ""}),
    );
    targets.record(
        &dir.label,
        "write_file",
        &json!({"path": "src/main.rs", "content": ""}),
    );
    let mut recorded = targets.snapshot().unwrap();
    recorded.sort();
    assert_eq!(recorded, ["src/lib.rs", "src/main.rs"]);
}
