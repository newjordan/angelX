use super::*;
use crate::agent::harness::{self, Tool, book};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "angel-labyrinth-native-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn labyrinth_policies_are_native_legend_routes_on_the_wire() {
    let root = root();
    initialize(&root).unwrap();
    observe_iteration(
        &root,
        "deli",
        "test native campaign",
        &["source-grounded lead".into()],
        &[],
    )
    .unwrap();
    let cue = context(&root, "test native campaign");
    assert!(cue.contains(&legend::MAP.cells()));
    assert!(cue.contains(&book::d3_roles::pages(legend::DATA, [9])));
    assert!(!cue.contains("choose a testable door"));
    assert!(!cue.contains("T4 is evidence"));
    let map = load(&root).unwrap().unwrap();
    let plan = map.plan(Some("test native campaign"), 3);
    assert!(plan["selection"].is_null() && plan["authority"].is_null());
    let routes = plan["warpath"].as_str().unwrap();
    let decoded = book::ledger::read(&root, routes).unwrap();
    assert!(decoded.contains("exact candidate bytes") && decoded.contains("T4 measured"));
    let navigation = crate::agent::tools::labyrinth::LabyrinthTool::new(root.clone()).def();
    let campaign =
        crate::agent::tools::labyrinth_campaign::LabyrinthCampaignTool::new(root.clone(), None)
            .def();
    assert_eq!(navigation.description, legend::MAP.cells());
    assert_eq!(campaign.description, legend::CAMPAIGN.cells());
    for route in [
        legend::COORDINATOR,
        legend::LITERATURE,
        legend::ATTACK,
        legend::REFEREE,
        legend::WRITER,
        legend::INTEGRATE,
        legend::LEADS,
        legend::FILES,
        legend::EXEC,
    ] {
        assert_eq!(book::find(route).unwrap().route, route);
        assert!(
            !book::ledger::read(&root, &route.cells())
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        book::ledger::read(&root, &legend::REFEREE.cells())
            .unwrap()
            .contains("your own verification code")
    );
    assert!(legend::escalation(8).is_some() && legend::escalation(9).is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn complete_workflow_is_available_offline_without_replacing_local_material() {
    let root = root();
    initialize(&root).unwrap();
    for (name, bytes) in resources::FILES {
        assert_eq!(
            std::fs::read(root.join("labyrinth/workflow").join(name)).unwrap(),
            *bytes,
            "{name}"
        );
    }
    for name in [
        "SKILL.md",
        "references/campaigns.md",
        "references/saturation.md",
        "templates/briefs/attack.md",
        "templates/briefs/referee.md",
        "templates/briefs/writer.md",
        "agents/openai.yaml",
        "tests/test_lab.py",
        "examples/triangle-counts/exhaustive_n8.py",
    ] {
        assert!(root.join("labyrinth/workflow").join(name).is_file());
    }
    let skill = root.join("labyrinth/workflow/SKILL.md");
    std::fs::write(&skill, b"local operator annotation").unwrap();
    initialize(&root).unwrap();
    assert_eq!(std::fs::read(skill).unwrap(), b"local operator annotation");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn transaction_source_mismatch_does_not_publish_any_part() {
    let root = root();
    std::fs::write(root.join("one.md"), b"one").unwrap();
    std::fs::write(root.join("two.md"), b"user work").unwrap();
    let edits = vec![
        (
            PathBuf::from("one.md"),
            Some(b"one".to_vec()),
            Some(b"replaced".to_vec()),
        ),
        (PathBuf::from("created.md"), None, Some(b"new".to_vec())),
        (PathBuf::from("two.md"), Some(b"stale".to_vec()), None),
    ];
    assert!(harness::confined_compare_replace_batch_no_symlinks(&root, &edits).is_err());
    assert_eq!(std::fs::read(root.join("one.md")).unwrap(), b"one");
    assert_eq!(std::fs::read(root.join("two.md")).unwrap(), b"user work");
    assert!(!root.join("created.md").exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn transaction_create_replace_delete_and_append_bounds_are_real_file_operations() {
    let root = root();
    std::fs::write(root.join("replace.md"), b"old").unwrap();
    std::fs::write(root.join("delete.md"), b"delete").unwrap();
    let edits = vec![
        (
            PathBuf::from("replace.md"),
            Some(b"old".to_vec()),
            Some(b"changed".to_vec()),
        ),
        (PathBuf::from("create.md"), None, Some(b"created".to_vec())),
        (PathBuf::from("delete.md"), Some(b"delete".to_vec()), None),
    ];
    harness::confined_compare_replace_batch_no_symlinks(&root, &edits).unwrap();
    assert_eq!(std::fs::read(root.join("replace.md")).unwrap(), b"changed");
    assert_eq!(std::fs::read(root.join("create.md")).unwrap(), b"created");
    assert!(!root.join("delete.md").exists());
    harness::confined_append_no_symlinks(&root, Path::new("events.jsonl"), b"first\n", 12).unwrap();
    assert!(
        harness::confined_append_no_symlinks(&root, Path::new("events.jsonl"), b"oversized\n", 12)
            .is_err()
    );
    assert_eq!(
        std::fs::read(root.join("events.jsonl")).unwrap(),
        b"first\n"
    );
    harness::confined_append_no_symlinks(&root, Path::new("events.jsonl"), b"next\n", 12).unwrap();
    assert_eq!(
        std::fs::read(root.join("events.jsonl")).unwrap(),
        b"first\nnext\n"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn strict_transactions_reject_aliases_preserve_modes_and_break_hard_links() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = root();
    let outside = root.with_extension("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("source.md"), b"old").unwrap();
    std::fs::set_permissions(
        outside.join("source.md"),
        std::fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    std::fs::hard_link(outside.join("source.md"), root.join("hard.md")).unwrap();
    harness::confined_compare_replace_batch_no_symlinks(
        &root,
        &[(
            PathBuf::from("hard.md"),
            Some(b"old".to_vec()),
            Some(b"new".to_vec()),
        )],
    )
    .unwrap();
    assert_eq!(std::fs::read(outside.join("source.md")).unwrap(), b"old");
    assert_eq!(
        std::fs::metadata(root.join("hard.md"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    symlink(&outside, root.join("alias")).unwrap();
    std::fs::hard_link(outside.join("source.md"), root.join("append-alias.jsonl")).unwrap();
    assert!(
        harness::confined_append_no_symlinks(&root, Path::new("append-alias.jsonl"), b"extra", 64)
            .is_err()
    );
    assert_eq!(std::fs::read(outside.join("source.md")).unwrap(), b"old");
    assert!(
        harness::confined_compare_replace_batch_no_symlinks(
            &root,
            &[(
                PathBuf::from("alias/source.md"),
                Some(b"old".to_vec()),
                Some(b"bad".to_vec())
            )]
        )
        .is_err()
    );
    assert_eq!(std::fs::read(outside.join("source.md")).unwrap(), b"old");
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(outside).unwrap();
}

#[cfg(unix)]
#[test]
fn a_busy_append_lock_does_not_modify_the_event_stream() {
    use std::os::fd::AsRawFd;
    let root = root();
    let file =
        harness::confined_open_rw_no_symlinks(&root, Path::new("events.jsonl"), true).unwrap();
    assert_eq!(
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert!(
        harness::confined_append_no_symlinks(&root, Path::new("events.jsonl"), b"unexpected\n", 64)
            .is_err()
    );
    assert!(std::fs::read(root.join("events.jsonl")).unwrap().is_empty());
    drop(file);
    harness::confined_append_no_symlinks(&root, Path::new("events.jsonl"), b"actual\n", 64)
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
