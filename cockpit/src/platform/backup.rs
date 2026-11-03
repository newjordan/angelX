//! Tiered, hardlinked snapshots of the angelX state tree (`~/.angelX`).
//!
//! The state tree mixes the irreplaceable (sessions, memories, atlas,
//! knowledge, goals — megabytes) with the bulky and regenerable
//! (loop-experiments, trajectories — gigabytes). One flat mirror would be
//! slow and mostly waste; tiers let a snapshot cost only what matters:
//!
//! * `core` — state a user would mourn. The default.
//! * `bulk` — core plus large evidence stores (trajectories, experience…).
//! * `all`  — everything except runtime/ephemeral entries (`proc`, caches,
//!   `*.err`/`*.log` droppings).
//!
//! Each snapshot is a directory whose unchanged files are hardlinks into
//! the previous snapshot, so keeping history costs only changed bytes.
//! A `manifest.json` (sha256 per file) is written last, into a staging
//! directory that is renamed into place only when complete; `latest` is a
//! symlink repointed atomically. A reader therefore only ever sees whole
//! snapshots.
//!
//! The target is a **generic slot**: `~/.angelX/backup.json`
//! (`angelX.backup/v1`) or `ANGEL_BACKUP_TARGET` names either a local path
//! (including a mounted share) or an `[user@]host:/abs/path` rsync target.
//! Unconfigured, backup is dormant and says so. Nothing in the public build
//! names a specific host; operators fill the slot with their own machine.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

pub(crate) const BACKUP_CONFIG_SCHEMA: &str = "angelX.backup/v1";
const MANIFEST_SCHEMA: &str = "angelX.backup-manifest/v1";

/// Top-level state entries per tier. Names are directories (or absent) under
/// the angelX state root; small top-level config files ride along in core.
const CORE_DIRS: &[&str] = &[
    "sessions",
    "memories",
    "atlas",
    "knowledge",
    "caddy",
    "loops",
    "goals",
    "continual-harness",
    // Saved conversations reference these exact catalog generations by digest.
    "book-generations",
];
const BULK_EXTRA_DIRS: &[&str] = &[
    "trajectories",
    "experience",
    "cut",
    "rl",
    "world-rewards",
    "yukon-strategy",
    "wish-paint",
];
/// Never backed up at any tier: live runtime state and caches.
const ALWAYS_EXCLUDED: &[&str] = &["proc", "cache", "build-cache", ".backup-staging"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tier {
    Core,
    Bulk,
    All,
}

impl Tier {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "core" => Ok(Self::Core),
            "bulk" => Ok(Self::Bulk),
            "all" => Ok(Self::All),
            other => Err(format!("unknown backup tier {other:?} (core, bulk, all)")),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Bulk => "bulk",
            Self::All => "all",
        }
    }

    /// Directory names this tier includes. `All` is resolved by listing the
    /// source root (see [`collect_sources`]), so it returns the union too.
    fn dirs(self) -> Vec<&'static str> {
        let mut dirs = CORE_DIRS.to_vec();
        if self != Self::Core {
            dirs.extend_from_slice(BULK_EXTRA_DIRS);
        }
        dirs
    }
}

