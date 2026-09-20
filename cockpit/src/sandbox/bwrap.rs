//! Opt-in mount confinement for tools that must create nested namespaces.
//! Bubblewrap confines the namespace before any agent command runs. Large
//! alias sets use our trusted native setup stage within that namespace.
//! There is deliberately no fallback to an unconfined exec.

use super::{SandboxPolicy, install_inet_socket_filter};
use std::ffi::CString;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

const EXECUTABLE: &str = "/usr/bin/bwrap";

/// Pseudo-devices mirrored into the private `/dev`. Kept identical to the set
/// `bwrap --dev` populates so a nested grader sandbox can rebuild its own.
const STANDARD_DEVICES: [&str; 6] = [
    "/dev/null",
    "/dev/zero",
    "/dev/full",
    "/dev/random",
    "/dev/urandom",
    "/dev/tty",
];

#[derive(Debug)]
struct PreparedCommand {
    command: Command,
    // Pin mount sources until bwrap consumes the descriptors. Canonical path
    // strings alone would race an allowed directory replaced with a symlink.
    _sources: Vec<File>,
    _setup_limit: Option<super::alias_mounts::SetupLimit>,
}

pub(super) fn pin_source(root: &Path) -> io::Result<File> {
    #[repr(C)]
    struct OpenHow {
        flags: u64,
        mode: u64,
        resolve: u64,
    }
    let path = CString::new(root.as_os_str().as_bytes())?;
    let how = OpenHow {
        flags: (libc::O_PATH | libc::O_CLOEXEC) as u64,
        mode: 0,
        // RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS. Canonicalize happened
        // earlier; reject a symlink substituted anywhere before opening.
        resolve: 0x02 | 0x04,
    };
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            libc::AT_FDCWD,
            path.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        )
    };
    if fd == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd as i32) })
}

fn trusted_device_source(root: &Path) -> io::Result<bool> {
    if !root.starts_with("/dev") || root.starts_with("/dev/shm") {
        return Ok(false);
    }
    // dev-bind has no fd variant. Only root-owned device hierarchies whose
    // directory entries the agent cannot replace may use that path operation.
    for parent in root.ancestors() {
        let metadata = parent.symlink_metadata()?;
        if metadata.is_dir() && (metadata.uid() != 0 || metadata.mode() & 0o022 != 0) {
            return Err(io::Error::other(
                "bwrap refuses a mutable device mount source",
            ));
        }
    }
    Ok(true)
}

pub(super) fn close_inherited_descriptors(sources: &[File]) -> io::Result<()> {
    let keep: Vec<_> = sources.iter().map(AsRawFd::as_raw_fd).collect();
    // Collect before closing: the iterator itself owns a temporary descriptor.
    // This helper has no threads, so no concurrent opens can reuse a number.
    let descriptors: Vec<_> = std::fs::read_dir("/proc/self/fd")?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().parse::<i32>().ok()))
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();
    for fd in descriptors {
        if fd > 2 && !keep.contains(&fd) {
            unsafe {
                libc::close(fd);
            }
        }
    }
    Ok(())
}

