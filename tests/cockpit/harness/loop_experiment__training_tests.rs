use super::*;
use crate::agent::club::{ClubReply, StreamDelta};
use serde_json::{Value, json};
use std::sync::atomic::AtomicUsize;

const TASK: &str = "Owned native recovery task — café";
const ANSWER: &str = "Measured the isolated candidate.\n";
const GREEN: &str = "printf x >> .angel-experiment-tmp/verifier-count; printf '%s' 'test result: ok. 3 passed; 0 failed; 0 ignored;'";

struct NativeFixtureClub {
    step: AtomicUsize,
    trap: Option<PathBuf>,
}
impl Club for NativeFixtureClub {
    fn label(&self) -> &str {
        "owned-native-training-no-model"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Err("must use native tool loop".into())
    }
    fn chat_streaming(
        &self,
        _: &[ChatMsg],
        tools: &[ToolDef],
        _: &AtomicBool,
        _: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        assert!(!tools.iter().any(|tool| {
            ["tool_repair", "git_commit", "git_diff", "spawn", "delegate"]
                .contains(&tool.name.as_str())
        }));
        let step = self.step.fetch_add(1, Ordering::Relaxed);
        let (name, args) = if let Some(trap) = &self.trap {
            match step {
                0 => (
                    "write_file",
                    json!({"path":trap,"content":"forged by file tool"}),
                ),
                1 => {
                    // All paths are newly owned numeric temp fixtures. The
                    // configured filter must actually run during parent probes.
                    let command = format!(
                        "printf forged > '{}'; printf 'child candidate\\n' > candidate.txt; git config filter.ownedtrap.clean 'printf x >> .angel-experiment-tmp/filter-count; printf forged > {}; /bin/cat'; printf '*.txt filter=ownedtrap\\n' > .gitattributes; printf attempted",
                        trap.display(),
                        trap.display()
                    );
                    ("shell", json!({"command":command}))
                }
                _ => return Ok(ClubReply::Text(ANSWER.into())),
            }
        } else if step == 0 {
            (
                "write_file",
                json!({"path":"candidate.txt","content":"child candidate\n"}),
            )
        } else {
            return Ok(ClubReply::Text(ANSWER.into()));
        };
        Ok(ClubReply::Calls(vec![crate::agent::club::ToolCall {
            id: format!("native-{step}"),
            name: name.into(),
            args,
        }]))
    }
}

