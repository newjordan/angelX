//! Test support the moved game tests share: the one process-wide environment
//! lock and the guard that restores a variable. (The cockpit has its own pair
//! in its test process; this crate's tests run in theirs.)

use std::cell::Cell;
use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub(crate) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

thread_local! {
    static CAPACITY: Cell<bool> = const { Cell::new(false) };
}

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
        unsafe { std::env::set_var(key, value) };
        Self { key, old }
    }

    pub(crate) fn unset(key: &'static str) -> Self {
        let old = std::env::var_os(key);
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
