//! Local campaign artifacts and coordinator-only, independently checked integration.
//!
//! Agent files are immutable, private inputs. Canonical research files change only
//! after their exact source identities, referee corrections and copied-document
//! checks have been validated together. A durable journal makes an interrupted
//! multi-file integration visible and recoverable instead of silently accepted.

use super::Map;
use super::campaign::{
    CampaignIntegrator, Correction, DOCUMENT_EDIT_BYTES, IntegrationBundle, RoleEvidence,
    apply_claim_correction,
};
use crate::agent::harness::{
    confined_append_no_symlinks, confined_compare_replace_batch_no_symlinks,
    confined_open_read_no_symlinks, confined_open_rw_no_symlinks, confined_publish_new_no_symlinks,
    confined_read_limited_no_symlinks,
};
use crate::knowledge::cut::sha256_hex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const PRIVATE: &str = "labyrinth/angel";
const EVENTS: &str = "labyrinth/angel/events.jsonl";
const TRANSACTIONS: &str = "labyrinth/angel/transactions";
const MAX_FILE: usize = 2 * 1024 * 1024;
const MAX_ARTIFACT: usize = 8 * 1024 * 1024;
const MAX_ARCHIVE: usize = 256 * 1024 * 1024;
const MAX_TOTAL: usize = 16 * 1024 * 1024;
const MAX_EVENTS: usize = 8 * 1024 * 1024;
const MAX_JOURNAL: usize = 2 * MAX_TOTAL;
const EVENT_TYPES: &[&str] = &[
    "proposed",
    "computed",
    "proved",
    "refuted",
    "modified",
    "literature",
    "question",
    "answered",
    "reviewed",
    "documented",
    "monitor",
    "milestone",
];

type Change = (PathBuf, Option<Vec<u8>>, Option<Vec<u8>>);

fn error(code: &str, path: Option<&str>, message: impl AsRef<str>) -> String {
    json!({"code":code,"path":path,"message":message.as_ref()}).to_string()
}

fn id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 240
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(error(
            "invalid_identity",
            None,
            "campaign and agent identities must be bounded ASCII names",
        ));
    }
    Ok(())
}

fn relative(value: &str) -> Result<&Path, String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.len() > 1024
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(error(
            "invalid_path",
            Some(value),
            "an unaliased workspace-relative file path is required",
        ));
    }
    Ok(path)
}

fn sha(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(error(
            "invalid_digest",
            None,
            "expected a SHA-256 source identity",
        ));
    }
    Ok(())
}

fn read(workspace: &Path, path: &str, limit: usize) -> Result<Vec<u8>, String> {
    let relative = relative(path)?;
    confined_read_limited_no_symlinks(workspace, relative, limit)
        .map_err(|message| error("artifact_read", Some(path), message))?
        .ok_or_else(|| {
            error(
                "byte_budget",
                Some(path),
                "artifact exceeds its byte budget",
            )
        })
}

fn optional(workspace: &Path, path: &str, limit: usize) -> Result<Option<Vec<u8>>, String> {
    relative(path)?;
    match std::fs::symlink_metadata(workspace.join(path)) {
        Ok(_) => read(workspace, path, limit).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(error("artifact_read", Some(path), e.to_string())),
    }
}

fn json_file(workspace: &Path, path: &str) -> Result<Value, String> {
    serde_json::from_slice(&read(workspace, path, MAX_FILE)?)
        .map_err(|e| error("invalid_json", Some(path), e.to_string()))
}

fn serialized(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| error("serialization", None, e.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_FILE {
        return Err(error(
            "byte_budget",
            None,
            "canonical document exceeds 2 MiB",
        ));
    }
    Ok(bytes)
}

fn private(workspace: &Path) -> Result<(), String> {
    let path = format!("{PRIVATE}/.gitignore");
    if optional(workspace, &path, 4096)?.is_none() {
        match confined_publish_new_no_symlinks(workspace, Path::new(&path), b"*\n") {
            Ok(()) => (),
            Err(_) if read(workspace, &path, 4096).is_ok_and(|b| b == b"*\n") => (),
            Err(e) => return Err(error("private_artifacts", Some(&path), e)),
        }
    }
    if read(workspace, &path, 4096)? != b"*\n" {
        return Err(error(
            "private_artifacts",
            Some(&path),
            "automatic campaign artifacts must remain ignored: .gitignore must contain only '*'",
        ));
    }
    Ok(())
}

struct Lease(std::fs::File);

impl Lease {
    fn acquire(workspace: &Path) -> Result<Self, String> {
        private(workspace)?;
        let file = confined_open_rw_no_symlinks(
            workspace,
            Path::new("labyrinth/angel/workflow.lock"),
            true,
        )
        .map_err(|e| error("workflow_lock", None, e))?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: the strict opened regular file remains owned for the lease.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(error(
                    "workflow_busy",
                    None,
                    "another coordinator holds this workspace",
                ));
            }
        }
        Ok(Self(file))
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: this lease still owns the descriptor being unlocked.
            let _ = unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

#[derive(Debug)]
pub(crate) struct ReadLease(std::fs::File);

impl Drop for ReadLease {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: the shared reader lease owns its strict-opened descriptor.
            let _ = unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

/// Read-only callers never initialize a map or create a lock. Once a coordinator
/// exists, all cooperating readers retain a shared lease through their full read.
pub(crate) fn read_lease(workspace: &Path) -> Result<Option<ReadLease>, String> {
    let path = Path::new("labyrinth/angel/workflow.lock");
    match std::fs::symlink_metadata(workspace.join(path)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(error("workflow_lock", None, e.to_string())),
        Ok(_) => (),
    }
    let file = confined_open_read_no_symlinks(workspace, path)
        .map_err(|e| error("workflow_lock", None, e))?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: the read lease owns this regular file for the lock lifetime.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) } != 0 {
            return Err(error(
                "workflow_busy",
                None,
                "canonical research state is being integrated",
            ));
        }
    }
    Ok(Some(ReadLease(file)))
}

pub(crate) fn now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let z = (seconds / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// Append immediately; do not reconstruct a session's discoveries at its end.
/// The private timeline is merged with authored events by the embedded engine.
pub(crate) fn append_event(workspace: &Path, event: &Value) -> Result<Value, String> {
    let _lease = Lease::acquire(workspace)?;
    ensure_no_pending(workspace)?;
    let (record, bytes) = event_record(event)?;
    confined_append_no_symlinks(workspace, Path::new(EVENTS), &bytes, MAX_EVENTS)
        .map_err(|e| error("event_append", Some(EVENTS), e))?;
    Ok(record)
}

fn event_record(event: &Value) -> Result<(Value, Vec<u8>), String> {
    let kind = event["type"].as_str().unwrap_or_default();
    if !EVENT_TYPES.contains(&kind) {
        return Err(error("event_type", None, "unknown Labyrinth event type"));
    }
    let summary = event["summary"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 8000)
        .ok_or_else(|| {
            error(
                "event_summary",
                None,
                "event needs a bounded factual summary",
            )
        })?;
    for key in ["nodes", "evidence"] {
        if event[key].as_array().is_none_or(|a| {
            a.len() > 128
                || a.iter()
                    .any(|v| v.as_str().is_none_or(|s| s.is_empty() || s.len() > 1024))
        }) {
            return Err(error("event_schema", None, format!("invalid {key}")));
        }
    }
    let mut record = event.clone();
    record["ts"] = json!(now());
    record["summary"] = json!(summary);
    let mut bytes =
        serde_json::to_vec(&record).map_err(|e| error("serialization", None, e.to_string()))?;
    if bytes.len() > 32 * 1024 {
        return Err(error("byte_budget", None, "event exceeds 32 KiB"));
    }
    bytes.push(b'\n');
    Ok((record, bytes))
}

fn publish(workspace: &Path, path: &str, bytes: &[u8]) -> Result<(), String> {
    match confined_publish_new_no_symlinks(workspace, relative(path)?, bytes) {
        Ok(()) => Ok(()),
        Err(_) if read(workspace, path, MAX_FILE).is_ok_and(|old| old == bytes) => Ok(()),
        Err(e) => Err(error("immutable_artifact_conflict", Some(path), e)),
    }
}

/// Retain the final message byte-for-byte and every named piece of agent code
/// and data. The index is published last; no incomplete archive is consumable.
pub(crate) fn archive_report(
    workspace: &Path,
    campaign_id: &str,
    agent_id: &str,
    role: &str,
    source: &Path,
    report: &[u8],
    artifacts: &[String],
) -> Result<Value, String> {
    id(campaign_id)?;
    id(agent_id)?;
    if !matches!(
        role,
        "attack" | "literature" | "referee" | "writer" | "coordinator" | "editor"
    ) {
        return Err(error("agent_role", None, "unknown campaign role"));
    }
    if report.is_empty() || report.len() > MAX_FILE || artifacts.len() > 64 {
        return Err(error(
            "byte_budget",
            None,
            "agent archive exceeds its budget",
        ));
    }
    std::str::from_utf8(report).map_err(|_| error("agent_report", None, "report must be UTF-8"))?;
    let root = format!("{PRIVATE}/campaigns/{campaign_id}/agents/{agent_id}");
    let mut files = Vec::new();
    let mut total = report.len();
    let mut seen = BTreeSet::new();
    for path in artifacts {
        relative(path)?;
        if !seen.insert(path) {
            return Err(error(
                "duplicate_artifact",
                Some(path),
                "duplicate archived source",
            ));
        }
        let bytes = read(source, path, MAX_FILE)?;
        total = total.saturating_add(bytes.len());
        if total > MAX_TOTAL {
            return Err(error("byte_budget", None, "archive exceeds 16 MiB"));
        }
        files.push((format!("{root}/artifacts/{path}"), bytes));
    }
    private(workspace)?;
    let report_path = format!("{root}/report.raw.md");
    publish(workspace, &report_path, report)?;
    let date = now();
    let header = format!(
        "<!-- agent: {agent_id}; role: {role}; date: {date}; not yet independently refereed -->\n"
    );
    let mut annotated = header.into_bytes();
    annotated.extend_from_slice(report);
    publish(workspace, &format!("{root}/report.md"), &annotated)?;
    let mut index = json!({"schema":1,"campaign_id":campaign_id,"agent_id":agent_id,"role":role,
        "report_path":report_path,"report_sha256":sha256_hex(report),"review":"under-review","artifacts":[]});
    for (path, bytes) in files {
        publish(workspace, &path, &bytes)?;
        index["artifacts"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path":path,"sha256":sha256_hex(&bytes)}));
    }
    publish(
        workspace,
        &format!("{root}/archive.json"),
        &serialized(&index)?,
    )?;
    append_event(
        workspace,
        &json!({"type":if role == "literature" {"literature"} else {"computed"},
        "summary":format!("{role} {agent_id} report archived (under review)"),"nodes":[],
        "evidence":[report_path],"campaign_id":campaign_id,"agent_id":agent_id}),
    )?;
    Ok(index)
}

#[derive(Clone)]
struct SourceManifest {
    hash: String,
    files: BTreeMap<String, (String, String)>,
}

fn agent_root(campaign: &str, role: &RoleEvidence) -> Result<String, String> {
    id(campaign)?;
    id(&role.id)?;
    Ok(format!(
        "{PRIVATE}/campaigns/{campaign}/agents/{}/",
        role.id
    ))
}

fn retained(workspace: &Path, path: &str, digest: &str, prefix: &str) -> Result<Vec<u8>, String> {
    sha(digest)?;
    relative(path)?;
    if !path.starts_with(prefix) {
        return Err(error(
            "artifact_owner",
            Some(path),
            "artifact is outside its immutable agent archive",
        ));
    }
    let bytes = read(workspace, path, MAX_ARTIFACT)?;
    if sha256_hex(&bytes) != digest {
        return Err(error(
            "artifact_changed",
            Some(path),
            "retained artifact SHA-256 does not match its receipt",
        ));
    }
    Ok(bytes)
}

fn role_manifest(
    workspace: &Path,
    campaign: &str,
    role: &RoleEvidence,
) -> Result<(Value, SourceManifest), String> {
    let prefix = agent_root(campaign, role)?;
    let report_bytes = retained(workspace, &role.report_path, &role.report_sha256, &prefix)?;
    if report_bytes.len() > 128 * 1024 {
        return Err(error(
            "byte_budget",
            Some(&role.report_path),
            "role report exceeds 128 KiB",
        ));
    }
    let report: Value = serde_json::from_slice(&report_bytes)
        .map_err(|e| error("agent_report", Some(&role.report_path), e.to_string()))?;
    let result_bytes = retained(workspace, &role.result_path, &role.result_sha256, &prefix)?;
    let result: Value = serde_json::from_slice(&result_bytes)
        .map_err(|e| error("agent_result", Some(&role.result_path), e.to_string()))?;
    if result["report_sha256"] != role.report_sha256
        || result["id"] != role.id
        || result["role"] != role.role
    {
        return Err(error(
            "agent_provenance",
            Some(&role.result_path),
            "runtime result does not bind its verbatim role report and identity",
        ));
    }
    if role.artifacts.len() > 256 {
        return Err(error("byte_budget", None, "too many agent artifacts"));
    }
    let mut total = 0usize;
    let mut seen = BTreeSet::new();
    for artifact in &role.artifacts {
        if !seen.insert(&artifact.path) {
            return Err(error(
                "duplicate_artifact",
                Some(&artifact.path),
                "duplicate agent artifact",
            ));
        }
        total += retained(workspace, &artifact.path, &artifact.sha256, &prefix)?.len();
        if total > MAX_ARCHIVE {
            return Err(error("byte_budget", None, "agent artifact budget exceeded"));
        }
    }
    relative(&role.workspace_path)?;
    if !role.workspace_path.starts_with(&prefix) {
        return Err(error(
            "artifact_owner",
            Some(&role.workspace_path),
            "source snapshot is outside this agent archive",
        ));
    }
    let manifest_path = format!("{prefix}source-manifest.json");
    let manifest = json_file(workspace, &manifest_path)?;
    if manifest["schema"] != 1 {
        return Err(error(
            "source_manifest",
            Some(&manifest_path),
            "unsupported source manifest",
        ));
    }
    let files = manifest["files"]
        .as_array()
        .filter(|a| a.len() <= 4096)
        .ok_or_else(|| {
            error(
                "source_manifest",
                Some(&manifest_path),
                "missing bounded file list",
            )
        })?;
    let mut sources = BTreeMap::new();
    for file in files {
        let source_path = file["source_path"]
            .as_str()
            .ok_or_else(|| error("source_manifest", None, "missing source_path"))?;
        relative(source_path)?;
        let path = file["path"]
            .as_str()
            .ok_or_else(|| error("source_manifest", None, "missing archived path"))?;
        let digest = file["sha256"]
            .as_str()
            .ok_or_else(|| error("source_manifest", None, "missing file SHA"))?;
        total += retained(workspace, path, digest, &prefix)?.len();
        if total > MAX_ARCHIVE {
            return Err(error(
                "byte_budget",
                None,
                "source and artifact snapshot exceeds 256 MiB",
            ));
        }
        if sources
            .insert(
                source_path.to_string(),
                (path.to_string(), digest.to_string()),
            )
            .is_some()
        {
            return Err(error(
                "source_manifest",
                Some(source_path),
                "duplicate source name",
            ));
        }
    }
    let mut ordered: Vec<_> = sources
        .iter()
        .map(|(name, (_, hash))| (name.as_str(), hash.as_str()))
        .collect();
    ordered.sort_by(|left, right| Path::new(left.0).cmp(Path::new(right.0)));
    let hash = sha256_hex(
        &serde_json::to_vec(&ordered).map_err(|e| error("serialization", None, e.to_string()))?,
    );
    if manifest["source_sha256"] != hash || manifest["source_after_sha256"] != hash {
        return Err(error(
            "stale_sources",
            Some(&manifest_path),
            "source snapshot or check inputs changed",
        ));
    }
    if result["source_sha256"] != hash {
        return Err(error(
            "agent_provenance",
            Some(&role.result_path),
            "runtime result does not bind its frozen source snapshot",
        ));
    }
    Ok((
        report,
        SourceManifest {
            hash,
            files: sources,
        },
    ))
}

struct Reviewed {
    claims: BTreeMap<String, Value>,
    verdicts: BTreeMap<String, String>,
    reasons: BTreeMap<String, String>,
    author_source_sha256: String,
    referee_source_sha256: String,
    writer_sources: SourceManifest,
    writer_before_sources: SourceManifest,
    coordinator_sources: SourceManifest,
}

fn bounded_items<'a>(value: &'a Value, name: &str) -> Result<&'a Vec<Value>, String> {
    value[name]
        .as_array()
        .filter(|a| a.len() <= 256)
        .ok_or_else(|| error("campaign_schema", None, format!("missing bounded {name}")))
}