fn native(
    root: &Path,
    source: &Path,
    command: &str,
    trap: Option<PathBuf>,
) -> LoopExperimentResult {
    let club: Arc<dyn Club> = Arc::new(NativeFixtureClub {
        step: AtomicUsize::new(0),
        trap,
    });
    run_loop_experiment(
        LoopExperimentRequest {
            workspace: source.into(),
            artifact_dir: root.join("artifacts"),
            task: TASK.into(),
            max_hops: 8,
            deadline_secs: 30,
            token_budget: 32768,
            verify_command: Some(command.into()),
            policy_note: None,
        },
        club,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

fn row(root: &Path) -> Value {
    let bytes = std::fs::read(root.join("artifacts/training-trajectory.jsonl")).unwrap();
    let lines = bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "one actual producer row");
    serde_json::from_slice(lines[0]).unwrap()
}

fn native_logging(root: &Path) -> (crate::tests::TestEnvGuard, crate::tests::TestEnvGuard) {
    (
        crate::tests::TestEnvGuard::set("ANGEL_TRAJECTORY_LOG", "1"),
        crate::tests::TestEnvGuard::set(
            "ANGEL_TRAJECTORY_DIR",
            root.join("native-log").to_str().unwrap(),
        ),
    )
}

fn assert_native_observations(root: &Path, measured: bool, club_label: &str) {
    let path = root
        .join("native-log")
        .join(format!("session-{}.jsonl", std::process::id()));
    let log = std::fs::read_to_string(&path).expect("actual native per-process trajectory log");
    let rows: Vec<Value> = log
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        // This sink is process-wide: unrelated concurrent turns may append.
        // Keep every row from our club, including any invalid shadow row.
        .filter(|row| row["club"] == club_label)
        .collect();
    assert!(
        rows.iter()
            .any(|row| row["training_capture"]["status"] == "pending_evaluator"),
        "the actual native turn must emit a pending observation: {rows:?}"
    );
    assert_eq!(
        rows.iter()
            .any(|row| row["data_class"] == "measured_coding_eval"),
        measured
    );
    for row in rows {
        if row["data_class"] == "measured_coding_eval" {
            assert!(row["evaluator_decision_sha256"].as_str().is_some());
        } else {
            assert_eq!(
                row["data_class"], "coding_eval_observation",
                "no implicit legacy shadow row"
            );
            assert!(row.get("reward").is_none() || row["reward"].is_null());
            assert!(row["evaluator_decision_sha256"].is_null());
        }
    }
}

#[test]
fn native_training_confines_file_shell_git_filter_and_verifier_under_guarded_and_yolo() {
    // env-lock-exempt: super::tests::fixture in loop_experiment.rs holds crate::tests::env_lock for the entire closure.
    for yolo in ["0", "1"] {
        super::tests::fixture(|root, source| {
            let _native_log = native_logging(root);
            let store = root.join("authority");
            std::fs::create_dir(&store).unwrap();
            let trap = store.join("sentinel");
            std::fs::write(&trap, "unchanged").unwrap();
            let _authority = crate::tests::TestEnvGuard::set(
                "ANGEL_CODING_TRAINING_AUTHORITY_DIR",
                store.to_str().unwrap(),
            );
            let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "tests");
            let _artifact = crate::tests::TestEnvGuard::unset("ANGEL_EVALUATOR_ARTIFACT_DIR");
            let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", yolo);
            let _ambient = crate::tests::TestEnvGuard::set(
                "ANGEL_TASK_ACCEPT_CMD",
                "printf ambient > .angel-experiment-tmp/ambient-verifier",
            );
            let command = format!("printf forged > '{}'; {GREEN}", trap.display());
            let result = native(root, source, &command, Some(trap.clone()));
            assert!(result.error.is_none(), "{:?}", result.error);
            assert_eq!(result.verification.as_ref().unwrap().exit_code, Some(0));
            assert_eq!(
                result.training_capture.as_ref().unwrap()["status"],
                "captured",
                "{:?}",
                result.training_capture
            );
            assert_eq!(std::fs::read_to_string(&trap).unwrap(), "unchanged");
            let working = root.join("artifacts/working");
            assert_eq!(
                std::fs::read(working.join(".angel-experiment-tmp/verifier-count")).unwrap(),
                b"x"
            );
            assert!(
                !std::fs::read(working.join(".angel-experiment-tmp/filter-count"))
                    .unwrap()
                    .is_empty()
            );
            assert!(
                !working
                    .join(".angel-experiment-tmp/ambient-verifier")
                    .exists()
            );
            assert_eq!(
                std::fs::read_to_string(source.join("candidate.txt")).unwrap(),
                "committed\n"
            );
            let emitted = row(root);
            assert_eq!(emitted["evaluator_task"], TASK);
            assert_eq!(
                emitted["repo"]["root"],
                json!(working.canonicalize().unwrap())
            );
            assert_eq!(emitted["data_class"], "measured_coding_eval");
            assert_native_observations(root, true, "owned-native-training-no-model");
        });
    }
}

