//! What an edit to a live module became. The worker publishes after every
//! iteration; a watch opened before a write settles once two scans have
//! completed after it (the second began after the write landed) and each
//! watched module holds no unjudged candidate. Nothing here reads files or
//! evaluates JavaScript.
use super::{Registry, Snapshot, Status};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a caller may wait for a verdict before the outcome is delivered
/// later instead. Measured edit-to-verdict: 0.3 s for a JavaScript edit (two
/// scans past the 120 ms debounce), 0.95 s for a Bend source edit (build and
/// proofs included, accepted or rejected).
pub const WINDOW: Duration = Duration::from_millis(2000);
const MAX_WATCHES: usize = 32;
const MAX_ERROR: usize = 2000;
const SLICE: Duration = Duration::from_millis(50);

/// The loader's verdict on one module after an edit: the same status the
/// `/reload status` line shows, and how long the verdict took.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub status: Status,
    pub took: Duration,
}

impl Outcome {
    /// The `/reload status` heading followed by one line per outcome.
    pub fn text(root: &Path, outcomes: &[Outcome]) -> String {
        let mut lines = vec![Snapshot::heading(root)];
        lines.extend(outcomes.iter().map(|o| o.status.line(Some(o.took))));
        lines.join("\n")
    }
}

#[derive(Default)]
pub struct Board {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    scans: u64,
    modules: BTreeMap<String, (Status, bool)>,
    watches: Vec<Watch>,
    serial: u64,
}

struct Watch {
    serial: u64,
    opened: Instant,
    scans: u64,
    before: BTreeMap<String, Option<Status>>,
    waiting: bool,
    done: Option<Vec<Outcome>>,
}

impl Board {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Open a watch on `ids` just before a write lands in them.
    pub fn watch(&self, ids: &[String]) -> u64 {
        let mut state = self.lock();
        state.serial += 1;
        let watch = Watch {
            serial: state.serial,
            opened: Instant::now(),
            scans: state.scans,
            before: ids
                .iter()
                .map(|id| {
                    let status = state.modules.get(id).map(|(status, _)| status.clone());
                    (id.clone(), status)
                })
                .collect(),
            waiting: false,
            done: None,
        };
        if state.watches.len() >= MAX_WATCHES {
            state.watches.remove(0);
        }
        let serial = watch.serial;
        state.watches.push(watch);
        serial
    }

    /// Close a watch whose write did not land.
    pub fn forget(&self, watch: u64) {
        self.lock().watches.retain(|open| open.serial != watch);
    }

    /// Wait up to `window` for the watch to settle. `Some` holds the modules
    /// the edit changed or left failing, and is empty when it changed nothing.
    /// `None` means no verdict yet: the watch stays open for [`Board::take`].
    pub fn settle(
        &self,
        watch: u64,
        window: Duration,
        stop: &dyn Fn() -> bool,
    ) -> Option<Vec<Outcome>> {
        let until = Instant::now() + window;
        let mut state = self.lock();
        state
            .watches
            .iter_mut()
            .find(|open| open.serial == watch)?
            .waiting = true;
        loop {
            let open = state.watches.iter().position(|open| open.serial == watch)?;
            if state.watches[open].done.is_some() {
                return state.watches.remove(open).done;
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() || stop() {
                state.watches[open].waiting = false;
                return None;
            }
            state = self
                .changed
                .wait_timeout(state, left.min(SLICE))
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }

    /// Verdicts reached after their writer stopped waiting, each delivered once.
    pub fn take(&self) -> Vec<Outcome> {
        let mut delivered: Vec<Outcome> = Vec::new();
        self.lock().watches.retain_mut(|open| {
            if open.waiting {
                return true;
            }
            let Some(done) = open.done.take() else {
                return true;
            };
            for outcome in done {
                if !delivered.contains(&outcome) {
                    delivered.push(outcome);
                }
            }
            false
        });
        delivered
    }

    /// Called after the UI snapshot is visible, so a verdict is never ahead of it.
    pub(super) fn publish(&self, scan: Scan) {
        let mut state = self.lock();
        let State {
            scans,
            modules,
            watches,
            ..
        } = &mut *state;
        *scans = scan.scans;
        *modules = scan.modules;
        let now = Instant::now();
        let mut finished = false;
        for open in watches.iter_mut().filter(|open| open.done.is_none()) {
            let settled = open
                .before
                .keys()
                .all(|id| modules.get(id).is_none_or(|(_, settled)| *settled));
            if *scans < open.scans + 2 || !settled {
                continue;
            }
            let took = now.saturating_duration_since(open.opened);
            open.done = Some(
                open.before
                    .iter()
                    .filter_map(|(id, before)| {
                        let (status, _) = modules.get(id)?;
                        (before.as_ref() != Some(status) || status.error.is_some()).then(|| {
                            Outcome {
                                status: bounded(status),
                                took,
                            }
                        })
                    })
                    .collect(),
            );
            finished = true;
        }
        drop(state);
        if finished {
            self.changed.notify_all();
        }
    }
}

/// The registry's per-module status and whether its candidate is judged.
pub(super) struct Scan {
    scans: u64,
    modules: BTreeMap<String, (Status, bool)>,
}

impl Scan {
    pub(super) fn of(registry: &Registry, snapshot: &Snapshot) -> Self {
        Self {
            scans: registry.scans,
            modules: snapshot
                .statuses
                .iter()
                .map(|status| {
                    let settled = registry.settled(&status.id);
                    (status.id.clone(), (status.clone(), settled))
                })
                .collect(),
        }
    }
}

fn bounded(status: &Status) -> Status {
    let mut status = status.clone();
    if let Some(error) = &mut status.error
        && error.chars().count() > MAX_ERROR
    {
        *error = error
            .chars()
            .take(MAX_ERROR)
            .chain(std::iter::once('…'))
            .collect();
    }
    status
}
