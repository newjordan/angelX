use super::*;

#[test]
fn sha256_matches_the_known_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    // Two blocks (>55 bytes) — exercises the padding path.
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn authored_units_read_each_mutation_tool() {
    let write = serde_json::json!({ "path": "a.rs", "content": "fn main() {}" });
    assert_eq!(
        authored_units("write_file", &write),
        vec![("a.rs".to_string(), "fn main() {}".to_string())]
    );
    let replace = serde_json::json!({ "path": "a.rs", "old": "x", "new": "y" });
    assert_eq!(
        authored_units("str_replace", &replace),
        vec![("a.rs".to_string(), "y".to_string())]
    );
    let multi = serde_json::json!({
        "path": "a.rs",
        "edits": [{ "old": "x", "new": "one" }, { "old": "y", "new": "two" }],
    });
    assert_eq!(
        authored_units("multi_edit", &multi),
        vec![("a.rs".to_string(), "one\ntwo".to_string())]
    );
    // A read is not a mutation.
    assert!(authored_units("read_file", &write).is_empty());
}

#[test]
fn patch_units_keep_only_the_added_text_per_file() {
    let unified = "--- a/src/one.rs\n+++ b/src/one.rs\n@@ -1,2 +1,3 @@\n ctx\n-gone\n+kept\n+also\n\
                       --- a/src/two.rs\n+++ b/src/two.rs\n@@ -1 +1 @@\n+second\n";
    let units = authored_units("apply_patch", &serde_json::json!({ "diff": unified }));
    assert_eq!(
        units,
        vec![
            ("src/one.rs".to_string(), "kept\nalso".to_string()),
            ("src/two.rs".to_string(), "second".to_string()),
        ]
    );
    // The Codex freeform envelope, the other dialect apply_patch accepts.
    let freeform = "*** Begin Patch\n*** Add File: new.rs\n+fn main() {}\n*** End Patch\n";
    assert_eq!(
        authored_units("apply_patch", &serde_json::json!({ "diff": freeform })),
        vec![("new.rs".to_string(), "fn main() {}".to_string())]
    );
}

#[test]
fn mutation_targets_skip_authored_bodies() {
    let hunk = "x".repeat(80_000);
    let write = serde_json::json!({ "path": "src/kernel.cu", "content": hunk });
    assert_eq!(
        mutation_targets("write_file", &write),
        vec!["src/kernel.cu".to_string()]
    );
    assert_eq!(
        authored_units("write_file", &write)[0].1.len(),
        80_000,
        "authored capture still keeps the body"
    );
    assert!(
        mutation_targets("write_file", &serde_json::json!({ "path": "src/a.rs" })).is_empty(),
        "write without content is not a settle target"
    );

    let diff = format!(
        "*** Begin Patch\n*** Update File: src/kernel.cu\n@@\n-{hunk}\n+{hunk}\n*** End Patch\n"
    );
    let args = serde_json::json!({ "diff": diff });
    assert_eq!(
        mutation_targets("apply_patch", &args),
        vec!["src/kernel.cu".to_string()]
    );
    let mut visited = Vec::new();
    for_each_mutation_target_path("apply_patch", &args, |path| {
        visited.push(path.to_string());
        false
    });
    assert_eq!(visited, vec!["src/kernel.cu".to_string()]);

    let repeated = "*** Update File: src/a.rs\n+one\n*** Update File: src/b.rs\n+two\n\
                        *** Update File: src/a.rs\n+three\n";
    assert_eq!(
        mutation_targets("apply_patch", &serde_json::json!({ "diff": repeated })),
        vec!["src/a.rs".to_string(), "src/b.rs".to_string()]
    );
    assert_eq!(
        mutation_targets(
            "apply_patch",
            &serde_json::json!({ "diff": "*** Delete File: src/gone.rs\n" })
        ),
        vec!["src/gone.rs".to_string()],
        "delete-only patches still mutate their declared target"
    );
    assert_eq!(
        mutation_targets(
            "apply_patch",
            &serde_json::json!({
                "diff": "*** Begin Patch\n[src/hash.rs#deadbeef]\nDEL 3\n*** End Patch\n"
            })
        ),
        vec!["src/hash.rs".to_string()],
        "hashline sections expose their target before mutation"
    );
}

