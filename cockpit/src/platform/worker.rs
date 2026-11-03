//! Bounded, request-scoped worker processes for replaceable application logic.
//!
//! The host retains authority and state ownership; this boundary only exchanges
//! bytes with a child. Each request gets a fresh process and a whole-request
//! deadline. Callers own the protocol and must validate the returned bytes.

use std::process::Command;
use std::time::Duration;

/// Send one bounded request and collect one bounded response. A failed or
/// timed-out process never returns partial output as a successful response.
#[cfg(test)]
pub(crate) fn run(
    command: Command,
    input: &[u8],
    timeout: Duration,
    input_limit: usize,
    output_limit: usize,
) -> Result<Vec<u8>, String> {
    run_cancellable(command, input, timeout, input_limit, output_limit, || false)
}

/// As `run`, but also stops promptly when the owning request is interrupted.
#[cfg(unix)]
pub(crate) fn run_cancellable(
    mut command: Command,
    input: &[u8],
    timeout: Duration,
    input_limit: usize,
    output_limit: usize,
    interrupted: impl Fn() -> bool,
) -> Result<Vec<u8>, String> {
    use crate::agent::sandbox::process_owner::{Child, OwnedCommandExt};
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::time::Instant;

    if input.len() > input_limit {
        return Err("worker input exceeded its limit".into());
    }
    if timeout.is_zero() {
        return Err("worker deadline must be positive".into());
    }
    if interrupted() {
        return Err("worker request interrupted".into());
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or("worker deadline is too large")?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = command
        .spawn_owned()
        .map_err(|error| format!("could not start worker: {error}"))?;
    // Group signals must precede reaping the leader: once its PID is released,
    // a new and unrelated group could acquire the same numeric ID. The claim
    // pins the unreaped direct child while the signal is sent. This only covers
    // descendants that remain in the private process group.
    let kill_group = |pid: libc::pid_t| {
        // SAFETY: the claim protects this unreaped child and its group ID.
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
            // If the child moved itself out of the group, still stop it.
            libc::kill(pid, libc::SIGKILL);
        }
    };
    let stop = |child: &mut Child| {
        if child
            .with_unreaped_identity(|pid| {
                kill_group(pid);
                Ok(())
            })
            .is_ok()
        {
            let _ = child.wait();
        }
    };
    let Some(stdin) = child.stdin.take() else {
        stop(&mut child);
        return Err("worker input pipe unavailable".into());
    };
    let Some(mut stdout) = child.stdout.take() else {
        stop(&mut child);
        return Err("worker output pipe unavailable".into());
    };
    for fd in [stdin.as_raw_fd(), stdout.as_raw_fd()] {
        // SAFETY: both descriptors are owned by these live pipe handles.
        let ok = unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            flags >= 0 && libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == 0
        };
        if !ok {
            stop(&mut child);
            return Err("could not configure worker pipes".into());
        }
    }
    let mut stdin = Some(stdin);
    let mut written = 0;
    let mut result = Vec::new();
    let mut closed = false;
    let mut status = None;
    let mut chunk = [0u8; 8192];
    loop {
        if interrupted() {
            stop(&mut child);
            return Err("worker request interrupted".into());
        }
        if Instant::now() >= deadline {
            stop(&mut child);
            return Err("worker timed out".into());
        }
        if let Some(pipe) = stdin.as_mut() {
            while written < input.len() {
                match pipe.write(&input[written..]) {
                    Ok(0) => {
                        stop(&mut child);
                        return Err("worker closed its input".into());
                    }
                    Ok(n) => written += n,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        stop(&mut child);
                        return Err("worker input failed".into());
                    }
                }
            }
            if written == input.len() {
                stdin.take(); // EOF is part of the request protocol.
            }
        }
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) => {
                    closed = true;
                    break;
                }
                Ok(n) => {
                    if n > output_limit.saturating_sub(result.len()) {
                        stop(&mut child);
                        return Err("worker output exceeded its limit".into());
                    }
                    result.extend_from_slice(&chunk[..n]);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    stop(&mut child);
                    return Err("worker output failed".into());
                }
            }
        }
        if status.is_none() {
            let mut may_reap = false;
            let ready = child.with_unreaped_identity(|pid| {
                let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
                loop {
                    // SAFETY: the direct child is claimed. WNOWAIT retains its
                    // identity until its group has been signalled.
                    let result = unsafe {
                        libc::waitid(
                            libc::P_PID,
                            pid as libc::id_t,
                            &mut info,
                            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                        )
                    };
                    if result == 0 {
                        let exited = unsafe { info.si_pid() } != 0;
                        // Keep a successful leader unreaped while a descendant
                        // still holds stdout. It may be producing the rest of
                        // the response; killing it now would return a truncated
                        // result as success. A failed leader can stop promptly.
                        let failed = exited
                            && (info.si_code != libc::CLD_EXITED
                                || unsafe { info.si_status() } != 0);
                        may_reap = exited && (closed || failed);
                        if may_reap {
                            kill_group(pid);
                        }
                        return Ok(());
                    }
                    let error = std::io::Error::last_os_error();
                    if error.kind() != std::io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                }
            });
            if ready.is_err() {
                stop(&mut child);
                return Err("could not wait for worker".into());
            }
            if may_reap {
                match child.wait() {
                    Ok(exit) => status = Some(exit),
                    Err(_) => return Err("could not wait for worker".into()),
                }
            }
        }
        if let Some(exit) = status {
            if !exit.success() {
                return Err(format!("worker exited with {exit}"));
            }
            if closed {
                return if written == input.len() {
                    Ok(result)
                } else {
                    Err("worker closed its input".into())
                };
            }
        }
        // Poll both pipes together. This keeps a worker that writes before it
        // reads from deadlocking, and leaves no detached reader/writer thread.
        let mut fds = [
            libc::pollfd {
                fd: stdin.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                events: libc::POLLOUT,
                revents: 0,
            },
            libc::pollfd {
                fd: if closed { -1 } else { stdout.as_raw_fd() },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let remaining = deadline.saturating_duration_since(Instant::now());
        // Poll periodically even after EOF so the child's exit is observed.
        let millis = remaining.as_millis().saturating_add(1).min(25) as i32;
        // SAFETY: fds contains two valid poll descriptors or the ignored -1.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, millis) };
        if ready < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            stop(&mut child);
            return Err("worker pipe polling failed".into());
        }
    }
}

#[cfg(not(unix))]
pub(crate) fn run_cancellable(
    _command: Command,
    _input: &[u8],
    _timeout: Duration,
    _input_limit: usize,
    _output_limit: usize,
    _interrupted: impl Fn() -> bool,
) -> Result<Vec<u8>, String> {
    Err("worker processes are unavailable on this platform".into())
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/worker__tests.rs"]
mod tests;
