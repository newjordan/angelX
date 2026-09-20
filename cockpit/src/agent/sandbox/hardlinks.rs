//! Pre-launch inode-alias protection. Only metadata and path digests are retained.
use std::fs::File;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

// Bound metadata work, not the worker's workspace size. Incomplete coverage is
// reported; unscanned directories are never made read-only.
const MAX_ENTRIES: usize = 50_000;

#[derive(Debug, Default)]
pub(super) struct Scan {
    pub paths: Vec<PathBuf>,
    pub limited: Vec<PathBuf>,
    pub visited: usize,
    /// Shared temp roots excluded from the walk (documented non-promise).
    pub skipped_temp_roots: Vec<PathBuf>,
}

impl Scan {
    pub fn receipt(&self) -> serde_json::Value {
        let digests = |paths: &[PathBuf]| {
            paths
                .iter()
                .map(|path| super::sha256_hex(path.as_os_str().as_bytes()))
                .collect::<Vec<_>>()
        };
        serde_json::json!({
            "hardlink_readonly_count": self.paths.len(),
            "hardlink_path_sha256": digests(&self.paths),
            "scan_entries": self.visited,
            "scan_limit": MAX_ENTRIES,
            "scan_unscanned_sha256": digests(&self.limited),
            "scan_skipped_temp_roots_sha256": digests(&self.skipped_temp_roots),
            "scan_complete": self.limited.is_empty(),
        })
    }

    /// The pre-exec stderr line: counts only. The per-path digest arrays can
    /// run to kilobytes on a limited scan and, since the helper shares the
    /// child's stderr, they used to displace the child's own output from every
    /// bounded tool receipt (a `node --test` tail became a hash dump). The full
    /// digest receipt stays available under `ANGEL_SANDBOX_TRACE=1`.
    pub fn stderr_line(&self) -> serde_json::Value {
        if matches!(
            std::env::var("ANGEL_SANDBOX_TRACE").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        ) {
            return self.receipt();
        }
        serde_json::json!({
            "hardlink_readonly_count": self.paths.len(),
            "scan_entries": self.visited,
            "scan_limit": MAX_ENTRIES,
            "scan_unscanned_count": self.limited.len(),
            "scan_skipped_temp_roots": self.skipped_temp_roots.len(),
            "scan_complete": self.limited.is_empty(),
        })
    }
}

pub(super) fn scan(roots: &[PathBuf]) -> io::Result<Scan> {
    let mut roots = roots
        .iter()
        .filter(|path| !path.starts_with("/proc"))
        .map(|path| path.canonicalize())
        .collect::<io::Result<Vec<_>>>()?;
    roots.sort();
    roots.dedup();
    // Shared temp roots (/tmp, /var/tmp, the process temp dir) are writable by
    // policy and are NOT alias-scanned: they hold tens of thousands of foreign
    // entries, so walking them on every spawn cost seconds and broke tool
    // liveness, and writes under them are a documented non-promise
    // (docs/security/workspace-confinement.md, default-command-temp). Explicit
    // workspace/cache roots that merely live under a temp root stay scanned.
    let shared_temp = shared_temp_roots();
    let mut skipped_temp = Vec::new();
    roots.retain(|root| {
        if shared_temp.iter().any(|temp| temp == root) {
            skipped_temp.push(root.clone());
            false
        } else {
            true
        }
    });
    let mut disjoint = Vec::<PathBuf>::new();
    for root in roots {
        if root == std::path::Path::new("/") {
            return Err(io::Error::other(
                "hardlink scan rejects a host-root write grant",
            ));
        }
        if !disjoint.iter().any(|parent| root.starts_with(parent)) {
            disjoint.push(root);
        }
    }
    let mut scan = scan_with_limit(&disjoint, MAX_ENTRIES)?;
    scan.skipped_temp_roots = skipped_temp;
    Ok(scan)
}

/// Canonical shared temp roots the alias scan never walks.
fn shared_temp_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
        std::env::temp_dir(),
    ];
    if let Some(dir) = std::env::var_os("TMPDIR") {
        roots.push(PathBuf::from(dir));
    }
    roots
        .into_iter()
        .filter_map(|root| root.canonicalize().ok())
        .collect()
}

