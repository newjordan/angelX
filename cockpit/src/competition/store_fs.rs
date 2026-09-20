use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::Path;

pub(super) struct StoreLease(File);

impl StoreLease {
    pub(super) fn acquire(path: &Path) -> Result<Self, String> {
        reject_symlink(path)?;
        let file = open_private(path, OpenKind::ReadWrite)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: the returned lease retains this live descriptor.
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result != 0 {
                return Err(io_error("lock", path, std::io::Error::last_os_error()));
            }
        }
        Ok(Self(file))
    }
}

impl Drop for StoreLease {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: this lease exclusively owns the descriptor until drop.
            let _ = unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum OpenKind {
    Read,
    WriteExisting,
    Append,
    CreateNew,
    ReadWrite,
}

pub(super) fn open_private(path: &Path, kind: OpenKind) -> Result<File, String> {
    let mut options = OpenOptions::new();
    match kind {
        OpenKind::Read => options.read(true),
        OpenKind::WriteExisting => options.write(true),
        OpenKind::Append => options.append(true).create(true),
        OpenKind::CreateNew => options.write(true).create_new(true),
        OpenKind::ReadWrite => options.read(true).write(true).create(true),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|error| io_error("open", path, error))?;
    if !file
        .metadata()
        .map_err(|error| io_error("stat", path, error))?
        .is_file()
    {
        return Err(format!("not a regular file: {}", path.display()));
    }
    secure_file(path)?;
    Ok(file)
}

pub(super) fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let mut file = open_private(path, OpenKind::Read)?;
    if file
        .metadata()
        .map_err(|error| io_error("stat", path, error))?
        .len()
        > max
    {
        return Err("action journal exceeds bound".into());
    }
    let mut raw = Vec::new();
    Read::by_ref(&mut file)
        .take(max + 1)
        .read_to_end(&mut raw)
        .map_err(|error| io_error("read", path, error))?;
    if raw.len() as u64 > max {
        return Err("action journal exceeds bound".into());
    }
    Ok(raw)
}

pub(super) fn reject_symlink(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(format!("refusing symlink: {}", path.display()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("inspect", path, error)),
    }
}

fn secure_mode(path: &Path, mode: u32) -> Result<(), String> {
    reject_symlink(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|error| io_error("secure", path, error))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}

pub(super) fn secure_dir(path: &Path) -> Result<(), String> {
    secure_mode(path, 0o700)
}

pub(super) fn secure_file(path: &Path) -> Result<(), String> {
    secure_mode(path, 0o600)
}

pub(super) fn sync_dir(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error("sync", path, error))
}

pub(super) fn io_error(action: &str, path: &Path, error: std::io::Error) -> String {
    format!("{action} {}: {error}", path.display())
}
