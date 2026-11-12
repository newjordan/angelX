//! What the cockpit lends the realm host (`angel_realm_host::services`): the
//! club in hand as the model its wish grants ask, its owned child processes
//! for the painters, and the window's door for a joined host's paintings.

use angel_plug::services::{OwnedChild, Processes};
use angel_realm_host::services::{Host, Service};

/// The cockpit's owned child, retired with its process group when dropped.
struct Owned(Box<dyn OwnedChild>);

impl Service for Owned {
    fn take_stdin(&mut self) -> Option<std::process::ChildStdin> {
        self.0.take_stdin()
    }

    fn take_stdout(&mut self) -> Option<std::process::ChildStdout> {
        self.0.take_stdout()
    }

    fn alive(&mut self) -> bool {
        self.0.alive()
    }

    fn retire(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.0.retire()
    }
}

struct Lent(Box<dyn Processes>);

impl Host for Lent {
    fn spawn_service(&self, command: &mut std::process::Command) -> std::io::Result<Box<dyn Service>> {
        self.0.spawn_owned(command).map(|child| Box::new(Owned(child)) as Box<dyn Service>)
    }

    fn paint_host(&self, url: &str, trust: &angel_serve::client::Trust) {
        crate::cart::glass::set_paint_host(url, trust);
    }

    fn paint_dir(&self, dir: &std::path::Path) {
        crate::cart::glass::set_paint_dir(dir.to_path_buf());
    }

    fn looks_dir(&self, dir: &std::path::Path) {
        crate::cart::glass::set_looks_dir(dir.to_path_buf());
    }
}

/// Lend the realm host the cockpit's answers. The first call wins.
pub(crate) fn install(processes: Box<dyn Processes>) {
    angel_realm_host::services::install(Box::new(Lent(processes)));
}
