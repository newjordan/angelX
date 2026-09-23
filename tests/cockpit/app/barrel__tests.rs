use super::*;

#[test]
fn still_worker_export_reaches_the_native_status_reader() {
    let Some(worker) = crate::tests::operator_script("runtime/still-tick.mjs") else {
        return;
    };
    let fixture = crate::tests::TestGitWorkspace::new("still-worker-roundtrip");
    let root = fixture.path();
    let barrel = root.join("barrel");
    let state = root.join("still");
    let trajectories = root.join("trajectories");
    std::fs::create_dir_all(&barrel).unwrap();
    let now = now_secs();
    let record = serde_json::json!({
        "v":1,"ts":now-86_400,"digest":"local-scored-fixture",
        "context":[{"role":"user","content":"fixture"}],"answer":"fixture answer",
        "teacher":{"club":"fixture","model":"fixture"},
        "repo":{"key":"fixture"},"score":{"quality":9,"gap":2,"value":18}
    });
    std::fs::write(
        barrel.join(format!("barrel-{}.jsonl", utc_yyyymmdd(now - 86_400))),
        format!("{record}\n"),
    )
    .unwrap();
    let result = std::process::Command::new("node")
        .arg(worker)
        .args(["--force", "--distill"])
        .env("ANGEL_STILL", "1")
        .env("ANGEL_STILL_DIR", &state)
        .env("ANGEL_BARREL_DIR", &barrel)
        .env("ANGEL_TRAJECTORY_DIR", &trajectories)
        .env("ANGEL_EXPERIENCE_LOG", root.join("ledger.jsonl"))
        .env("ANGEL_BARREL_TTL_DAYS", "7")
        .env("ANGEL_STILL_MIN_VALUE", "10")
        .env("ANGEL_STILL_STUDENT_URL", "http://127.0.0.1:1/v1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = status_text_in(&cfg_in(&barrel, 1024 * 1024), &state, now);
    assert!(text.contains("unscored 0"), "{text}");
    assert!(text.contains("gap 2.0"), "{text}");
    assert!(!text.contains("last distill never"), "{text}");
    assert_eq!(std::fs::read_dir(trajectories).unwrap().count(), 1);
    assert!(!root.join("public").exists());
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("angel-barrel-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn cfg_in(dir: &Path, max_bytes: u64) -> BarrelCfg {
    BarrelCfg {
        enabled: true,
        teachers: vec!["sota".into()],
        dir: dir.to_path_buf(),
        max_bytes,
    }
}

fn job(ts: u64, user: &str, answer: &str) -> Job {
    Job {
        ts,
        repo: serde_json::json!({ "key": "k" }),
        club: "sota".into(),
        model: Some("m-1".into()),
        history: vec![ChatMsg::system("be brief"), ChatMsg::user(user)],
        answer: answer.into(),
    }
}

#[test]
fn redaction_drops_credential_lines_and_spares_code() {
    let text = "fn keyboard_input() {}\nOPENAI_API_KEY=sk-live-abc123\nAuthorization: Bearer eyJx\nplain prose line";
    let (out, n) = redact_text(text);
    assert_eq!(n, 2, "exactly the two credential lines are redacted");
    assert!(!out.contains("sk-live-abc123"));
    assert!(!out.contains("eyJx"));
    assert!(
        out.contains("fn keyboard_input() {}"),
        "code naming 'key' without an assignment shape survives"
    );
    assert!(out.contains("plain prose line"));
}

#[test]
fn utc_dates_are_civil_correct() {
    assert_eq!(utc_yyyymmdd(0), "19700101");
    assert_eq!(utc_yyyymmdd(86_399), "19700101");
    assert_eq!(utc_yyyymmdd(86_400), "19700102");
    assert_eq!(utc_yyyymmdd(1_000_000_000), "20010909");
    assert_eq!(utc_yyyymmdd(951_782_400), "20000229", "leap day");
}

#[test]
fn process_job_appends_a_redacted_record_and_status() {
    let dir = test_dir("append");
    let cfg = cfg_in(&dir, u64::MAX);
    let mut state = WriterState::default();
    process_job(
        &cfg,
        job(
            86_400,
            "here is my API_KEY=sk-live-zzz please use it",
            "done",
        ),
        &mut state,
    );
    let shard = std::fs::read_to_string(dir.join("barrel-19700102.jsonl")).expect("shard written");
    assert!(
        !shard.contains("sk-live-zzz"),
        "planted secret never reaches disk"
    );
    assert!(shard.contains("«redacted»"));
    let rec: serde_json::Value = serde_json::from_str(shard.lines().next().unwrap()).unwrap();
    assert_eq!(rec["kind"], "capture");
    assert_eq!(rec["teacher"]["club"], "sota");
    assert_eq!(rec["msgs"], 2);
    assert_eq!(rec["answer"], "done");
    assert!(rec["digest"].as_str().unwrap().len() == 16);
    let status: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("writer-status.json")).unwrap())
            .unwrap();
    assert_eq!(status["appended"], 1);
    assert_eq!(status["redacted_lines"], 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duplicate_context_is_dropped() {
    let dir = test_dir("dupe");
    let cfg = cfg_in(&dir, u64::MAX);
    let mut state = WriterState::default();
    process_job(
        &cfg,
        job(86_400, "same question", "same answer"),
        &mut state,
    );
    process_job(
        &cfg,
        job(90_000, "same question", "same answer"),
        &mut state,
    );
    let shard = std::fs::read_to_string(dir.join("barrel-19700102.jsonl")).unwrap();
    assert_eq!(shard.lines().count(), 1, "second identical capture deduped");
    assert_eq!(state.dropped_dupe, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cap_evicts_oldest_closed_shard_first() {
    let dir = test_dir("evict");
    // Big enough for roughly one ~280-byte record, so day two forces eviction.
    let cfg = cfg_in(&dir, 300);
    let mut state = WriterState::default();
    process_job(
        &cfg,
        job(86_400, "day one question", "day one answer"),
        &mut state,
    );
    assert!(dir.join("barrel-19700102.jsonl").exists());
    process_job(
        &cfg,
        job(2 * 86_400, "day two question", "day two answer"),
        &mut state,
    );
    assert!(
        !dir.join("barrel-19700102.jsonl").exists(),
        "oldest shard evicted at cap"
    );
    assert!(
        dir.join("barrel-19700103.jsonl").exists(),
        "current shard written"
    );
    assert_eq!(state.evicted_shards, 1);
    assert_eq!(state.appended, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cap_never_evicts_todays_shard() {
    let dir = test_dir("cap-today");
    let cfg = cfg_in(&dir, 300);
    let mut state = WriterState::default();
    process_job(
        &cfg,
        job(86_400, "first question", "first answer"),
        &mut state,
    );
    process_job(
        &cfg,
        job(86_500, "second question of the day", "second answer"),
        &mut state,
    );
    assert!(
        dir.join("barrel-19700102.jsonl").exists(),
        "live shard survives cap pressure"
    );
    assert_eq!(
        state.dropped_cap, 1,
        "over-cap same-day capture dropped, not evicted"
    );
    assert_eq!(state.appended, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn teacher_arming_matches_label_or_resolved_driver() {
    let cfg = BarrelCfg {
        enabled: true,
        teachers: vec!["grok".into(), "glm".into()],
        dir: PathBuf::from("."),
        max_bytes: 1,
    };
    assert!(cfg.teacher_armed("GROK", "whatever"));
    assert!(cfg.teacher_armed("sota", "GLM"));
    assert!(!cfg.teacher_armed("openai", "codex"));
    let none = BarrelCfg {
        teachers: Vec::new(),
        ..cfg
    };
    assert!(
        !none.teacher_armed("grok", "grok"),
        "empty allowlist captures nothing"
    );
}

#[test]
fn still_status_renders_disarmed_and_armed_fixtures() {
    let dir = test_dir("status");
    let still = dir.join("still");
    // Disarmed, nothing on disk: every line degrades gracefully.
    let cfg = BarrelCfg {
        enabled: false,
        teachers: Vec::new(),
        dir: dir.clone(),
        max_bytes: 1024 * 1024,
    };
    let text = status_text_in(&cfg, &still, 1_000);
    assert!(text.contains("capture: disarmed"));
    assert!(text.contains("no captures yet"));
    assert!(text.contains("no tick export yet"));
    // Armed with both status artifacts present.
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&still).unwrap();
    std::fs::write(
        dir.join("writer-status.json"),
        serde_json::json!({
            "shards": 3, "bytes": 2 * 1024 * 1024, "max_bytes": 8 * 1024 * 1024,
            "appended": 12, "dropped_dupe": 1, "dropped_cap": 2,
            "dropped_backpressure": 0, "redacted_lines": 7, "evicted_shards": 1
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        still.join("status.json"),
        serde_json::json!({
            "ts": 900, "unscored": 5, "scored_this_tick": 4, "score_failures": 1,
            "last_distill_week": "2026-W27",
            "gap_trend": [
                { "week": "2026-W26", "mean_gap": 6.4 },
                { "week": "2026-W27", "mean_gap": null }
            ]
        })
        .to_string(),
    )
    .unwrap();
    let armed = BarrelCfg {
        enabled: true,
        teachers: vec!["grok".into()],
        ..cfg
    };
    let text = status_text_in(&armed, &still, 1_000);
    assert!(text.contains("armed · teachers: grok"));
    assert!(text.contains("3 shard(s) · 2.0 / 8 MB"));
    assert!(text.contains("dropped 3 (dupe 1, cap 2, backpressure 0)"));
    assert!(text.contains("last tick 100s ago"));
    assert!(text.contains("last distill 2026-W27"));
    assert!(text.contains("2026-W26 gap 6.4 → 2026-W27 unscored"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn still_usage_is_returned_for_unknown_args() {
    let text = run(Some("bogus"));
    assert!(text.starts_with("usage: /still"));
}

#[test]
fn from_env_defaults_are_off_and_bounded() {
    let _guard = crate::tests::env_lock();
    for k in [
        "ANGEL_BARREL",
        "ANGEL_BARREL_TEACHERS",
        "ANGEL_BARREL_DIR",
        "ANGEL_BARREL_MAX_MB",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(k) };
    }
    let cfg = BarrelCfg::from_env();
    assert!(!cfg.enabled, "capture is default-off");
    assert!(cfg.teachers.is_empty());
    assert_eq!(cfg.max_bytes, 1024 * 1024 * 1024);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_BARREL", "1") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_BARREL_TEACHERS", " Grok , glm ,") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_BARREL_MAX_MB", "2") };
    let cfg = BarrelCfg::from_env();
    assert!(cfg.enabled);
    assert_eq!(cfg.teachers, vec!["grok".to_string(), "glm".to_string()]);
    assert_eq!(cfg.max_bytes, 2 * 1024 * 1024);
    for k in [
        "ANGEL_BARREL",
        "ANGEL_BARREL_TEACHERS",
        "ANGEL_BARREL_MAX_MB",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(k) };
    }
}