#[derive(Clone, Debug, Deserialize)]
struct BackupConfig {
    schema: String,
    target: String,
    #[serde(default)]
    tier: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ManifestFile {
    path: String,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symlink: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Manifest {
    schema: String,
    created_ms: u128,
    tier: String,
    host: String,
    files: Vec<ManifestFile>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SnapshotStats {
    pub files: usize,
    pub linked: usize,
    pub copied: usize,
    pub symlinks: usize,
    pub skipped: usize,
    pub bytes: u64,
}

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

fn snapshot_name(created_ms: u128) -> String {
    format!("{created_ms:013}-{}", std::process::id())
}

/// Hash a file by streaming it (bulk files can be far too large to buffer).
fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        context.update(&buffer[..read]);
    }
    let digest = context.finish();
    let mut hex = String::with_capacity(64);
    for byte in digest.as_ref() {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(hex)
}

/// Copy `source` to `temp` while hashing; returns the hex digest. The caller
/// renames or discards the temp once it knows whether a hardlink will do.
fn copy_with_hash(source: &Path, temp: &Path) -> std::io::Result<String> {
    use std::io::Write as _;
    let mut input = std::fs::File::open(source)?;
    let mut output = std::fs::File::create(temp)?;
    output.set_permissions(source.metadata()?.permissions())?;
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        context.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
    }
    output.sync_all()?;
    let digest = context.finish();
    let mut hex = String::with_capacity(64);
    for byte in digest.as_ref() {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(hex)
}

fn relative_posix(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

/// Files (and symlinks) a tier covers under `source_root`, sorted by relative
/// path. Directories are recreated on demand during copy; special files
/// (fifos, sockets, devices) are skipped and counted by the caller via the
/// returned skip count.
fn collect_sources(source_root: &Path, tier: Tier) -> Result<(Vec<PathBuf>, usize), String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if tier == Tier::All {
        let entries = std::fs::read_dir(source_root)
            .map_err(|e| format!("list {}: {e}", source_root.display()))?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if ALWAYS_EXCLUDED.contains(&name.as_str()) || name.starts_with(".incomplete-") {
                continue;
            }
            roots.push(entry.path());
        }
    } else {
        for dir in tier.dirs() {
            let path = source_root.join(dir);
            if path.exists() {
                roots.push(path);
            }
        }
        // Small top-level config files ride along in core and bulk.
        if let Ok(entries) = std::fs::read_dir(source_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if path.is_file()
                    && (name.ends_with(".json") || name.ends_with(".toml"))
                    && name != "backup.json"
                {
                    roots.push(path);
                }
            }
        }
    }
    // Log/error droppings never ride along, at any tier.
    let droppings = |path: &Path| {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        matches!(name, Some(n) if n.ends_with(".err") || n.ends_with(".log"))
    };
    let mut files = Vec::new();
    let mut skipped = 0_usize;
    let mut stack = roots;
    while let Some(path) = stack.pop() {
        if droppings(&path) {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let kind = metadata.file_type();
        if kind.is_symlink() || kind.is_file() {
            files.push(path);
        } else if kind.is_dir() {
            match std::fs::read_dir(&path) {
                Ok(entries) => stack.extend(entries.flatten().map(|entry| entry.path())),
                Err(_) => skipped += 1,
            }
        } else {
            skipped += 1;
        }
    }
    files.sort();
    Ok((files, skipped))
}

fn read_manifest(snapshot_dir: &Path) -> Option<Manifest> {
    let raw = std::fs::read(snapshot_dir.join("manifest.json")).ok()?;
    serde_json::from_slice(&raw).ok()
}

fn latest_snapshot_dir(target_root: &Path) -> Option<PathBuf> {
    let link = std::fs::read_link(target_root.join("latest")).ok()?;
    let resolved = if link.is_absolute() {
        link
    } else {
        target_root.join(link)
    };
    resolved.is_dir().then_some(resolved)
}

fn repoint_latest(target_root: &Path, name: &str) -> Result<(), String> {
    let temp_link = target_root.join(format!(".latest.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&temp_link);
    #[cfg(unix)]
    std::os::unix::fs::symlink(Path::new("snapshots").join(name), &temp_link)
        .map_err(|e| format!("link latest: {e}"))?;
    std::fs::rename(&temp_link, target_root.join("latest"))
        .map_err(|e| format!("publish latest: {e}"))?;
    Ok(())
}

// Session snapshots can change after the initial source inventory. Read the
// *copied* sessions, then add every generation they reference before the
// backup manifest is published. Deserialization skips message bodies and
// attachments instead of keeping another full copy of the conversation.
#[derive(Deserialize)]
struct SessionGenerationRefs {
    schema: String,
    history: Vec<SessionGenerationMessage>,
}

#[derive(Deserialize)]
struct SessionGenerationMessage {
    #[serde(default)]
    book_memory: Option<SessionGenerationMemory>,
}

#[derive(Deserialize)]
struct SessionGenerationMemory {
    #[serde(default)]
    catalog_digest: Option<String>,
}

fn include_session_generations(
    source_root: &Path,
    staging: &Path,
    manifest_files: &mut Vec<ManifestFile>,
    stats: &mut SnapshotStats,
) -> Result<(), String> {
    let mut digests = std::collections::BTreeSet::new();
    for entry in manifest_files.iter().filter(|entry| {
        entry.symlink.is_none()
            && entry.path.starts_with("sessions/")
            && entry.path.ends_with(".json")
    }) {
        let file = std::fs::File::open(staging.join(&entry.path))
            .map_err(|error| format!("read copied session {}: {error}", entry.path))?;
        let session: SessionGenerationRefs = match serde_json::from_reader(file) {
            Ok(session) => session,
            // Backups have always preserved non-session and damaged files in
            // this directory as-is. They do not have a usable session binding.
            Err(error) if !error.is_io() => continue,
            Err(error) => return Err(format!("read copied session {}: {error}", entry.path)),
        };
        if session.schema != "angel-project-session/v1" {
            continue;
        }
        for digest in session
            .history
            .into_iter()
            .filter_map(|message| message.book_memory.and_then(|memory| memory.catalog_digest))
        {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(format!(
                    "invalid book generation in copied session {}",
                    entry.path
                ));
            }
            digests.insert(digest);
        }
    }

    for digest in digests {
        let relative = format!("book-generations/{digest}.json");
        if let Some(entry) = manifest_files.iter().find(|entry| entry.path == relative) {
            if entry.symlink.is_some() || entry.sha256.as_deref() != Some(digest.as_str()) {
                return Err(format!("invalid copied book generation {digest}"));
            }
            continue;
        }
        let bytes = crate::agent::harness::book::generation_store::load_in(
            &source_root.join("book-generations"),
            &digest,
        )
        .map_err(|error| {
            format!("cannot complete backup for session generation {digest}: {error}")
        })?;
        let directory = staging.join("book-generations");
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("create {}: {error}", directory.display()))?;
        super::workspace_store::write_private_atomic_exact(&staging.join(&relative), &bytes)?;
        stats.files += 1;
        stats.copied += 1;
        stats.bytes = stats.bytes.saturating_add(bytes.len() as u64);
        manifest_files.push(ManifestFile {
            path: relative,
            size: bytes.len() as u64,
            sha256: Some(digest),
            symlink: None,
        });
    }
    Ok(())
}

/// Take one snapshot of `source_root` into `target_root`. Unchanged files
/// (same relative path and sha256 as the previous snapshot) are hardlinked
/// from it; everything else is copied. Returns the snapshot directory name.
pub(crate) fn snapshot_local(
    source_root: &Path,
    target_root: &Path,
    tier: Tier,
) -> Result<(String, SnapshotStats), String> {
    snapshot_local_after_collection(source_root, target_root, tier, || {})
}

// The callback permits a deterministic test of writes that land after the
// initial directory inventory. Production always passes a no-op.
fn snapshot_local_after_collection(
    source_root: &Path,
    target_root: &Path,
    tier: Tier,
    after_collection: impl FnOnce(),
) -> Result<(String, SnapshotStats), String> {
    let snapshots = target_root.join("snapshots");
    std::fs::create_dir_all(&snapshots)
        .map_err(|e| format!("create {}: {e}", snapshots.display()))?;
    let previous = latest_snapshot_dir(target_root);
    let previous_hashes: BTreeMap<String, String> = previous
        .as_deref()
        .and_then(read_manifest)
        .map(|manifest| {
            manifest
                .files
                .into_iter()
                .filter_map(|file| file.sha256.map(|sha| (file.path, sha)))
                .collect()
        })
        .unwrap_or_default();

    let created_ms = now_ms();
    let name = snapshot_name(created_ms);
    let staging = snapshots.join(format!(".incomplete-{name}"));
    std::fs::create_dir_all(&staging).map_err(|e| format!("create {}: {e}", staging.display()))?;
    let outcome = (|| {
        let (sources, skipped) = collect_sources(source_root, tier)?;
        after_collection();
        let mut stats = SnapshotStats {
            skipped,
            ..SnapshotStats::default()
        };
        let mut manifest_files = Vec::with_capacity(sources.len());
        for source in &sources {
            let Some(relative) = relative_posix(source_root, source) else {
                stats.skipped += 1;
                continue;
            };
            let destination = staging.join(&relative);
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("create {}: {e}", parent.display()))?;
            }
            let metadata = std::fs::symlink_metadata(source)
                .map_err(|e| format!("stat {}: {e}", source.display()))?;
            if metadata.file_type().is_symlink() {
                let target = std::fs::read_link(source)
                    .map_err(|e| format!("readlink {}: {e}", source.display()))?;
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, &destination)
                    .map_err(|e| format!("symlink {}: {e}", destination.display()))?;
                stats.symlinks += 1;
                stats.files += 1;
                manifest_files.push(ManifestFile {
                    path: relative,
                    size: 0,
                    sha256: None,
                    symlink: Some(target.to_string_lossy().into_owned()),
                });
                continue;
            }
            // Copy beside the destination while hashing; if the digest
            // matches the previous snapshot, discard the copy and hardlink.
            let temp = destination.with_extension(format!("part.{}", std::process::id()));
            let sha = copy_with_hash(source, &temp)
                .map_err(|e| format!("copy {}: {e}", source.display()))?;
            let unchanged = matches!(
                previous_hashes.get(&relative),
                Some(previous_sha) if previous_sha == &sha
            );
            let mut linked = false;
            if unchanged && let Some(previous) = &previous {
                linked = std::fs::hard_link(previous.join(&relative), &destination).is_ok();
            }
            if linked {
                let _ = std::fs::remove_file(&temp);
                stats.linked += 1;
            } else {
                std::fs::rename(&temp, &destination)
                    .map_err(|e| format!("publish {relative}: {e}"))?;
                stats.copied += 1;
            }
            stats.files += 1;
            stats.bytes = stats.bytes.saturating_add(metadata.len());
            manifest_files.push(ManifestFile {
                path: relative,
                size: metadata.len(),
                sha256: Some(sha),
                symlink: None,
            });
        }
        include_session_generations(source_root, &staging, &mut manifest_files, &mut stats)?;
        manifest_files.sort_by(|left, right| left.path.cmp(&right.path));
        let manifest = Manifest {
            schema: MANIFEST_SCHEMA.to_string(),
            created_ms,
            tier: tier.label().to_string(),
            host: host_name(),
            files: manifest_files,
        };
        let body =
            serde_json::to_vec_pretty(&manifest).map_err(|e| format!("encode manifest: {e}"))?;
        super::workspace_store::write_private_atomic_exact(&staging.join("manifest.json"), &body)?;
        Ok(stats)
    })();
    let stats = match outcome {
        Ok(stats) => stats,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    let final_dir = snapshots.join(&name);
    std::fs::rename(&staging, &final_dir).map_err(|e| format!("publish snapshot {name}: {e}"))?;
    repoint_latest(target_root, &name)?;
    Ok((name, stats))
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RestoreStats {
    pub restored: usize,
    pub identical: usize,
    pub refused: usize,
}

/// Verify `snapshot_dir` against its manifest. Every regular file must hash
/// to its recorded digest; symlinks must point where recorded.
pub(crate) fn verify_snapshot(snapshot_dir: &Path) -> Result<Manifest, String> {
    let manifest = read_manifest(snapshot_dir)
        .ok_or_else(|| format!("no readable manifest in {}", snapshot_dir.display()))?;
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(format!("unsupported manifest schema {:?}", manifest.schema));
    }
    for file in &manifest.files {
        // Manifest paths are data: never let one escape the snapshot tree.
        if file.path.starts_with('/') || file.path.split('/').any(|part| part == "..") {
            return Err(format!("manifest path escapes snapshot: {:?}", file.path));
        }
        let path = snapshot_dir.join(&file.path);
        if let Some(expected_target) = &file.symlink {
            let target = std::fs::read_link(&path)
                .map_err(|e| format!("readlink {}: {e}", path.display()))?;
            if target.to_string_lossy() != expected_target.as_str() {
                return Err(format!("symlink {} target mismatch", file.path));
            }
            continue;
        }
        let expected = file
            .sha256
            .as_deref()
            .ok_or_else(|| format!("manifest entry {} has no hash", file.path))?;
        let actual = hash_file(&path).map_err(|e| format!("hash {}: {e}", path.display()))?;
        if actual != expected {
            return Err(format!("hash mismatch for {}", file.path));
        }
    }
    Ok(manifest)
}

