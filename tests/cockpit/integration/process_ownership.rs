#![cfg(target_os = "linux")]
// Model-free process cohort: the production helper, real PPid/session tracing,
// signal escalation, externally killed worker and main-process exit reaper.
#[path = "../../../cockpit/src/agent/sandbox/process_owner.rs"]
#[allow(dead_code)]
mod process_owner;

use process_owner::OwnedCommandExt;
use std::collections::BTreeMap;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

mod tests {
    pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn snapshot() -> BTreeMap<i32, (i32, i32, u64)> {
    std::fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse().ok()?;
            let text = std::fs::read_to_string(entry.path().join("stat")).ok()?;
            let fields: Vec<_> = text.rsplit_once(')')?.1.split_whitespace().collect();
            Some((
                pid,
                (
                    fields.get(1)?.parse().ok()?,
                    fields.get(3)?.parse().ok()?,
                    fields.get(19)?.parse().ok()?,
                ),
            ))
        })
        .collect()
}

fn related(root: i32, table: &BTreeMap<i32, (i32, i32, u64)>) -> Vec<i32> {
    let mut owned = vec![root];
    loop {
        let before = owned.len();
        for (&pid, &(ppid, sid, _)) in table {
            if !owned.contains(&pid) && (owned.contains(&ppid) || sid == root) {
                owned.push(pid);
            }
        }
        if before == owned.len() {
            return owned;
        }
    }
}

struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let table = snapshot();
        for pid in related(self.0.id() as i32, &table).into_iter().rev() {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
        let _ = self.0.wait();
    }
}
fn helper(script: &str) -> Worker {
    let mut command = Command::new(env!("CARGO_BIN_EXE_angel-sandbox"));
    command
        .args(["--sandbox-exec", "--", "sh", "-c", script])
        .env(
            "ANGEL_INTERNAL_SANDBOX_POLICY",
            r#"{"writable_roots":[],"allow_network":false,"enforce":false}"#,
        )
        .env("ANGEL_INTERNAL_SANDBOX_BACKEND", "landlock")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    Worker(command.spawn().unwrap())
}
fn wait_tree(worker: &Worker, minimum: usize) -> BTreeMap<i32, (i32, i32, u64)> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let table = snapshot();
        if related(worker.0.id() as i32, &table).len() >= minimum {
            return table;
        }
        assert!(Instant::now() < deadline, "worker tree never became ready");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn settled(worker: &mut Worker) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = worker.0.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "worker did not reap");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn assert_no_orphans(root: i32, before: &BTreeMap<i32, (i32, i32, u64)>) {
    let after = snapshot();
    let survivors: Vec<_> = related(root, before)
        .into_iter()
        .filter(|pid| *pid != root && after.get(pid).is_some_and(|row| row.2 == before[pid].2))
        .collect();
    eprintln!(
        "LIFECYCLE_TRACE {}",
        serde_json::json!({"method":"ppid_chain+session+starttime", "root":root, "observed":related(root,before), "orphans":survivors})
    );
    assert!(
        survivors.is_empty(),
        "owned descendants survived: {survivors:?}"
    );
}

#[test]
fn helper_command_child_leaves_caller_session_and_has_no_controlling_tty() {
    let _lock = crate::tests::env_lock();
    let parent_sid = unsafe { libc::getsid(0) };
    let output = Command::new(env!("CARGO_BIN_EXE_angel-sandbox"))
        .args([
            "--sandbox-exec",
            "--",
            "python3",
            "-c",
            "import json,os\nstat=open('/proc/self/stat').read()\nfields=stat.rsplit(')',1)[1].split()\nprint(json.dumps({'pid':os.getpid(),'ppid':os.getppid(),'sid':os.getsid(0),'pgrp':os.getpgrp(),'tty_nr':int(fields[4])}))",
        ])
        .env(
            "ANGEL_INTERNAL_SANDBOX_POLICY",
            r#"{"writable_roots":[],"allow_network":false,"enforce":false}"#,
        )
        .env("ANGEL_INTERNAL_SANDBOX_BACKEND", "landlock")
        .env("ANGEL_INTERNAL_SANDBOX_LIFECYCLE", "attached")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "helper failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let child: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let pid = child["pid"].as_i64().unwrap();
    let sid = child["sid"].as_i64().unwrap();
    let pgrp = child["pgrp"].as_i64().unwrap();
    let tty_nr = child["tty_nr"].as_i64().unwrap();
    assert_ne!(
        sid, parent_sid as i64,
        "command child shared the caller session"
    );
    assert_eq!(sid, pid, "command child is not its session leader");
    assert_eq!(pgrp, pid, "command child is not its process-group leader");
    assert_eq!(tty_nr, 0, "command child kept a controlling terminal");
}

