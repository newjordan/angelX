//! Display-only delegate activity, consumed while the specialist is running.
//! Store bounded labels and timestamps, never child prose, reasoning or args.
use super::*;
use std::sync::{Mutex, OnceLock};

struct State {
    started: Instant,
    event: Option<Instant>,
    progress: Option<Instant>,
    label: String,
    phase: String,
    calls: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct DelegateSnapshot {
    pub(crate) label: String,
    pub(crate) phase: String,
    pub(crate) elapsed_secs: u64,
    pub(crate) event_age_secs: Option<u64>,
    pub(crate) progress_age_secs: Option<u64>,
    pub(crate) calls: usize,
    pub(crate) delegates: usize,
}

type Key = (usize, u64);
fn entries() -> &'static Mutex<HashMap<Key, State>> {
    static ENTRIES: OnceLock<Mutex<HashMap<Key, State>>> = OnceLock::new();
    ENTRIES.get_or_init(Mutex::default)
}

pub(crate) fn owned_delegate_snapshot(owner: usize) -> Option<DelegateSnapshot> {
    let owner = super::activity::root_owner(owner);
    let entries = entries().lock().unwrap_or_else(|e| e.into_inner());
    let mut owned = entries.iter().filter(|((key, _), _)| *key == owner);
    let (_, mut latest) = owned.next()?;
    let mut delegates = 1;
    for (_, state) in owned {
        delegates += 1;
        if (state.progress, state.started) > (latest.progress, latest.started) {
            latest = state;
        }
    }
    Some(DelegateSnapshot {
        label: latest.label.clone(),
        phase: latest.phase.clone(),
        elapsed_secs: latest.started.elapsed().as_secs(),
        event_age_secs: latest.event.map(|at| at.elapsed().as_secs()),
        progress_age_secs: latest.progress.map(|at| at.elapsed().as_secs()),
        calls: latest.calls,
        delegates,
    })
}

fn label(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).take(48).collect()
}

struct Activity(Key);
impl Activity {
    fn new(cancel: &AtomicBool, club: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let key = (
            super::activity::root_owner(cancel as *const _ as usize),
            SEQ.fetch_add(1, Ordering::Relaxed),
        );
        entries().lock().unwrap_or_else(|e| e.into_inner()).insert(
            key,
            State {
                started: Instant::now(),
                event: None,
                progress: None,
                label: label(club),
                phase: "waiting for model".into(),
                calls: 0,
            },
        );
        Self(key)
    }

    fn observe(
        &self,
        event: &TurnEvent,
        running: &mut std::collections::BTreeMap<ToolEventId, String>,
    ) {
        let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = entries.get_mut(&self.0) else {
            return;
        };
        state.event = Some(Instant::now());
        let phase = match event {
            TurnEvent::Token(text) if !text.is_empty() => Some("responding".into()),
            TurnEvent::Reasoning(text) if !text.is_empty() => Some("thinking".into()),
            TurnEvent::ToolCall { id, name, .. } => {
                state.calls += 1;
                running.insert(id.clone(), label(name));
                Some(format!("tool {}", label(name)))
            }
            TurnEvent::ToolResult { id, .. } => {
                running.remove(id);
                Some(running.last_key_value().map_or_else(
                    || "waiting for model".into(),
                    |(_, name)| format!("tool {name}"),
                ))
            }
            // Heartbeats/notices are liveness, never productive output.
            _ => None,
        };
        if let Some(phase) = phase {
            state.progress = state.event;
            state.phase = phase;
        }
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        entries()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}

/// The worker remains on its original thread (preserving thread-local lineage).
/// A scoped observer consumes live events and retains the original failure count.
/// A completion flag also bounds shutdown if a provider retains a sender clone.
pub(crate) fn observe_delegate_turn<T>(
    cancel: &AtomicBool,
    club: &str,
    run: impl FnOnce(&mpsc::Sender<TurnEvent>) -> T,
) -> (T, usize) {
    let activity = Activity::new(cancel, club);
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        let activity = &activity;
        let done = &done;
        let observer = scope.spawn(move || {
            let mut failures = 0;
            let mut running = std::collections::BTreeMap::new();
            let mut observe = |event| {
                activity.observe(&event, &mut running);
                if matches!(event, TurnEvent::ToolResult { outcome, .. }
                    if outcome.execution != ExecutionOutcome::Succeeded)
                {
                    failures += 1;
                }
            };
            while !done.load(Ordering::Acquire) {
                match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(event) => observe(event),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            // Capture every event already queued by the completed turn. Do not
            // wait for an accidentally retained sender to close.
            for event in rx.try_iter() {
                observe(event);
            }
            failures
        });
        struct Finish<'a>(&'a AtomicBool);
        impl Drop for Finish<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let finish = Finish(done);
        let result = run(&tx);
        drop(finish);
        drop(tx);
        (
            result,
            observer
                .join()
                .expect("delegate activity observer panicked"),
        )
    })
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/exec__delegate_activity__tests.rs"]
mod tests;
