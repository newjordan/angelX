use super::*;

#[test]
fn parses_a_local_block() {
    let draft = "Some analysis.\n\n```swarm-test\nwhere: local\ncmd: cargo test --quiet foo\n\
                 claim: the parser handles empty input\nwhy: cheap to check\n```\nMore prose.";
    let reqs = parse_requests(draft);
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].placement, Placement::Local);
    assert_eq!(reqs[0].cmd, "cargo test --quiet foo");
    assert!(reqs[0].claim.contains("empty input"));
    let stripped = strip_blocks(draft);
    assert!(!stripped.contains("swarm-test"));
    assert!(stripped.contains("More prose."));
}

#[test]
fn parses_all_placement_hints() {
    assert_eq!(
        parse_placement("remote:spark   # has the GPU"),
        Some(Placement::Remote("spark".to_string()))
    );
    assert_eq!(
        parse_placement("peer:atlas.seraph"),
        Some(Placement::Peer("atlas.seraph".to_string()))
    );
    assert_eq!(
        parse_placement("phone:math"),
        Some(Placement::Phone("math".to_string()))
    );
    assert_eq!(parse_placement("local"), Some(Placement::Local));
    assert_eq!(parse_placement("remote:"), None);
    assert_eq!(parse_placement("phon:math"), None);
}

#[test]
fn phone_block_can_use_claim_as_question() {
    let reqs =
        parse_requests("```swarm-test\nwhere: phone:math\nclaim: Is this invariant true?\n```");
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].placement, Placement::Phone("math".to_string()));
    assert!(reqs[0].cmd.is_empty());

    // A mistyped placement is discarded, never silently run as local shell.
    assert!(
        parse_requests("```swarm-test\nwhere: phon:math\ncmd: cargo test\nclaim: typo\n```")
            .is_empty()
    );
}

#[test]
fn duplicate_agent_requests_run_only_once() {
    let request = TestRequest {
        claim: "parser accepts empty input".to_string(),
        cmd: "cargo test parser_empty".to_string(),
        placement: Placement::Local,
        why: "agent one".to_string(),
    };
    let mut duplicate = request.clone();
    duplicate.why = "agent two reached the same request".to_string();
    let remote = TestRequest {
        placement: Placement::Remote("spark".to_string()),
        ..request.clone()
    };

    let unique = dedupe_requests(vec![request.clone(), duplicate, remote.clone()], 3);
    assert_eq!(unique.len(), 2);
    assert_eq!(unique[0].why, "agent one", "first request wins stably");
    assert_eq!(unique[1].placement, remote.placement);
    assert_eq!(dedupe_requests(vec![request.clone(), remote], 1).len(), 1);
    assert!(dedupe_requests(vec![request], 0).is_empty());
}

#[test]
fn policy_allows_safe_local_and_denies_danger() {
    let r = Router::from_env();
    let ok = TestRequest {
        claim: String::new(),
        cmd: "cargo test --quiet".to_string(),
        placement: Placement::Local,
        why: String::new(),
    };
    assert!(r.approve(&ok).is_ok());

    for bad in [
        "rm -rf /",
        "echo hi; sudo reboot",
        "cat /etc/passwd",
        "echo $(whoami)",
        "echo `whoami`",
    ] {
        let req = TestRequest {
            claim: String::new(),
            cmd: bad.to_string(),
            placement: Placement::Local,
            why: String::new(),
        };
        assert!(r.approve(&req).is_err(), "should deny: {bad}");
    }

    let weird = TestRequest {
        claim: String::new(),
        cmd: "perl -e 'print 1'".to_string(),
        placement: Placement::Local,
        why: String::new(),
    };
    assert!(r.approve(&weird).is_err());
}

