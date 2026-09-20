//! Structural run telemetry for the reinforcement pipeline.
//!
//! Publish-only observability: the loop and the promotion gate report phases,
//! counters, decisions, and opaque identifiers here, and the cockpit's RL
//! stage panel reads the snapshot each frame (the `agentviz` pattern —
//! lock-and-swap from the worker, cheap clone from the UI tick).
//!
//! Deliberately structural: NO task text, prompts, or candidate output ever
//! enters a snapshot. Held-out cohorts stay private even if a rendered screen
//! is later fed back to a model. Publishing never influences the run — every
//! publisher is fire-and-forget and lock-failure-silent.

use std::collections::VecDeque;
use std::sync::Mutex;

/// Bounded event-timeline length: old transitions fall off the back.
const MAX_EVENTS: usize = 96;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RlPhase {
    #[default]
    Idle,
    Generation,
    Scoring,
    Reflection,
    Gate,
    FinalAudit,
    Release,
    Done,
    Failed,
}

impl RlPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Generation => "generate",
            Self::Scoring => "score",
            Self::Reflection => "reflect",
            Self::Gate => "gate",
            Self::FinalAudit => "audit",
            Self::Release => "release",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

/// One held-out case's fill state during a cohort evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseSlot {
    /// Opaque cohort case ID (already required to be uninformative).
    pub id: String,
    pub requested_per_policy: usize,
    pub incumbent_observed: usize,
    pub candidate_observed: usize,
    pub delta: Option<f32>,
    pub complete: bool,
    pub failed: bool,
}

/// Batch-health counters (mirrors `BatchMetrics`, minus latencies).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BatchCounters {
    pub generated: usize,
    pub accepted: usize,
    pub rejected_stale: usize,
    pub rejected_straggler: usize,
    pub solve_rate: f32,
    pub advantage_variance: f32,
}

