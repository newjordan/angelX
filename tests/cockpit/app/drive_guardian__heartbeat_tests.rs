//! What the guardian's heartbeat tells its peers. The three activity strings
//! are the wire contract; the state they stand for is what the guardian is
//! doing right now, so a label must never outlive its cause.
use super::*;
use crate::tests::{TestEnvGuard, env_lock};

// The wire strings, spelled out here so a rename on the code side shows up.
const SLEEPING: &str = "guardian-sleeping";
const WATCHING: &str = "guardian-watching";
const RESURRECTING: &str = "guardian-resurrecting";

#[test]
fn the_phase_labels_are_the_strings_peers_match_on() {
    assert_eq!(GuardianPhase::Sleeping.label(), SLEEPING);
    assert_eq!(GuardianPhase::Watching.label(), WATCHING);
    assert_eq!(GuardianPhase::Resurrecting.label(), RESURRECTING);
}

/// A throwaway rig: a state root, a peer directory holding one long-dead
/// instance whose checkout is present here, and a guardian ready to answer it.
struct Rig {
    root: PathBuf,
    state_root: PathBuf,
    peers_dir: PathBuf,
    _peer_env: TestEnvGuard,
    // Restore the environment before releasing the shared lock.
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Rig {
    fn new(tag: &str) -> Self {
        let lock = env_lock();
        let peer_env = TestEnvGuard::unset("ANGEL_PEER_DIR");
        let root = std::env::temp_dir().join(format!(
            "angel-guardian-heartbeat-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let state_root = root.join("state");
        let workspace = root.join("checkout");
        std::fs::create_dir_all(workspace.join(".git")).expect("create checkout");
        std::fs::create_dir_all(&state_root).expect("create state root");
        let peers_dir = peer_dir(&state_root);
        std::fs::create_dir_all(&peers_dir).expect("create peer dir");
        let mut dead = PeerBeat::new(&state_root, &workspace, Some("session-1".to_string()));
        dead.heartbeat.instance_id = "dead-1".to_string();
        dead.heartbeat.updated_ms = 1;
        std::fs::write(
            peers_dir.join("dead-1.json"),
            serde_json::to_vec(&dead.heartbeat).expect("encode heartbeat"),
        )
        .expect("write the dead peer's last heartbeat");
        Self {
            root,
            state_root,
            peers_dir,
            _peer_env: peer_env,
            _lock: lock,
        }
    }

    fn guardian(&self, watch_only: bool) -> Guardian {
        let workspace = self.root.join("checkout");
        let mut beat = PeerBeat::new(&self.state_root, &workspace, None);
        beat.set_activity(SLEEPING, None);
        Guardian::new(
            self.state_root.clone(),
            self.peers_dir.clone(),
            180_000,
            watch_only,
            None,
            beat,
        )
    }

    /// What peers read off this guardian's heartbeat file.
    fn published_activity(&self, guardian: &Guardian) -> String {
        read_peers(&self.peers_dir, None)
            .into_iter()
            .find(|peer| peer.instance_id == guardian.beat.heartbeat.instance_id)
            .map(|peer| peer.activity)
            .expect("the guardian's heartbeat is published")
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn a_failed_spawn_does_not_leave_the_guardian_resurrecting() {
    let rig = Rig::new("spawn-failed");
    // The resurrection log directory cannot be made: a file sits where it goes.
    std::fs::write(rig.state_root.join("guardian"), b"in the way").expect("block the log dir");
    let mut guardian = rig.guardian(false);

    guardian.turn();

    assert!(guardian.child.is_none(), "nothing was spawned");
    let record = guard::read_record(&rig.peers_dir, "dead-1").expect("the failure is recorded");
    assert!(
        !record.success && record.summary.starts_with("spawn failed"),
        "{record:?}"
    );
    assert_eq!(guardian.beat.heartbeat.activity, SLEEPING);
    // The next scan finds the death answered; peers still read an idle guardian.
    guardian.turn();
    assert_eq!(rig.published_activity(&guardian), SLEEPING);
}

#[test]
fn a_watch_only_guardian_shows_watching_while_a_corpse_is_there_and_sleeps_after() {
    let rig = Rig::new("watch-only");
    let mut guardian = rig.guardian(true);

    guardian.turn();
    assert_eq!(guardian.beat.heartbeat.activity, WATCHING);
    guardian.turn();
    assert_eq!(guardian.beat.heartbeat.activity, WATCHING);

    std::fs::remove_file(rig.peers_dir.join("dead-1.json")).expect("the peer is gone");
    guardian.turn();
    assert_eq!(guardian.beat.heartbeat.activity, SLEEPING);
}

#[test]
fn a_finished_child_returns_the_guardian_to_sleeping() {
    let rig = Rig::new("child-exited");
    let mut guardian = rig.guardian(true);
    let mut finished = std::process::Command::new("true")
        .spawn()
        .expect("spawn a child that exits at once");
    finished.wait().expect("the child exits");
    let candidate = guard::Candidate {
        instance_id: "dead-1".to_string(),
        host: "rig".to_string(),
        session_id: "session-1".to_string(),
        workspace: rig.root.join("checkout"),
        last_seen_ms: 1,
    };
    guardian.child = Some((finished, candidate, now_ms()));
    guardian.beat.set_activity(RESURRECTING, None);

    guardian.turn();

    assert!(guardian.child.is_none());
    assert_eq!(guardian.beat.heartbeat.activity, SLEEPING);
}

#[test]
fn a_running_child_shows_resurrecting() {
    let rig = Rig::new("child-running");
    let mut guardian = rig.guardian(true);
    let running = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn a child that keeps running");
    let candidate = guard::Candidate {
        instance_id: "dead-1".to_string(),
        host: "rig".to_string(),
        session_id: "session-1".to_string(),
        workspace: rig.root.join("checkout"),
        last_seen_ms: 1,
    };
    guardian.child = Some((running, candidate, now_ms()));

    guardian.turn();

    assert_eq!(guardian.beat.heartbeat.activity, RESURRECTING);
    if let Some((mut child, _, _)) = guardian.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}
