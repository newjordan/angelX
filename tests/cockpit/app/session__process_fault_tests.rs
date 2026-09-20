//! Invoked only by the owned process-fault runner with bounded JSON stdin.
use super::*;
use std::io::Read;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    root: PathBuf,
    action: String,
    phase: Option<String>,
}

fn history(count: usize) -> Vec<ChatMsg> {
    (0..count)
        .map(|index| {
            let mut msg = ChatMsg::user(format!("accepted message {index} 界"));
            let mut origin = crate::agent::club::owned_recovery_context_ref();
            origin.import_id = format!("owned-import-{index}");
            msg.recovery_context.push(origin);
            msg
        })
        .collect()
}

#[test]
#[ignore = "owned subprocess worker; requires bounded request on stdin"]
fn persistence_process_worker() {
    let _lock = crate::tests::env_lock();
    let mut input = Vec::new();
    std::io::stdin().take(4097).read_to_end(&mut input).unwrap();
    assert!(input.len() <= 4096);
    let request: Request = serde_json::from_slice(&input).unwrap();
    let root = request.root.canonicalize().unwrap();
    assert_eq!(root, request.root);
    assert_eq!(root.parent(), Some(Path::new("/tmp")));
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("angel-session-crash-")
    );
    assert_eq!(
        std::fs::read(root.join("OWNER")).unwrap(),
        b"angel-owned-persistence/v1\n"
    );
    for name in ["workspace", "sessions"] {
        let path = root.join(name);
        if path.exists() {
            assert!(
                std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_dir()
            );
        } else {
            std::fs::create_dir(&path).unwrap();
        }
    }
    let _identity = crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
    let sessions = root.join("sessions");
    let _sessions =
        crate::tests::TestEnvGuard::set("ANGEL_SESSION_DIR", sessions.to_str().unwrap());
    let workspace = root.join("workspace");
    let session = Session::at_for(sessions, "owned".into(), &workspace);
    match request.action.as_str() {
        "seed" => session.checkpoint_for_exit(&history(1)).unwrap(),
        "exit" => session.checkpoint_for_exit(&history(2)).unwrap(),
        "restart" => {
            let loaded = load_for("owned", &workspace).unwrap();
            let expected = history(loaded.len());
            assert!(same_persisted_history(&loaded, &expected));
            std::fs::write(
                root.join("restart.json"),
                serde_json::to_vec(&serde_json::json!({
                    "messages": loaded.len(),
                    "origins": loaded.iter().map(|m| m.recovery_context.len()).sum::<usize>(),
                    "exact_history": true,
                    "loader": "session::load_for",
                }))
                .unwrap(),
            )
            .unwrap();
        }
        "enospc" => {
            #[cfg(target_os = "linux")]
            {
                use std::io::Write;
                use std::os::unix::ffi::OsStrExt;
                let sessions = root.join("sessions");
                let path = std::ffi::CString::new(sessions.as_os_str().as_bytes()).unwrap();
                let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
                assert_eq!(unsafe { libc::statfs(path.as_ptr(), &mut stat) }, 0);
                let capacity = stat.f_blocks * stat.f_bsize as u64;
                assert_eq!(
                    stat.f_type, 0x0102_1994,
                    "refuse fill outside private tmpfs"
                );
                assert_eq!(capacity, 1_048_576, "refuse unexpected filesystem capacity");
                session.checkpoint_for_exit(&history(1)).unwrap();
                let before = std::fs::read(session.path()).unwrap();
                std::fs::write(root.join("before.json"), &before).unwrap();
                let filler_path = sessions.join("owned-filler");
                let mut filler = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&filler_path)
                    .unwrap();
                let block = [0_u8; 16_384];
                let mut written = 0_u64;
                let actual_errno = loop {
                    assert!(
                        written <= capacity + block.len() as u64,
                        "bounded fill failed to reach ENOSPC"
                    );
                    match filler.write(&block) {
                        Ok(0) => panic!("zero fill write"),
                        Ok(count) => written += count as u64,
                        Err(error) => break error.raw_os_error(),
                    }
                };
                assert_eq!(actual_errno, Some(libc::ENOSPC));
                let failure = session.checkpoint_for_exit(&history(2)).unwrap_err();
                assert!(
                    matches!(&failure, SessionSaveError::Write(message)
                    if message.contains("os error 28")),
                    "{failure:?}"
                );
                assert_eq!(std::fs::read(session.path()).unwrap(), before);
                assert_eq!(
                    session.save_status(),
                    SessionSaveStatus::Failed(failure.clone())
                );
                assert!(
                    !session
                        .path()
                        .with_extension(format!("json.{}.tmp", std::process::id()))
                        .exists()
                );
                let old = load_for("owned", &workspace).unwrap();
                assert!(same_persisted_history(&old, &history(1)));
                std::fs::write(
                    root.join("disk-full.json"),
                    serde_json::to_vec(&serde_json::json!({
                        "filesystem_magic": stat.f_type,
                        "capacity_bytes": capacity, "filled_bytes": written,
                        "actual_errno": actual_errno, "diagnostic": failure.to_string(),
                        "accepted_messages": 2, "recovered_messages": old.len(),
                        "lost_accepted_messages_before_retry": 1,
                        "lost_accepted_origins_before_retry": 1,
                        "old_snapshot_unchanged": true,
                    }))
                    .unwrap(),
                )
                .unwrap();
                drop(filler);
                std::fs::remove_file(filler_path).unwrap();
                session.checkpoint_for_exit(&history(2)).unwrap();
                assert_eq!(session.save_status(), SessionSaveStatus::Healthy);
                let recovered = load_for("owned", &workspace).unwrap();
                assert!(same_persisted_history(&recovered, &history(2)));
                std::fs::write(
                    root.join("after.json"),
                    std::fs::read(session.path()).unwrap(),
                )
                .unwrap();
                std::fs::write(root.join("disk-full-recovered.json"), serde_json::to_vec(&serde_json::json!({
                    "messages": recovered.len(), "origins": 2,
                    "lost_accepted_messages": 0, "lost_accepted_origins": 0,
                    "retry": "same exact checkpoint_for_exit history after removing only owned filler",
                    "diagnostic_required_no_model": true,
                })).unwrap()).unwrap();
            }
            #[cfg(not(target_os = "linux"))]
            panic!("owned tmpfs ENOSPC requires Linux");
        }
        "backlog" => {
            session.checkpoint_for_exit(&history(1)).unwrap();
            std::fs::write(
                root.join("before.json"),
                std::fs::read(session.path()).unwrap(),
            )
            .unwrap();
            let (sender, receiver) = mpsc::sync_channel(1);
            let first = session
                .queue_history(&sender, Arc::from(history(2)))
                .unwrap();
            let WriterJob::Save(snapshot) = receiver.recv().unwrap() else {
                panic!("save required")
            };
            let (ready, reached) = mpsc::sync_channel(1);
            let (release, resume) = mpsc::sync_channel(1);
            let writer = std::thread::Builder::new()
                .name("session-saver".into())
                .spawn(move || {
                    write_queued_snapshot_observed(snapshot, |phase| {
                        if phase == SessionWritePhase::SyncFile {
                            ready.send(()).unwrap();
                            resume.recv_timeout(Duration::from_secs(5)).unwrap();
                        }
                    });
                })
                .unwrap();
            reached.recv_timeout(Duration::from_secs(2)).unwrap();
            let second = session
                .queue_history(&sender, Arc::from(history(3)))
                .unwrap();
            assert!(matches!(
                session.queue_history(&sender, Arc::from(history(4))),
                Err(SessionSaveError::WriterBacklog)
            ));
            assert_eq!(
                session.save_status(),
                SessionSaveStatus::Failed(SessionSaveError::WriterBacklog)
            );
            let recovered = load_for("owned", &workspace).unwrap();
            assert!(same_persisted_history(&recovered, &history(1)));
            std::fs::write(root.join("backlog.json"), serde_json::to_vec(&serde_json::json!({
                "diagnostic": SessionSaveError::WriterBacklog.to_string(),
                "accepted_messages": 4, "admitted_snapshot_messages": 3,
                "recovered_before_release": recovered.len(),
                "lost_accepted_messages_if_killed_here": 3,
                "lost_accepted_origins_if_killed_here": 3,
                "loss_measurement": "observed loader state; this case releases rather than kills",
            })).unwrap()).unwrap();
            release.send(()).unwrap();
            writer.join().unwrap();
            first.wait(Duration::ZERO).unwrap();
            assert_eq!(
                session.save_status(),
                SessionSaveStatus::Failed(SessionSaveError::WriterBacklog)
            );
            let WriterJob::Save(snapshot) = receiver.recv().unwrap() else {
                panic!("save required")
            };
            std::thread::Builder::new()
                .name("session-saver".into())
                .spawn(move || write_queued_snapshot(snapshot))
                .unwrap()
                .join()
                .unwrap();
            second.wait(Duration::ZERO).unwrap();
            assert_eq!(
                session.save_status(),
                SessionSaveStatus::Failed(SessionSaveError::WriterBacklog)
            );
            session.checkpoint_for_exit(&history(4)).unwrap();
            assert_eq!(session.save_status(), SessionSaveStatus::Healthy);
            assert!(same_persisted_history(
                &load_for("owned", &workspace).unwrap(),
                &history(4)
            ));
            std::fs::write(
                root.join("after.json"),
                std::fs::read(session.path()).unwrap(),
            )
            .unwrap();
        }
        "crash" | "crash-exit" => {
            let phase = request.phase.unwrap();
            let (sender, receiver) = mpsc::sync_channel(1);
            let exit_admission = request.action == "crash-exit";
            let exit_waiter = if exit_admission {
                let exit_session = session.clone();
                Some(std::thread::spawn(move || {
                    exit_session.checkpoint_for_exit_to(&sender, &history(2))
                }))
            } else {
                session
                    .queue_history(&sender, Arc::from(history(2)))
                    .unwrap();
                None
            };
            // For crash-exit only the actual exit method can admit this snapshot.
            // Receiving it establishes entry before a writer can expose the kill marker.
            let WriterJob::Save(snapshot) = receiver.recv_timeout(Duration::from_secs(2)).unwrap()
            else {
                panic!("save required")
            };
            let attempt = snapshot.attempt.clone();
            let marker = root.join("ready.json");
            let writer = std::thread::Builder::new()
                .name("session-saver".into())
                .spawn(move || {
                    write_queued_snapshot_observed(snapshot, |current| {
                        if current.label() == phase {
                            std::fs::write(
                                &marker,
                                serde_json::to_vec(&serde_json::json!({
                                    "phase": phase, "accepted_messages": 2, "accepted_origins": 2,
                                    "attempt_id": attempt.id, "status": "pending",
                                    "actual_exit_admission": exit_admission
                                }))
                                .unwrap(),
                            )
                            .unwrap();
                            // Parent owns and kills this process. A failed runner cannot hang forever.
                            std::thread::sleep(Duration::from_secs(30));
                            panic!("owned fault runner failed to settle child before deadline");
                        }
                    });
                })
                .unwrap();
            writer.join().unwrap();
            if let Some(waiter) = exit_waiter {
                waiter.join().unwrap().unwrap();
            }
            panic!("requested phase was not held");
        }
        _ => panic!("unsupported owned action"),
    }
}
