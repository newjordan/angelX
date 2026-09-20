use super::*;

#[test]
fn verification_target_dep_budget_exhaustion_stops_further_reads() {
    let root = std::env::temp_dir().join(format!(
        "angel-owned-dep-budget-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let dep = root.join("owned.d");
    std::fs::write(&dep, b"out: subject.rs\n").unwrap();
    let mut remaining = 3;
    assert!(bounded_dep(&dep, &root, &mut remaining).is_none());
    assert_eq!(remaining, 0);
    assert!(bounded_dep(&dep, &root, &mut remaining).is_none());
    std::fs::remove_dir_all(root).unwrap();
}
