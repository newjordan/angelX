use super::*;
use crate::agent::harness::{link_child_owner, output_timed_captured_cancellable};
#[cfg(target_os = "linux")]
use crate::agent::harness::take_tool_idle_escalation;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn owner(cancel: &AtomicBool) -> usize {
    cancel as *const _ as usize
}

/// A request reaches only a call that was already running when it was made,
/// and is consumed by it.
#[test]
fn a_release_reaches_a_running_call_once_and_never_a_later_one() {
    let cancel = AtomicBool::new(false);
    let call_started = Instant::now();
    request_release(owner(&cancel), Release::Stop);
    assert!(release_pending(owner(&cancel)));
    assert_eq!(
        take_release(owner(&cancel), call_started),
        Some(Release::Stop)
    );
    assert!(!release_pending(owner(&cancel)), "consumed");
    assert_eq!(take_release(owner(&cancel), call_started), None);

    request_release(owner(&cancel), Release::HandOff);
    std::thread::sleep(Duration::from_millis(5));
    let later_call = Instant::now();
    assert_eq!(
        take_release(owner(&cancel), later_call),
        None,
        "a call started after the request does not inherit it"
    );
    assert!(
        release_pending(owner(&cancel)),
        "left for the call it was meant for"
    );
    clear_release(owner(&cancel));
    assert!(!release_pending(owner(&cancel)));
}

/// The exec wait sees the turn's linked (effective) cancel; the watchdog
/// keys its request by the root owner.
#[test]
fn a_linked_owner_finds_its_roots_release() {
    let root = AtomicBool::new(false);
    let child = AtomicBool::new(false);
    let _link = link_child_owner(&child, &root);
    let call_started = Instant::now();
    request_release(owner(&root), Release::HandOff);
    assert_eq!(
        take_release(owner(&child), call_started),
        Some(Release::HandOff)
    );
    assert!(!release_pending(owner(&root)));
}

fn wedged_sleep_released_with(release: Release) -> (crate::agent::harness::TimedCapture, Duration) {
    let cancel = AtomicBool::new(false);
    let key = owner(&cancel);
    let requester = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        request_release(key, release);
    });
    let mut cmd = std::process::Command::new("sleep");
    cmd.arg("30");
    take_watchdog_release();
    let started = Instant::now();
    let capture = output_timed_captured_cancellable(cmd, None, Some(&cancel)).unwrap();
    requester.join().unwrap();
    assert!(
        !cancel.load(Ordering::Acquire),
        "the turn was not cancelled"
    );
    (capture, started.elapsed())
}

/// A stop ends the call's process, not the turn: the wait returns as a
/// recoverable failure and says the watchdog did it.
#[cfg(target_os = "linux")]
#[test]
fn a_stop_release_ends_a_wedged_call_without_cancelling_the_turn() {
    let _guard = crate::tests::env_lock();
    let (capture, waited) = wedged_sleep_released_with(Release::Stop);
    assert!(
        waited < Duration::from_secs(10),
        "released, not waited out: {waited:?}"
    );
    assert!(capture.timed_out);
    assert!(!capture.cancelled);
    assert_eq!(take_watchdog_release(), Some(Release::Stop));
    assert!(!take_tool_idle_escalation(), "not a tool-idle escalation");
}

/// With no hand-off armed (sealed tasks, hand-off off, a non-shell wait), a
/// hand-off request still frees the turn: the call is stopped.
#[cfg(target_os = "linux")]
#[test]
fn a_handoff_release_with_no_handoff_armed_stops_the_call() {
    let _guard = crate::tests::env_lock();
    let (capture, waited) = wedged_sleep_released_with(Release::HandOff);
    assert!(
        waited < Duration::from_secs(10),
        "released, not waited out: {waited:?}"
    );
    assert!(capture.timed_out);
    assert!(capture.handed_off.is_none());
    assert_eq!(take_watchdog_release(), Some(Release::Stop));
}
