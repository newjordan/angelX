use super::*;

#[test]
fn parses_valid_rules_and_skips_bad_ones() {
    let json = r#"[
            {"pattern": "(?i)as an ai", "reminder": "answer directly"},
            {"pattern": "TODO", "reminder": ""},
            {"pattern": "(unclosed", "reminder": "bad regex is skipped"},
            {"reminder": "no pattern"},
            {"pattern": "stub", "reminder": "no stubs"}
        ]"#;
    let rules = parse_rules(json);
    // Only the two well-formed rules survive; empty-reminder and bad-regex
    // and missing-pattern entries are dropped.
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].reminder, "answer directly");
    assert_eq!(rules[1].reminder, "no stubs");
}

#[test]
fn first_new_match_respects_fired_set() {
    let rules = StreamRules {
        rules: parse_rules(
            r#"[{"pattern":"foo","reminder":"r1"},{"pattern":"bar","reminder":"r2"}]"#,
        ),
    };
    let mut fired = HashSet::new();
    // Matches rule 0 first.
    let (idx, rule) = rules.first_new_match("xx foo yy", &fired).unwrap();
    assert_eq!(idx, 0);
    assert_eq!(rule.reminder, "r1");
    // After firing 0, the same text no longer trips it.
    fired.insert(0);
    assert!(rules.first_new_match("xx foo yy", &fired).is_none());
    // A different pattern still trips.
    let (idx, _) = rules.first_new_match("foo and bar", &fired).unwrap();
    assert_eq!(idx, 1);
}

#[test]
fn empty_or_garbage_source_yields_no_rules() {
    assert!(parse_rules("").is_empty());
    assert!(parse_rules("not json").is_empty());
    assert!(parse_rules("{}").is_empty());
    assert!(parse_rules("[]").is_empty());
}

#[test]
fn reload_keeps_last_valid_rules_through_bad_or_missing_replacements() {
    let dir =
        std::env::temp_dir().join(format!("angel-stream-rules-{}-reload", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rules.json");
    let mut cache = RulesCache::default();
    assert!(cache.snapshot(Some(&path)).is_empty());
    std::fs::write(&path, r#"[{"pattern":"old","reminder":"first"}]"#).unwrap();
    let old = cache.snapshot(Some(&path));
    assert_eq!(
        old.first_new_match("old", &HashSet::new())
            .unwrap()
            .1
            .reminder,
        "first"
    );
    for bad in [
        "[",
        "{}",
        r#"[{"pattern":"(","reminder":"broken"}]"#,
        r#"[{"pattern":"new","reminder":"second"},{}]"#,
    ] {
        std::fs::write(&path, bad).unwrap();
        assert!(Arc::ptr_eq(&old, &cache.snapshot(Some(&path))));
        assert_eq!(cache.rejected.as_deref(), Some(bad));
        assert!(Arc::ptr_eq(&old, &cache.snapshot(Some(&path))));
    }
    std::fs::write(&path, " ".repeat(MAX_RULES_BYTES as usize + 1)).unwrap();
    assert!(Arc::ptr_eq(&old, &cache.snapshot(Some(&path))));
    let oversized_pattern = format!(
        r#"[{{"pattern":"{}","reminder":"second"}}]"#,
        "a".repeat(MAX_PATTERN_BYTES + 1)
    );
    std::fs::write(&path, oversized_pattern).unwrap();
    assert!(Arc::ptr_eq(&old, &cache.snapshot(Some(&path))));
    std::fs::remove_file(&path).unwrap();
    assert!(Arc::ptr_eq(&old, &cache.snapshot(Some(&path))));
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let path_bytes = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path_bytes.as_ptr(), 0o600) }, 0);
        assert!(Arc::ptr_eq(&old, &cache.snapshot(Some(&path))));
        std::fs::remove_file(&path).unwrap();
    }
    std::fs::write(&path, r#"[{"pattern":"new","reminder":"second"}]"#).unwrap();
    let new = cache.snapshot(Some(&path));
    assert!(cache.rejected.is_none());
    assert!(new.first_new_match("old", &HashSet::new()).is_none());
    assert_eq!(
        new.first_new_match("new", &HashSet::new())
            .unwrap()
            .1
            .reminder,
        "second"
    );
    assert!(old.first_new_match("old", &HashSet::new()).is_some());
    std::fs::write(&path, "[]").unwrap();
    assert!(cache.snapshot(Some(&path)).is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn simultaneous_requests_share_a_stable_snapshot_and_new_source_is_independent() {
    let dir = std::env::temp_dir().join(format!(
        "angel-stream-rules-{}-concurrent",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rules.json");
    std::fs::write(&path, r#"[{"pattern":"old","reminder":"first"}]"#).unwrap();
    let cache = Arc::new(Mutex::new(RulesCache::default()));
    let old = cache.lock().unwrap().snapshot(Some(&path));
    std::fs::write(&path, r#"[{"pattern":"new","reminder":"second"}]"#).unwrap();
    let snapshots = std::thread::scope(|scope| {
        (0..8)
            .map(|_| {
                let cache = Arc::clone(&cache);
                let path = &path;
                scope.spawn(move || cache.lock().unwrap().snapshot(Some(path)))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(
        snapshots
            .iter()
            .all(|snapshot| Arc::ptr_eq(snapshot, &snapshots[0]))
    );
    assert!(
        snapshots[0]
            .first_new_match("new", &HashSet::new())
            .is_some()
    );
    assert!(old.first_new_match("old", &HashSet::new()).is_some());
    assert!(
        cache
            .lock()
            .unwrap()
            .snapshot(Some(&dir.join("other.json")))
            .is_empty()
    );
    std::fs::remove_dir_all(dir).unwrap();
}
