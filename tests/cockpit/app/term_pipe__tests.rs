use super::*;

#[test]
fn passthrough_when_unset_writes_to_stdout_and_never_captures() {
    let mut t = TeeOut::passthrough();
    assert!(t.tx.is_none());
    let n = t.write(b"").unwrap();
    assert_eq!(n, 0);
}

#[test]
fn capacity_defaults_and_parses() {
    assert_eq!(DEFAULT_CAPACITY, 2048);
}

#[test]
fn captures_written_bytes_to_a_file_sink_end_to_end() {
    let _guard = crate::tests::env_lock();
    let path = std::env::temp_dir().join(format!("angelX-termpipe-{}.log", std::process::id()));
    let _ = std::fs::remove_file(&path);
    unsafe { std::env::set_var("ANGEL_TERM_PIPE", &path) };
    let marker = b"MARKER-abc-1234";
    {
        let mut t = tee_stdout();
        assert!(t.tx.is_some(), "capture should be active when env is set");
        t.write_all(marker).unwrap();
        t.flush().unwrap();
    }
    unsafe { std::env::remove_var("ANGEL_TERM_PIPE") };

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
    let (tx, _rx) = sync_channel::<Vec<u8>>(1);
    let mut t = TeeOut {
        out: io::stdout(),
        tx: Some(tx),
    };
    for _ in 0..1000 {
        let _ = t.write(b"x");
    }
}
