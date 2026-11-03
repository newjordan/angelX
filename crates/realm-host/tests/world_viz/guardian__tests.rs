//! The sleeping giant's state derivation and poses.
use super::*;
use angel_live::host::peer_watch::Heartbeat;

fn peer(activity: &str, updated_ms: u128) -> Heartbeat {
    Heartbeat {
        schema: "angelX.heartbeat/v1".to_string(),
        instance_id: format!("{activity}-1"),
        host: "rig".to_string(),
        pid: 7,
        version: "0.0.0".to_string(),
        started_ms: 0,
        updated_ms,
        workspace: "/work".to_string(),
        session_id: Some("s-1".to_string()),
        activity: activity.to_string(),
        detail: None,
        cpu_pct: 0.0,
        mem_pct: 0.0,
        gpu_pct: 0.0,
    }
}

const T0: u128 = 10_000_000;

#[test]
fn no_guardian_no_giant() {
    assert_eq!(guardian_state(&[], T0), GuardianState::Absent);
    // A fallen knight alone does not conjure a giant into the realm.
    assert_eq!(
        guardian_state(&[peer("turn", T0 - 200_000)], T0),
        GuardianState::Absent
    );
}

#[test]
fn giant_sleeps_watches_and_resurrects() {
    let sleeping = vec![peer("guardian-sleeping", T0), peer("turn", T0)];
    assert_eq!(guardian_state(&sleeping, T0), GuardianState::Sleeping);

    let fallen = vec![peer("guardian-watching", T0), peer("turn", T0 - 200_000)];
    assert_eq!(guardian_state(&fallen, T0), GuardianState::Watching);

    let raising = vec![
        peer("guardian-resurrecting", T0),
        peer("resurrecting", T0),
        peer("turn", T0 - 200_000),
    ];
    assert_eq!(guardian_state(&raising, T0), GuardianState::Resurrecting);
}

#[test]
fn giant_pose_dwarfs_a_villager() {
    for state in [
        GuardianState::Sleeping,
        GuardianState::Watching,
        GuardianState::Resurrecting,
    ] {
        let (width, height, _) = giant_pose(state).expect("pose");
        assert!(width > 0.30 && height > 0.58, "{state:?}: {width}x{height}");
        assert!(giant_position(state, (10, 10), Some((20, 20))).is_some());
    }
    assert!(giant_pose(GuardianState::Absent).is_none());
    // Kneeling happens at the granary, not the keep.
    let pos = giant_position(GuardianState::Resurrecting, (10, 10), Some((20, 20))).unwrap();
    assert_eq!(pos, (20.5, 21.4));
}
