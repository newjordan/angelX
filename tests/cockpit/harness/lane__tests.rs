use super::*;

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::tests::env_lock()
}

struct EnvGuard {
    key: &'static str,
    prev: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: Option<&str>) -> Self {
        let prev = std::env::var_os(key);
        match value {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var(key, v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(key) },
        }
        Self { key, prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.prev {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn file_identity(path: &Path) -> (std::time::SystemTime, u64) {
    let meta = std::fs::metadata(path).unwrap();
    (meta.modified().unwrap(), meta.len())
}

fn overwrite_preserving_identity(path: &Path, bytes: &[u8]) {
    let (mtime, len) = file_identity(path);
    assert_eq!(len, bytes.len() as u64, "replacement must keep len");
    std::fs::write(path, bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    assert_eq!(file_identity(path), (mtime, len));
}

fn bump_mtime(path: &Path) {
    let bumped = file_identity(path).0 + std::time::Duration::from_secs(2);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(bumped)
        .unwrap();
}

#[test]
fn parses_treebeard_aliases() {
    assert_eq!(Lane::parse("treebeard"), Lane::Treebeard);
    assert_eq!(Lane::parse("RLM"), Lane::Treebeard);
    assert_eq!(Lane::parse("hi/q"), Lane::Treebeard);
    assert_eq!(Lane::parse("default"), Lane::Default);
    assert_eq!(Lane::parse(""), Lane::Default);
}

#[test]
fn treebeard_enables_handle_read_unless_forced_off() {
    let _g = env_lock();
    let _lane = EnvGuard::set("ANGEL_LANE", Some("treebeard"));
    let _store = EnvGuard::set("ANGEL_HANDLE_STORE", Some("1"));
    let _read = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", None);
    assert!(handle_read_tool_enabled());

    let _off = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", Some("0"));
    assert!(!handle_read_tool_enabled());
}

#[test]
fn treebeard_lowers_offload_floors_when_env_unset() {
    let _g = env_lock();
    let _cm = EnvGuard::set("ANGEL_HANDLE_CODE_MODE_MIN_BYTES", None);
    let _sc = EnvGuard::set("ANGEL_HANDLE_SUBCALL_MIN_BYTES", None);

    let _def = EnvGuard::set("ANGEL_LANE", Some("default"));
    assert_eq!(code_mode_offload_min_bytes(), 4096);
    assert_eq!(lane_subcall_offload_min_bytes(), 2048);

    let _tb = EnvGuard::set("ANGEL_LANE", None);
    assert_eq!(code_mode_offload_min_bytes(), 1024);
    assert_eq!(lane_subcall_offload_min_bytes(), 512);

    let _explicit = EnvGuard::set("ANGEL_HANDLE_CODE_MODE_MIN_BYTES", Some("9999"));
    assert_eq!(code_mode_offload_min_bytes(), 9999);
}

#[test]
fn treebeard_system_block_is_nonempty_only_on_lane() {
    let _g = env_lock();
    let _off = EnvGuard::set("ANGEL_LANE", Some("default"));
    assert!(lane_system_suffix().is_empty());
    let _on = EnvGuard::set("ANGEL_LANE", None);
    assert!(lane_system_suffix().contains("treebeard lane"));
    assert!(lane_system_suffix().contains("handle_read"));
    assert!(lane_system_suffix().contains("handle_put"));
    assert!(!lane_system_suffix().contains("popcorn-submit-hiq"));
    assert!(!lane_system_suffix().contains("B6LDP"));
}

#[test]
fn living_competition_suffix_reads_peer_state() {
    let _g = env_lock();
    let _lane_off = EnvGuard::set("ANGEL_LANE", Some("default"));
    assert!(living_competition_system_suffix().is_empty());

    let peer = std::env::temp_dir().join(format!(
        "angel-peer-test-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(
            &peer,
            r#"{"geomean_us":867.912,"name":"b200_c3_peer.txt","path":"/tmp/c3.txt","p1_us":1684.5,"shapes":{"32768x1":38800.0,"512x640":1684.5,"128x256":61.2,"256x64":144.0},"shape_bests":{"32768x1":{"us":38300.0,"name":"b200_r7"},"128x256":{"us":60.9,"name":"multi128"},"256x64":{"us":130.0,"name":"r7v2"}}}"#,
        )
        .unwrap();
    let _peer = EnvGuard::set(
        "POPCORN_PEER_STATE",
        Some(peer.to_str().expect("utf8 path")),
    );
    let _lane = EnvGuard::set("ANGEL_LANE", None);
    let _gpu = EnvGuard::set("ANGEL_GPU_COMP_LOCAL_MOA", Some("1"));
    let s = living_competition_system_suffix();
    assert_eq!(
        load_living_peer_p1_us().map(|u| (u * 10.0).round() / 10.0),
        Some(1684.5)
    );
    let holds = load_living_peer_shape_holds(3);
    assert!(
        holds.iter().any(|h| h.key == "256x64"),
        "shape holds include 256x64: {holds:?}"
    );
    let open = load_living_peer_open_levers(3);
    assert!(
        !open.is_empty() && open[0].key == "32768x1",
        "highest board µs first: {open:?}"
    );
    assert!(
        s.contains("PRIMARY HOLD floor") && s.contains("38300") && s.contains("NEW_HOLD"),
        "root names PRIMARY HOLD densify floor: {s}"
    );
    assert!(
        open[0].geo_drop_if_half_pct > 0.0,
        "half-geo impact on open levers: {open:?}"
    );
    let _ = std::fs::remove_file(&peer);
    assert!(s.contains("867.91"), "got: {s}");
    assert!(s.contains("living B200 peer"), "got: {s}");
    assert!(s.contains("512"), "P1 open lever mentioned: {s}");
    assert!(
        s.contains("Primary attack") && s.contains("512x640"),
        "root names primary attack lever: {s}"
    );
    assert!(
        s.contains("½geo↓") || s.contains("half-cut"),
        "root names geomean impact: {s}"
    );
    assert!(
        s.contains("1684") || s.contains("1685"),
        "dynamic P1 peer shape in suffix: {s}"
    );
    assert!(
        s.contains("Open levers") && s.contains("512x640"),
        "root names open levers by board µs: {s}"
    );
    assert!(
        s.contains("Shape holds") && s.contains("256x64"),
        "root names shape holds: {s}"
    );
}

#[test]
fn living_peer_json_caches_by_mtime_len() {
    let _g = env_lock();
    let peer = std::env::temp_dir().join(format!(
        "angel-peer-cache-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(
            &peer,
            r#"{"geomean_us":867.912,"name":"b200_c3_peer.txt","path":"/tmp/c3.txt","p1_us":1684.5,"shapes":{"32768x1":38800.0,"512x640":1684.5}}"#,
        )
        .unwrap();
    let _peer = EnvGuard::set(
        "POPCORN_PEER_STATE",
        Some(peer.to_str().expect("utf8 path")),
    );
    let first = load_living_peer_snapshot().expect("first load");
    assert_eq!(first.1, "b200_c3_peer.txt");
    let first_p1 = load_living_peer_p1_us();
    let first_open = load_living_peer_open_levers(1);

    let original = std::fs::read(&peer).unwrap();
    let mut garbage = vec![b'x'; original.len()];
    let last = garbage.len() - 1;
    garbage[0] = b'{';
    garbage[last] = b'}';
    overwrite_preserving_identity(&peer, &garbage);
    assert_eq!(
        load_living_peer_snapshot().expect("cache hit"),
        first,
        "unchanged mtime/len must keep the cached snapshot even if bytes changed"
    );
    assert_eq!(load_living_peer_p1_us(), first_p1);
    assert_eq!(load_living_peer_open_levers(1), first_open);

    std::fs::write(
            &peer,
            r#"{"geomean_us":900.25,"name":"updated_peer.txt","path":"/tmp/c4.txt","p1_us":1700.0,"shapes":{"32768x1":40000.0,"512x640":1700.0}}"#,
        )
        .unwrap();
    bump_mtime(&peer);
    let busted = load_living_peer_snapshot().expect("mtime/len change");
    assert_eq!(busted.1, "updated_peer.txt");
    assert!(
        (busted.0 - 900.25).abs() < 1e-9,
        "busted geo: {:?}",
        busted.0
    );
    assert_eq!(
        load_living_peer_p1_us().map(|u| (u * 10.0).round() / 10.0),
        Some(1700.0)
    );
    let open = load_living_peer_open_levers(1);
    assert_eq!(open[0].key, "32768x1");
    assert!((open[0].board_us - 40000.0).abs() < 1e-9);

    std::fs::remove_file(&peer).unwrap();
    assert!(
        load_living_peer_snapshot().is_none(),
        "deleted peer file must not leak a previous cache"
    );
}

#[test]
fn forge_train_snap_reads_status_and_formats_fragment() {
    let _g = env_lock();
    let id = std::process::id();
    let status = std::env::temp_dir().join(format!("forge-when-free-status-{id}.json"));
    let cycle = std::env::temp_dir().join(format!("forge-last-cycle-{id}.json"));
    let idle = std::env::temp_dir().join(format!("forge-when-free-idle-{id}.json"));
    std::fs::write(
        &status,
        r#"{"state":"training","train_step":40,"train_total":200,"train_eta_sec":2280}"#,
    )
    .unwrap();
    let _s = EnvGuard::set(
        "FORGE_WHEN_FREE_STATUS",
        Some(status.to_str().expect("utf8")),
    );
    let snap = load_forge_train_snap().expect("snap");
    assert_eq!(snap.state, "training");
    assert_eq!(snap.train_step, Some(40));
    assert_eq!(snap.train_total, Some(200));
    let frag = forge_train_strip_fragment(&snap);
    assert_eq!(frag, "forge 40/200 ~38m");
    // Mid-train PRIMARY attack surface from pulse stamp.
    std::fs::write(
            &status,
            r#"{"state":"training","train_step":40,"train_total":200,"train_eta_sec":2280,"open_lever_top":"32768x1","train_loss_live":0.2,"measured_hold_us":38300.0,"preference_n":96,"coding_eval_n":24,"coding_eval_primary_n":4,"version":"v3","next_adapter_version":"v3","gpu_free_mib":15000.0,"free_mib_min":14900.0}"#,
        )
        .unwrap();
    let snap_p = load_forge_train_snap().expect("primary mid-train");
    assert_eq!(snap_p.open_lever_top.as_deref(), Some("32768x1"));
    assert_eq!(snap_p.measured_hold_us, Some(38300.0));
    assert_eq!(snap_p.preference_n, Some(96));
    assert_eq!(snap_p.coding_eval_n, Some(24));
    assert_eq!(snap_p.coding_eval_primary_n, Some(4));
    assert_eq!(snap_p.version.as_deref(), Some("v3"));
    assert_eq!(snap_p.gpu_free_mib, Some(15000.0));
    let frag_p = forge_train_strip_fragment(&snap_p);
    assert!(
        frag_p.contains("→32k")
            && frag_p.contains("L0.20")
            && frag_p.contains("H38k")
            && frag_p.contains("pref96")
            && frag_p.contains("ce24")
            && frag_p.contains("→v3")
            && frag_p.contains("f15G"),
        "mid-train strip names adapter+PRIMARY+HOLD+pref+ce+free: {frag_p}"
    );
    // Steps complete → post phase even without train_phase field.
    std::fs::write(
        &status,
        r#"{"state":"training","train_step":200,"train_total":200,"train_phase":"adapter_eval"}"#,
    )
    .unwrap();
    let snap_post = load_forge_train_snap().expect("post");
    assert_eq!(snap_post.train_phase.as_deref(), Some("adapter_eval"));
    assert_eq!(forge_train_strip_fragment(&snap_post), "forge 200/200 eval");
    // last cycle when not training
    std::fs::write(
            &cycle,
            r#"{"version":"v1","gate_pass":true,"promoted":false,"adapter_local":"/home/u/angel-forge/adapters/v1","open_lever_top":"32768x1","free_train_primary_n":4,"measured_hold_us":38300.0,"train_loss":0.4,"train_loss_min":0.25,"train_loss_max":0.53,"preference_n":96,"coding_eval_n":24,"coding_eval_primary_n":4}"#,
        )
        .unwrap();
    std::fs::write(&idle, r#"{"state":"polling","note":"waiting"}"#).unwrap();
    let _s2 = EnvGuard::set("FORGE_WHEN_FREE_STATUS", Some(idle.to_str().expect("utf8")));
    let _c = EnvGuard::set("FORGE_LAST_CYCLE", Some(cycle.to_str().expect("utf8")));
    let done = load_forge_train_snap().expect("cycle");
    assert_eq!(done.state, "done");
    assert!(done.adapter_local);
    assert_eq!(done.open_lever_top.as_deref(), Some("32768x1"));
    assert_eq!(done.free_train_primary_n, Some(4));
    assert_eq!(done.measured_hold_us, Some(38300.0));
    assert_eq!(done.coding_eval_n, Some(24));
    assert_eq!(done.train_loss_min, Some(0.25));
    assert_eq!(done.train_loss_max, Some(0.53));
    let frag = forge_train_strip_fragment(&done);
    assert!(
        frag.contains("forge v1 gate✓ local")
            && frag.contains("L↓0.25")
            && frag.contains("→32k")
            && frag.contains("ftP4")
            && frag.contains("H38k")
            && frag.contains("pref96")
            && frag.contains("ce24"),
        "got: {frag}"
    );
    // Done status breadcrumb (finalize) preferred over last-cycle when present.
    std::fs::write(
            &status,
            r#"{"state":"done","version":"v2","gate_pass":true,"promoted":false,"adapter_local":"/home/u/angel-forge/adapters/v2","open_lever_top":"32768x1","free_train_primary_n":4,"measured_hold_us":38300.0,"train_loss":0.4,"ok":true}"#,
        )
        .unwrap();
    let _s_done = EnvGuard::set(
        "FORGE_WHEN_FREE_STATUS",
        Some(status.to_str().expect("utf8")),
    );
    let done_st = load_forge_train_snap().expect("done status");
    assert_eq!(done_st.version.as_deref(), Some("v2"));
    assert_eq!(done_st.measured_hold_us, Some(38300.0));
    let frag_st = forge_train_strip_fragment(&done_st);
    assert!(
        frag_st.contains("forge v2") && frag_st.contains("H38k"),
        "done status strip: {frag_st}"
    );
    // Root LID suffix names adapter when Treebeard + last-cycle (not live done status).
    std::fs::write(&idle, r#"{"state":"polling","note":"waiting"}"#).unwrap();
    let _s_idle_root = EnvGuard::set("FORGE_WHEN_FREE_STATUS", Some(idle.to_str().expect("utf8")));
    let _lane = EnvGuard::set("ANGEL_LANE", Some("treebeard"));
    let root = free_train_system_suffix();
    assert!(root.contains("free-train"), "got: {root}");
    assert!(root.contains("v1"), "got: {root}");
    assert!(root.contains("gate=pass"), "got: {root}");
    assert!(
        root.contains("local-disk") || root.contains("AUTOPROMOTE"),
        "got: {root}"
    );
    assert!(
        root.contains("next_PRIMARY=32768x1") || root.contains("32768"),
        "root names next PRIMARY lever: {root}"
    );
    assert!(
        root.contains("free_train_PRIMARY_rows=4"),
        "root names free_train PRIMARY mass: {root}"
    );
    assert!(
        root.contains("PRIMARY_HOLD_floor=38300us") || root.contains("NEW_HOLD"),
        "root names PRIMARY HOLD floor: {root}"
    );
    assert!(
        root.contains("Lrange=0.250–0.530") || root.contains("Lrange=0.25"),
        "last-cycle root names Lrange: {root}"
    );
    // In-flight training suffix (restore mid-train status after post-phase write).
    std::fs::write(
        &status,
        r#"{"state":"training","train_step":40,"train_total":200,"train_eta_sec":2280}"#,
    )
    .unwrap();
    let _s3 = EnvGuard::set(
        "FORGE_WHEN_FREE_STATUS",
        Some(status.to_str().expect("utf8")),
    );
    let flying = free_train_system_suffix();
    assert!(flying.contains("in flight"), "got: {flying}");
    assert!(flying.contains("40/200"), "got: {flying}");
    // Mid-train status with PRIMARY stamp names attack surface in root LID.
    std::fs::write(
            &status,
            r#"{"state":"training","train_step":90,"train_total":200,"train_eta_sec":1200,"open_lever_top":"32768x1","free_train_primary_n":4,"train_loss_live":0.2,"train_loss_min":0.15,"train_loss_max":0.45}"#,
        )
        .unwrap();
    let _s4 = EnvGuard::set(
        "FORGE_WHEN_FREE_STATUS",
        Some(status.to_str().expect("utf8")),
    );
    let flying_p = free_train_system_suffix();
    assert!(
        flying_p.contains("next_PRIMARY=32768x1"),
        "in-flight root names PRIMARY: {flying_p}"
    );
    assert!(
        flying_p.contains("prior_free_train_PRIMARY_rows=4"),
        "in-flight root names prior densify: {flying_p}"
    );
    assert!(
        flying_p.contains("Lrange=0.150–0.450") || flying_p.contains("Lrange=0.15"),
        "in-flight root names Lrange: {flying_p}"
    );
    let frag_l = forge_train_strip_fragment(&load_forge_train_snap().expect("flying snap"));
    assert!(
        frag_l.contains("L↓0.15") && frag_l.contains("L0.20"),
        "mid-train strip L↓ chip: {frag_l}"
    );
    let _ = std::fs::remove_file(&status);
    let _ = std::fs::remove_file(&cycle);
    let _ = std::fs::remove_file(&idle);
}

#[test]
fn forge_train_snap_caches_by_mtime_len() {
    let _g = env_lock();
    let id = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let status = std::env::temp_dir().join(format!("forge-snap-cache-status-{id}-{nanos}.json"));
    let missing = std::env::temp_dir().join(format!("forge-snap-cache-missing-{id}-{nanos}.json"));
    std::fs::write(
        &status,
        r#"{"state":"training","train_step":40,"train_total":200,"train_eta_sec":2280}"#,
    )
    .unwrap();
    let _s = EnvGuard::set(
        "FORGE_WHEN_FREE_STATUS",
        Some(status.to_str().expect("utf8")),
    );
    let _c = EnvGuard::set("FORGE_LAST_CYCLE", Some(missing.to_str().expect("utf8")));
    let first = load_forge_train_snap().expect("first load");
    assert_eq!(first.train_step, Some(40));

    let original = std::fs::read(&status).unwrap();
    let mut garbage = vec![b'x'; original.len()];
    let last = garbage.len() - 1;
    garbage[0] = b'{';
    garbage[last] = b'}';
    overwrite_preserving_identity(&status, &garbage);
    assert_eq!(
        load_forge_train_snap().expect("cache hit"),
        first,
        "unchanged mtime/len must keep the cached snap even if bytes changed"
    );

    std::fs::write(
        &status,
        r#"{"state":"training","train_step":90,"train_total":200,"train_eta_sec":1200}"#,
    )
    .unwrap();
    bump_mtime(&status);
    let busted = load_forge_train_snap().expect("mtime/len change");
    assert_eq!(busted.train_step, Some(90));
    assert_ne!(busted, first);

    std::fs::remove_file(&status).unwrap();
    assert!(
        load_forge_train_snap().is_none(),
        "deleted status must not leak a previous cache"
    );
}

#[test]
fn subcall_depth_guard_restores_and_gates_spawn() {
    let _g = env_lock();
    let _depth = EnvGuard::set("ANGEL_TREEBEARD_MAX_DEPTH", Some("2"));
    let _lane = EnvGuard::set("ANGEL_LANE", Some("treebeard"));
    assert_eq!(subcall_depth(), 0);
    assert!(spawn_nesting_allowed());
    {
        let _d1 = SubcallDepthGuard::enter();
        assert_eq!(subcall_depth(), 1);
        assert!(spawn_nesting_allowed());
        {
            let _d2 = SubcallDepthGuard::enter();
            assert_eq!(subcall_depth(), 2);
            assert!(!spawn_nesting_allowed());
        }
        assert_eq!(subcall_depth(), 1);
    }
    assert_eq!(subcall_depth(), 0);
}

#[test]
fn eager_offload_floor_respects_lane_and_env() {
    let _g = env_lock();
    let _e = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", None);
    let _def = EnvGuard::set("ANGEL_LANE", Some("default"));
    assert_eq!(eager_tool_offload_min_bytes(), 16 * 1024);
    let _tb = EnvGuard::set("ANGEL_LANE", None);
    assert_eq!(eager_tool_offload_min_bytes(), 16 * 1024);
    let _x = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", Some("100"));
    assert_eq!(eager_tool_offload_min_bytes(), 100);
}
