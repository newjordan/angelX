//! Agent-activity telemetry hook.
//!
//! Moa/swarm pipeline stages and `spawn` formations publish their current
//! fan-out here (stage name + one label per seat). The miniworld reads the
//! snapshot each tick and musters an army for it — soldiers for proposer
//! waves, magistrates for judge panels, sentries for verify rounds. Publishing
//! is a lock-and-swap (worker threads), reading is a cheap clone (UI tick).

use std::sync::Mutex;

/// One seat's lifecycle within the current stage. `Running` is the implied
/// default for any seat without a published update, so `seat_states` can stay
/// empty on publish and existing [`stage`] callers keep working unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeatState {
    #[default]
    Running,
    /// The seat came back with a usable result.
    Returned,
    /// The seat came back with an error.
    Failed,
    /// The seat was abandoned at the wave deadline/quorum/cancel barrier.
    Cut,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StageSnapshot {
    /// Stable publish identity; seat updates change `seq`, not this identity.
    pub stage_id: u64,
    pub name: String,
    pub agents: Vec<String>,
    /// Per-seat lifecycle, index-aligned with `agents`. Starts empty on every
    /// publish (all seats implicitly `Running`) and may stay shorter than
    /// `agents`; read through [`StageSnapshot::seat_state`].
    pub seat_states: Vec<SeatState>,
    /// Monotonic per publish — the world re-musters when this moves. Seat
    /// updates bump it too, so consumers keyed off the seq re-read for pips.
    pub seq: u64,
}

impl StageSnapshot {
    /// The state of seat `idx`; `Running` when no update has been published.
    #[cfg(test)]
    pub fn seat_state(&self, idx: usize) -> SeatState {
        self.seat_states.get(idx).copied().unwrap_or_default()
    }

    /// How many seats have come back with a result (`Returned`) — the "3/6
    /// back" count for status rows and the portal.
    pub fn returned(&self) -> usize {
        self.seat_states
            .iter()
            .filter(|s| matches!(s, SeatState::Returned))
            .count()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActivitySnapshot {
    /// Monotonic revision of the activity signal, including a transition to
    /// the empty state. Portal frames use this to reject late results.
    pub sequence: u64,
    pub stage: Option<StageSnapshot>,
}

#[derive(Debug)]
struct StageState {
    sequence: u64,
    current: Option<StageSnapshot>,
}

static STAGE: Mutex<StageState> = Mutex::new(StageState {
    sequence: 0,
    current: None,
});

pub fn stage(name: impl Into<String>, agents: Vec<String>) {
    let Ok(mut g) = STAGE.lock() else { return };
    g.sequence = g.sequence.saturating_add(1);
    let seq = g.sequence;
    g.current = Some(StageSnapshot {
        stage_id: seq,
        name: name.into(),
        agents,
        seat_states: Vec::new(),
        seq,
    });
}

/// Publish one seat's lifecycle within the *current* stage. A no-op when no
/// stage is live or `idx` falls outside its roster — a straggler reporting
/// after the next stage re-published must not corrupt the new formation.
/// Bumps the sequence only on a real change, so seq-keyed consumers never
/// re-form for a repeat.
pub fn stage_seat_update(idx: usize, state: SeatState) {
    if !seat_states_enabled() {
        return;
    }
    let Ok(mut g) = STAGE.lock() else { return };
    let changed = match g.current.as_mut() {
        Some(cur) if idx < cur.agents.len() => {
            if cur.seat_states.len() <= idx {
                cur.seat_states.resize(idx + 1, SeatState::default());
            }
            if cur.seat_states[idx] == state {
                false
            } else {
                cur.seat_states[idx] = state;
                true
            }
        }
        _ => false,
    };
    if changed {
        g.sequence = g.sequence.saturating_add(1);
        let seq = g.sequence;
        if let Some(cur) = g.current.as_mut() {
            cur.seq = seq;
        }
    }
}

/// `ANGEL_VIZ_SEAT_STATES=0` opts out of per-seat telemetry (the stage-level
/// muster keeps working). Read once; the gate speaks when it suppresses.
fn seat_states_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        let on = !matches!(
            std::env::var("ANGEL_VIZ_SEAT_STATES")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "0" | "off" | "false" | "no"
        );
        if !on {
            eprintln!("AGENTVIZ: per-seat states disabled (ANGEL_VIZ_SEAT_STATES)");
        }
        on
    })
}

