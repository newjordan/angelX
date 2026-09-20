//! Append-only, content-addressed evaluator evidence artifacts.

use super::{EvaluatorEvidence, Reward, RewardInput};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const ARTIFACT_SCHEMA: &str = "angel.rlvr.evaluator-artifact/v2";
const ARTIFACT_MAGIC: &[u8] = b"ANGEL-EVALUATOR-ARTIFACT-V2\n";
const MAX_METADATA_BYTES: usize = 64 * 1024;
const MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;
static ARTIFACT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

type ArtifactFields<'a> = (&'a [u8], &'a [u8], &'a [u8]);

fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[derive(Deserialize, Serialize)]
struct ArtifactMetadata {
    schema: String,
    source: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    sandbox_stderr_prefix_len: usize,
    exit_code: Option<i32>,
    succeeded: bool,
    workspace_path_sha256: String,
    workspace_before_sha256: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    workspace_changed_paths: Vec<String>,
    workspace_sha256: String,
    command_sha256: String,
    execution_policy_sha256: String,
    execution_id: String,
    duration_ns: u64,
    timed_out: bool,
    stdout_total_bytes: u64,
    stderr_total_bytes: u64,
    stdout_truncated: bool,
    stderr_truncated: bool,
    verifier_contract_sha256: String,
    subject_sha256: String,
    raw_output_sha256: String,
    manifest_sha256: String,
}

impl EvaluatorEvidence {
    pub fn persist_append_only(&self, root: &Path) -> Result<PathBuf, String> {
        self.validate_integrity()?;
        std::fs::create_dir_all(root)
            .map_err(|error| format!("could not create evaluator artifact store: {error}"))?;
        let metadata = ArtifactMetadata {
            schema: ARTIFACT_SCHEMA.to_string(),
            source: self.source.clone(),
            sandbox_stderr_prefix_len: self.sandbox_stderr_prefix_len,
            exit_code: self.exit_code,
            succeeded: self.succeeded,
            workspace_path_sha256: self.workspace_path_sha256.clone(),
            workspace_before_sha256: self.workspace_before_sha256.clone(),
            workspace_changed_paths: self.workspace_changed_paths.clone(),
            workspace_sha256: self.workspace_sha256.clone(),
            command_sha256: self.command_sha256.clone(),
            execution_policy_sha256: self.execution_policy_sha256.clone(),
            execution_id: self.execution_id.clone(),
            duration_ns: self.duration_ns,
            timed_out: self.timed_out,
            stdout_total_bytes: self.stdout_total_bytes,
            stderr_total_bytes: self.stderr_total_bytes,
            stdout_truncated: self.stdout_truncated,
            stderr_truncated: self.stderr_truncated,
            verifier_contract_sha256: self.verifier_contract_sha256.clone(),
            subject_sha256: self.subject_sha256.clone(),
            raw_output_sha256: self.raw_output_sha256.clone(),
            manifest_sha256: self.manifest_sha256.clone(),
        };
        let metadata = serde_json::to_vec(&metadata)
            .map_err(|error| format!("could not encode evaluator artifact: {error}"))?;
        let bytes = encode_artifact(&metadata, &self.stdout, &self.stderr)?;
        let path = root.join(format!("{}.evidence", self.manifest_sha256));
        let serial = ARTIFACT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = root.join(format!(
            ".evaluator-artifact-{}-{serial}.tmp",
            std::process::id()
        ));
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("could not create evaluator artifact temp file: {error}"))?;
        if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            let _ = std::fs::remove_file(&temporary);
            return Err(format!("could not persist evaluator artifact: {error}"));
        }
        drop(file);
        match std::fs::hard_link(&temporary, &path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = match read_bounded(&path) {
                    Ok(existing) => existing,
                    Err(error) => {
                        let _ = std::fs::remove_file(&temporary);
                        return Err(error);
                    }
                };
                if existing != bytes {
                    let _ = std::fs::remove_file(&temporary);
                    return Err(format!(
                        "evaluator artifact {} already exists with different bytes",
                        self.manifest_sha256
                    ));
                }
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(format!("could not publish evaluator artifact: {error}"));
            }
        }
        std::fs::remove_file(&temporary)
            .map_err(|error| format!("could not remove evaluator artifact temp file: {error}"))?;
        if let Ok(directory) = std::fs::File::open(root) {
            let _ = directory.sync_all();
        }
        Ok(path)
    }
}

