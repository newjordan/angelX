use super::super::Candidate;
use super::super::evaluator::PolicyEvaluationRequest;
use super::*;
use crate::club::{ChatMsg, Club, ClubReply, StreamDelta, ToolDef};
use serde_json::json;
use std::sync::Mutex;

const TASK: &str = "make the objective's verifier pass";

/// A club that does the work once and then answers, so the attempt under
/// test is a real experiment artifact rather than a hand-built directory.
struct WorkerClub {
    /// When set, the attempt also rewrites the verifier's own script — the
    /// exact self-grading move the verifier scope exists to catch.
    tamper_verifier: bool,
}

impl WorkerClub {
    fn new() -> Self {
        Self {
            tamper_verifier: false,
        }
    }
}

impl Club for WorkerClub {
    fn label(&self) -> &str {
        "objective-case-worker"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("this fixture works through the tool loop".into())
    }
    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        _tools: &[ToolDef],
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if messages.len() == 2 {
            if self.tamper_verifier {
                return Ok(ClubReply::Calls(vec![crate::club::ToolCall {
                    id: "tamper".into(),
                    name: "write_file".into(),
                    args: json!({"path":"tests/check.sh","content":"#!/bin/sh\nexit 0\n"}),
                }]));
            }
            return Ok(ClubReply::Calls(vec![crate::club::ToolCall {
                id: "work".into(),
                name: "write_file".into(),
                args: json!({"path":"result.txt","content":"done"}),
            }]));
        }
        Ok(ClubReply::Text("wrote the artifact".into()))
    }
}

fn git(root: &Path, args: &[&str]) {
    assert!(
        crate::harness::pinned_git_command(root, args)
            .status()
            .unwrap()
            .success(),
        "git {args:?}"
    );
}

struct Fixture {
    root: PathBuf,
    fixture: PathBuf,
    attempts: PathBuf,
    attempt: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Build a frozen case source, run one real attempt in it, and return the
/// artifacts. `marker` distinguishes independent case sources.
fn fixture(tag: &str, marker: &str) -> Fixture {
    fixture_with_club(tag, marker, Arc::new(WorkerClub::new()))
}

fn fixture_with_club(tag: &str, marker: &str, club: Arc<dyn Club>) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "angel-objective-case-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let source = root.join("source");
    std::fs::create_dir_all(source.join("tests")).unwrap();
    std::fs::write(source.join("app.txt"), format!("{marker} app\n")).unwrap();
    std::fs::write(
        source.join("tests/check.sh"),
        "#!/bin/sh\ntest \"$(cat result.txt)\" = done\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            source.join("tests/check.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    git(&source, &["init", "-q"]);
    git(&source, &["add", "-f", "-A", "--", "."]);
    git(
        &source,
        &[
            "-c",
            "user.name=case",
            "-c",
            "user.email=case@localhost",
            "commit",
            "-q",
            "-m",
            "frozen",
        ],
    );
    let cancel = AtomicBool::new(false);
    let fixture = root.join("fixture");
    crate::harness::freeze_active_source(&source, &fixture, &cancel).unwrap();
    let attempts = root.join("attempts");
    std::fs::create_dir_all(&attempts).unwrap();
    let attempt = attempts.join("attempt-0001-v0");
    crate::harness::run_loop_experiment(
        crate::harness::LoopExperimentRequest {
            workspace: fixture.clone(),
            artifact_dir: attempt.clone(),
            task: TASK.to_string(),
            max_hops: 0,
            deadline_secs: 0,
            token_budget: 0,
            verify_command: None,
            policy_note: None,
        },
        club,
        Arc::new(cancel),
    )
    .unwrap();
    Fixture {
        root,
        fixture,
        attempts,
        attempt,
    }
}

fn receipt(fixture: &Fixture) -> Candidate {
    Candidate {
        policy_version: 0,
        latency: std::time::Duration::ZERO,
        output: format!(
            "attempt {} · patch deadbeef · stop answer\nwork · stop answer",
            fixture.attempt.display()
        ),
    }
}

fn request<'a>(
    candidate: &'a Candidate,
    manifest: &'a str,
    prompt: &'a str,
) -> PolicyEvaluationRequest<'a> {
    PolicyEvaluationRequest {
        cohort_manifest_sha256: manifest,
        cohort_role: "promotion",
        case_id: "objective",
        task: TASK,
        prompt_sha256: prompt,
        policy_version: 0,
        sample_index: 0,
        candidate,
    }
}