#[test]
fn relative_native_artifacts_keep_absolute_sandbox_roots_and_capture() {
    // env-lock-exempt: super::tests::fixture in loop_experiment.rs holds crate::tests::env_lock for the entire closure.
    super::tests::fixture(|root, source| {
        let _native_log = native_logging(root);
        let store = root.join("authority");
        std::fs::create_dir(&store).unwrap();
        let trap = store.join("sentinel");
        std::fs::write(&trap, "unchanged").unwrap();
        let _authority = crate::tests::TestEnvGuard::set(
            "ANGEL_CODING_TRAINING_AUTHORITY_DIR",
            store.to_str().unwrap(),
        );
        let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "tests");
        let _artifact = crate::tests::TestEnvGuard::unset("ANGEL_EVALUATOR_ARTIFACT_DIR");
        // Do not mutate process cwd in a parallel test. Reach this owned temp
        // fixture through a genuinely relative request from the test cwd.
        let cwd = std::env::current_dir().unwrap();
        let relative = |path: &Path| {
            let mut value = PathBuf::from(".");
            for part in cwd.components() {
                if matches!(part, std::path::Component::Normal(_)) {
                    value.push("..");
                }
            }
            value.push(path.strip_prefix("/").unwrap());
            value
        };
        let artifact_dir = relative(&root.join("artifacts"));
        assert!(artifact_dir.is_relative());
        let club = Arc::new(NativeFixtureClub {
            step: AtomicUsize::new(0),
            trap: Some(trap.clone()),
        });
        // This tests path/capture semantics, not host throughput. Sample the
        // same snapshot, Git and sandbox startup work before choosing a budget.
        // Use the slowest of three samples to include cold-start/load costs;
        // eight hops each get eight such work units, with the old 30s floor.
        // The scripted club itself responds synchronously without model I/O.
        let mut baseline = Duration::ZERO;
        for _ in 0..3 {
            let started = Instant::now();
            let cancel = AtomicBool::new(false);
            snapshot_live(source, None, &cancel).unwrap();
            git_output(source, &["diff", "--binary", "HEAD"], &cancel).unwrap();
            let out = crate::agent::tools::shell::ShellTool::in_dir(source.into())
                .call(&json!({"command":"printf calibration-complete", "read_only":true}))
                .unwrap();
            assert!(out.contains("calibration-complete"), "{out}");
            baseline = baseline.max(started.elapsed());
        }
        let budget = baseline.saturating_mul(8 * 8);
        let deadline_secs = budget.as_secs().saturating_add(1).max(30);
        eprintln!(
            "T06F_NATIVE_BASELINE samples=3 slowest_ms={} deadline_secs={deadline_secs}",
            baseline.as_millis()
        );
        let result = run_loop_experiment(
            LoopExperimentRequest {
                workspace: relative(source),
                artifact_dir,
                task: TASK.into(),
                max_hops: 8,
                deadline_secs,
                token_budget: 32768,
                verify_command: Some(format!("printf forged > '{}'; {GREEN}", trap.display())),
                policy_note: None,
            },
            club.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.answer, ANSWER);
        assert!(
            club.step.load(Ordering::Relaxed) >= 3,
            "actual native turn reached its answer"
        );
        assert_eq!(
            result.artifact_dir,
            root.join("artifacts").canonicalize().unwrap()
        );
        assert!(result.patch_path.as_ref().unwrap().is_absolute());
        assert_eq!(
            result.training_capture.as_ref().unwrap()["status"],
            "captured"
        );
        assert_eq!(std::fs::read_to_string(trap).unwrap(), "unchanged");
        assert_eq!(
            std::fs::read(root.join("artifacts/working/.angel-experiment-tmp/verifier-count"))
                .unwrap(),
            b"x"
        );
        assert_eq!(
            std::fs::read_to_string(source.join("candidate.txt")).unwrap(),
            "committed\n"
        );
        assert_native_observations(root, true, "owned-native-training-no-model");
    });
}

#[test]
fn native_training_keeps_generic_and_failed_verifiers_as_raw_observations() {
    // env-lock-exempt: super::tests::fixture in loop_experiment.rs holds crate::tests::env_lock for the entire closure.
    for (command, exit) in [
        ("printf 'generic useful check'", 0),
        (
            "printf '%s' 'test result: ok. 3 passed; 0 failed;'; exit 7",
            7,
        ),
    ] {
        super::tests::fixture(|root, source| {
            let _native_log = native_logging(root);
            let store = root.join("authority");
            let _authority = crate::tests::TestEnvGuard::set(
                "ANGEL_CODING_TRAINING_AUTHORITY_DIR",
                store.to_str().unwrap(),
            );
            let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "tests");
            let _artifact = crate::tests::TestEnvGuard::unset("ANGEL_EVALUATOR_ARTIFACT_DIR");
            let result = native(root, source, command, None);
            assert!(result.error.is_none(), "{:?}", result.error);
            assert_eq!(result.verification.unwrap().exit_code, Some(exit));
            assert_eq!(
                result.training_capture.unwrap()["status"],
                "unsupported_or_ineligible"
            );
            let emitted = row(root);
            assert_eq!(emitted["data_class"], "coding_eval_observation");
            assert!(emitted.get("reward").is_none() || emitted["reward"].is_null());
            assert!(emitted["evaluator_decision_sha256"].is_null());
            assert_native_observations(root, false, "owned-native-training-no-model");
        });
    }
}

