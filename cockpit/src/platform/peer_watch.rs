//! Peer angelX watch: instances publish heartbeats to a shared directory
//! and read each other's, so one cockpit can see the fleet — which machines
//! are up, what each instance is doing, and how loaded they are.
//!
//! The shared directory is a **generic slot**, like backup's target:
//! `~/.angelX/peers.json` (`angelX.peers/v1`, `{"dir": "..."}`) or
//! `ANGEL_PEER_DIR` names it; unset, it defaults to `~/.angelX/peers/`
//! (same-machine instances see each other with zero setup). Point the slot
//! at any directory every machine can reach — a tailnet share, NFS, a
//! synced folder on a fleet services host — and the same code watches
//! across machines. No host is named in the public build.
//!
//! Heartbeats are small atomically-published JSON files, one per instance,
//! rewritten on a throttle. Readers tolerate torn or foreign files, and a
//! peer whose heartbeat ages past [`STALE_AFTER`] counts as gone: a dead
//! laptop must never look alive, and a crashed instance must never need
//! cleanup from its peers.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub(crate) const PEERS_CONFIG_SCHEMA: &str = "angelX.peers/v1";
const HEARTBEAT_SCHEMA: &str = "angelX.heartbeat/v1";
// The heartbeat record and its staleness rule live in the Kernel's host
// services, shared with the realm host's world view.
pub(crate) use angel_live::host::peer_watch::{Heartbeat, STALE_AFTER_MS};
/// Minimum spacing between heartbeat writes from one instance.
const BEAT_INTERVAL: Duration = Duration::from_secs(5);
/// Cap on peer files read per scan, so a hostile or bloated shared
/// directory cannot make a scan unbounded.
const MAX_PEERS: usize = 256;
const MAX_HEARTBEAT_BYTES: u64 = 16 * 1024;

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn host_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "unknown".to_string())
}

#[derive(Clone, Debug, Deserialize)]
struct PeersConfig {
    schema: String,
    dir: String,
}

/// Resolve the shared peer directory: `ANGEL_PEER_DIR`, then
/// `~/.angelX/peers.json`, then the default `~/.angelX/peers/`.
pub(crate) fn peer_dir(state_root: &Path) -> PathBuf {
    if let Ok(env) = std::env::var("ANGEL_PEER_DIR")
        && !env.trim().is_empty()
    {
        return PathBuf::from(env.trim());
    }
    if let Ok(raw) = std::fs::read(state_root.join("peers.json"))
        && let Ok(config) = serde_json::from_slice::<PeersConfig>(&raw)
        && config.schema == PEERS_CONFIG_SCHEMA
        && !config.dir.trim().is_empty()
    {
        let dir = config.dir.trim();
        if let Some(rest) = dir.strip_prefix("~/") {
            return std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(rest);
        }
        return PathBuf::from(dir);
    }
    state_root.join("peers")
}

/// This process's stable instance id: host + pid + start time. Unique per
/// launch, stable for the process lifetime, safe as a file name.
pub(crate) fn instance_id(started_ms: u128) -> String {
    let host: String = host_name()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("{host}-{}-{started_ms:013}", std::process::id())
}

/// Throttled heartbeat writer owned by the app's sampler. Beats are
/// best-effort bulk-tier writes: losing one only ages this peer's record.
#[derive(Debug)]
pub(crate) struct PeerBeat {
    dir: PathBuf,
    pub heartbeat: Heartbeat,
    last_written: Option<Instant>,
}

impl PeerBeat {
    pub(crate) fn new(state_root: &Path, workspace: &Path, session_id: Option<String>) -> Self {
        let started_ms = now_ms();
        Self {
            dir: peer_dir(state_root),
            heartbeat: Heartbeat {
                schema: HEARTBEAT_SCHEMA.to_string(),
                instance_id: instance_id(started_ms),
                host: host_name(),
                pid: std::process::id(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                started_ms,
                updated_ms: started_ms,
                workspace: workspace.to_string_lossy().into_owned(),
                session_id,
                activity: "idle".to_string(),
                detail: None,
                cpu_pct: 0.0,
                mem_pct: 0.0,
                gpu_pct: 0.0,
            },
            last_written: None,
        }
    }

    pub(crate) fn set_activity(&mut self, activity: &str, detail: Option<String>) {
        self.heartbeat.activity = activity.to_string();
        self.heartbeat.detail = detail;
        // An activity change is exactly what peers watch for: beat promptly.
        self.last_written = None;
    }

    /// Write the heartbeat if the throttle allows. Load fields update from
    /// the sampler's latest snapshot.
    pub(crate) fn beat(&mut self, cpu_pct: f32, mem_pct: f32, gpu_pct: f32) {
        self.heartbeat.cpu_pct = cpu_pct;
        self.heartbeat.mem_pct = mem_pct;
        self.heartbeat.gpu_pct = gpu_pct;
        if self
            .last_written
            .is_some_and(|written| written.elapsed() < BEAT_INTERVAL)
        {
            return;
        }
        self.heartbeat.updated_ms = now_ms();
        if let Ok(body) = serde_json::to_vec(&self.heartbeat) {
            let path = self
                .dir
                .join(format!("{}.json", self.heartbeat.instance_id));
            if super::workspace_store::write_atomic(&path, &body).is_ok() {
                self.last_written = Some(Instant::now());
            }
        }
    }

    /// Best-effort goodbye: remove our heartbeat so peers do not wait out
    /// the stale horizon after a clean exit.
    pub(crate) fn retire(&self) {
        let path = self
            .dir
            .join(format!("{}.json", self.heartbeat.instance_id));
        let _ = super::workspace_store::remove_durable(&path);
    }
}

/// Read every peer heartbeat in `dir` (excluding `own_id`), newest first.
/// Unreadable, oversized, or foreign-schema files are skipped silently: the
/// shared directory is an untrusted input boundary.
pub(crate) fn read_peers(dir: &Path, own_id: Option<&str>) -> Vec<Heartbeat> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut peers = Vec::new();
    for entry in entries.flatten() {
        if peers.len() >= MAX_PEERS {
            break;
        }
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > MAX_HEARTBEAT_BYTES {
            continue;
        }
        let Ok(raw) = std::fs::read(&path) else {
            continue;
        };
        let Ok(heartbeat) = serde_json::from_slice::<Heartbeat>(&raw) else {
            continue;
        };
        if heartbeat.schema != HEARTBEAT_SCHEMA || Some(heartbeat.instance_id.as_str()) == own_id {
            continue;
        }
        peers.push(heartbeat);
    }
    peers.sort_by_key(|peer| std::cmp::Reverse(peer.updated_ms));
    peers
}

