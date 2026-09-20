use super::*;
#[test]
fn spawn_default_has_no_120_second_cut_and_explicit_seconds_are_exact() {
    let _guard = crate::tests::env_lock();
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_SPAWN_TIMEOUT");
    let default = configured_formation_timeout(&serde_json::json!({}));
    assert!(default.is_zero());
    let _cap = crate::tests::TestEnvGuard::set("ANGEL_SPAWN_TIMEOUT", "7200");
    assert_eq!(
        configured_formation_timeout(&serde_json::json!({})).as_secs(),
        7200
    );
    assert_eq!(
        configured_formation_timeout(&serde_json::json!({"timeout_secs": 1})).as_secs(),
        1
    );
    assert!(configured_formation_timeout(&serde_json::json!({"timeout_secs": 0})).is_zero());
}
