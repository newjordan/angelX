//! What the host sets once, before play: the cockpit from its environment
//! (`ANGEL_DELVE_FEEL`, `ANGEL_DELVE_OPEN`). Unset, everything is off.
//!
//! Tests (and the cockpit's tests, through the `testing` feature) read the
//! same environment themselves when no host has set anything.

use std::sync::atomic::{AtomicBool, Ordering};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

/// The host's choices for the whole process.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// Keep the feel ledger (`ANGEL_DELVE_FEEL=1`).
    pub feel_ledger: bool,
    /// Lay every delve floor open (`ANGEL_DELVE_OPEN=1`): one map to
    /// explore, no doors between fights (`rules::depths`). A run takes it
    /// when it starts (`Run::open_floors`).
    pub open_floors: bool,
}

static FEEL_LEDGER: AtomicBool = AtomicBool::new(false);
static OPEN_FLOORS: AtomicBool = AtomicBool::new(false);

struct ScopedState {
    next_token: u64,
    stack: Vec<(u64, Settings)>,
}

impl ScopedState {
    fn token(&mut self) -> u64 {
        loop {
            let token = self.next_token;
            self.next_token = self.next_token.wrapping_add(1);
            if !self.stack.iter().any(|(live, _)| *live == token) {
                return token;
            }
        }
    }
}

thread_local! {
    // Context scopes are intentionally thread-local: the rules reducer may
    // temporarily install an immutable session context without changing the
    // process-wide host settings used by legacy callers.
    static SCOPED: RefCell<ScopedState> = RefCell::new(ScopedState { next_token: 0, stack: Vec::new() });
}

/// A thread-bound temporary settings override. Nested scopes restore the
/// previous visible value on drop, including during unwinding.
pub struct Scope {
    token: u64,
    _thread_bound: PhantomData<Rc<()>>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        SCOPED.with(|state| {
            state
                .borrow_mut()
                .stack
                .retain(|(token, _)| *token != self.token);
        });
    }
}

/// Temporarily override settings for this thread only.
pub fn scoped(settings: Settings) -> Scope {
    SCOPED.with(|state| {
        let mut state = state.borrow_mut();
        let token = state.token();
        state.stack.push((token, settings));
        Scope {
            token,
            _thread_bound: PhantomData,
        }
    })
}

/// Take the host's settings.
pub fn set(settings: Settings) {
    #[cfg(any(test, feature = "testing"))]
    from_env();
    FEEL_LEDGER.store(settings.feel_ledger, Ordering::Relaxed);
    OPEN_FLOORS.store(settings.open_floors, Ordering::Relaxed);
}

/// The settings in force.
pub fn get() -> Settings {
    if let Some(settings) =
        SCOPED.with(|state| state.borrow().stack.last().map(|(_, value)| *value))
    {
        return settings;
    }
    #[cfg(any(test, feature = "testing"))]
    from_env();
    Settings {
        feel_ledger: FEEL_LEDGER.load(Ordering::Relaxed),
        open_floors: OPEN_FLOORS.load(Ordering::Relaxed),
    }
}

/// Under test, the environment stands in for a host, once, before anything
/// a test sets.
#[cfg(any(test, feature = "testing"))]
fn from_env() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        FEEL_LEDGER.store(
            std::env::var("ANGEL_DELVE_FEEL").is_ok_and(|v| v == "1"),
            Ordering::Relaxed,
        );
        OPEN_FLOORS.store(
            std::env::var("ANGEL_DELVE_OPEN").is_ok_and(|v| v == "1"),
            Ordering::Relaxed,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn options(feel_ledger: bool, open_floors: bool) -> Settings {
        Settings {
            feel_ledger,
            open_floors,
        }
    }

    #[test]
    fn nested_scope_restores_the_previous_settings() {
        let baseline = scoped(options(false, false));
        let outer = scoped(options(true, false));
        assert_eq!(get(), options(true, false));
        {
            let _inner = scoped(options(false, true));
            assert_eq!(get(), options(false, true));
        }
        assert_eq!(get(), options(true, false));
        drop(outer);
        assert_eq!(get(), options(false, false));
        drop(baseline);
    }

    #[test]
    fn out_of_order_drop_and_counter_wrap_keep_live_tokens_distinct() {
        SCOPED.with(|state| state.borrow_mut().next_token = u64::MAX);
        let baseline = scoped(options(false, false));
        let outer = scoped(options(true, false));
        let inner = scoped(options(false, true));
        assert_eq!(
            SCOPED.with(|state| state.borrow().stack.last().unwrap().0),
            1
        );
        // Force allocation to wrap over all three live IDs; it must find 2.
        SCOPED.with(|state| state.borrow_mut().next_token = u64::MAX);
        let newer = scoped(options(true, true));
        let tokens: Vec<_> = SCOPED.with(|state| {
            state
                .borrow()
                .stack
                .iter()
                .map(|(token, _)| *token)
                .collect()
        });
        assert_eq!(tokens, vec![u64::MAX, 0, 1, 2]);
        drop(outer); // An outer scope may end before its inner scopes.
        assert_eq!(get(), options(true, true));
        drop(newer);
        assert_eq!(get(), options(false, true));
        drop(inner);
        assert_eq!(get(), options(false, false));
        drop(baseline);
    }

    #[test]
    fn scope_restores_after_unwind() {
        let baseline = scoped(options(false, false));
        let result = std::panic::catch_unwind(|| {
            let _scope = scoped(options(true, true));
            assert_eq!(get(), options(true, true));
            panic!("exercise scope drop during unwind");
        });
        assert!(result.is_err());
        assert_eq!(get(), options(false, false));
        drop(baseline);
    }

    #[test]
    fn concurrent_threads_keep_distinct_scopes() {
        let baseline = scoped(options(false, false));
        let rendezvous = Arc::new(Barrier::new(3));
        let threads = [options(true, false), options(false, true)].map(|expected| {
            let rendezvous = rendezvous.clone();
            std::thread::spawn(move || {
                let _scope = scoped(expected);
                rendezvous.wait();
                assert_eq!(get(), expected);
                rendezvous.wait();
            })
        });
        rendezvous.wait();
        assert_eq!(get(), options(false, false));
        rendezvous.wait();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(get(), options(false, false));
        drop(baseline);
    }
}
