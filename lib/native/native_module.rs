//! Loading operator-trusted native modules from immutable private snapshots.
//! Loaded code has full process authority; the ABI is not a sandbox.

use std::ffi::{CStr, CString, OsStr};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// Owns a loader handle. Callers must keep it alive while using any symbol.
pub(crate) struct Library(*mut std::ffi::c_void);

// A loaded image is immutable. Its implementation must be reentrant.
unsafe impl Send for Library {}
unsafe impl Sync for Library {}

impl Drop for Library {
    fn drop(&mut self) {
        unsafe { libc::dlclose(self.0) };
    }
}

impl Library {
    /// Look up a symbol. Its type and lifetime are the caller's responsibility.
    pub(crate) fn symbol(&self, name: &CStr) -> *mut std::ffi::c_void {
        unsafe { libc::dlsym(self.0, name.as_ptr()) }
    }
}

fn read(path: &Path, limit: u64) -> Result<(Vec<u8>, Fingerprint), String> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| format!("native module: {error}"))?;
    let meta = file.metadata().map_err(|error| error.to_string())?;
    if !meta.is_file() || meta.len() > limit {
        return Err("native module must be a bounded regular file".into());
    }
    let identity = Fingerprint::from_metadata(&meta);
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > limit {
        return Err("native module is empty or exceeds its byte limit".into());
    }
    let after = file.metadata().map_err(|error| error.to_string())?;
    if Fingerprint::from_metadata(&after) != identity {
        return Err("native module changed during capture".into());
    }
    Ok((bytes, identity))
}

/// Capture a complete publication without following the final symlink.
#[cfg(test)]
pub(crate) fn read_stable(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    if !path.is_absolute() {
        return Err("native module path must be absolute".into());
    }
    let expected = fingerprint(path).map_err(|error| error.to_string())?;
    read_stable_matching(path, limit, expected)
}

fn read_stable_matching(path: &Path, limit: u64, expected: Fingerprint) -> Result<Vec<u8>, String> {
    let (bytes, first) = read(path, limit)?;
    let (second_bytes, second) = read(path, limit)?;
    if first != expected
        || second != expected
        || bytes != second_bytes
        || fingerprint(path).map_err(|error| error.to_string())? != expected
    {
        return Err("native module changed during capture".into());
    }
    Ok(bytes)
}

struct SnapshotDir(PathBuf);
impl Drop for SnapshotDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Load exactly these bytes; the private copy survives until `dlopen` finishes.
/// Native constructors execute during this call, before any ABI validation.
pub(crate) fn load(bytes: &[u8]) -> Result<Library, String> {
    let template = std::env::temp_dir().join("angel-native-module-XXXXXX");
    let mut template = CString::new(template.as_os_str().as_bytes())
        .map_err(|error| error.to_string())?
        .into_bytes_with_nul();
    let ptr = unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) };
    if ptr.is_null() {
        return Err(format!(
            "native module snapshot: {}",
            std::io::Error::last_os_error()
        ));
    }
    let name = unsafe { CStr::from_ptr(ptr) };
    let directory = SnapshotDir(PathBuf::from(OsStr::from_bytes(name.to_bytes())));
    let copy = directory.0.join("module.so");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&copy)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    drop(file);
    let path = CString::new(copy.as_os_str().as_bytes()).map_err(|error| error.to_string())?;
    let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        return Err("native module could not be loaded".into());
    }
    Ok(Library(handle))
}

