//! The sleeping giant's bookkeeping: which dead peers may be resurrected,
//! who has claimed them, and what happened. Pure selection plus small
//! atomic files in the shared peer directory — execution (spawning a
//! model) lives in `drive::guardian`.
//!
//! Fencing, because resurrection is automatic: a guardian may act only on
//! a peer whose heartbeat is stale, whose session no live instance also
//! holds, and only after winning an atomic `create_new` claim in the peer
//! directory. A finished resurrection writes a durable record beside the
//! claim, so the same death is never resurrected twice — and if the
//! "dead" instance beats again first, nobody acts at all.

use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use super::peer_watch::{Heartbeat, STALE_AFTER_MS};

/// Default silence before a peer counts as down for resurrection. Longer
/// than the display staleness horizon: acting is weightier than noticing.
pub(crate) const DEFAULT_STALE_MS: u128 = 180_000;
/// A claim older than this with no outcome record may be stolen: the
/// claiming guardian is presumed dead too.
pub(crate) const CLAIM_TTL_MS: u128 = 30 * 60 * 1000;

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Age evidence for a claim file whose body cannot be parsed, measured
/// against the caller's clock.
fn claim_file_age_ms(path: &Path, now_ms: u128) -> Option<u128> {
    let modified_ms = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some(now_ms.saturating_sub(modified_ms))
}

/// A peer a guardian on this rig could resurrect, fenced and ready.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub instance_id: String,
    pub host: String,
    pub session_id: String,
    pub workspace: PathBuf,
    pub last_seen_ms: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Claim {
    guardian_instance: String,
    guardian_host: String,
    guardian_pid: u32,
    session_id: String,
    claimed_ms: u128,
}

/// Durable outcome of one resurrection attempt, read by guardians (skip
/// logic), cockpits (`/peers`), and the world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ResurrectionRecord {
    pub dead_instance: String,
    pub dead_host: String,
    pub old_session_id: String,
    pub new_session_id: Option<String>,
    pub guardian_host: String,
    pub started_ms: u128,
    pub finished_ms: u128,
    pub success: bool,
    pub summary: String,
}

fn claim_path(peer_dir: &Path, instance_id: &str) -> PathBuf {
    peer_dir.join(format!("{instance_id}.claim.json"))
}

fn record_path(peer_dir: &Path, instance_id: &str) -> PathBuf {
    peer_dir.join(format!("{instance_id}.resurrection.json"))
}

pub(crate) fn read_record(peer_dir: &Path, instance_id: &str) -> Option<ResurrectionRecord> {
    let raw = std::fs::read(record_path(peer_dir, instance_id)).ok()?;
    serde_json::from_slice(&raw).ok()
}

/// Every resurrection record in the peer directory, newest first. This is
/// the `/peers` board's memory: heartbeats say who is silent now, records
/// say who was raised (or failed to rise) and into which session.
pub(crate) fn read_records(peer_dir: &Path) -> Vec<ResurrectionRecord> {
    let Ok(entries) = std::fs::read_dir(peer_dir) else {
        return Vec::new();
    };
    let mut records: Vec<ResurrectionRecord> = entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .ends_with(".resurrection.json")
        })
        .filter_map(|entry| std::fs::read(entry.path()).ok())
        .filter_map(|raw| serde_json::from_slice(&raw).ok())
        .collect();
    records.sort_by_key(|record| std::cmp::Reverse(record.finished_ms));
    records
}

pub(crate) fn write_record(peer_dir: &Path, record: &ResurrectionRecord) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(record).map_err(|e| format!("encode record: {e}"))?;
    super::workspace_store::write_private_atomic_exact(
        &record_path(peer_dir, &record.dead_instance),
        &body,
    )
}

/// Select resurrection candidates from one peer scan. Pure: filesystem
/// facts it needs (workspace exists, prior record) are passed in as
/// predicates so the fencing rules are unit-testable without a disk.
pub(crate) fn select_candidates(
    peers: &[Heartbeat],
    at_ms: u128,
    stale_ms: u128,
    workspace_present: impl Fn(&Path) -> bool,
    already_resurrected: impl Fn(&str, &str) -> bool,
) -> Vec<Candidate> {
    let live_sessions: std::collections::BTreeSet<&str> = peers
        .iter()
        .filter(|peer| peer.age_ms(at_ms) <= STALE_AFTER_MS)
        .filter_map(|peer| peer.session_id.as_deref())
        .collect();
    let mut candidates = Vec::new();
    for peer in peers {
        if peer.activity.starts_with("guardian") || peer.activity == "resurrecting" {
            continue; // giants do not resurrect giants
        }
        if peer.age_ms(at_ms) <= stale_ms {
            continue; // still breathing
        }
        let Some(session_id) = peer.session_id.as_deref() else {
            continue; // nothing to resurrect
        };
        if live_sessions.contains(session_id) {
            continue; // someone live holds this session now
        }
        if already_resurrected(&peer.instance_id, session_id) {
            continue; // this death was already answered
        }
        let workspace = PathBuf::from(&peer.workspace);
        if !workspace_present(&workspace) {
            continue; // another rig's guardian may have this checkout
        }
        candidates.push(Candidate {
            instance_id: peer.instance_id.clone(),
            host: peer.host.clone(),
            session_id: session_id.to_string(),
            workspace,
            last_seen_ms: peer.updated_ms,
        });
    }
    candidates.sort_by_key(|candidate| candidate.last_seen_ms);
    candidates
}

