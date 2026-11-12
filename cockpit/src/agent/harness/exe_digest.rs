//! A memo of the running executable's SHA-256, so that starting the program
//! does not read and hash the whole image again when it has not changed.
//!
//! The run identity binds every request to the exact build, and the first
//! model request waits for that digest. Hashing a large image costs from
//! tens of milliseconds to a few hundred, on every start, for a value that
//! only changes when the binary does.
//!
//! The memo is a pure function of the image's bytes, keyed by what the kernel
//! says about the open file: device, inode, length, modification and change
//! times. A change to the content moves the change time, which user space
//! cannot set, so a stale entry cannot match. Two further guards keep the memo
//! as strict as hashing again:
//!
//! * an entry is stored only when the image has been quiet for [`SETTLE`]
//!   before it was read, so no rewrite can have landed inside one clock tick
//!   of the recorded times (the same rule a version-control index uses for
//!   its timestamp cache), and only when the times are unchanged after the
//!   read;
//! * the memo lives in a private directory the task sandbox cannot write to
//!   (`~/.angelX/cache`), is never followed through a symlink, and is read
//!   only when the directory and the entry belong to this user and are closed
//!   to everyone else. An entry that is malformed, foreign, or names another
//!   key is ignored and the image is hashed.
//!
//! A miss is a normal hash. Nothing here can fail a run; `ANGEL_EXE_DIGEST_CACHE=0`
//! turns the memo off.

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA: &str = "angel-exe-digest/v1";
/// An image younger than this when it is hashed may still be rewritten within
/// one clock tick of its recorded times, so its digest is not kept.
const SETTLE: Duration = Duration::from_secs(2);
/// Entries kept per user; images from older builds are dropped oldest first.
const KEEP: usize = 16;
const MAX_ENTRY_BYTES: u64 = 512;

/// What the kernel reports about one open file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    dev: u64,
    ino: u64,
    len: u64,
    nlink: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

impl Identity {
    pub(crate) fn of(file: &File) -> std::io::Result<Self> {
        let meta = file.metadata()?;
        Ok(Self {
            dev: meta.dev(),
            ino: meta.ino(),
            len: meta.len(),
            nlink: meta.nlink(),
            mtime: meta.mtime(),
            mtime_nsec: meta.mtime_nsec(),
            ctime: meta.ctime(),
            ctime_nsec: meta.ctime_nsec(),
        })
    }

    fn key(&self) -> String {
        format!(
            "{:x}-{:x}-{:x}-{}.{:09}-{}.{:09}",
            self.dev, self.ino, self.len, self.mtime, self.mtime_nsec, self.ctime, self.ctime_nsec
        )
    }

    /// Quiet for [`SETTLE`] at `now`, and still linked into the filesystem.
    fn settled(&self, now: SystemTime) -> bool {
        let newest = self
            .mtime
            .max(self.ctime)
            .try_into()
            .ok()
            .map(|secs: u64| UNIX_EPOCH + Duration::from_secs(secs));
        self.nlink > 0
            && newest.is_some_and(|newest| {
                now.duration_since(newest)
                    .is_ok_and(|quiet| quiet >= SETTLE)
            })
    }
}

/// Where the memo lives for this process, or `None` when it is off.
pub(crate) fn default_dir() -> Option<PathBuf> {
    if !crate::agent::harness::env_flag("ANGEL_EXE_DIGEST_CACHE", true) {
        return None;
    }
    if let Some(dir) = std::env::var_os("ANGEL_EXE_DIGEST_DIR") {
        let dir = PathBuf::from(dir);
        return dir.is_absolute().then_some(dir);
    }
    // Unit-test binaries are rebuilt constantly and must not write a user's
    // home; the tests that exercise the memo name a directory explicitly.
    if cfg!(test) {
        return None;
    }
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".angelX/cache/exe-digest"))
}

/// The digest of `file`: from the memo in `dir` when the image is the one a
/// previous run hashed, else `hash(file)`, which is then remembered.
pub(crate) fn digest_with(
    dir: Option<&Path>,
    file: &mut File,
    hash: impl FnOnce(&mut File) -> std::io::Result<String>,
) -> std::io::Result<String> {
    digest_at(dir, file, SystemTime::now(), hash)
}

/// [`digest_with`] with the clock named, so the quiet-period rule is testable.
fn digest_at(
    dir: Option<&Path>,
    file: &mut File,
    now: SystemTime,
    hash: impl FnOnce(&mut File) -> std::io::Result<String>,
) -> std::io::Result<String> {
    let Some(dir) = dir else {
        return hash(file);
    };
    let Ok(before) = Identity::of(file) else {
        return hash(file);
    };
    if let Some(hit) = lookup(dir, &before) {
        return Ok(hit);
    }
    let digest = hash(file)?;
    if Identity::of(file).is_ok_and(|after| after == before) && before.settled(now) {
        store(dir, &before, &digest);
    }
    Ok(digest)
}

fn effective_uid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

/// A plain directory, ours, closed to everyone else.
fn private_directory(dir: &Path) -> bool {
    std::fs::symlink_metadata(dir).is_ok_and(|meta| {
        meta.is_dir() && meta.uid() == effective_uid() && meta.mode() & 0o077 == 0
    })
}

fn valid_digest(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn lookup(dir: &Path, identity: &Identity) -> Option<String> {
    if !private_directory(dir) {
        return None;
    }
    let key = identity.key();
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(dir.join(&key))
        .ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file()
        || meta.uid() != effective_uid()
        || meta.mode() & 0o077 != 0
        || meta.len() > MAX_ENTRY_BYTES
    {
        return None;
    }
    let mut text = String::new();
    file.read_to_string(&mut text).ok()?;
    let mut fields = text.split_whitespace();
    let (schema, stored_key, digest) = (fields.next()?, fields.next()?, fields.next()?);
    (schema == SCHEMA && stored_key == key && fields.next().is_none() && valid_digest(digest))
        .then(|| digest.to_string())
}

fn store(dir: &Path, identity: &Identity, digest: &str) {
    let _ = try_store(dir, identity, digest);
}

fn try_store(dir: &Path, identity: &Identity, digest: &str) -> std::io::Result<()> {
    if !valid_digest(digest) {
        return Ok(());
    }
    if let Some(parent) = dir.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    if !private_directory(dir) {
        return Ok(());
    }
    let key = identity.key();
    let temp = dir.join(format!(".{key}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temp)?;
        file.write_all(format!("{SCHEMA} {key} {digest}\n").as_bytes())?;
        std::fs::rename(&temp, dir.join(&key))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
        return result;
    }
    prune(dir);
    Ok(())
}

/// Keep the newest [`KEEP`] entries. Best effort: a failure leaves extras.
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut kept: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            let modified = meta.modified().ok()?;
            meta.is_file().then(|| (modified, entry.path()))
        })
        .collect();
    if kept.len() <= KEEP {
        return;
    }
    kept.sort();
    let excess = kept.len() - KEEP;
    for (_, path) in kept.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/exe_digest__tests.rs"]
mod tests;
