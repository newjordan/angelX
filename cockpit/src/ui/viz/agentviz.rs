//! Agent-activity telemetry hook.
//!
//! Moa/swarm pipeline stages and `spawn` formations publish their current
//! fan-out here (stage name + one label per seat). The miniworld reads the
//! snapshot each tick and musters an army for it — soldiers for proposer
//! waves, magistrates for judge panels, sentries for verify rounds. Publishing
//! is a lock-and-swap (worker threads), reading is a cheap clone (UI tick).

use std::sync::Mutex;

pub use crate::telemetry::{SeatState, StageSnapshot};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActivitySnapshot {
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

pub fn current() -> Option<StageSnapshot> {
    activity().stage
}

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

pub fn activity_sequence() -> u64 {
    STAGE.lock().map(|g| g.sequence).unwrap_or_default()
}

pub fn activity() -> ActivitySnapshot {
    STAGE
        .lock()
        .map(|g| ActivitySnapshot {
            sequence: g.sequence,
            stage: g.current.clone(),
        })
        .unwrap_or_default()
}

pub fn clear() {
    if let Ok(mut g) = STAGE.lock()
        && g.current.take().is_some()
    {
        g.sequence = g.sequence.saturating_add(1);
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/agentviz__tests.rs"]
mod tests;