#[test]
fn policy_checks_every_program_in_shell_chains() {
    let _guard = crate::tests::env_lock();
    let _ssh = crate::tests::TestEnvGuard::set("ANGEL_SWARM_SSH_SPARK", "runner@gpu.example");
    for safe in [
        "cargo test --quiet && true",
        "FOO=1 env BAR=2 cargo test --quiet",
        "echo 'semicolon; inside data' 2>&1",
        "printf ok | wc -c",
    ] {
        assert!(programs_allowed(safe, LOCAL_ALLOW), "should allow: {safe}");
    }
    for bypass in [
        "cargo test; perl -e 'print 1'",
        "cargo test | perl -e 'print 1'",
        "env perl -e 'print 1'",
        "cargo test && /usr/bin/perl -e 'print 1'",
        "cargo test --quiet &",
        "echo `perl -e 'print 1'`",
        "echo $(perl -e 'print 1')",
        "echo 'unterminated",
    ] {
        assert!(
            !programs_allowed(bypass, LOCAL_ALLOW),
            "should reject every executable: {bypass}"
        );
    }
    assert!(uses_inline_interpreter("python3 -c 'print(1)'"));
    assert!(uses_inline_interpreter("env X=1 python -cprint(1)"));
    assert!(!uses_inline_interpreter("python3 -m pytest tests"));
    assert!(!uses_inline_interpreter("python3 tests/check.py"));

    let remote_router = Router {
        workspace: std::env::temp_dir(),
        remote_enabled: true,
        peer_enabled: false,
        phone_enabled: false,
        peer_cmd: String::new(),
        timeout: Duration::from_secs(1),
    };
    let inline = TestRequest {
        claim: String::new(),
        cmd: "python3 -c 'import os; os.remove(\"important\")'".to_string(),
        placement: Placement::Remote("spark".to_string()),
        why: String::new(),
    };
    assert!(
        remote_router
            .approve(&inline)
            .expect_err("remote inline code must be denied")
            .contains("inline interpreter")
    );
}

#[test]
fn delegated_capture_is_bounded_and_kills_process_groups() {
    let mut noisy = Command::new("sh");
    noisy.args(["-c", "head -c 2097152 /dev/zero"]);
    let (out, timed) = run_capture(noisy, Duration::from_secs(5)).unwrap();
    assert!(!timed);
    assert!(
        out.stdout.len() <= (1 << 20) + 128,
        "capture must stay bounded (head+tail marker may add a few bytes): {}",
        out.stdout.len()
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("output bytes omitted"),
        "oversized capture must make its omitted middle explicit"
    );

    let mut hanging = Command::new("sh");
    hanging.args(["-c", "sleep 30 & wait"]);
    let start = std::time::Instant::now();
    let (_, timed) = run_capture(hanging, Duration::from_millis(50)).unwrap();
    assert!(timed);
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "grandchild-held pipes must not wedge the timeout"
    );
}

#[test]
fn non_local_placements_gated_by_their_flag() {
    let r = Router::from_env(); // env unset in test → all disabled
    for placement in [
        Placement::Remote("spark".to_string()),
        Placement::Peer("atlas".to_string()),
        Placement::Phone("math".to_string()),
    ] {
        let req = TestRequest {
            claim: String::new(),
            cmd: "cargo test".to_string(),
            placement,
            why: String::new(),
        };
        assert!(r.approve(&req).is_err());
    }
}

#[test]
fn phone_targets_resolve() {
    let _guard = crate::tests::env_lock();
    let _deep_phone = crate::tests::TestEnvGuard::unset("ANGEL_PHONE_DEEPSEEK_URL");
    let _deep_bag = crate::tests::TestEnvGuard::unset("ANGEL_DEEPSEEK_URL");
    let _deep_phone_model = crate::tests::TestEnvGuard::unset("ANGEL_PHONE_DEEPSEEK_MODEL");
    let _deep_bag_model = crate::tests::TestEnvGuard::unset("ANGEL_DEEPSEEK_MODEL");
    let _spark_phone = crate::tests::TestEnvGuard::unset("ANGEL_PHONE_SPARK_URL");
    let _spark_bag = crate::tests::TestEnvGuard::unset("ANGEL_SPARK_URL");
    let _spark_phone_model = crate::tests::TestEnvGuard::unset("ANGEL_PHONE_SPARK_MODEL");
    let _spark_bag_model = crate::tests::TestEnvGuard::unset("ANGEL_SPARK_MODEL");
    let _r1_phone = crate::tests::TestEnvGuard::unset("ANGEL_PHONE_SPARK_R1_URL");
    let _r1_bag = crate::tests::TestEnvGuard::unset("ANGEL_SPARK_R1_URL");
    // the default / first phone is deepseek (external SOTA API)
    let (url, model, _) = phone_target("default").expect("default resolves");
    assert!(
        url.contains("deepseek"),
        "default should be deepseek: {url}"
    );
    assert!(model.contains("deepseek"));
    assert!(phone_target("deepseek").is_some());
    assert!(phone_target("math").is_some()); // alias → deepseek
    assert!(phone_target("code").is_none());
    assert!(phone_target("spark-r1").is_none());

    let _configured =
        crate::tests::TestEnvGuard::set("ANGEL_PHONE_SPARK_URL", "http://runner.example/v1");
    let (url, model, _) = phone_target("code").expect("explicit fleet phone resolves");
    assert_eq!(url, "http://runner.example/v1");
    assert!(model.is_empty(), "unpinned fleet models follow /models");
    // unknown + unconfigured → None
    assert!(phone_target("totally-unknown-xyz").is_none());
}