/// Restore a verified snapshot into `destination_root`. Files whose current
/// content already matches are left alone; differing files are only
/// overwritten with `force`. Restored files publish atomically.
pub(crate) fn restore_local(
    snapshot_dir: &Path,
    destination_root: &Path,
    force: bool,
) -> Result<RestoreStats, String> {
    let manifest = verify_snapshot(snapshot_dir)?;
    if manifest
        .files
        .iter()
        .any(|file| file.path.starts_with("book-generations/"))
    {
        // Catalog snapshots are private session dependencies. Restore their
        // directory with the same owner-only policy used by the live store.
        let dir = super::workspace_store::private_io::PrivateDirectory::open(
            &destination_root.join("book-generations"),
        )
        .map_err(|error| format!("restore book generation directory: {error}"))?;
        dir.secure_owner_only()
            .map_err(|error| format!("restore book generation directory: {error}"))?;
    }
    let mut stats = RestoreStats::default();
    for file in &manifest.files {
        let destination = destination_root.join(&file.path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        if let Some(target) = &file.symlink {
            match std::fs::read_link(&destination) {
                Ok(current) if current.to_string_lossy() == target.as_str() => {
                    stats.identical += 1;
                }
                Ok(_) if !force => stats.refused += 1,
                _ => {
                    let _ = std::fs::remove_file(&destination);
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(target, &destination)
                        .map_err(|e| format!("symlink {}: {e}", destination.display()))?;
                    stats.restored += 1;
                }
            }
            continue;
        }
        let expected = file.sha256.as_deref().unwrap_or_default();
        if destination.is_file() {
            match hash_file(&destination) {
                Ok(current) if current == expected => {
                    stats.identical += 1;
                    continue;
                }
                _ if !force => {
                    stats.refused += 1;
                    continue;
                }
                _ => {}
            }
        }
        let source = snapshot_dir.join(&file.path);
        let temp = destination.with_extension(format!("restore.{}", std::process::id()));
        let sha = copy_with_hash(&source, &temp)
            .map_err(|e| format!("copy {}: {e}", source.display()))?;
        if sha != expected {
            let _ = std::fs::remove_file(&temp);
            return Err(format!(
                "snapshot file changed during restore: {}",
                file.path
            ));
        }
        std::fs::rename(&temp, &destination)
            .map_err(|e| format!("publish {}: {e}", destination.display()))?;
        stats.restored += 1;
    }
    Ok(stats)
}

/// Snapshot directory names, oldest first (names sort by creation time).
pub(crate) fn list_snapshots(target_root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(target_root.join("snapshots")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with(".incomplete-"))
        .collect();
    names.sort();
    names
}

/// Remove all but the newest `keep` snapshots. The `latest` target is never
/// pruned. Returns the removed names.
pub(crate) fn prune_local(target_root: &Path, keep: usize) -> Result<Vec<String>, String> {
    let names = list_snapshots(target_root);
    let latest = latest_snapshot_dir(target_root)
        .and_then(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()));
    let removable = names.len().saturating_sub(keep.max(1));
    let mut removed = Vec::new();
    for name in names.into_iter().take(removable) {
        if Some(&name) == latest.as_ref() {
            continue;
        }
        std::fs::remove_dir_all(target_root.join("snapshots").join(&name))
            .map_err(|e| format!("prune {name}: {e}"))?;
        removed.push(name);
    }
    Ok(removed)
}