/// The latest published stage, if any. `None` after [`clear`] — the army
/// disbands.
pub fn current() -> Option<StageSnapshot> {
    activity().stage
}

/// Compact right-rail pips for the live tool strip. Formats under the lock so
/// the draw path does not clone the stage snapshot (name + roster) each frame.
pub(crate) fn current_seat_pips() -> Option<String> {
    let Ok(g) = STAGE.lock() else {
        return None;
    };
    let stage = g.current.as_ref()?;
    if stage.agents.len() <= 1 || stage.seat_states.is_empty() {
        return None;
    }
    Some(format!(
        "{} · {}/{} back · ",
        stage.name,
        stage.returned(),
        stage.agents.len()
    ))
}

/// Read the current activity and its revision under one lock. Unlike
/// [`current`], this preserves the sequence of an empty/disbanded state.
pub fn activity() -> ActivitySnapshot {
    STAGE
        .lock()
        .map(|g| ActivitySnapshot {
            sequence: g.sequence,
            stage: g.current.clone(),
        })
        .unwrap_or_default()
}

/// Turn over — drop the stage so the world disbands the muster.
pub fn clear() {
    if let Ok(mut g) = STAGE.lock()
        && g.current.take().is_some()
    {
        g.sequence = g.sequence.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::SeatState;

    /// Both tests mutate the process-global stage; serialize them so the
    /// parallel test runner can't interleave a `clear` into an assertion.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn publish_roundtrips_and_clear_disbands() {
        let _serial = serial();
        super::stage("proposers", vec!["a".into(), "b".into()]);
        let s = super::current().expect("published stage visible");
        assert_eq!(s.name, "proposers");
        assert_eq!(s.agents.len(), 2);
        let seq0 = s.seq;
        super::stage("judge", vec!["j1".into()]);
        assert!(super::current().unwrap().seq > seq0);
        super::clear();
        assert!(super::current().is_none());
        let cleared = super::activity();
        assert!(cleared.sequence > seq0);
        assert!(cleared.stage.is_none());
        super::stage("verify", vec!["v1".into()]);
        assert!(super::current().unwrap().seq > cleared.sequence);
        super::clear();
    }

    #[test]
    fn seat_updates_merge_into_the_snapshot_and_bump_seq() {
        let _serial = serial();
        super::stage("proposer wave 2", vec!["a".into(), "b".into(), "c".into()]);
        let s0 = super::current().expect("published stage visible");
        assert!(s0.seat_states.is_empty(), "publish starts all-Running");
        assert_eq!(s0.seat_state(1), SeatState::Running);
        assert_eq!(s0.returned(), 0);
        assert!(
            super::current_seat_pips().is_none(),
            "strip pips wait for a published seat state"
        );

        super::stage_seat_update(1, SeatState::Returned);
        let s1 = super::current().unwrap();
        assert!(s1.seq > s0.seq, "a seat change bumps the seq");
        assert_eq!(s1.stage_id, s0.stage_id, "seat results preserve identity");
        assert_eq!(
            s1.seat_states,
            vec![SeatState::Running, SeatState::Returned]
        );
        assert_eq!(s1.returned(), 1);
        assert_eq!(
            super::current_seat_pips().as_deref(),
            Some("proposer wave 2 · 1/3 back · ")
        );

        // A repeat of the same state is a no-op — no seq churn.
        super::stage_seat_update(1, SeatState::Returned);
        assert_eq!(super::current().unwrap().seq, s1.seq);

        super::stage_seat_update(0, SeatState::Failed);
        super::stage_seat_update(2, SeatState::Cut);
        let s2 = super::current().unwrap();
        assert_eq!(
            s2.seat_states,
            vec![SeatState::Failed, SeatState::Returned, SeatState::Cut]
        );
        assert_eq!(s2.returned(), 1, "only Returned counts as back");

        // Out-of-roster updates are dropped without a bump.
        super::stage_seat_update(9, SeatState::Returned);
        assert_eq!(super::current().unwrap().seq, s2.seq);

        // No live stage: the update is a no-op, not a resurrection.
        super::clear();
        super::stage_seat_update(0, SeatState::Returned);
        assert!(super::current().is_none());

        // A fresh publish resets every seat to implicit Running.
        super::stage("judge", vec!["j1".into()]);
        assert!(super::current().unwrap().seat_states.is_empty());
        assert!(
            super::current_seat_pips().is_none(),
            "a one-seat stage has no strip pips"
        );
        super::clear();
    }
}
