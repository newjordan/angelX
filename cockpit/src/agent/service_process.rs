//! Process ownership for a long-lived stdio service, including wrapper children.
//!
//! Keep the launcher unreaped until its private process group has been retired.
//! Its owned-child claim excludes it from the ordinary orphan reaper, and an
//! unreaped PID cannot be reused for an unrelated process group. Retirement
//! validates that identity under the reaper lock before signalling: process
//! shutdown may already have taken destructive ownership of all descendants.

use crate::agent::sandbox::process_owner::{Child, OwnedCommandExt};
use std::io;
use std::process::{ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};

pub(crate) struct ServiceChild {
    child: Child,
    retired: bool,
    retirement_result: Option<io::Result<ExitStatus>>,
}

impl ServiceChild {
    pub(crate) fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }
        Ok(Self {
            child: command.spawn_owned()?,
            retired: false,
            retirement_result: None,
        })
    }

    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    pub(crate) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub(crate) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub(crate) fn alive(&mut self) -> bool {
        if self.retired {
            return false;
        }
        #[cfg(unix)]
        {
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            // SAFETY: this is our claimed direct child. WNOWAIT leaves its wait
            // status and PID identity held until private-group retirement.
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.id() as libc::id_t,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            result == 0 && info.si_signo == 0
        }
        #[cfg(not(unix))]
        {
            // No process-group ownership on this platform. A completed probe
            // may collect the direct status, which Child retains for wait().
            matches!(self.child.try_wait(), Ok(None))
        }
    }

    /// Stop this service's group, then collect its direct child's own status.
    /// No raw wait/kill handle is exposed: every status-consuming path retires
    /// the private group first, even when the launcher exited on its own.
    pub(crate) fn retire(&mut self) -> io::Result<ExitStatus> {
        if !self.retired {
            // Consume the signalling decision before any status wait. An
            // ECHILD (global cleanup won) or repeated Drop never signals again.
            self.retired = true;
            #[cfg(unix)]
            let signalled = self.child.with_unreaped_identity(|pid| {
                // SAFETY: spawn created this private group, and the ownership
                // guard pins its unreaped leader through both signals. SIGKILL
                // also retires wrapper children that ignore SIGTERM. A server
                // may have left its original group, so still signal its PID.
                unsafe { libc::killpg(pid, libc::SIGKILL) };
                if unsafe { libc::kill(pid, libc::SIGKILL) } == 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error())
                }
            });
            #[cfg(not(unix))]
            let signalled = self.child.kill();
            // Never block waiting/draining while holding the ownership lock.
            self.retirement_result = Some(signalled.and_then(|()| self.child.wait()));
        }
        match &self.retirement_result {
            Some(Ok(status)) => Ok(*status),
            Some(Err(error)) => Err(error.raw_os_error().map_or_else(
                || io::Error::new(error.kind(), error.to_string()),
                io::Error::from_raw_os_error,
            )),
            None => Err(io::Error::other("service retirement interrupted")),
        }
    }
}

impl Drop for ServiceChild {
    fn drop(&mut self) {
        let _ = self.retire();
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/service_process__tests.rs"]
pub(crate) mod tests;