// ---------------------------------------------------------------------------
// Target slot: config file, environment, explicit flag (in that order).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Target {
    Local(PathBuf),
    Remote { ssh: String, path: String },
}

fn parse_target(raw: &str) -> Result<Target, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("empty backup target".to_string());
    }
    if let Some(rest) = raw.strip_prefix("rsync://") {
        let (ssh, path) = rest
            .split_once(":/")
            .map(|(ssh, path)| (ssh.to_string(), format!("/{path}")))
            .ok_or_else(|| format!("rsync target must be rsync://user@host:/abs/path: {raw}"))?;
        return Ok(Target::Remote { ssh, path });
    }
    // [user@]host:/abs/path (a colon before any slash). A bare host is an
    // ssh destination as-is, so ssh_config aliases carry user/hostname.
    if let Some((ssh, path)) = raw.split_once(':')
        && !ssh.is_empty()
        && !ssh.contains('/')
        && ssh
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '-'))
        && path.starts_with('/')
    {
        return Ok(Target::Remote {
            ssh: ssh.to_string(),
            path: path.to_string(),
        });
    }
    let expanded = if let Some(rest) = raw.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(rest)
    } else {
        PathBuf::from(raw)
    };
    Ok(Target::Local(expanded))
}

fn load_config(state_root: &Path) -> Option<BackupConfig> {
    let raw = std::fs::read(state_root.join("backup.json")).ok()?;
    let config: BackupConfig = serde_json::from_slice(&raw).ok()?;
    (config.schema == BACKUP_CONFIG_SCHEMA).then_some(config)
}

