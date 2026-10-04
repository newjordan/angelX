#[cfg(target_os = "linux")]
use super::*;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::process::Stdio;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
#[path = "capture_evidence_fixture.rs"]
pub(crate) mod capture_evidence_fixture;

/// Run process-reaper tests in a separate image; installing its global exit
/// handler in the parallel Rust suite would take ownership of other fixtures.
#[cfg(target_os = "linux")]
pub(crate) fn isolated_fixture(filter: &str, env_name: &str) {
    let listing = Command::new("/proc/self/exe")
        .args(["--exact", filter, "--ignored", "--list"])
        .output()
        .expect("list isolated subprocess fixture");
    assert!(listing.status.success());
    let listing = String::from_utf8(listing.stdout).unwrap();
    assert_eq!(
        listing
            .lines()
            .filter(|line| line.ends_with(": test"))
            .collect::<Vec<_>>(),
        [format!("{filter}: test")],
        "fixture filter must select exactly one ignored test"
    );
    let root =
        std::env::temp_dir().join(format!("angel-service-{env_name}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut child = Command::new("/proc/self/exe")
        .args([
            "--exact",
            filter,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(env_name, &root)
        .stdin(Stdio::null())
        .spawn_owned()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(12);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            // Its installed subreaper/exit handler cleans the wrapper and all
            // recorded descendants before this owned child is waited.
            unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
            let _ = child.wait();
            std::fs::remove_dir_all(&root).unwrap();
            panic!("service retirement fixture exceeded its outer deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    std::fs::remove_dir_all(root).unwrap();
    assert!(status.success(), "service retirement fixture failed");
}

#[cfg(target_os = "linux")]
pub(crate) struct ServiceFixture {
    marker: PathBuf,
    release: PathBuf,
}

#[cfg(target_os = "linux")]
impl ServiceFixture {
    pub(crate) fn new(root: &Path, mode: &str) -> Self {
        Self {
            marker: root.join(format!("{mode}.pid")),
            release: root.join(format!("{mode}.release")),
        }
    }

    pub(crate) fn args(&self) -> Vec<String> {
        vec![
            "-c".into(),
            r#"import os, signal, sys, time
from pathlib import Path
marker, release = map(Path, sys.argv[1:])
read_fd, write_fd = os.pipe()
descendant = os.fork()
if descendant == 0:
    os.close(read_fd)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    os.write(write_fd, b'ready')
    os.close(write_fd)
    time.sleep(30)
    os._exit(0)
os.close(write_fd)
os.read(read_fd, 5)
os.close(read_fd)
marker.write_text(f'{os.getpid()} {descendant} {os.getpgrp()}')
while not release.exists():
    time.sleep(0.005)
os._exit(17)
"#
            .into(),
            self.marker.to_string_lossy().into_owned(),
            self.release.to_string_lossy().into_owned(),
        ]
    }

    pub(crate) fn pids(&self) -> (i32, i32, i32) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Ok(text) = std::fs::read_to_string(&self.marker) {
                let pids = text
                    .split_whitespace()
                    .filter_map(|word| word.parse::<i32>().ok())
                    .collect::<Vec<_>>();
                if let [leader, descendant, group] = pids.as_slice() {
                    return (*leader, *descendant, *group);
                }
            }
            assert!(Instant::now() < deadline, "wrapper fixture did not start");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub(crate) fn release(&self) {
        std::fs::write(&self.release, "exit").unwrap();
    }

    pub(crate) fn assert_reaped(pid: i32) {
        let path = PathBuf::from(format!("/proc/{pid}/stat"));
        let deadline = Instant::now() + Duration::from_secs(2);
        while path.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!path.exists(), "owned fixture PID {pid} was not reaped");
    }

    pub(crate) fn assert_unreaped_launcher(pid: i32) {
        // Give the steady-state orphan reaper a full iteration. The direct
        // service claim must keep this exited launcher and its PGID pinned.
        std::thread::sleep(Duration::from_millis(300));
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        assert_eq!(
            stat.rsplit_once(')').unwrap().1.split_whitespace().next(),
            Some("Z"),
            "an alive probe must leave the launcher wait status owned"
        );
    }
}

#[cfg(target_os = "linux")]
pub(crate) struct FixtureCleanup;

#[cfg(target_os = "linux")]
impl FixtureCleanup {
    pub(crate) fn new() -> Self {
        crate::agent::sandbox::process_owner::initialize().unwrap();
        Self
    }
}

#[cfg(target_os = "linux")]
impl Drop for FixtureCleanup {
    fn drop(&mut self) {
        // This is the isolated fixture's final ownership cleanup, including
        // assertion failures. No other suite's child runs in this process.
        crate::agent::sandbox::process_owner::cleanup();
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn service_child_retirement_fixture() {
    let root = std::env::var_os("ANGEL_T_SERVICE_CHILD")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = FixtureCleanup::new();
    let fixture = ServiceFixture::new(Path::new(&root), "owned-child");
    let mut command = Command::new("python3");
    command
        .args(fixture.args())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut child = ServiceChild::spawn(&mut command).unwrap();
    let (leader, descendant, group) = fixture.pids();
    assert_eq!(group, leader, "the service must own a private group");
    assert!(child.alive());
    fixture.release();
    let deadline = Instant::now() + Duration::from_secs(2);
    while child.alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!child.alive());
    ServiceFixture::assert_unreaped_launcher(leader);
    assert_eq!(child.retire().unwrap().code(), Some(17));
    assert!(child.retired);
    ServiceFixture::assert_reaped(leader);
    ServiceFixture::assert_reaped(descendant);
    assert_eq!(child.retire().unwrap().code(), Some(17));
    assert!(!child.alive());
    drop(child); // cached status only; the group identity was released above.

    // A pipe/setup error can return before a full MCP/LSP client exists. The
    // local process guard must still retire its live wrapper and descendants.
    let fixture = ServiceFixture::new(Path::new(&root), "setup-failure");
    let mut pids = None;
    let setup: io::Result<()> = (|| {
        let mut command = Command::new("python3");
        command.args(fixture.args()).stdout(Stdio::null());
        let mut child = ServiceChild::spawn(&mut command)?;
        pids = Some(fixture.pids());
        child
            .take_stdout()
            .ok_or_else(|| io::Error::other("stdout setup unavailable"))?;
        Ok(())
    })();
    assert!(setup.is_err());
    let (leader, descendant, group) = pids.unwrap();
    assert_eq!(leader, group);
    ServiceFixture::assert_reaped(leader);
    ServiceFixture::assert_reaped(descendant);
}

#[cfg(target_os = "linux")]
#[test]
fn service_child_preserves_wait_ownership_and_retires_wrapper_descendants() {
    let filter = format!(
        "{}::service_child_retirement_fixture",
        module_path!().split_once("::").unwrap().1
    );
    isolated_fixture(&filter, "ANGEL_T_SERVICE_CHILD");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn global_cleanup_retirement_fixture() {
    assert!(
        std::env::var_os("ANGEL_T_SERVICE_GLOBAL_CLEANUP").is_some(),
        "subprocess fixture requires its parent test"
    );
    let _cleanup = FixtureCleanup::new();
    let mut command = Command::new("sleep");
    command
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = ServiceChild::spawn(&mut command).unwrap();
    let pid = child.id();
    let mut signals = 0;
    child
        .child
        .with_unreaped_identity(|owned| {
            assert_eq!(owned as u32, pid);
            signals += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(signals, 1, "normal claimed identity permits signalling");
    crate::agent::sandbox::process_owner::cleanup();
    ServiceFixture::assert_reaped(pid as i32);
    let refused = child.child.with_unreaped_identity(|_| {
        signals += 1;
        Ok(())
    });
    assert_eq!(refused.unwrap_err().raw_os_error(), Some(libc::ECHILD));
    assert_eq!(signals, 1, "shutdown must not call the signal callback");
    for _ in 0..2 {
        assert_eq!(
            child.retire().unwrap_err().raw_os_error(),
            Some(libc::ECHILD)
        );
        assert!(child.retired);
        assert!(!child.alive());
    }
    drop(child); // Reuses the refused decision; no numeric PID/PGID signal.
}

#[cfg(target_os = "linux")]
#[test]
fn global_cleanup_cannot_turn_later_retirement_into_a_stale_group_signal() {
    let filter = format!(
        "{}::global_cleanup_retirement_fixture",
        module_path!().split_once("::").unwrap().1
    );
    isolated_fixture(&filter, "ANGEL_T_SERVICE_GLOBAL_CLEANUP");
}
