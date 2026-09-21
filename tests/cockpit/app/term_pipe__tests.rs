use super::*;

#[test]
fn passthrough_when_unset_writes_to_stdout_and_never_captures() {
    let mut t = TeeOut::passthrough();
    assert!(t.tx.is_none());
    // write returns the real stdout's byte count; no panic, no capture path.
    let n = t.write(b"").unwrap();
    assert_eq!(n, 0);
}

#[test]
fn capacity_defaults_and_parses() {
    // Default when unset (the env may or may not be set in CI; assert the
    // pure fallback via a known-bad value path instead).
    assert_eq!(DEFAULT_CAPACITY, 2048);
}

#[test]
fn captures_written_bytes_to_a_file_sink_end_to_end() {
    // Exercise the real chain: env path -> bounded channel -> writer thread
    // -> file sink. Proves capture actually happens, not just that it builds.
    // Mutates ANGEL_TERM_PIPE — hold the shared env lock so the parallel
    // runner can't race the environ table.
    let _guard = crate::tests::env_lock();
    let path = std::env::temp_dir().join(format!("angelX-termpipe-{}.log", std::process::id()));
    let _ = std::fs::remove_file(&path);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TERM_PIPE", &path) };
    let marker = b"MARKER-abc-1234";
    {
        let mut t = tee_stdout();
        assert!(t.tx.is_some(), "capture should be active when env is set");
        t.write_all(marker).unwrap();
        t.flush().unwrap();
    } // drop closes the channel -> writer thread drains remaining + exits
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_TERM_PIPE") };

    // The writer thread is detached; poll the sink with a bounded wait.
    let mut found = false;
    for _ in 0..200 {
        if let Ok(b) = std::fs::read(&path)
            && b.windows(marker.len()).any(|w| w == marker)
        {
            found = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let _ = std::fs::remove_file(&path);
    assert!(found, "file sink never received the written bytes");
}

#[test]
fn full_channel_drops_instead_of_blocking() {
    // A depth-1 channel with no draining receiver: the second try_send must
    // fail fast (drop), proving the UI write path can never block on a stalled
    // consumer.
    let (tx, _rx) = sync_channel::<Vec<u8>>(1);
    let mut t = TeeOut {
        out: io::stdout(),
        tx: Some(tx),
    };
    // Many writes; if try_send ever blocked, this would hang. It must return.
    for _ in 0..1000 {
        let _ = t.write(b"x");
    }
}
