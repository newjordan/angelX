use super::*;

#[test]
fn fleet_overwatch_totals_parse() {
    let text = "-- TOTALS --\n  FLEET  GPU   58.5%   CPU   51.0%\n";
    let sample = parse_fleet_overwatch_totals(text).unwrap();
    assert_eq!(sample.gpu_pct, 58.5);
    assert_eq!(sample.cpu_pct, 51.0);
}

#[cfg(unix)]
#[test]
fn fleet_overwatch_hung_tree_returns_within_its_probe_deadline() {
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30 & wait"]);
    let started = Instant::now();

    let sample = fleet_overwatch_from_command(command, Duration::from_millis(50));

    assert!(sample.is_none());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "fleet overwatch probe left its inflight worker wedged: {:?}",
        started.elapsed()
    );
}

#[test]
fn overwatch_cmd_derives_from_launch_inputs_without_a_literal_home() {
    let home = |value: &str| Some(std::ffi::OsString::from(value));
    assert_eq!(
        overwatch_cmd_from(None, home("/home/user")),
        "/home/user/.local/bin/overwatch"
    );
    assert_eq!(
        overwatch_cmd_from(
            Some(" /opt/fleet/overwatch ".to_string()),
            home("/home/user")
        ),
        "/opt/fleet/overwatch"
    );
    assert_eq!(
        overwatch_cmd_from(Some("   ".to_string()), home("/home/user")),
        "/home/user/.local/bin/overwatch"
    );
    assert_eq!(overwatch_cmd_from(None, home("")), "overwatch");
    assert_eq!(overwatch_cmd_from(None, None), "overwatch");
}

#[test]
fn gpu_polling_is_opt_in_for_fast_terminal_refresh() {
    let _guard = crate::tests::env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OVERWATCH_GPU") };
    assert_eq!(read_gpu_pct(), None);
}

#[test]
fn periodic_refresh_stays_under_50ms_without_gpu_poll() {
    let _guard = crate::tests::env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OVERWATCH_GPU") };
    let mut overwatch = Overwatch::new();
    overwatch.last_sample = Instant::now() - OVERWATCH_INTERVAL - Duration::from_millis(1);
    let t0 = Instant::now();
    overwatch.refresh();
    let elapsed = t0.elapsed();
    eprintln!("overwatch-periodic-refresh took {elapsed:?}");
    assert!(
        elapsed < Duration::from_millis(50),
        "overwatch-periodic-refresh took {elapsed:?}, expected <50ms"
    );
}
