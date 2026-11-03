//! What the cockpit lends the realm host (`angel_realm_host::services`): the
//! club in hand as the model its wish grants ask, its owned child processes
//! for the painters, and the window's door for a joined host's paintings.

use crate::agent::club::Club;
use crate::agent::service_process::ServiceChild;
use angel_realm_host::services::{Host, Oracle, Service};
use std::sync::Arc;
use std::time::Duration;

/// A club as the realm's oracle: one buffered reply, allowed at least the
/// asked-for time (a longer bound the operator set stands).
struct ClubOracle(Arc<dyn Club>);

impl Oracle for ClubOracle {
    fn respond(&self, prompt: &str, at_least: Duration) -> Result<String, String> {
        crate::agent::club::with_read_timeout_at_least(at_least, || self.0.respond(prompt))
    }

    fn label(&self) -> &str {
        self.0.label()
    }

    fn warm_up(&self) {
        self.0.warm_up();
    }
}

/// `club` as the realm host's oracle.
pub(crate) fn oracle(club: Arc<dyn Club>) -> Arc<dyn Oracle> {
    Arc::new(ClubOracle(club))
}

/// The cockpit's owned child, retired with its process group when dropped.
struct Owned(ServiceChild);

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

struct Lent;

impl Host for Lent {
    fn spawn_service(&self, command: &mut std::process::Command) -> std::io::Result<Box<dyn Service>> {
        ServiceChild::spawn(command).map(|child| Box::new(Owned(child)) as Box<dyn Service>)
    }

    fn paint_host(&self, url: &str, trust: &angel_serve::client::Trust) {
        crate::ui::glass::set_paint_host(url, trust);
    }

    fn paint_dir(&self, dir: &std::path::Path) {
        crate::ui::glass::set_paint_dir(dir.to_path_buf());
    }
}

/// Lend the realm host the cockpit's answers. The first call wins.
pub(crate) fn install() {
    angel_realm_host::services::install(Box::new(Lent));
}

#[cfg(test)]
#[path = "../../../tests/cockpit/drive/realm_services__tests.rs"]
mod tests;