fn validate_bundle(
    workspace: &Path,
    bundle: &IntegrationBundle,
    final_checks: bool,
) -> Result<Reviewed, String> {
    id(&bundle.id)?;
    id(&bundle.campaign_id)?;
    id(&bundle.coordinator_id)?;
    let timestamp = bundle.reviewed_at.as_bytes();
    if timestamp.len() != 20
        || timestamp[4] != b'-'
        || timestamp[7] != b'-'
        || timestamp[10] != b'T'
        || timestamp[13] != b':'
        || timestamp[16] != b':'
        || timestamp[19] != b'Z'
        || timestamp.iter().enumerate().any(|(index, byte)| {
            !matches!(index, 4 | 7 | 10 | 13 | 16 | 19) && !byte.is_ascii_digit()
        })
    {
        return Err(error(
            "review_date",
            None,
            "coordinator must bind a fixed ISO UTC review timestamp before checked copies",
        ));
    }
    if bundle.schema != 1
        || bundle.task.is_empty()
        || bundle.task.len() > 16000
        || bundle.claims.len() > 128
        || bundle.nodes.len() > 128
        || bundle.sota.len() > 128
        || bundle.edits.len() > 128
        || bundle.corrections.len() > 128
        || bundle.checks.len() > 32
    {
        return Err(error(
            "campaign_schema",
            None,
            "invalid or over-budget integration bundle",
        ));
    }
    let roles = [
        &bundle.author,
        &bundle.literature,
        &bundle.referee,
        &bundle.writer_before,
        &bundle.writer,
        &bundle.coordinator,
    ];
    let mut identities = BTreeSet::new();
    for (role, expected) in roles.iter().zip([
        "attack",
        "literature",
        "referee",
        "writer",
        "writer",
        "coordinator",
    ]) {
        if role.role != expected || !identities.insert(&role.id) {
            return Err(error(
                "referee_independence",
                None,
                "author, referee, writer and coordinator must be different identified roles",
            ));
        }
    }
    if bundle.coordinator.id != bundle.coordinator_id {
        return Err(error(
            "coordinator_identity",
            None,
            "coordinator archive identity changed",
        ));
    }
    let (author, author_sources) = role_manifest(workspace, &bundle.campaign_id, &bundle.author)?;
    let _ = role_manifest(workspace, &bundle.campaign_id, &bundle.literature)?;
    let (referee, referee_sources) =
        role_manifest(workspace, &bundle.campaign_id, &bundle.referee)?;
    let (writer_before, writer_before_sources) =
        role_manifest(workspace, &bundle.campaign_id, &bundle.writer_before)?;
    let (writer, writer_sources, coordinator_sources) = if final_checks {
        let (writer, sources) = role_manifest(workspace, &bundle.campaign_id, &bundle.writer)?;
        let (_, coordinator) = role_manifest(workspace, &bundle.campaign_id, &bundle.coordinator)?;
        let metadata = json_file(workspace, &bundle.writer.result_path)?;
        if writer != writer_before
            || metadata["draft_role_id"] != bundle.writer_before.id
            || metadata["draft_result_sha256"] != bundle.writer_before.result_sha256
        {
            return Err(error(
                "writer_drift",
                None,
                "checked writer copy is not bound to the retained original writer draft",
            ));
        }
        (writer, sources, coordinator)
    } else {
        (
            writer_before,
            SourceManifest {
                hash: String::new(),
                files: BTreeMap::new(),
            },
            SourceManifest {
                hash: String::new(),
                files: BTreeMap::new(),
            },
        )
    };
    let originals = bounded_items(&author, "claims")?;
    let mut original_claims = BTreeMap::new();
    for claim in originals {
        let claim_id = claim["id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 240)
            .ok_or_else(|| error("claim_identity", None, "author claim needs an id"))?;
        if original_claims
            .insert(claim_id.to_string(), claim.clone())
            .is_some()
        {
            return Err(error(
                "claim_identity",
                Some(claim_id),
                "duplicate author claim",
            ));
        }
    }
    let referee_verdicts = bounded_items(&referee, "verdicts")?;
    let mut verdicts = BTreeMap::new();
    let mut reasons = BTreeMap::new();
    let mut required_corrections = BTreeMap::new();
    for verdict in referee_verdicts {
        let claim = verdict["claim_id"]
            .as_str()
            .filter(|s| original_claims.contains_key(*s))
            .ok_or_else(|| {
                error(
                    "referee_verdict",
                    None,
                    "referee names an unknown author claim",
                )
            })?;
        let status = verdict["verdict"].as_str().unwrap_or_default();
        if !matches!(
            status,
            "ESTABLISHED" | "ESTABLISHED WITH CORRECTIONS" | "GAP" | "FALSE"
        ) || verdict["reason"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty() || s.len() > 16000)
            || verdicts
                .insert(claim.to_string(), status.to_string())
                .is_some()
        {
            return Err(error(
                "referee_verdict",
                Some(claim),
                "invalid or duplicate independent verdict",
            ));
        }
        reasons.insert(
            claim.to_string(),
            verdict["reason"].as_str().unwrap().to_string(),
        );
        let fixes = verdict["corrections"].as_array();
        if !verdict["corrections"].is_null() && fixes.is_none()
            || (status == "ESTABLISHED WITH CORRECTIONS")
                != fixes.is_some_and(|fixes| !fixes.is_empty())
        {
            return Err(error(
                "corrections_missing",
                Some(claim),
                "only a corrections verdict may carry its nonempty exact correction list",
            ));
        }
        for correction in fixes.into_iter().flatten() {
            serde_json::from_value::<Correction>(correction.clone()).map_err(|_| {
                error(
                    "correction_schema",
                    None,
                    "invalid correction text field or schema",
                )
            })?;
            let correction_id = correction["id"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 240)
                .ok_or_else(|| {
                    error("correction_schema", None, "referee correction needs an id")
                })?;
            if correction["claim_id"] != claim
                || correction["old"]
                    .as_str()
                    .is_none_or(|s| s.is_empty() || s.len() > 16000)
                || correction["new"].as_str().is_none_or(|s| s.len() > 16000)
                || required_corrections
                    .insert(correction_id.to_string(), correction.clone())
                    .is_some()
            {
                return Err(error(
                    "correction_schema",
                    Some(correction_id),
                    "invalid or duplicate exact correction",
                ));
            }
        }
        if status == "ESTABLISHED WITH CORRECTIONS"
            && !required_corrections
                .values()
                .any(|c| c["claim_id"] == claim)
        {
            return Err(error(
                "corrections_missing",
                Some(claim),
                "corrections verdict has no retained exact correction",
            ));
        }
    }
    if verdicts.len() != original_claims.len() {
        return Err(error(
            "referee_verdict",
            None,
            "every author claim needs an independent verdict",
        ));
    }
    let declared: BTreeSet<_> = bundle.applied_corrections.iter().collect();
    let written: BTreeSet<_> = writer["applied_corrections"]
        .as_array()
        .ok_or_else(|| {
            error(
                "corrections_missing",
                None,
                "writer did not list applied corrections",
            )
        })?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if declared.len() != bundle.applied_corrections.len()
        || declared.len() != required_corrections.len()
        || declared
            .iter()
            .any(|c| !required_corrections.contains_key(c.as_str()))
        || written.len() != required_corrections.len()
        || written
            .iter()
            .any(|c| !required_corrections.contains_key(*c))
        || bundle.corrections.len() != required_corrections.len()
    {
        return Err(error(
            "corrections_missing",
            None,
            "all referee corrections must be applied, without extras",
        ));
    }
    for correction in &bundle.corrections {
        let saved = required_corrections.get(&correction.id).ok_or_else(|| {
            error(
                "correction_schema",
                Some(&correction.id),
                "correction was not made by the referee",
            )
        })?;
        if saved["claim_id"] != correction.claim_id
            || saved["old"] != correction.old
            || saved["new"] != correction.new
            || saved.get("field").cloned().unwrap_or(Value::Null)
                != serde_json::to_value(correction.field).unwrap()
        {
            return Err(error(
                "correction_changed",
                Some(&correction.id),
                "correction differs from the retained referee report",
            ));
        }
    }
    let writer_claims = bounded_items(&writer, "claims")?;
    let mut final_claims = BTreeMap::new();
    for claim in &bundle.claims {
        let claim_id = claim["id"]
            .as_str()
            .filter(|s| original_claims.contains_key(*s))
            .ok_or_else(|| {
                error(
                    "claim_identity",
                    None,
                    "integrated claim was not reported by the author",
                )
            })?;
        let original = &original_claims[claim_id];
        original["statement"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 16000)
            .ok_or_else(|| {
                error(
                    "claim_schema",
                    Some(claim_id),
                    "claim needs a bounded statement",
                )
            })?;
        let raw_verdict = referee_verdicts
            .iter()
            .find(|verdict| verdict["claim_id"] == claim_id)
            .unwrap();
        let verdict = &verdicts[claim_id];
        let mut corrected_text = original.clone();
        if verdict == "GAP" {
            corrected_text["test"] = raw_verdict
                .get("test")
                .filter(|value| value.is_string())
                .cloned()
                .unwrap_or_else(|| original["test"].clone());
            corrected_text["lesson"] = raw_verdict["reason"].clone();
        } else if verdict == "FALSE" {
            corrected_text["lesson"] = raw_verdict
                .get("lesson")
                .filter(|value| {
                    value
                        .as_str()
                        .is_some_and(|text| !text.trim().is_empty() && text.len() <= 8000)
                })
                .cloned()
                .unwrap_or_else(|| raw_verdict["reason"].clone());
        }
        for correction in &bundle.corrections {
            if correction.claim_id == claim_id {
                apply_claim_correction(&mut corrected_text, correction)
                    .map_err(|message| error("correction_anchor", Some(&correction.id), message))?;
            }
        }
        let writer_claim = writer_claims
            .iter()
            .find(|c| c["id"] == claim_id)
            .ok_or_else(|| {
                error(
                    "writer_drift",
                    Some(claim_id),
                    "writer omitted the checked claim",
                )
            })?;
        let original_status = original["status"].as_str().unwrap_or_default();
        let expected_status = if verdict == "FALSE" {
            "REFUTED"
        } else if verdict == "GAP" {
            "CONJECTURE"
        } else {
            original_status
        };
        if !matches!(
            expected_status,
            "PROVED"
                | "PROVED-CONDITIONAL"
                | "COMPUTED"
                | "EVIDENCE"
                | "CONJECTURE"
                | "REFUTED"
                | "DEAD END"
        ) || claim["statement"] != corrected_text["statement"]
            || writer_claim["statement"] != corrected_text["statement"]
            || claim["status"] != expected_status
            || writer_claim["status"] != expected_status
            || claim["referee"]["id"] != bundle.referee.id
            || claim["referee"]["verdict"] != *verdict
        {
            return Err(error(
                "writer_drift",
                Some(claim_id),
                "writer changed a claim beyond the referee's exact corrections or verdict",
            ));
        }
        if matches!(expected_status, "CONJECTURE")
            && claim["test"].as_str().is_none_or(|s| s.trim().is_empty())
        {
            return Err(error(
                "conjecture_test",
                Some(claim_id),
                "conjecture needs its concrete test",
            ));
        }
        if claim["route"] != original["route"] {
            return Err(error(
                "writer_drift",
                Some(claim_id),
                "the attempted route differs from the retained author's report",
            ));
        }
        if verdict == "GAP" {
            let test = raw_verdict["test"]
                .as_str()
                .or_else(|| original["test"].as_str())
                .filter(|test| !test.trim().is_empty() && test.len() <= 8000);
            if test.is_none()
                || claim["test"].as_str() != test
                || claim["lesson"] != raw_verdict["reason"]
            {
                return Err(error(
                    "failed_route_provenance",
                    Some(claim_id),
                    "a checked gap must retain the referee's reason and its referee/author next test",
                ));
            }
        }
        if matches!(expected_status, "REFUTED" | "DEAD END") {
            if claim["lesson"].as_str().is_none_or(|s| s.trim().is_empty()) {
                return Err(error(
                    "refutation_lesson",
                    Some(claim_id),
                    "a closed route needs its lesson",
                ));
            }
            if expected_status == "REFUTED" {
                let witness = claim_artifact(workspace, bundle, &claim["counterexample"], true)?;
                if verdict == "FALSE" {
                    let raw = raw_verdict["counterexample"].as_str().ok_or_else(|| {
                        error(
                            "claim_artifact",
                            Some(claim_id),
                            "FALSE verdict has no retained referee-owned counterexample",
                        )
                    })?;
                    relative(raw)?;
                    let expected =
                        format!("{}{raw}", agent_root(&bundle.campaign_id, &bundle.referee)?);
                    let lesson = raw_verdict
                        .get("lesson")
                        .and_then(Value::as_str)
                        .filter(|lesson| !lesson.trim().is_empty() && lesson.len() <= 8000)
                        .unwrap_or(raw_verdict["reason"].as_str().unwrap());
                    if !raw.starts_with("artifacts/")
                        || witness != expected
                        || claim["lesson"] != lesson
                    {
                        return Err(error(
                            "failed_route_provenance",
                            Some(claim_id),
                            "refutation must retain the exact independently reviewed witness and lesson",
                        ));
                    }
                }
            }
        }
        if ["test", "lesson"].into_iter().any(|field| {
            claim[field] != corrected_text[field] || writer_claim[field] != corrected_text[field]
        }) {
            return Err(error(
                "writer_drift",
                Some(claim_id),
                "writer changed claim text beyond the referee's exact corrections or verdict",
            ));
        }
        if matches!(expected_status, "PROVED" | "PROVED-CONDITIONAL") {
            claim_artifact(workspace, bundle, &claim["proof"], false)?;
        }
        if final_claims
            .insert(claim_id.to_string(), claim.clone())
            .is_some()
        {
            return Err(error(
                "claim_identity",
                Some(claim_id),
                "duplicate integrated claim",
            ));
        }
    }
    if final_claims.len() != original_claims.len() || writer_claims.len() != original_claims.len() {
        return Err(error(
            "writer_drift",
            None,
            "writer/integration must retain every independently reviewed claim",
        ));
    }
    let mut checked_ids = BTreeSet::new();
    let mut counts = BTreeMap::<&str, usize>::new();
    for receipt in &bundle.checks {
        if !final_checks && matches!(receipt.stage.as_str(), "writer" | "coordinator") {
            continue;
        }
        id(&receipt.id)?;
        if !checked_ids.insert(&receipt.id)
            || !matches!(
                receipt.stage.as_str(),
                "referee" | "writer-before" | "writer" | "coordinator"
            )
            || receipt.exit_code != Some(0)
            || receipt.timed_out
            || receipt.cancelled
            || !receipt.fresh
            || receipt.command.is_empty()
            || receipt.command.len() > 64
            || receipt.command.iter().any(|s| s.len() > 4096)
        {
            return Err(error(
                "check_failed",
                Some(&receipt.id),
                "a fresh successful coordinator-run check is required",
            ));
        }
        let (role, source) = match receipt.stage.as_str() {
            "referee" => (&bundle.referee, &referee_sources),
            "writer-before" => (&bundle.writer_before, &writer_before_sources),
            "writer" => (&bundle.writer, &writer_sources),
            _ => (&bundle.coordinator, &coordinator_sources),
        };
        let prefix = agent_root(&bundle.campaign_id, role)?;
        if receipt.workspace_path != role.workspace_path
            || receipt.source_sha256 != source.hash
            || receipt.source_after_sha256 != source.hash
            || receipt.command_sha256
                != sha256_hex(
                    &serde_json::to_vec(&receipt.command)
                        .map_err(|e| error("serialization", None, e.to_string()))?,
                )
        {
            return Err(error(
                "stale_check",
                Some(&receipt.id),
                "check receipt does not bind the retained source and argv",
            ));
        }
        retained(
            workspace,
            &receipt.output_path,
            &receipt.output_sha256,
            &prefix,
        )?;
        if receipt.output_artifacts.len() > 1024 {
            return Err(error(
                "byte_budget",
                Some(&receipt.id),
                "check output inventory exceeds its bounded file count",
            ));
        }
        let mut output_names = BTreeSet::new();
        for artifact in &receipt.output_artifacts {
            if !output_names.insert(&artifact.path) {
                return Err(error(
                    "check_output",
                    Some(&receipt.id),
                    "check output inventory contains duplicate paths",
                ));
            }
            retained(workspace, &artifact.path, &artifact.sha256, &prefix)?;
        }
        if let Some(code) = &receipt.code_path {
            let digest = receipt
                .code_sha256
                .as_deref()
                .ok_or_else(|| error("check_code", Some(code), "check code has no SHA-256"))?;
            if receipt
                .code_after_sha256
                .as_deref()
                .is_some_and(|after| after != digest)
            {
                return Err(error(
                    "check_code",
                    Some(code),
                    "independent code changed during the fresh check",
                ));
            }
            retained(workspace, code, digest, &prefix)?;
            if receipt.stage == "referee" {
                if bundle
                    .author
                    .artifacts
                    .iter()
                    .any(|artifact| artifact.sha256 == digest)
                    || author_sources
                        .files
                        .values()
                        .any(|(_, author_hash)| author_hash == digest)
                {
                    return Err(error(
                        "referee_independence",
                        Some(code),
                        "referee reused an author's input/code artifact",
                    ));
                }
                let specified = referee["independent_check"]["code"]
                    .as_str()
                    .ok_or_else(|| {
                        error(
                            "referee_independence",
                            None,
                            "referee did not retain its own verification code",
                        )
                    })?;
                let specified_command: Vec<String> = serde_json::from_value(
                    referee["independent_check"]["argv"].clone(),
                )
                .map_err(|_| {
                    error(
                        "referee_independence",
                        None,
                        "referee must retain its exact executable check argv",
                    )
                })?;
                if specified_command != receipt.command {
                    return Err(error(
                        "referee_independence",
                        Some(code),
                        "check receipt argv differs from the referee's retained independent code execution",
                    ));
                }
                super::campaign::validate_independent_command(&receipt.command, specified)
                    .map_err(|e| error("referee_independence", Some(code), e))?;
                if specified != code
                    && format!("{prefix}{specified}") != *code
                    && !source
                        .files
                        .get(specified)
                        .is_some_and(|(_, hash)| hash == digest)
                {
                    return Err(error(
                        "referee_independence",
                        Some(code),
                        "executed code differs from the referee's specified independent check",
                    ));
                }
            }
        } else if receipt.stage == "referee" {
            return Err(error(
                "referee_independence",
                None,
                "referee check must execute its retained own code",
            ));
        }
        *counts.entry(receipt.stage.as_str()).or_default() += 1;
    }
    if counts.get("referee").copied().unwrap_or(0) < 1
        || final_checks
            && (counts.get("writer-before").copied().unwrap_or(0) < 1
                || counts.get("writer").copied().unwrap_or(0) < 2
                || counts.get("coordinator").copied().unwrap_or(0) < 1)
    {
        return Err(error(
            "checks_missing",
            None,
            "require independent referee, two writer-copy builds, and the coordinator spot check",
        ));
    }
    Ok(Reviewed {
        claims: final_claims,
        verdicts,
        reasons,
        author_source_sha256: author_sources.hash,
        referee_source_sha256: referee_sources.hash,
        writer_sources,
        writer_before_sources,
        coordinator_sources,
    })
}

fn claim_artifact(
    workspace: &Path,
    bundle: &IntegrationBundle,
    reference: &Value,
    referee_allowed: bool,
) -> Result<String, String> {
    let path = reference
        .as_str()
        .or_else(|| reference["path"].as_str())
        .ok_or_else(|| {
            error(
                "claim_artifact",
                None,
                "claim requires a retained proof, witness or counterexample artifact",
            )
        })?;
    for role in [&bundle.author, &bundle.referee] {
        if role.id == bundle.referee.id && !referee_allowed {
            continue;
        }
        if let Some(artifact) = role.artifacts.iter().find(|a| a.path == path) {
            if reference["sha256"]
                .as_str()
                .is_some_and(|hash| hash != artifact.sha256)
            {
                return Err(error(
                    "artifact_changed",
                    Some(path),
                    "claim artifact digest differs from its archive",
                ));
            }
            retained(
                workspace,
                path,
                &artifact.sha256,
                &agent_root(&bundle.campaign_id, role)?,
            )?;
            return Ok(path.to_string());
        }
    }
    Err(error(
        "claim_artifact",
        Some(path),
        "claim artifact is absent from the author's/referee's retained code and data",
    ))
}

fn canonical_path(path: &str) -> Result<&Path, String> {
    let relative = relative(path)?;
    let forbidden = [
        ".git",
        ".agents",
        ".codex",
        ".aws",
        ".env",
        "target",
        "node_modules",
        "labyrinth/angel",
        "labyrinth/knowledge.json",
        "labyrinth/sota.json",
        "labyrinth/frontier.json",
        "labyrinth/events.jsonl",
        "labyrinth/lab.py",
        "labyrinth/dashboard",
    ];
    if forbidden
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
    {
        return Err(error(
            "canonical_path",
            Some(path),
            "writer edits must name explicit reader-facing documents, not runtime, generated files or coordinator data",
        ));
    }
    let extension = relative
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if !matches!(extension, "md" | "tex" | "rst" | "txt" | "adoc") {
        return Err(error(
            "canonical_path",
            Some(path),
            "writer edits are restricted to reader-facing text documents",
        ));
    }
    Ok(relative)
}

fn document_changes(
    workspace: &Path,
    bundle: &IntegrationBundle,
    reviewed: &Reviewed,
) -> Result<Vec<Change>, String> {
    let writer = json_file(workspace, &bundle.writer.report_path)?;
    let proposed = bounded_items(&writer, "edits")?;
    if proposed.len() != bundle.edits.len()
        || proposed.iter().zip(&bundle.edits).any(|(proposal, edit)| {
            proposal["path"] != edit.path
                || proposal["old"] != edit.old
                || proposal["new"] != edit.new
        })
    {
        return Err(error(
            "writer_drift",
            None,
            "canonical edits differ from the retained writer draft",
        ));
    }
    validate_proposals(&writer, bundle)?;
    let mut files = BTreeMap::<String, (Vec<u8>, Vec<u8>)>::new();
    for edit in &bundle.edits {
        canonical_path(&edit.path)?;
        sha(&edit.before_sha256)?;
        sha(&edit.after_sha256)?;
        if edit.old.is_empty()
            || edit.old.len() > DOCUMENT_EDIT_BYTES
            || edit.new.len() > DOCUMENT_EDIT_BYTES
        {
            return Err(error(
                "edit_budget",
                Some(&edit.path),
                "each edit needs nonempty OLD text and OLD/NEW fragments within 128 KiB",
            ));
        }
        if !files.contains_key(&edit.path) {
            let original = read(workspace, &edit.path, MAX_FILE)?;
            if reviewed
                .writer_before_sources
                .files
                .get(&edit.path)
                .is_none_or(|(_, digest)| digest != &sha256_hex(&original))
            {
                return Err(error(
                    "stale_writer_sources",
                    Some(&edit.path),
                    "writer baseline check did not use these canonical preimage bytes",
                ));
            }
            files.insert(edit.path.clone(), (original.clone(), original));
        }
        let (_, current) = files.get_mut(&edit.path).unwrap();
        if sha256_hex(current) != edit.before_sha256 {
            return Err(error(
                "stale_writer_sources",
                Some(&edit.path),
                "canonical source changed since this writer edit was tested",
            ));
        }
        let text = std::str::from_utf8(current).map_err(|_| {
            error(
                "canonical_document",
                Some(&edit.path),
                "document is not UTF-8",
            )
        })?;
        if text.match_indices(&edit.old).count() != 1 {
            return Err(error(
                "edit_anchor",
                Some(&edit.path),
                "each OLD must occur exactly once, including after preceding edits",
            ));
        }
        let updated = text.replacen(&edit.old, &edit.new, 1).into_bytes();
        if updated.len() > MAX_FILE || sha256_hex(&updated) != edit.after_sha256 {
            return Err(error(
                "writer_drift",
                Some(&edit.path),
                "edit output does not match its tested source identity",
            ));
        }
        *current = updated;
    }
    let mut changes = Vec::new();
    for (path, (before, after)) in files {
        let hash = sha256_hex(&after);
        if [&reviewed.writer_sources, &reviewed.coordinator_sources]
            .iter()
            .any(|sources| {
                sources
                    .files
                    .get(&path)
                    .is_none_or(|(_, actual)| actual != &hash)
            })
        {
            return Err(error(
                "stale_writer_check",
                Some(&path),
                "both writer-copy builds and coordinator replay must check these exact edited bytes",
            ));
        }
        changes.push((PathBuf::from(path), Some(before), Some(after)));
    }
    Ok(changes)
}

fn validate_proposals(writer: &Value, bundle: &IntegrationBundle) -> Result<(), String> {
    for (key, value) in [
        ("nodes", json!(bundle.nodes)),
        ("sota", json!(bundle.sota)),
        ("frontier", bundle.frontier.clone().unwrap_or(Value::Null)),
    ] {
        if writer[key] != value {
            return Err(error(
                "writer_drift",
                None,
                format!("{key} update differs from the retained writer draft"),
            ));
        }
    }
    Ok(())
}

fn extend_strings(
    node: &mut Value,
    key: &str,
    additions: impl IntoIterator<Item = String>,
) -> Result<(), String> {
    if node.get(key).is_none() {
        node[key] = json!([]);
    }
    let values = node[key]
        .as_array_mut()
        .ok_or_else(|| error("node_schema", None, format!("{key} must be an array")))?;
    for addition in additions {
        if !values.iter().any(|value| value == &addition) {
            values.push(json!(addition));
        }
    }
    Ok(())
}

fn merge_nodes(
    workspace: &Path,
    knowledge: &mut Value,
    bundle: &IntegrationBundle,
    reviewed: &Reviewed,
) -> Result<(), String> {
    let nodes = knowledge["nodes"].as_array_mut().ok_or_else(|| {
        error(
            "knowledge_schema",
            Some("labyrinth/knowledge.json"),
            "missing nodes array",
        )
    })?;
    let mut seen = BTreeSet::new();
    for patch in &bundle.nodes {
        let node_id = patch["id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 240)
            .ok_or_else(|| error("node_identity", None, "canonical node update needs an id"))?;
        if !seen.insert(node_id) {
            return Err(error(
                "node_identity",
                Some(node_id),
                "duplicate canonical update",
            ));
        }
        let mut node = nodes
            .iter()
            .find(|n| n["id"] == node_id)
            .cloned()
            .unwrap_or_else(|| json!({}));
        let previous = node.clone();
        let patch_fields = patch
            .as_object()
            .ok_or_else(|| error("node_schema", None, "node update must be an object"))?;
        if patch_fields
            .get("failed_routes")
            .is_some_and(|value| value != &previous["failed_routes"])
        {
            return Err(error(
                "failed_route_provenance",
                Some(node_id),
                "only deterministic checked-negative projection appends failed routes",
            ));
        }
        if previous["tier"] == "T1" {
            return Err(error(
                "tier_discipline",
                Some(node_id),
                "a campaign cannot rewrite or reclassify a published T1 result",
            ));
        }
        for (key, value) in patch_fields {
            if matches!(key.as_str(), "links" | "evidence" | "where")
                && node[key].is_array()
                && value.is_array()
            {
                let values = node[key].as_array_mut().unwrap();
                for addition in value.as_array().unwrap() {
                    if !values.contains(addition) {
                        values.push(addition.clone());
                    }
                }
            } else {
                node[key] = value.clone();
            }
        }
        let kind = node["kind"].as_str().unwrap_or_default().to_string();
        let carries_claim = |kind: &str| {
            matches!(
                kind,
                "theorem" | "exhaustive" | "evidence" | "conjecture" | "hunch" | "deadend"
            )
        };
        if carries_claim(previous["kind"].as_str().unwrap_or_default()) && !carries_claim(&kind) {
            return Err(error(
                "tier_discipline",
                Some(node_id),
                "reclassifying a claim as a tool/source cannot bypass its review and evidence boundary",
            ));
        }
        if matches!(kind.as_str(), "method" | "source" | "family" | "question")
            && !node["tier"].is_null()
        {
            return Err(error(
                "tier_discipline",
                Some(node_id),
                "tools, references and open doors do not carry result tiers",
            ));
        }
        if matches!(
            kind.as_str(),
            "theorem" | "exhaustive" | "evidence" | "conjecture" | "deadend"
        ) {
            let provenance_error = || {
                error(
                    "node_provenance",
                    Some(node_id),
                    "claim-bearing updates require an explicit reviewed claim ID, a matching node ID, or one unique exact reviewed statement",
                )
            };
            let claim_id = match node.get("claim_id") {
                Some(value) => value
                    .as_str()
                    .filter(|id| reviewed.claims.contains_key(*id))
                    .ok_or_else(provenance_error)?
                    .to_string(),
                None if reviewed.claims.contains_key(node_id) => node_id.to_string(),
                None => {
                    let statement = node["statement"].as_str().ok_or_else(provenance_error)?;
                    let mut matches = reviewed
                        .claims
                        .iter()
                        .filter(|(_, claim)| claim["statement"].as_str() == Some(statement));
                    let (id, _) = matches.next().ok_or_else(provenance_error)?;
                    if matches.next().is_some() {
                        return Err(provenance_error());
                    }
                    id.clone()
                }
            };
            let claim = reviewed.claims.get(&claim_id).ok_or_else(|| {
                error(
                    "node_provenance",
                    Some(node_id),
                    "claim-bearing updates must name a retained independently reviewed claim",
                )
            })?;
            let verdict = &reviewed.verdicts[&claim_id];
            let established = matches!(
                verdict.as_str(),
                "ESTABLISHED" | "ESTABLISHED WITH CORRECTIONS"
            );
            if node["statement"] != claim["statement"] {
                return Err(error(
                    "writer_drift",
                    Some(node_id),
                    "canonical statement differs from the independently corrected claim",
                ));
            }
            node["claim_id"] = json!(claim_id);
            let tier = node["tier"].as_str().unwrap_or_default().to_string();
            if tier == "T1" {
                return Err(error(
                    "tier_discipline",
                    Some(node_id),
                    "campaign referees cannot create or modify published T1 claims",
                ));
            }
            if tier == "T2" {
                if !established
                    || !matches!(
                        claim["status"].as_str(),
                        Some("PROVED" | "PROVED-CONDITIONAL")
                    )
                    || kind != "theorem"
                {
                    return Err(error(
                        "tier_discipline",
                        Some(node_id),
                        "T2 requires a retained proof and independently established mathematical claim",
                    ));
                }
                let proof = node.get("proof_artifact").unwrap_or(&claim["proof"]);
                let proof_path = claim_artifact(workspace, bundle, proof, false)?;
                extend_strings(&mut node, "evidence", [proof_path])?;
                if claim["status"] == "PROVED-CONDITIONAL"
                    && node["hypothesis"]
                        .as_str()
                        .is_none_or(|s| s.trim().is_empty())
                {
                    return Err(error(
                        "tier_discipline",
                        Some(node_id),
                        "conditional theorem must retain its named hypothesis",
                    ));
                }
            } else if tier == "T3" {
                let certificate = &node["certification"];
                if !established
                    || claim["status"] != "COMPUTED"
                    || kind != "exhaustive"
                    || certificate["exhaustive"] != true
                    || certificate["scope"]
                        .as_str()
                        .is_none_or(|s| s.trim().is_empty())
                {
                    return Err(error(
                        "tier_discipline",
                        Some(node_id),
                        "T3 requires explicit exhaustive scope and independently validated certificate code",
                    ));
                }
                let code = claim_artifact(workspace, bundle, &certificate["code"], false)?;
                let independent =
                    claim_artifact(workspace, bundle, &certificate["independent_code"], true)?;
                let witness = claim_artifact(workspace, bundle, &certificate["witness"], false)?;
                if code == independent
                    || sha256_hex(&read(workspace, &code, MAX_ARTIFACT)?)
                        == sha256_hex(&read(workspace, &independent, MAX_ARTIFACT)?)
                    || !independent.starts_with(&agent_root(&bundle.campaign_id, &bundle.referee)?)
                {
                    return Err(error(
                        "tier_discipline",
                        Some(node_id),
                        "T3 validation must retain an independent implementation",
                    ));
                }
                if !bundle.checks.iter().any(|receipt| {
                    receipt.stage == "referee"
                        && receipt.code_path.as_deref() == Some(independent.as_str())
                }) {
                    return Err(error(
                        "tier_discipline",
                        Some(node_id),
                        "the retained independent certificate implementation must be the referee code actually executed",
                    ));
                }
                extend_strings(&mut node, "evidence", [code, independent, witness])?;
            } else if matches!(kind.as_str(), "theorem" | "exhaustive") {
                return Err(error(
                    "tier_discipline",
                    Some(node_id),
                    "unchecked computation belongs in T4 evidence, not an established theorem",
                ));
            }
            if kind == "evidence" && tier != "T4" {
                return Err(error(
                    "tier_discipline",
                    Some(node_id),
                    "review agreement keeps computational evidence at T4",
                ));
            }
            if node["status"] == "refuted" || kind == "deadend" {
                if !matches!(claim["status"].as_str(), Some("REFUTED" | "DEAD END")) {
                    return Err(error(
                        "refutation_provenance",
                        Some(node_id),
                        "refutation was not reviewed",
                    ));
                }
                if claim["status"] == "REFUTED" {
                    let witness =
                        claim_artifact(workspace, bundle, &claim["counterexample"], true)?;
                    extend_strings(&mut node, "evidence", [witness])?;
                }
                node["lesson"] = claim["lesson"].clone();
            }
            if previous["tier"] != node["tier"]
                && matches!(tier.as_str(), "T2" | "T3")
                && !patch_fields.contains_key("tier")
            {
                return Err(error(
                    "tier_discipline",
                    Some(node_id),
                    "tier promotion must be explicit in the coordinator's update",
                ));
            }
            let new_review = json!({"state":if established {"refereed"} else {"under-review"},
                "by":[bundle.referee.id],"verdict":if established {"ESTABLISHED"} else {verdict.as_str()},"date":bundle.reviewed_at,
                "report":bundle.referee.report_path,"report_sha256":bundle.referee.report_sha256,
                "coordinator":bundle.coordinator_id,"human_check":"pending"});
            if previous["review"].is_object() {
                if node.get("review_history").is_none() {
                    node["review_history"] = json!([]);
                }
                let history = node["review_history"]
                    .as_array_mut()
                    .filter(|a| a.len() < 256)
                    .ok_or_else(|| {
                        error(
                            "review_history",
                            Some(node_id),
                            "review history exceeds its budget",
                        )
                    })?;
                history.push(json!({"review":previous["review"],"statement_sha256":sha256_hex(previous["statement"].as_str().unwrap_or_default().as_bytes())}));
            }
            let mut review = node["review"].as_object().cloned().unwrap_or_default();
            for (key, value) in new_review.as_object().unwrap() {
                review.insert(key.clone(), value.clone());
            }
            node["review"] = json!(review);
            extend_strings(
                &mut node,
                "evidence",
                [
                    bundle.author.report_path.clone(),
                    bundle.referee.report_path.clone(),
                ],
            )?;
            node["campaign_provenance"] = json!({"campaign":bundle.campaign_id,"integration":bundle.id,"claim":claim_id,
                "author":bundle.author.id,"referee":bundle.referee.id,"writer":bundle.writer.id,
                "coordinator":bundle.coordinator_id,
                "author_source_sha256":reviewed.author_source_sha256,
                "referee_source_sha256":reviewed.referee_source_sha256,
                "checks_receipt":format!("{PRIVATE}/campaigns/{}/integrations/{}.json",bundle.campaign_id,bundle.id)});
        }
        node["updated"] = json!(&bundle.reviewed_at[..10]);
        if let Some(existing) = nodes.iter_mut().find(|n| n["id"] == node_id) {
            *existing = node;
        } else {
            nodes.push(node);
        }
    }
    Map::parse(knowledge.clone())
        .map_err(|e| error("knowledge_validation", Some("labyrinth/knowledge.json"), e))?;
    Ok(())
}

fn merge_sota(
    sota: &mut Value,
    bundle: &IntegrationBundle,
    reviewed: &Reviewed,
    knowledge: &Value,
) -> Result<(), String> {
    if !sota.is_object() {
        return Err(error(
            "sota_schema",
            Some("labyrinth/sota.json"),
            "SOTA must be an object",
        ));
    }
    let mut groups = sota["groups"]
        .as_array()
        .filter(|a| a.len() <= 256)
        .ok_or_else(|| error("sota_schema", None, "missing bounded groups"))?
        .clone();
    if groups
        .iter()
        .any(|g| g.as_str().is_none_or(|s| s.is_empty() || s.len() > 240))
    {
        return Err(error("sota_schema", None, "invalid group name"));
    }
    let entries = sota["entries"]
        .as_array_mut()
        .filter(|a| a.len() <= 4096)
        .ok_or_else(|| error("sota_schema", None, "missing bounded entries"))?;
    let mut ids = BTreeSet::new();
    for patch in &bundle.sota {
        let entry_id = patch["id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 240)
            .ok_or_else(|| error("sota_schema", None, "SOTA entry needs an id"))?;
        if !ids.insert(entry_id) {
            return Err(error("sota_schema", Some(entry_id), "duplicate SOTA patch"));
        }
        let mut row = entries
            .iter()
            .find(|row| row["id"] == entry_id)
            .cloned()
            .unwrap_or_else(|| json!({}));
        let old = row.clone();
        let fields = patch
            .as_object()
            .ok_or_else(|| error("sota_schema", None, "SOTA patch must be an object"))?;
        for (key, value) in fields {
            if key != "previous" {
                row[key] = value.clone();
            }
        }
        for key in ["id", "group", "cls", "result", "status"] {
            if row[key]
                .as_str()
                .is_none_or(|s| s.is_empty() || s.len() > 16000)
            {
                return Err(error(
                    "sota_schema",
                    Some(entry_id),
                    format!("missing bounded {key}"),
                ));
            }
        }
        for key in ["refs", "lit"] {
            if row[key].as_array().is_none_or(|a| {
                a.len() > 128 || a.iter().any(|v| v.as_str().is_none_or(|s| s.len() > 1024))
            }) {
                return Err(error(
                    "sota_schema",
                    Some(entry_id),
                    format!("invalid {key}"),
                ));
            }
        }
        let claim_id = row["claim_id"]
            .as_str()
            .or_else(|| {
                row["node_id"].as_str().and_then(|node_id| {
                    knowledge["nodes"]
                        .as_array()?
                        .iter()
                        .find(|n| n["id"] == node_id)?["claim_id"]
                        .as_str()
                })
            })
            .ok_or_else(|| {
                error(
                    "sota_provenance",
                    Some(entry_id),
                    "new best-result rows must name their reviewed claim or node",
                )
            })?;
        let claim = reviewed.claims.get(claim_id).ok_or_else(|| {
            error(
                "sota_provenance",
                Some(entry_id),
                "SOTA claim was not independently reviewed in this campaign",
            )
        })?;
        let verdict = &reviewed.verdicts[claim_id];
        if !matches!(
            verdict.as_str(),
            "ESTABLISHED" | "ESTABLISHED WITH CORRECTIONS"
        ) {
            return Err(error(
                "sota_provenance",
                Some(entry_id),
                "a gap or false verdict cannot become the best established result",
            ));
        }
        if matches!(
            claim["status"].as_str(),
            Some("EVIDENCE" | "CONJECTURE" | "DEAD END")
        ) && matches!(row["kind"].as_str(), Some("proved" | "exhaustive"))
        {
            return Err(error(
                "tier_discipline",
                Some(entry_id),
                "SOTA cannot describe computational evidence/conjectures as proved or exhaustive",
            ));
        }
        let mut previous = old["previous"].as_array().cloned().unwrap_or_default();
        if patch
            .get("previous")
            .is_some_and(|p| p.as_array().is_none_or(|a| a != &previous))
        {
            return Err(error(
                "sota_history",
                Some(entry_id),
                "only the coordinator appends history; writers cannot replace it",
            ));
        }
        if old["result"]
            .as_str()
            .is_some_and(|result| row["result"] != result)
        {
            let date = old["updated"]
                .as_str()
                .unwrap_or(&bundle.reviewed_at[..10])
                .to_string();
            previous.push(
                json!({"date":date,"was":old["result"],"status":old["status"],"refs":old["refs"]}),
            );
        }
        if previous.len() > 4096 {
            return Err(error(
                "sota_history",
                Some(entry_id),
                "SOTA history budget exceeded",
            ));
        }
        row["previous"] = json!(previous);
        row["updated"] = json!(&bundle.reviewed_at[..10]);
        row["status"] = json!(format!(
            "{} (independent AI referee {}; coordinator {}; human check pending)",
            row["status"].as_str().unwrap(),
            bundle.referee.id,
            bundle.coordinator_id
        ));
        row["review"] = json!({"state":"refereed","by":[bundle.referee.id],"report":bundle.referee.report_path,
            "report_sha256":bundle.referee.report_sha256,"campaign":bundle.id});
        if !groups.contains(&row["group"]) {
            groups.push(row["group"].clone());
        }
        if let Some(existing) = entries.iter_mut().find(|e| e["id"] == entry_id) {
            *existing = row;
        } else {
            entries.push(row);
        }
    }
    sota["groups"] = json!(groups);
    validate_sota(sota)?;
    Ok(())
}

fn validate_sota(sota: &Value) -> Result<(), String> {
    let groups = sota["groups"]
        .as_array()
        .filter(|a| a.len() <= 256)
        .ok_or_else(|| error("sota_schema", None, "SOTA needs bounded groups"))?;
    let entries = sota["entries"]
        .as_array()
        .filter(|a| a.len() <= 4096)
        .ok_or_else(|| error("sota_schema", None, "SOTA needs bounded entries"))?;
    let mut seen = BTreeSet::new();
    for row in entries {
        let name = row["id"]
            .as_str()
            .ok_or_else(|| error("sota_schema", None, "SOTA row needs an id"))?;
        if !seen.insert(name) || !groups.contains(&row["group"]) {
            return Err(error(
                "sota_schema",
                Some(name),
                "duplicate SOTA row or unknown group",
            ));
        }
        for key in ["cls", "result", "status", "updated"] {
            if row[key]
                .as_str()
                .is_none_or(|s| s.is_empty() || s.len() > 16000)
            {
                return Err(error("sota_schema", Some(name), format!("invalid {key}")));
            }
        }
        for key in ["refs", "lit"] {
            if row[key]
                .as_array()
                .is_none_or(|a| a.len() > 128 || a.iter().any(|v| v.as_str().is_none()))
            {
                return Err(error("sota_schema", Some(name), format!("invalid {key}")));
            }
        }
        if let Some(previous) = row.get("previous") {
            let history = previous
                .as_array()
                .filter(|a| a.len() <= 4096)
                .ok_or_else(|| {
                    error("sota_history", Some(name), "history is not a bounded list")
                })?;
            for old in history {
                if old["date"]
                    .as_str()
                    .is_none_or(|s| s.is_empty() || s.len() > 80)
                    || old["was"]
                        .as_str()
                        .is_none_or(|s| s.is_empty() || s.len() > 16000)
                {
                    return Err(error(
                        "sota_history",
                        Some(name),
                        "history needs every date and old result",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn integer(value: &Value, name: &str) -> Result<i64, String> {
    value
        .as_i64()
        .filter(|n| n.unsigned_abs() <= 9_007_199_254_740_991)
        .ok_or_else(|| {
            error(
                "frontier_number",
                None,
                format!("{name} must be an exactly represented bounded integer"),
            )
        })
}

fn union(mut ranges: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    ranges.sort_unstable();
    let mut combined: Vec<(u64, u64)> = Vec::new();
    for (lo, hi) in ranges {
        if let Some(last) = combined
            .last_mut()
            .filter(|last| lo <= last.1.saturating_add(1))
        {
            last.1 = last.1.max(hi);
        } else {
            combined.push((lo, hi));
        }
    }
    combined
}

fn overlap(left: &[(u64, u64)], right: &[(u64, u64)]) -> bool {
    let (mut a, mut b) = (0, 0);
    while a < left.len() && b < right.len() {
        if left[a].0 <= right[b].1 && right[b].0 <= left[a].1 {
            return true;
        }
        if left[a].1 < right[b].0 {
            a += 1;
        } else {
            b += 1;
        }
    }
    false
}

fn count(ranges: &[(u64, u64)]) -> u64 {
    ranges.iter().map(|(lo, hi)| hi - lo + 1).sum()
}

/// Compute exact interval cardinalities without expanding large admissible grids.
/// Unknown and conditional points never inflate the resolved fraction.
pub(crate) fn frontier_status(frontier: &Value, sota: Option<&Value>) -> Result<Value, String> {
    let sizes = frontier["sizes"]
        .as_array()
        .filter(|a| a.len() <= 256)
        .ok_or_else(|| {
            error(
                "frontier_schema",
                Some("labyrinth/frontier.json"),
                "frontier needs bounded sizes",
            )
        })?;
    let mut seen = BTreeSet::new();
    let mut summary = Vec::new();
    for row in sizes {
        let size = integer(&row["size"], "size")?;
        if !seen.insert(size) {
            return Err(error("frontier_schema", None, "duplicate frontier size"));
        }
        let lo = integer(&row["lo"], "lo")?;
        let hi = integer(&row["hi"], "hi")?;
        let step = integer(row.get("step").unwrap_or(&json!(1)), "step")?;
        if step <= 0 || hi < lo {
            return Err(error(
                "frontier_schema",
                None,
                "invalid admissible range or step",
            ));
        }
        let admissible = ((hi - lo) / step) as u64 + 1;
        let grid = |value: &Value| -> Result<u64, String> {
            let n = integer(value, "frontier value")?;
            if n < lo || n > hi || (n - lo) % step != 0 {
                return Err(error(
                    "frontier_grid",
                    None,
                    "claimed value is outside the admissible grid",
                ));
            }
            Ok(((n - lo) / step) as u64)
        };
        let mut buckets = BTreeMap::<&str, Vec<(u64, u64)>>::new();
        for (key, status) in [
            ("realized", "realized"),
            ("impossible", "impossible"),
            ("conditional", "gap-conditional"),
            ("conj_impossible", "conj-impossible"),
        ] {
            if let Some(items) = row.get(key) {
                let list = items
                    .as_array()
                    .filter(|a| a.len() <= 8192)
                    .ok_or_else(|| {
                        error(
                            "frontier_schema",
                            None,
                            "frontier point list exceeds budget",
                        )
                    })?;
                for item in list {
                    let value = if let Some(array) = item.as_array() {
                        array
                            .first()
                            .ok_or_else(|| error("frontier_schema", None, "empty realization"))?
                    } else {
                        item
                    };
                    let index = grid(value)?;
                    buckets.entry(status).or_default().push((index, index));
                }
            }
        }
        if let Some(intervals) = row.get("intervals") {
            let list = intervals
                .as_array()
                .filter(|a| a.len() <= 8192)
                .ok_or_else(|| {
                    error("frontier_schema", None, "frontier intervals exceed budget")
                })?;
            for interval in list {
                let status = interval["status"].as_str().unwrap_or_default();
                if !matches!(
                    status,
                    "realized" | "impossible" | "gap-conditional" | "conj-impossible" | "unknown"
                ) {
                    return Err(error(
                        "frontier_schema",
                        None,
                        "unknown frontier classification",
                    ));
                }
                let first = grid(&interval["v0"])?;
                let last = grid(&interval["v1"])?;
                if last < first {
                    return Err(error("frontier_grid", None, "reversed frontier interval"));
                }
                if status != "unknown" {
                    if interval["why"].as_str().is_none_or(|s| s.trim().is_empty()) {
                        return Err(error(
                            "frontier_provenance",
                            None,
                            "classified ranges need their witness, proof or named hypothesis",
                        ));
                    }
                    buckets.entry(status).or_default().push((first, last));
                }
            }
        }
        let realized = union(buckets.remove("realized").unwrap_or_default());
        let impossible = union(buckets.remove("impossible").unwrap_or_default());
        if overlap(&realized, &impossible) {
            return Err(error(
                "frontier_contradiction",
                None,
                format!("size {size} is claimed both realized and impossible"),
            ));
        }
        let resolved = count(&realized) + count(&impossible);
        let all_claimed = union(
            realized
                .iter()
                .chain(impossible.iter())
                .chain(buckets.values().flatten())
                .copied()
                .collect(),
        );
        let conditional = count(&all_claimed) - resolved;
        if let Some(bounds) = row.get("bounds") {
            let object = bounds
                .as_object()
                .ok_or_else(|| error("frontier_schema", None, "bounds must be an object"))?;
            let mut numeric = BTreeMap::new();
            for (field, value) in object {
                if value.is_number() {
                    let n = integer(value, field)?;
                    if n < lo || n > hi {
                        return Err(error(
                            "frontier_bounds",
                            None,
                            "numerical bound is outside the declared admissible range",
                        ));
                    }
                    numeric.insert(field.as_str(), n);
                }
            }
            if numeric
                .get("lower_proved")
                .zip(numeric.get("upper"))
                .is_some_and(|(lower, upper)| lower > upper)
            {
                return Err(error(
                    "frontier_bounds",
                    None,
                    "proved lower bound exceeds upper bound",
                ));
            }
        }
        summary.push(json!({"size":size,"admissible":admissible,"realized":count(&realized),
            "impossible":count(&impossible),"resolved":resolved,"conditional_or_conjectural":conditional,
            "unknown":admissible-count(&all_claimed),"resolved_fraction":resolved as f64 / admissible as f64}));
    }
    if let Some(sota) = sota {
        validate_sota(sota)?;
        for entry in sota["entries"].as_array().unwrap() {
            if let Some(binding) = entry.get("frontier") {
                let size = integer(&binding["size"], "SOTA size")?;
                let field = binding["field"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        error(
                            "sota_frontier",
                            None,
                            "numerical SOTA binding needs its bound field",
                        )
                    })?;
                let value = integer(&binding["value"], "SOTA bound")?;
                let row = sizes
                    .iter()
                    .find(|row| row["size"] == size)
                    .ok_or_else(|| {
                        error(
                            "sota_frontier",
                            None,
                            "SOTA size is absent from the frontier",
                        )
                    })?;
                if row["bounds"][field] != value {
                    return Err(error(
                        "sota_frontier",
                        None,
                        format!(
                            "SOTA entry {} and frontier size {size} disagree on {field}",
                            entry["id"]
                        ),
                    ));
                }
            }
        }
    }
    Ok(json!({"sizes":summary}))
}

fn validate_new_frontier(
    frontier: &Value,
    old: Option<&Value>,
    knowledge: &Value,
) -> Result<(), String> {
    for row in frontier["sizes"].as_array().into_iter().flatten() {
        for interval in row["intervals"].as_array().into_iter().flatten() {
            if interval["status"] != "impossible" {
                continue;
            }
            let unchanged = old
                .and_then(|f| f["sizes"].as_array())
                .into_iter()
                .flatten()
                .filter(|prior| prior["size"] == row["size"])
                .any(|prior| {
                    prior["intervals"]
                        .as_array()
                        .is_some_and(|list| list.contains(interval))
                });
            if unchanged {
                continue;
            }
            let why = interval["why"].as_str().unwrap_or_default();
            let node = knowledge["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|node| node["id"] == why)
                .ok_or_else(|| {
                    error(
                        "frontier_provenance",
                        Some(why),
                        "new impossible ranges need their retained proved or certified node",
                    )
                })?;
            if !matches!(node["tier"].as_str(), Some("T1" | "T2" | "T3"))
                || super::passage(node) != super::Passage::Charted
            {
                return Err(error(
                    "frontier_provenance",
                    Some(why),
                    "computational evidence cannot establish impossibility",
                ));
            }
        }
        let prior_points = old
            .and_then(|f| f["sizes"].as_array())
            .and_then(|rows| rows.iter().find(|prior| prior["size"] == row["size"]))
            .and_then(|prior| prior["impossible"].as_array());
        if row["impossible"].as_array().is_some_and(|points| {
            points
                .iter()
                .any(|point| prior_points.is_none_or(|old| !old.contains(point)))
        }) {
            return Err(error(
                "frontier_provenance",
                None,
                "new impossible points must use intervals carrying their proof-node reference",
            ));
        }
    }
    Ok(())
}

fn merge_frontier(old: Option<&Value>, patch: &Value) -> Result<Value, String> {
    let mut frontier = old
        .cloned()
        .unwrap_or_else(|| json!({"meta":{},"sizes":[]}));
    let fields = patch
        .as_object()
        .ok_or_else(|| error("frontier_schema", None, "frontier update must be an object"))?;
    for (key, value) in fields {
        if key == "sizes" {
            let updates = value.as_array().filter(|a| a.len() <= 256).ok_or_else(|| {
                error(
                    "frontier_schema",
                    None,
                    "frontier size update is not bounded",
                )
            })?;
            let sizes = frontier["sizes"]
                .as_array_mut()
                .ok_or_else(|| error("frontier_schema", None, "frontier needs sizes"))?;
            let mut seen = BTreeSet::new();
            for update in updates {
                let size = integer(&update["size"], "size")?;
                if !seen.insert(size) {
                    return Err(error("frontier_schema", None, "duplicate frontier update"));
                }
                if let Some(row) = sizes.iter_mut().find(|row| row["size"] == size) {
                    merge_object(row, update);
                } else {
                    sizes.push(update.clone());
                }
            }
        } else if value.is_object() && frontier[key].is_object() {
            merge_object(&mut frontier[key], value);
        } else {
            frontier[key] = value.clone();
        }
    }
    Ok(frontier)
}

fn merge_object(target: &mut Value, patch: &Value) {
    if target.is_object() && patch.is_object() {
        for (key, value) in patch.as_object().unwrap() {
            if target[key].is_object() && value.is_object() {
                merge_object(&mut target[key], value);
            } else {
                target[key] = value.clone();
            }
        }
    } else {
        *target = patch.clone();
    }
}

fn source_bindings(
    workspace: &Path,
    bundle: &IntegrationBundle,
) -> Result<BTreeMap<String, Option<Vec<u8>>>, String> {
    let bundle_json =
        serde_json::to_value(bundle).map_err(|e| error("serialization", None, e.to_string()))?;
    let declared = bundle_json["canonical_sources"]
        .as_array()
        .filter(|a| a.len() == 3)
        .ok_or_else(|| {
            error(
                "canonical_sources",
                None,
                "campaign must retain identities of knowledge, SOTA and frontier at launch",
            )
        })?;
    let receipt_path = bundle_json["canonical_sources_path"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "{PRIVATE}/campaigns/{}/canonical-sources.json",
                bundle.campaign_id
            )
        });
    relative(&receipt_path)?;
    let expected_prefix = format!("{PRIVATE}/campaigns/{}/", bundle.campaign_id);
    if !receipt_path.starts_with(&expected_prefix)
        || !receipt_path.ends_with("/canonical-sources.json")
        || receipt_path.contains("/workspaces/")
    {
        return Err(error(
            "canonical_sources",
            Some(&receipt_path),
            "invalid launch source-receipt owner",
        ));
    }
    let bytes = read(workspace, &receipt_path, MAX_FILE)?;
    if bundle_json["canonical_sources_sha256"]
        .as_str()
        .is_none_or(|digest| sha256_hex(&bytes) != digest)
    {
        return Err(error(
            "artifact_changed",
            Some(&receipt_path),
            "launch source receipt changed",
        ));
    }
    let saved: Value = serde_json::from_slice(&bytes)
        .map_err(|e| error("canonical_sources", Some(&receipt_path), e.to_string()))?;
    let saved_sources = saved
        .as_array()
        .or_else(|| saved["sources"].as_array())
        .ok_or_else(|| {
            error(
                "canonical_sources",
                Some(&receipt_path),
                "source receipt needs an immutable array",
            )
        })?;
    if saved_sources != declared {
        return Err(error(
            "canonical_sources",
            Some(&receipt_path),
            "integration source identities differ from the retained launch receipt",
        ));
    }
    let required: BTreeSet<&str> = [
        "labyrinth/knowledge.json",
        "labyrinth/sota.json",
        "labyrinth/frontier.json",
    ]
    .into_iter()
    .collect();
    let mut found = BTreeMap::new();
    for source in declared {
        let path = source["path"]
            .as_str()
            .filter(|path| required.contains(*path))
            .ok_or_else(|| error("canonical_sources", None, "unexpected canonical source"))?;
        let current = optional(workspace, path, MAX_FILE)?;
        let expected = source["sha256"].as_str();
        if expected.is_some() {
            sha(expected.unwrap())?;
        }
        if current.as_ref().map(|bytes| sha256_hex(bytes)).as_deref() != expected {
            return Err(error(
                "stale_canonical_sources",
                Some(path),
                "canonical research state changed after this campaign was briefed",
            ));
        }
        if found.insert(path.to_string(), current).is_some() {
            return Err(error(
                "canonical_sources",
                Some(path),
                "duplicate source identity",
            ));
        }
    }
    if found.len() != required.len() || found["labyrinth/knowledge.json"].is_none() {
        return Err(error(
            "canonical_sources",
            None,
            "campaign requires an initialized, source-bound research map",
        ));
    }
    Ok(found)
}

fn transaction_names(workspace: &Path) -> Result<Vec<String>, String> {
    let directory = workspace.join(TRANSACTIONS);
    if std::fs::symlink_metadata(&directory)
        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        return Ok(Vec::new());
    }
    // This strict anchor also rejects any aliased parent before enumeration.
    read(workspace, &format!("{PRIVATE}/.gitignore"), 4096)?;
    let metadata = std::fs::symlink_metadata(&directory)
        .map_err(|e| error("transaction_read", Some(TRANSACTIONS), e.to_string()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(error(
            "transaction_read",
            Some(TRANSACTIONS),
            "transactions must be an unaliased directory",
        ));
    }
    let mut names = Vec::new();
    let mut entries = 0usize;
    for entry in std::fs::read_dir(&directory)
        .map_err(|e| error("transaction_read", Some(TRANSACTIONS), e.to_string()))?
    {
        entries += 1;
        if entries > 1024 {
            return Err(error("byte_budget", None, "too many transaction artifacts"));
        }
        let name = entry
            .map_err(|e| error("transaction_read", None, e.to_string()))?
            .file_name();
        let name = name
            .to_str()
            .ok_or_else(|| error("transaction_read", None, "invalid transaction filename"))?;
        if let Some(token) = name.strip_suffix(".prepared.json") {
            sha(token)?;
            names.push(token.to_string());
        }
        if names.len() > 256 {
            return Err(error(
                "byte_budget",
                None,
                "transaction journal budget exceeded; archive completed campaigns",
            ));
        }
    }
    names.sort();
    Ok(names)
}

fn pending_transactions(workspace: &Path) -> Result<Vec<String>, String> {
    let mut pending = Vec::new();
    let mut total = 0usize;
    for token in transaction_names(workspace)? {
        let prepared_path = format!("{TRANSACTIONS}/{token}.prepared.json");
        let bytes = read(workspace, &prepared_path, MAX_JOURNAL)?;
        total += bytes.len();
        if total > 4 * MAX_TOTAL {
            return Err(error(
                "byte_budget",
                Some(TRANSACTIONS),
                "transaction journals exceed aggregate 64 MiB budget",
            ));
        }
        let journal: Value = serde_json::from_slice(&bytes)
            .map_err(|e| error("transaction_read", Some(&prepared_path), e.to_string()))?;
        if journal["schema"] != 1 || journal["id"] != token {
            return Err(error(
                "transaction_read",
                Some(&prepared_path),
                "invalid transaction identity",
            ));
        }
        let mut terminal = 0;
        for state in ["complete", "aborted"] {
            if let Some(marker) = optional(
                workspace,
                &format!("{TRANSACTIONS}/{token}.{state}.json"),
                MAX_FILE,
            )? {
                let marker: Value = serde_json::from_slice(&marker)
                    .map_err(|e| error("transaction_read", None, e.to_string()))?;
                if marker["id"] != token
                    || marker["state"] != state
                    || marker["prepared_sha256"] != sha256_hex(&bytes)
                {
                    return Err(error(
                        "transaction_read",
                        None,
                        "terminal transaction marker does not bind its prepared journal",
                    ));
                }
                terminal += 1;
            }
        }
        if terminal > 1 {
            return Err(error(
                "transaction_read",
                None,
                "transaction has conflicting terminal markers",
            ));
        }
        if terminal == 0 {
            pending.push(token);
        }
    }
    Ok(pending)
}

pub(crate) fn ensure_no_pending(workspace: &Path) -> Result<(), String> {
    let pending = pending_transactions(workspace)?;
    if !pending.is_empty() {
        return Err(error(
            "integration_pending",
            Some(TRANSACTIONS),
            format!(
                "interrupted coordinator transaction(s): {}; run labyrinth campaign recover before consuming the map",
                pending.join(", ")
            ),
        ));
    }
    Ok(())
}

pub(crate) fn ensure_ready(workspace: &Path) -> Result<(), String> {
    ensure_no_pending(workspace)
}

fn marker(workspace: &Path, token: &str, state: &str, prepared: &[u8]) -> Result<(), String> {
    publish(
        workspace,
        &format!("{TRANSACTIONS}/{token}.{state}.json"),
        &serialized(
            &json!({"schema":1,"id":token,"state":state,"prepared_sha256":sha256_hex(prepared)}),
        )?,
    )
}

fn journal_changes(journal: &Value) -> Result<Vec<Change>, String> {
    let changes = journal["changes"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 64)
        .ok_or_else(|| {
            error(
                "transaction_read",
                None,
                "transaction needs bounded file changes",
            )
        })?;
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut total = 0usize;
    for change in changes {
        let path = change["path"]
            .as_str()
            .ok_or_else(|| error("transaction_read", None, "missing transaction path"))?;
        if !seen.insert(path) {
            return Err(error(
                "transaction_read",
                Some(path),
                "duplicate transaction target",
            ));
        }
        if !matches!(
            path,
            "labyrinth/knowledge.json" | "labyrinth/sota.json" | "labyrinth/frontier.json" | EVENTS
        ) {
            canonical_path(path)?;
        }
        let data = |key| -> Result<Option<Vec<u8>>, String> {
            if change[key].is_null() {
                return Ok(None);
            }
            let bytes = change[key]
                .as_str()
                .ok_or_else(|| {
                    error(
                        "transaction_read",
                        Some(path),
                        "transaction source is not text",
                    )
                })?
                .as_bytes()
                .to_vec();
            if bytes.len() > MAX_FILE && path != EVENTS {
                return Err(error(
                    "byte_budget",
                    Some(path),
                    "journal file exceeds 2 MiB",
                ));
            }
            Ok(Some(bytes))
        };
        let before = data("before")?;
        let after = data("after")?;
        total += before.as_ref().map_or(0, Vec::len) + after.as_ref().map_or(0, Vec::len);
        if total > MAX_TOTAL {
            return Err(error(
                "byte_budget",
                None,
                "transaction content exceeds 16 MiB",
            ));
        }
        result.push((PathBuf::from(path), before, after));
    }
    Ok(result)
}

/// Explicit recovery never treats partially installed canonical state as valid.
/// It restores only bytes that are still exactly a prepared before/after image.
pub(crate) fn recover(workspace: &Path) -> Result<Value, String> {
    let _lease = Lease::acquire(workspace)?;
    let mut recovered = Vec::new();
    for token in pending_transactions(workspace)? {
        let prepared = read(
            workspace,
            &format!("{TRANSACTIONS}/{token}.prepared.json"),
            MAX_JOURNAL,
        )?;
        let journal: Value = serde_json::from_slice(&prepared)
            .map_err(|e| error("transaction_read", None, e.to_string()))?;
        let mut rollback = Vec::new();
        for (path, before, after) in journal_changes(&journal)? {
            let name = path.to_str().unwrap();
            let current = optional(
                workspace,
                name,
                if name == EVENTS { MAX_EVENTS } else { MAX_FILE },
            )?;
            if current == before {
                continue;
            }
            if current != after {
                return Err(error(
                    "recovery_conflict",
                    Some(name),
                    "canonical file contains independent edits; recovery will not overwrite them",
                ));
            }
            rollback.push((path, after, before));
        }
        if !rollback.is_empty() {
            confined_compare_replace_batch_no_symlinks(workspace, &rollback)
                .map_err(|e| error("recovery_failed", None, e))?;
        }
        marker(workspace, &token, "aborted", &prepared)?;
        recovered.push(token);
    }
    ensure_no_pending(workspace)?;
    Ok(json!({"recovered":recovered,"canonical_state":"restored"}))
}

fn event_count(workspace: &Path, path: &str) -> Result<usize, String> {
    let Some(bytes) = optional(workspace, path, MAX_EVENTS)? else {
        return Ok(0);
    };
    if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
        return Err(error(
            "event_log",
            Some(path),
            "event journal has a partial final record",
        ));
    }
    let mut events = 0;
    for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        if line.len() > 32 * 1024 {
            return Err(error(
                "byte_budget",
                Some(path),
                "event record exceeds 32 KiB",
            ));
        }
        let event: Value = serde_json::from_slice(line)
            .map_err(|e| error("event_log", Some(path), e.to_string()))?;
        event_record(&event)?;
        if event["ts"]
            .as_str()
            .is_none_or(|s| s.is_empty() || s.len() > 80)
        {
            return Err(error(
                "event_log",
                Some(path),
                "event lacks its original timestamp",
            ));
        }
        events += 1;
        if events > 32768 {
            return Err(error(
                "byte_budget",
                Some(path),
                "event-count budget exceeded",
            ));
        }
    }
    Ok(events)
}

/// Validate authored data, private timeline and transaction state together.
/// This deliberately does not execute workspace-authored code or blueprints.
pub(crate) fn check(workspace: &Path) -> Result<Value, String> {
    let _lease = read_lease(workspace)?;
    ensure_no_pending(workspace)?;
    let knowledge = json_file(workspace, "labyrinth/knowledge.json")?;
    let map = Map::parse(knowledge.clone())
        .map_err(|e| error("knowledge_validation", Some("labyrinth/knowledge.json"), e))?;
    let sota = optional(workspace, "labyrinth/sota.json", MAX_FILE)?
        .map(|b| {
            serde_json::from_slice::<Value>(&b)
                .map_err(|e| error("sota_schema", Some("labyrinth/sota.json"), e.to_string()))
        })
        .transpose()?;
    if let Some(sota) = &sota {
        validate_sota(sota)?;
    }
    let frontier = optional(workspace, "labyrinth/frontier.json", MAX_FILE)?
        .map(|b| {
            serde_json::from_slice::<Value>(&b).map_err(|e| {
                error(
                    "frontier_schema",
                    Some("labyrinth/frontier.json"),
                    e.to_string(),
                )
            })
        })
        .transpose()?;
    let frontier = frontier
        .as_ref()
        .map(|f| frontier_status(f, sota.as_ref()))
        .transpose()?;
    let events =
        event_count(workspace, EVENTS)? + event_count(workspace, "labyrinth/events.jsonl")?;
    let mut passages = BTreeMap::<&str, usize>::new();
    for node in &map.nodes {
        *passages.entry(super::passage(node).label()).or_default() += 1;
    }
    Ok(
        json!({"valid":true,"nodes":map.nodes.len(),"passages":passages,"events":events,
        "sota_entries":sota.as_ref().and_then(|s| s["entries"].as_array()).map_or(0, Vec::len),
        "frontier":frontier,"pending_transactions":[],"artifact_visibility":"local/private"}),
    )
}

pub(crate) fn status(workspace: &Path) -> Result<Value, String> {
    check(workspace)
}

pub(crate) struct ArtifactIntegrator;

impl CampaignIntegrator for ArtifactIntegrator {
    fn integrate(
        &self,
        workspace: &Path,
        bundle: &IntegrationBundle,
        cancel: &AtomicBool,
    ) -> Result<Value, String> {
        integrate(workspace, bundle, cancel)
    }

    fn event(&self, workspace: &Path, event: &Value) -> Result<(), String> {
        append_event(workspace, event).map(|_| ())
    }

    fn preview(&self, workspace: &Path, bundle: &IntegrationBundle) -> Result<Value, String> {
        preview_canonical(workspace, bundle)
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/labyrinth/workflow__tests.rs"]
mod tests;

fn completed_outcome(
    workspace: &Path,
    bundle: &IntegrationBundle,
    bundle_sha256: &str,
) -> Result<Option<Value>, String> {
    for token in transaction_names(workspace)? {
        let complete = format!("{TRANSACTIONS}/{token}.complete.json");
        if optional(workspace, &complete, MAX_FILE)?.is_none() {
            continue;
        }
        let prepared = read(
            workspace,
            &format!("{TRANSACTIONS}/{token}.prepared.json"),
            MAX_JOURNAL,
        )?;
        let journal: Value = serde_json::from_slice(&prepared)
            .map_err(|e| error("transaction_read", None, e.to_string()))?;
        if journal["bundle_sha256"] != bundle_sha256
            || journal["bundle"]["id"] != bundle.id
            || journal["bundle"]["campaign_id"] != bundle.campaign_id
        {
            continue;
        }
        let mut differences = Vec::new();
        for (path, _, after) in journal_changes(&journal)? {
            let name = path.to_str().unwrap();
            let current = optional(
                workspace,
                name,
                if name == EVENTS { MAX_EVENTS } else { MAX_FILE },
            )?;
            let same = if name == EVENTS {
                current
                    .as_ref()
                    .zip(after.as_ref())
                    .is_some_and(|(current, after)| current.starts_with(after))
            } else {
                current == after
            };
            if !same {
                differences.push(name.to_string());
            }
        }
        let mut outcome = journal["outcome"].clone();
        if outcome["integrated"] != true {
            return Err(error(
                "transaction_read",
                None,
                "completed journal lacks its original integration outcome",
            ));
        }
        outcome["already_integrated"] = json!(true);
        outcome["canonical_current"] = json!(differences.is_empty());
        outcome["changed_since_integration"] = json!(differences);
        return Ok(Some(outcome));
    }
    Ok(None)
}

fn canonical_changes(
    workspace: &Path,
    bundle: &IntegrationBundle,
    reviewed: &Reviewed,
    sources: &BTreeMap<String, Option<Vec<u8>>>,
) -> Result<(Vec<Change>, Option<Value>), String> {
    let mut knowledge: Value =
        serde_json::from_slice(sources["labyrinth/knowledge.json"].as_ref().unwrap())
            .map_err(|e| error("knowledge_schema", None, e.to_string()))?;
    Map::parse(knowledge.clone()).map_err(|e| error("knowledge_validation", None, e))?;
    let mut changes = Vec::new();
    merge_nodes(workspace, &mut knowledge, bundle, &reviewed)?;
    project_failed_routes(&mut knowledge, bundle, reviewed)?;
    let old_frontier: Option<Value> = sources["labyrinth/frontier.json"]
        .as_ref()
        .map(|b| {
            serde_json::from_slice(b).map_err(|e| error("frontier_schema", None, e.to_string()))
        })
        .transpose()?;
    let frontier = bundle
        .frontier
        .as_ref()
        .map(|patch| merge_frontier(old_frontier.as_ref(), patch))
        .transpose()?
        .or_else(|| old_frontier.clone());
    let mut sota: Value = sources["labyrinth/sota.json"]
        .as_ref()
        .map(|b| serde_json::from_slice(b).map_err(|e| error("sota_schema", None, e.to_string())))
        .transpose()?
        .unwrap_or_else(|| json!({"about":"","groups":[],"entries":[]}));
    merge_sota(&mut sota, bundle, &reviewed, &knowledge)?;
    let numerical = frontier
        .as_ref()
        .map(|f| frontier_status(f, Some(&sota)))
        .transpose()?;
    if bundle.frontier.is_some() {
        validate_new_frontier(
            frontier.as_ref().unwrap(),
            old_frontier.as_ref(),
            &knowledge,
        )?;
        if let Some(numerical) = &numerical {
            let old_status = old_frontier
                .as_ref()
                .map(|f| frontier_status(f, None))
                .transpose()?;
            for size in numerical["sizes"].as_array().unwrap() {
                let old = old_status
                    .as_ref()
                    .and_then(|old| old["sizes"].as_array())
                    .and_then(|rows| rows.iter().find(|row| row["size"] == size["size"]));
                if old.is_some_and(|row| {
                    row["resolved"] == size["resolved"] && row["admissible"] == size["admissible"]
                }) {
                    continue;
                }
                if knowledge.get("frontier_history").is_none() {
                    knowledge["frontier_history"] = json!([]);
                }
                let history = knowledge["frontier_history"]
                    .as_array_mut()
                    .filter(|a| a.len() < 4096)
                    .ok_or_else(|| {
                        error(
                            "frontier_history",
                            None,
                            "frontier history exceeds its budget",
                        )
                    })?;
                history.push(json!({"date":&bundle.reviewed_at[..10],"size":size["size"],"resolved":size["resolved_fraction"],
                    "why":format!("independently reviewed campaign {}", bundle.id),"campaign":bundle.id}));
            }
        }
        changes.push((
            PathBuf::from("labyrinth/frontier.json"),
            sources["labyrinth/frontier.json"].clone(),
            Some(serialized(frontier.as_ref().unwrap())?),
        ));
    }
    // The knowledge file also serves as an unchanged CAS guard if this campaign
    // documents a reviewed failure without proposing graph updates.
    let knowledge_bytes = if bundle.nodes.is_empty()
        && bundle.frontier.is_none()
        && !reviewed
            .verdicts
            .values()
            .any(|verdict| matches!(verdict.as_str(), "GAP" | "FALSE"))
    {
        sources["labyrinth/knowledge.json"]
            .as_ref()
            .unwrap()
            .clone()
    } else {
        serialized(&knowledge)?
    };
    changes.push((
        PathBuf::from("labyrinth/knowledge.json"),
        sources["labyrinth/knowledge.json"].clone(),
        Some(knowledge_bytes),
    ));
    if !bundle.sota.is_empty() {
        changes.push((
            PathBuf::from("labyrinth/sota.json"),
            sources["labyrinth/sota.json"].clone(),
            Some(serialized(&sota)?),
        ));
    }
    for path in ["labyrinth/sota.json", "labyrinth/frontier.json"] {
        if !changes.iter().any(|(name, _, _)| name == Path::new(path)) {
            if let Some(bytes) = &sources[path] {
                changes.push((
                    PathBuf::from(path),
                    Some(bytes.clone()),
                    Some(bytes.clone()),
                ));
            }
        }
    }
    Ok((changes, numerical))
}

fn project_failed_routes(
    knowledge: &mut Value,
    bundle: &IntegrationBundle,
    reviewed: &Reviewed,
) -> Result<(), String> {
    let negative: Vec<_> = reviewed
        .verdicts
        .iter()
        .filter(|(_, verdict)| matches!(verdict.as_str(), "GAP" | "FALSE"))
        .collect();
    if negative.is_empty() {
        return Ok(());
    }
    let node = knowledge["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["id"] == bundle.door_id)
        .ok_or_else(|| {
            error(
                "failed_route_door",
                Some(&bundle.door_id),
                "checked negative outcome must name its existing selected research door",
            )
        })?;
    if !matches!(
        node["kind"].as_str(),
        Some("question" | "conjecture" | "hunch")
    ) {
        return Err(error(
            "failed_route_door",
            Some(&bundle.door_id),
            "route attempts annotate an unresolved question, conjecture or hunch",
        ));
    }
    if node.get("failed_routes").is_none() {
        node["failed_routes"] = json!([]);
    }
    let routes = node["failed_routes"]
        .as_array_mut()
        .filter(|routes| routes.len() < 128)
        .ok_or_else(|| {
            error(
                "failed_route_budget",
                Some(&bundle.door_id),
                "failed routes require bounded curated history",
            )
        })?;
    let receipt = bundle
        .checks
        .iter()
        .find(|receipt| receipt.stage == "referee")
        .ok_or_else(|| {
            error(
                "checks_missing",
                None,
                "negative outcomes need their independently executed referee check",
            )
        })?;
    for (claim_id, verdict) in negative {
        let claim = &reviewed.claims[claim_id];
        let attempt_id = sha256_hex(format!("{}:{claim_id}:{verdict}", bundle.id).as_bytes());
        if routes.iter().any(|route| route["id"] == attempt_id) {
            continue;
        }
        if routes.len() >= 128 {
            return Err(error(
                "failed_route_budget",
                Some(&bundle.door_id),
                "failed-route history is full",
            ));
        }
        let route = claim["route"]
            .as_str()
            .or_else(|| claim["statement"].as_str())
            .unwrap_or_default();
        if route.is_empty() || route.len() > 16000 {
            return Err(error(
                "failed_route_provenance",
                Some(claim_id),
                "failed route needs its exact bounded attempted statement",
            ));
        }
        let mut attempt = json!({"id":attempt_id,"claim_id":claim_id,"verdict":verdict,"route":route,
            "lesson":claim["lesson"].as_str().filter(|s|!s.trim().is_empty()).unwrap_or(&reviewed.reasons[claim_id]),
            "reason":reviewed.reasons[claim_id],"campaign":bundle.campaign_id,"integration":bundle.id,
            "author_report":{"path":bundle.author.report_path,"sha256":bundle.author.report_sha256},
            "referee_report":{"path":bundle.referee.report_path,"sha256":bundle.referee.report_sha256},
            "check":{"path":receipt.output_path,"sha256":receipt.output_sha256},
            "referee_check":{"id":receipt.id,"code_path":receipt.code_path,"code_sha256":receipt.code_sha256,
                "output_path":receipt.output_path,"output_sha256":receipt.output_sha256,
                "source_sha256":receipt.source_sha256,"command_sha256":receipt.command_sha256}});
        if !claim["counterexample"].is_null() {
            attempt["counterexample"] = claim["counterexample"].clone();
        }
        if let Some(test) = claim["test"].as_str() {
            attempt["next_test"] = json!(test);
        }
        routes.push(attempt);
    }
    Ok(())
}

/// Stage the coordinator's exact JSON images as data for the isolated writer
/// and spot checks. This cannot write canonical files or bypass final checks.
pub(crate) fn preview_canonical(
    workspace: &Path,
    bundle: &IntegrationBundle,
) -> Result<Value, String> {
    let _lease = read_lease(workspace)?;
    ensure_no_pending(workspace)?;
    let reviewed = validate_bundle(workspace, bundle, false)?;
    validate_proposals(
        &json_file(workspace, &bundle.writer_before.report_path)?,
        bundle,
    )?;
    let sources = source_bindings(workspace, bundle)?;
    let (changes, numerical) = canonical_changes(workspace, bundle, &reviewed, &sources)?;
    let mut files = Vec::new();
    for (path, _, after) in changes {
        let after = after.unwrap();
        let content = std::str::from_utf8(&after).map_err(|_| {
            error(
                "canonical_document",
                path.to_str(),
                "canonical preview must be UTF-8",
            )
        })?;
        files.push(json!({"path":path,"content":content,"sha256":sha256_hex(&after)}));
    }
    Ok(json!({"files":files,"frontier":numerical,"canonical_writes":0}))
}

fn checked_integration_changes(
    workspace: &Path,
    bundle: &IntegrationBundle,
    reviewed: &Reviewed,
    sources: &BTreeMap<String, Option<Vec<u8>>>,
) -> Result<(Vec<Change>, Option<Value>), String> {
    let mut changes = document_changes(workspace, bundle, reviewed)?;
    let (canonical, numerical) = canonical_changes(workspace, bundle, reviewed, sources)?;
    for (path, _, after) in &canonical {
        let name = path.to_str().unwrap();
        let hash = sha256_hex(after.as_ref().unwrap());
        if [&reviewed.writer_sources, &reviewed.coordinator_sources]
            .iter()
            .any(|sources| {
                sources
                    .files
                    .get(name)
                    .is_none_or(|(_, actual)| actual != &hash)
            })
        {
            return Err(error(
                "stale_canonical_check",
                Some(name),
                "writer and coordinator checks did not consume these exact proposed research JSON bytes",
            ));
        }
    }
    changes.extend(canonical);
    Ok((changes, numerical))
}

/// Inspect a complete frozen bundle with the same source and check validators
/// used by integration. This never publishes a transaction or calls a model.
pub(crate) fn check_integration(workspace: &Path, path: &str) -> Result<Value, String> {
    let _lease = read_lease(workspace)?;
    ensure_no_pending(workspace)?;
    let bundle: IntegrationBundle = serde_json::from_value(json_file(workspace, path)?)
        .map_err(|e| error("campaign_schema", Some(path), e.to_string()))?;
    let reviewed = validate_bundle(workspace, &bundle, true)?;
    let sources = source_bindings(workspace, &bundle)?;
    let (changes, numerical) =
        checked_integration_changes(workspace, &bundle, &reviewed, &sources)?;
    let files: Vec<_> = changes
        .iter()
        .map(|(path, _, after)| json!({"path":path,"sha256":sha256_hex(after.as_ref().unwrap())}))
        .collect();
    Ok(
        json!({"valid":true,"bundle":bundle.id,"files":files,"frontier":numerical,
        "canonical_writes":0,"model_calls":0,"integrated":false}),
    )
}

/// The coordinator is the single canonical writer. Reports and AI review never
/// gain reward, tier or publication authority by being archived or agreeing.
pub(crate) fn integrate(
    workspace: &Path,
    bundle: &IntegrationBundle,
    cancel: &AtomicBool,
) -> Result<Value, String> {
    let _lease = Lease::acquire(workspace)?;
    ensure_no_pending(workspace)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(error(
            "cancelled",
            None,
            "integration cancelled before validation",
        ));
    }
    let bundle_value =
        serde_json::to_value(bundle).map_err(|e| error("serialization", None, e.to_string()))?;
    let bundle_sha256 = sha256_hex(
        &serde_json::to_vec(&bundle_value)
            .map_err(|e| error("serialization", None, e.to_string()))?,
    );
    if let Some(outcome) = completed_outcome(workspace, bundle, &bundle_sha256)? {
        return Ok(outcome);
    }
    let reviewed = validate_bundle(workspace, bundle, true)?;
    let sources = source_bindings(workspace, bundle)?;
    let (mut changes, numerical) =
        checked_integration_changes(workspace, bundle, &reviewed, &sources)?;
    let (_, reviewed_event) = event_record(&json!({"type":"reviewed",
        "summary":format!("Campaign {}: {} claims independently refereed; exact corrections, writer checks and coordinator replay integrated", bundle.id, bundle.claims.len()),
        "nodes":bundle.nodes.iter().map(|n| n["id"].clone()).collect::<Vec<_>>(),
        "evidence":[bundle.author.report_path,bundle.literature.report_path,bundle.referee.report_path,bundle.writer.report_path,bundle.coordinator.report_path],
        "campaign_id":bundle.id,"coordinator":bundle.coordinator_id}))?;
    let events_before = optional(workspace, EVENTS, MAX_EVENTS)?;
    let mut events_after = events_before.clone().unwrap_or_default();
    if !events_after.is_empty() && events_after.last() != Some(&b'\n') {
        return Err(error(
            "event_log",
            Some(EVENTS),
            "cannot append to a partial event record",
        ));
    }
    if events_after.len() + reviewed_event.len() > MAX_EVENTS {
        return Err(error("byte_budget", Some(EVENTS), "event journal is full"));
    }
    events_after.extend_from_slice(&reviewed_event);
    changes.push((PathBuf::from(EVENTS), events_before, Some(events_after)));
    if changes.len() > 64 {
        return Err(error(
            "byte_budget",
            None,
            "canonical integration exceeds 64 files",
        ));
    }
    let total: usize = changes
        .iter()
        .map(|(_, before, after)| {
            before.as_ref().map_or(0, Vec::len) + after.as_ref().map_or(0, Vec::len)
        })
        .sum();
    if total > MAX_TOTAL {
        return Err(error(
            "byte_budget",
            None,
            "canonical integration exceeds its 16 MiB transaction budget",
        ));
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let token = sha256_hex(
        format!(
            "{}:{bundle_sha256}:{}:{nonce}",
            bundle.id,
            std::process::id()
        )
        .as_bytes(),
    );
    let outcome = json!({"integrated":true,"campaign":bundle.id,"transaction":token,"bundle_sha256":bundle_sha256,
        "nodes_updated":bundle.nodes.len(),"sota_updated":bundle.sota.len(),"documents_updated":bundle.edits.iter().map(|e| &e.path).collect::<BTreeSet<_>>(),
        "frontier":numerical,"review":"independent AI referee; human check pending","publication":"local/private"});
    let journal = json!({"schema":1,"id":token,"campaign":bundle.id,"prepared_at":now(),
        "bundle_sha256":bundle_sha256,"bundle":bundle_value,"outcome":outcome,"changes":[]});
    let mut journal = journal;
    let mut journal_files = Vec::new();
    for (path, before, after) in &changes {
        let text = |bytes: &Option<Vec<u8>>| -> Result<Value, String> {
            bytes
                .as_ref()
                .map(|b| {
                    std::str::from_utf8(b).map(|s| json!(s)).map_err(|_| {
                        error(
                            "transaction_source",
                            path.to_str(),
                            "canonical source is not UTF-8",
                        )
                    })
                })
                .transpose()
                .map(|v| v.unwrap_or(Value::Null))
        };
        journal_files.push(json!({"path":path,"before":text(before)?,"after":text(after)?}));
    }
    journal["changes"] = json!(journal_files);
    let mut prepared = serde_json::to_vec_pretty(&journal)
        .map_err(|e| error("serialization", None, e.to_string()))?;
    prepared.push(b'\n');
    if prepared.len() > MAX_JOURNAL {
        return Err(error(
            "byte_budget",
            None,
            "durable transaction journal exceeds its budget",
        ));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(error(
            "cancelled",
            None,
            "integration cancelled before publication",
        ));
    }
    let transactions = transaction_names(workspace)?;
    if transactions.len() >= 256 {
        return Err(error(
            "byte_budget",
            None,
            "transaction count budget exceeded",
        ));
    }
    let mut journal_total = prepared.len();
    for previous in transactions {
        journal_total += read(
            workspace,
            &format!("{TRANSACTIONS}/{previous}.prepared.json"),
            MAX_JOURNAL,
        )?
        .len();
        if journal_total > 4 * MAX_TOTAL {
            return Err(error(
                "byte_budget",
                Some(TRANSACTIONS),
                "this integration would exceed the aggregate 64 MiB journal budget",
            ));
        }
    }
    // Canonical provenance uses this immutable external receipt to avoid a
    // self-hash cycle with the checked JSON source snapshots. Standalone
    // callers get the same retained receipt as a running campaign.
    let receipt_path = format!(
        "{PRIVATE}/campaigns/{}/integrations/{}.json",
        bundle.campaign_id, bundle.id
    );
    match optional(workspace, &receipt_path, MAX_FILE)? {
        Some(bytes) => {
            let existing: Value = serde_json::from_slice(&bytes)
                .map_err(|e| error("integration_receipt", Some(&receipt_path), e.to_string()))?;
            if existing != bundle_value {
                return Err(error(
                    "integration_receipt",
                    Some(&receipt_path),
                    "integration identity already has a different immutable bundle",
                ));
            }
        }
        None => publish(workspace, &receipt_path, &serialized(&bundle_value)?)?,
    }
    confined_publish_new_no_symlinks(
        workspace,
        Path::new(&format!("{TRANSACTIONS}/{token}.prepared.json")),
        &prepared,
    )
    .map_err(|e| error("transaction_prepare", None, e))?;
    if let Err(e) = confined_compare_replace_batch_no_symlinks(workspace, &changes) {
        let restored = changes.iter().all(|(path, before, _)| {
            optional(workspace, path.to_str().unwrap(), MAX_EVENTS)
                .is_ok_and(|current| current == *before)
        });
        if restored {
            marker(workspace, &token, "aborted", &prepared)?;
        }
        return Err(error(
            if restored {
                "integration_conflict"
            } else {
                "integration_pending"
            },
            None,
            e,
        ));
    }
    marker(workspace, &token, "complete", &prepared)?;
    Ok(outcome)
}
