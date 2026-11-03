//! Workspace-scoped persistence paths and the atomic store writers.
//!
//! The cockpit's `platform::workspace_store` re-exports these at their old
//! paths; the repo-identity layer (git probes, the work-context store) stays
//! there because it needs cockpit state.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write as _;
use std::path::Path;

/// A stable, filesystem-safe key for a workspace path: a sanitized,
/// length-bounded prefix plus a deterministic hash suffix.
pub fn workspace_key(workspace: &Path) -> String {
    let s = workspace.to_string_lossy();
    let mut hasher = DefaultHasher::new();
    if workspace.to_str().is_some() {
        // Keep existing keys for all UTF-8 paths; invalid paths must not
        // collapse to the same replacement-character spelling.
        s.hash(&mut hasher);
    } else {
        b"angel-workspace-non-utf8-v1".hash(&mut hasher);
        workspace.as_os_str().as_encoded_bytes().hash(&mut hasher);
    }
    let h = hasher.finish();
    let sanitized: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed: String = sanitized.trim_matches('-').chars().take(60).collect();
    let trimmed = if trimmed.is_empty() {
        "ws".to_string()
    } else {
        trimmed
    };
    format!("{trimmed}-{h:016x}")
}

/// Atomically publish a private state file and make the file plus directory
/// entry durable before returning. A unique same-directory temporary keeps
/// concurrent cockpit processes from corrupting each other's staging bytes.
///
/// This is the **durable** tier: after it returns, the bytes survive a power
/// cut. Use it for state that cannot be regenerated (sessions, memories,
/// goals, credentials, ledgers). For regenerable or bulk output where a torn
/// last write is recoverable by rewriting, use [`write_atomic`], which keeps
/// the atomic publish but skips both fsyncs and secret redaction.
pub fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let redacted = crate::host::secrets::redact_bytes(bytes);
    write_atomic_inner(path, redacted.as_slice(), true)
}

/// Atomically publish a file (unique temp sibling + rename) **without**
/// fsync or redaction. A crash can lose the newest write or expose the
/// previous complete file, but a reader never observes torn bytes and
/// concurrent writers never corrupt each other's staging files.
///
/// The **bulk** tier: only for non-sensitive, regenerable output (art,
/// exports, reports, caches with a rebuild path). Anything a user would
/// mourn belongs in [`write_private_atomic`].
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic_inner(path, bytes, false)
}

/// Durable publish of **exact** bytes: fsync file + directory and private
/// (0600) like [`write_private_atomic`], but without secret redaction.
/// Only for stores whose bytes must round-trip verbatim — credential files
/// holding live tokens, or content whose hash certifies the exact body.
/// Callers own proving the bytes belong on disk unredacted.
pub fn write_private_atomic_exact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic_inner(path, bytes, true)
}

/// Serialize read/compare/write transactions across processes. Keep this
/// sibling inode after unlocking: unlinking it would let a third writer lock
/// a different inode while another process still holds the old one.
pub fn try_private_lock(path: &Path) -> Result<std::fs::File, String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().ok_or("state path has no file name")?);
    name.push(".lock");
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(parent.join(name))
        .map_err(|error| error.to_string())?;
    file.try_lock()
        .map_err(|error| format!("state is busy: {error}"))?;
    Ok(file)
}

fn write_atomic_inner(path: &Path, bytes: &[u8], durable: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create state directory {}: {error}", parent.display()))?;
    static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("state path has no file name: {}", path.display()))?
        .to_string_lossy();
    let temp = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        serial
    ));
    #[cfg(any(test, feature = "fault-hooks"))]
    fault::before_atomic_open(&temp);
    let mut owns_temp = false;
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            // Durable state is private by default; bulk output keeps the
            // caller's umask so exported artifacts stay shareable.
            options
                .mode(if durable { 0o600 } else { 0o666 })
                .custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options
            .open(&temp)
            .map_err(|error| format!("open state temp {}: {error}", temp.display()))?;
        owns_temp = true;
        file.write_all(bytes)
            .map_err(|error| format!("write state temp {}: {error}", temp.display()))?;
        if durable {
            file.sync_all()
                .map_err(|error| format!("sync state temp {}: {error}", temp.display()))?;
        }
        drop(file);
        #[cfg(any(test, feature = "fault-hooks"))]
        fault::before_atomic_rename(&temp);
        std::fs::rename(&temp, path).map_err(|error| {
            format!(
                "publish state {} from {}: {error}",
                path.display(),
                temp.display()
            )
        })?;
        owns_temp = false;
        #[cfg(any(test, feature = "fault-hooks"))]
        fault::after_atomic_rename(path)?;
        #[cfg(unix)]
        if durable {
            std::fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| format!("sync state directory {}: {error}", parent.display()))?;
        }
        Ok(())
    })();
    if result.is_err() && owns_temp {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

/// Test-only fault injection for the atomic writers: one-shot hooks armed per
/// thread, so other tests and writers are unaffected. Compiled for this
/// crate's tests and for dependents that enable the `fault-hooks` feature
/// (the cockpit's dev-dependency); a release build carries none of it.
#[cfg(any(test, feature = "fault-hooks"))]
pub mod fault {
    use std::cell::RefCell;
    use std::path::Path;

    pub type OpenHook = Box<dyn FnOnce(&Path)>;
    pub type RenameHook = Box<dyn FnOnce(&Path)>;
    pub type PublishedHook = Box<dyn FnOnce(&Path) -> Result<(), String>>;

    thread_local! {
        pub static BEFORE_OPEN: RefCell<Option<OpenHook>> = const { RefCell::new(None) };
        pub static BEFORE_RENAME: RefCell<Option<RenameHook>> = const { RefCell::new(None) };
        pub static AFTER_RENAME: RefCell<Option<PublishedHook>> = const { RefCell::new(None) };
    }

    pub(super) fn before_atomic_open(path: &Path) {
        if let Some(hook) = BEFORE_OPEN.with(|slot| slot.borrow_mut().take()) {
            hook(path);
        }
    }

    pub(super) fn before_atomic_rename(path: &Path) {
        if let Some(hook) = BEFORE_RENAME.with(|slot| slot.borrow_mut().take()) {
            hook(path);
        }
    }

    pub(super) fn after_atomic_rename(path: &Path) -> Result<(), String> {
        AFTER_RENAME
            .with(|slot| slot.borrow_mut().take())
            .map_or(Ok(()), |hook| hook(path))
    }
}