pub fn load_evaluator_artifact(path: &Path) -> Result<EvaluatorEvidence, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect evaluator artifact: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("evaluator artifact must be a regular non-symlink file".into());
    }
    let bytes = read_bounded(path)?;
    let expected_manifest = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".evidence"))
        .ok_or("invalid evaluator artifact filename")?;
    load_evaluator_artifact_bytes(&bytes, expected_manifest)
}

/// Decode exactly the bytes already read by a digest-addressed authority reader.
pub(crate) fn load_evaluator_artifact_bytes(
    bytes: &[u8],
    expected_manifest: &str,
) -> Result<EvaluatorEvidence, String> {
    let (metadata, stdout, stderr) = decode_artifact(bytes)?;
    let metadata: ArtifactMetadata = serde_json::from_slice(metadata)
        .map_err(|error| format!("invalid evaluator artifact metadata: {error}"))?;
    if metadata.schema != ARTIFACT_SCHEMA {
        return Err(format!(
            "unsupported evaluator artifact schema: {}",
            metadata.schema
        ));
    }
    if metadata.manifest_sha256 != expected_manifest {
        return Err("evaluator artifact filename does not match its manifest".into());
    }
    let evidence = EvaluatorEvidence {
        source: metadata.source,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
        sandbox_stderr_prefix_len: metadata.sandbox_stderr_prefix_len,
        output: super::combined_process_output(
            stdout,
            stderr
                .get(metadata.sandbox_stderr_prefix_len..)
                .ok_or("invalid evaluator artifact sandbox stderr boundary")?,
        ),
        exit_code: metadata.exit_code,
        succeeded: metadata.succeeded,
        workspace_path_sha256: metadata.workspace_path_sha256,
        workspace_before_sha256: metadata.workspace_before_sha256,
        workspace_changed_paths: metadata.workspace_changed_paths,
        workspace_sha256: metadata.workspace_sha256,
        command_sha256: metadata.command_sha256,
        execution_policy_sha256: metadata.execution_policy_sha256,
        execution_id: metadata.execution_id,
        duration_ns: metadata.duration_ns,
        timed_out: metadata.timed_out,
        stdout_total_bytes: metadata.stdout_total_bytes,
        stderr_total_bytes: metadata.stderr_total_bytes,
        stdout_truncated: metadata.stdout_truncated,
        stderr_truncated: metadata.stderr_truncated,
        verifier_contract_sha256: metadata.verifier_contract_sha256,
        subject_sha256: metadata.subject_sha256,
        raw_output_sha256: metadata.raw_output_sha256,
        manifest_sha256: metadata.manifest_sha256,
    };
    evidence.validate_integrity()?;
    Ok(evidence)
}

pub fn replay_evaluator_artifact(path: &Path, reward: &dyn Reward) -> Result<f32, String> {
    let evidence = load_evaluator_artifact(path)?;
    reward.score(RewardInput::EvaluatorEvidence(&evidence))
}

pub(crate) fn persist_if_configured(
    evidence: &EvaluatorEvidence,
) -> Result<Option<PathBuf>, String> {
    let Some(root) = std::env::var_os("ANGEL_EVALUATOR_ARTIFACT_DIR") else {
        return Ok(None);
    };
    evidence.persist_append_only(Path::new(&root)).map(Some)
}

fn encode_artifact(metadata: &[u8], stdout: &[u8], stderr: &[u8]) -> Result<Vec<u8>, String> {
    if metadata.len() > MAX_METADATA_BYTES
        || stdout.len() > MAX_STREAM_BYTES
        || stderr.len() > MAX_STREAM_BYTES
    {
        return Err("evaluator artifact exceeds bounded field limits".into());
    }
    let mut bytes = Vec::with_capacity(
        ARTIFACT_MAGIC.len() + metadata.len() + stdout.len() + stderr.len() + 24,
    );
    bytes.extend_from_slice(ARTIFACT_MAGIC);
    for field in [metadata, stdout, stderr] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field);
    }
    Ok(bytes)
}

