//! One pinned objective case for the technical reinforcement campaign.
//!
//! A case binds what the release gate needs and nothing more:
//!
//! * the **frozen source identity** — the active source snapshot taken by
//!   [`crate::harness::freeze_active_source`] (tracked plus untracked
//!   non-ignored files, modes preserved, quarantine excluded), so every sample
//!   starts from the same bytes and the case has a stable identity;
//! * the **operator's verifier command** and its **owner-declared input scope**:
//!   the candidate may author code and tests anywhere outside that scope, but the
//!   verifier-owned paths must still be byte-identical to the frozen source — a
//!   candidate cannot rewrite the thing that judges it;
//! * the **result** — measured by evaluator-owned execution of that command in
//!   the candidate's own working copy, whose baseline tree, staged patch and
//!   recorded source digest are all bound before it is measured.
//!
//! Nothing here is candidate-authored: the case identity in the receipt is the
//! framework's own, and the outcome reward reads the verifier process's verdict.

use super::evaluator::{
    EvaluatorSpec, PolicyEvaluationEvidence, PolicyEvaluationRequest, PolicyEvaluator,
};
use super::{COMMAND_SUCCESS_VERIFIER_CONTRACT, EvaluatorEvidence};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Prefix every generated candidate receipt carries, so the attempt this sample
/// came from can be resolved without trusting prose.
const ATTEMPT_MARKER: &str = "attempt ";
/// Bounds for the reflection evidence pack: reflection needs the shape of what
/// happened, never a whole trajectory or a huge diff.
const PACK_ANSWER_CHARS: usize = 400;
const PACK_PATCH_CHARS: usize = 600;
const PACK_TOOL_ERRORS: usize = 3;

/// Display-only sink for a measurement the evaluator took part in. The
/// authoritative verdict is always the receipt consumed by the promotion gate;
/// this only lets the RL stage show every measured attempt.
///
/// The two durations are the actual recorded times of one attempt:
/// `generation_ms` is the candidate's own generation time (the engine's
/// `Candidate::latency`, measured by the generator around the attempt) and
/// `verification_ms` is the evaluator-run verifier's wall time for that same
/// sample. Neither is synthesised or clamped.
pub(crate) type MeasurementSink = Arc<dyn Fn(f32, u64, u64) + Send + Sync>;

/// The case identity lines an evaluator observes. Framework-authored, derived
/// from the frozen source digest and the verifier digest only.
fn case_identity(fixture_sha256: &str, command_sha256: &str) -> String {
    let mut lines = vec![
        format!("angel.rlvr.objective-case/v1 source {fixture_sha256}"),
        format!("angel.rlvr.objective-case/v1 verifier {command_sha256}"),
    ];
    lines.sort();
    let mut bytes = String::new();
    for line in &lines {
        bytes.push_str(line);
        bytes.push('\n');
    }
    bytes
}

fn bounded(text: &str, chars: usize) -> String {
    let mut out = String::new();
    for character in text.chars().take(chars) {
        out.push(character);
    }
    if text.chars().count() > chars {
        out.push('…');
    }
    out
}