#[test]
fn overlapping_or_aliased_authority_disables_capture_without_stopping_recovery() {
    // env-lock-exempt: super::tests::fixture in loop_experiment.rs holds crate::tests::env_lock for the entire closure.
    for alias in [false, true] {
        super::tests::fixture(|root, source| {
            let _native_log = native_logging(root);
            let overlap = root.join("artifacts/working/authority");
            let configured = if alias {
                let link = root.join("authority-alias");
                #[cfg(unix)]
                std::os::unix::fs::symlink(&overlap, &link).unwrap();
                link
            } else {
                overlap
            };
            let _authority = crate::tests::TestEnvGuard::set(
                "ANGEL_CODING_TRAINING_AUTHORITY_DIR",
                configured.to_str().unwrap(),
            );
            // Invalid capture configuration keeps the ordinary recovery kit;
            // use existing simple fixture Club rather than strict-kit assertion.
            let club: Arc<dyn Club> = Arc::new(super::tests::SelectedLeaf {
                calls: AtomicUsize::new(0),
                held: false,
                entered: Arc::new(AtomicBool::new(false)),
            });
            let mut request = super::tests::request(root, source);
            request.verify_command = Some("test -f candidate.txt".into());
            let result =
                run_loop_experiment(request, club, Arc::new(AtomicBool::new(false))).unwrap();
            assert!(result.error.is_none(), "{:?}", result.error);
            assert_eq!(result.verification.unwrap().exit_code, Some(0));
            assert_eq!(result.training_capture.unwrap()["status"], "disabled");
            assert!(!root.join("artifacts/training-trajectory.jsonl").exists());
            assert_native_observations(root, false, "owned-glm-route");
        });
    }
}

#[test]
fn typed_recovery_evaluator_cancel_retains_execution_and_never_labels() {
    // env-lock-exempt: super::tests::fixture in loop_experiment.rs holds crate::tests::env_lock for the entire closure.
    super::tests::fixture(|root, source| {
        let _native_log = native_logging(root);
        let store = root.join("authority");
        let _authority = crate::tests::TestEnvGuard::set(
            "ANGEL_CODING_TRAINING_AUTHORITY_DIR",
            store.to_str().unwrap(),
        );
        let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "tests");
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let request = LoopExperimentRequest { workspace:source.into(), artifact_dir:root.join("artifacts"),
            task:TASK.into(), max_hops:8, deadline_secs:30, token_budget:32768,
            verify_command:Some("printf entered > .angel-experiment-tmp/verifier-entered; sleep 30; printf '%s' 'test result: ok. 3 passed; 0 failed;'".into()),
            policy_note:None };
        let handle = std::thread::spawn(move || {
            run_loop_experiment(
                request,
                Arc::new(NativeFixtureClub {
                    step: AtomicUsize::new(0),
                    trap: None,
                }),
                thread_cancel,
            )
        });
        let marker = root.join("artifacts/working/.angel-experiment-tmp/verifier-entered");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !marker.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(marker.exists(), "actual evaluator must start");
        cancel.store(true, Ordering::Release);
        let result = handle.join().unwrap().unwrap();
        assert!(result.verification.unwrap().cancelled);
        assert_eq!(result.training_capture.unwrap()["status"], "cancelled");
        assert!(row(root)["evaluator_decision_sha256"].is_null());
        assert_native_observations(root, false, "owned-native-training-no-model");
    });
}

