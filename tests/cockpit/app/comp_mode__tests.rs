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

// --- `/turbo`'s race cadence --------------------------------------------------
//
// Two switches, not one. Lean mode (`set`) is the render and tooling posture the
// HUD reports. The race cadence is `ANGEL_COMPETITION_MODE`, which the turn
// classifier, the schema picker and the caddy board read each turn. `/turbo
// podrace` and `/turbo handoff` raise both; `/turbo off` has to lower both.

/// A process with no lean or race posture exported, and the loop's and goal's
/// state files kept off the operator's disk. Dropping it lowers whatever `/turbo`
/// raised, then puts the environment back.
struct StandardProcess {
    _guards: Vec<crate::tests::TestEnvGuard>,
    files: [std::path::PathBuf; 2],
}

impl Drop for StandardProcess {
    fn drop(&mut self) {
        set(false);
        for file in &self.files {
            let _ = std::fs::remove_file(file);
        }
    }
}

fn standard_process(tag: &str) -> StandardProcess {
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let goal = dir.join(format!("angel_comp_mode_{tag}_goal_{pid}.json"));
    let looped = dir.join(format!("angel_comp_mode_{tag}_loop_{pid}.json"));
    let mut guards: Vec<_> = [
        "ANGEL_COMP_MODE",
        "ANGEL_TURBO",
        "ANGEL_TURBO_MODE",
        "ANGEL_LEAN_MODE",
        "ANGEL_LEAN",
        "ANGEL_COMPETITION_MODE",
        "ANGEL_TURN_DEADLINE_SECS",
        "ANGEL_COMPETITION_TURN_DEADLINE_SECS",
        "ANGEL_LOOP_STATE_DIR",
    ]
    .into_iter()
    .map(crate::tests::TestEnvGuard::unset)
    .collect();
    guards.push(crate::tests::TestEnvGuard::set(
        "ANGEL_GOAL_FILE",
        &goal.to_string_lossy(),
    ));
    guards.push(crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        &looped.to_string_lossy(),
    ));
    StandardProcess {
        _guards: guards,
        files: [goal, looped],
    }
}

fn turn_trigger(text: &str) -> Option<&'static str> {
    crate::agent::harness::competition_mode_trigger_live(&[crate::agent::club::ChatMsg::user(text)])
}

fn competition_flag() -> Option<std::ffi::OsString> {
    std::env::var_os("ANGEL_COMPETITION_MODE")
}

#[test]
fn turbo_off_after_podrace_returns_turns_to_standard() {
    let _guard = crate::tests::env_lock();
    let _env = standard_process("off");
    invalidate_cache();
    let mut app = crate::seed_preview_app();
    assert_eq!(turn_trigger("how is the build?"), None);

    let reply = app.toggle_comp(Some("podrace"));
    assert!(reply.contains("PODRACE ENGAGED"), "{reply}");
    assert!(enabled());
    assert_eq!(
        turn_trigger("how is the build?"),
        Some("ANGEL_COMPETITION_MODE")
    );

    let reply = app.toggle_comp(Some("off"));
    assert!(reply.contains("OFF"), "{reply}");
    assert!(!enabled());
    assert_eq!(
        turn_trigger("how is the build?"),
        None,
        "the HUD says standard mode; the next turn must run as one"
    );
    assert!(competition_flag().is_none());
    assert!(!crate::agent::harness::use_essential_schemas(false, false));
}

#[test]
fn turbo_rich_alias_also_lowers_the_race_cadence() {
    let _guard = crate::tests::env_lock();
    let _env = standard_process("rich");
    invalidate_cache();
    let mut app = crate::seed_preview_app();
    app.toggle_comp(Some("race"));
    assert_eq!(turn_trigger("hello"), Some("ANGEL_COMPETITION_MODE"));
    app.toggle_comp(Some("rich"));
    assert_eq!(turn_trigger("hello"), None);
    assert!(competition_flag().is_none());
}

/// What the race cadence does while it is up, so lowering it later cannot
/// change any of it.
#[test]
fn turbo_podrace_keeps_the_race_cadence_while_it_is_on() {
    let _guard = crate::tests::env_lock();
    let _env = standard_process("while-on");
    let _deadline = crate::tests::TestEnvGuard::set("ANGEL_COMPETITION_TURN_DEADLINE_SECS", "45");
    invalidate_cache();
    let mut app = crate::seed_preview_app();
    app.toggle_comp(Some("podrace"));

    // Every turn is a race turn whatever the operator typed, and the trigger
    // names the flag that armed it.
    for text in ["", "hello", "please fix the parser"] {
        assert_eq!(
            turn_trigger(text),
            Some("ANGEL_COMPETITION_MODE"),
            "{text:?}"
        );
    }
    assert!(crate::agent::harness::competition_mode_active(&[]));
    // A race turn advertises the essential schemas, and its deadline comes from
    // the competition knob; an ordinary turn does neither.
    assert!(crate::agent::harness::use_essential_schemas(false, true));
    assert_eq!(
        crate::agent::harness::configured_turn_deadline_secs_for(true),
        45
    );
    assert_eq!(
        crate::agent::harness::configured_turn_deadline_secs_for(false),
        0
    );
    // Lean mode rides along and the HUD says so.
    assert!(enabled());
    assert!(status_text().contains("⚡ TURBO / COMP MODE ON"));
    assert_eq!(competition_flag().as_deref(), Some("1".as_ref()));
}