/// Mean load across live peers — the fleet aggregate the status bar shows
/// when no external fleet probe answers.
pub(crate) fn fleet_means(peers: &[Heartbeat], at_ms: u128) -> Option<(f32, f32)> {
    let live: Vec<&Heartbeat> = peers.iter().filter(|peer| peer.is_live(at_ms)).collect();
    if live.is_empty() {
        return None;
    }
    let count = live.len() as f32;
    let cpu = live.iter().map(|peer| peer.cpu_pct).sum::<f32>() / count;
    let gpu = live.iter().map(|peer| peer.gpu_pct).sum::<f32>() / count;
    Some((cpu, gpu))
}

/// What changed between two peer scans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PeerEvent {
    Appeared {
        instance_id: String,
        host: String,
        activity: String,
    },
    ActivityChanged {
        instance_id: String,
        host: String,
        from: String,
        to: String,
    },
    Gone {
        instance_id: String,
        host: String,
    },
}

impl PeerEvent {
    pub(crate) fn notice_text(&self) -> String {
        match self {
            Self::Appeared { host, activity, .. } => {
                format!("[peers] angelX on {host} appeared ({activity})")
            }
            Self::ActivityChanged { host, from, to, .. } => {
                format!("[peers] angelX on {host}: {from} → {to}")
            }
            Self::Gone { host, .. } => format!("[peers] angelX on {host} went silent"),
        }
    }
}

/// Stateful differ: feed it successive scans, get back what changed. Only
/// live peers are tracked, so a peer crossing the stale horizon reads as
/// `Gone` exactly once.
#[derive(Debug, Default)]
pub(crate) struct PeerWatcher {
    known: BTreeMap<String, (String, String)>, // instance_id -> (host, activity)
}

impl PeerWatcher {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn observe(&mut self, peers: &[Heartbeat], at_ms: u128) -> Vec<PeerEvent> {
        let mut events = Vec::new();
        let mut current: BTreeMap<String, (String, String)> = BTreeMap::new();
        for peer in peers.iter().filter(|peer| peer.is_live(at_ms)) {
            current.insert(
                peer.instance_id.clone(),
                (peer.host.clone(), peer.activity.clone()),
            );
            match self.known.get(&peer.instance_id) {
                None => events.push(PeerEvent::Appeared {
                    instance_id: peer.instance_id.clone(),
                    host: peer.host.clone(),
                    activity: peer.activity.clone(),
                }),
                Some((_, activity)) if activity != &peer.activity => {
                    events.push(PeerEvent::ActivityChanged {
                        instance_id: peer.instance_id.clone(),
                        host: peer.host.clone(),
                        from: activity.clone(),
                        to: peer.activity.clone(),
                    });
                }
                Some(_) => {}
            }
        }
        for (instance_id, (host, _)) in &self.known {
            if !current.contains_key(instance_id) {
                events.push(PeerEvent::Gone {
                    instance_id: instance_id.clone(),
                    host: host.clone(),
                });
            }
        }
        self.known = current;
        events
    }
}

/// Operator-facing peer table for `/peers`.
pub(crate) fn format_peers(peers: &[Heartbeat], at_ms: u128) -> String {
    if peers.is_empty() {
        return "no peer angelX instances are publishing heartbeats here".to_string();
    }
    let mut lines = vec![format!("{} peer instance(s):", peers.len())];
    for peer in peers {
        let age_s = peer.age_ms(at_ms) / 1000;
        let state = if peer.is_live(at_ms) { "live" } else { "stale" };
        let detail = peer
            .detail
            .as_deref()
            .map(|detail| format!(" — {detail}"))
            .unwrap_or_default();
        lines.push(format!(
            "  {} pid {} · {} ({state}, {age_s}s ago) · cpu {:.0}% mem {:.0}% gpu {:.0}% · {}{detail}",
            peer.host,
            peer.pid,
            peer.activity,
            peer.cpu_pct,
            peer.mem_pct,
            peer.gpu_pct,
            peer.workspace,
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/peer_watch__tests.rs"]
mod tests;
