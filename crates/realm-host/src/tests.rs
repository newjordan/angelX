//! Test support the moved game tests share: the one process-wide environment
//! lock and the guard that restores a variable. (The cockpit has its own pair
//! in its test process; this crate's tests run in theirs.)

use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

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