fn command(
    executable: &Path,
    policy: &SandboxPolicy,
    program: OsString,
    args: impl IntoIterator<Item = OsString>,
    detached: bool,
) -> io::Result<PreparedCommand> {
    if !executable.is_file() {
        return Err(io::Error::other(format!(
            "bwrap sandbox unavailable at {}; refusing unconfined fallback",
            executable.display()
        )));
    }
    let mut roots = Vec::new();
    for root in &policy.writable_roots {
        // The replacement /proc belongs to our private PID namespace. Resolving
        // /proc/self/task before bwrap would instead bind the helper's host PID.
        if root == Path::new("/proc/self/task") {
            continue;
        }
        let root = root.canonicalize().map_err(|error| {
            io::Error::other(format!("bwrap writable root {}: {error}", root.display()))
        })?;
        if root == Path::new("/") || root.starts_with("/proc") || root.starts_with("/sys") {
            return Err(io::Error::other(format!(
                "bwrap rejects host kernel write root {}",
                root.display()
            )));
        }
        roots.push(root);
    }
    roots.sort_by(|a, b| {
        a.components()
            .count()
            .cmp(&b.components().count())
            .then(a.cmp(b))
    });
    roots.dedup();
    let mut command = Command::new(executable);
    if !detached {
        command.args(["--die-with-parent", "--new-session"]);
    }
    // Detached callers already establish their own process group and detached
    // stdio. Keep descendants in that group so proc_stop can reap them, and do
    // not kill them merely because the owning cockpit restarts.
    command.args(["--unshare-user", "--unshare-pid", "--cap-drop", "ALL"]);
    let sealed = !policy.sealed_reads.is_empty();
    let mut sources = Vec::new();
    if sealed {
        super::sealed::validate_policy(policy).map_err(io::Error::other)?;
        command.args(["--tmpfs", "/"]);
        for root in &policy.sealed_reads {
            let source = pin_source(root)?;
            let fd = source.as_raw_fd();
            if unsafe { libc::fcntl(fd, libc::F_SETFD, 0) } == -1 {
                return Err(io::Error::last_os_error());
            }
            command.arg("--ro-bind-fd").arg(fd.to_string()).arg(root);
            sources.push(source);
        }
        // Preserve merged-/usr aliases without mounting host root.
        for alias in ["/bin", "/sbin", "/lib", "/lib64", "/libx32"] {
            if let Ok(target) = std::fs::read_link(alias) {
                command.arg("--symlink").arg(target).arg(alias);
            }
        }
        // The sealed root is a bare tmpfs, so /tmp does not exist at all.
        // Without it toolchains fall back to the cwd for temp files
        // (libiberty's `choose_tmpdir`), and a read-only cwd turns an ordinary
        // link into SIGABRT rather than a clean error: gcc expands
        // `-plugin-opt=-fresolution=%u.res` on every link, and
        // `make_temp_file_with_prefix` aborts when `mkstemps` fails. This is
        // scratch, never host state. Mounted before the policy's writable
        // roots so an explicit /tmp grant still binds over it; `--remount-ro /`
        // is not recursive, so /tmp stays writable while the root is sealed.
        //
        // Sealed only: `--ro-bind / /` promises read-all, and a private /tmp
        // there would hide the host's from the payload.
        command.args(["--perms", "01777", "--tmpfs", "/tmp"]);
    } else {
        command.args(["--ro-bind", "/", "/"]);
    }
    command.args(["--proc", "/proc", "--tmpfs", "/dev"]);
    // The standard pseudo-device set every container runtime provides
    // (`bwrap --dev`, Docker, systemd PrivateDevices). Nested verifiers that
    // run their own `bwrap --dev /dev` bind-mount exactly these names from the
    // outer /dev and fail closed when one is missing. None is host-backed
    // storage or a real terminal: `--new-session` leaves no controlling TTY
    // for `/dev/tty` to reach in attached mode.
    for device in STANDARD_DEVICES {
        command.args(["--dev-bind", device, device]);
    }
    command.args([
        "--symlink",
        "/proc/self/fd",
        "/dev/fd",
        "--symlink",
        "/proc/self/fd/0",
        "/dev/stdin",
        "--symlink",
        "/proc/self/fd/1",
        "/dev/stdout",
        "--symlink",
        "/proc/self/fd/2",
        "/dev/stderr",
    ]);
    // Do not use --dev: it also creates pts/shm/mqueue mounts and console
    // links the policy did not authorize. Explicit GPU/shared-memory policy
    // roots are mounted individually below.
    let hardlinks = super::hardlinks::scan(&roots)?;
    for root in roots {
        if trusted_device_source(&root)? {
            // Recognized host devices live under root-owned /dev. Unlike
            // ordinary bind-fd, dev-bind preserves device access explicitly
            // requested by the policy. Never infer additional device roots.
            command.arg("--dev-bind").arg(&root).arg(&root);
        } else {
            let source = pin_source(&root)?;
            let fd = source.as_raw_fd();
            // Single-threaded helper: retain this explicit bwrap input across
            // exec. Bubblewrap consumes bind-fd inputs before payload launch.
            if unsafe { libc::fcntl(fd, libc::F_SETFD, 0) } == -1 {
                return Err(io::Error::last_os_error());
            }
            command.arg("--bind-fd").arg(fd.to_string()).arg(&root);
            sources.push(source);
        }
    }
    // Overlays must follow every writable bind, including nested roots.
    // Only regular files with nlink > 1 are pinned read-only. Directories the
    // scan could not finish (`hardlinks.limited`) are reported in the receipt,
    // never mounted: a read-only directory over a writable root denied the
    // sandboxed child its own cwd (batch-13 rehearsal, five sandbox tests EACCES).
    let args: Vec<_> = args.into_iter().collect();
    // Bubblewrap counts both argv and --args contents against MAX_ARGS=9000.
    // Large alias sets need one trusted bulk setup pass, not per-file CLI
    // options or nested namespaces (which repeatedly walk the mount table).
    let bulk = command.get_args().count() + hardlinks.paths.len() * 3 + args.len() + 8 >= 8_000
        || super::alias_mounts::needs_setup_limit(hardlinks.paths.len(), sources.len())?;
    if !bulk {
        for path in &hardlinks.paths {
            let source = pin_source(path)?;
            let fd = source.as_raw_fd();
            if unsafe { libc::fcntl(fd, libc::F_SETFD, 0) } == -1 {
                return Err(io::Error::last_os_error());
            }
            command.arg("--ro-bind-fd").arg(fd.to_string()).arg(path);
            sources.push(source);
        }
    }
    eprintln!("sandbox-hardlinks: {}", hardlinks.stderr_line());
    command.args(["--remount-ro", "/dev"]);
    if sealed {
        command.args(["--remount-ro", "/"]);
    }
    if !policy.allow_network {
        command.arg("--unshare-net");
    }
    let setup_limit = if bulk {
        super::alias_mounts::prepare(&mut command, &mut sources, &hardlinks.paths, program, args)?
    } else {
        command.arg("--").arg(program).args(args);
        None
    };
    Ok(PreparedCommand {
        command,
        _sources: sources,
        _setup_limit: setup_limit,
    })
}

pub(super) fn exec(
    policy: &SandboxPolicy,
    program: OsString,
    args: impl IntoIterator<Item = OsString>,
    detached: bool,
) -> io::Result<()> {
    let mut prepared = command(Path::new(EXECUTABLE), policy, program, args, detached)?;
    // Single-threaded helper only. Set this before bwrap so neither its setup
    // nor the payload can acquire privileges through setuid executables.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if !policy.allow_network {
        install_inet_socket_filter().map_err(io::Error::other)?;
    }
    // bwrap closes consumed --bind-fd inputs but preserves other inherited
    // descriptors. Do not expose a host writable file/socket to the payload.
    close_inherited_descriptors(&prepared._sources)?;
    Err(prepared.command.exec())
}

#[cfg(test)]
pub(crate) fn diagnostic_plan(policy: &SandboxPolicy) -> String {
    match command(
        Path::new(EXECUTABLE),
        policy,
        "sh".into(),
        ["-c".into(), "echo hello-sandbox".into()],
        false,
    ) {
        Ok(prepared) => format!("{:?}", prepared.command),
        Err(error) => format!("bwrap plan failed before launch: {error}"),
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/sandbox__bwrap__tests.rs"]
mod tests;
