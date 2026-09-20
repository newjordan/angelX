use super::*;

#[test]
fn dossier_compiler_output_reaches_the_native_context_reader() {
    let _lock = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER", "1");
    let _belief = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER_MIN_BELIEF", "0.5");
    let _age = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_AGE_DAYS");
    let _bytes = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_BYTES");
    let temp = crate::tests::TestGitWorkspace::new("dossier-compiler");
    let workspace = temp.path().join("project");
    let cut = temp.path().join("cut");
    let out = temp.path().join("dossier");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&cut).unwrap();
    let key = artifact_key(&workspace);
    let now = now_secs();
    let rows = (1..=3)
        .map(|session| {
            serde_json::json!({
                "kind":"event", "v":3, "ts":now, "session":session, "seq":0,
                "event":"cmd", "repo":{"key":key,"root":workspace,"slug":"project"},
                "cmd":{"text":"cargo check","exit":0,"verdict":"pass",
                    "shell":"bash","pipefail":true,"source":"agent","independent":true,
                    "timed_out":false,"dur_ms":12,"tool":"shell","bytes_out":0}
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let ledger = temp.path().join("ledger.jsonl");
    std::fs::write(&ledger, rows).unwrap();
    let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/runtime/repo-dossier.mjs");
    let result = std::process::Command::new("node")
        .arg(compiler)
        .arg("--refresh")
        .arg("--ledger")
        .arg(&ledger)
        .arg("--cut")
        .arg(&cut)
        .arg("--out")
        .arg(&out)
        .current_dir(temp.path())
        .output()
        .expect("Node.js runs the dossier compiler");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(out.join(format!("{key}.json")).is_file());
    assert!(out.join("graph.json").is_file());
    let context = broker_context_block_in(&out, &workspace, now);
    assert!(context.contains("cargo check"), "{context}");
    assert!(!temp.path().join("public").exists());
}

fn artifact() -> serde_json::Value {
    serde_json::json!({
        "v": 1,
        "repo": { "key": "k", "root": "/r", "slug": "u/p" },
        "generatedAt": "2026-07-06T00:00:00.000Z",
        "facts": [
            { "kind": "ritual", "class": "test", "text": "cargo test",
              "meanDurMs": 5000, "belief": 0.82, "ageDays": 1.0,
              "evidence": "9 runs, 9 pass, 4 session(s), last 2026-07-06" },
            { "kind": "ritual", "class": "build", "text": "cargo build",
              "meanDurMs": 200, "belief": 0.55, "ageDays": 1.0,
              "evidence": "3 runs, 2 pass, 2 session(s), last 2026-07-01" },
            { "kind": "trap", "text": "npm test", "meanDurMs": null, "belief": 0.78, "ageDays": 1.0,
              "evidence": "3 runs, 0 pass (3 fail), 3 session(s), last 2026-07-05" },
        ],
        "thread": { "ts": 1_783_300_000u64, "stop": "answer", "driver": "gemma", "ok": true },
    })
}

#[test]
fn renders_confident_facts_and_withholds_the_rest() {
    let block = render_block(&artifact(), 0.70, 2048, 1_783_300_000 + 7_200);
    assert!(block.contains(DOSSIER_BLOCK_HEADER));
    assert!(block.ends_with(&format!("{DOSSIER_BLOCK_SENTINEL}\n")));
    // The confident ritual renders with its class, duration, and evidence.
    assert!(
        block.contains("test: `cargo test` (9 runs, 9 pass"),
        "{block}"
    );
    assert!(block.contains("~5s"));
    // The trap renders as a trap.
    assert!(block.contains("trap: `npm test` fails here"));
    // The 0.55 build ritual is withheld, and the block says so.
    assert!(!block.contains("cargo build"));
    assert!(block.contains("1 fact(s) below 0.70 belief withheld"));
    // The thread line is humanized.
    assert!(block.contains("last session here: 2h ago, ended with answer (gemma)"));
}

/// What the block asserts is what the agent was told to run — the input to the
/// self-confirmation guard. Rituals *and* traps count; prose lines do not.
#[test]
fn asserted_commands_are_every_command_the_block_names() {
    let block = render_block(&artifact(), 0.70, 2048, 1_783_300_000 + 7_200);
    let asserted = asserted_commands(&block);
    assert!(asserted.contains(&"cargo test".to_string()), "{asserted:?}");
    // A trap puts its command in the agent's mouth exactly as a ritual does.
    assert!(asserted.contains(&"npm test".to_string()), "{asserted:?}");
    // The withheld build ritual was never asserted, so it never contaminates.
    assert!(!asserted.contains(&"cargo build".to_string()));
    // Prose lines ("last session here: …"), the header, and the sentinel are
    // not commands — a `: ` alone must not be mistaken for a fact line.
    assert!(!asserted.iter().any(|c| c.contains("last session")));
    assert_eq!(asserted.len(), 2, "{asserted:?}");
    // Nothing asserted from an empty block.
    assert!(asserted_commands("").is_empty());
}

#[test]
fn nothing_confident_and_no_thread_renders_nothing() {
    let a = serde_json::json!({
        "facts": [ { "kind": "ritual", "class": "test", "text": "x", "belief": 0.5,
                     "evidence": "" } ],
        "thread": null,
    });
    assert_eq!(render_block(&a, 0.70, 2048, 0), "");
    assert_eq!(render_block(&serde_json::json!({}), 0.70, 2048, 0), "");
}

#[test]
fn size_cap_truncates_but_always_closes_the_block() {
    let block = render_block(&artifact(), 0.70, 96, 1_783_300_000);
    assert!(block.len() <= 96 + DOSSIER_BLOCK_HEADER.len() + 8);
    assert!(block.contains('…'));
    assert!(block.trim_end().ends_with(DOSSIER_BLOCK_SENTINEL));
}

#[test]
fn context_block_in_reads_by_workspace_key_and_survives_garbage() {
    let base = std::env::temp_dir().join(format!("angel-dossier-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let ws = Path::new("/home/u/proj");
    let key = crate::tools::work_landing::workspace_key(ws);

    // No artifact → empty.
    assert_eq!(context_block_in(&base, ws, 0), "");
    // Garbage artifact → empty, no panic.
    std::fs::write(base.join(format!("{key}.json")), "{not json").unwrap();
    assert_eq!(context_block_in(&base, ws, 0), "");
    // Real artifact → rendered block.
    std::fs::write(
        base.join(format!("{key}.json")),
        serde_json::to_string(&artifact()).unwrap(),
    )
    .unwrap();
    let block = context_block_in(&base, ws, 1_783_300_000);
    assert!(block.contains("cargo test"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn broker_recomputes_freshness_when_external_tick_stops() {
    let _guard = crate::tests::env_lock();
    let _max_age = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_AGE_DAYS");
    let _belief = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MIN_BELIEF");
    let _bytes = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_BYTES");
    let base = isolated_dir("broker-freshness");
    let ws = fake_ws("freshness");
    let path = write_workspace_artifact(&base, &ws, &artifact());
    let generated = iso_day("2026-07-06").unwrap() as u64 * 86_400;
    assert!(broker_context_block_in(&base, &ws, generated + 5 * 86_400).contains("cargo test"));
    assert_eq!(
        broker_context_block_in(&base, &ws, generated + 40 * 86_400),
        ""
    );
    let mut unverifiable = artifact();
    unverifiable.as_object_mut().unwrap().remove("generatedAt");
    unverifiable["thread"] = serde_json::Value::Null;
    std::fs::write(&path, serde_json::to_string(&unverifiable).unwrap()).unwrap();
    assert_eq!(
        broker_context_block_in(&base, &ws, generated + 5 * 86_400),
        "",
        "facts without a generation time cannot prove read-time freshness"
    );
    let _ = std::fs::remove_dir_all(base);
}

fn isolated_dir(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("angel-dossier-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn fake_ws(tag: &str) -> PathBuf {
    PathBuf::from(format!("/home/u/proj-dossier-{tag}-{}", std::process::id()))
}

fn write_workspace_artifact(dir: &Path, workspace: &Path, artifact: &serde_json::Value) -> PathBuf {
    let path = dir.join(format!("{}.json", artifact_key(workspace)));
    std::fs::write(&path, serde_json::to_string(artifact).unwrap()).unwrap();
    path
}

fn file_identity(path: &Path) -> (std::time::SystemTime, u64) {
    let meta = std::fs::metadata(path).unwrap();
    (meta.modified().unwrap(), meta.len())
}

fn overwrite_preserving_identity(path: &Path, bytes: &[u8]) {
    let (mtime, len) = file_identity(path);
    assert_eq!(len, bytes.len() as u64, "replacement must keep len");
    std::fs::write(path, bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    assert_eq!(file_identity(path), (mtime, len));
}

#[test]
fn broker_block_caches_by_mtime_len_and_unix_day() {
    let _guard = crate::tests::env_lock();
    let _max_age = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_AGE_DAYS");
    let _belief = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MIN_BELIEF");
    let _bytes = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_BYTES");
    let base = isolated_dir("broker-cache");
    let ws = fake_ws("cache");
    let generated = iso_day("2026-07-06").unwrap() as u64 * 86_400;
    let now = generated + 5 * 86_400;

    assert_eq!(
        broker_context_block_in(&base, &ws, now),
        "",
        "missing file → empty"
    );

    let path = write_workspace_artifact(&base, &ws, &artifact());
    let first = broker_context_block_in(&base, &ws, now);
    assert!(first.contains("cargo test"), "{first}");

    let original = std::fs::read(&path).unwrap();
    let mut garbage = vec![b'x'; original.len()];
    let last = garbage.len() - 1;
    garbage[0] = b'{';
    garbage[last] = b'}';
    overwrite_preserving_identity(&path, &garbage);
    let hit = broker_context_block_in(&base, &ws, now);
    assert_eq!(
        hit, first,
        "unchanged mtime/len must keep the cached render even if bytes changed"
    );
    assert_eq!(
        broker_context_block_in(&base, &ws, now + 86_400),
        "",
        "unix-day is part of the key: next day re-reads the now-unparseable file"
    );

    let mut updated = artifact();
    updated["facts"][0]["text"] = serde_json::json!("pytest -q");
    std::fs::write(&path, serde_json::to_string(&updated).unwrap()).unwrap();
    let bumped = file_identity(&path).0 + std::time::Duration::from_secs(2);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(bumped)
        .unwrap();
    let busted = broker_context_block_in(&base, &ws, now);
    assert!(
        busted.contains("pytest -q"),
        "mtime/len change must re-read: {busted}"
    );
    assert!(
        !busted.contains("cargo test"),
        "stale cached ritual must not survive an identity change: {busted}"
    );

    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        broker_context_block_in(&base, &ws, now),
        "",
        "deleted artifact must not leak a previous cache"
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn broker_block_disabled_does_not_leak_cache() {
    let _guard = crate::tests::env_lock();
    let _max_age = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER_MAX_AGE_DAYS", "36500");
    let _belief = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MIN_BELIEF");
    let _bytes = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER_MAX_BYTES");
    let _on = crate::tests::TestEnvGuard::unset("ANGEL_DOSSIER");
    let base = isolated_dir("broker-disabled");
    let dir_s = base.to_str().expect("utf8 temp dir").to_string();
    let _dir = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER_DIR", &dir_s);
    let ws = fake_ws("disabled");
    write_workspace_artifact(&base, &ws, &artifact());

    let live = broker_context_block(&ws);
    assert!(
        live.contains("cargo test"),
        "enabled path must populate a cached block: {live}"
    );
    let _off = crate::tests::TestEnvGuard::set("ANGEL_DOSSIER", "0");
    assert_eq!(
        broker_context_block(&ws),
        "",
        "ANGEL_DOSSIER=0 must not return a previously cached block"
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn build_ritual_is_picked_by_belief_and_never_a_test_command() {
    // The best-believed fact in this artifact is `cargo test` (0.82). The
    // Cut runs its verify on EVERY write, so a test ritual must never be
    // eligible however confident it is — only the `build` class is.
    assert_eq!(
        pick_build_ritual(&artifact(), 0.50).as_deref(),
        Some("cargo build")
    );
    // Below the belief threshold the repo has taught us nothing usable, and
    // the caller falls through to the project-type default.
    assert_eq!(pick_build_ritual(&artifact(), 0.70), None);
    // No artifact / no facts → nothing.
    assert_eq!(pick_build_ritual(&serde_json::json!({}), 0.0), None);
}

#[test]
fn ago_is_humane() {
    assert_eq!(ago(1_000_000, 1_000_000 - 30), "1m ago");
    assert_eq!(ago(1_000_000, 1_000_000 - 1_800), "30m ago");
    assert_eq!(ago(1_000_000, 1_000_000 - 7_200), "2h ago");
    assert_eq!(ago(1_000_000, 1_000_000 - 300_000), "3d ago");
}