fn scan_with_limit(roots: &[PathBuf], limit: usize) -> io::Result<Scan> {
    let mut scan = Scan::default();
    let mut aliases = std::collections::BTreeMap::<(u64, u64), (u64, Vec<PathBuf>)>::new();
    let mut pending = roots.to_vec();
    while let Some(path) = pending.pop() {
        if path.starts_with("/proc") {
            continue;
        }
        if scan.visited >= limit {
            scan.limited.push(path);
            continue;
        }
        scan.visited += 1;
        let scan_error = |operation: &str, error: io::Error| {
            io::Error::new(
                error.kind(),
                format!(
                    "hardlink scan {operation}: path_sha256={} errno={:?}: {error}",
                    super::sha256_hex(path.as_os_str().as_bytes()),
                    error.raw_os_error(),
                ),
            )
        };
        // An entry the scan cannot inspect (another user's 0700 directory under a
        // shared temp root, a file that vanished mid-walk) must never abort the
        // sandbox launch: it is recorded as unscanned and the receipt says the
        // alias check was partial. Only a genuinely broken root fails the plan.
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                scan.limited.push(path);
                continue;
            }
            Err(error) => return Err(scan_error("metadata", error)),
        };
        if metadata.is_file() && metadata.nlink() > 1 {
            let (links, paths) = aliases.entry((metadata.dev(), metadata.ino())).or_default();
            *links = (*links).max(metadata.nlink());
            paths.push(path);
        } else if metadata.is_dir() {
            // Pin before enumeration; a swapped ancestor symlink is rejected.
            let directory: File = match super::bwrap::pin_source(&path) {
                Ok(directory) => directory,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    scan.limited.push(path);
                    continue;
                }
                Err(error) => return Err(scan_error("pin directory", error)),
            };
            use std::os::fd::AsRawFd;
            let entries =
                match std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd())) {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                        scan.limited.push(path);
                        continue;
                    }
                    Err(error) => return Err(scan_error("enumerate directory", error)),
                };
            for entry in entries {
                // Bound memory as well as metadata calls. Protect the entire
                // directory if enumeration would exceed the remaining budget.
                if pending.len() >= limit.saturating_sub(scan.visited) {
                    scan.limited.push(path.clone());
                    break;
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(scan_error("read directory entry", error)),
                };
                pending.push(path.join(entry.file_name()));
            }
        }
    }
    // Cargo commonly hardlinks incremental artifacts within its own writable
    // trees. Only protect an inode when some alias is outside the observed
    // grants. Missing/inaccessible/unscanned aliases remain conservatively
    // external because the observed count is then lower than st_nlink.
    for (links, mut paths) in aliases.into_values() {
        paths.sort();
        paths.dedup();
        if links > paths.len() as u64 {
            scan.paths.extend(paths);
        }
    }
    scan.paths.sort();
    scan.paths.dedup();
    scan.limited.sort();
    scan.limited.dedup();
    Ok(scan)
}

/// Copy work is bounded; exhausted or failed copies are explicit degradation.
pub(super) fn privatize(scan: &Scan) -> serde_json::Value {
    privatize_with_limits(scan, 64, 64 * 1024 * 1024)
}

fn privatize_with_limits(scan: &Scan, max_count: usize, max_bytes: u64) -> serde_json::Value {
    use std::collections::HashMap;
    let mut counts = HashMap::new();
    for path in &scan.paths {
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            *counts.entry((meta.dev(), meta.ino())).or_insert(0_u64) += 1;
        }
    }
    let external: std::collections::HashSet<_> = scan
        .paths
        .iter()
        .filter_map(|path| {
            let meta = std::fs::symlink_metadata(path).ok()?;
            let key = (meta.dev(), meta.ino());
            (meta.nlink() > *counts.get(&key).unwrap_or(&0)).then_some(key)
        })
        .collect();
    let mut copied = Vec::new();
    let mut unprotected = scan.limited.clone();
    let mut bytes = 0_u64;
    let mut attempted = 0;
    for path in &scan.paths {
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            unprotected.push(path.clone());
            continue;
        };
        if !external.contains(&(meta.dev(), meta.ino())) {
            continue; // Every alias is in the scanned writable trees.
        }
        if attempted >= max_count || meta.len() > max_bytes.saturating_sub(bytes) {
            unprotected.push(path.clone());
            continue;
        }
        // Charge attempted bytes too, so IO failures cannot evade the copy bound.
        attempted += 1;
        bytes += meta.len();
        if private_copy(path, &meta).is_ok() {
            copied.push(path.clone());
        } else {
            unprotected.push(path.clone());
        }
    }
    serde_json::json!({
        "aliases_copied": copied,
        "aliases_unprotected": unprotected,
        "alias_copy_bytes_attempted": bytes,
        "alias_copy_max_bytes": max_bytes,
        "alias_copy_max_count": max_count,
        "scan": scan.receipt(),
    })
}

fn private_copy(path: &std::path::Path, expected: &std::fs::Metadata) -> io::Result<()> {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("alias has no parent"))?;
    let directory = super::bwrap::pin_source(parent)?;
    let pinned = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
    let target = pinned.join(
        path.file_name()
            .ok_or_else(|| io::Error::other("alias has no name"))?,
    );
    let mut source = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&target)?;
    let metadata = source.metadata()?;
    if !metadata.is_file()
        || (metadata.dev(), metadata.ino(), metadata.len())
            != (expected.dev(), expected.ino(), expected.len())
    {
        return Err(io::Error::other("alias changed during scan"));
    }
    let temporary = pinned.join(format!(
        ".angel-alias-{}-{}",
        std::process::id(),
        metadata.ino()
    ));
    let mut copy = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let result = (|| {
        let count = io::copy(&mut (&mut source).take(metadata.len()), &mut copy)?;
        if count != metadata.len() || source.metadata()?.len() != metadata.len() {
            return Err(io::Error::other("alias size changed during copy"));
        }
        copy.set_permissions(metadata.permissions())?;
        copy.set_times(
            std::fs::FileTimes::new()
                .set_modified(metadata.modified()?)
                .set_accessed(metadata.accessed()?),
        )?;
        let current = std::fs::symlink_metadata(&target)?;
        if (current.dev(), current.ino()) != (metadata.dev(), metadata.ino()) {
            return Err(io::Error::other("alias replaced during copy"));
        }
        std::fs::rename(&temporary, &target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
pub(crate) struct TestRoot(PathBuf);

#[cfg(test)]
impl TestRoot {
    pub(crate) fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "angel-hardlink-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create hardlink fixture: {error}"),
            }
        }
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/sandbox__hardlinks__tests.rs"]
mod tests;
