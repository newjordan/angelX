//! Guardian fencing: candidate selection and atomic claims.
use super::*;
use std::process::{Command, Stdio};
use std::time::Duration;

fn heartbeat(
    id: &str,
    activity: &str,
    session: Option<&str>,
    workspace: &str,
    updated_ms: u128,
) -> Heartbeat {
    Heartbeat {
        schema: "angelX.heartbeat/v1".to_string(),
        instance_id: id.to_string(),
        host: "rig".to_string(),
        pid: 7,
        version: "0.0.0".to_string(),
        started_ms: 0,
        updated_ms,
        workspace: workspace.to_string(),
        session_id: session.map(str::to_string),
        activity: activity.to_string(),
        detail: None,
        cpu_pct: 0.0,
        mem_pct: 0.0,
        gpu_pct: 0.0,
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "angel-guardian-test-{}-{}-{serial}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const T0: u128 = 10_000_000;
const STALE: u128 = 180_000;

fn candidate() -> Candidate {
    Candidate {
        instance_id: "dead-1".to_string(),
        host: "rig".to_string(),
        session_id: "s-1".to_string(),
        workspace: PathBuf::from("/work/a"),
        last_seen_ms: T0,
    }
}

/// The clock every expiry test reasons against. Claim ages are explicit
/// literals below, never wall-clock arithmetic.
const NOW_MS: u128 = 1_750_000_000_000;

fn set_mtime_ms(path: &Path, modified_ms: u128) {
    let modified = std::time::UNIX_EPOCH + Duration::from_millis(modified_ms as u64);
    let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
}

fn put_claim(peer_dir: &Path, instance: &str, guardian: &str, claimed_ms: u128, mtime_ms: u128) {
    let path = peer_dir.join(format!("{instance}.claim.json"));
    std::fs::write(
        &path,
        serde_json::json!({
            "guardian_instance": guardian,
            "guardian_host": "rig",
            "guardian_pid": 11,
            "session_id": "s-1",
            "claimed_ms": claimed_ms,
        })
        .to_string(),
    )
    .unwrap();
    set_mtime_ms(&path, mtime_ms);
}

/// Wait for a child without sleeping. A joined waiter plus a bounded
/// `recv_timeout` leaves the child killable by pid if the test hangs.
fn wait_bounded(
    child: std::process::Child,
    timeout: Duration,
    label: &str,
) -> std::process::ExitStatus {
    let pid = child.id();
    let (sender, receiver) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let mut child = child;
        let _ = sender.send(child.wait());
    });
    match receiver.recv_timeout(timeout) {
        Ok(status) => {
            let _ = waiter.join();
            status.unwrap()
        }
        Err(_) => {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
            let _ = waiter.join();
            panic!("{label} child timed out after {timeout:?}");
        }
    }
}

/// A filter that matches nothing still exits successfully; the harness
/// output must prove the one child test actually ran and passed.
fn assert_single_test_passed(log_path: &Path, test_name: &str) {
    let log = std::fs::read_to_string(log_path).unwrap();
    assert!(
        log.contains("running 1 test"),
        "child ran the wrong filter: {log}"
    );
    assert!(log.contains(test_name), "{log}");
    assert!(log.contains("test result: ok. 1 passed; 0 failed"), "{log}");
}

fn claim_guardian(peer_dir: &Path, instance: &str) -> Option<String> {
    let raw = std::fs::read(peer_dir.join(format!("{instance}.claim.json"))).ok()?;
    let claim: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    claim["guardian_instance"].as_str().map(str::to_string)
}