fn decode_artifact(bytes: &[u8]) -> Result<ArtifactFields<'_>, String> {
    let Some(mut remainder) = bytes.strip_prefix(ARTIFACT_MAGIC) else {
        return Err("invalid evaluator artifact magic".into());
    };
    let mut fields = Vec::with_capacity(3);
    for (index, limit) in [MAX_METADATA_BYTES, MAX_STREAM_BYTES, MAX_STREAM_BYTES]
        .into_iter()
        .enumerate()
    {
        let length_bytes: [u8; 8] = remainder
            .get(..8)
            .ok_or_else(|| format!("evaluator artifact field {index} is truncated"))?
            .try_into()
            .map_err(|_| "invalid evaluator artifact length".to_string())?;
        remainder = &remainder[8..];
        let length = usize::try_from(u64::from_be_bytes(length_bytes))
            .map_err(|_| "evaluator artifact length does not fit this platform".to_string())?;
        if length > limit {
            return Err(format!(
                "evaluator artifact field {index} exceeds its limit"
            ));
        }
        let field = remainder
            .get(..length)
            .ok_or_else(|| format!("evaluator artifact field {index} is truncated"))?;
        fields.push(field);
        remainder = &remainder[length..];
    }
    if !remainder.is_empty() {
        return Err("evaluator artifact has trailing bytes".into());
    }
    Ok((fields[0], fields[1], fields[2]))
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("could not open evaluator artifact: {error}"))?;
    let max = ARTIFACT_MAGIC.len() + MAX_METADATA_BYTES + 2 * MAX_STREAM_BYTES + 24;
    let mut bytes = Vec::new();
    file.take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read evaluator artifact: {error}"))?;
    if bytes.len() > max {
        return Err("evaluator artifact exceeds total size limit".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reinforce::{TEST_VERIFIER_CONTRACT, TestReward};

    #[test]
    fn evaluator_artifact_roundtrip_replays_reward_and_rejects_tamper() {
        // This fixture spawns raw `git` (not the env-stripping pinned
        // command), so it holds the crate env lock: a concurrent test that
        // sets GIT_DIR/GIT_WORK_TREE (the workspace-evidence decoy) would
        // otherwise redirect the spawned git into a different work tree and
        // fail `git add` with "pathspec did not match" at random.
        let _guard = crate::tests::env_lock();
        let root =
            std::env::temp_dir().join(format!("angel-evaluator-artifact-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        let store = root.join("store");
        std::fs::create_dir_all(&workspace).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&workspace)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::write(workspace.join("fixture.txt"), "frozen fixture").unwrap();
        let status = std::process::Command::new("git")
            .args(["add", "fixture.txt"])
            .current_dir(&workspace)
            .status()
            .unwrap();
        assert!(status.success());

        let evidence = EvaluatorEvidence::run_shell(
            "artifact-test",
            "printf '%s' 'test result: ok. 3 passed; 0 failed;'",
            &workspace,
            TEST_VERIFIER_CONTRACT,
            "artifact-subject",
        )
        .unwrap();
        let path = evidence.persist_append_only(&store).unwrap();
        assert_eq!(evidence.persist_append_only(&store).unwrap(), path);
        let writers = (0..8)
            .map(|_| {
                let evidence = evidence.clone();
                let store = store.clone();
                std::thread::spawn(move || evidence.persist_append_only(&store).unwrap())
            })
            .collect::<Vec<_>>();
        assert!(
            writers
                .into_iter()
                .all(|writer| writer.join().unwrap() == path)
        );
        assert!(std::fs::read_dir(&store).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let loaded = load_evaluator_artifact(&path).unwrap();
        assert_eq!(loaded.manifest_sha256(), evidence.manifest_sha256());
        assert_eq!(loaded.raw_output_sha256(), evidence.raw_output_sha256());
        assert_eq!(loaded.output(), evidence.output());
        assert_eq!(replay_evaluator_artifact(&path, &TestReward).unwrap(), 1.0);

        let original = std::fs::read(&path).unwrap();
        let mut tampered = original.clone();
        *tampered.last_mut().unwrap() ^= 1;
        std::fs::write(&path, &tampered).unwrap();
        assert!(load_evaluator_artifact(&path).is_err());
        assert!(
            evidence
                .persist_append_only(&store)
                .unwrap_err()
                .contains("different bytes")
        );

        std::fs::write(&path, &original).unwrap();
        let wrong_name = store.join("wrong.evidence");
        std::fs::write(&wrong_name, &original).unwrap();
        assert!(
            load_evaluator_artifact(&wrong_name)
                .unwrap_err()
                .contains("filename")
        );

        #[cfg(unix)]
        {
            let link = store.join("link.evidence");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(
                load_evaluator_artifact(&link)
                    .unwrap_err()
                    .contains("non-symlink")
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