#[test]
fn unconfigured_recovery_raw_log_remains_an_explicit_experiment_observation() {
    // env-lock-exempt: super::tests::fixture in loop_experiment.rs holds crate::tests::env_lock for the entire closure.
    super::tests::fixture(|root, source| {
        let _native_log = native_logging(root);
        let _authority = crate::tests::TestEnvGuard::unset("ANGEL_CODING_TRAINING_AUTHORITY_DIR");
        let club: Arc<dyn Club> = Arc::new(super::tests::SelectedLeaf {
            calls: AtomicUsize::new(0),
            held: false,
            entered: Arc::new(AtomicBool::new(false)),
        });
        let result = run_loop_experiment(
            super::tests::request(root, source),
            club,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_native_observations(root, false, "owned-glm-route");
    });
}

#[test]
#[ignore = "retains actual native producer rows for CLI + consumer integration"]
fn export_owned_native_training_rows_for_consumer() {
    let _lock = crate::tests::env_lock();
    let root = PathBuf::from(
        std::env::var_os("ANGEL_NATIVE_TRAINING_FIXTURE_EXPORT")
            .expect("explicit owned fixture root"),
    );
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("angel-native-training-owned-")
    );
    std::fs::create_dir(&root).unwrap();
    let _artifact = crate::tests::TestEnvGuard::unset("ANGEL_EVALUATOR_ARTIFACT_DIR");
    let _log = crate::tests::TestEnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "0");
    for name in ["green", "popcorn", "failed"] {
        let case = root.join(name);
        std::fs::create_dir(&case).unwrap();
        let source = case.join("source");
        std::fs::create_dir(&source).unwrap();
        let cancel = AtomicBool::new(false);
        git_output(&source, &["init", "-q"], &cancel).unwrap();
        std::fs::write(source.join("candidate.txt"), "committed\n").unwrap();
        git_output(&source, &["add", "."], &cancel).unwrap();
        git_output(
            &source,
            &["commit", "-q", "-m", "owned native training base"],
            &cancel,
        )
        .unwrap();
        let store = case.join("authority");
        let _authority = crate::tests::TestEnvGuard::set(
            "ANGEL_CODING_TRAINING_AUTHORITY_DIR",
            store.to_str().unwrap(),
        );
        let _trajectory = crate::tests::TestEnvGuard::set(
            "ANGEL_TRAJECTORY_DIR",
            case.join("native-log").to_str().unwrap(),
        );
        let _reward = crate::tests::TestEnvGuard::set(
            "ANGEL_RL_REWARD",
            if name == "popcorn" {
                "popcorn_peer"
            } else {
                "tests"
            },
        );
        let peer = case.join("peer.json");
        std::fs::write(
            &peer,
            r#"{"name":"captured-original","geomean_us":100.0,"shapes":{"32768x1":100.0}}"#,
        )
        .unwrap();
        let _peer = crate::tests::TestEnvGuard::set("POPCORN_PEER_STATE", peer.to_str().unwrap());
        let command = match name {
            "popcorn" => {
                "printf x >> .angel-experiment-tmp/verifier-count; printf '%s' 'shape=32768x1 score_us=50us\ntest result: ok. 3 passed; 0 failed;'"
            }
            "failed" => {
                "printf x >> .angel-experiment-tmp/verifier-count; printf '%s' 'test result: ok. 3 passed; 0 failed;'; exit 7"
            }
            _ => GREEN,
        };
        let result = native(&case, &source, command, None);
        assert!(result.error.is_none(), "{name}: {:?}", result.error);
        assert_eq!(
            std::fs::read(case.join("artifacts/working/.angel-experiment-tmp/verifier-count"))
                .unwrap(),
            b"x"
        );
        let emitted = row(&case);
        assert_native_observations(&case, name != "failed", "owned-native-training-no-model");
        if name == "failed" {
            assert_eq!(result.verification.unwrap().exit_code, Some(7));
            assert!(emitted["evaluator_decision_sha256"].is_null());
            std::fs::write(
                case.join("expected-rejection.json"),
                serde_json::to_vec(&json!({
                    "reason":"actual failed evaluator observation has no authoritative decision",
                    "trajectory":"artifacts/training-trajectory.jsonl",
                }))
                .unwrap(),
            )
            .unwrap();
        } else {
            assert_eq!(result.training_capture.unwrap()["status"], "captured");
            let request = json!({"schema":"angel-coding-training-request/v1",
                "decision_sha256":emitted["evaluator_decision_sha256"],
                "task":emitted["evaluator_task"], "answer":emitted["answer"],
                "reward":emitted["reward"], "competition":emitted["competition"],
                "evaluator_evidence_manifest_sha256":emitted["evaluator_evidence_manifest_sha256"]});
            let bytes = serde_json::to_vec(&request).unwrap();
            if name == "popcorn" {
                std::fs::write(&peer, r#"{"name":"changed-after-decision","geomean_us":40.0,"shapes":{"32768x1":40.0}}"#).unwrap();
            }
            let receipt = crate::drive::reinforce::training::audit(&store, &bytes).unwrap();
            if name == "popcorn" {
                assert_eq!(receipt["baseline"]["baseline_us"], 100.0);
            }
            std::fs::write(case.join("request.json"), bytes).unwrap();
            std::fs::write(
                case.join("expected-receipt.json"),
                serde_json::to_vec_pretty(&receipt).unwrap(),
            )
            .unwrap();
        }
    }
    println!("actual native training rows retained at {}", root.display());
}