fn put_record(peer_dir: &Path, instance: &str, old_session: &str) {
    std::fs::write(
        peer_dir.join(format!("{instance}.resurrection.json")),
        serde_json::json!({
            "dead_instance": instance,
            "dead_host": "rig",
            "old_session_id": old_session,
            "new_session_id": "s-recovered",
            "guardian_host": "rig-b",
            "started_ms": 1,
            "finished_ms": 2,
            "success": true,
            "summary": "repaired and continued",
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn only_dead_session_holders_with_local_workspaces_are_candidates() {
    let peers = vec![
        // Breathing: never a candidate.
        heartbeat("live-1", "turn", Some("s-live"), "/work/a", T0),
        // Dead with a session and a workspace here: the candidate.
        heartbeat("dead-1", "turn", Some("s-dead"), "/work/a", T0 - STALE - 1),
        // Dead but sessionless: nothing to resurrect.
        heartbeat("dead-2", "idle", None, "/work/a", T0 - STALE - 1),
        // Dead with a session, but its checkout is not on this rig.
        heartbeat(
            "dead-3",
            "turn",
            Some("s-far"),
            "/work/elsewhere",
            T0 - STALE - 1,
        ),
        // Dead guardian: giants do not resurrect giants.
        heartbeat(
            "dead-4",
            "guardian-sleeping",
            Some("s-g"),
            "/work/a",
            T0 - STALE - 1,
        ),
        // Dead, but already answered.
        heartbeat("dead-5", "turn", Some("s-done"), "/work/a", T0 - STALE - 1),
    ];
    let candidates = select_candidates(
        &peers,
        T0,
        STALE,
        |path| path == Path::new("/work/a"),
        |_, session| session == "s-done",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].instance_id, "dead-1");
    assert_eq!(candidates[0].session_id, "s-dead");
}

#[test]
fn a_live_holder_of_the_same_session_blocks_resurrection() {
    let peers = vec![
        heartbeat("dead-1", "turn", Some("s-1"), "/work/a", T0 - STALE - 1),
        // The supposedly-dead session is alive under a new instance.
        heartbeat("live-2", "turn", Some("s-1"), "/work/a", T0),
    ];
    let candidates = select_candidates(&peers, T0, STALE, |_| true, |_, _| false);
    assert!(candidates.is_empty(), "{candidates:?}");
}

#[test]
fn claim_is_won_by_exactly_one_guardian() {
    let fixture = Fixture::new();
    let candidate = Candidate {
        instance_id: "dead-1".to_string(),
        host: "rig".to_string(),
        session_id: "s-1".to_string(),
        workspace: PathBuf::from("/work/a"),
        last_seen_ms: 0,
    };
    assert!(try_claim(&fixture.0, &candidate, "guardian-a", "rig-a").unwrap());
    assert!(!try_claim(&fixture.0, &candidate, "guardian-b", "rig-b").unwrap());
    // Even the winner cannot double-claim its own fresh claim.
    assert!(!try_claim(&fixture.0, &candidate, "guardian-a", "rig-a").unwrap());
}

#[test]
fn record_roundtrips_and_blocks_reresurrection() {
    let fixture = Fixture::new();
    let record = ResurrectionRecord {
        dead_instance: "dead-1".to_string(),
        dead_host: "rig".to_string(),
        old_session_id: "s-1".to_string(),
        new_session_id: Some("s-2".to_string()),
        guardian_host: "rig-b".to_string(),
        started_ms: 1,
        finished_ms: 2,
        success: true,
        summary: "repaired and continued".to_string(),
    };
    write_record(&fixture.0, &record).unwrap();
    let read = read_record(&fixture.0, "dead-1").unwrap();
    assert_eq!(read.new_session_id.as_deref(), Some("s-2"));
    assert!(read.success);
    let peers = vec![heartbeat(
        "dead-1",
        "turn",
        Some("s-1"),
        "/work/a",
        T0 - STALE - 1,
    )];
    let candidates = select_candidates(
        &peers,
        T0,
        STALE,
        |_| true,
        |instance, session| {
            read_record(&fixture.0, instance).is_some_and(|record| record.old_session_id == session)
        },
    );
    assert!(candidates.is_empty());
}

#[test]
fn read_records_lists_every_record_newest_first() {
    let fixture = Fixture::new();
    assert!(read_records(&fixture.0).is_empty());
    let make = |instance: &str, finished_ms: u128, success: bool| ResurrectionRecord {
        dead_instance: instance.to_string(),
        dead_host: "rig".to_string(),
        old_session_id: format!("s-{instance}"),
        new_session_id: success.then(|| format!("s-{instance}-2")),
        guardian_host: "rig-b".to_string(),
        started_ms: finished_ms - 1,
        finished_ms,
        success,
        summary: "outcome".to_string(),
    };
    write_record(&fixture.0, &make("dead-old", 10, true)).unwrap();
    write_record(&fixture.0, &make("dead-new", 30, false)).unwrap();
    write_record(&fixture.0, &make("dead-mid", 20, true)).unwrap();
    // Heartbeats and claims in the same directory are not records.
    std::fs::write(fixture.0.join("some-peer.json"), "{}").unwrap();
    std::fs::write(fixture.0.join("dead-old.claim.json"), "{}").unwrap();
    let records = read_records(&fixture.0);
    let order: Vec<&str> = records
        .iter()
        .map(|record| record.dead_instance.as_str())
        .collect();
    assert_eq!(order, ["dead-new", "dead-mid", "dead-old"]);
    assert!(!records[0].success && records[0].new_session_id.is_none());
}

#[test]
fn concurrent_guardians_claiming_an_unclaimed_death_have_exactly_one_winner() {
    let fixture = Fixture::new();
    let candidate = candidate();
    const GUARDIANS: usize = 8;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(GUARDIANS));
    let mut results = Vec::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..GUARDIANS)
            .map(|n| {
                let peer_dir = fixture.0.clone();
                let candidate = candidate.clone();
                let barrier = std::sync::Arc::clone(&barrier);
                scope.spawn(move || {
                    barrier.wait();
                    try_claim_at(
                        &peer_dir,
                        &candidate,
                        &format!("guardian-{n}"),
                        "rig",
                        NOW_MS,
                    )
                    .map(|won| (won, n))
                })
            })
            .collect();
        for handle in handles {
            results.push(handle.join().unwrap());
        }
    });
    let winners: Vec<usize> = results
        .iter()
        .filter_map(|result| {
            let (won, n) = result.as_ref().expect("claim attempts must not error");
            won.then_some(*n)
        })
        .collect();
    assert_eq!(winners.len(), 1, "{results:?}");
    assert_eq!(
        claim_guardian(&fixture.0, "dead-1").as_deref(),
        Some(format!("guardian-{}", winners[0]).as_str())
    );
    let names: Vec<String> = std::fs::read_dir(&fixture.0)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["dead-1.claim.json".to_string()], "{names:?}");
}

