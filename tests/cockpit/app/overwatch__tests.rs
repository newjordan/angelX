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

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn fleet_overwatch_complete_output_fixture() {
    use crate::agent::process_test_support::FixtureCleanup;
    use crate::agent::process_test_support::capture_evidence_fixture::EvidenceFixture;

    let root = std::env::var_os("ANGEL_T_OVERWATCH_EOF")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = FixtureCleanup::new();
    for stream in ["stdout", "stderr"] {
        let mut fixture = EvidenceFixture::new(
            std::path::Path::new(&root),
            "overwatch",
            stream,
            "FLEET GPU 58.5% CPU 51.0%\n",
        );
        let started = Instant::now();
        let sample = fleet_overwatch_from_command(fixture.command(), Duration::from_millis(100));
        let elapsed = started.elapsed();
        fixture.finish();
        eprintln!("overwatch missing {stream} EOF: elapsed={elapsed:?} sample={sample:?}");
        assert!(
            sample.is_none(),
            "{stream}: accepted an unfinished measurement"
        );
        assert!(elapsed < Duration::from_secs(3), "{stream}: {elapsed:?}");
    }

    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 'FLEET GPU 58.5%% CPU 51.0%%\\n'"]);
    let sample = fleet_overwatch_from_command(command, Duration::from_secs(1)).unwrap();
    assert_eq!(sample.cpu_pct, 51.0);
    assert_eq!(sample.gpu_pct, 58.5);

    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 'FLEET GPU 58.5%% CPU 51.0%%\\n'; exit 17"]);
    assert!(fleet_overwatch_from_command(command, Duration::from_secs(1)).is_none());

    // A valid-looking terminal measurement after a capped output still cannot
    // become a complete sample; keep the existing byte-cap behavior.
    let mut command = Command::new("/usr/bin/python3");
    command.args(["-c", "import sys; sys.stdout.write('x' * (2 * 1024 * 1024)); print('\\nFLEET GPU 58.5% CPU 51.0%')"]);
    assert!(fleet_overwatch_from_command(command, Duration::from_secs(2)).is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn fleet_overwatch_requires_complete_output_before_sampling() {
    crate::agent::process_test_support::isolated_fixture(
        &format!(
            "{}::fleet_overwatch_complete_output_fixture",
            module_path!().split_once("::").unwrap().1
        ),
        "ANGEL_T_OVERWATCH_EOF",
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
fn bare_overwatch_command_is_found_on_path_without_a_cwd_relative_file() {
    let _guard = crate::tests::env_lock();
    let dir = std::env::temp_dir().join(format!("angel-overwatch-path-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let name = "angel-overwatch-path-fixture";
    let executable = dir.join(name);
    std::fs::write(&executable, b"fixture").unwrap();
    let _path = crate::tests::TestEnvGuard::set("PATH", dir.to_str().unwrap());
    let cmd = overwatch_cmd_from(Some(name.to_string()), None);
    assert!(!Path::new(&cmd).exists());
    assert!(overwatch_cmd_available(&cmd));
    assert!(overwatch_cmd_available(executable.to_str().unwrap()));
    assert!(!overwatch_cmd_available("angel-overwatch-missing-fixture"));
    let _ = std::fs::remove_dir_all(dir);
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
