//! A change hint for module sources.
//!
//! The loader compares file metadata for every module on every scan. That is
//! cheap for one module and not for fifty polled ten times a second. inotify
//! says "something under this module changed"; it never replaces the metadata
//! comparison, it only lets a module whose cached capture matched the disk at
//! its last full check skip that comparison until an event, an unreadable
//! queue, or [`RECHECK`] says to look again. Without inotify, with a watch
//! that cannot be placed, or after any doubt, the module is compared on every
//! scan exactly as before.
//!
//! The comparison stays the authority: an event only schedules it.

#[cfg(target_os = "linux")]
mod imp {
    use std::collections::{BTreeMap, HashMap, HashSet};
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    /// Longest a module is trusted without a full comparison, for sources
    /// inotify cannot see (some network and overlay mounts).
    const RECHECK: Duration = Duration::from_secs(5);
    /// Directories watched under one module, and how deep.
    const MAX_DIRS: usize = 128;
    const MAX_DEPTH: usize = 8;

    const MASK: u32 = libc::IN_CREATE
        | libc::IN_DELETE
        | libc::IN_MODIFY
        | libc::IN_ATTRIB
        | libc::IN_CLOSE_WRITE
        | libc::IN_MOVED_FROM
        | libc::IN_MOVED_TO
        | libc::IN_DELETE_SELF
        | libc::IN_MOVE_SELF
        | libc::IN_ONLYDIR
        | libc::IN_DONT_FOLLOW;

    pub(crate) struct Watch {
        fd: libc::c_int,
        /// Watch descriptor to the module it sits under.
        owners: HashMap<libc::c_int, String>,
        /// Modules whose cache matched the disk at the stored time, with every
        /// directory watched and no event seen since.
        clean: BTreeMap<String, Instant>,
        /// Modules a watch could not be placed for: never trusted.
        incomplete: HashSet<String>,
        /// When each module's current full check began.
        armed: BTreeMap<String, Instant>,
        buffer: Vec<u8>,
    }

    impl Watch {
        pub(crate) fn new() -> Option<Self> {
            // SAFETY: plain syscall; the descriptor is owned by this value.
            let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
            (fd >= 0).then(|| Self {
                fd,
                owners: HashMap::new(),
                clean: BTreeMap::new(),
                incomplete: HashSet::new(),
                armed: BTreeMap::new(),
                buffer: vec![0; 16 * 1024],
            })
        }

        /// Read every queued event. Anything under a module makes it dirty; a
        /// lost queue makes every module dirty.
        pub(crate) fn drain(&mut self) {
            loop {
                // SAFETY: the buffer is owned and the length passed is its own.
                let read = unsafe {
                    libc::read(self.fd, self.buffer.as_mut_ptr().cast(), self.buffer.len())
                };
                if read <= 0 {
                    // EAGAIN: drained. Any other error: the next full check
                    // after RECHECK still holds, and the queue is not trusted
                    // past it.
                    if read < 0
                        && std::io::Error::last_os_error().kind() != std::io::ErrorKind::WouldBlock
                    {
                        self.clean.clear();
                    }
                    return;
                }
                let read = read as usize;
                let mut at = 0;
                while at + 16 <= read {
                    let word = |from: usize| -> [u8; 4] {
                        self.buffer[at + from..at + from + 4]
                            .try_into()
                            .unwrap_or([0; 4])
                    };
                    let wd = i32::from_ne_bytes(word(0));
                    let mask = u32::from_ne_bytes(word(4));
                    let len = u32::from_ne_bytes(word(12)) as usize;
                    at += 16 + len;
                    if mask & libc::IN_Q_OVERFLOW != 0 {
                        self.clean.clear();
                        continue;
                    }
                    if let Some(id) = self.owners.get(&wd).cloned() {
                        self.clean.remove(&id);
                        if mask & libc::IN_IGNORED != 0 {
                            self.owners.remove(&wd);
                        }
                    }
                }
            }
        }

        /// Whether `id` matched the disk at its last full check and nothing
        /// has moved under it since.
        pub(crate) fn trusted(&self, id: &str) -> bool {
            self.clean.get(id).is_some_and(|at| at.elapsed() < RECHECK)
        }

        /// A full comparison of `id` is about to run: watch its directories
        /// first, so an edit during or after the comparison raises an event.
        pub(crate) fn arm(&mut self, id: &str, root: &Path) {
            self.clean.remove(id);
            self.armed.insert(id.to_owned(), Instant::now());
            let mut dirs = 0;
            if self.watch_tree(id, root, 0, &mut dirs).is_err() {
                self.incomplete.insert(id.to_owned());
            } else {
                self.incomplete.remove(id);
            }
        }

