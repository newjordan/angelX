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
fn treebeard_route_rides_only_on_its_lane() {
    let _g = env_lock();
    let _off = EnvGuard::set("ANGEL_LANE", Some("default"));
    assert_eq!(lane_route(), None);
    let _on = EnvGuard::set("ANGEL_LANE", None);
    assert_eq!(
        lane_route(),
        Some(crate::agent::harness::book::m_method::TREEBEARD)
    );
    // The contract's words are the route's pages.
    let pages = crate::agent::harness::book::m_method::TREEBEARD
        .sub()
        .pages
        .join(" ");
    assert!(pages.contains("treebeard lane"));
    assert!(pages.contains("handle_read"));
    assert!(pages.contains("handle_put"));
    assert!(!pages.contains("popcorn-submit-hiq"));
    assert!(!pages.contains("B6LDP"));
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
