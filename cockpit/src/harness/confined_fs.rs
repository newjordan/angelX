//! Race-resistant workspace filesystem operations.
//!
//! `safe_path` is useful validation, but a pathname can change after it is
//! checked. On Linux these helpers anchor the workspace with a directory file
//! descriptor and perform the actual operation with `openat2(RESOLVE_BENEATH |
//! RESOLVE_NO_MAGICLINKS)`. Older Linux kernels fall back to component-wise
//! `openat(O_NOFOLLOW)`, which is stricter about symlinks but remains confined.
//! Other platforms retain the canonical-path fallback used before this module.

use std::ffi::OsString;
#[cfg(not(target_os = "linux"))]
use std::fs::OpenOptions;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct ConfinedDirEntry {
    pub(crate) name: OsString,
    pub(crate) is_dir: bool,
}

/// Only the Linux `openat2` module resolves through this; other platforms go
/// straight to the canonical-path fallback.
#[cfg(target_os = "linux")]
fn normalized_relative(root: &Path, path: &Path) -> Result<PathBuf, String> {
    super::workspace_relative(root, path)
}

pub(crate) fn confined_read(root: &Path, rel: &Path) -> Result<Vec<u8>, String> {
    let mut file = imp::open_for_read(root, rel)
        .map_err(|e| format!("open {} for read: {e}", rel.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("read {}: {e}", rel.display()))?;
    Ok(bytes)
}

/// Read at most `max_bytes` from a regular file. Descriptor metadata rejects
/// oversized sparse files before allocation; `take(max + 1)` also bounds a file
/// that grows after the metadata check. `None` means the limit was exceeded.
pub(crate) fn confined_read_limited(
    root: &Path,
    rel: &Path,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, String> {
    let file = imp::open_for_read(root, rel)
        .map_err(|e| format!("open {} for read: {e}", rel.display()))?;
    read_limited_file(file, max_bytes)
}

/// Read a lexically selected regular file without following any path-component
/// symlink. Search policies use this when aliases must not reach excluded trees.
/// The trusted workspace root itself is opened once; each child is fd-relative.
pub(crate) fn confined_read_limited_no_symlinks(
    root: &Path,
    rel: &Path,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, String> {
    read_limited_file(confined_open_read_no_symlinks(root, rel)?, max_bytes)
}

/// Keep the opened source descriptor alive through asynchronous media decoding.
/// No path component may redirect the read after its workspace was selected.
pub(crate) fn confined_open_read_no_symlinks(
    root: &Path,
    rel: &Path,
) -> Result<std::fs::File, String> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        let (parent, name) = strict_parent_directory(root, rel, false)?;
        let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK;
        // SAFETY: parent owns a live directory descriptor; name is NUL terminated.
        let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if !file
            .metadata()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Err("source path must name a regular file".into());
        }
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        let _ = (root, rel);
        Err("strict source-bundle reads are unsupported on this platform".into())
    }
}

/// Create a new captured artifact without following any parent or leaf alias.
/// The content is published to the caller only after both file and parent sync.
pub(crate) fn confined_create_new_no_symlinks(
    root: &Path,
    rel: &Path,
    bytes: &[u8],
) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::fd::{AsRawFd, FromRawFd};
        let (parent, name) = strict_parent_directory(root, rel, true)?;
        let flags =
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW;
        // SAFETY: the directory and name stay alive; mode applies only to a new file.
        let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags, 0o600) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        parent.sync_all().map_err(|error| error.to_string())
    }
    #[cfg(not(unix))]
    {
        let _ = (root, rel, bytes);
        Err("strict artifact writes are unsupported on this platform".into())
    }
}

