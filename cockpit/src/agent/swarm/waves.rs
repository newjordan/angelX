//! Pure progressive-wave decision logic.

use crate::agent::harness::live_policy;
use super::GateAction;

const LEG: &str = "swarm-waves";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StopReason {
    MaxWidth,
    MaxWaves,
    Failed,
    LowNovelty,
    Agree,
    Stable,
}

impl StopReason {
    pub(crate) fn label(self) -> &'static str {
        match self {
            StopReason::MaxWidth => "max-width",
            StopReason::MaxWaves => "max-waves",
            StopReason::Failed => "failed",
            StopReason::LowNovelty => "low-novelty",
            StopReason::Agree => "agree",
            StopReason::Stable => "stable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Decision {
    Stop(StopReason),
    Expand {
        next_wave: usize,
    },
}

impl Decision {
    /// The leg contract: a stop names a reason, and an expand asks for at
    /// least one more proposer. Anything else keeps the native decision.
    pub(crate) fn check(&self) -> Result<(), String> {
        match self {
            Decision::Stop(_) => Ok(()),
            Decision::Expand { next_wave } if *next_wave >= 1 => Ok(()),
            Decision::Expand { next_wave } => {
                Err(format!("an expand needs a positive next wave, got {next_wave}"))
            }
        }
    }

    pub(crate) fn label(self) -> String {
        match self {
            Decision::Stop(reason) => format!("stop:{}", reason.label()),
            Decision::Expand { next_wave } => format!("expand:{next_wave}"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WaveInputs {
    pub(crate) wave_index: usize,
    pub(crate) max_waves: usize,
    pub(crate) launched_total: usize,
    pub(crate) max_width: usize,
    pub(crate) returned: usize,
    pub(crate) novel: usize,
    pub(crate) wave_size: usize,
    pub(crate) dissent: Option<f64>,
    pub(crate) prev_dissent: Option<f64>,
    pub(crate) novelty_floor: f64,
    pub(crate) dissent_lo: f64,
    pub(crate) dissent_epsilon: f64,
    pub(crate) growth: f64,
}

pub(crate) fn decide_wave(i: WaveInputs) -> Decision {
    decide_wave_with_source(i).0
}

/// Every float comparison and the grown size are computed in Rust (Bend has no
/// f64); the leg owns the stop order, the expand choice and the clamps.
pub(crate) fn decide_wave_with_source(i: WaveInputs) -> (Decision, live_policy::LegSource) {
    live_policy::decide_shipped(LEG, &decide_wave_input(i), Decision::check)
}

/// The leg's input record: the float comparisons resolved to booleans and the
/// grown size resolved to an integer, because Bend has no f64.
pub(crate) fn decide_wave_input(i: WaveInputs) -> serde_json::Value {
    serde_json::json!({
        "kind": "wave",
        "wave_index": i.wave_index,
        "max_waves": i.max_waves,
        "launched_total": i.launched_total,
        "max_width": i.max_width,
        "returned": i.returned,
        "novel_low": matches!(i.novel, 0) || (i.novel as f64 / i.returned.max(1) as f64) < i.novelty_floor,
        "dissent_agrees": matches!(i.dissent, Some(d) if d <= i.dissent_lo),
        "dissent_stable": matches!((i.prev_dissent, i.dissent), (Some(prev), Some(now)) if (now - prev).abs() <= i.dissent_epsilon),
        "room": i.max_width.saturating_sub(i.launched_total),
        "next_raw": next_wave_size(i.wave_size, i.growth, i.max_width.saturating_sub(i.launched_total)),
    })
}

pub(crate) fn next_wave_size(current: usize, growth: f64, remaining: usize) -> usize {
    if remaining == 0 {
        return 0;
    }
    let growth = if growth.is_finite() && growth > 1.0 {
        growth
    } else {
        1.0
    };
    let next = ((current.max(1) as f64) * growth).ceil() as usize;
    next.max(1).min(remaining)
}

/// The scale gate's emitted record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct GateOut {
    pub(crate) gate: GateAction,
}

/// The decision part of `scale::scale_signal_at`: the dominant share and the
/// entropy are floored to the relaxed/escalated flags natively (Bend has no
/// f64), and the leg orders relax before escalate before hold.
pub(crate) fn gate_at(relax: bool, escalate: bool) -> (GateAction, live_policy::LegSource) {
    let input = gate_input(relax, escalate);
    // A dominant pool must relax; anything else is out of contract.
    let check = move |out: &GateOut| match (relax, out.gate) {
        (true, GateAction::Relax) | (false, _) => Ok(()),
        (true, _) => Err(format!(
            "a dominant pool must relax, got {}",
            out.gate.label()
        )),
    };
    let (out, source) = live_policy::decide_shipped(LEG, &input, check);
    (out.gate, source)
}

pub(crate) fn gate_input(relax: bool, escalate: bool) -> serde_json::Value {
    serde_json::json!({"kind": "gate", "relax": relax, "escalate": escalate})
}

