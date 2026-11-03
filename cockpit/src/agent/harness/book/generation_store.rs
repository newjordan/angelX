//! Durable, content-addressed snapshots of the book's reloadable catalog.
//!
//! The caller validates the catalog schema. This store preserves its exact
//! canonical bytes so a restored conversation can recover the same generation.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::platform::workspace_store::private_io::PrivateDirectory;

const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    #[cfg(test)]
    {
        return TEST_ROOT
            .with(|root| root.borrow().clone())
            .unwrap_or_else(|| {
                static DEFAULT_TEST_ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
                DEFAULT_TEST_ROOT
                    .get_or_init(|| {
                        let nonce = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos();
                        std::env::temp_dir().join(format!(
                            "angel-book-generations-test-{}-{nonce}",
                            std::process::id()
                        ))
                    })
                    .clone()
            });
    }
    #[cfg(not(test))]
    crate::platform::workspace_store::angel_subdir("book-generations")
}

#[cfg(test)]
thread_local! {
    static TEST_ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Isolate a fixture without changing other parallel test threads' store.
#[cfg(test)]
pub(crate) struct TestRootGuard(Option<PathBuf>);

#[cfg(test)]
impl TestRootGuard {
    pub(crate) fn enter(root: PathBuf) -> Self {
        Self(TEST_ROOT.with(|slot| slot.replace(Some(root))))
    }
}

#[cfg(test)]
impl Drop for TestRootGuard {
    fn drop(&mut self) {
        TEST_ROOT.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

fn valid_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

struct Lock(std::fs::File);
impl Lock {
    fn acquire(dir: &PrivateDirectory) -> Result<Self, String> {
        let file = dir
            .lock_file(OsStr::new(".publish.lock"))
            .map_err(|error| format!("book generation lock: {error}"))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            // SAFETY: file owns a live descriptor until the guard is dropped.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(Self(file));
            }
            let error = std::io::Error::last_os_error();
            if !matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) {
                return Err(format!("book generation lock: {error}"));
            }
            if Instant::now() >= deadline {
                return Err("book generation lock timed out".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        // SAFETY: the guard still owns the lock's descriptor.
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

fn read_generation(dir: &PrivateDirectory, digest: &str) -> Result<Option<Vec<u8>>, String> {
    if !valid_digest(digest) {
        return Err("invalid book generation digest".into());
    }
    let name = format!("{digest}.json");
    let Some(file) = dir
        .existing(OsStr::new(&name))
        .map_err(|error| format!("book generation read: {error}"))?
    else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.take((MAX_CATALOG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("book generation read: {error}"))?;
    if bytes.len() > MAX_CATALOG_BYTES || crate::knowledge::cut::sha256_hex(&bytes) != digest {
        return Err("book generation is corrupt or exceeds its byte limit".into());
    }
    Ok(Some(bytes))
}

/// Durably publish exact validated bytes before recording their digest in the
/// conversation. Concurrent publishers of the same content share one file.
pub(crate) fn publish(bytes: &[u8]) -> Result<String, String> {
    publish_in(&root(), bytes)
}

fn publish_in(path: &Path, bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() || bytes.len() > MAX_CATALOG_BYTES {
        return Err("book generation has an invalid byte length".into());
    }
    let digest = crate::knowledge::cut::sha256_hex(bytes);
    let dir = PrivateDirectory::open(path)
        .map_err(|error| format!("book generation directory: {error}"))?;
    dir.secure_owner_only()
        .map_err(|error| format!("book generation directory: {error}"))?;
    let _lock = Lock::acquire(&dir)?;
    if read_generation(&dir, &digest)?.is_some() {
        // A prior publisher may have renamed the file but failed its directory
        // fsync. Do not report durable publication without retrying that sync.
        dir.sync()
            .map_err(|error| format!("book generation publication: {error}"))?;
        return Ok(digest);
    }
    let name = format!("{digest}.json");
    let temporary = format!(
        ".{}-{}-{}.tmp",
        digest,
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let mut file = dir
        .create_new(OsStr::new(&temporary))
        .map_err(|error| format!("book generation staging: {error}"))?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        dir.publish(OsStr::new(&temporary), &file, OsStr::new(&name), None)?;
        dir.sync()
    })();
    if result.is_err() {
        let _ = dir.remove_owned(OsStr::new(&temporary), &file);
    }
    result.map_err(|error| format!("book generation publication: {error}"))?;
    Ok(digest)
}

/// Load only a generation whose bytes still match the saved content identity.
pub(crate) fn load(digest: &str) -> Result<Vec<u8>, String> {
    load_in(&root(), digest)
}

pub(crate) fn load_in(path: &Path, digest: &str) -> Result<Vec<u8>, String> {
    if !valid_digest(digest) {
        return Err("invalid book generation digest".into());
    }
    let dir = PrivateDirectory::open_existing(path)
        .map_err(|error| format!("book generation directory: {error}"))?;
    read_generation(&dir, digest)?.ok_or_else(|| format!("book generation {digest} is missing"))
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__generation_store_tests.rs"]
mod tests;
