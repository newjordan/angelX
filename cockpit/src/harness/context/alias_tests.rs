use super::*;
use std::os::unix::fs::symlink;

fn fixture(tag: &str, test: impl FnOnce(&Path)) {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-boundary-alias-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    test(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn absent_aliased_goal_store_does_not_poison_future_fallback_reads() {
    // env-lock-exempt: fixture in this module holds crate::tests::env_lock for the entire closure.
    fixture("goal", |root| {
        let physical = root.join("physical");
        let alias = root.join("alias");
        let workspace = physical.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        symlink(&physical, &alias).unwrap();
        let store = alias.join(".angel0/goals");
        let file = store.join("goal.json");
        let _goal = crate::tests::TestEnvGuard::set("ANGEL_GOAL_FILE", file.to_str().unwrap());
        assert!(!store.exists());
        assert!(crate::goal::load_for(&workspace).is_none());
        // Exercise the same canonical fallback used by non-Linux confined reads.
        // Linux's descriptor implementation alone would hide this cache defect.
        let _ = safe_path(&store, "goal.json");
        let mut goal = crate::goal::Goal::new("restore only this project's goal");
        crate::goal::save_for(&mut goal, &workspace).unwrap();
        let resolved =
            safe_path(&store, "goal.json").expect("created aliased store remains readable");
        let decoded: crate::goal::Goal =
            serde_json::from_slice(&std::fs::read(resolved).unwrap()).unwrap();
        assert_eq!(decoded.text, goal.text);
        assert_eq!(crate::goal::load_for(&workspace).unwrap().text, goal.text);

        let outside = physical.join("outside.json");
        std::fs::write(&outside, "outside sentinel").unwrap();
        symlink(&outside, store.join("escape.json")).unwrap();
        assert!(safe_path(&store, "escape.json").is_err());
        assert!(safe_path(&store, "../outside.json").is_err());
    });
}

#[test]
fn an_existing_cached_boundary_stays_pinned_when_its_alias_retargets() {
    fixture("retarget", |root| {
        let first = root.join("first");
        let second = root.join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        std::fs::write(first.join("state"), "first").unwrap();
        std::fs::write(second.join("state"), "second").unwrap();
        let alias = root.join("alias");
        symlink(&first, &alias).unwrap();
        assert_eq!(
            safe_path(&alias, "state").unwrap(),
            first.join("state").canonicalize().unwrap()
        );
        std::fs::remove_file(&alias).unwrap();
        symlink(&second, &alias).unwrap();
        assert!(
            safe_path(&alias, "state").is_err(),
            "retarget must not rebind an established trust boundary"
        );
        assert_eq!(
            std::fs::read_to_string(second.join("state")).unwrap(),
            "second"
        );
    });
}
