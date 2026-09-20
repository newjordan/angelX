use super::*;

#[test]
fn seatbelt_profile_mirrors_the_landlock_posture() {
    let policy = SandboxPolicy {
        writable_roots: vec![PathBuf::from("/angel-nonexistent-root/work \"space\"")],
        allow_network: false,
        enforce: true,
        mandatory: false,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    };
    let profile = seatbelt_profile(&policy);
    let lines: Vec<&str> = profile.lines().collect();
    // Order carries the semantics: SBPL is last-match-wins, so the broad
    // read-everything / deny-writes pair must precede every allowance, and
    // the network deny must come last.
    assert_eq!(lines[0], "(version 1)");
    assert_eq!(lines[1], "(allow default)");
    assert_eq!(lines[2], "(deny file-write*)");
    assert_eq!(lines.last(), Some(&"(deny network*)"));
    assert!(profile.contains(r#"(allow file-write* (literal "/dev/null"))"#));
    // A nonexistent root passes through verbatim (canonicalize fallback)
    // with SBPL string escaping applied to the embedded quote.
    assert!(
        profile
            .contains(r#"(allow file-write* (subpath "/angel-nonexistent-root/work \"space\""))"#),
        "{profile}"
    );

    let open = seatbelt_profile(&SandboxPolicy {
        writable_roots: Vec::new(),
        allow_network: true,
        enforce: true,
        mandatory: false,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    });
    assert!(
        !open.contains("network"),
        "permissive network must not emit a deny: {open}"
    );
}