fn resolve_target(
    state_root: &Path,
    flag: Option<&str>,
) -> Result<Option<(Target, Option<Tier>)>, String> {
    if let Some(flag) = flag {
        return Ok(Some((parse_target(flag)?, None)));
    }
    if let Ok(env) = std::env::var("ANGEL_BACKUP_TARGET")
        && !env.trim().is_empty()
    {
        let tier = std::env::var("ANGEL_BACKUP_TIER")
            .ok()
            .map(|raw| Tier::parse(&raw))
            .transpose()?;
        return Ok(Some((parse_target(&env)?, tier)));
    }
    match load_config(state_root) {
        Some(config) => {
            let tier = config.tier.as_deref().map(Tier::parse).transpose()?;
            Ok(Some((parse_target(&config.target)?, tier)))
        }
        None => Ok(None),
    }
}

fn run_command(program: &str, args: &[String]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("run {program}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Remote snapshot: assemble locally under the target slot's staging area,
/// then rsync it with `--link-dest` against the previous remote snapshot so
/// unchanged files hardlink on the far side too. The manifest ships last,
/// into an `.incomplete-` directory that is renamed and made `latest` only
/// once complete.
fn snapshot_remote(
    state_root: &Path,
    ssh: &str,
    remote_root: &str,
    tier: Tier,
) -> Result<(String, SnapshotStats), String> {
    let staging_root = state_root.join(".backup-staging");
    std::fs::create_dir_all(&staging_root)
        .map_err(|e| format!("create {}: {e}", staging_root.display()))?;
    let (name, stats) = snapshot_local(state_root, &staging_root, tier)?;
    let local_snapshot = staging_root.join("snapshots").join(&name);
    let previous = run_command(
        "ssh",
        &[
            ssh.to_string(),
            format!("readlink {remote_root}/latest 2>/dev/null || true"),
        ],
    )?
    .trim()
    .to_string();
    let incomplete = format!("{remote_root}/snapshots/.incomplete-{name}");
    run_command("ssh", &[ssh.to_string(), format!("mkdir -p {incomplete}")])?;
    let mut rsync_args = vec!["-a".to_string(), "--delete".to_string()];
    if !previous.is_empty() {
        rsync_args.push(format!("--link-dest={remote_root}/{previous}"));
    }
    rsync_args.push(format!("{}/", local_snapshot.display()));
    rsync_args.push(format!("{ssh}:{incomplete}/"));
    let publish = (|| {
        run_command("rsync", &rsync_args)?;
        run_command(
            "ssh",
            &[
                ssh.to_string(),
                format!(
                    "mv {incomplete} {remote_root}/snapshots/{name} && \
                     ln -sfn snapshots/{name} {remote_root}/latest"
                ),
            ],
        )
    })();
    if let Err(error) = publish {
        let _ = run_command("ssh", &[ssh.to_string(), format!("rm -rf {incomplete}")]);
        return Err(error);
    }
    // The local staging copy has served its purpose; keep only the newest
    // as the hardlink basis for the next run so staging stays bounded.
    let _ = prune_local(&staging_root, 1);
    Ok((name, stats))
}

/// Fetch one file from the latest snapshot at the configured target into
/// `staging_dir`, verified against the snapshot manifest's hash. Used by
/// the guardian to recover a dead peer's session file without restoring a
/// whole snapshot. Returns the fetched file's path.
pub(crate) fn fetch_latest_file(
    state_root: &Path,
    relative: &str,
    staging_dir: &Path,
) -> Result<PathBuf, String> {
    if relative.starts_with('/') || relative.split('/').any(|part| part == "..") {
        return Err(format!("backup path escapes snapshot: {relative:?}"));
    }
    let Some((target, _)) = resolve_target(state_root, None)? else {
        return Err(dormant());
    };
    std::fs::create_dir_all(staging_dir)
        .map_err(|e| format!("create {}: {e}", staging_dir.display()))?;
    let fetched = staging_dir.join(
        relative
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| format!("backup path has no file name: {relative:?}"))?,
    );
    let manifest_path = staging_dir.join("manifest.json");
    match &target {
        Target::Local(root) => {
            let snapshot = latest_snapshot_dir(root)
                .ok_or_else(|| "no snapshots at backup target".to_string())?;
            std::fs::copy(snapshot.join("manifest.json"), &manifest_path)
                .map_err(|e| format!("read snapshot manifest: {e}"))?;
            std::fs::copy(snapshot.join(relative), &fetched)
                .map_err(|e| format!("fetch {relative}: {e}"))?;
        }
        Target::Remote { ssh, path } => {
            let latest = run_command("ssh", &[ssh.clone(), format!("readlink {path}/latest")])?
                .trim()
                .to_string();
            if latest.is_empty() {
                return Err("no snapshots at backup target".to_string());
            }
            for (remote_name, local) in [("manifest.json", &manifest_path), (relative, &fetched)] {
                run_command(
                    "rsync",
                    &[
                        "-a".to_string(),
                        format!("{ssh}:{path}/{latest}/{remote_name}"),
                        local.display().to_string(),
                    ],
                )?;
            }
        }
    }
    let manifest: Manifest = serde_json::from_slice(
        &std::fs::read(&manifest_path).map_err(|e| format!("read manifest: {e}"))?,
    )
    .map_err(|e| format!("parse manifest: {e}"))?;
    let expected = manifest
        .files
        .iter()
        .find(|file| file.path == relative)
        .and_then(|file| file.sha256.as_deref())
        .ok_or_else(|| format!("{relative} is not in the latest snapshot"))?;
    let actual = hash_file(&fetched).map_err(|e| format!("hash fetched file: {e}"))?;
    if actual != expected {
        let _ = std::fs::remove_file(&fetched);
        return Err(format!("fetched {relative} fails manifest verification"));
    }
    Ok(fetched)
}

const USAGE: &str = "\
usage: angel --backup <command> [options]

  run [--tier core|bulk|all] [--to TARGET]     take a snapshot now
  restore [--from TARGET] [--snapshot NAME] [--force]
  status [--from TARGET]                       target, tier, snapshots
  prune --keep N [--from TARGET]               (local targets only)

TARGET is a local path or [user@]host:/abs/path (rsync over ssh). Default
target/tier come from ~/.angelX/backup.json (schema angelX.backup/v1:
{\"schema\":\"angelX.backup/v1\",\"target\":\"...\",\"tier\":\"core\"}) or
ANGEL_BACKUP_TARGET / ANGEL_BACKUP_TIER. Unconfigured, backup is dormant.
";

pub(crate) fn cli(args: impl Iterator<Item = std::ffi::OsString>) -> std::io::Result<()> {
    let args: Vec<String> = args.map(|arg| arg.to_string_lossy().into_owned()).collect();
    let state_root = super::workspace_store::angel_dir();
    let result = dispatch(&state_root, &args);
    match result {
        Ok(text) => {
            println!("{text}");
            Ok(())
        }
        Err(error) => Err(std::io::Error::other(error)),
    }
}

fn take_flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn dormant() -> String {
    "backup is not configured: set a target in ~/.angelX/backup.json \
     (schema angelX.backup/v1), ANGEL_BACKUP_TARGET, or --to/--from"
        .to_string()
}

fn dispatch(state_root: &Path, args: &[String]) -> Result<String, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(USAGE.to_string());
    };
    let rest = &args[1..];
    match command {
        "run" => {
            let flag_target = take_flag(rest, "--to");
            let Some((target, config_tier)) = resolve_target(state_root, flag_target.as_deref())?
            else {
                return Ok(dormant());
            };
            let tier = match take_flag(rest, "--tier") {
                Some(raw) => Tier::parse(&raw)?,
                None => config_tier.unwrap_or(Tier::Core),
            };
            let (name, stats) = match &target {
                Target::Local(path) => snapshot_local(state_root, path, tier)?,
                Target::Remote { ssh, path } => snapshot_remote(state_root, ssh, path, tier)?,
            };
            Ok(format!(
                "snapshot {name} ({tier:?}): {} files, {} copied, {} linked, {} symlinks, {} skipped, {} bytes",
                stats.files, stats.copied, stats.linked, stats.symlinks, stats.skipped, stats.bytes
            ))
        }
        "restore" => {
            let flag_target = take_flag(rest, "--from");
            let Some((target, _)) = resolve_target(state_root, flag_target.as_deref())? else {
                return Ok(dormant());
            };
            let force = rest.iter().any(|arg| arg == "--force");
            let snapshot_dir = match &target {
                Target::Local(root) => match take_flag(rest, "--snapshot") {
                    Some(name) => root.join("snapshots").join(name),
                    None => latest_snapshot_dir(root)
                        .ok_or_else(|| "no snapshots at target".to_string())?,
                },
                Target::Remote { .. } => {
                    return Err("remote restore: rsync the snapshot directory down first, \
                         then restore with --from <local target>"
                        .to_string());
                }
            };
            let stats = restore_local(&snapshot_dir, state_root, force)?;
            let mut text = format!(
                "restored {} files ({} already identical, {} refused)",
                stats.restored, stats.identical, stats.refused
            );
            if stats.refused > 0 && !force {
                text.push_str("; re-run with --force to overwrite differing files");
            }
            Ok(text)
        }
        "status" => {
            let flag_target = take_flag(rest, "--from");
            let Some((target, config_tier)) = resolve_target(state_root, flag_target.as_deref())?
            else {
                return Ok(dormant());
            };
            let tier = config_tier.unwrap_or(Tier::Core);
            match &target {
                Target::Local(root) => {
                    let names = list_snapshots(root);
                    let latest = latest_snapshot_dir(root)
                        .and_then(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
                        .unwrap_or_else(|| "none".to_string());
                    Ok(format!(
                        "target {} (local), tier {tier:?}\nsnapshots: {}\nlatest: {latest}",
                        root.display(),
                        names.len()
                    ))
                }
                Target::Remote { ssh, path } => Ok(format!(
                    "target {ssh}:{path} (rsync/ssh), tier {tier:?}\n\
                     (snapshot listing is local-only; run status with --from a local path)"
                )),
            }
        }
        "prune" => {
            let keep: usize = take_flag(rest, "--keep")
                .and_then(|raw| raw.parse().ok())
                .ok_or_else(|| "prune requires --keep N".to_string())?;
            let flag_target = take_flag(rest, "--from");
            let Some((target, _)) = resolve_target(state_root, flag_target.as_deref())? else {
                return Ok(dormant());
            };
            match &target {
                Target::Local(root) => {
                    let removed = prune_local(root, keep)?;
                    Ok(format!("pruned {} snapshots", removed.len()))
                }
                Target::Remote { .. } => {
                    Err("prune is local-only; remove old snapshot dirs on the host".to_string())
                }
            }
        }
        "help" | "--help" | "-h" => Ok(USAGE.to_string()),
        other => Err(format!("unknown backup command {other:?}\n{USAGE}")),
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/backup__tests.rs"]
mod tests;