/// The final verdict of one cohort evaluation (promotion or final audit).
#[derive(Clone, Debug, PartialEq)]
pub struct CohortVerdict {
    pub role: String,
    pub decision: String,
    pub promoted: bool,
    pub mean_delta: Option<f32>,
    pub delta_lower_bound: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RlEvent {
    pub seq: u64,
    pub label: String,
}

#[derive(Clone, Debug, Default)]
pub struct RlSnapshot {
    /// Monotonic per publish — readers re-render when this moves.
    pub seq: u64,
    /// Which pipeline is running ("technical", "nontechnical", "ablation", …).
    pub source: String,
    pub phase: RlPhase,
    /// 1-based round in progress.
    pub round: usize,
    pub rounds: usize,
    pub policy_version: u64,
    pub batch: Option<BatchCounters>,
    pub best_reward: Option<f32>,
    /// Role of the cohort currently being evaluated, when one is.
    pub cohort_role: Option<String>,
    pub cohort_cases_total: usize,
    pub cases: Vec<CaseSlot>,
    /// Latest promotion-gate verdict this run.
    pub gate: Option<CohortVerdict>,
    /// Latest final-audit verdict this run.
    pub audit: Option<CohortVerdict>,
    pub released: bool,
    pub events: VecDeque<RlEvent>,
}

static SNAPSHOT: Mutex<Option<RlSnapshot>> = Mutex::new(None);

fn publish(mutate: impl FnOnce(&mut RlSnapshot)) {
    let Ok(mut guard) = SNAPSHOT.lock() else {
        return;
    };
    let snapshot = guard.get_or_insert_with(RlSnapshot::default);
    snapshot.seq += 1;
    mutate(snapshot);
}

fn note(snapshot: &mut RlSnapshot, label: String) {
    let seq = snapshot.seq;
    snapshot.events.push_back(RlEvent { seq, label });
    while snapshot.events.len() > MAX_EVENTS {
        snapshot.events.pop_front();
    }
}

/// Start a fresh run snapshot, replacing any prior run's state.
pub fn begin(source: &str, rounds: usize) {
    let Ok(mut guard) = SNAPSHOT.lock() else {
        return;
    };
    let seq = guard.as_ref().map_or(1, |s| s.seq + 1);
    let mut snapshot = RlSnapshot {
        seq,
        source: source.to_string(),
        rounds,
        ..RlSnapshot::default()
    };
    note(&mut snapshot, format!("run · {source} · {rounds} round(s)"));
    *guard = Some(snapshot);
}

pub fn phase(phase: RlPhase) {
    publish(|snapshot| {
        if snapshot.phase != phase {
            snapshot.phase = phase;
            note(snapshot, format!("phase · {}", phase.label()));
        }
    });
}

pub fn round(round: usize, policy_version: u64) {
    publish(|snapshot| {
        snapshot.round = round;
        snapshot.policy_version = policy_version;
        snapshot.phase = RlPhase::Generation;
        note(
            snapshot,
            format!("round {round} · policy v{policy_version}"),
        );
    });
}

pub fn batch(counters: BatchCounters, best_reward: Option<f32>) {
    publish(|snapshot| {
        snapshot.phase = RlPhase::Scoring;
        let label = format!(
            "batch · {}/{} accepted · solve {:.0}% · advar {:.4}",
            counters.accepted,
            counters.generated,
            counters.solve_rate * 100.0,
            counters.advantage_variance
        );
        snapshot.batch = Some(counters);
        snapshot.best_reward = best_reward.filter(|r| r.is_finite());
        note(snapshot, label);
    });
}

pub fn cohort_begin(role: &str, cases_total: usize, samples_per_case: usize) {
    publish(|snapshot| {
        snapshot.phase = if role == "final-audit" {
            RlPhase::FinalAudit
        } else {
            RlPhase::Gate
        };
        snapshot.cohort_role = Some(role.to_string());
        snapshot.cohort_cases_total = cases_total;
        snapshot.cases.clear();
        note(
            snapshot,
            format!("cohort · {role} · {cases_total} cases × {samples_per_case}/policy"),
        );
    });
}

pub fn case_slot(slot: CaseSlot) {
    publish(|snapshot| {
        let label = format!(
            "case {} · inc {}/{} cand {}/{}{}",
            slot.id,
            slot.incumbent_observed,
            slot.requested_per_policy,
            slot.candidate_observed,
            slot.requested_per_policy,
            slot.delta
                .map(|d| format!(" · Δ{d:+.3}"))
                .unwrap_or_default(),
        );
        if let Some(existing) = snapshot.cases.iter_mut().find(|c| c.id == slot.id) {
            *existing = slot;
        } else {
            snapshot.cases.push(slot);
        }
        note(snapshot, label);
    });
}

pub fn verdict(verdict: CohortVerdict) {
    publish(|snapshot| {
        note(snapshot, format!("{} · {}", verdict.role, verdict.decision));
        if verdict.role == "final-audit" {
            snapshot.audit = Some(verdict);
        } else {
            snapshot.gate = Some(verdict);
        }
    });
}

pub fn released() {
    publish(|snapshot| {
        snapshot.phase = RlPhase::Release;
        snapshot.released = true;
        note(snapshot, "release · candidate minted".to_string());
    });
}

pub fn done() {
    publish(|snapshot| {
        if !matches!(snapshot.phase, RlPhase::Failed) {
            snapshot.phase = RlPhase::Done;
            note(snapshot, "run · complete".to_string());
        }
    });
}

pub fn failed() {
    publish(|snapshot| {
        snapshot.phase = RlPhase::Failed;
        note(snapshot, "run · failed".to_string());
    });
}

/// The latest snapshot, if any run has published since startup.
pub fn current() -> Option<RlSnapshot> {
    SNAPSHOT.lock().ok().and_then(|guard| guard.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The snapshot is a process-global; other reinforce tests may publish
    /// concurrently. Retry the whole sequence until our source survives the
    /// read-back so the assertion is about our own writes.
    fn with_own_run(assertions: impl Fn(RlSnapshot)) {
        for _ in 0..16 {
            begin("telemetry-test", 2);
            round(1, 0);
            batch(
                BatchCounters {
                    generated: 12,
                    accepted: 10,
                    solve_rate: 0.5,
                    advantage_variance: 0.02,
                    ..BatchCounters::default()
                },
                Some(1.0),
            );
            cohort_begin("promotion", 2, 4);
            case_slot(CaseSlot {
                id: "case-a".into(),
                requested_per_policy: 4,
                incumbent_observed: 4,
                candidate_observed: 4,
                delta: Some(0.2),
                complete: true,
                failed: false,
            });
            verdict(CohortVerdict {
                role: "promotion".into(),
                decision: "promoted".into(),
                promoted: true,
                mean_delta: Some(0.2),
                delta_lower_bound: Some(0.1),
            });
            let Some(snapshot) = current() else { continue };
            if snapshot.source == "telemetry-test" && snapshot.gate.is_some() {
                assertions(snapshot);
                return;
            }
        }
        panic!("telemetry snapshot never survived concurrent publishers");
    }

    #[test]
    fn run_sequence_accumulates_structural_state() {
        with_own_run(|snapshot| {
            assert_eq!(snapshot.rounds, 2);
            assert_eq!(snapshot.round, 1);
            assert_eq!(snapshot.batch.map(|b| b.accepted), Some(10));
            assert_eq!(snapshot.cases.len(), 1);
            assert_eq!(snapshot.cases[0].delta, Some(0.2));
            assert!(snapshot.gate.as_ref().unwrap().promoted);
            assert!(
                snapshot
                    .events
                    .iter()
                    .any(|event| event.label.contains("promotion · promoted"))
            );
            // Structural-only: no event may carry task or prompt text — the
            // only free-form field is the opaque case id.
            assert!(
                snapshot
                    .events
                    .iter()
                    .all(|event| !event.label.contains("reverse:"))
            );
        });
    }

    #[test]
    fn case_slot_updates_in_place() {
        begin("telemetry-upsert", 1);
        cohort_begin("promotion", 1, 4);
        for observed in [2usize, 4] {
            case_slot(CaseSlot {
                id: "case-x".into(),
                requested_per_policy: 4,
                incumbent_observed: observed,
                candidate_observed: observed,
                delta: None,
                complete: false,
                failed: false,
            });
        }
        if let Some(snapshot) = current()
            && snapshot.source == "telemetry-upsert"
        {
            // Concurrent suites may append their own cases to the global
            // snapshot; the upsert contract is only about OUR case id.
            let mine: Vec<_> = snapshot
                .cases
                .iter()
                .filter(|case| case.id == "case-x")
                .collect();
            assert_eq!(
                mine.len(),
                1,
                "case-x upserts in place: {:?}",
                snapshot.cases
            );
            assert_eq!(mine[0].incumbent_observed, 4);
        }
    }

    #[test]
    fn event_timeline_stays_bounded() {
        begin("telemetry-bound", 1);
        for _ in 0..(MAX_EVENTS * 2) {
            phase(RlPhase::Generation);
            phase(RlPhase::Scoring);
        }
        if let Some(snapshot) = current() {
            assert!(snapshot.events.len() <= MAX_EVENTS);
        }
    }
}
