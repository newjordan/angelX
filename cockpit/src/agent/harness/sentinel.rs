//! The turn sentinel: deterministic, model-free overwatch for a turn that
//! carries a wall-clock budget (`ANGEL_TURN_DEADLINE_SECS` / `--deadline-secs`).
//!
//! It exists for one measured failure class: a budgeted run (Terminal-Bench 2,
//! 900 s adapter cap) burning the whole budget without landing any output
//! artifact, so the external kill leaves nothing to grade. The sentinel
//! watches each hop's elapsed fraction, mutation progress, green state, and
//! operator-named artifacts, and renders latched verdicts:
//!
//! - `Drift`: half the budget is gone before the first edit.
//! - `Deliver`: 85% is gone with no edit landed, or named artifacts missing.
//!
//! Quiet by design (operator rule: noisy interventions are unwelcome).
//! `Observe` renders verdicts as operator notices only; `Intervene` may
//! additionally raise the existing first-write advisory once. A green
//! verification silences the sentinel outright — the green path owns its
//! own nudges. Judgment calls about *thinking quality* stay with the local
//! overwatch lanes (Treebeard); this core only reports facts.

use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SentinelMode {
    Off,
    Observe,
    Intervene,
}

pub(crate) fn parse_mode(raw: &str) -> SentinelMode {
    match raw.trim().to_ascii_lowercase().as_str() {
        "off" | "0" | "none" => SentinelMode::Off,
        "intervene" | "on" | "1" => SentinelMode::Intervene,
        _ => SentinelMode::Observe,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    Quiet,
    Drift { budget_spent_bp: u64 },
    Deliver {
        budget_spent_bp: u64,
        artifacts_missing: Option<usize>,
    },
}

impl Verdict {
    pub(crate) fn describe(&self) -> String {
        match self {
            Verdict::Quiet => "sentinel: quiet".to_string(),
            Verdict::Drift { budget_spent_bp } => format!(
                "sentinel: {}% of the turn budget spent before the first edit",
                budget_spent_bp / 100
            ),
            Verdict::Deliver {
                budget_spent_bp,
                artifacts_missing,
            } => match artifacts_missing {
                Some(n) => format!(
                    "sentinel: {}% of the turn budget spent, {n} named artifact(s) still missing",
                    budget_spent_bp / 100
                ),
                None => format!(
                    "sentinel: {}% of the turn budget spent, nothing landed yet",
                    budget_spent_bp / 100
                ),
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Observation {
    pub elapsed: Duration,
    pub mutated_this_hop: bool,
    pub green: bool,
    pub artifacts_missing: Option<usize>,
}

const DRIFT_BP: u64 = 5_000;
const DELIVER_BP: u64 = 8_500;

pub(crate) struct Sentinel {
    budget: Option<Duration>,
    mode: SentinelMode,
    any_mutation: bool,
    drift_fired: bool,
    deliver_fired: bool,
}

impl Sentinel {
    pub(crate) fn new(budget: Option<Duration>, mode: SentinelMode) -> Self {
        Self {
            budget: budget.filter(|b| !b.is_zero()),
            mode,
            any_mutation: false,
            drift_fired: false,
            deliver_fired: false,
        }
    }

    pub(crate) fn from_env(budget_secs: usize) -> Self {
        let budget = (budget_secs > 0).then(|| Duration::from_secs(budget_secs as u64));
        let mode = match std::env::var("ANGEL_SENTINEL") {
            Ok(raw) => parse_mode(&raw),
            Err(_) if budget.is_some() => SentinelMode::Observe,
            Err(_) => SentinelMode::Off,
        };
        Self::new(budget, mode)
    }

    pub(crate) fn intervene_armed(&self) -> bool {
        self.mode == SentinelMode::Intervene && self.budget.is_some()
    }

    pub(crate) fn observe(&mut self, obs: Observation) -> Verdict {
        let Some(budget) = self.budget else {
            return Verdict::Quiet;
        };
        if self.mode == SentinelMode::Off {
            return Verdict::Quiet;
        }
        self.any_mutation |= obs.mutated_this_hop;
        if obs.green {
            return Verdict::Quiet;
        }
        let spent_bp = (obs.elapsed.as_millis() as u64)
            .saturating_mul(10_000)
            .checked_div(budget.as_millis() as u64)
            .unwrap_or(10_000);
        let artifacts_open = obs.artifacts_missing.is_some_and(|n| n > 0);
        if spent_bp >= DELIVER_BP
            && !self.deliver_fired
            && (!self.any_mutation || artifacts_open)
        {
            self.deliver_fired = true;
            self.drift_fired = true;
            return Verdict::Deliver {
                budget_spent_bp: spent_bp,
                artifacts_missing: obs.artifacts_missing,
            };
        }
        if spent_bp >= DRIFT_BP && !self.drift_fired && (!self.any_mutation || artifacts_open) {
            self.drift_fired = true;
            return Verdict::Drift {
                budget_spent_bp: spent_bp,
            };
        }
        Verdict::Quiet
    }
}

pub(crate) fn configured_artifacts() -> Vec<PathBuf> {
    let Ok(raw) = std::env::var("ANGEL_SENTINEL_ARTIFACTS") else {
        return Vec::new();
    };
    raw.split([',', ':'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

pub(crate) fn missing_artifact_paths(named: &[PathBuf], workspace: &Path) -> Vec<PathBuf> {
    named
        .iter()
        .map(|p| {
            if p.is_absolute() {
                p.as_path().to_path_buf()
            } else {
                workspace.join(p.as_path())
            }
        })
        .filter(|full| !full.exists())
        .collect()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn missing_artifacts(named: &[PathBuf], workspace: &Path) -> Option<usize> {
    if named.is_empty() {
        return None;
    }
    Some(missing_artifact_paths(named, workspace).len())
}

pub(crate) fn configured_liveness_cmd() -> Option<String> {
    std::env::var("ANGEL_SENTINEL_LIVENESS_CMD")
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|cmd| !cmd.is_empty())
}

pub(crate) const GATE_MAX_BLOCKS: u8 = 2;

pub(crate) fn gate_should_block(
    intervene_armed: bool,
    artifacts_missing: usize,
    exact_dir_violations: usize,
    liveness_failed: bool,
    owned_service_pending: bool,
    blocks_used: u8,
    budget_left: bool,
) -> bool {
    intervene_armed
        && budget_left
        && blocks_used < GATE_MAX_BLOCKS
        && (artifacts_missing > 0
            || exact_dir_violations > 0
            || liveness_failed
            || owned_service_pending)
}

pub(crate) struct ExactDir {
    pub dir: PathBuf,
    pub expected: std::collections::BTreeSet<String>,
}

pub(crate) fn parse_exact_dirs(raw: &str) -> Vec<ExactDir> {
    raw.split(';')
        .filter_map(|spec| {
            let (dir, names) = spec.trim().split_once('=')?;
            let expected: std::collections::BTreeSet<String> = names
                .split('+')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .collect();
            (!dir.trim().is_empty() && !expected.is_empty()).then(|| ExactDir {
                dir: PathBuf::from(dir.trim()),
                expected,
            })
        })
        .collect()
}

pub(crate) fn configured_exact_dirs() -> Vec<ExactDir> {
    std::env::var("ANGEL_SENTINEL_EXACT_DIRS")
        .map(|raw| parse_exact_dirs(&raw))
        .unwrap_or_default()
}

pub(crate) fn exact_dir_violations(specs: &[ExactDir], workspace: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for spec in specs {
        let dir = if spec.dir.is_absolute() {
            spec.dir.clone()
        } else {
            workspace.join(&spec.dir)
        };
        let actual: std::collections::BTreeSet<String> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        if actual != spec.expected {
            let extra: Vec<_> = actual.difference(&spec.expected).cloned().collect();
            let missing: Vec<_> = spec.expected.difference(&actual).cloned().collect();
            violations.push(format!(
                "{} must contain exactly {{{}}}; extra: [{}], missing: [{}] \
                 (move build outputs elsewhere, e.g. /tmp)",
                dir.display(),
                spec.expected
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                extra.join(", "),
                missing.join(", ")
            ));
        }
    }
    violations
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/sentinel__tests.rs"]
mod tests;
