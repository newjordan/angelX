//! Test support the moved game tests share: the one process-wide environment
//! lock and the guard that restores a variable. (The cockpit has its own pair
//! in its test process; this crate's tests run in theirs.)

use std::cell::Cell;
use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// THE process-wide env lock for this crate's tests. Process environment is
/// global, so per-module locks can't serialize against each other.
pub(crate) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    // Poison-tolerant: a test that panics while holding this must not cascade
    // `PoisonError` into every other env-serialized test. The guard only
    // serializes process-global env access; there is no protected invariant.
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

thread_local! {
    /// Whether this test's thread holds the browser capacity (`browser_capacity`).
    static CAPACITY: Cell<bool> = const { Cell::new(false) };
}

/// The process's whole browser capacity, for one test. Two browser
/// generations may be resident at once (`angel_serve::http::preparation`),
/// the tests of one process share them, and a delve that is hosted, or
/// restored from a hosted checkpoint, keeps one for as long as it lives. Run
/// side by side such tests were turned away ("browser generations busy") by
/// whoever else held them, and the test that hosts two to see a third
/// refused found one gone. So every test that admits a generation holds this
/// first (`together_web::shipped_prepared` checks): it takes `env_lock` too,
/// as hosting reads the environment, and waits until the generations an
/// earlier test dropped have retired, which their workers do a few
/// milliseconds after the drop.
pub(crate) struct BrowserCapacity {
    _env: std::sync::MutexGuard<'static, ()>,
}

pub(crate) fn browser_capacity() -> BrowserCapacity {
    let env = env_lock();
    let until = Instant::now() + Duration::from_secs(60);
    while angel_serve::http::preparation::resident() != 0 {
        assert!(Instant::now() < until, "an earlier test's browser generations never retired");
        std::thread::sleep(Duration::from_millis(5));
    }
    CAPACITY.with(|held| held.set(true));
    BrowserCapacity { _env: env }
}

impl Drop for BrowserCapacity {
    fn drop(&mut self) {
        CAPACITY.with(|held| held.set(false));
    }
}

/// Whether the calling test holds the browser capacity.
pub(crate) fn capacity_held() -> bool {
    CAPACITY.with(Cell::get)
}

pub(crate) struct TestEnvGuard {
    key: &'static str,
    old: Option<OsString>,
}

impl TestEnvGuard {
    pub(crate) fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var_os(key);
        // The caller holds `env_lock()`.
        unsafe { std::env::set_var(key, value) };
        Self { key, old }
    }

    /// Clear a variable for the lifetime of the guard, restoring the prior value on drop.
    pub(crate) fn unset(key: &'static str) -> Self {
        let old = std::env::var_os(key);
        // The caller holds `env_lock()`.
        unsafe { std::env::remove_var(key) };
        Self { key, old }
    }
}

impl Drop for TestEnvGuard {
    fn drop(&mut self) {
        if let Some(old) = &self.old {
            unsafe { std::env::set_var(self.key, old) };
        } else {
            unsafe { std::env::remove_var(self.key) };
        }
    }
}
