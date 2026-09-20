//! Descriptor-based private store I/O. Payload serialization stays with callers.
use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path};

pub(crate) struct PrivateDirectory(File);

fn component(name: &OsStr) -> io::Result<CString> {
    if name.is_empty() || name.as_bytes().contains(&b'/') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid store file name",
        ));
    }
    CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in store path"))
}

impl PrivateDirectory {
    /// Traverse every component without following symlinks. Only newly created
    /// directories get 0700; existing parent directories are never chmodded.
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        Self::walk(path, true)
    }

    pub(crate) fn open_existing(path: &Path) -> io::Result<Self> {
        Self::walk(path, false)
    }

    fn walk(path: &Path, create: bool) -> io::Result<Self> {
        let path = resolve_system_prefix(path);
        let path = path.as_ref();
        let mut directory = Self(File::open(if path.is_absolute() { "/" } else { "." })?);
        for entry in path.components() {
            let name = match entry {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => name,
                Component::ParentDir => OsStr::new(".."),
                Component::Prefix(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "unsupported store path prefix",
                    ));
                }
            };
            let next = match directory.open_at(name, libc::O_RDONLY | libc::O_DIRECTORY) {
                Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
                    let name_c = component(name)?;
                    // SAFETY: the descriptor and NUL-terminated name stay live.
                    if unsafe { libc::mkdirat(directory.0.as_raw_fd(), name_c.as_ptr(), 0o700) } < 0
                    {
                        let error = io::Error::last_os_error();
                        if error.kind() != io::ErrorKind::AlreadyExists {
                            return Err(error);
                        }
                    }
                    directory.open_at(name, libc::O_RDONLY | libc::O_DIRECTORY)?
                }
                result => result?,
            };
            directory = Self(next);
        }
        if directory.0.metadata()?.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "private store directory has another owner",
            ));
        }
        Ok(directory)
    }

    /// Only for a directory the caller explicitly defines as a private store.
    /// Ordinary opens above preserve existing parent permissions.
    pub(crate) fn secure_owner_only(&self) -> io::Result<()> {
        if self.0.metadata()?.mode() & 0o077 != 0 {
            self.0
                .set_permissions(std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    fn open_at(&self, name: &OsStr, flags: i32) -> io::Result<File> {
        let name = component(name)?;
        // SAFETY: openat receives a live directory and C string. Ownership of
        // the returned descriptor is transferred once into File.
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }

    fn checked(&self, name: &OsStr, flags: i32) -> io::Result<File> {
        let file = self.open_at(name, flags)?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.nlink() != 1
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "private store entry is not a singly-linked owner file",
            ));
        }
        // An existing owner file can migrate safely. This descriptor is checked
        // before fchmod, and no caller can write bytes before this returns.
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        self.matches(name, &file)?;
        Ok(file)
    }

    fn matches(&self, name: &OsStr, file: &File) -> io::Result<()> {
        let name = component(name)?;
        let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: fstatat initializes stat on success and does not follow links.
        if unsafe {
            libc::fstatat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                metadata.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        let named = unsafe { metadata.assume_init() };
        let opened = file.metadata()?;
        // dev_t is signed i32 on Apple and u64 on Linux; MetadataExt uses u64.
        #[allow(clippy::unnecessary_cast)]
        let same_identity =
            (named.st_dev as u64, named.st_ino as u64) == (opened.dev(), opened.ino());
        if !same_identity
            || named.st_mode & libc::S_IFMT != libc::S_IFREG
            || named.st_nlink != 1
            || named.st_uid != unsafe { libc::geteuid() }
            || named.st_mode & 0o777 != 0o600
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "private store path no longer names its checked file",
            ));
        }
        Ok(())
    }

    pub(crate) fn append(&self, name: &OsStr) -> io::Result<File> {
        self.checked(name, libc::O_WRONLY | libc::O_APPEND | libc::O_CREAT)
    }

    pub(crate) fn lock_file(&self, name: &OsStr) -> io::Result<File> {
        self.checked(name, libc::O_RDWR | libc::O_CREAT)
    }

    pub(crate) fn create_new(&self, name: &OsStr) -> io::Result<File> {
        self.checked(name, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)
    }

    pub(crate) fn existing(&self, name: &OsStr) -> io::Result<Option<File>> {
        match self.checked(name, libc::O_RDONLY) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            result => result.map(Some),
        }
    }

    pub(crate) fn remove_owned(&self, name: &OsStr, file: &File) -> io::Result<()> {
        self.matches(name, file)?;
        let name = component(name)?;
        // SAFETY: unlink only the name that still matches the caller's held FD.
        if unsafe { libc::unlinkat(self.0.as_raw_fd(), name.as_ptr(), 0) } < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(crate) fn publish(
        &self,
        temporary: &OsStr,
        file: &File,
        name: &OsStr,
        previous: Option<&File>,
    ) -> io::Result<()> {
        self.matches(temporary, file)?;
        if let Some(previous) = previous {
            self.matches(name, previous)?;
        } else if self.existing(name)?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "private store destination appeared while staging",
            ));
        }
        // renameat moves the entry and never follows a substituted symlink.
        self.rename(temporary, name)
    }

    pub(crate) fn sync(&self) -> io::Result<()> {
        self.0.sync_all()
    }

    /// Enumerate the held directory, never its original mutable pathname.
    pub(crate) fn names(&self) -> io::Result<Vec<OsString>> {
        struct Stream(*mut libc::DIR);
        impl Drop for Stream {
            fn drop(&mut self) {
                unsafe {
                    libc::closedir(self.0);
                }
            }
        }
        // Opening "." relative to the held FD gives each enumeration an
        // independent offset; dup() would share the caller's directory offset.
        let fd = self
            .open_at(OsStr::new("."), libc::O_RDONLY | libc::O_DIRECTORY)?
            .into_raw_fd();
        let raw = unsafe { libc::fdopendir(fd) };
        if raw.is_null() {
            let error = io::Error::last_os_error();
            unsafe {
                libc::close(fd);
            }
            return Err(error);
        }
        let stream = Stream(raw);
        let mut names = Vec::new();
        loop {
            // readdir uses errno to distinguish end-of-directory from error.
            #[cfg(any(target_os = "linux", target_os = "dragonfly"))]
            unsafe {
                *libc::__errno_location() = 0;
            }
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
            unsafe {
                *libc::__error() = 0;
            }
            #[cfg(any(target_os = "android", target_os = "netbsd", target_os = "openbsd"))]
            unsafe {
                *libc::__errno() = 0;
            }
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                let error = io::Error::last_os_error();
                if error.raw_os_error().unwrap_or(0) != 0 {
                    return Err(error);
                }
                break;
            }
            // SAFETY: d_name is terminated by readdir, and copied before the
            // next call can reuse the stream's buffer.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name != b"." && name != b".." {
                names.push(OsString::from_vec(name.to_vec()));
            }
        }
        names.sort();
        Ok(names)
    }

    fn rename(&self, source: &OsStr, target: &OsStr) -> io::Result<()> {
        let source = component(source)?;
        let target = component(target)?;
        // SAFETY: both names are relative to the same pinned directory.
        if unsafe {
            libc::renameat(
                self.0.as_raw_fd(),
                source.as_ptr(),
                self.0.as_raw_fd(),
                target.as_ptr(),
            )
        } < 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Caller retains its stable companion flock across rotation and append.
    #[cfg(test)]
    pub(crate) fn rotate_if(&self, name: &OsStr, target: &OsStr, bytes: u64) -> io::Result<()> {
        let Some(file) = self.existing(name)? else {
            return Ok(());
        };
        if file.metadata()?.len() < bytes {
            return Ok(());
        }
        let target_file = self.existing(target)?;
        self.matches(name, &file)?;
        if let Some(target_file) = target_file.as_ref() {
            self.matches(target, target_file)?;
        }
        self.rename(name, target)?;
        self.matches(target, &file)
    }

    pub(crate) fn replace(&self, name: &OsStr, bytes: &[u8]) -> io::Result<()> {
        let redacted = crate::secrets::redact_bytes(bytes);
        let bytes = redacted.as_slice();
        let previous = self.existing(name)?;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let mut temporary = OsString::from(".");
        temporary.push(name);
        temporary.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut file = self.create_new(&temporary)?;
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            self.publish(&temporary, &file, name, previous.as_ref())?;
            self.sync()
        })();
        if result.is_err() {
            let _ = self.remove_owned(&temporary, &file);
        }
        result
    }
}

