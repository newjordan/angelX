//! Peer heartbeat, staleness, and watcher-diff behavior.
use super::*;

fn heartbeat(id: &str, host: &str, activity: &str, updated_ms: u128) -> Heartbeat {
    Heartbeat {
        schema: HEARTBEAT_SCHEMA.to_string(),
        instance_id: id.to_string(),
        host: host.to_string(),
        pid: 4242,
        version: "0.0.0".to_string(),
        started_ms: updated_ms.saturating_sub(60_000),
        updated_ms,
        workspace: "/home/user/project".to_string(),
        session_id: None,
        activity: activity.to_string(),
        detail: None,
        cpu_pct: 25.0,
        mem_pct: 40.0,
        gpu_pct: 10.0,
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "angel-peer-test-{}-{}-{serial}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn put(&self, beat: &Heartbeat) {
        std::fs::write(
            self.0.join(format!("{}.json", beat.instance_id)),
            serde_json::to_vec(beat).unwrap(),
        )
        .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn read_peers_skips_own_foreign_and_torn_files() {
    let fixture = Fixture::new();
    fixture.put(&heartbeat("a-1", "casper", "turn", 1_000_000));
    fixture.put(&heartbeat("b-2", "rig2", "idle", 999_000));
    std::fs::write(fixture.0.join("a-1.json.tmp"), b"torn {").unwrap();
    std::fs::write(fixture.0.join("junk.json"), b"{\"not\":\"a heartbeat\"}").unwrap();
    std::fs::write(fixture.0.join("notes.txt"), b"hello").unwrap();
    let peers = read_peers(&fixture.0, Some("b-2"));
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].instance_id, "a-1");
    assert_eq!(peers[0].host, "casper");
}

#[test]
fn watcher_reports_appear_change_and_gone_exactly_once() {
    let mut watcher = PeerWatcher::new();
    let t0: u128 = 10_000_000;
    let events = watcher.observe(&[heartbeat("a-1", "casper", "idle", t0)], t0);
    assert_eq!(
        events,
        vec![PeerEvent::Appeared {
            instance_id: "a-1".into(),
            host: "casper".into(),
            activity: "idle".into(),
        }]
    );
    // Steady state: no repeat events.
    assert!(
        watcher
            .observe(
                &[heartbeat("a-1", "casper", "idle", t0 + 5_000)],
                t0 + 5_000
            )
            .is_empty()
    );
    // Activity transition.
    let events = watcher.observe(
        &[heartbeat("a-1", "casper", "turn", t0 + 10_000)],
        t0 + 10_000,
    );
    assert_eq!(
        events,
        vec![PeerEvent::ActivityChanged {
            instance_id: "a-1".into(),
            host: "casper".into(),
            from: "idle".into(),
            to: "turn".into(),
        }]
    );
    // Heartbeat ages past the stale horizon: Gone, once.
    let stale_at = t0 + 10_000 + STALE_AFTER_MS + 1;
    let events = watcher.observe(&[heartbeat("a-1", "casper", "turn", t0 + 10_000)], stale_at);
    assert_eq!(
        events,
        vec![PeerEvent::Gone {
            instance_id: "a-1".into(),
            host: "casper".into(),
        }]
    );
    assert!(watcher.observe(&[], stale_at + 5_000).is_empty());
}

#[test]
fn fleet_means_ignore_stale_peers() {
    let t0: u128 = 10_000_000;
    let mut live = heartbeat("a-1", "casper", "turn", t0);
    live.cpu_pct = 50.0;
    live.gpu_pct = 90.0;
    let mut stale = heartbeat("b-2", "rig2", "turn", t0 - STALE_AFTER_MS - 1_000);
    stale.cpu_pct = 100.0;
    stale.gpu_pct = 100.0;
    let (cpu, gpu) = fleet_means(&[live, stale], t0).unwrap();
    assert_eq!(cpu, 50.0);
    assert_eq!(gpu, 90.0);
    assert!(fleet_means(&[], t0).is_none());
}

#[test]
fn beat_publishes_and_retire_removes_the_heartbeat_file() {
    let fixture = Fixture::new();
    let state = fixture.0.join("state");
    std::fs::create_dir_all(&state).unwrap();
    // Point the slot at a fixture directory through the config file path
    // peer_dir reads (peers.json under the state root).
    let shared = fixture.0.join("shared");
    std::fs::write(
        state.join("peers.json"),
        serde_json::json!({"schema": PEERS_CONFIG_SCHEMA, "dir": shared.to_string_lossy()})
            .to_string(),
    )
    .unwrap();
    let mut beat = PeerBeat::new(&state, Path::new("/home/user/project"), None);
    beat.beat(10.0, 20.0, 0.0);
    let peers = read_peers(&shared, None);
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].instance_id, beat.heartbeat.instance_id);
    assert_eq!(peers[0].cpu_pct, 10.0);
    beat.retire();
    assert!(read_peers(&shared, None).is_empty());
}