        /// The comparison ran: `fresh` says the cache matched the disk. Only a
        /// match with every directory watched earns trust.
        pub(crate) fn confirm(&mut self, id: &str, fresh: bool) {
            if let Some(at) = self.armed.remove(id)
                && fresh
                && !self.incomplete.contains(id)
            {
                self.clean.insert(id.to_owned(), at);
            }
        }

        fn watch_tree(
            &mut self,
            id: &str,
            dir: &Path,
            depth: usize,
            dirs: &mut usize,
        ) -> Result<(), ()> {
            if depth > MAX_DEPTH || *dirs >= MAX_DIRS {
                return Err(());
            }
            *dirs += 1;
            let path = CString::new(dir.as_os_str().as_bytes()).map_err(|_| ())?;
            // SAFETY: `path` is a valid NUL-terminated string for the call.
            let wd = unsafe { libc::inotify_add_watch(self.fd, path.as_ptr(), MASK) };
            if wd < 0 {
                return Err(());
            }
            self.owners.insert(wd, id.to_owned());
            for entry in std::fs::read_dir(dir).map_err(|_| ())? {
                let entry = entry.map_err(|_| ())?;
                if entry.file_type().map_err(|_| ())?.is_dir() {
                    self.watch_tree(id, &entry.path(), depth + 1, dirs)?;
                }
            }
            Ok(())
        }
    }

    impl Drop for Watch {
        fn drop(&mut self) {
            // SAFETY: the descriptor is owned and closed once.
            unsafe { libc::close(self.fd) };
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) use imp::Watch;

/// Where inotify does not exist the loader compares every scan.
#[cfg(not(target_os = "linux"))]
pub(crate) struct Watch;

#[cfg(not(target_os = "linux"))]
impl Watch {
    pub(crate) fn new() -> Option<Self> {
        None
    }
    pub(crate) fn drain(&mut self) {}
    pub(crate) fn trusted(&self, _id: &str) -> bool {
        false
    }
    pub(crate) fn arm(&mut self, _id: &str, _root: &std::path::Path) {}
    pub(crate) fn confirm(&mut self, _id: &str, _fresh: bool) {}
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::Watch;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("angel-watch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("index.js"), "1").unwrap();
        dir
    }

    #[test]
    fn a_matching_module_is_trusted_until_something_under_it_changes() {
        let dir = scratch("trust");
        let mut watch = Watch::new().expect("inotify");
        assert!(!watch.trusted("m"), "never compared, never trusted");
        watch.arm("m", &dir);
        watch.confirm("m", true);
        watch.drain();
        assert!(watch.trusted("m"));
        std::fs::write(dir.join("index.js"), "22").unwrap();
        watch.drain();
        assert!(!watch.trusted("m"), "an edit schedules the comparison");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_nested_edit_or_a_new_file_also_schedules_the_comparison() {
        let dir = scratch("nested");
        let mut watch = Watch::new().expect("inotify");
        watch.arm("m", &dir);
        watch.confirm("m", true);
        std::fs::write(dir.join("nested").join("dep.js"), "x").unwrap();
        watch.drain();
        assert!(!watch.trusted("m"));
        watch.arm("m", &dir);
        watch.confirm("m", true);
        watch.drain();
        assert!(watch.trusted("m"));
        std::fs::create_dir(dir.join("nested").join("deeper")).unwrap();
        watch.drain();
        assert!(!watch.trusted("m"));
        watch.arm("m", &dir);
        watch.confirm("m", true);
        std::fs::write(dir.join("nested").join("deeper").join("late.js"), "y").unwrap();
        watch.drain();
        assert!(
            !watch.trusted("m"),
            "a directory made after the first arm is watched too"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_cache_is_never_trusted() {
        let dir = scratch("stale");
        let mut watch = Watch::new().expect("inotify");
        watch.arm("m", &dir);
        watch.confirm("m", false);
        watch.drain();
        assert!(
            !watch.trusted("m"),
            "a cache that differs from the disk keeps being compared"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_module_that_cannot_be_watched_is_always_compared() {
        let mut watch = Watch::new().expect("inotify");
        let missing =
            std::env::temp_dir().join(format!("angel-watch-missing-{}", std::process::id()));
        watch.arm("m", &missing);
        watch.confirm("m", true);
        assert!(!watch.trusted("m"));
    }
}
