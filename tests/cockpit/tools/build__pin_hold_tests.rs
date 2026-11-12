use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn a_held_latch_stops_its_waiter_until_released() {
    let latch = HashHold::new();
    latch.hold();
    let started = AtomicBool::new(false);
    let finished = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            started.store(true, Ordering::SeqCst);
            latch.wait_released();
            finished.store(true, Ordering::SeqCst);
        });
        while !started.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        std::thread::sleep(Duration::from_millis(80));
        assert!(
            !finished.load(Ordering::SeqCst),
            "a held reader must not start"
        );
        latch.release();
    });
    assert!(finished.load(Ordering::SeqCst));
}

#[test]
fn a_latch_that_was_never_held_never_waits() {
    let latch = HashHold::new();
    latch.wait_released();
    latch.release();
    latch.release();
    latch.wait_released();
}

#[test]
fn settling_a_pin_releases_a_held_reader_and_yields_the_same_digest() {
    let _capture = capture_lock();
    let root = std::env::temp_dir().join(format!("angel-pin-hold-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let executable = root.join("tool");
    std::fs::write(&executable, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();

    hold_pin_hashing();
    let pinned = capture_executable(executable.clone(), "held tool").unwrap();
    // The first reader to settle opens the latch for everyone, so a caller that
    // needs the digest can never wait on a held thread.
    let digest = pinned.digest().unwrap().cloned();
    release_pin_hashing();
    assert_eq!(
        digest,
        Some(crate::knowledge::cut::sha256_hex(b"#!/bin/sh\nexit 0\n"))
    );
    settle_pending_pins();
    let _ = std::fs::remove_dir_all(root);
}