#[test]
fn the_claim_ttl_is_thirty_minutes() {
    assert_eq!(CLAIM_TTL_MS, 30 * 60 * 1000);
}

#[test]
fn an_expired_claim_without_an_outcome_is_stolen_but_a_fresh_one_is_not() {
    let fixture = Fixture::new();
    let candidate = candidate();
    // 29m59s is still inside the window: the slow guardian owns it.
    put_claim(
        &fixture.0,
        "dead-1",
        "slow-guardian",
        NOW_MS - 1_799_000,
        NOW_MS - 1_799_000,
    );
    assert!(!try_claim_at(&fixture.0, &candidate, "other-guardian", "rig-b", NOW_MS).unwrap());
    assert_eq!(
        claim_guardian(&fixture.0, "dead-1").as_deref(),
        Some("slow-guardian")
    );

    // 30m01s with no record: the claimant is presumed dead too.
    put_claim(
        &fixture.0,
        "dead-1",
        "dead-guardian",
        NOW_MS - 1_801_000,
        NOW_MS - 1_801_000,
    );
    assert!(try_claim_at(&fixture.0, &candidate, "next-guardian", "rig-b", NOW_MS).unwrap());
    assert_eq!(
        claim_guardian(&fixture.0, "dead-1").as_deref(),
        Some("next-guardian")
    );
    let stolen: Vec<String> = std::fs::read_dir(&fixture.0)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("claim-stolen"))
        .collect();
    assert!(stolen.is_empty(), "steal litter left behind: {stolen:?}");
}