#[test]
fn sealed_task_edit_scope_rejects_out_of_scope_file_tools_before_mutation() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-cut-edit-scope-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("submission")).unwrap();
    let _scope = crate::tests::TestEnvGuard::set(TASK_EDITABLE_PATHS_ENV, r#"["submission"]"#);
    let _cut = crate::tests::TestEnvGuard::set("ANGEL_CUT", "0");
    let tool = capture_writes(
        Box::new(crate::harness::WriteFileTool { root: root.clone() }),
        root.clone(),
    );

    tool.call(&serde_json::json!({
        "path": "submission/best.heesch",
        "content": "allowed"
    }))
    .unwrap();
    let error = tool
        .call(&serde_json::json!({
            "path": "improve_defect.py",
            "content": "forbidden"
        }))
        .unwrap_err();
    assert!(error.contains("out-of-scope"), "{error}");
    assert!(!root.join("improve_defect.py").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("submission/best.heesch")).unwrap(),
        "allowed"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cut_persistence_scrubs_metadata_without_rewriting_authored_identity() {
    let _lock = crate::tests::env_lock();
    let secret = "fixture-cut-\"credential\"\\0123456789";
    let _key = crate::tests::TestEnvGuard::set("ANGEL_T_CUT_SECRET", secret);
    let dir = std::env::temp_dir().join(format!("angel-cut-redact-{}", std::process::id()));
    let _dir = crate::tests::TestEnvGuard::set("ANGEL_CUT_DIR", dir.to_str().unwrap());
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_CUT", "1");
    let authored = "hf_abcdefghijklmnopqrstuvwxyz01234567\nlet valid = 1;";
    let mut record = authored_record(
        "write_file",
        "src/a.rs",
        authored,
        &serde_json::json!({"key": "fixture"}),
        "fixture",
        86_400,
        1,
    );
    record["diagnostic"] = serde_json::json!({"text": secret, "exit": 0, "ok": true});
    append_row(&record);
    let body = std::fs::read_to_string(dir.join("authored-19700102.jsonl")).unwrap();
    let saved: Value = serde_json::from_str(body.trim()).unwrap();
    assert_eq!(saved["authored_sha256"], sha256_hex(authored.as_bytes()));
    assert_eq!(saved["authored_bytes"], authored.len());
    assert!(!saved["authored"].as_str().unwrap().contains("hf_"));
    assert_eq!(saved["diagnostic"]["text"], "«redacted:ANGEL_T_CUT_SECRET»");
    assert_eq!(saved["diagnostic"]["exit"], 0);
    assert_eq!(saved["diagnostic"]["ok"], true);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn authored_record_redacts_and_stamps_the_digest_of_the_full_text() {
    let authored = "let ok = 1;\nAPI_KEY=sk-live-should-never-land\nlet done = 2;\n";
    let rec = authored_record(
        "write_file",
        "src/a.rs",
        authored,
        &serde_json::json!({ "key": "k", "root": "/w", "slug": null }),
        "sota-moa",
        1_783_820_764,
        12,
    );
    let text = rec["authored"].as_str().unwrap();
    assert!(
        !text.contains("sk-live"),
        "credential line survived: {text}"
    );
    assert!(text.contains("«redacted»"));
    assert_eq!(rec["redacted_lines"], 1);
    // The digest and byte count describe what was WRITTEN, not the redacted copy.
    assert_eq!(rec["authored_sha256"], sha256_hex(authored.as_bytes()));
    assert_eq!(rec["authored_bytes"], authored.len());
    assert_eq!(rec["kind"], "authored");
    assert_eq!(rec["tool"], "write_file");
    assert_eq!(rec["path"], "src/a.rs");
    assert_eq!(rec["driver"], "sota-moa");
    assert_eq!(rec["repo"]["key"], "k");
    assert!(rec.get("authored_truncated").is_none());
}

#[test]
fn oversized_authored_text_is_capped_and_flagged() {
    let big = "x".repeat(AUTHORED_MAX_BYTES + 10);
    let rec = authored_record(
        "write_file",
        "big.rs",
        &big,
        &serde_json::json!({}),
        "single",
        0,
        1,
    );
    assert_eq!(rec["authored_truncated"], true);
    assert_eq!(rec["authored_bytes"], big.len());
    assert_eq!(rec["authored"].as_str().unwrap().len(), AUTHORED_MAX_BYTES);
}

#[test]
fn source_kind_admits_code_and_refuses_prose() {
    assert_eq!(source_kind("cockpit/src/cut.rs"), Some("rust"));
    assert_eq!(source_kind("cockpit/Cargo.toml"), Some("rust"));
    assert_eq!(source_kind("scripts/tick.mjs"), Some("js"));
    assert_eq!(source_kind("web/app.tsx"), Some("ts"));
    assert_eq!(source_kind("sidecar/run.py"), Some("py"));
    // A docs edit does not earn a compile.
    assert_eq!(source_kind("docs/plans/the-cut.md"), None);
    assert_eq!(source_kind("public/data/causal-graph.json"), None);
}

#[test]
fn machine_json_and_inline_note_only_speak_on_failure() {
    let pass = Machine::Ran {
        cmd: "cargo check".into(),
        dir: "cockpit".into(),
        source: "default",
        exit: Some(0),
        timed_out: false,
        dur_ms: 3411,
        err: String::new(),
    };
    assert!(pass.passed());
    assert_eq!(pass.inline_note(), None);
    let json = pass.to_json();
    assert_eq!(json["cmd"], "cargo check");
    assert_eq!(json["exit"], 0);
    assert_eq!(json["dur_ms"], 3411);

    let fail = Machine::Ran {
        cmd: "cargo check".into(),
        dir: "cockpit".into(),
        source: "default",
        exit: Some(101),
        timed_out: false,
        dur_ms: 900,
        err: "error[E0425]: cannot find value `nope`".into(),
    };
    assert!(!fail.passed());
    let note = fail.inline_note().expect("a failure speaks up");
    assert!(note.contains("post-write verify"));
    assert!(note.contains("E0425"));
    assert!(note.contains("fix this before continuing"));
    assert_eq!(fail.to_json()["exit"], 101);

    // A skip is recorded (so the tick can see why there is no verdict) but
    // never editorializes into the turn.
    let skipped = Machine::Skipped {
        reason: "not-source",
    };
    assert!(!skipped.passed());
    assert_eq!(skipped.inline_note(), None);
    assert_eq!(skipped.to_json()["skipped"], "not-source");
}

#[test]
fn timeout_reports_itself_as_a_non_verdict() {
    let slow = Machine::Ran {
        cmd: "cargo check".into(),
        dir: ".".into(),
        source: "default",
        exit: None,
        timed_out: true,
        dur_ms: 60_000,
        err: String::new(),
    };
    assert!(!slow.passed());
    let note = slow.inline_note().unwrap();
    assert!(note.contains("timed out"));
    // It must not read as "your code is broken".
    assert!(!note.contains("fix this"));
    assert_eq!(slow.to_json()["timed_out"], true);
}

/// A verdict from a crate-wide check (`cargo check` in `cockpit`) — the key
/// every write into that crate shares.
fn crate_check(exit: i32) -> Machine {
    Machine::Ran {
        cmd: "cargo check".into(),
        dir: "cockpit".into(),
        source: "default",
        exit: Some(exit),
        timed_out: false,
        dur_ms: 1200,
        err: if exit == 0 {
            String::new()
        } else {
            "error[E0425]: cannot find value `nope`".into()
        },
    }
}

/// A file-scoped check — its own key, because the file rides in the command.
fn file_check(file: &str, exit: i32) -> Machine {
    Machine::Ran {
        cmd: format!("node --check '{file}'"),
        dir: ".".into(),
        source: "default",
        exit: Some(exit),
        timed_out: false,
        dur_ms: 40,
        err: String::new(),
    }
}

fn reward_of(machines: &[Machine]) -> Option<f32> {
    let mut v = TurnVerdicts::default();
    for m in machines {
        v.observe(m);
    }
    v.reward()
}

#[test]
fn a_turn_that_wrote_no_source_is_never_labeled() {
    // The rule that protects the corpus: a turn with nothing to verify gets
    // no reward. A fabricated label is worse than no label.
    assert_eq!(reward_of(&[]), None, "a turn with no writes at all");
    assert_eq!(
        reward_of(&[
            Machine::Skipped {
                reason: "not-source"
            },
            Machine::Skipped {
                reason: "no-verify-command"
            },
        ]),
        None,
        "a docs/data turn earns no verdict, so it earns no reward"
    );
    // A verify that outran its deadline is explicitly NOT "your code is
    // broken" — it is no answer at all, and must not become a 0.0.
    assert_eq!(
        reward_of(&[Machine::Ran {
            cmd: "cargo check".into(),
            dir: "cockpit".into(),
            source: "default",
            exit: None,
            timed_out: true,
            dur_ms: 60_000,
            err: String::new(),
        }]),
        None,
        "a timeout is not a verdict"
    );
}

#[test]
fn a_clean_turn_scores_one() {
    assert_eq!(reward_of(&[crate_check(0)]), Some(1.0));
    assert_eq!(
        reward_of(&[crate_check(0), crate_check(0), file_check("a.js", 0)]),
        Some(1.0),
        "several writes, nothing broken"
    );
    // Skips ride along without diluting: they are not labels.
    assert_eq!(
        reward_of(&[
            crate_check(0),
            Machine::Skipped {
                reason: "not-source"
            },
        ]),
        Some(1.0),
        "a docs edit alongside a green code edit must not dilute the reward"
    );
}

#[test]
fn broke_then_repaired_lands_strictly_between_the_walk_away_and_the_clean_turn() {
    // THE case this whole reward exists to capture: angel broke its own
    // build and fixed it inside the same turn. The ledger contains two of
    // these, ever.
    let repaired = reward_of(&[crate_check(101), crate_check(0)]).unwrap();
    assert_eq!(repaired, 0.5);

    // Cross-file repair: the crate check goes red on a write to a.rs and
    // green after a write to b.rs (the caller was what needed fixing). Same
    // check, so the newer word supersedes — this is a recovery, not an
    // abandoned file.
    assert_eq!(
        reward_of(&[crate_check(101), crate_check(0)]),
        Some(0.5),
        "a repair that fixed the caller is still a repair"
    );

    // Thrash is priced: three breaks before the fix scores worse than one…
    let thrashed = reward_of(&[
        crate_check(101),
        crate_check(101),
        crate_check(101),
        crate_check(0),
    ])
    .unwrap();
    assert_eq!(thrashed, 0.25);
    assert!(thrashed < repaired);

    // …but no amount of thrash can push a recovery down to the walk-away's
    // 0.0, or the policy learns to abandon a broken build rather than keep
    // fixing it. And no recovery reaches 1.0, or breaking the build is free.
    for breaks in 1..25 {
        let mut runs: Vec<Machine> = (0..breaks).map(|_| crate_check(101)).collect();
        runs.push(crate_check(0));
        let r = reward_of(&runs).unwrap();
        assert!(
            r > 0.0,
            "{breaks} breaks then a fix scored {r}, at the floor"
        );
        assert!(
            r < 1.0,
            "{breaks} breaks then a fix scored {r}, a clean turn"
        );
    }
}

#[test]
fn a_turn_that_left_a_check_red_scores_zero() {
    assert_eq!(reward_of(&[crate_check(101)]), Some(0.0));
    // Earlier successes do not buy off a broken build left at the end.
    assert_eq!(
        reward_of(&[crate_check(0), crate_check(0), crate_check(101)]),
        Some(0.0),
        "three writes, the last one red: the project does not build"
    );
    // Mixed checks: b.js is green, but a.js was left broken and never
    // revisited. The turn still shipped a broken file.
    assert_eq!(
        reward_of(&[file_check("a.js", 1), file_check("b.js", 0)]),
        Some(0.0),
        "a green sibling file cannot mask the one left red"
    );
    // …and repairing a.js afterwards rescues it into the repair band.
    assert_eq!(
        reward_of(&[
            file_check("a.js", 1),
            file_check("b.js", 0),
            file_check("a.js", 0),
        ]),
        Some(2.0 / 3.0),
    );
}

#[test]
#[cfg(target_os = "linux")]
fn cancelled_post_write_verifier_reaps_tree_without_reward_or_backoff() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let root = std::env::temp_dir().join(format!(
        "angel-cut-cancel-{}-{}",
        std::process::id(),
        next_seq(),
    ));
    std::fs::create_dir_all(&root).unwrap();
    let command = "sleep 30 & echo $! > child.pid; echo $$ > verifier.pid; wait";
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let machine = std::thread::scope(|scope| {
        scope.spawn(|| {
            let until = Instant::now() + Duration::from_secs(2);
            while !root.join("verifier.pid").exists() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
            cancel.store(true, Ordering::Release);
        });
        run_verify(
            &root,
            VerifyPlan {
                cmd: command.into(),
                dir: ".".into(),
                source: "env",
            },
            Some(&cancel),
        )
    });
    assert!(matches!(
        machine,
        Machine::Skipped {
            reason: "cancelled"
        }
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        reward_of(&[machine]),
        None,
        "cancellation is not a code failure"
    );
    let key = format!("{}|.|{command}", root.display());
    assert!(!backoff().lock().unwrap().contains_key(&key));
    for file in ["verifier.pid", "child.pid"] {
        let pid: u32 = std::fs::read_to_string(root.join(file))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let live = || {
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .ok()
                .and_then(|stat| {
                    stat.rsplit_once(") ")
                        .map(|(_, fields)| !fields.starts_with('Z'))
                })
                .unwrap_or(false)
        };
        let until = Instant::now() + Duration::from_secs(2);
        while live() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !live(),
            "cancelled verifier left {file} process {pid} running"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn python_post_write_check_preserves_inventory_and_does_not_execute_source() {
    let _lock = crate::tests::env_lock();
    if !Command::new("python3")
        .args(["-I", "-S", "-B", "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "angel-cut-python-inventory-{}-{}",
        std::process::id(),
        next_seq(),
    ));
    std::fs::create_dir_all(&root).unwrap();
    let target = "sample '99 errors'.py";
    let source = "from pathlib import Path\nPath('MODULE_EXECUTED').write_text('bad')\n";
    std::fs::write(root.join(target), source).unwrap();
    for name in ["pathlib.py", "sitecustomize.py", "usercustomize.py"] {
        std::fs::write(
            root.join(name),
            "raise RuntimeError('startup or module shadow executed')\n",
        )
        .unwrap();
    }
    std::fs::write(
        root.join("startup.py"),
        "open('STARTUP_EXECUTED', 'w').write('bad')\n",
    )
    .unwrap();
    let _python_path = crate::tests::TestEnvGuard::set("PYTHONPATH", &root.to_string_lossy());
    let _startup = crate::tests::TestEnvGuard::set(
        "PYTHONSTARTUP",
        &root.join("startup.py").to_string_lossy(),
    );
    let inventory = || {
        let mut entries = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(
                    entry.file_type().unwrap().is_file(),
                    "checker created an unexpected directory"
                );
                (entry.file_name(), std::fs::read(entry.path()).unwrap())
            })
            .collect::<Vec<_>>();
        entries.sort();
        entries
    };
    let before = inventory();
    match run_verify(&root, default_plan(&root, target).unwrap(), None) {
        Machine::Ran {
            exit, timed_out, ..
        } => {
            assert_eq!(exit, Some(0));
            assert!(!timed_out);
        }
        _ => panic!("real Python syntax check should run"),
    }
    assert_eq!(inventory(), before, "syntax check changed the fixture");
    std::fs::write(root.join(target), "def broken(:\n").unwrap();
    let before_invalid = inventory();
    match run_verify(&root, default_plan(&root, target).unwrap(), None) {
        Machine::Ran {
            exit,
            timed_out,
            err,
            ..
        } => {
            assert_ne!(exit, Some(0));
            assert!(!timed_out);
            assert!(err.contains("SyntaxError"), "{err}");
        }
        _ => panic!("invalid Python syntax must retain its failing verdict"),
    }
    assert_eq!(
        inventory(),
        before_invalid,
        "failed check changed the fixture"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn verify_resolution_follows_the_plans_precedence() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-cut-resolve-{}", std::process::id()));
    let _ = std::fs::create_dir_all(root.join("cockpit/src"));
    let _ = std::fs::write(root.join("cockpit/Cargo.toml"), "[package]\n");

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CUT_VERIFY") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_DOSSIER", "0") }; // no learned ritual in play

    // Default: the nearest enclosing crate, NOT the workspace root (this
    // repo's Cargo.toml lives one level down — a root-only probe verifies
    // nothing).
    let targets = vec!["cockpit/src/cut.rs".to_string()];
    match resolve_verify(&root, &targets) {
        Verify::Run(plan) => {
            assert_eq!(plan.cmd, "cargo check");
            assert_eq!(plan.dir, "cockpit");
            assert_eq!(plan.source, "default");
        }
        _ => panic!("a .rs edit inside a crate must resolve a check"),
    }

    // Prose is skipped outright.
    assert!(matches!(
        resolve_verify(&root, &["docs/x.md".to_string()]),
        Verify::Skip("not-source")
    ));

    // The env override wins, and runs at the workspace root.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CUT_VERIFY", "make check") };
    match resolve_verify(&root, &targets) {
        Verify::Run(plan) => {
            assert_eq!(plan.cmd, "make check");
            assert_eq!(plan.dir, ".");
            assert_eq!(plan.source, "env");
        }
        _ => panic!("the env override must win"),
    }

    // ANGEL_CUT_VERIFY=0 is the documented opt-out.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CUT_VERIFY", "0") };
    assert!(matches!(resolve_verify(&root, &targets), Verify::Off));

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CUT_VERIFY") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_DOSSIER") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cargo_rituals_anchor_at_the_edited_files_crate() {
    let root = std::env::temp_dir().join(format!("angel-cut-ritual-{}", std::process::id()));
    let _ = std::fs::create_dir_all(root.join("cockpit/src"));
    let _ = std::fs::write(root.join("cockpit/Cargo.toml"), "[package]\n");

    // The first dogfood run: a "cargo check" ritual fired at the workspace
    // root of a repo whose crate lives in cockpit/ — five phantom reds.
    assert_eq!(
        ritual_dir("cargo check", &root, "cockpit/src/cut.rs"),
        "cockpit"
    );
    // Non-cargo rituals keep the root, where repo-level tools live.
    assert_eq!(ritual_dir("make check", &root, "cockpit/src/cut.rs"), ".");
    // No manifest anywhere: fall back to the root rather than skipping.
    assert_eq!(ritual_dir("cargo check", &root, "scripts/x.rs"), ".");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn environment_failures_never_convict_the_write() {
    assert!(environment_failure(
        Some(101),
        "error: could not find `Cargo.toml` in `/w` or any parent directory"
    ));
    assert!(environment_failure(Some(127), ""));
    assert!(environment_failure(
        Some(2),
        "sh: 1: tsc: command not found"
    ));
    // A real diagnostic stays a real verdict.
    assert!(!environment_failure(
        Some(101),
        "error[E0308]: mismatched types"
    ));
    assert!(!environment_failure(Some(1), "test failed: expected 3"));
}

#[test]
fn cut_is_on_by_default_and_opts_out_by_env() {
    let _lock = crate::tests::env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CUT") };
    assert!(CutCfg::from_env().enabled);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CUT", "0") };
    assert!(!CutCfg::from_env().enabled);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CUT") };
}

fn isolated_cut_dir(name: &str) -> (std::path::PathBuf, crate::tests::TestEnvGuard) {
    let dir = std::env::temp_dir().join(format!("angel-cut-t4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.to_string_lossy().into_owned();
    let guard = crate::tests::TestEnvGuard::set("ANGEL_CUT_DIR", &path);
    (dir, guard)
}

#[test]
fn cut_help_prints_usage() {
    let _lock = crate::tests::env_lock();
    let text = status_text(Some("help"), Path::new("/tmp/ws"));
    assert!(text.starts_with("usage: /cut"), "{text}");
    assert!(
        !text.contains("authored:"),
        "help must not scan the manifest: {text}"
    );
}

#[test]
fn empty_dir_reports_density_zero() {
    let _lock = crate::tests::env_lock();
    let (dir, _cut_dir) = isolated_cut_dir("empty");
    let _on = crate::tests::TestEnvGuard::unset("ANGEL_CUT");
    let text = status_text(None, Path::new("/tmp/ws"));
    assert!(text.contains("signal density is zero"), "{text}");
    assert!(text.contains("authored: 0"), "{text}");
    assert!(text.contains("labeled 0"), "{text}");
    assert!(text.contains("judged corpus is empty"), "{text}");
    assert!(text.contains("driver: (empty)"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_dir_reports_density_zero() {
    let _lock = crate::tests::env_lock();
    let (dir, _cut_dir) = isolated_cut_dir("missing");
    let _ = std::fs::remove_dir_all(&dir);
    let _on = crate::tests::TestEnvGuard::unset("ANGEL_CUT");
    let text = status_text(None, Path::new("/tmp/ws"));
    assert!(text.contains("missing"), "{text}");
    assert!(text.contains("signal density is zero"), "{text}");
    assert!(text.contains("authored: 0"), "{text}");
}

#[test]
fn seeded_shard_excludes_skipped_and_timeout_from_labeled_denominator() {
    let _lock = crate::tests::env_lock();
    let (dir, _cut_dir) = isolated_cut_dir("seeded");
    let _on = crate::tests::TestEnvGuard::unset("ANGEL_CUT");
    let shard = dir.join("authored-20260711.jsonl");
    std::fs::write(
            &shard,
            concat!(
                r#"{"v":1,"ts":1783820764,"session":1,"seq":1,"path":"src/a.rs","authored":"SECRET_BODY_MUST_NOT_LEAK","driver":"sota-moa","repo":{"key":"k"},"machine":{"cmd":"cargo check","exit":0,"timed_out":false}}"#,
                "\n",
                r#"{"v":1,"ts":1783820765,"session":1,"seq":2,"path":"docs/x.md","authored":"docs","driver":"sota-moa","repo":{"key":"k"},"machine":{"skipped":"not-source"}}"#,
                "\n",
                r#"{"v":1,"ts":1783820766,"session":1,"seq":3,"path":"src/b.rs","authored":"slow","driver":"single","repo":{"key":"k"},"machine":{"cmd":"cargo check","exit":null,"timed_out":true}}"#,
                "\n",
                r#"{"v":1,"ts":1783820767,"session":1,"seq":4,"path":"src/c.rs","authored":"broken","driver":"single","repo":{"key":"k"},"machine":{"cmd":"cargo check","exit":101,"timed_out":false}}"#,
                "\n",
            ),
        )
        .unwrap();
    let text = status_text(Some("ignored-arg"), Path::new("/tmp/ws"));
    assert!(text.contains("authored: 4"), "{text}");
    assert!(text.contains("labeled 2"), "{text}");
    assert!(text.contains("skipped 1"), "{text}");
    assert!(text.contains("timed_out 1"), "{text}");
    assert!(text.contains("pass rate: 1/2"), "{text}");
    assert!(
        !text.contains("SECRET_BODY_MUST_NOT_LEAK"),
        "authored body leaked into the summary: {text}"
    );
    assert!(text.contains("judged corpus is empty"), "{text}");
    assert!(
        !text.starts_with("usage:"),
        "unknown args must be ignored: {text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn malformed_jsonl_line_is_skipped() {
    let _lock = crate::tests::env_lock();
    let (dir, _cut_dir) = isolated_cut_dir("malformed");
    let _on = crate::tests::TestEnvGuard::unset("ANGEL_CUT");
    std::fs::write(
        dir.join("authored-20260711.jsonl"),
        concat!(
            r#"{"v":1,"path":"src/a.rs","machine":{"exit":0,"timed_out":false}}"#,
            "\n",
            "this is not json\n",
            "{\"v\":1\n",
            "\n",
            r#"{"v":1,"path":"src/b.rs","machine":{"exit":1,"timed_out":false}}"#,
            "\n",
        ),
    )
    .unwrap();
    let text = status_text(None, Path::new("/tmp/ws"));
    assert!(text.contains("authored: 2"), "{text}");
    assert!(text.contains("labeled 2"), "{text}");
    assert!(text.contains("pass rate: 1/2"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn disabled_cut_reports_disabled() {
    let _lock = crate::tests::env_lock();
    let (dir, _cut_dir) = isolated_cut_dir("disabled");
    let _off = crate::tests::TestEnvGuard::set("ANGEL_CUT", "0");
    std::fs::write(
        dir.join("authored-20260711.jsonl"),
        r#"{"v":1,"path":"src/a.rs","machine":{"exit":0,"timed_out":false}}"#,
    )
    .unwrap();
    let text = status_text(None, Path::new("/tmp/ws"));
    assert!(text.to_ascii_lowercase().contains("disabled"), "{text}");
    assert!(text.contains("ANGEL_CUT=0"), "{text}");
    assert!(text.contains("signal density is zero"), "{text}");
    assert!(
        !text.contains("authored: 1"),
        "disabled cut must not present leftover shards as a live corpus: {text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn human_verdicts_count_when_stamped() {
    let _lock = crate::tests::env_lock();
    let (dir, _cut_dir) = isolated_cut_dir("human");
    let _on = crate::tests::TestEnvGuard::unset("ANGEL_CUT");
    std::fs::write(
            dir.join("authored-20260711.jsonl"),
            concat!(
                r#"{"v":1,"path":"src/a.rs","cut":{"verdict":"KEPT"},"machine":{"cmd":"cargo check","exit":0,"timed_out":false,"session":1}}"#,
                "\n",
                r#"{"v":1,"path":"src/b.rs","cut":{"verdict":"EDITED"}}"#,
                "\n",
                r#"{"v":1,"path":"src/c.rs","cut":{"verdict":"DISCARDED"}}"#,
                "\n",
                r#"{"v":1,"path":"src/d.rs","cut":{"verdict":"SELF-SUPERSEDED"}}"#,
                "\n",
            ),
        )
        .unwrap();
    let text = status_text(None, Path::new("/tmp/ws"));
    assert!(text.contains("KEPT 1"), "{text}");
    assert!(text.contains("EDITED 1"), "{text}");
    assert!(text.contains("DISCARDED 1"), "{text}");
    assert!(text.contains("judged 3"), "{text}");
    assert!(!text.contains("judged corpus is empty"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn adjacent_fail_then_pass_counts_as_a_repair_trajectory() {
    let _lock = crate::tests::env_lock();
    let (dir, _cut_dir) = isolated_cut_dir("repair");
    let _on = crate::tests::TestEnvGuard::unset("ANGEL_CUT");
    std::fs::write(
            dir.join("authored-20260711.jsonl"),
            concat!(
                r#"{"v":1,"ts":1,"session":9,"seq":1,"path":"src/a.rs","machine":{"cmd":"cargo check","exit":101,"timed_out":false}}"#,
                "\n",
                r#"{"v":1,"ts":2,"session":9,"seq":2,"path":"src/a.rs","machine":{"cmd":"cargo check","exit":0,"timed_out":false}}"#,
                "\n",
            ),
        )
        .unwrap();
    let text = status_text(None, Path::new("/tmp/ws"));
    assert!(text.contains("repair trajectories: 1"), "{text}");
    assert!(text.contains("pass rate: 1/2"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
