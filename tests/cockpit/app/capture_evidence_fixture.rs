//! Real finite launcher with an escaped descendant retaining one output pipe.
//! Use only inside an isolated fixture with FixtureCleanup installed.

use super::ServiceFixture;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) struct EvidenceFixture {
    program: PathBuf,
    marker: PathBuf,
    cleaned: bool,
}

impl EvidenceFixture {
    pub(crate) fn new(root: &Path, label: &str, stream: &str, output: &str) -> Self {
        assert!(matches!(stream, "stdout" | "stderr"));
        let marker = root.join(format!("{label}-{stream}.pid"));
        let program = root.join(format!("{label}-{stream}.py"));
        let script = format!(
            r#"#!/usr/bin/python3
import os, signal, time
marker, stream, output = {marker}, {stream}, {output}
read_fd, write_fd = os.pipe()
descendant = os.fork()
if descendant == 0:
    os.close(read_fd)
    os.setsid()
    os.close(2 if stream == 'stdout' else 1)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    os.write(write_fd, b'ready')
    os.close(write_fd)
    time.sleep(30)
    os._exit(0)
os.close(write_fd)
os.read(read_fd, 5)
os.close(read_fd)
with open(marker, 'w') as receipt:
    receipt.write(f'{{os.getpid()}} {{descendant}}')
os.write(1, output.encode())
os._exit(0)
"#,
            marker = serde_json::to_string(marker.to_str().unwrap()).unwrap(),
            stream = serde_json::to_string(stream).unwrap(),
            output = serde_json::to_string(output).unwrap(),
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            program,
            marker,
            cleaned: false,
        }
    }

    pub(crate) fn command(&self) -> Command {
        Command::new(&self.program)
    }

    pub(crate) fn finish(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;
        let Ok(receipt) = std::fs::read_to_string(&self.marker) else {
            return; // Final isolated FixtureCleanup owns any failed setup.
        };
        let pids = receipt
            .split_whitespace()
            .map(|value| value.parse::<i32>().unwrap())
            .collect::<Vec<_>>();
        let [leader, descendant] = pids.as_slice() else {
            panic!("evidence fixture identity receipt malformed");
        };
        let stat = std::fs::read_to_string(format!("/proc/{descendant}/stat")).ok();
        let parent = stat.as_deref().and_then(|stat| {
            stat.rsplit_once(')')?
                .1
                .split_whitespace()
                .nth(1)?
                .parse::<u32>()
                .ok()
        });
        if parent == Some(std::process::id()) {
            // This live, adopted fixture descendant still belongs to our
            // isolated subreaper. Never signal a stale numeric identity.
            unsafe { libc::kill(*descendant, libc::SIGKILL) };
        }
        ServiceFixture::assert_reaped(*leader);
        ServiceFixture::assert_reaped(*descendant);
    }
}

impl Drop for EvidenceFixture {
    fn drop(&mut self) {
        self.finish();
    }
}