/// Atomically claim a dead instance. Exactly one guardian wins: the claim
/// file is created with `create_new`, so a racing guardian gets `false`,
/// never a shared claim. A stale claim (older than [`CLAIM_TTL_MS`], no
/// outcome record) may be stolen — its guardian is presumed dead too.
pub(crate) fn try_claim(
    peer_dir: &Path,
    candidate: &Candidate,
    guardian_instance: &str,
    guardian_host: &str,
) -> Result<bool, String> {
    try_claim_at(
        peer_dir,
        candidate,
        guardian_instance,
        guardian_host,
        now_ms(),
    )
}

/// Whether a durable outcome already answers this exact death. The same
/// instance dying later in a different session is a new death.
fn already_answered(peer_dir: &Path, candidate: &Candidate) -> bool {
    read_record(peer_dir, &candidate.instance_id)
        .is_some_and(|record| record.old_session_id == candidate.session_id)
}

/// [`try_claim`] against an explicit clock. Expiry is wall-clock business;
/// taking `now_ms` here lets tests pin the 30-minute contract exactly.
pub(crate) fn try_claim_at(
    peer_dir: &Path,
    candidate: &Candidate,
    guardian_instance: &str,
    guardian_host: &str,
    now_ms: u128,
) -> Result<bool, String> {
    std::fs::create_dir_all(peer_dir).map_err(|e| format!("create {}: {e}", peer_dir.display()))?;
    if already_answered(peer_dir, candidate) {
        return Ok(false);
    }
    let path = claim_path(peer_dir, &candidate.instance_id);
    let claim = Claim {
        guardian_instance: guardian_instance.to_string(),
        guardian_host: guardian_host.to_string(),
        guardian_pid: std::process::id(),
        session_id: candidate.session_id.clone(),
        claimed_ms: now_ms,
    };
    let body = serde_json::to_vec(&claim).map_err(|e| format!("encode claim: {e}"))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            let written = file.write_all(&body).and_then(|()| file.sync_all());
            if let Err(error) = written {
                drop(file);
                // The create_new above made this file ours; a partial body
                // must not fence the instance off for the whole claim TTL.
                let _ = std::fs::remove_file(&path);
                return Err(format!("write claim {}: {error}", path.display()));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            // Steal only an expired, unanswered claim. A claim whose body
            // never landed (the guardian died between create_new and the
            // write) has no parseable age; its file mtime is then the only
            // evidence, and past the TTL it is as stale as any claim.
            let expired = match std::fs::read(&path)
                .ok()
                .and_then(|raw| serde_json::from_slice::<Claim>(&raw).ok())
            {
                Some(old) => now_ms.saturating_sub(old.claimed_ms) > CLAIM_TTL_MS,
                None => claim_file_age_ms(&path, now_ms).is_some_and(|age| age > CLAIM_TTL_MS),
            };
            // Recheck under the steal: a record may have landed since the
            // initial fence, and only this death's record blocks it.
            if !expired || already_answered(peer_dir, candidate) {
                return Ok(false);
            }
            let stolen = peer_dir.join(format!(
                ".{}.claim-stolen-{}",
                candidate.instance_id,
                std::process::id()
            ));
            // Rename-aside is the atomic steal: only one stealer succeeds.
            if std::fs::rename(&path, &stolen).is_err() {
                return Ok(false);
            }
            let _ = std::fs::remove_file(&stolen);
            try_claim_at(
                peer_dir,
                candidate,
                guardian_instance,
                guardian_host,
                now_ms,
            )
        }
        Err(error) => Err(format!("claim {}: {error}", path.display())),
    }
}

/// Append one line to the guardian's own log (operator forensics; the
/// durable facts live in claim/record files, so a plain append is fine).
pub(crate) fn log_line(state_root: &Path, text: &str) {
    let dir = state_root.join("guardian");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("guardian.log"))
    {
        let _ = writeln!(file, "[{}] {text}", now_ms());
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/guardian__tests.rs"]
mod tests;