#[test]
fn a_torn_claim_older_than_the_ttl_is_recovered_but_a_fresh_torn_one_is_not() {
    let fixture = Fixture::new();
    let candidate = candidate();
    let path = fixture.0.join("dead-1.claim.json");
    // A guardian that died between create_new and its body write leaves this.
    std::fs::write(&path, b"{\"guardian_instance\":\"half").unwrap();
    set_mtime_ms(&path, NOW_MS - 1_799_000);
    assert!(!try_claim_at(&fixture.0, &candidate, "other-guardian", "rig-b", NOW_MS).unwrap());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"{\"guardian_instance\":\"half"
    );

    set_mtime_ms(&path, NOW_MS - 1_801_000);
    assert!(try_claim_at(&fixture.0, &candidate, "recoverer", "rig-b", NOW_MS).unwrap());
    assert_eq!(
        claim_guardian(&fixture.0, "dead-1").as_deref(),
        Some("recoverer")
    );
}

#[test]
fn an_expired_claim_with_an_outcome_record_is_never_stolen() {
    let fixture = Fixture::new();
    let candidate = candidate();
    put_claim(
        &fixture.0,
        "dead-1",
        "old-guardian",
        NOW_MS - 1_801_000,
        NOW_MS - 1_801_000,
    );
    put_record(&fixture.0, "dead-1", "s-1");
    assert!(!try_claim_at(&fixture.0, &candidate, "other-guardian", "rig-b", NOW_MS).unwrap());
    assert_eq!(
        claim_guardian(&fixture.0, "dead-1").as_deref(),
        Some("old-guardian")
    );
}

#[test]
fn a_record_without_a_claim_answers_the_same_death_but_not_a_later_one() {
    let fixture = Fixture::new();
    put_record(&fixture.0, "dead-1", "s-1");
    assert!(!try_claim_at(&fixture.0, &candidate(), "guardian", "rig", NOW_MS).unwrap());
    assert!(!fixture.0.join("dead-1.claim.json").exists());

    // The same instance dying later in a different session is a new death.
    let later = Candidate {
        session_id: "s-2".to_string(),
        ..candidate()
    };
    assert!(try_claim_at(&fixture.0, &later, "guardian", "rig", NOW_MS).unwrap());
    assert_eq!(
        claim_guardian(&fixture.0, "dead-1").as_deref(),
        Some("guardian")
    );
}

#[test]
fn a_record_for_another_session_does_not_fence_a_stale_claim() {
    let fixture = Fixture::new();
    put_claim(
        &fixture.0,
        "dead-1",
        "old-guardian",
        NOW_MS - 1_801_000,
        NOW_MS - 1_801_000,
    );
    // The record answers s-1; the candidate is the instance's later death.
    put_record(&fixture.0, "dead-1", "s-1");
    let later = Candidate {
        session_id: "s-2".to_string(),
        ..candidate()
    };
    assert!(try_claim_at(&fixture.0, &later, "new-guardian", "rig-b", NOW_MS).unwrap());
    assert_eq!(
        claim_guardian(&fixture.0, "dead-1").as_deref(),
        Some("new-guardian")
    );
}

#[test]
fn dead_guardians_and_resurrecting_children_are_never_candidates() {
    for activity in [
        "guardian-sleeping",
        "guardian-watching",
        "guardian-resurrecting",
        "resurrecting",
    ] {
        let peers = vec![heartbeat(
            "dead-guardian",
            activity,
            Some("s-giant"),
            "/work/a",
            T0 - STALE - 1,
        )];
        let candidates = select_candidates(&peers, T0, STALE, |_| true, |_, _| false);
        assert!(
            candidates.is_empty(),
            "activity {activity:?} became a candidate"
        );
    }
}

const WATCH_ONLY_CHILD: &str = "ANGEL_T_GUARDIAN_WATCH_ONLY_CHILD";

