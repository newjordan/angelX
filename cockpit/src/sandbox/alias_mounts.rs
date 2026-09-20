//! Bulk inode-alias overlays inside Bubblewrap's already confined namespace.
//! A sealed memfd carries the plan instead of one CLI operation per file.
//! Only this trusted single-threaded setup process receives mount capability;
//! the actual worker receives no capabilities, setup FDs or loader overrides.

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Seek, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const SEALS: i32 = libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE;
const CAP_SYS_ADMIN: u32 = 21;
const CAP_SETPCAP: u32 = 8;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    aliases: Vec<(i32, Vec<u8>)>,
    program: Vec<u8>,
    args: Vec<Vec<u8>>,
    env: Vec<(Vec<u8>, Vec<u8>)>,
    original_nofile: Option<u64>,
}

#[derive(Debug)]
pub(super) struct SetupLimit(libc::rlimit);

impl Drop for SetupLimit {
    fn drop(&mut self) {
        // A failed preparation must not alter the calling process's limits.
        unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &self.0) };
    }
}

fn file_limit() -> io::Result<libc::rlimit> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(limit)
}

pub(super) fn needs_setup_limit(alias_count: usize, existing_sources: usize) -> io::Result<bool> {
    Ok(alias_count
        .saturating_add(existing_sources)
        .saturating_add(64) as u64
        >= file_limit()?.rlim_cur)
}

fn reserve_descriptors(count: usize) -> io::Result<Option<SetupLimit>> {
    let original = file_limit()?;
    let highest = std::fs::read_dir("/proc/self/fd")?
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u64>().ok())
        .max()
        .unwrap_or(2);
    let required = highest.saturating_add(count as u64).saturating_add(64);
    if required <= original.rlim_cur {
        return Ok(None);
    }
    if required > original.rlim_max {
        return Err(io::Error::other(format!(
            "hardlink setup needs {required} descriptors but the host hard limit is {}; raise the worker's RLIMIT_NOFILE before retrying",
            original.rlim_max
        )));
    }
    let setup = libc::rlimit {
        rlim_cur: required,
        rlim_max: original.rlim_max,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &setup) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(Some(SetupLimit(original)))
}

