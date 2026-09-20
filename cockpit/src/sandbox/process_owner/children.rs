//! Direct-child wait ownership and non-destructive steady-state reaping.
//!
//! Every cockpit process spawn (including library PTYs) enters `claim_spawn`.
//! Its lock covers only spawn and registration, never waiting or pipe draining.
//! The reaper takes the same lock before identifying and waiting for *specific*
//! unclaimed zombies. It cannot consume the status belonging to a live Child
//! handle, including a handle moved to another thread after its spawner exits.
use std::collections::HashMap;
use std::io;
use std::ops::{Deref, DerefMut};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct Claims {
    next: u64,
    direct: HashMap<u32, u64>,
    opaque: usize,
    stopped: bool,
}

fn claims() -> &'static Mutex<Claims> {
    static CLAIMS: OnceLock<Mutex<Claims>> = OnceLock::new();
    CLAIMS.get_or_init(|| Mutex::new(Claims::default()))
}

/// A waiter owns this claim until it observes an exit or drops its handle.
/// Generation matching prevents an old handle's drop from removing a newer
/// claim after a PID has been reused. An unknown library PID conservatively
/// pauses reaping until that library relinquishes its handle.
#[derive(Debug)]
pub(crate) struct ChildClaim {
    identity: Option<(u32, u64)>,
}

impl Drop for ChildClaim {
    fn drop(&mut self) {
        let mut claims = claims().lock().unwrap_or_else(|error| error.into_inner());
        if let Some((pid, generation)) = self.identity {
            if claims.direct.get(&pid) == Some(&generation) {
                claims.direct.remove(&pid);
            }
        } else {
            claims.opaque -= 1;
        }
    }
}

/// The callback must only spawn the child and return its handle; it must not
/// wait for command completion. This also covers portable-pty's library spawn.
pub(crate) fn claim_spawn<T, E>(
    spawn: impl FnOnce() -> Result<T, E>,
    pid: impl FnOnce(&T) -> Option<u32>,
) -> Result<(T, ChildClaim), E> {
    let mut claims = claims().lock().unwrap_or_else(|error| error.into_inner());
    let child = spawn()?;
    let identity = pid(&child).map(|pid| {
        claims.next = claims.next.wrapping_add(1);
        let generation = claims.next;
        claims.direct.insert(pid, generation);
        (pid, generation)
    });
    if identity.is_none() {
        claims.opaque += 1;
    }
    Ok((child, ChildClaim { identity }))
}

/// std::process::Child with a claim that follows the handle through moves.
/// Dropping it relinquishes the wait status, but never signals a live process.
#[derive(Debug)]
pub(crate) struct Child {
    inner: std::process::Child,
    claim: Option<ChildClaim>,
}

impl Child {
    pub(crate) fn id(&self) -> u32 {
        self.inner.id()
    }

    pub(crate) fn wait(&mut self) -> io::Result<ExitStatus> {
        let status = self.inner.wait()?;
        self.claim.take();
        Ok(status)
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.inner.try_wait()?;
        if status.is_some() {
            self.claim.take();
        }
        Ok(status)
    }

    pub(crate) fn wait_with_output(self) -> io::Result<Output> {
        let Self { inner, claim } = self;
        let result = inner.wait_with_output();
        drop(claim);
        result
    }
}

impl Deref for Child {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Child {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub(crate) trait OwnedCommandExt {
    fn spawn_owned(&mut self) -> io::Result<Child>;
    fn status_owned(&mut self) -> io::Result<ExitStatus>;
    /// Capture both streams with null stdin. Active call sites use these
    /// ordinary output defaults; explicit stream routing uses spawn_owned.
    fn output_owned(&mut self) -> io::Result<Output>;
}

impl OwnedCommandExt for Command {
    fn spawn_owned(&mut self) -> io::Result<Child> {
        let (inner, claim) = claim_spawn(|| self.spawn(), |child| Some(child.id()))?;
        Ok(Child {
            inner,
            claim: Some(claim),
        })
    }

    fn status_owned(&mut self) -> io::Result<ExitStatus> {
        self.spawn_owned()?.wait()
    }

    fn output_owned(&mut self) -> io::Result<Output> {
        self.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn_owned()?
            .wait_with_output()
    }
}

#[cfg(target_os = "linux")]
pub(super) fn stop_reaping() {
    // This lock also waits for an in-flight targeted reap to finish before
    // shutdown's separate, destructive ownership cleanup starts.
    claims()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .stopped = true;
}

#[cfg(target_os = "linux")]
pub(super) fn reap_unclaimed() {
    let claims = match claims().try_lock() {
        Ok(claims) => claims,
        Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return,
    };
    if claims.stopped || claims.opaque != 0 {
        return;
    }
    // A readiness probe only: WNOWAIT leaves every wait status untouched.
    // Idle cockpits therefore perform one syscall rather than a /proc scan.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let ready = unsafe {
        libc::waitid(
            libc::P_ALL,
            0,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if ready != 0 || unsafe { info.si_pid() } == 0 {
        return;
    }
    // Thread exit can transfer an ordinary direct child to the group leader.
    // Registry ownership, not its current parent thread, decides who may wait.
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        return;
    };
    let parent = std::process::id();
    for task in tasks.flatten() {
        let Ok(children) = std::fs::read_to_string(task.path().join("children")) else {
            continue;
        };
        for pid in children
            .split_whitespace()
            .filter_map(|word| word.parse::<u32>().ok())
        {
            if claims.direct.contains_key(&pid) {
                continue;
            }
            let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                continue;
            };
            let Some((_, tail)) = stat.rsplit_once(')') else {
                continue;
            };
            let mut fields = tail.split_whitespace();
            if fields.next() != Some("Z")
                || fields.next().and_then(|value| value.parse::<u32>().ok()) != Some(parent)
            {
                continue;
            }
            // Only this unclaimed zombie can be consumed. While the registry
            // lock is held no registered spawn can reuse this PID, and a
            // zombie's PID cannot otherwise be recycled before it is waited.
            unsafe { libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), libc::WNOHANG) };
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/sandbox__process_owner__children__tests.rs"]
mod tests;