#[test]
fn watch_only_logs_an_eligible_death_and_never_claims_while_guardians_are_excluded() {
    if std::env::var_os(WATCH_ONLY_CHILD).is_some() {
        crate::drive::guardian::cli_guardian(vec![
            "--watch-only".to_string(),
            "--once".to_string(),
        ])
        .unwrap();
        return;
    }
    let fixture = Fixture::new();
    let home = fixture.0.join("home");
    let peers = fixture.0.join("peers");
    let workspace = fixture.0.join("workspace");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&peers).unwrap();
    std::fs::create_dir_all(workspace.join(".git")).unwrap();
    let workspace = workspace.to_string_lossy().into_owned();
    let dead_at = now_ms().saturating_sub(DEFAULT_STALE_MS + 60_000);
    for beat in [
        heartbeat("dead-1", "turn", Some("s-dead"), &workspace, dead_at),
        heartbeat(
            "giant-1",
            "guardian-sleeping",
            Some("s-giant"),
            &workspace,
            dead_at,
        ),
    ] {
        std::fs::write(
            peers.join(format!("{}.json", beat.instance_id)),
            serde_json::to_vec(&beat).unwrap(),
        )
        .unwrap();
    }
    let log_path = fixture.0.join("child.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "platform::guardian::tests::watch_only_logs_an_eligible_death_and_never_claims_while_guardians_are_excluded",
            "--test-threads=1",
            "--nocapture",
        ])
        .env("HOME", &home)
        .env("ANGEL_PEER_DIR", &peers)
        .env(WATCH_ONLY_CHILD, &fixture.0)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let status = wait_bounded(child, Duration::from_secs(10), "watch-only");
    assert!(
        status.success(),
        "{}",
        std::fs::read_to_string(&log_path).unwrap()
    );
    assert_single_test_passed(
        &log_path,
        "platform::guardian::tests::watch_only_logs_an_eligible_death_and_never_claims_while_guardians_are_excluded",
    );

    let claims: Vec<String> = std::fs::read_dir(&peers)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".claim.json"))
        .collect();
    assert!(claims.is_empty(), "watch-only claimed: {claims:?}");
    assert!(read_record(&peers, "dead-1").is_none());
    let guardian_log = std::fs::read_to_string(home.join(".angelX/guardian/guardian.log")).unwrap();
    assert!(
        guardian_log.contains("watch-only: dead-1"),
        "{guardian_log}"
    );
    assert!(!guardian_log.contains("giant-1"), "{guardian_log}");
}

#[cfg(unix)]
#[test]
fn failed_claim_write_leaves_no_claim_to_fence_the_instance() {
    const CHILD_ROOT: &str = "ANGEL_T_GUARDIAN_CLAIM_EFBIG_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = PathBuf::from(root);
        struct RestoreLimit {
            limit: libc::rlimit,
            signal: libc::sighandler_t,
        }
        impl Drop for RestoreLimit {
            fn drop(&mut self) {
                unsafe {
                    let _ = libc::setrlimit(libc::RLIMIT_FSIZE, &self.limit);
                    libc::signal(libc::SIGXFSZ, self.signal);
                }
            }
        }
        let restore = unsafe {
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            assert_eq!(libc::getrlimit(libc::RLIMIT_FSIZE, &mut limit), 0);
            let signal = libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
            assert_ne!(signal, libc::SIG_ERR);
            RestoreLimit { limit, signal }
        };
        let limited = libc::rlimit {
            rlim_cur: 0,
            rlim_max: restore.limit.rlim_max,
        };
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limited) }, 0);
        let mut candidate = candidate();
        candidate.instance_id = "dead-1".to_string();
        let error = try_claim(&root, &candidate, "guardian", "rig").unwrap_err();
        assert!(error.starts_with("write claim"), "{error}");
        drop(restore);
        assert!(
            !root.join("dead-1.claim.json").exists(),
            "a partial claim must not fence the instance"
        );
        assert!(try_claim(&root, &candidate, "guardian", "rig").unwrap());
        return;
    }
    let fixture = Fixture::new();
    let log_path = fixture.0.join("child.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "platform::guardian::tests::failed_claim_write_leaves_no_claim_to_fence_the_instance",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(CHILD_ROOT, &fixture.0)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let status = wait_bounded(child, Duration::from_secs(10), "claim EFBIG");
    assert!(
        status.success(),
        "{}",
        std::fs::read_to_string(&log_path).unwrap()
    );
    assert_single_test_passed(
        &log_path,
        "platform::guardian::tests::failed_claim_write_leaves_no_claim_to_fence_the_instance",
    );
}
