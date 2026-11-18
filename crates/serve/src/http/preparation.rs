//! One retained immutable browser generation, prepared away from the host's
//! frame. HTTP connection threads may wait; game/WS threads never do.
use super::{Assets, ClientBundle};
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

const GENERATIONS: usize = 2;
pub const MAX_FILES: usize = 4096;
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 256 * 1024 * 1024;
static RESIDENT: AtomicUsize = AtomicUsize::new(0);

pub fn resident() -> usize {
    RESIDENT.load(Ordering::Acquire)
}

struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        RESIDENT.fetch_sub(1, Ordering::AcqRel);
    }
}
struct State {
    outcome: Mutex<Option<Result<Arc<Assets>, String>>>,
    changed: Condvar,
    cancel: AtomicBool,
    reported: AtomicBool,
    waiters: AtomicUsize,
    permit: Mutex<Option<Permit>>,
}
struct Owner(Arc<State>);
impl Drop for Owner {
    fn drop(&mut self) {
        self.0.cancel.store(true, Ordering::Release);
        self.0.changed.notify_all();
    }
}
#[derive(Clone)]
pub struct PreparedAssets(Arc<Owner>);
impl std::fmt::Debug for PreparedAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedAssets").finish_non_exhaustive()
    }
}
impl PreparedAssets {
    pub(crate) fn ready(assets: Assets) -> Self {
        Self(Arc::new(Owner(Arc::new(State {
            outcome: Mutex::new(Some(Ok(Arc::new(assets)))),
            changed: Condvar::new(),
            cancel: AtomicBool::new(false),
            reported: AtomicBool::new(false),
            waiters: AtomicUsize::new(0),
            permit: Mutex::new(None),
        }))))
    }
    pub fn start(
        capture: impl FnOnce(&AtomicBool) -> io::Result<ClientBundle> + Send + 'static,
    ) -> io::Result<Self> {
        Self::start_static(capture, &[])
    }

    pub fn start_static(
        capture: impl FnOnce(&AtomicBool) -> io::Result<ClientBundle> + Send + 'static,
        prepared: &'static [super::StaticFile],
    ) -> io::Result<Self> {
        RESIDENT
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < GENERATIONS).then_some(n + 1)
            })
            .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "browser generations busy"))?;
        let state = Arc::new(State {
            outcome: Mutex::new(None),
            changed: Condvar::new(),
            cancel: AtomicBool::new(false),
            reported: AtomicBool::new(false),
            waiters: AtomicUsize::new(0),
            permit: Mutex::new(Some(Permit)),
        });
        let worker = state.clone();
        std::thread::Builder::new()
            .name("browser-assets".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let bundle = capture(&worker.cancel)?;
                    validate(&bundle)?;
                    if worker.cancel.load(Ordering::Acquire) {
                        return Err(super::cancelled());
                    }
                    Assets::prepare_until(&bundle, prepared, &worker.cancel).map(Arc::new)
                }))
                .unwrap_or_else(|_| Err(io::Error::other("browser preparation panicked")))
                .map_err(|e| e.to_string());
                if outcome.is_err() {
                    worker
                        .permit
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                }
                *worker.outcome.lock().unwrap_or_else(|e| e.into_inner()) = Some(outcome);
                worker.changed.notify_all();
            })?;
        Ok(Self(Arc::new(Owner(state))))
    }
    pub fn failure(&self) -> Option<String> {
        self.0
            .0
            .outcome
            .try_lock()
            .ok()?
            .as_ref()?
            .as_ref()
            .err()
            .cloned()
    }
    pub fn take_failure(&self) -> Option<String> {
        if self.0.0.reported.load(Ordering::Acquire) {
            return None;
        }
        let error = self.failure()?;
        (!self.0.0.reported.swap(true, Ordering::AcqRel)).then_some(error)
    }
    pub(crate) fn wait(&self, stop: &AtomicBool) -> Result<Arc<Assets>, ()> {
        let state = &self.0.0;
        struct Waiting<'a>(&'a AtomicUsize);
        impl Drop for Waiting<'_> {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::AcqRel);
            }
        }
        let mut waiting = None;
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut outcome = state.outcome.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if stop.load(Ordering::Acquire) || state.cancel.load(Ordering::Acquire) {
                return Err(());
            }
            if let Some(outcome) = &*outcome {
                return outcome.as_ref().cloned().map_err(|_| ());
            }
            if std::time::Instant::now() >= deadline {
                return Err(());
            }
            if waiting.is_none() {
                state
                    .waiters
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                        (n < 16).then_some(n + 1)
                    })
                    .map_err(|_| ())?;
                waiting = Some(Waiting(&state.waiters));
            }
            outcome = state
                .changed
                .wait_timeout(outcome, Duration::from_millis(20))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}
fn validate(bundle: &ClientBundle) -> io::Result<()> {
    let mut total = bundle.index_html.len();
    if total > MAX_FILE_BYTES || bundle.files.len() > MAX_FILES {
        return Err(io::Error::other("browser bundle limit"));
    }
    for (name, bytes, mime) in &bundle.files {
        if bytes.len() > MAX_FILE_BYTES || name.len() > 1024 || mime.len() > 256 {
            return Err(io::Error::other("browser file limit"));
        }
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("browser size overflow"))?;
        if total > MAX_TOTAL_BYTES {
            return Err(io::Error::other("browser total limit"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Instant;
    #[test]
    fn resident_capacity_failure_panic_and_cancellation_release_exactly() {
        let stop = AtomicBool::new(false);
        let (release, held) = mpsc::channel();
        let first = PreparedAssets::start(move |cancel| {
            held.recv().unwrap();
            assert!(cancel.load(Ordering::Acquire));
            Ok(ClientBundle::placeholder())
        })
        .unwrap();
        let second = PreparedAssets::start(|_| Ok(ClientBundle::placeholder())).unwrap();
        assert!(second.wait(&stop).is_ok());
        assert_eq!(
            PreparedAssets::start(|_| Ok(ClientBundle::placeholder()))
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        drop(first);
        assert!(PreparedAssets::start(|_| Ok(ClientBundle::placeholder())).is_err());
        release.send(()).unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        let third = loop {
            if let Ok(generation) = PreparedAssets::start(|_| panic!("capture fixture")) {
                break generation;
            }
            assert!(Instant::now() < until);
            std::thread::yield_now();
        };
        assert!(third.wait(&stop).is_err());
        assert!(third.wait(&stop).is_err());
        drop(third);
        drop(second);
        let invalid = PreparedAssets::start(|_| {
            Ok(ClientBundle {
                files: vec![(
                    "too-large".into(),
                    vec![0; MAX_FILE_BYTES + 1],
                    "text/plain".into(),
                )],
                ..ClientBundle::default()
            })
        })
        .unwrap();
        assert!(invalid.wait(&stop).is_err());
        drop(invalid);
        let until = Instant::now() + Duration::from_secs(2);
        while RESIDENT.load(Ordering::Acquire) != 0 {
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
    }
}