/// A measured sample must be bound to the case's frozen source, the
/// attempt's recorded patch and its working copy — a swapped attempt, a
/// forged patch, a hand-edited working copy or a rewritten verifier is
/// refused before any evidence exists.
#[test]
fn objective_case_receipts_bind_the_case_spec_identity_and_reject_forgeries() {
    let _lock = crate::tests::env_lock();
    let _authority = crate::tests::TestEnvGuard::unset("ANGEL_CODING_TRAINING_AUTHORITY_DIR");
    let case = fixture("binding", "case-a");
    let other = fixture("binding-other", "case-b");
    let manifest = "a".repeat(64);
    let prompt = "b".repeat(64);
    let evaluator = ObjectiveCaseEvaluator::new(
        &case.fixture,
        "sh tests/check.sh",
        &case.attempts,
        &["tests".to_string()],
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert!(evaluator.has_verifier_scope());

    let candidate = receipt(&case);
    let spec = evaluator.spec();
    let evidence = evaluator
        .evaluate(&request(&candidate, &manifest, &prompt))
        .unwrap();
    let policy_sha256 = spec.execution_policy_sha256();
    assert_eq!(evidence.outcome.command_sha256(), spec.command_sha256());
    assert_eq!(evidence.outcome.execution_policy_sha256(), policy_sha256);
    assert_eq!(
        evidence.outcome.verifier_contract_sha256(),
        crate::cut::sha256_hex(spec.outcome_contract().as_bytes())
    );
    assert_eq!(
        evidence.outcome.subject_sha256(),
        crate::cut::sha256_hex(
            request(&candidate, &manifest, &prompt)
                .canonical_subject()
                .as_bytes()
        )
    );
    assert!(
        evidence.outcome.succeeded(),
        "the verifier ran the frozen script"
    );

    // The measurement sink reports the two real durations for the sample:
    // the candidate's own recorded generation time, and the verifier's
    // measured wall time.
    let captured: Arc<Mutex<Vec<(f32, u64, u64)>>> = Arc::new(Mutex::new(Vec::new()));
    let sink_slot = Arc::clone(&captured);
    let observed = ObjectiveCaseEvaluator::new(
        &case.fixture,
        "sh tests/check.sh",
        &case.attempts,
        &["tests".to_string()],
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
    .with_measurement_sink(Arc::new(move |reward, generation_ms, verification_ms| {
        if let Ok(mut rows) = sink_slot.lock() {
            rows.push((reward, generation_ms, verification_ms));
        }
    }));
    let timed = Candidate {
        policy_version: 0,
        latency: std::time::Duration::from_millis(1_234),
        output: candidate.output.clone(),
    };
    observed
        .evaluate(&request(&timed, &manifest, &prompt))
        .expect("a bound attempt measures");
    let rows = captured.lock().unwrap().clone();
    assert_eq!(rows.len(), 1, "one sample was measured");
    assert_eq!(
        rows[0].1, 1_234,
        "the reported generation time is the candidate's own recorded timing"
    );
    assert_eq!(rows[0].0, 1.0, "the frozen verifier passes for this work");
    assert!(
        rows[0].2 < 60_000,
        "the verifier time is a real measurement, got {}ms",
        rows[0].2
    );

    // Cross-case swap: an attempt from another case's frozen source.
    let swapped = receipt(&other);
    let error = match evaluator.evaluate(&request(&swapped, &manifest, &prompt)) {
        Ok(_) => panic!("a swapped attempt must not produce evidence"),
        Err(error) => error,
    };
    assert!(
        error.contains("frozen source") || error.contains("outside this campaign"),
        "a swapped attempt must be refused, got: {error}"
    );

    // A receipt naming a path outside this campaign's attempts.
    let outside = Candidate {
        output: format!("attempt {} · patch x · stop answer", other.root.display()),
        ..candidate.clone()
    };
    let error = match evaluator.evaluate(&request(&outside, &manifest, &prompt)) {
        Ok(_) => panic!("an attempt outside the campaign must be refused"),
        Err(error) => error,
    };
    assert!(
        error.contains("outside this campaign") || error.contains("frozen source"),
        "{error}"
    );

    // Forged patch bytes.
    let patch_path = case.attempt.join("candidate.patch");
    let patch = std::fs::read(&patch_path).unwrap();
    std::fs::write(&patch_path, b"forged patch\n").unwrap();
    let error = match evaluator.evaluate(&request(&candidate, &manifest, &prompt)) {
        Ok(_) => panic!("a forged patch must not produce evidence"),
        Err(error) => error,
    };
    assert!(error.contains("patch does not match"), "{error}");
    std::fs::write(&patch_path, &patch).unwrap();

    // A change made after the attempt recorded its patch, and never staged:
    // `git diff HEAD` sees it, so it cannot ride along unbound.
    std::fs::write(case.attempt.join("working/unstaged.txt"), "late\n").unwrap();
    let error = match evaluator.evaluate(&request(&candidate, &manifest, &prompt)) {
        Ok(_) => panic!("an unbound working-tree change must be refused"),
        Err(error) => error,
    };
    assert!(error.contains("patch does not match"), "{error}");
    std::fs::remove_file(case.attempt.join("working/unstaged.txt")).unwrap();

    // A working copy edited after the attempt was recorded.
    std::fs::write(case.attempt.join("working/extra.txt"), "smuggled\n").unwrap();
    git(
        &case.attempt.join("working"),
        &["add", "-f", "-A", "--", "."],
    );
    let error = match evaluator.evaluate(&request(&candidate, &manifest, &prompt)) {
        Ok(_) => panic!("an edited working copy must not produce evidence"),
        Err(error) => error,
    };
    assert!(error.contains("patch does not match"), "{error}");
}

/// The scope check has to be reachable for the real threat: the attempt
/// itself rewrites the script that judges it, so the patch and working copy
/// agree and only the declared verifier scope can refuse the sample.
#[test]
fn candidate_cannot_rewrite_the_verifier_during_the_attempt() {
    let _lock = crate::tests::env_lock();
    let _authority = crate::tests::TestEnvGuard::unset("ANGEL_CODING_TRAINING_AUTHORITY_DIR");
    let case = fixture_with_club(
        "tamper",
        "case-c",
        Arc::new(WorkerClub {
            tamper_verifier: true,
        }),
    );
    let manifest = "a".repeat(64);
    let prompt = "b".repeat(64);
    let evaluator = ObjectiveCaseEvaluator::new(
        &case.fixture,
        "sh tests/check.sh",
        &case.attempts,
        &["tests".to_string()],
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let candidate = receipt(&case);
    let error = match evaluator.evaluate(&request(&candidate, &manifest, &prompt)) {
        Ok(_) => panic!("a rewritten verifier must not produce evidence"),
        Err(error) => error,
    };
    assert!(error.contains("verifier-owned input"), "{error}");

    // Without a declared scope the same attempt is only exploration: the
    // operator named no verifier-owned inputs, so nothing can be released.
    let unscoped = ObjectiveCaseEvaluator::new(
        &case.fixture,
        "sh tests/check.sh",
        &case.attempts,
        &[],
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert!(!unscoped.has_verifier_scope());
    assert!(
        unscoped
            .evaluate(&request(&candidate, &manifest, &prompt))
            .is_ok()
    );
}
