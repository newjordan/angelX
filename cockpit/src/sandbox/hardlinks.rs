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
mod tests {
    use super::*;

    #[test]
    fn landlock_only_hardlink_write_keeps_outside_unchanged_and_local_links_work() {
        let _guard = crate::tests::env_lock();
        if let Some(root) = std::env::var_os("ANGEL_T_ALIAS_CHILD") {
            let root = PathBuf::from(root);
            let policy = super::super::SandboxPolicy {
                writable_roots: vec![root.clone()],
                allow_network: false,
                enforce: true,
                mandatory: true,
                sealed_reads: vec![],
                deny_reads: vec![],
            };
            unsafe {
                std::env::set_var(
                    super::super::HELPER_POLICY_ENV,
                    serde_json::to_string(&policy).unwrap(),
                );
            }
            let args = [
                std::ffi::OsString::from("--"),
                std::ffi::OsString::from("/bin/sh"),
                std::ffi::OsString::from("-c"),
                std::ffi::OsString::from(
                    "printf changed > \"$1/alias\" && ln \"$1/alias\" \"$1/new-local-link\" && test \"$(cat \"$1/new-local-link\")\" = changed && ! (printf escape > \"$1/../outside\")",
                ),
                std::ffi::OsString::from("alias-test"),
                root.into_os_string(),
            ];
            super::super::exec_helper_inner(args, || {
                super::super::compatibility::classify(
                    true,
                    true,
                    true,
                    false,
                    "bwrap: setting up uid map: Permission denied",
                )
            })
            .unwrap();
            return;
        }
        let fixture = TestRoot::new();
        let root = fixture.path().join("workspace");
        std::fs::create_dir(&root).unwrap();
        let outside = fixture.path().join("outside");
        std::fs::write(&outside, "original").unwrap();
        std::fs::hard_link(&outside, root.join("alias")).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child.args(["--exact", "sandbox::hardlinks::tests::landlock_only_hardlink_write_keeps_outside_unchanged_and_local_links_work", "--nocapture"]);
        child.env("ANGEL_T_ALIAS_CHILD", &root);
        let channel = super::super::status::attach(&mut child).unwrap();
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let receipt = channel.receive().expect("helper status receipt");
        assert_eq!(receipt["sandbox_profile"], "landlock-only");
        assert!(
            receipt["notice"]
                .as_str()
                .unwrap()
                .contains("userns denied by AppArmor")
        );
        assert_eq!(
            receipt["aliases_copied"],
            serde_json::json!([root.join("alias")])
        );
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "original");
    }

    #[test]
    fn landlock_alias_copy_preserves_outside_and_new_local_links() {
        let _guard = crate::tests::env_lock();
        let fixture = TestRoot::new();
        let root = fixture.path().join("workspace");
        std::fs::create_dir(&root).unwrap();
        let outside = fixture.path().join("outside");
        let alias = root.join("alias");
        std::fs::write(&outside, "original").unwrap();
        std::fs::hard_link(&outside, &alias).unwrap();
        let metadata = std::fs::metadata(&outside).unwrap();
        let aliases = scan(std::slice::from_ref(&root)).unwrap();
        let limited = privatize_with_limits(&aliases, 0, 0);
        assert_eq!(limited["aliases_unprotected"], serde_json::json!([alias]));
        let receipt = privatize(&aliases);
        assert_eq!(receipt["aliases_copied"], serde_json::json!([alias]));
        assert_eq!(std::fs::metadata(&alias).unwrap().mode(), metadata.mode());
        assert_eq!(
            std::fs::metadata(&alias).unwrap().modified().unwrap(),
            metadata.modified().unwrap()
        );
        std::fs::write(&alias, "changed").unwrap();
        std::fs::hard_link(&alias, root.join("local-link")).unwrap();
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "original");
        assert_eq!(
            std::fs::read_to_string(root.join("local-link")).unwrap(),
            "changed"
        );
        assert!(
            privatize(&scan(std::slice::from_ref(&root)).unwrap())["aliases_copied"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn complete_internal_alias_groups_remain_writable_but_external_aliases_do_not() {
        let fixture = TestRoot::new();
        let first = fixture.path().join("first");
        let second = fixture.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let local = first.join("local");
        let sibling = second.join("local-alias");
        std::fs::write(&local, "local").unwrap();
        std::fs::hard_link(&local, &sibling).unwrap();
        let outside = fixture.path().join("outside");
        std::fs::write(&outside, "outside").unwrap();
        let external = first.join("external");
        std::fs::hard_link(&outside, &external).unwrap();
        let partial_grant = scan(std::slice::from_ref(&first)).unwrap();
        assert_eq!(partial_grant.paths, vec![external.clone(), local.clone()]);
        let whole_grant = scan(&[first.clone(), second]).unwrap();
        assert_eq!(whole_grant.paths, vec![external]);
        // A bounded/incomplete walk cannot infer that unseen aliases are local.
        let incomplete = scan_with_limit(&[local], 1).unwrap();
        assert_eq!(incomplete.paths.len(), 1);
    }

    #[test]
    fn hardlink_scan_reports_aliases_and_limits_without_following_symlinks() {
        let fixture = TestRoot::new();
        let root = fixture.path().join("workspace");
        std::fs::create_dir(&root).unwrap();
        let outside = fixture.path().join("outside");
        std::fs::write(&outside, "original").unwrap();
        std::fs::hard_link(&outside, root.join("alias")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("symlink")).unwrap();
        std::fs::write(root.join("ordinary"), "local").unwrap();
        let scan = scan(std::slice::from_ref(&root)).unwrap();
        assert_eq!(scan.paths, vec![root.join("alias")]);
        assert!(scan.limited.is_empty());
        let receipt = scan.receipt();
        assert_eq!(receipt["hardlink_readonly_count"], 1);
        assert!(!receipt.to_string().contains("original"));
        assert!(!receipt.to_string().contains(&root.display().to_string()));
        let limited = scan_with_limit(&[root], 1).unwrap();
        assert!(!limited.limited.is_empty());
    }
}