#[test]
fn helper_detached_command_child_stays_in_owner_session_and_group() {
    let _lock = crate::tests::env_lock();
    let payload = "import json,os\nprint(json.dumps({'pid':os.getpid(),'ppid':os.getppid(),'sid':os.getsid(0),'pgrp':os.getpgrp(),'supervisor_sid':os.getsid(os.getppid()),'supervisor_pgrp':os.getpgid(os.getppid())}))";
    let output = Command::new(env!("CARGO_BIN_EXE_angel-sandbox"))
        .args(["--sandbox-exec", "--", "python3", "-c", payload])
        .env(
            "ANGEL_INTERNAL_SANDBOX_POLICY",
            r#"{"writable_roots":[],"allow_network":false,"enforce":false}"#,
        )
        .env("ANGEL_INTERNAL_SANDBOX_BACKEND", "landlock")
        .env("ANGEL_INTERNAL_SANDBOX_LIFECYCLE", "detached")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "helper failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let child: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let sid = child["sid"].as_i64().unwrap();
    let pgrp = child["pgrp"].as_i64().unwrap();
    let supervisor_sid = child["supervisor_sid"].as_i64().unwrap();
    let supervisor_pgrp = child["supervisor_pgrp"].as_i64().unwrap();
    assert_eq!(
        sid, supervisor_sid,
        "detached command child left the owner session"
    );
    assert_eq!(
        pgrp, supervisor_pgrp,
        "detached command child left the owner process group"
    );
}

#[test]
fn lifecycle_cancel_reaps_setsid_and_sigterm_ignoring_children() {
    let _lock = crate::tests::env_lock();
    // Keep the inner shell present: some shells exec their final simple
    // command, which made the old five-process readiness condition impossible.
    let mut worker = helper("setsid sh -c 'trap \"\" TERM; sleep 300 & wait' & sleep 300 & wait");
    let root = worker.0.id() as i32;
    let before = wait_tree(&worker, 5);
    unsafe {
        libc::kill(root, libc::SIGTERM);
    }
    assert_eq!(settled(&mut worker).signal(), Some(libc::SIGTERM));
    assert_no_orphans(root, &before);
}

#[test]
fn lifecycle_kill_worker_preserves_signal_and_reaps_children() {
    let _lock = crate::tests::env_lock();
    let mut worker = helper("sleep 300 & wait");
    let root = worker.0.id() as i32;
    let before = wait_tree(&worker, 3);
    let command_pid = before
        .iter()
        .find(|(_, row)| row.0 == root)
        .map(|(pid, _)| *pid)
        .unwrap();
    unsafe {
        libc::kill(command_pid, libc::SIGKILL);
    }
    assert_eq!(settled(&mut worker).signal(), Some(libc::SIGKILL));
    assert_no_orphans(root, &before);
}

#[test]
fn lifecycle_normal_exit_reaps_double_fork() {
    let _lock = crate::tests::env_lock();
    let mut worker = helper(
        "python3 -c 'import os,time; p=os.fork(); (os.setsid(), os.fork(), time.sleep(300)) if p == 0 else time.sleep(0.4)'",
    );
    let root = worker.0.id() as i32;
    let before = wait_tree(&worker, 4);
    assert!(settled(&mut worker).success());
    assert_no_orphans(root, &before);
}

#[test]
fn lifecycle_main_fixture() {
    if std::env::var_os("ANGEL_T_PROCESS_EXIT_FIXTURE").is_none() {
        return;
    }
    process_owner::initialize().unwrap();
    let _worker = helper("setsid sleep 300 & sleep 300 & wait");
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn lifecycle_sigint_exit_reaps_entire_binary_tree() {
    let _lock = crate::tests::env_lock();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "lifecycle_main_fixture", "--nocapture"])
        .env("ANGEL_T_PROCESS_EXIT_FIXTURE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let mut worker = Worker(command.spawn().unwrap());
    let root = worker.0.id() as i32;
    let before = wait_tree(&worker, 5);
    unsafe {
        libc::kill(root, libc::SIGINT);
    }
    assert_eq!(settled(&mut worker).code(), Some(128 + libc::SIGINT));
    assert_no_orphans(root, &before);
}

