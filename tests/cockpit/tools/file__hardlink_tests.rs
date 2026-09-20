use super::*;
use std::os::unix::fs::MetadataExt;

#[test]
fn external_patch_keeps_scratch_inside_writable_workspace() {
    let _lock = crate::tests::env_lock();
    let fixture = crate::agent::sandbox::HardlinkTestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("target.txt"), "before\n").unwrap();
    let diff = fixture.path().join("change.diff");
    std::fs::write(
        &diff,
        "--- a/target.txt\n+++ b/target.txt\n@@ -1 +1 @@\n-before\n+after\n",
    )
    .unwrap();
    check_and_apply_with(
        "patch",
        &["--batch", "--forward", "--dry-run", "-p1", "-i"],
        &["--batch", "--forward", "-p1", "-i"],
        &diff,
        &root,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("target.txt")).unwrap(),
        "after\n"
    );
}

#[test]
fn file_tools_break_hardlinks_without_changing_outside_inode() {
    let _lock = crate::tests::env_lock();
    let fixture = crate::agent::sandbox::HardlinkTestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let outside = fixture.path().join("outside");
    std::fs::write(&outside, "before\n").unwrap();
    let cases: Vec<(Box<dyn Tool>, Value)> = vec![
        (
            Box::new(WriteFileTool { root: root.clone() }),
            serde_json::json!({"path":"alias", "content":"after\n"}),
        ),
        (
            Box::new(StrReplaceTool { root: root.clone() }),
            serde_json::json!({"path":"alias", "old":"before", "new":"after"}),
        ),
        (
            Box::new(MultiEditTool { root: root.clone() }),
            serde_json::json!({"path":"alias", "edits":[{"old":"before", "new":"after"}]}),
        ),
        (
            Box::new(ApplyPatchTool { root: root.clone() }),
            serde_json::json!({"diff":"*** Begin Patch\n*** Update File: alias\n@@\n-before\n+after\n*** End Patch"}),
        ),
    ];
    for (tool, args) in cases {
        let alias = root.join("alias");
        std::fs::hard_link(&outside, &alias).unwrap();
        let result = tool.call(&args).unwrap();
        assert!(
            result.contains("hardlink_broken: true"),
            "{}: {result}",
            tool.name()
        );
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "before\n");
        assert_eq!(std::fs::read_to_string(&alias).unwrap(), "after\n");
        assert_eq!(std::fs::metadata(&alias).unwrap().nlink(), 1);
        assert_ne!(
            std::fs::metadata(&alias).unwrap().ino(),
            std::fs::metadata(&outside).unwrap().ino()
        );
        std::fs::remove_file(alias).unwrap();
    }
}

#[test]
fn rejected_edit_does_not_claim_link_breaking() {
    let _lock = crate::tests::env_lock();
    let fixture = crate::agent::sandbox::HardlinkTestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let outside = fixture.path().join("outside");
    std::fs::write(&outside, "before").unwrap();
    std::fs::hard_link(&outside, root.join("alias")).unwrap();
    let error = StrReplaceTool { root: root.clone() }
        .call(&serde_json::json!({"path":"alias", "old":"missing", "new":"after"}))
        .unwrap_err();
    assert!(!error.contains("hardlink_broken"));
    assert_eq!(std::fs::metadata(root.join("alias")).unwrap().nlink(), 2);
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "before");
}
