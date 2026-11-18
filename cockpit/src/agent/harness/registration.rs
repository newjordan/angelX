//! Revertible registration — the provider side of the revertible-effect
//! discipline in *A Programming Paradigm for Spatiotemporal Composability*
//! (arXiv 2608.25512, §3.1 and §5.1.1).
//!
//! An effect in that model is a pair: a transformation of the shared context
//! together with the inverse that undoes it, handed back at the point of
//! application (§3.1.2, Definition 8). A registration in the tool registry is
//! exactly such an effect — it installs a capability, a schema, a child
//! process, a temporary directory — so it hands back a [`Disposable`] that
//! reverts it, and a [`RegistrationScope`] accumulates those inverses.
//!
//! Three properties are load-bearing, and each one is witnessed by
//! `harness/tests/registration_inverse.rs`:
//!
//! * **Once-only.** An inverse fires at most once. Firing twice would apply an
//!   inverse at a state that no application of the effect produced (§5.1.1), so
//!   the second call is a no-op instead of a double free.
//! * **LIFO.** Inverses fire in reverse order of registration — the fold
//!   `inverse ← value ∘ inverse` of §5.1.1, Algorithm 1.
//! * **Cascade.** A nested scope's recovery is itself an effect on the parent,
//!   so disposing the parent disposes its children (§5.1.1 parent composition).
//!
//! What the runtime does *not* check is the witness: that an inverse really
//! reverts the effect it accompanies is an obligation on the provider rather
//! than a property the runtime verifies (§5.1.1, §6.1). In this cockpit that
//! obligation is discharged by tests, which is the only witness we accept.

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

type Inverse = Box<dyn FnOnce() + Send + Sync>;

pub struct Disposable {
    armed: AtomicBool,
    undo: Mutex<Option<Inverse>>,
}

impl Disposable {
    pub fn new(undo: impl FnOnce() + Send + Sync + 'static) -> Self {
        Self {
            armed: AtomicBool::new(true),
            undo: Mutex::new(Some(Box::new(undo))),
        }
    }

    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::Acquire)
    }

    pub fn fire(&self) -> bool {
        if !self.armed.swap(false, Ordering::AcqRel) {
            return false;
        }
        let undo = self
            .undo
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(undo) = undo {
            undo();
        }
        true
    }
}

impl Drop for Disposable {
    fn drop(&mut self) {
        self.fire();
    }
}

pub struct RegistrationScope {
    entries: Arc<Mutex<Vec<Arc<Disposable>>>>,
    armed: Arc<AtomicBool>,
}

impl Default for RegistrationScope {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistrationScope {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            armed: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn track(&self, disposable: Arc<Disposable>) -> Arc<Disposable> {
        self.entries().push(Arc::clone(&disposable));
        disposable
    }

    pub fn track_inverse(&self, undo: impl FnOnce() + Send + Sync + 'static) -> Arc<Disposable> {
        self.track(Arc::new(Disposable::new(undo)))
    }

    pub fn len(&self) -> usize {
        self.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }

    pub fn dispose(&self) -> usize {
        if !self.armed.swap(false, Ordering::AcqRel) {
            return 0;
        }
        let drained = std::mem::take(&mut *self.entries());
        let mut fired = 0;
        for entry in drained.iter().rev() {
            if entry.fire() {
                fired += 1;
            }
        }
        fired
    }

    pub fn into_disposable(self) -> Disposable {
        let entries = Arc::clone(&self.entries);
        self.armed.store(false, Ordering::Release);
        Disposable::new(move || {
            let drained = {
                let mut guard = entries
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                std::mem::take(&mut *guard)
            };
            for entry in drained.iter().rev() {
                entry.fire();
            }
        })
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, Vec<Arc<Disposable>>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Drop for RegistrationScope {
    fn drop(&mut self) {
        self.dispose();
    }
}

pub struct ProviderScope {
    remaining: AtomicUsize,
    teardown: Mutex<Option<Inverse>>,
}

impl ProviderScope {
    pub fn new(registrations: usize, teardown: impl FnOnce() + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            remaining: AtomicUsize::new(registrations),
            teardown: Mutex::new(Some(Box::new(teardown))),
        })
    }

    pub fn registration(self: &Arc<Self>) -> Arc<Disposable> {
        let scope = Arc::clone(self);
        Arc::new(Disposable::new(move || {
            scope.release_one();
        }))
    }

    pub fn remaining(&self) -> usize {
        self.remaining.load(Ordering::Acquire)
    }

    fn release_one(&self) -> bool {
        let left = self
            .remaining
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
            .map(|previous| previous - 1)
            .unwrap_or(0);
        if left > 0 {
            return false;
        }
        let teardown = self
            .teardown
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(teardown) = teardown {
            teardown();
            return true;
        }
        false
    }
}
