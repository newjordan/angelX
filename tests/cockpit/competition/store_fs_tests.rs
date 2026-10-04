use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDir(std::path::PathBuf);

impl TestDir {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "angel-competition-files-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn nonregular_open_fixture() {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
    use std::sync::mpsc;
    use std::time::Duration;

    let root = std::env::var_os("ANGEL_T_COMPETITION_FIFO")
        .expect("subprocess fixture requires its parent test");
    let root = std::path::PathBuf::from(root);
    for (label, kind) in [
        ("read", OpenKind::Read),
        ("write-existing", OpenKind::WriteExisting),
        ("append", OpenKind::Append),
    ] {
        let path = root.join(label);
        let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: name is a live, NUL-terminated path in this fixture's directory.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let worker_path = path.clone();
        let (send, receive) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            send.send(open_private(&worker_path, kind).map(|_| ()))
                .unwrap();
        });
        let first = receive.recv_timeout(Duration::from_millis(500));
        let blocked = first.is_err();
        let result = match first {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Release the old blocking implementation before asserting,
                // so even the red regression leaves no blocked worker.
                let peer = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(&path)
                    .unwrap();
                let result = receive.recv_timeout(Duration::from_secs(1)).unwrap();
                drop(peer);
                result
            }
            Err(error) => panic!("store worker disconnected: {error}"),
        };
        worker.join().unwrap();
        assert!(!blocked, "{label} waited for a FIFO peer before validation");
        assert!(result.is_err());
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_fifo());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn fifo_reads_writes_and_appends_refuse_without_peer() {
    crate::agent::process_test_support::isolated_fixture(
        &format!(
            "{}::nonregular_open_fixture",
            module_path!().split_once("::").unwrap().1
        ),
        "ANGEL_T_COMPETITION_FIFO",
    );
}

#[test]
fn regular_reads_preserve_bytes_and_enforce_bound() {
    let dir = TestDir::new();
    let path = dir.0.join("receipt.json");
    let data = b"{\"receipt\":true}\n\0\xff";
    fs::write(&path, data).unwrap();
    assert_eq!(read_bounded(&path, data.len() as u64).unwrap(), data);
    assert!(read_bounded(&path, data.len() as u64 - 1).is_err());
    assert_eq!(fs::read(path).unwrap(), data);
}

#[test]
fn regular_append_and_existing_write_keep_file_semantics() {
    use std::io::{Seek, Write};
    let dir = TestDir::new();
    let path = dir.0.join("events.jsonl");
    fs::write(&path, b"one").unwrap();
    open_private(&path, OpenKind::Append)
        .unwrap()
        .write_all(b"two")
        .unwrap();
    let mut file = open_private(&path, OpenKind::WriteExisting).unwrap();
    file.seek(std::io::SeekFrom::Start(0)).unwrap();
    file.write_all(b"ONE").unwrap();
    drop(file);
    assert_eq!(fs::read(path).unwrap(), b"ONEtwo");
}

#[cfg(unix)]
#[test]
fn nofollow_and_private_regular_permissions_remain_in_force() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TestDir::new();
    let path = dir.0.join("receipt");
    fs::write(&path, b"evidence").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let link = dir.0.join("alias");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    for kind in [OpenKind::Read, OpenKind::WriteExisting, OpenKind::Append] {
        assert!(open_private(&link, kind).is_err());
    }
    assert_eq!(fs::read(&path).unwrap(), b"evidence");
    drop(open_private(&path, OpenKind::Read).unwrap());
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
