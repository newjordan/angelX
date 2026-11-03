use super::*;

#[test]
fn published_generation_is_stable_and_checked_on_load() {
    let workspace = crate::tests::TestGitWorkspace::new("book-generation-store");
    let root = workspace.path().join("generations");
    let first = br#"{"chapters":{"a":{"one":["safe content"]}},"schema":"angel.book-catalog/v1"}"#;
    let second = br#"{"chapters":{"a":{"one":["new content"]}},"schema":"angel.book-catalog/v1"}"#;
    let a = publish_in(&root, first).unwrap();
    assert_eq!(a, crate::knowledge::cut::sha256_hex(first));
    assert_eq!(publish_in(&root, first).unwrap(), a);
    let b = publish_in(&root, second).unwrap();
    assert_ne!(a, b);
    assert_eq!(load_in(&root, &a).unwrap(), first);
    assert_eq!(load_in(&root, &b).unwrap(), second);
    std::fs::write(root.join(format!("{a}.json")), b"changed").unwrap();
    assert!(load_in(&root, &a).unwrap_err().contains("corrupt"));
    assert!(publish_in(&root, first).unwrap_err().contains("corrupt"));
}

#[test]
fn rejects_missing_bad_digest_and_oversized_generation() {
    let workspace = crate::tests::TestGitWorkspace::new("book-generation-invalid");
    let root = workspace.path().join("generations");
    std::fs::create_dir(&root).unwrap();
    assert!(load_in(&root, "../bad").unwrap_err().contains("invalid"));
    assert!(
        load_in(&root, &"a".repeat(64))
            .unwrap_err()
            .contains("missing")
    );
    assert!(publish_in(&root, b"").is_err());
    assert!(publish_in(&root, &vec![b'x'; MAX_CATALOG_BYTES + 1]).is_err());
}

#[test]
fn concurrent_publishers_share_the_exact_generation() {
    let workspace = crate::tests::TestGitWorkspace::new("book-generation-concurrent");
    let root = workspace.path().join("generations");
    let bytes = b"{\"schema\":\"angel.book-catalog/v1\"}";
    let digests = std::thread::scope(|scope| {
        let handles = (0..8)
            .map(|_| scope.spawn(|| publish_in(&root, bytes)))
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(digests.iter().all(|digest| digest == &digests[0]));
    assert_eq!(load_in(&root, &digests[0]).unwrap(), bytes);
}
