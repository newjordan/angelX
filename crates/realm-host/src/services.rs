//! What the realm host asks of the cockpit it is linked into: a model to ask,
//! a place to start a long-lived helper process, and where a joined seat's
//! host keeps its paintings.
//!
//! The cockpit owns all three (its clubs, its process ownership and its
//! window), so it lends them: [`Oracle`] per wish, [`Host`] once at startup
//! with [`install`]. Without a host (this crate's tests, an example that never
//! starts the cockpit) a helper process is started plainly in a group of its
//! own and killed with that group, and a joined seat's paint door is not told.

use std::io;
use std::process::{ChildStdin, ChildStdout, Command, ExitStatus};
use std::sync::OnceLock;

pub use angel_live::realm::Oracle;

/// A helper process that talks over pipes and is retired with its group when
/// dropped.
pub trait Service: Send {
    fn take_stdin(&mut self) -> Option<ChildStdin>;
    fn take_stdout(&mut self) -> Option<ChildStdout>;
    /// Whether the process still runs.
    fn alive(&mut self) -> bool;
    /// Stop the process and its group, then collect its status.
    fn retire(&mut self) -> io::Result<ExitStatus>;
}

/// The cockpit's answers, lent once.
pub trait Host: Send + Sync {
    /// Start `command` as an owned service (the cockpit's process ownership
    /// reaps it with the rest at shutdown).
    fn spawn_service(&self, command: &mut Command) -> io::Result<Box<dyn Service>>;

    /// The socket of the host a seat has joined: the paintings this machine
    /// does not keep are fetched from beside it.
    fn paint_host(&self, url: &str, trust: &angel_serve::client::Trust);

    /// The directory the paintings of the workspace in hand are kept in: the
    /// window reads them from there.
    fn paint_dir(&self, dir: &std::path::Path);
}

static HOST: OnceLock<Box<dyn Host>> = OnceLock::new();

/// Lend the cockpit's answers. The first call wins; returns whether this call
/// installed them.
pub fn install(host: Box<dyn Host>) -> bool {
    HOST.set(host).is_ok()
}

/// Start `command` as a service: the cockpit's when it lent one, else plain.
pub fn spawn_service(command: &mut Command) -> io::Result<Box<dyn Service>> {
    match HOST.get() {
        Some(host) => host.spawn_service(command),
        None => Plain::spawn(command).map(|plain| Box::new(plain) as Box<dyn Service>),
    }
}

/// Tell the cockpit which host a joined seat's paintings come from.
pub fn paint_host(url: &str, trust: &angel_serve::client::Trust) {
    if let Some(host) = HOST.get() {
        host.paint_host(url, trust);
    }
}

/// Tell the cockpit where the workspace's paintings are kept.
pub fn paint_dir(dir: &std::path::Path) {
    if let Some(host) = HOST.get() {
        host.paint_dir(dir);
    }
}

/// A service started with no cockpit: its own process group, killed whole.
struct Plain {
    child: std::process::Child,
    retired: Option<io::Result<ExitStatus>>,
}

impl Plain {
    fn spawn(command: &mut Command) -> io::Result<Plain> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }
        Ok(Plain { child: command.spawn()?, retired: None })
    }
}

impl Service for Plain {
    fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    fn alive(&mut self) -> bool {
        self.retired.is_none() && matches!(self.child.try_wait(), Ok(None))
    }

    fn retire(&mut self) -> io::Result<ExitStatus> {
        if self.retired.is_none() {
            #[cfg(unix)]
            // SAFETY: `spawn` made this child the leader of a group of its own.
            unsafe {
                libc::killpg(self.child.id() as libc::pid_t, libc::SIGKILL);
            }
            let _ = self.child.kill();
            self.retired = Some(self.child.wait());
        }
        match self.retired.as_ref() {
            Some(Ok(status)) => Ok(*status),
            Some(Err(error)) => Err(io::Error::new(error.kind(), error.to_string())),
            None => Err(io::Error::other("service retirement interrupted")),
        }
    }
}

impl Drop for Plain {
    fn drop(&mut self) {
        let _ = self.retire();
    }
}