#[test]
fn ssh_targets_have_no_compiled_identity_or_endpoint() {
    let _guard = crate::tests::env_lock();
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_SWARM_SSH_SPARK");
    assert!(ssh_target("spark").is_none());

    let _configured =
        crate::tests::TestEnvGuard::set("ANGEL_SWARM_SSH_SPARK", "runner@gpu.example");
    assert_eq!(ssh_target("spark").as_deref(), Some("runner@gpu.example"));
}

#[test]
#[ignore = "spawns a sandboxed subprocess (landlock); run with --ignored"]
fn local_execution_runs_and_captures() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping local execution test");
        return;
    }
    let r = Router::from_env();
    let req = TestRequest {
        claim: "echo round-trips".to_string(),
        cmd: "echo swarm-ok && true".to_string(),
        placement: Placement::Local,
        why: String::new(),
    };
    let res = r.run(&req);
    assert_eq!(res.verdict, "pass", "detail: {}", res.detail);
    assert!(res.detail.contains("swarm-ok"), "detail: {}", res.detail);
}

#[test]
#[ignore = "ssh round-trip to spark; run with --ignored"]
fn remote_spark_executes() {
    let _guard = crate::tests::env_lock();
    if ssh_target("spark").is_none() {
        eprintln!("ANGEL_SWARM_SSH_SPARK is absent; skipping live remote contract");
        return;
    }
    let _remote = crate::tests::TestEnvGuard::set("ANGEL_SWARM_REMOTE", "1");
    let r = Router::from_env();
    let req = TestRequest {
        claim: "spark has a cargo toolchain".to_string(),
        cmd: "cargo --version".to_string(),
        placement: Placement::Remote("spark".to_string()),
        why: "confirm the remote runner".to_string(),
    };
    let res = r.run(&req);
    eprintln!("remote:spark → [{}] {}", res.verdict, res.detail);
    assert_eq!(res.verdict, "pass", "detail: {}", res.detail);
    assert!(res.detail.contains("cargo"), "detail: {}", res.detail);
}

#[test]
#[ignore = "phones the live DeepSeek API (needs DEEPSEEK_API_KEY); run with --ignored"]
fn phone_deepseek_answers() {
    let _guard = crate::tests::env_lock();
    if std::env::var_os("DEEPSEEK_API_KEY").is_none() {
        eprintln!("DEEPSEEK_API_KEY is absent; skipping live phone contract");
        return;
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SWARM_PHONE", "1") };
    let r = Router::from_env();
    let req = TestRequest {
        claim: "17 * 23 = 391".to_string(),
        cmd: "What is 17 * 23? Reply with just the number.".to_string(),
        placement: Placement::Phone("default".to_string()), // → deepseek
        why: "arithmetic check".to_string(),
    };
    let res = r.run(&req);
    eprintln!("phone:default(deepseek) → [{}] {}", res.verdict, res.detail);
    assert_eq!(res.verdict, "phoned", "detail: {}", res.detail);
    assert!(res.detail.contains("391"), "detail: {}", res.detail);
}