/// Bounded, inspectable summary of what one attempt did, read from the attempt's
/// own artifacts: its answer, the patch it authored, and its tool errors. The
/// physical verifier verdict is appended by the caller from the receipt, never
/// from this text.
pub(crate) fn attempt_evidence_pack(attempt: &Path) -> String {
    let result: serde_json::Value = std::fs::read_to_string(attempt.join("result.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(serde_json::Value::Null);
    let answer = result["answer"].as_str().unwrap_or("");
    let stop = result["stop_reason"].as_str().unwrap_or("unknown");
    let error = result["error"].as_str().unwrap_or("");
    let patch = std::fs::read_to_string(attempt.join("candidate.patch")).unwrap_or_default();
    let files = patch
        .lines()
        .filter_map(|line| line.strip_prefix("diff --git "))
        .count();
    let (added, removed) = patch.lines().fold((0usize, 0usize), |(a, r), line| {
        if line.starts_with("+++") || line.starts_with("---") {
            (a, r)
        } else if line.starts_with('+') {
            (a + 1, r)
        } else if line.starts_with('-') {
            (a, r + 1)
        } else {
            (a, r)
        }
    });
    let events: serde_json::Value = std::fs::read_to_string(attempt.join("tool-events.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(serde_json::Value::Null);
    let mut calls = 0usize;
    let mut errors: Vec<String> = Vec::new();
    if let Some(rows) = events["events"].as_array() {
        for row in rows {
            if row["kind"] == "tool_result" {
                calls += 1;
                let outcome = row["outcome"].as_str().unwrap_or("");
                if !outcome.contains("Succeeded") && errors.len() < PACK_TOOL_ERRORS {
                    errors.push(format!(
                        "{}: {}",
                        row["name"].as_str().unwrap_or("tool"),
                        bounded(row["summary"].as_str().unwrap_or(""), 120)
                    ));
                }
            }
        }
    }
    let mut pack = format!(
        "work · stop {stop} · answer: {}",
        bounded(answer.trim(), PACK_ANSWER_CHARS)
    );
    if !error.is_empty() {
        pack.push_str(&format!(
            "\nwork · error: {}",
            bounded(error, PACK_ANSWER_CHARS)
        ));
    }
    pack.push_str(&format!(
        "\npatch · {files} file(s) · +{added}/-{removed} line(s)"
    ));
    if !patch.trim().is_empty() {
        pack.push_str(&format!(
            "\npatch excerpt: {}",
            bounded(patch.trim(), PACK_PATCH_CHARS)
        ));
    }
    pack.push_str(&format!("\ntools · {calls} result(s)"));
    for entry in errors {
        pack.push_str(&format!("\ntool error · {entry}"));
    }
    pack
}

/// Evaluator-owned measurement of one objective case.
pub(crate) struct ObjectiveCaseEvaluator {
    spec: EvaluatorSpec,
    fixture_root: PathBuf,
    command: String,
    inventory_command: String,
    attempts_root: PathBuf,
    /// Owner-declared verifier inputs (relative paths). Empty means the operator
    /// named no verifier-owned scope, so the case can only be measured
    /// exploration and is never releasable.
    verifier_scope: Vec<String>,
    fixture_digest: String,
    fixture_tree: String,
    cancel: Arc<AtomicBool>,
    sink: Option<MeasurementSink>,
}

impl ObjectiveCaseEvaluator {
    /// `fixture_root` is the frozen case source; `attempts_root` bounds which
    /// working copies this evaluator may measure.
    pub(crate) fn new(
        fixture_root: &Path,
        command: &str,
        attempts_root: &Path,
        verifier_scope: &[String],
        cancel: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let tools: Vec<PathBuf> = super::evaluator::absolute_executable_paths(command);
        let declared: Vec<&Path> = tools.iter().map(PathBuf::as_path).collect();
        let spec = EvaluatorSpec::command_success(fixture_root, command, &declared)?;
        let fixture_root =
            std::fs::canonicalize(fixture_root).map_err(|error| error.to_string())?;
        let attempts_root =
            std::fs::canonicalize(attempts_root).map_err(|error| error.to_string())?;
        let identity = case_identity(spec.fixture_sha256(), spec.command_sha256());
        let fixture_digest = crate::harness::source_snapshot_digest(&fixture_root, &cancel)?;
        let fixture_tree = crate::harness::committed_tree_hash(&fixture_root, &cancel)?;
        Ok(Self {
            spec,
            fixture_root,
            command: command.to_string(),
            inventory_command: format!("printf '%s' '{identity}'"),
            attempts_root,
            verifier_scope: verifier_scope.to_vec(),
            fixture_digest,
            fixture_tree,
            cancel,
            sink: None,
        })
    }

    /// Report every measurement this evaluator takes to the RL stage.
    pub(crate) fn with_measurement_sink(mut self, sink: MeasurementSink) -> Self {
        self.sink = Some(sink);
        self
    }

    /// True when the operator declared the inputs this case's verifier owns.
    pub(crate) fn has_verifier_scope(&self) -> bool {
        !self.verifier_scope.is_empty()
    }

    /// Resolve and bind the candidate's attempt: it must be one of this
    /// campaign's attempts, it must have been asked this case's objective, it
    /// must have started from this case's frozen source, and the recorded patch
    /// must still be exactly the staged change in its working copy. A swapped,
    /// forged or partially coped attempt fails here, before any measurement.
    fn bind_attempt(
        &self,
        candidate_output: &str,
        task: &str,
    ) -> Result<(PathBuf, PathBuf), String> {
        if self.cancel.load(Ordering::Acquire) {
            return Err("campaign cancelled".into());
        }
        let value = candidate_output
            .strip_prefix(ATTEMPT_MARKER)
            .ok_or_else(|| "candidate receipt does not name an attempt".to_string())?;
        let dir = value
            .split(['·', '\n'])
            .next()
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
            .ok_or_else(|| "candidate receipt names no attempt directory".to_string())?;
        let attempt = std::fs::canonicalize(dir)
            .map_err(|error| format!("candidate attempt is unavailable: {error}"))?;
        if !attempt.starts_with(&self.attempts_root) {
            return Err("candidate attempt lies outside this campaign's attempts".into());
        }
        let raw = std::fs::read_to_string(attempt.join("result.json"))
            .map_err(|error| format!("candidate attempt has no result receipt: {error}"))?;
        let result: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|error| format!("candidate attempt receipt is unreadable: {error}"))?;
        let recorded_task = result["task_sha256"]
            .as_str()
            .ok_or_else(|| "candidate attempt receipt has no objective digest".to_string())?;
        if recorded_task != crate::cut::sha256_hex(task.as_bytes()) {
            return Err("candidate attempt was made for a different objective".into());
        }
        let recorded_source = result["snapshot_sha256"]
            .as_str()
            .ok_or_else(|| "candidate attempt receipt has no source digest".to_string())?;
        if recorded_source != self.fixture_digest {
            return Err("candidate attempt did not start from this case's frozen source".into());
        }
        let working = attempt.join("working");
        if crate::harness::committed_tree_hash(&working, &self.cancel)? != self.fixture_tree {
            return Err("candidate working copy does not descend from the frozen source".into());
        }
        let recorded_patch = std::fs::read(attempt.join("candidate.patch"))
            .map_err(|error| format!("candidate attempt has no patch: {error}"))?;
        let staged = crate::harness::candidate_patch_since_baseline(&working, &[], &self.cancel)?;
        if crate::cut::sha256_hex(&staged) != crate::cut::sha256_hex(&recorded_patch) {
            return Err("candidate patch does not match its working copy".into());
        }
        Ok((attempt, working))
    }

    /// The declared verifier inputs must be untouched by the candidate: an agent
    /// may author code and tests outside the scope, but it may not rewrite the
    /// thing that judges it.
    fn verifier_inputs_untouched(&self, working: &Path) -> Result<(), String> {
        if self.verifier_scope.is_empty() {
            return Ok(());
        }
        let drift = crate::harness::candidate_patch_since_baseline(
            working,
            &self.verifier_scope,
            &self.cancel,
        )?;
        if !drift.is_empty() {
            return Err(format!(
                "verifier-owned input was modified by the candidate: {}",
                self.verifier_scope.join(", ")
            ));
        }
        Ok(())
    }

    /// Run the operator's verifier in `working` under evaluator-owned execution,
    /// bounded only by the campaign's own cancellation. The receipt path binds
    /// the evidence to the case's frozen identity (`spec.outcome_contract()`).
    fn measure(
        &self,
        working: &Path,
        subject: &str,
        contract: &str,
        candidate_bytes: &[u8],
    ) -> Result<EvaluatorEvidence, String> {
        self.verifier_inputs_untouched(working)?;
        EvaluatorEvidence::run_shell_for_objective(
            "objective case verifier",
            &self.command,
            working,
            contract,
            subject,
            candidate_bytes,
            &self.cancel,
        )
    }

    /// Physical measurement of one attempt, for the training batch: same
    /// execution boundary, no receipt store. The second value is the verifier's
    /// own verdict text, so a red attempt is explainable rather than mysterious;
    /// the third is the verifier's measured wall time for this sample.
    pub(crate) fn measure_attempt(
        &self,
        candidate_output: &str,
        task: &str,
    ) -> Result<(f32, String, u64), String> {
        use super::{Reward, RewardInput};
        let (attempt, working) = self.bind_attempt(candidate_output, task)?;
        let candidate_bytes = std::fs::read(attempt.join("candidate.patch"))
            .map_err(|error| format!("candidate attempt has no patch to measure: {error}"))?;
        let subject = candidate_output.to_string();
        let started = Instant::now();
        let evidence = self.measure(
            &working,
            &subject,
            COMMAND_SUCCESS_VERIFIER_CONTRACT,
            &candidate_bytes,
        )?;
        let verification_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let reward = super::promotion::TechnicalReward::ObjectivePass
            .score(RewardInput::EvaluatorEvidence(&evidence))?;
        let tail: String = evidence
            .output()
            .trim()
            .chars()
            .rev()
            .take(200)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let receipt = format!(
            "verifier exit {} · reward {reward:.2}{}",
            evidence
                .exit_code()
                .map_or("none".to_string(), |code| code.to_string()),
            if tail.is_empty() {
                String::new()
            } else {
                format!(" · {}", tail.replace('\n', " "))
            }
        );
        Ok((reward, receipt, verification_ms))
    }
}

impl PolicyEvaluator for ObjectiveCaseEvaluator {
    fn spec(&self) -> &EvaluatorSpec {
        &self.spec
    }

    fn evaluate(
        &self,
        request: &PolicyEvaluationRequest<'_>,
    ) -> Result<PolicyEvaluationEvidence, String> {
        let (attempt, working) = self.bind_attempt(&request.candidate.output, request.task)?;
        let candidate_bytes = std::fs::read(attempt.join("candidate.patch"))
            .map_err(|error| format!("candidate attempt has no patch to measure: {error}"))?;
        // Inventory: the framework's own case identity, executed (not asserted)
        // in the same workspace as the outcome — the receipt pair must share one
        // start state — under the same bound policy.
        let inventory = EvaluatorEvidence::run_shell_for_objective(
            "objective case identity",
            &self.inventory_command,
            &working,
            &self.spec.inventory_contract(),
            &request.canonical_inventory_subject(),
            &candidate_bytes,
            &self.cancel,
        )?;
        let started = Instant::now();
        let outcome = self.measure(
            &working,
            &request.canonical_subject(),
            self.spec.outcome_contract(),
            &candidate_bytes,
        )?;
        if let Some(sink) = &self.sink {
            use super::Reward;
            let reward = super::promotion::TechnicalReward::ObjectivePass
                .score(super::RewardInput::EvaluatorEvidence(&outcome))
                .unwrap_or(0.0);
            let generation_ms =
                u64::try_from(request.candidate.latency.as_millis()).unwrap_or(u64::MAX);
            let verification_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            sink(reward, generation_ms, verification_ms);
        }
        Ok(PolicyEvaluationEvidence { inventory, outcome })
    }
}

#[cfg(test)]
mod tests {
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
}