fn inherit(file: &File) -> io::Result<()> {
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn prepare(
    command: &mut Command,
    sources: &mut Vec<File>,
    aliases: &[PathBuf],
    program: OsString,
    args: Vec<OsString>,
) -> io::Result<Option<SetupLimit>> {
    // Several fleet hosts launch with a 1,024 soft FD limit. Pinning a large
    // workspace needs more setup FDs; use the existing hard allowance, then
    // restore the worker's original soft limit after those inputs are closed.
    let setup_limit = reserve_descriptors(aliases.len())?;
    let mut mounts = Vec::with_capacity(aliases.len());
    for path in aliases {
        let source = super::bwrap::pin_source(path)?;
        inherit(&source)?;
        mounts.push((source.as_raw_fd(), path.as_os_str().as_bytes().to_vec()));
        sources.push(source);
    }
    let plan = Plan {
        aliases: mounts,
        program: program.into_vec(),
        args: args.into_iter().map(OsString::into_vec).collect(),
        env: std::env::vars_os()
            .map(|(key, value)| (key.into_vec(), value.into_vec()))
            .collect(),
        original_nofile: setup_limit.as_ref().map(|limit| limit.0.rlim_cur),
    };
    let fd = unsafe {
        libc::memfd_create(
            c"angel-alias-plan".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut input = unsafe { File::from_raw_fd(fd) };
    serde_json::to_writer(&mut input, &plan)?;
    input.flush()?;
    input.rewind()?;
    if unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, SEALS) } == -1 {
        return Err(io::Error::last_os_error());
    }
    inherit(&input)?;
    // The current helper stays pinned, including when its installed path is
    // atomically replaced while this worker is being launched.
    let helper = File::open("/proc/self/exe")?;
    inherit(&helper)?;
    let helper_path = format!("/proc/self/fd/{}", helper.as_raw_fd());
    sources.push(input);
    sources.push(helper);
    command.args(["--cap-add", "CAP_SYS_ADMIN", "--cap-add", "CAP_SETPCAP"]);
    // No worker-controlled LD_PRELOAD/LD_LIBRARY_PATH or language startup
    // environment may run in the setup phase. Restore the original environment
    // only after every overlay is installed and all capabilities are dropped.
    command.env_clear();
    command
        .arg("--")
        .arg(helper_path)
        .args(["--sandbox-exec", "--mount-aliases"])
        .arg(fd.to_string());
    Ok(setup_limit)
}

#[repr(C)]
#[derive(Default)]
struct CapHeader {
    version: u32,
    pid: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CapData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

fn capabilities() -> io::Result<[CapData; 2]> {
    let mut header = CapHeader {
        version: 0x2008_0522,
        pid: 0,
    };
    let mut data = [CapData::default(); 2];
    if unsafe { libc::syscall(libc::SYS_capget, &mut header, data.as_mut_ptr()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(data)
}

fn drop_capabilities() -> io::Result<()> {
    // Drop the bounding set too: an eventual uid-0 exec must not reacquire
    // setup capabilities. SETPCAP remains effective until the final capset.
    for cap in 0..64 {
        if unsafe { libc::prctl(libc::PR_CAPBSET_READ, cap, 0, 0, 0) } == -1 {
            if io::Error::last_os_error().raw_os_error() == Some(libc::EINVAL) {
                break;
            }
            return Err(io::Error::last_os_error());
        }
        if unsafe { libc::prctl(libc::PR_CAPBSET_DROP, cap, 0, 0, 0) } == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    if unsafe {
        libc::prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL,
            0,
            0,
            0,
        )
    } == -1
    {
        return Err(io::Error::last_os_error());
    }
    let header = CapHeader {
        version: 0x2008_0522,
        pid: 0,
    };
    let empty = [CapData::default(); 2];
    if unsafe { libc::syscall(libc::SYS_capset, &header, empty.as_ptr()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[repr(C)]
struct MountAttr {
    attr_set: u64,
    attr_clr: u64,
    propagation: u64,
    userns_fd: u64,
}

fn setup_error(stage: &str) -> io::Error {
    let error = io::Error::last_os_error();
    io::Error::new(error.kind(), format!("hardlink {stage}: {error}"))
}

fn protect(source: i32, path: &Path) -> io::Result<()> {
    // Open both endpoints by descriptor. move_mount targets the pinned dentry,
    // so replacing an ancestor with a symlink cannot redirect the mount.
    let target = super::bwrap::pin_source(path)?;
    let original = std::fs::metadata(format!("/proc/self/fd/{source}"))?;
    let destination = target.metadata()?;
    if !original.is_file()
        || (original.dev(), original.ino()) != (destination.dev(), destination.ino())
    {
        return Err(io::Error::other(
            "hardlink mount target changed during setup",
        ));
    }
    // Clone the verified alias in this mount namespace. The original source
    // descriptor pins identity across namespace setup, but open_tree cannot
    // clone that host-namespace mount directly. This also retains any stricter
    // flags already applied to the workspace mount.
    // OPEN_TREE_CLONE | OPEN_TREE_CLOEXEC | AT_EMPTY_PATH.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_open_tree,
            target.as_raw_fd(),
            c"".as_ptr(),
            1 | libc::O_CLOEXEC | libc::AT_EMPTY_PATH,
        )
    };
    if fd < 0 {
        return Err(setup_error("open_tree"));
    }
    let mount = unsafe { File::from_raw_fd(fd as i32) };
    // MOUNT_ATTR_RDONLY | NOSUID | NODEV, retaining all other source flags.
    let attrs = MountAttr {
        attr_set: 1 | 2 | 4,
        attr_clr: 0,
        propagation: 0,
        userns_fd: 0,
    };
    if unsafe {
        libc::syscall(
            libc::SYS_mount_setattr,
            mount.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH,
            &attrs,
            std::mem::size_of::<MountAttr>(),
        )
    } == -1
    {
        return Err(setup_error("mount_setattr"));
    }
    // MOVE_MOUNT_F_EMPTY_PATH | MOVE_MOUNT_T_EMPTY_PATH.
    if unsafe {
        libc::syscall(
            libc::SYS_move_mount,
            mount.as_raw_fd(),
            c"".as_ptr(),
            target.as_raw_fd(),
            c"".as_ptr(),
            0x04 | 0x40,
        )
    } == -1
    {
        return Err(setup_error("move_mount"));
    }
    Ok(())
}

pub(super) fn exec(args: &[OsString]) -> io::Result<()> {
    let fd = if let [fd] = args {
        fd.to_str()
            .and_then(|fd| fd.parse::<i32>().ok())
            .filter(|fd| *fd > 2)
    } else {
        None
    }
    .ok_or_else(|| io::Error::other("alias setup requires one plan descriptor"))?;
    let caps = capabilities()?;
    let required = (1 << CAP_SYS_ADMIN) | (1 << CAP_SETPCAP);
    let seals = unsafe { libc::fcntl(fd, libc::F_GET_SEALS) };
    if caps[0].effective != required
        || caps[1].effective != 0
        || unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } != 1
        || seals < 0
        || seals & SEALS != SEALS
    {
        return Err(io::Error::other(
            "alias setup requires confined capabilities and a sealed plan",
        ));
    }
    let input = unsafe { File::from_raw_fd(fd) };
    let plan: Plan = serde_json::from_reader(input)?;
    for (source, path) in &plan.aliases {
        protect(*source, Path::new(&OsString::from_vec(path.clone())))?;
    }
    drop_capabilities()?;
    super::bwrap::close_inherited_descriptors(&[])?;
    if let Some(original) = plan.original_nofile {
        let mut limit = file_limit()?;
        limit.rlim_cur = original;
        if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) } == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    let mut command = Command::new(OsString::from_vec(plan.program));
    command.args(plan.args.into_iter().map(OsString::from_vec));
    command.env_clear().envs(
        plan.env
            .into_iter()
            .map(|(key, value)| (OsString::from_vec(key), OsString::from_vec(value))),
    );
    Err(command.exec())
}
