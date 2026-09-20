//! Producer-owned coding-eval decisions and a read-only training auditor.
//!
//! The operator must isolate this store from candidate/inbox writers. A digest
//! authenticates bytes, not authorship; unrestricted same-UID shell access is
//! not an authority boundary. No input row can select a root, path or command.
use super::*;
use serde_json::{Value, json};
use std::io::{Read, Write};

const DECISION_SCHEMA: &str = "angel-coding-training-decision/v1";
const REQUEST_SCHEMA: &str = "angel-coding-training-request/v1";
const RECEIPT_SCHEMA: &str = "angel-coding-training-admission/v1";
const OWNER: &str = "coding_eval";
const JSON_LIMIT: usize = 2 * 1024 * 1024;
const EVIDENCE_LIMIT: usize = 129 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    schema: String,
    data_class: String,
    evaluator_artifact_sha256: String,
    evaluator_evidence_manifest_sha256: String,
    subject_sha256: String,
    task_sha256: String,
    answer_sha256: String,
    reward_owner: String,
    reward_contract: String,
    reward: f32,
    baseline: Option<CodingEvalCompetition>,
    competition: Option<Value>,
    lineage: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    decision_sha256: String,
    task: String,
    answer: String,
    reward: f64,
    competition: Value,
    evaluator_evidence_manifest_sha256: String,
}

pub(crate) struct PublishedDecision {
    pub decision_sha256: String,
    pub artifact_sha256: String,
    pub manifest_sha256: String,
    pub reward: f32,
    pub competition: Option<Value>,
}

