//! Opt-in, payload-free breadcrumbs for work before and between provider calls.
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

struct Trace {
    id: u64,
    started: Instant,
}
thread_local! {
    static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) };
}
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Nested harness entry shares the worker clock; independent turns get a new id.
pub(crate) struct Scope(bool);
impl Scope {
    pub(crate) fn enter() -> Self {
        let owns = TRACE.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_some() || std::env::var("ANGEL_TURN_PHASE_TRACE").as_deref() != Ok("1") {
                return false;
            }
            *slot = Some(Trace {
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                started: Instant::now(),
            });
            true
        });
        if owns {
            mark("start");
        }
        Self(owns)
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if self.0 {
            mark("end");
            TRACE.with(|slot| *slot.borrow_mut() = None);
        }
    }
}

/// Labels are static, never prompts, tool arguments, credentials, or paths.
pub(crate) fn mark(phase: &'static str) {
    TRACE.with(|slot| {
        if let Some(trace) = slot.borrow().as_ref() {
            eprintln!(
                "[turn-phase] pid={} turn={} elapsed_ms={} phase={phase}",
                std::process::id(),
                trace.id,
                trace.started.elapsed().as_millis()
            );
        }
    });
}

pub(crate) struct Hop;
impl Drop for Hop {
    fn drop(&mut self) {
        mark("hop_end");
    }
}