/// A club that reports whether the controller's policy note reached its system
/// prompt, so note application is observed rather than assumed.
struct NoteEchoClub;

impl Club for NoteEchoClub {
    fn label(&self) -> &str {
        "note-echo-fixture"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Err("must use native tool loop".into())
    }
    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        _tools: &[ToolDef],
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        let system = messages
            .iter()
            .find(|message| message.role == ChatRole::System)
            .map(|message| message.content.clone())
            .unwrap_or_default();
        Ok(ClubReply::Text(
            if system.contains("NOTE-XYZ") {
                "note-applied"
            } else {
                "note-absent"
            }
            .to_string(),
        ))
    }
}

/// Zero bounds mean the operator declined a cap: the attempt still runs to a
/// real answer, the verifier still produces physical evidence, and the policy
/// note reaches the attempt's system prompt.
#[test]
fn zero_bounds_run_uncapped_and_apply_the_policy_note() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-unbounded-{}", std::process::id()));
    let source = root.join("source");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("candidate.txt"), "baseline\n").unwrap();
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&source)
        .status()
        .unwrap();
    assert!(status.success());

    let run = |note: Option<&str>, artifact: &str| {
        let request = LoopExperimentRequest {
            workspace: source.clone(),
            artifact_dir: root.join(artifact),
            task: TASK.into(),
            max_hops: 0,
            deadline_secs: 0,
            token_budget: 0,
            verify_command: Some("printf '%s' 'test result: ok. 1 passed; 0 failed;'".into()),
            policy_note: note.map(str::to_string),
        };
        run_loop_experiment(
            request,
            Arc::new(NoteEchoClub),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
    };

    let applied = run(Some("NOTE-XYZ"), "applied");
    assert!(applied.error.is_none(), "{:?}", applied.error);
    assert_eq!(applied.stop_reason, "answer", "an uncapped attempt answers");
    assert_ne!(applied.stop_reason, "max_hops");
    assert_ne!(applied.stop_reason, "deadline");
    assert_eq!(applied.answer, "note-applied");
    let verification = applied.verification.as_ref().expect("verifier ran");
    assert_eq!(verification.exit_code, Some(0));
    assert!(
        applied.evidence.is_some(),
        "physical verifier evidence leaves the attempt"
    );

    let absent = run(None, "absent");
    assert_eq!(absent.answer, "note-absent", "no note means no note");
    assert!(absent.evidence.is_some());
    let _ = std::fs::remove_dir_all(&root);
}