fn wait_pid_file(path: &std::path::Path) -> i32 {
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && let Ok(pid) = text.trim().parse()
        {
            return pid;
        }
        assert!(Instant::now() < until, "PID payload was never published");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn assert_reaped_while_alive(pid: i32, grace: Duration) {
    let identity = snapshot().get(&pid).map(|row| row.2);
    let until = Instant::now() + grace;
    loop {
        let current = snapshot().get(&pid).map(|row| row.2);
        if current.is_none() || (identity.is_some() && current != identity) {
            return;
        }
        assert!(
            Instant::now() < until,
            "PID {pid} still exists after grace; zombies are not reaped processes"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn lifecycle_steady_state_fixture() {
    let Some(root) = std::env::var_os("ANGEL_T_PROCESS_STEADY_FIXTURE") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    process_owner::initialize().unwrap();

    // A failed spawn must leave neither a claim nor a held registry lock.
    assert!(
        Command::new(root.join("missing-command"))
            .spawn_owned()
            .is_err()
    );
    assert_eq!(
        Command::new("sh")
            .args(["-c", "exit 41"])
            .status_owned()
            .unwrap()
            .code(),
        Some(41)
    );
    let output = Command::new("sh")
        .args(["-c", "printf out; printf err >&2; exit 43"])
        .output_owned()
        .unwrap();
    assert_eq!(output.status.code(), Some(43));
    assert_eq!(output.stdout, b"out");
    assert_eq!(output.stderr, b"err");
    assert_eq!(
        Command::new("bash")
            .args([
                "--noprofile",
                "--norc",
                "-o",
                "pipefail",
                "-c",
                "sh -c 'exit 31' | cat"
            ])
            .status_owned()
            .unwrap()
            .code(),
        Some(31)
    );

    let mut orphan_pids = Vec::new();
    for iteration in 0..4 {
        // This direct child is born in a thread that exits and transfers its
        // Child to another waiter. It becomes parented by the group leader,
        // just like an orphan, but its exit receipt must remain protected.
        let mut moved = std::thread::spawn(|| {
            Command::new("sh")
                .args(["-c", "sleep 0.05; exit 29"])
                .spawn_owned()
                .unwrap()
        })
        .join()
        .unwrap();
        let marker = root.join(format!("orphan-{iteration}.pid"));
        let program = r#"import os,sys,time
from pathlib import Path
child = os.fork()
if child == 0:
    os.setsid()
    grandchild = os.fork()
    if grandchild:
        os._exit(0)
    Path(sys.argv[1]).write_text(str(os.getpid()))
    time.sleep(0.025)
    os._exit(37)
os.waitpid(child, 0)
time.sleep(0.05)
sys.exit(23)
"#;
        let status = Command::new("python3")
            .args(["-c", program])
            .arg(&marker)
            .status_owned()
            .unwrap();
        assert_eq!(status.code(), Some(23));
        let orphan = wait_pid_file(&marker);
        orphan_pids.push(orphan);
        assert_reaped_while_alive(orphan, Duration::from_secs(2));
        // Allow a reaper tick with the protected child already a zombie.
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(moved.wait().unwrap().code(), Some(29));
        for pid in &orphan_pids {
            assert!(!snapshot().contains_key(pid), "orphan accumulation: {pid}");
        }
        eprintln!(
            "LIFECYCLE_STEADY iteration={iteration} owned_zombies=0 foreground_exit=23 moved_waiter_exit=29 parent_alive=true"
        );
    }

    // Dropping a handle relinquishes a receipt; it does not kill intentionally
    // detached work. Its natural exit must still be reaped while Angel lives.
    let marker = root.join("detached-finished");
    let detached = Command::new("sh")
        .args(["-c", "sleep 0.5; printf done > \"$1\"", "detached"])
        .arg(&marker)
        .spawn_owned()
        .unwrap();
    let detached_pid = detached.id() as i32;
    drop(detached);
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        snapshot().contains_key(&detached_pid),
        "reaper killed live detached work"
    );
    assert_reaped_while_alive(detached_pid, Duration::from_secs(2));
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "done");

    // Concurrent spawn/register/wait boundaries, with different exact results.
    let waiters: Vec<_> = (50..58)
        .map(|code| {
            std::thread::spawn(move || {
                let mut child = Command::new("sh")
                    .args(["-c", &format!("sleep 0.3; exit {code}")])
                    .spawn_owned()
                    .unwrap();
                std::thread::sleep(Duration::from_millis(400));
                assert_eq!(child.wait().unwrap().code(), Some(code));
            })
        })
        .collect();
    for waiter in waiters {
        waiter.join().unwrap();
    }

    // Cancellation and timeout escalation keep signal receipts and reap dead
    // adopted grandchildren, without signaling another job's process group.
    for signal in [libc::SIGTERM, libc::SIGKILL] {
        let marker = root.join(format!("cancel-{signal}.pid"));
        let mut command = Command::new("sh");
        command
            .args(["-c", "sleep 60 & echo $! > \"$1\"; wait", "cancel"])
            .arg(&marker)
            .process_group(0);
        let mut child = command.spawn_owned().unwrap();
        let descendant = wait_pid_file(&marker);
        assert_eq!(unsafe { libc::killpg(child.id() as i32, signal) }, 0);
        assert_eq!(child.wait().unwrap().signal(), Some(signal));
        assert_reaped_while_alive(descendant, Duration::from_secs(2));
    }
    eprintln!(
        "LIFECYCLE_STEADY detached_survived=true concurrent_receipts=8 cancellation_receipts=true parent_alive=true"
    );
}

#[test]
fn lifecycle_steady_state_reaps_without_stealing_waiters_or_killing_siblings() {
    let _lock = tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-steady-owner-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut sentinel = Worker(Command::new("sleep").arg("30").spawn().unwrap());
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "lifecycle_steady_state_fixture",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("ANGEL_T_PROCESS_STEADY_FIXTURE", &root)
        .stdin(Stdio::null())
        .process_group(0);
    let mut worker = Worker(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = worker.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "steady-state fixture exceeded its deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "steady-state ownership fixture failed");
    assert!(
        sentinel.0.try_wait().unwrap().is_none(),
        "unrelated sibling was killed"
    );
    std::fs::remove_dir_all(root).unwrap();
}