use std::sync::{Arc, RwLock};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fingerprint {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}
impl Fingerprint {
    fn from_metadata(m: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            dev: m.dev(),
            ino: m.ino(),
            len: m.len(),
            mtime: m.mtime(),
            mtime_nsec: m.mtime_nsec(),
            ctime: m.ctime(),
            ctime_nsec: m.ctime_nsec(),
        }
    }
}
fn fingerprint(path: &Path) -> std::io::Result<Fingerprint> {
    std::fs::symlink_metadata(path).map(|m| Fingerprint::from_metadata(&m))
}
pub(crate) struct Publisher {
    pub(crate) previous: Option<Fingerprint>,
    last_error: Option<String>,
    label: &'static str,
    missing_is_unavailable: bool,
}
impl Publisher {
    pub(crate) fn new(label: &'static str, missing_is_unavailable: bool) -> Self {
        Self {
            previous: None,
            last_error: None,
            label,
            missing_is_unavailable,
        }
    }
    pub(crate) fn refresh<T>(
        &mut self,
        path: &Path,
        active: &RwLock<Arc<T>>,
        load: impl FnOnce(&Path, Fingerprint) -> Result<Arc<T>, String>,
    ) {
        let current = match fingerprint(path) {
            Ok(v) => v,
            Err(e) => {
                let kind = if self.missing_is_unavailable {
                    "unavailable"
                } else {
                    "ignored"
                };
                self.report(format!("{} module {kind}: {e}", self.label));
                return;
            }
        };
        if self.previous == Some(current) {
            self.last_error = None;
            return;
        }
        match load(path, current) {
            Ok(next) => {
                // A module destructor may reenter its host; unload outside the lock.
                let old = {
                    let mut slot = active.write().unwrap_or_else(|e| e.into_inner());
                    std::mem::replace(&mut *slot, next)
                };
                drop(old);
                self.previous = Some(current);
                self.last_error = None;
            }
            Err(error) => self.report(format!("{} module ignored: {error}", self.label)),
        }
    }
    fn report(&mut self, error: String) {
        if self.last_error.as_deref() != Some(error.as_str()) {
            eprintln!("[angel] {error}");
            self.last_error = Some(error)
        }
    }
}
/// Configuration for one independently reloadable native ABI.
#[derive(Clone, Copy)]
pub(crate) struct ReloadConfig {
    pub(crate) label: &'static str,
    pub(crate) thread_name: &'static str,
    pub(crate) fallback: &'static str,
    pub(crate) worker_subject: &'static str,
    pub(crate) missing_is_unavailable: bool,
    pub(crate) byte_limit: u64,
    pub(crate) interval: Duration,
}

/// A hot call only clones the current generation; all file operations occur on
/// the reload thread. Generations can outlive a replacement through their Arcs.
pub(crate) enum ReloadHost<T> {
    Baseline(Arc<T>),
    Reloadable(Arc<RwLock<Arc<T>>>),
}
impl<T: Send + Sync + 'static> ReloadHost<T> {
    pub(crate) fn new(
        path: Option<PathBuf>,
        baseline: Arc<T>,
        loader: fn(&[u8]) -> Result<Arc<T>, String>,
        config: ReloadConfig,
    ) -> Self {
        let Some(path) = path else {
            return Self::Baseline(baseline);
        };
        if !path.is_absolute() {
            eprintln!(
                "[angel] {} module path must be absolute; using {}",
                config.label, config.fallback
            );
            return Self::Baseline(baseline);
        }
        let active = Arc::new(RwLock::new(Arc::clone(&baseline)));
        let shared = Arc::downgrade(&active);
        if let Err(error) = std::thread::Builder::new()
            .name(config.thread_name.into())
            .spawn(move || {
                let mut publisher = Publisher::new(config.label, config.missing_is_unavailable);
                loop {
                    let Some(active) = shared.upgrade() else {
                        break;
                    };
                    publisher.refresh(&path, &active, |p, fingerprint| {
                        read_stable_matching(p, config.byte_limit, fingerprint)
                            .and_then(|bytes| loader(&bytes))
                    });
                    drop(active);
                    std::thread::sleep(config.interval);
                }
            })
        {
            eprintln!(
                "[angel] {} reload worker failed: {error}",
                config.worker_subject
            );
            return Self::Baseline(baseline);
        }
        Self::Reloadable(active)
    }
    #[inline]
    pub(crate) fn current(&self) -> Arc<T> {
        match self {
            Self::Baseline(g) => Arc::clone(g),
            Self::Reloadable(lock) => Arc::clone(&lock.read().unwrap_or_else(|e| e.into_inner())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "native-module-directory-swap-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn capture_rejects_directory_swap_and_retries_current_generation() {
        let temp = TempDir::new();
        let root = &temp.0;
        std::fs::create_dir(root.join("a")).unwrap();
        std::fs::create_dir(root.join("b")).unwrap();
        std::fs::write(root.join("a/module"), b"A").unwrap();
        std::fs::write(root.join("b/module"), b"B").unwrap();
        let link = root.join("current");
        symlink("a", &link).unwrap();
        let path = link.join("module");
        let active = RwLock::new(Arc::new(Vec::new()));
        let mut publisher = Publisher::new("test", false);

        publisher.refresh(&path, &active, |p, expected| {
            // Both file reads see B, but the watched path is restored to A.
            std::fs::remove_file(&link).unwrap();
            symlink("b", &link).unwrap();
            let result = read_stable_matching(p, 1024, expected);
            std::fs::remove_file(&link).unwrap();
            symlink("a", &link).unwrap();
            result.map(Arc::new)
        });
        assert!(publisher.previous.is_none());
        assert!(active.read().unwrap().is_empty());

        publisher.refresh(&path, &active, |p, expected| {
            read_stable_matching(p, 1024, expected).map(Arc::new)
        });
        assert_eq!(active.read().unwrap().as_slice(), b"A");
        assert!(publisher.previous.is_some());
        publisher.refresh(&path, &active, |_, _| panic!("unchanged file reloaded"));
    }
}