#[cfg(unix)]
fn strict_parent_directory(
    root: &Path,
    rel: &Path,
    create: bool,
) -> Result<(std::fs::File, std::ffi::CString), String> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let rel = super::workspace_relative(root, rel)?;
    let mut components = rel.components().collect::<Vec<_>>();
    let leaf = components
        .pop()
        .ok_or("source path must name a regular file")?;
    let name = CString::new(leaf.as_os_str().as_bytes()).map_err(|_| "source path contains NUL")?;
    let mut directory = std::fs::File::open(root).map_err(|error| error.to_string())?;
    for component in components {
        let part = CString::new(component.as_os_str().as_bytes())
            .map_err(|_| "source path contains NUL")?;
        if create {
            // SAFETY: directory is an owned fd and part is a valid C string.
            let result = unsafe { libc::mkdirat(directory.as_raw_fd(), part.as_ptr(), 0o700) };
            if result < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY;
        // SAFETY: each component is opened relative to the retained parent fd.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), part.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        directory = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    Ok((directory, name))
}

fn read_limited_file(file: std::fs::File, max_bytes: usize) -> Result<Option<Vec<u8>>, String> {
    let len = file
        .metadata()
        .map_err(|e| format!("stat opened file: {e}"))?
        .len();
    if len > max_bytes as u64 {
        return Ok(None);
    }
    let cap = max_bytes.saturating_add(1);
    let mut bytes = Vec::with_capacity((len as usize).min(cap));
    file.take(cap as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read opened file: {e}"))?;
    Ok((bytes.len() <= max_bytes).then_some(bytes))
}

#[derive(Debug)]
pub(crate) struct ConfinedReadPrefix {
    pub(crate) bytes: Vec<u8>,
    pub(crate) total_bytes: u64,
    pub(crate) truncated: bool,
    pub(crate) binary: bool,
}

/// Read a bounded prefix from a regular workspace file while retaining its
/// total size and binary status. Unlike [`confined_read_limited`], this is for
/// explicit preview surfaces that should remain useful when a text file is
/// larger than their context budget.
pub(crate) fn confined_read_prefix(
    root: &Path,
    rel: &Path,
    max_bytes: usize,
) -> Result<ConfinedReadPrefix, String> {
    let file = imp::open_for_read(root, rel)
        .map_err(|e| format!("open {} for read: {e}", rel.display()))?;
    let total_bytes = file
        .metadata()
        .map_err(|e| format!("stat {}: {e}", rel.display()))?
        .len();
    const BINARY_PROBE_BYTES: usize = 8192;
    let cap = max_bytes.saturating_add(1).max(BINARY_PROBE_BYTES);
    let capacity = usize::try_from(total_bytes).unwrap_or(cap).min(cap);
    let mut bytes = Vec::with_capacity(capacity);
    file.take(cap as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read {}: {e}", rel.display()))?;
    let truncated = bytes.len() > max_bytes || total_bytes > max_bytes as u64;
    let binary = bytes.iter().take(BINARY_PROBE_BYTES).any(|byte| *byte == 0);
    bytes.truncate(max_bytes);
    Ok(ConfinedReadPrefix {
        bytes,
        total_bytes,
        truncated,
        binary,
    })
}

/// One bounded, physical-line page from a regular file opened beneath `root`.
///
/// The caller supplies a one-based `offset` and a maximum number of complete
/// lines.  We deliberately scan with a fixed-size buffer instead of
/// `read_to_end`, `read_line`, or `BufRead::lines`: a hostile sparse file or a
/// single giant line must not make a normal agent read allocate in proportion to
/// the file's size.  Seeking to an arbitrary line would require an index, so an
/// explicit deep offset is capped by `max_scan_bytes` rather than turning this
/// small tool into an unbounded parser.
#[derive(Debug)]
pub(crate) struct ConfinedReadPage {
    pub(crate) bytes: Vec<u8>,
    pub(crate) total_bytes: u64,
    /// The next one-based line offset when another page is available.
    pub(crate) next_offset: Option<usize>,
    /// The page hit the byte cap before the line cap: how many file bytes lie
    /// after the returned prefix. The page self-corrects (prefix + truncation
    /// marker + next offset) instead of erroring into a blind retry.
    pub(crate) truncated_more_bytes: Option<u64>,
    /// The requested line lies beyond EOF.  An empty file at offset one is not
    /// considered past EOF so the legacy empty-string result remains intact.
    pub(crate) before_offset_eof: bool,
    pub(crate) binary: bool,
}

pub(crate) fn confined_read_page(
    root: &Path,
    rel: &Path,
    offset: usize,
    max_lines: usize,
    max_bytes: usize,
    max_scan_bytes: usize,
) -> Result<ConfinedReadPage, String> {
    debug_assert!(offset >= 1);
    debug_assert!(max_lines > 0);
    debug_assert!(max_bytes > 0);
    let mut file = imp::open_for_read(root, rel)
        .map_err(|e| format!("open {} for read: {e}", rel.display()))?;
    let total_bytes = file
        .metadata()
        .map_err(|e| format!("stat {}: {e}", rel.display()))?
        .len();

    // Preserve the existing binary-file posture without first materializing the
    // entire file. A short read is enough for the heuristic and the descriptor
    // is rewound before the actual page scan.
    const BINARY_PROBE_BYTES: usize = 8192;
    let mut probe = [0u8; BINARY_PROBE_BYTES];
    let probe_len = total_bytes.min(BINARY_PROBE_BYTES as u64) as usize;
    let read = file
        .read(&mut probe[..probe_len])
        .map_err(|e| format!("read {}: {e}", rel.display()))?;
    if probe[..read].contains(&0) {
        return Ok(ConfinedReadPage {
            bytes: Vec::new(),
            total_bytes,
            next_offset: None,
            truncated_more_bytes: None,
            before_offset_eof: false,
            binary: true,
        });
    }
    file.seek(std::io::SeekFrom::Start(0))
        .map_err(|e| format!("seek {}: {e}", rel.display()))?;

    let mut page = Vec::with_capacity(max_bytes);
    let mut buffer = [0u8; 8192];
    let mut current_line = 1usize;
    let mut at_offset = offset == 1;
    let mut scan_bytes = 0usize;
    let mut page_lines = 0usize;
    let mut processed_bytes = 0u64;
    let mut truncated_more_bytes: Option<u64> = None;

    'read: loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("read {}: {e}", rel.display()))?;
        if read == 0 {
            break;
        }
        for &byte in &buffer[..read] {
            processed_bytes = processed_bytes.saturating_add(1);
            if !at_offset {
                scan_bytes = scan_bytes.saturating_add(1);
                if scan_bytes > max_scan_bytes {
                    return Err(format!(
                        "read {}: line offset {offset} exceeds the bounded {}-byte scan; start from an earlier page",
                        rel.display(),
                        max_scan_bytes
                    ));
                }
                if byte == b'\n' {
                    current_line = current_line.saturating_add(1);
                    if current_line == offset {
                        at_offset = true;
                    }
                }
                continue;
            }

            if page.len() == max_bytes {
                // Byte cap reached before the line cap: serve the largest
                // prefix that fits instead of erroring. The current byte is
                // counted but not pushed, so the remainder runs from it
                // through EOF. A giant single line cannot advance by lines;
                // the next offset then names the line after it.
                truncated_more_bytes = Some(
                    total_bytes
                        .saturating_sub(processed_bytes)
                        .saturating_add(1)
                        .max(1),
                );
                break 'read;
            }
            page.push(byte);
            if byte == b'\n' {
                page_lines = page_lines.saturating_add(1);
                if page_lines == max_lines {
                    break 'read;
                }
            }
        }
    }

    let before_offset_eof = !at_offset;
    // A truncated page always implies more content, even when the unpushed
    // byte that proved it was the file's last (processed_bytes == total).
    let more_pages = truncated_more_bytes.is_some()
        || (!before_offset_eof && !page.is_empty() && processed_bytes < total_bytes);
    let next_offset = more_pages.then_some(offset.saturating_add(page_lines.max(1)));
    Ok(ConfinedReadPage {
        bytes: page,
        total_bytes,
        next_offset,
        truncated_more_bytes,
        before_offset_eof,
        binary: false,
    })
}

thread_local! {
    static HARDLINK_BREAKS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Attach a receipt only for replacements actually published on this thread.
/// Nested tool helpers may share the counter; failed validation never increments it.
pub(crate) fn hardlink_result(
    operation: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
    let before = HARDLINK_BREAKS.get();
    let result = operation();
    if HARDLINK_BREAKS.get() == before {
        return result;
    }
    let annotate = |text: String| format!("{text} · hardlink_broken: true");
    result.map(annotate).map_err(annotate)
}

/// Detach an existing multiply-linked target before a pathname-based writer.
/// The actual replacement still uses the descriptor-confined atomic primitive.
pub(crate) fn confined_break_hardlink(root: &Path, rel: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let file = match imp::open_for_read(root, rel) {
            Ok(file) => file,
            Err(error) => {
                // New patch destinations have no inode to detach. Other errors
                // remain the patch applier's validation responsibility.
                let _ = error;
                return Ok(());
            }
        };
        if file.metadata().map_err(|e| e.to_string())?.nlink() > 1 {
            confined_edit(root, rel, |bytes| Ok((bytes, ())))?;
        }
    }
    Ok(())
}

pub(crate) fn confined_write(root: &Path, rel: &Path, bytes: &[u8]) -> Result<(), String> {
    imp::write(root, rel, bytes, false).map_err(|e| format!("write {}: {e}", rel.display()))
}

/// Read, transform, and atomically replace one regular file. The edit handle
/// validates the target up front (regular file, no final-symlink indirection),
/// the callback runs before the first write — preserving multi-edit's
/// all-validation-before-mutation contract — and the rewrite lands as a
/// fsynced temp sibling renamed over the name, so an interruption mid-write
/// can never leave the file holding mixed old/new content.
pub(crate) fn confined_edit<T>(
    root: &Path,
    rel: &Path,
    edit: impl FnOnce(Vec<u8>) -> Result<(Vec<u8>, T), String>,
) -> Result<T, String> {
    let mut file = imp::open_for_edit(root, rel)
        .map_err(|e| format!("open {} for edit: {e}", rel.display()))?;
    let mut current = Vec::new();
    file.read_to_end(&mut current)
        .map_err(|e| format!("read {}: {e}", rel.display()))?;
    let (updated, result) = edit(current)?;
    drop(file);
    imp::replace_contents(root, rel, &updated)
        .map_err(|e| format!("write {}: {e}", rel.display()))?;
    Ok(result)
}

pub(crate) fn confined_create_new(root: &Path, rel: &Path, bytes: &[u8]) -> Result<(), String> {
    imp::write(root, rel, bytes, true).map_err(|e| format!("write {}: {e}", rel.display()))
}

pub(crate) fn confined_remove_file(root: &Path, rel: &Path) -> Result<(), String> {
    imp::remove_file(root, rel).map_err(|e| format!("remove {}: {e}", rel.display()))
}

pub(crate) fn confined_read_dir(root: &Path, rel: &Path) -> Result<Vec<ConfinedDirEntry>, String> {
    imp::read_dir(root, rel).map_err(|e| format!("list {}: {e}", rel.display()))
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
    use std::os::unix::ffi::OsStrExt;

    const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
    const RESOLVE_BENEATH: u64 = 0x08;

    #[repr(C)]
    struct OpenHow {
        flags: u64,
        mode: u64,
        resolve: u64,
    }

    struct Root {
        fd: OwnedFd,
        path: PathBuf,
    }

    impl Root {
        fn open(path: &Path) -> io::Result<Self> {
            let root_path = path.to_path_buf();
            let path = cstring(path)?;
            // The workspace root is trusted configuration. Resolving it once
            // here gives every subsequent operation a stable directory anchor.
            let fd = unsafe {
                libc::open(
                    path.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(Self {
                    fd: unsafe { OwnedFd::from_raw_fd(fd) },
                    path: root_path,
                })
            }
        }

        /// Resolve existing in-workspace symlinks to a root-relative spelling.
        /// The final `openat2` is still the security boundary: if any component
        /// changes after this convenience resolution, an outbound replacement
        /// is rejected rather than followed.
        fn resolve_full(&self, rel: &Path) -> Result<PathBuf, String> {
            let rel = normalized_relative(&self.path, rel)?;
            let fd_root = PathBuf::from(format!("/proc/self/fd/{}", self.fd.as_raw_fd()));
            let canonical_root = match fd_root.canonicalize() {
                Ok(root) => root,
                // `/proc` is not guaranteed to be mounted in a minimal
                // container. The direct relative spelling remains secure at
                // `openat2`; only absolute in-root symlink compatibility is
                // unavailable in that environment.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                    ) =>
                {
                    return Ok(rel);
                }
                Err(e) => {
                    return Err(format!("resolve workspace {}: {e}", fd_root.display()));
                }
            };
            let mut cursor = fd_root.join(&rel);
            let mut missing = Vec::<OsString>::new();
            let resolved = loop {
                match std::fs::symlink_metadata(&cursor) {
                    Ok(_) => {
                        let mut p = cursor
                            .canonicalize()
                            .map_err(|e| format!("resolve path {}: {e}", cursor.display()))?;
                        for component in missing.iter().rev() {
                            p.push(component);
                        }
                        break p;
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        let name = cursor.file_name().ok_or_else(|| {
                            format!("cannot resolve path ancestor for {}", rel.display())
                        })?;
                        missing.push(name.to_os_string());
                        cursor = cursor
                            .parent()
                            .ok_or_else(|| {
                                format!("cannot resolve path ancestor for {}", rel.display())
                            })?
                            .to_path_buf();
                    }
                    Err(e) => return Err(format!("resolve path {}: {e}", cursor.display())),
                }
            };
            resolved
                .strip_prefix(&canonical_root)
                .map(Path::to_path_buf)
                .map_err(|_| format!("path resolves outside the workspace: {:?}", rel))
        }

        /// Resolve only the parent directory, retaining the final component.
        /// Mutating operations combine this with `O_NOFOLLOW`, so a final
        /// symlink is never used as a write/delete indirection.
        fn resolve_parent(&self, rel: &Path) -> Result<PathBuf, String> {
            let rel = normalized_relative(&self.path, rel)?;
            let name = rel
                .file_name()
                .ok_or_else(|| "path names the workspace directory".to_string())?
                .to_os_string();
            let parent = rel.parent().unwrap_or_else(|| Path::new(""));
            let mut resolved = self.resolve_full(parent)?;
            resolved.push(name);
            Ok(resolved)
        }

        fn open_resolved(&self, rel: &Path, flags: i32, mode: u32) -> io::Result<OwnedFd> {
            match openat2(self.fd.as_raw_fd(), rel, flags, mode) {
                Err(e) if e.raw_os_error() == Some(libc::ENOSYS) => {
                    openat_nofollow(self.fd.as_raw_fd(), rel, flags, mode)
                }
                result => result,
            }
        }

        fn ensure_parents(&self, rel: &Path) -> io::Result<()> {
            let components: Vec<OsString> = rel
                .components()
                .map(|c| c.as_os_str().to_os_string())
                .collect();
            let mut prefix = PathBuf::new();
            for component in components.iter().take(components.len().saturating_sub(1)) {
                let parent = prefix.clone();
                prefix.push(component);
                match self.open_resolved(
                    &prefix,
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                    0,
                ) {
                    Ok(_) => continue,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
                let parent_fd = self.open_resolved(
                    &parent,
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                    0,
                )?;
                let name = cstring(Path::new(component))?;
                let rc = unsafe { libc::mkdirat(parent_fd.as_raw_fd(), name.as_ptr(), 0o777) };
                if rc != 0 {
                    let e = io::Error::last_os_error();
                    if e.kind() != io::ErrorKind::AlreadyExists {
                        return Err(e);
                    }
                }
                // Verify that a racing creator supplied an in-workspace
                // directory, not a symlink or a non-directory entry.
                self.open_resolved(
                    &prefix,
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                    0,
                )?;
            }
            Ok(())
        }
    }

    fn cstring(path: &Path) -> io::Result<CString> {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
    }

    fn duplicate(fd: RawFd) -> io::Result<OwnedFd> {
        let new_fd = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
        if new_fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(new_fd) })
        }
    }

    fn require_regular(fd: &OwnedFd) -> io::Result<()> {
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "workspace file operation requires a regular file",
            ));
        }
        Ok(())
    }

    fn openat2(dirfd: RawFd, rel: &Path, flags: i32, mode: u32) -> io::Result<OwnedFd> {
        if rel.as_os_str().is_empty() {
            return duplicate(dirfd);
        }
        let rel = cstring(rel)?;
        let how = OpenHow {
            flags: flags as u64,
            mode: mode as u64,
            resolve: RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS,
        };
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                dirfd,
                rel.as_ptr(),
                &how as *const OpenHow,
                std::mem::size_of::<OpenHow>(),
            ) as i32
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
    }

    /// Secure fallback for pre-5.6 kernels. It rejects symlinks in the resolved
    /// spelling rather than following them, but never reverts to a pathname
    /// check followed by an ambient open.
    fn openat_nofollow(root_fd: RawFd, rel: &Path, flags: i32, mode: u32) -> io::Result<OwnedFd> {
        let components: Vec<OsString> = rel
            .components()
            .map(|c| c.as_os_str().to_os_string())
            .collect();
        if components.is_empty() {
            return duplicate(root_fd);
        }
        let mut dir = duplicate(root_fd)?;
        for component in &components[..components.len() - 1] {
            let name = cstring(Path::new(component))?;
            let fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            dir = unsafe { OwnedFd::from_raw_fd(fd) };
        }
        let name = cstring(Path::new(components.last().unwrap()))?;
        let fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                mode as libc::mode_t,
            )
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
    }

    pub(super) fn open_for_read(root: &Path, rel: &Path) -> Result<std::fs::File, String> {
        let root = Root::open(root).map_err(|e| format!("open workspace: {e}"))?;
        let rel = root.resolve_full(rel)?;
        let fd = root
            .open_resolved(&rel, libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NONBLOCK, 0)
            .map_err(|e| e.to_string())?;
        require_regular(&fd).map_err(|e| e.to_string())?;
        Ok(std::fs::File::from(fd))
    }

    pub(super) fn write(
        root: &Path,
        rel: &Path,
        bytes: &[u8],
        create_new: bool,
    ) -> Result<(), String> {
        let root = Root::open(root).map_err(|e| format!("open workspace: {e}"))?;
        let rel = root.resolve_parent(rel)?;
        root.ensure_parents(&rel).map_err(|e| e.to_string())?;
        if !create_new {
            return replace_file(&root, &rel, bytes);
        }
        let flags = libc::O_WRONLY
            | libc::O_CREAT
            | libc::O_EXCL
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK;
        let fd = root
            .open_resolved(&rel, flags, 0o666)
            .map_err(|e| e.to_string())?;
        require_regular(&fd).map_err(|e| e.to_string())?;
        let mut file = std::fs::File::from(fd);
        file.write_all(bytes).map_err(|e| e.to_string())
    }

    /// Crash-atomic replacement: write to an `O_EXCL` temp sibling inside the
    /// already-resolved parent directory, fsync, then `renameat` over the final
    /// name. An interruption mid-write can strand a dot-temp but can never
    /// leave the target holding mixed old/new content — the failure the plain
    /// truncate-then-write path produced. All directory handles come from the
    /// `RESOLVE_BENEATH` chain, so nothing re-resolves ambiently.
    fn replace_file(root: &Root, rel: &Path, bytes: &[u8]) -> Result<(), String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);

        let name = rel
            .file_name()
            .ok_or_else(|| "path names the workspace directory".to_string())?;
        let parent = rel.parent().unwrap_or_else(|| Path::new(""));
        let parent_fd = root
            .open_resolved(
                parent,
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                0,
            )
            .map_err(|e| e.to_string())?;
        // Capture the existing entry's nature and mode before mutating: a
        // final-symlink indirection stays rejected (parity with O_NOFOLLOW on
        // the direct path), and a replaced executable stays executable.
        let name_c = cstring(Path::new(name)).map_err(|e| e.to_string())?;
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        let existing_mode = if unsafe {
            libc::fstatat(
                parent_fd.as_raw_fd(),
                name_c.as_ptr(),
                &mut stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } == 0
        {
            if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
                return Err("workspace file operation requires a regular file".to_string());
            }
            Some(stat.st_mode & 0o7777)
        } else {
            None
        };
        let (tmp_c, tmp_fd) = loop {
            let tmp_name = format!(
                ".{}.angel-tmp.{}.{}",
                name.to_string_lossy(),
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed),
            );
            let tmp_c = cstring(Path::new(&tmp_name)).map_err(|e| e.to_string())?;
            let fd = unsafe {
                libc::openat(
                    parent_fd.as_raw_fd(),
                    tmp_c.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_CLOEXEC
                        | libc::O_NOFOLLOW,
                    0o666 as libc::mode_t,
                )
            };
            if fd >= 0 {
                break (tmp_c, unsafe { OwnedFd::from_raw_fd(fd) });
            }
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::AlreadyExists {
                return Err(e.to_string());
            }
        };
        let discard_tmp = || {
            unsafe { libc::unlinkat(parent_fd.as_raw_fd(), tmp_c.as_ptr(), 0) };
        };
        if let Some(mode) = existing_mode
            && unsafe { libc::fchmod(tmp_fd.as_raw_fd(), mode as libc::mode_t) } != 0
        {
            let e = io::Error::last_os_error();
            discard_tmp();
            return Err(e.to_string());
        }
        let mut tmp = std::fs::File::from(tmp_fd);
        if let Err(e) = tmp.write_all(bytes).and_then(|()| tmp.sync_all()) {
            discard_tmp();
            return Err(e.to_string());
        }
        let rc = unsafe {
            libc::renameat(
                parent_fd.as_raw_fd(),
                tmp_c.as_ptr(),
                parent_fd.as_raw_fd(),
                name_c.as_ptr(),
            )
        };
        if rc != 0 {
            let e = io::Error::last_os_error();
            discard_tmp();
            return Err(e.to_string());
        }
        if existing_mode.is_some() && stat.st_nlink > 1 {
            super::HARDLINK_BREAKS.set(super::HARDLINK_BREAKS.get().wrapping_add(1));
        }
        Ok(())
    }

    pub(super) fn replace_contents(root: &Path, rel: &Path, bytes: &[u8]) -> Result<(), String> {
        let root = Root::open(root).map_err(|e| format!("open workspace: {e}"))?;
        let rel = root.resolve_parent(rel)?;
        replace_file(&root, &rel, bytes)
    }

    pub(super) fn open_for_edit(root: &Path, rel: &Path) -> Result<std::fs::File, String> {
        let root = Root::open(root).map_err(|e| format!("open workspace: {e}"))?;
        let rel = root.resolve_parent(rel)?;
        let fd = root
            .open_resolved(
                &rel,
                libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
                0,
            )
            .map_err(|e| e.to_string())?;
        require_regular(&fd).map_err(|e| e.to_string())?;
        Ok(std::fs::File::from(fd))
    }

    pub(super) fn remove_file(root: &Path, rel: &Path) -> Result<(), String> {
        let root = Root::open(root).map_err(|e| format!("open workspace: {e}"))?;
        let rel = root.resolve_parent(rel)?;
        let name = rel
            .file_name()
            .ok_or_else(|| "path names the workspace directory".to_string())?;
        let parent = rel.parent().unwrap_or_else(|| Path::new(""));
        let parent_fd = root
            .open_resolved(
                parent,
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                0,
            )
            .map_err(|e| e.to_string())?;
        let name = cstring(Path::new(name)).map_err(|e| e.to_string())?;
        let rc = unsafe { libc::unlinkat(parent_fd.as_raw_fd(), name.as_ptr(), 0) };
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error().to_string())
        }
    }

    struct Dir(*mut libc::DIR);
    impl Drop for Dir {
        fn drop(&mut self) {
            unsafe {
                libc::closedir(self.0);
            }
        }
    }

    pub(super) fn read_dir(root: &Path, rel: &Path) -> Result<Vec<ConfinedDirEntry>, String> {
        let root = Root::open(root).map_err(|e| format!("open workspace: {e}"))?;
        let rel = root.resolve_full(rel)?;
        let fd = root
            .open_resolved(
                &rel,
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                0,
            )
            .map_err(|e| e.to_string())?;
        let raw_fd = fd.into_raw_fd();
        let dir_ptr = unsafe { libc::fdopendir(raw_fd) };
        if dir_ptr.is_null() {
            unsafe {
                libc::close(raw_fd);
            }
            return Err(io::Error::last_os_error().to_string());
        }
        let dir = Dir(dir_ptr);
        let mut entries = Vec::new();
        loop {
            unsafe {
                *libc::__errno_location() = 0;
            }
            let entry = unsafe { libc::readdir(dir.0) };
            if entry.is_null() {
                let errno = unsafe { *libc::__errno_location() };
                if errno != 0 {
                    return Err(io::Error::from_raw_os_error(errno).to_string());
                }
                break;
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            let rc = unsafe {
                libc::fstatat(
                    libc::dirfd(dir.0),
                    name.as_ptr(),
                    &mut stat,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if rc != 0 {
                continue; // entry raced away; omit it from this snapshot
            }
            entries.push(ConfinedDirEntry {
                name: std::ffi::OsStr::from_bytes(name.to_bytes()).to_os_string(),
                is_dir: stat.st_mode & libc::S_IFMT == libc::S_IFDIR,
            });
        }
        Ok(entries)
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::*;

    fn path(root: &Path, rel: &Path) -> Result<PathBuf, String> {
        let rel = rel
            .to_str()
            .ok_or_else(|| format!("path is not valid UTF-8: {}", rel.display()))?;
        super::super::safe_path(root, rel)
    }

    pub(super) fn open_for_read(root: &Path, rel: &Path) -> Result<std::fs::File, String> {
        std::fs::File::open(path(root, rel)?).map_err(|e| e.to_string())
    }

    pub(super) fn write(
        root: &Path,
        rel: &Path,
        bytes: &[u8],
        create_new: bool,
    ) -> Result<(), String> {
        let path = path(root, rel)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if !create_new {
            return replace_at(&path, bytes);
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())
    }

    /// Crash-atomic replacement mirroring the Linux path: fsynced temp sibling
    /// renamed over the target, preserving an existing target's permissions.
    fn replace_at(path: &Path, bytes: &[u8]) -> Result<(), String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);

        let name = path
            .file_name()
            .ok_or_else(|| "path names the workspace directory".to_string())?;
        let permissions = match std::fs::symlink_metadata(path) {
            Ok(meta) if !meta.is_file() => {
                return Err("workspace file operation requires a regular file".to_string());
            }
            Ok(meta) => Some(meta.permissions()),
            Err(_) => None,
        };
        let tmp_path = path.with_file_name(format!(
            ".{}.angel-tmp.{}.{}",
            name.to_string_lossy(),
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed),
        ));
        let written = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .and_then(|mut tmp| {
                if let Some(permissions) = permissions {
                    tmp.set_permissions(permissions)?;
                }
                tmp.write_all(bytes)?;
                tmp.sync_all()
            })
            .and_then(|()| std::fs::rename(&tmp_path, path));
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp_path);
        }
        written.map_err(|e| e.to_string())
    }

    pub(super) fn replace_contents(root: &Path, rel: &Path, bytes: &[u8]) -> Result<(), String> {
        replace_at(&path(root, rel)?, bytes)
    }

    pub(super) fn open_for_edit(root: &Path, rel: &Path) -> Result<std::fs::File, String> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path(root, rel)?)
            .map_err(|e| e.to_string())
    }

    pub(super) fn remove_file(root: &Path, rel: &Path) -> Result<(), String> {
        std::fs::remove_file(path(root, rel)?).map_err(|e| e.to_string())
    }

    pub(super) fn read_dir(root: &Path, rel: &Path) -> Result<Vec<ConfinedDirEntry>, String> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path(root, rel)?).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            entries.push(ConfinedDirEntry {
                name: entry.file_name(),
                is_dir: entry.file_type().map(|t| t.is_dir()).unwrap_or(false),
            });
        }
        Ok(entries)
    }
}
