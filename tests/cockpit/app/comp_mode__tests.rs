use super::*;

#[test]
fn parses_truthy_and_falsy() {
    assert!(!parse(None));
    for v in ["", "0", "false", "no", "off", "disabled"] {
        assert!(!parse(Some(v)), "{v}");
    }
    for v in ["1", "true", "yes", "on", "comp", "lean"] {
        assert!(parse(Some(v)), "{v}");
    }
}

#[test]
fn comp_mode_set_and_status() {
    let _guard = crate::tests::env_lock();
    let _clean = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _clean_turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    assert!(!enabled());
    assert!(status_text().contains("OFF"));

    set(true);
    assert!(enabled());
    assert!(status_text().contains("⚡ TURBO / COMP MODE ON"));

    set(false);
    assert!(!enabled());
    assert!(status_text().contains("OFF"));
}

#[test]
fn ambient_stage_sim_allowed_only_when_comp_mode_off() {
    let _guard = crate::tests::env_lock();
    let _off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    invalidate_cache();
    assert!(ambient_stage_sim_allowed());
    set(true);
    assert!(!ambient_stage_sim_allowed());
    set(false);
    assert!(ambient_stage_sim_allowed());
}

#[test]
fn stage_world_mirrors_skip_hidden_and_comp_without_slowing_default() {
    let _guard = crate::tests::env_lock();
    let _off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    invalidate_cache();
    assert!(
        stage_world_mirrors_allowed(true),
        "default visible Stage still earns mirrors"
    );
    assert!(
        !stage_world_mirrors_allowed(false),
        "Hidden / unpainted Stage must not pay mirrors"
    );

    set(true);
    assert!(
        !stage_world_mirrors_allowed(true),
        "comp/lean must skip mirrors even while chrome is on screen"
    );
    assert!(!stage_world_mirrors_allowed(false));

    set(false);
    assert!(
        stage_world_mirrors_allowed(true),
        "default cockpit must not stay gated after /comp off"
    );
}
