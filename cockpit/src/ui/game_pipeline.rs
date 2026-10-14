//! The game's pictures, made off the terminal's thread.
//!
//! The terminal asks each frame for the picture it wants (a key and a
//! recipe: blend the snapshot pair, draw, scale, encode). One worker makes
//! it while the terminal goes on presenting. A request replaces the one
//! still waiting, since only the newest matters, and the terminal places the
//! newest finished picture. It waits a few milliseconds for this frame's
//! own picture, then shows the last one rather than hold the frame.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Draw and encode one picture: PNG bytes, or None when it cannot be made.
pub(crate) type Recipe = Box<dyn FnOnce() -> Option<Vec<u8>> + Send>;

/// A finished picture: its key and its PNG.
pub(crate) type Finished = (u64, Arc<Vec<u8>>);

#[derive(Default)]
struct Slot {
    pending: Option<(u64, Recipe)>,
    /// The key being made now.
    making: Option<u64>,
    done: Option<Finished>,
    stop: bool,
}

struct Shared {
    slot: Mutex<Slot>,
    changed: Condvar,
}

fn lock(shared: &Shared) -> MutexGuard<'_, Slot> {
    shared
        .slot
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) struct Pipeline {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Pipeline::new()
    }
}

impl Pipeline {
    pub(crate) fn new() -> Pipeline {
        let shared = Arc::new(Shared {
            slot: Mutex::new(Slot::default()),
            changed: Condvar::new(),
        });
        let worker = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("delve-render".into())
                .spawn(move || work(&shared))
                .ok()
        };
        Pipeline { shared, worker }
    }

    /// Ask for the picture `key`. Nothing happens when it is already made
    /// or being made; a recipe still waiting for the worker is replaced.
    pub(crate) fn request(&self, key: u64, recipe: Recipe) {
        let mut slot = lock(&self.shared);
        if slot.done.as_ref().is_some_and(|(k, _)| *k == key)
            || slot.making == Some(key)
            || slot.pending.as_ref().is_some_and(|(k, _)| *k == key)
        {
            return;
        }
        slot.pending = Some((key, recipe));
        drop(slot);
        self.shared.changed.notify_all();
    }

    /// The picture `key`, waiting at most `budget` for it to be finished.
    pub(crate) fn wait(&self, key: u64, budget: Duration) -> Option<Arc<Vec<u8>>> {
        let deadline = Instant::now() + budget;
        let mut slot = lock(&self.shared);
        loop {
            if let Some((k, png)) = &slot.done
                && *k == key
            {
                return Some(Arc::clone(png));
            }
            let waiting =
                slot.making == Some(key) || slot.pending.as_ref().is_some_and(|(k, _)| *k == key);
            let rest = deadline.saturating_duration_since(Instant::now());
            if !waiting || rest.is_zero() {
                return None;
            }
            slot = self
                .shared
                .changed
                .wait_timeout(slot, rest)
                .map_or_else(|p| p.into_inner().0, |(g, _)| g);
        }
    }

    /// The newest finished picture.
    pub(crate) fn latest(&self) -> Option<Finished> {
        lock(&self.shared).done.clone()
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        lock(&self.shared).stop = true;
        self.shared.changed.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn work(shared: &Shared) {
    loop {
        let (key, recipe) = {
            let mut slot = lock(shared);
            loop {
                if slot.stop {
                    return;
                }
                if let Some(job) = slot.pending.take() {
                    slot.making = Some(job.0);
                    break job;
                }
                slot = shared
                    .changed
                    .wait(slot)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };
        let png = recipe();
        let mut slot = lock(shared);
        slot.making = None;
        if let Some(png) = png {
            slot.done = Some((key, Arc::new(png)));
        }
        drop(slot);
        shared.changed.notify_all();
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/game_pipeline__tests.rs"]
mod tests;
