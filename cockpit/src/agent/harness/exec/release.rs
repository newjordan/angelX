//! Watchdog release: the loop watchdog (on the UI thread) asks the foreground
//! wait of a wedged turn to let go of the call it is blocked on. The request
//! is keyed by the turn's root owner and stamped when it is made, so it only
//! reaches a call that was already running then: a later call of the same
//! turn never inherits it.
//!
//! [`Release::HandOff`] moves the call to the background-job table exactly as
//! the hand-off limit would (the process keeps running); where no hand-off is
//! armed, or filing the job fails, the call is stopped instead.
//! [`Release::Stop`] stops the call's process group. Either way the tool
//! returns and the turn continues; the turn worker reads which one happened
//! with [`take_watchdog_release`] and tells the model.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Release {
    HandOff,
    Stop,
}

#[derive(Clone, Copy, Debug)]
struct Request {
    release: Release,
    issued: Instant,
}

fn requests() -> &'static Mutex<HashMap<usize, Request>> {
    static REQUESTS: OnceLock<Mutex<HashMap<usize, Request>>> = OnceLock::new();
    REQUESTS.get_or_init(Mutex::default)
}

thread_local! {
    // Trusted execution metadata: set by the wait that honoured a release.
    static RELEASED: std::cell::Cell<Option<Release>> = const { std::cell::Cell::new(None) };
}

/// Ask the call the turn rooted at `owner` is blocked on to let go.
pub(crate) fn request_release(owner: usize, release: Release) {
    requests().lock().unwrap_or_else(|e| e.into_inner()).insert(
        owner,
        Request {
            release,
            issued: Instant::now(),
        },
    );
}

/// Withdraw any release still waiting for the turn rooted at `owner`.
pub(crate) fn clear_release(owner: usize) {
    requests()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&owner);
}

/// Whether a release is still waiting for the turn rooted at `owner`.
pub(crate) fn release_pending(owner: usize) -> bool {
    requests()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains_key(&owner)
}

/// The release for a call (owned by `owner`, a root or linked owner) that
/// started at `call_started`, consumed. A request made before the call
/// started is left alone.
pub(super) fn take_release(owner: usize, call_started: Instant) -> Option<Release> {
    let root = super::activity::root_owner(owner);
    let mut requests = requests().lock().unwrap_or_else(|e| e.into_inner());
    let request = *requests.get(&root)?;
    if request.issued < call_started {
        return None;
    }
    requests.remove(&root);
    Some(request.release)
}

pub(super) fn note_released(release: Release) {
    RELEASED.with(|cell| cell.set(Some(release)));
}

/// What the watchdog released on this thread's last call, consumed.
pub(crate) fn take_watchdog_release() -> Option<Release> {
    RELEASED.with(|cell| cell.take())
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/exec__release__tests.rs"]
mod tests;
