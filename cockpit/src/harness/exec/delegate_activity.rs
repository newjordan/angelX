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
mod tests {
    use super::*;

    #[test]
    fn delegate_activity_is_live_owned_and_released_even_with_retained_sender() {
        let parent = AtomicBool::new(false);
        let nested = AtomicBool::new(false);
        let leaf = AtomicBool::new(false);
        let foreign = AtomicBool::new(false);
        let _link = link_child_owner(&nested, &parent);
        let _leaf_link = link_child_owner(&leaf, &nested);
        let owner = &parent as *const _ as usize;
        let mut retained = None;
        let (result, failures) = observe_delegate_turn(&leaf, "test", |tx| {
            retained = Some(tx.clone());
            tx.send(TurnEvent::Reasoning("private payload".into()))
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let state = owned_delegate_snapshot(owner).unwrap();
                if state.phase == "thinking" {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "activity was buffered until completion"
                );
                std::thread::yield_now();
            }
            assert!(owned_delegate_snapshot(&foreign as *const _ as usize).is_none());
            tx.send(TurnEvent::ToolResult {
                id: ToolEventId("failed".into()),
                name: "shell".into(),
                summary: "failed".into(),
                outcome: ToolOutcome {
                    execution: ExecutionOutcome::Failed,
                    verification: VerificationOutcome::NotApplicable,
                },
            })
            .unwrap();
            Err::<(), _>("original failure")
        });
        assert_eq!(result, Err("original failure"));
        assert_eq!(failures, 1);
        assert!(owned_delegate_snapshot(owner).is_none());
        drop(retained);
    }

    #[test]
    fn delegate_heartbeat_does_not_erase_silence_and_panic_cleans_up() {
        let cancel = AtomicBool::new(false);
        let owner = &cancel as *const _ as usize;
        let activity = Activity::new(&cancel, "test\n\u{1b}[m");
        let mut running = std::collections::BTreeMap::new();
        activity.observe(&TurnEvent::Heartbeat, &mut running);
        let state = owned_delegate_snapshot(owner).unwrap();
        assert_eq!(state.event_age_secs, Some(0));
        assert_eq!(state.progress_age_secs, None);
        assert_eq!(state.phase, "waiting for model");
        assert!(!state.label.contains('\u{1b}'));
        drop(activity);
        let result = std::panic::catch_unwind(|| {
            observe_delegate_turn(&cancel, "test", |_| panic!("worker panic"));
        });
        assert!(result.is_err());
        assert!(owned_delegate_snapshot(owner).is_none());
    }
}