fn digest_ok(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn subject(task: &str, answer: &str) -> String {
    format!(
        "task={}:{}\nanswer={}:{}",
        task.len(),
        task,
        answer.len(),
        answer
    )
}

fn lineage(evidence: &EvaluatorEvidence) -> Value {
    json!({
        "scope": "evaluator_execution_and_exact_task_answer",
        "workspace_path_sha256": evidence.workspace_path_sha256,
        "workspace_before_sha256": evidence.workspace_before_sha256,
        "workspace_after_sha256": evidence.workspace_sha256,
        "command_sha256": evidence.command_sha256,
        "execution_policy_sha256": evidence.execution_policy_sha256,
        "execution_id": evidence.execution_id,
        "verifier_contract_sha256": evidence.verifier_contract_sha256,
        "raw_output_sha256": evidence.raw_output_sha256,
    })
}

/// The root comes from operator configuration. Only lowercase digest names can
/// resolve beneath it; openat pins the directory and refuses object symlinks.
fn read_object(root: &Path, digest: &str, suffix: &str, limit: usize) -> Result<Vec<u8>, String> {
    if !digest_ok(digest) {
        return Err("invalid authority digest".into());
    }
    #[cfg(unix)]
    let file = {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::OpenOptionsExt;
        let directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)
            .map_err(|error| format!("open configured authority: {error}"))?;
        let name =
            std::ffi::CString::new(format!("{digest}{suffix}")).map_err(|e| e.to_string())?;
        // SAFETY: directory is retained and the only relative name is generated.
        let descriptor = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if descriptor < 0 {
            return Err(format!(
                "open authority object: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: openat returned one owned descriptor.
        unsafe { std::fs::File::from_raw_fd(descriptor) }
    };
    #[cfg(not(unix))]
    return Err("coding authority no-follow reads currently require Unix".into());
    #[cfg(unix)]
    {
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() > limit as u64 {
            return Err("authority object must be a bounded regular file".into());
        }
        let mut bytes = Vec::new();
        file.take((limit + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > limit {
            return Err("authority object exceeds byte limit".into());
        }
        Ok(bytes)
    }
}

fn publish_decision(root: &Path, bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > JSON_LIMIT {
        return Err("coding decision exceeds byte limit".into());
    }
    let digest = crate::cut::sha256_hex(bytes);
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let temporary = root.join(format!(
        ".coding-decision-{}-{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
    let publish = (|| {
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        match std::fs::hard_link(&temporary, root.join(format!("{digest}.decision.json"))) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if read_object(root, &digest, ".decision.json", JSON_LIMIT)? != bytes {
                    return Err(
                        "coding decision identity already exists with different bytes".into(),
                    );
                }
            }
            Err(error) => return Err(error.to_string()),
        }
        std::fs::File::open(root)
            .and_then(|file| file.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(digest)
    })();
    drop(file);
    let cleanup = std::fs::remove_file(&temporary).map_err(|e| e.to_string());
    match publish {
        Ok(digest) => {
            cleanup?;
            Ok(digest)
        }
        Err(error) => Err(error),
    }
}

fn replay(
    evidence: &EvaluatorEvidence,
    contract: &str,
    baseline: &Option<CodingEvalCompetition>,
) -> Result<CodingEvalScore, String> {
    validate_coding_eval_evidence(evidence)?;
    let reward = match (contract, baseline) {
        ("tests", None) => TestReward.score(RewardInput::EvaluatorEvidence(evidence))?,
        ("popcorn_peer", Some(baseline)) => {
            if baseline.evidence_manifest_sha256 != evidence.manifest_sha256
                || !baseline.baseline_us.is_finite()
                || baseline.baseline_us <= 0.0
                || !popcorn_competition_signal(evidence.output())
            {
                return Err("coding decision baseline does not bind this evidence".into());
            }
            PopcornPeerReward::new()
                .with_baseline(baseline.baseline_us)
                .score(RewardInput::CandidateOutput(evidence.output()))?
        }
        _ => return Err("unsupported coding decision scoring contract".into()),
    };
    if !reward.is_finite() {
        return Err("non-finite coding decision reward".into());
    }
    Ok(CodingEvalScore {
        reward,
        competition: baseline.clone(),
    })
}

pub(super) fn publish(
    root: &Path,
    task: &str,
    answer: &str,
    evidence: &EvaluatorEvidence,
    scoring: &CodingEvalScore,
) -> Result<PublishedDecision, String> {
    let subject_sha256 = crate::cut::sha256_hex(subject(task, answer).as_bytes());
    if subject_sha256 != evidence.subject_sha256 {
        return Err("coding decision subject mismatch".into());
    }
    let contract = if scoring.competition.is_some() {
        "popcorn_peer"
    } else {
        "tests"
    };
    let reproduced = replay(evidence, contract, &scoring.competition)?;
    if reproduced.reward.to_bits() != scoring.reward.to_bits() {
        return Err("coding decision reward is not reproducible under retained contract".into());
    }
    // Publish actual evaluator-owned bytes before creating the authoritative
    // decision. Never import inbox-supplied artifact bytes into this store.
    evidence.persist_append_only(root)?;
    let bytes = read_object(
        root,
        evidence.manifest_sha256(),
        ".evidence",
        EVIDENCE_LIMIT,
    )?;
    let artifact_sha256 = crate::cut::sha256_hex(&bytes);
    let competition = coding_eval_competition_meta(evidence, scoring);
    let decision = Decision {
        schema: DECISION_SCHEMA.into(),
        data_class: "verified_coding_eval".into(),
        evaluator_artifact_sha256: artifact_sha256.clone(),
        evaluator_evidence_manifest_sha256: evidence.manifest_sha256.clone(),
        subject_sha256,
        task_sha256: crate::cut::sha256_hex(task.as_bytes()),
        answer_sha256: crate::cut::sha256_hex(answer.as_bytes()),
        reward_owner: OWNER.into(),
        reward_contract: contract.into(),
        reward: scoring.reward,
        baseline: scoring.competition.clone(),
        competition: competition.clone(),
        lineage: lineage(evidence),
    };
    let decision_sha256 = publish_decision(
        root,
        &serde_json::to_vec(&decision).map_err(|e| e.to_string())?,
    )?;
    Ok(PublishedDecision {
        decision_sha256,
        artifact_sha256,
        manifest_sha256: evidence.manifest_sha256.clone(),
        reward: scoring.reward,
        competition,
    })
}

pub(super) fn publish_if_configured(
    task: &str,
    answer: &str,
    evidence: &EvaluatorEvidence,
    scoring: &CodingEvalScore,
) -> Result<Option<PublishedDecision>, String> {
    let Some(root) = std::env::var_os("ANGEL_CODING_TRAINING_AUTHORITY_DIR") else {
        return Ok(None);
    };
    publish(Path::new(&root), task, answer, evidence, scoring).map(Some)
}

/// Pure read-only admission. Replays the stored decision, never the living peer,
/// original verifier command, or a program named by an incoming row.
pub(crate) fn audit(root: &Path, input: &[u8]) -> Result<Value, String> {
    if input.len() > JSON_LIMIT {
        return Err("coding audit request exceeds byte limit".into());
    }
    let request: Request =
        serde_json::from_slice(input).map_err(|e| format!("invalid coding audit request: {e}"))?;
    if request.schema != REQUEST_SCHEMA || !request.reward.is_finite() {
        return Err("invalid coding audit schema or reward".into());
    }
    let bytes = read_object(root, &request.decision_sha256, ".decision.json", JSON_LIMIT)?;
    if crate::cut::sha256_hex(&bytes) != request.decision_sha256 {
        return Err("coding decision bytes do not match identity".into());
    }
    let decision: Decision =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid coding decision: {e}"))?;
    if decision.schema != DECISION_SCHEMA
        || decision.data_class != "verified_coding_eval"
        || decision.reward_owner != OWNER
        || decision.subject_sha256
            != crate::cut::sha256_hex(subject(&request.task, &request.answer).as_bytes())
        || decision.task_sha256 != crate::cut::sha256_hex(request.task.as_bytes())
        || decision.answer_sha256 != crate::cut::sha256_hex(request.answer.as_bytes())
        || (decision.reward as f64).to_bits() != request.reward.to_bits()
        || serde_json::to_value(&decision.competition).map_err(|e| e.to_string())?
            != request.competition
        || decision.evaluator_evidence_manifest_sha256 != request.evaluator_evidence_manifest_sha256
    {
        return Err("coding request does not match original producer decision".into());
    }
    let artifact = read_object(
        root,
        &decision.evaluator_evidence_manifest_sha256,
        ".evidence",
        EVIDENCE_LIMIT,
    )?;
    if crate::cut::sha256_hex(&artifact) != decision.evaluator_artifact_sha256 {
        return Err("coding evaluator artifact bytes do not match original decision".into());
    }
    let evidence = artifact::load_evaluator_artifact_bytes(
        &artifact,
        &decision.evaluator_evidence_manifest_sha256,
    )?;
    if evidence.subject_sha256 != decision.subject_sha256 || lineage(&evidence) != decision.lineage
    {
        return Err("coding evaluator subject or lineage mismatch".into());
    }
    let reproduced = replay(&evidence, &decision.reward_contract, &decision.baseline)?;
    if reproduced.reward.to_bits() != decision.reward.to_bits()
        || coding_eval_competition_meta(&evidence, &reproduced) != decision.competition
    {
        return Err("coding original reward or competition metadata failed replay".into());
    }
    Ok(json!({
        "schema": RECEIPT_SCHEMA, "request_sha256": crate::cut::sha256_hex(input),
        "data_class": "verified_coding_eval", "decision_sha256": request.decision_sha256,
        "evaluator_artifact_sha256": decision.evaluator_artifact_sha256,
        "evaluator_evidence_manifest_sha256": decision.evaluator_evidence_manifest_sha256,
        "subject_sha256": decision.subject_sha256, "task_sha256": decision.task_sha256,
        "answer_sha256": decision.answer_sha256, "reward": decision.reward,
        "reward_owner": decision.reward_owner, "reward_contract": decision.reward_contract,
        "baseline": decision.baseline, "competition": decision.competition, "lineage": decision.lineage,
    }))
}

pub(crate) fn audit_cli(root: &Path) -> std::io::Result<()> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take((JSON_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    let receipt = audit(root, &bytes).map_err(std::io::Error::other)?;
    let output = serde_json::to_vec(&receipt).map_err(std::io::Error::other)?;
    if output.len() > JSON_LIMIT {
        return Err(std::io::Error::other(
            "coding audit receipt exceeds byte limit",
        ));
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&output)?;
    stdout.write_all(b"\n")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/reinforce/training__tests.rs"]
mod tests;
