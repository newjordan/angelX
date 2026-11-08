use super::*;
use std::time::{Duration, Instant};

fn wake_fd() -> i32 {
    let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
    assert!(fd >= 0, "eventfd");
    fd
}

#[test]
fn a_quiet_reaper_sleeps_the_whole_wait_instead_of_polling() {
    let fd = wake_fd();
    let started = Instant::now();
    reaper_sleep_on(fd, Duration::from_millis(120));
    assert!(
        started.elapsed() >= Duration::from_millis(100),
        "woke with nothing to do"
    );
    unsafe { libc::close(fd) };
}

#[test]
fn a_signal_wakes_the_reaper_at_once() {
    let fd = wake_fd();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(40));
        let one = 1u64;
        unsafe { libc::write(fd, (&raw const one).cast(), std::mem::size_of::<u64>()) };
    });
    let started = Instant::now();
    reaper_sleep_on(fd, Duration::from_secs(10));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the reaper slept through a signal"
    );
    writer.join().unwrap();
    // The wake was drained: the next sleep waits again.
    let again = Instant::now();
    reaper_sleep_on(fd, Duration::from_millis(80));
    assert!(again.elapsed() >= Duration::from_millis(60));
    unsafe { libc::close(fd) };
}

#[test]
fn without_a_wake_descriptor_the_reaper_still_sleeps_briefly() {
    let started = Instant::now();
    reaper_sleep_on(-1, Duration::from_secs(10));
    assert!(started.elapsed() < Duration::from_secs(1));
}