/// Lean mode alone is not the race: only podrace and handoff raise the flag.
#[test]
fn turbo_on_loop_and_flash_never_raise_the_race_cadence() {
    let _guard = crate::tests::env_lock();
    let _env = standard_process("lean-only");
    invalidate_cache();
    let mut app = crate::seed_preview_app();
    for arg in ["on", "lean", "turbo", "loop", "flash", "off"] {
        app.toggle_comp(Some(arg));
        assert!(competition_flag().is_none(), "/turbo {arg}");
        assert_eq!(turn_trigger("hello"), None, "/turbo {arg}");
    }
}

/// An operator who exports the flag at launch opted in for the whole process:
/// `/turbo off` brings back what launch had, not the absence of the flag.
#[test]
fn a_flag_exported_at_launch_outlives_turbo_off() {
    let _guard = crate::tests::env_lock();
    let _env = standard_process("launch-on");
    let _launch = crate::tests::TestEnvGuard::set("ANGEL_COMPETITION_MODE", "1");
    invalidate_cache();
    let mut app = crate::seed_preview_app();

    app.toggle_comp(Some("off"));
    assert_eq!(competition_flag().as_deref(), Some("1".as_ref()));
    app.toggle_comp(Some("podrace"));
    app.toggle_comp(Some("off"));
    assert_eq!(competition_flag().as_deref(), Some("1".as_ref()));
    assert_eq!(turn_trigger("hello"), Some("ANGEL_COMPETITION_MODE"));
    assert!(
        !enabled(),
        "lean mode is off; the launch opt-in is separate"
    );
}

#[test]
fn a_flag_exported_off_at_launch_comes_back_off() {
    let _guard = crate::tests::env_lock();
    let _env = standard_process("launch-zero");
    let _launch = crate::tests::TestEnvGuard::set("ANGEL_COMPETITION_MODE", "0");
    invalidate_cache();
    let mut app = crate::seed_preview_app();

    app.toggle_comp(Some("podrace"));
    assert_eq!(turn_trigger("hello"), Some("ANGEL_COMPETITION_MODE"));
    app.toggle_comp(Some("off"));
    assert_eq!(competition_flag().as_deref(), Some("0".as_ref()));
    assert_eq!(turn_trigger("hello"), None);
}

/// Podrace and handoff share one raise: raising twice must not mistake the
/// first raise for the operator's own value.
#[test]
fn raising_the_race_cadence_twice_still_restores_launch() {
    let _guard = crate::tests::env_lock();
    let _env = standard_process("twice");
    invalidate_cache();
    engage_competition();
    engage_competition();
    assert_eq!(competition_flag().as_deref(), Some("1".as_ref()));
    set(false);
    assert!(competition_flag().is_none());
    // Nothing is left to restore: a second `off` leaves a later export alone.
    let _later = crate::tests::TestEnvGuard::set("ANGEL_COMPETITION_MODE", "1");
    set(false);
    assert_eq!(competition_flag().as_deref(), Some("1".as_ref()));
}

/// The comp seat starts its race with `/loop podrace`. That path neither reads
/// nor writes the race flag, and `/turbo off` leaves its loop running.
#[test]
fn loop_podrace_runs_without_the_flag_and_survives_turbo_off() {
    let _guard = crate::tests::env_lock();
    let _env = standard_process("loop-podrace");
    invalidate_cache();
    let mut app = crate::seed_preview_app();
    app.input = "/loop podrace now ship the candidate".to_string();
    app.submit();
    assert!(
        app.loop_ctl.podrace,
        "{:?}",
        app.messages.last().map(|m| &m.text)
    );
    assert_eq!(
        app.loop_ctl.status,
        crate::drive::loop_ctl::LoopStatus::Running
    );
    assert!(competition_flag().is_none());
    assert!(!enabled(), "/loop podrace is not lean mode");

    app.toggle_comp(Some("off"));
    assert!(competition_flag().is_none());
    assert!(app.loop_ctl.podrace);
    assert_eq!(
        app.loop_ctl.status,
        crate::drive::loop_ctl::LoopStatus::Running
    );
    app.input = "/loop stop".to_string();
    app.submit();
}