/// macOS ships `/etc`, `/tmp` and `/var` as symlinks into `/private`. The store
/// refuses symlinked ancestors by design (a swapped ancestor is an attack), so
/// exactly those three system prefixes — verified to point where Apple puts
/// them, and nothing else — are mapped to their canonical directories before
/// the no-follow walk. Elsewhere the path is used as given.
#[cfg(target_os = "macos")]
fn resolve_system_prefix(path: &Path) -> std::borrow::Cow<'_, Path> {
    for prefix in ["/etc", "/tmp", "/var"] {
        let Ok(rest) = path.strip_prefix(prefix) else {
            continue;
        };
        let name = &prefix[1..];
        let canonical = Path::new("/private").join(name);
        let points_into_private = std::fs::read_link(prefix)
            .map(|target| target == Path::new("private").join(name) || target == canonical)
            .unwrap_or(false);
        if points_into_private {
            return std::borrow::Cow::Owned(canonical.join(rest));
        }
    }
    std::borrow::Cow::Borrowed(path)
}

#[cfg(not(target_os = "macos"))]
fn resolve_system_prefix(path: &Path) -> std::borrow::Cow<'_, Path> {
    std::borrow::Cow::Borrowed(path)
}

pub(crate) fn parent(path: &Path) -> io::Result<(PrivateDirectory, &OsStr)> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "store path has no file name")
    })?;
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok((PrivateDirectory::open(directory)?, name))
}

#[cfg(test)]
pub(crate) fn append(path: &Path) -> io::Result<File> {
    let (directory, name) = parent(path)?;
    directory.append(name)
}
pub(crate) fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (directory, name) = parent(path)?;
    directory.replace(name, bytes)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/workspace_store__private_io__tests.rs"]
mod tests;
