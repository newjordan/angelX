//! Dedicated anonymous helper metadata file, closed before the candidate exec.
//! Candidate stdout/stderr can never manufacture this channel's receipts.
use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::unix::fs::FileExt;
use std::os::unix::process::CommandExt;

const ENV: &str = "ANGEL_INTERNAL_SANDBOX_STATUS_FD";

pub(crate) struct Receiver {
    file: File,
}

pub(crate) fn attach(command: &mut std::process::Command) -> std::io::Result<Receiver> {
    #[cfg(target_os = "linux")]
    let file = {
        let fd = unsafe { libc::memfd_create(c"angel-sandbox-status".as_ptr(), libc::MFD_CLOEXEC) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        unsafe { File::from_raw_fd(fd) }
    };
    #[cfg(not(target_os = "linux"))]
    let file = File::options().read(true).write(true).open("/dev/null")?;
    let fd = file.as_raw_fd();
    command.env(ENV, fd.to_string());
    // Only async-signal-safe fcntl in the forked command child. Parent remains CLOEXEC.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(Receiver { file })
}

impl Receiver {
    pub(crate) fn receive(&self) -> Option<serde_json::Value> {
        let size = usize::try_from(self.file.metadata().ok()?.len()).ok()?;
        let mut bytes = vec![0; size];
        self.file.read_exact_at(&mut bytes, 0).ok()?;
        serde_json::from_slice(&bytes).ok()
    }
}

pub(super) fn publish(receipt: &serde_json::Value) {
    let fd = std::env::var(ENV).ok().and_then(|s| s.parse::<i32>().ok());
    unsafe {
        std::env::remove_var(ENV);
    }
    if let Some(fd) = fd.filter(|fd| *fd > 2) {
        // Helper owns the inherited descriptor; drop closes it before command execution.
        let mut file = unsafe { File::from_raw_fd(fd) };
        if let Ok(bytes) = serde_json::to_vec(receipt) {
            let _ = file.write_all(&bytes);
        }
    }
}