/// The shared freezer is the only source copier: ignored heavy artifacts stay
/// out, dirty operator edits come along, executable bits survive, and it
/// refuses destinations that could escape into the source or the quarantine.
#[test]
fn frozen_source_excludes_ignored_artifacts_and_preserves_dirty_executable_source() {
    let root = std::env::temp_dir().join(format!("angel-freeze-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let source = root.join("source");
    std::fs::create_dir_all(source.join("scripts")).unwrap();
    let git = |args: &[&str], cwd: &Path| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(cwd)
                .status()
                .unwrap()
                .success(),
            "git {args:?}"
        );
    };
    git(&["init", "-q"], &source);
    std::fs::write(source.join(".gitignore"), "target/\nnode_modules/\n").unwrap();
    std::fs::write(source.join("app.txt"), "committed\n").unwrap();
    std::fs::write(source.join("scripts/check.sh"), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            source.join("scripts/check.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    git(&["add", "-f", "-A", "--", "."], &source);
    git(
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@localhost",
            "commit",
            "-q",
            "-m",
            "base",
        ],
        &source,
    );
    // Dirty operator work after the commit, plus ignored heavy trees.
    std::fs::write(source.join("app.txt"), "dirty operator edit\n").unwrap();
    std::fs::create_dir_all(source.join("target/debug")).unwrap();
    std::fs::write(source.join("target/debug/heavy.bin"), vec![7u8; 4096]).unwrap();
    std::fs::create_dir_all(source.join("node_modules/dep")).unwrap();
    std::fs::write(source.join("node_modules/dep/index.js"), "module\n").unwrap();

    let cancel = AtomicBool::new(false);
    let frozen = root.join("frozen");
    let digest = freeze_active_source(&source, &frozen, &cancel).unwrap();
    assert_eq!(digest.len(), 64, "the frozen digest is a content hash");
    assert!(
        !frozen.join("target").exists(),
        "ignored build output stays out"
    );
    assert!(
        !frozen.join("node_modules").exists(),
        "ignored deps stay out"
    );
    assert_eq!(
        std::fs::read_to_string(frozen.join("app.txt")).unwrap(),
        "dirty operator edit\n",
        "uncommitted operator work is what gets frozen"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(frozen.join("scripts/check.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "an executable verifier stays executable");
        assert!(
            std::process::Command::new(frozen.join("scripts/check.sh"))
                .status()
                .unwrap()
                .success(),
            "the frozen verifier actually runs"
        );
    }
    // A frozen source is a real baseline: a second freeze of the same bytes has
    // the same tree, and the digest is stable across freezes.
    assert_eq!(
        committed_tree_hash(&frozen, &cancel).unwrap(),
        committed_tree_hash(&frozen, &cancel).unwrap()
    );

    // Cancellation before copying is honoured.
    let cancelled = AtomicBool::new(true);
    assert!(
        freeze_active_source(&source, &root.join("cancelled"), &cancelled).is_err(),
        "a cancelled freeze copies nothing"
    );
    assert!(
        !root.join("cancelled").exists()
            || std::fs::read_dir(root.join("cancelled")).unwrap().count() == 0
    );

    // A symlinked parent must not smuggle the destination inside the source:
    // the nearest existing ancestor is canonicalized before anything is created.
    #[cfg(unix)]
    {
        let link = root.join("link-to-source");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(
            freeze_active_source(&source, &link.join("frozen"), &cancel).is_err(),
            "a symlinked parent inside the source is refused"
        );
        assert!(
            !source.join("frozen").exists(),
            "nothing was created inside the source"
        );
    }

    // Destination safety: never inside the source, never in the quarantine,
    // never an existing path.
    assert!(freeze_active_source(&source, &source.join("inside"), &cancel).is_err());
    assert!(
        freeze_active_source(&source, &root.join("off-limits/frozen"), &cancel).is_err(),
        "the quarantine is never a freeze destination"
    );
    assert!(
        freeze_active_source(&source, &frozen, &cancel).is_err(),
        "an existing destination is refused rather than merged"
    );
    let _ = std::fs::remove_dir_all(&root);
}
