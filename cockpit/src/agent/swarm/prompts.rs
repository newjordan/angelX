//! The persona roster and the stage prompts' routes, plus the swarm knobs.
//! Every word the stages hear is a page in Volume IV of the book.

use super::*;
use crate::agent::harness::book::{Route, d3_roles, d4_angles};

pub(crate) const PERSONAS: &[(&str, Route)] = &[
    ("first-principles", d4_angles::FIRST_PRINCIPLES),
    ("red-team", d4_angles::RED_TEAM),
    ("pragmatist", d4_angles::PRAGMATIST),
    ("lateral", d4_angles::LATERAL),
    ("systems", d4_angles::SYSTEMS),
    ("empiricist", d4_angles::EMPIRICIST),
];

pub(crate) const JUDGE_DIMS: &[(&str, f64)] = &[
    ("correctness", 0.40),
    ("insight", 0.25),
    ("rigor", 0.20),
    ("novelty", 0.15),
];

pub(crate) fn compaction_prep_instruction() -> Option<String> {
    let every = env_usize("ANGEL_MOA_COMPACT_EVERY_LOOPS", 3);
    if every == 0 {
        return None;
    }
    Some(format!("{} every={every}", d3_roles::CHECKPOINT.cells()))
}

pub(crate) fn answer_stage_with_compaction_prep(stage: &str) -> String {
    match compaction_prep_instruction() {
        Some(prep) if stage.trim().is_empty() => prep,
        Some(prep) => format!("{stage}\n\n{prep}"),
        None => stage.to_string(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mandate {
    Open,
    Tight,
}

#[derive(Clone)]
pub(crate) struct Knobs {
    pub(crate) width: usize,
    pub(crate) max_width: usize,
    pub(crate) max_waves: usize,
    pub(crate) wave_growth: f64,
    pub(crate) novelty_floor: f64,
    pub(crate) dissent_epsilon: f64,
    pub(crate) hard_factor: usize,
    pub(crate) refine_width: usize,
    pub(crate) layers: usize,
    pub(crate) always: bool,
    pub(crate) engaged: bool,
    pub(crate) mode: MoaMode,
    pub(crate) reflect: bool,
    pub(crate) judge: bool,
    pub(crate) judge_panel: usize,
    pub(crate) dims: bool,
    pub(crate) keep: usize,
    pub(crate) samples: usize,
    pub(crate) verify: usize,
    pub(crate) verify_guard: bool,
    pub(crate) research: bool,
    pub(crate) cite: bool,
    pub(crate) hedge: bool,
    pub(crate) search_url: String,
    pub(crate) delegate: bool,
    pub(crate) max_tests: usize,
    pub(crate) dissent_gate: bool,
    pub(crate) scale_threshold: usize,
    pub(crate) cluster: bool,
    pub(crate) cluster_sim: f64,
    pub(crate) rep_mode: String,
    pub(crate) pod_size: usize,
    pub(crate) pod_keep: usize,
    pub(crate) judge_fanout: usize,
    pub(crate) judge_weights: bool,
    pub(crate) agg_fanin: usize,
    pub(crate) dominant_relax: f64,
    pub(crate) eclusters_hi: f64,
    pub(crate) seat_efforts: SeatEfforts,
}

#[derive(Clone, Default)]
pub(crate) struct SeatEfforts {
    pub(crate) propose: Option<String>,
    pub(crate) judge: Option<String>,
    pub(crate) verify: Option<String>,
    pub(crate) aggregate: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MoaMode {
    Auto,
    Manual,
    Always,
}

impl MoaMode {
    pub(crate) fn from_env_var(var: &str, default: MoaMode) -> MoaMode {
        match std::env::var(var)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "manual" | "summon" | "off" => MoaMode::Manual,
            "always" | "full" | "max" => MoaMode::Always,
            "auto" | "heuristic" => MoaMode::Auto,
            _ => default,
        }
    }
}

impl Default for Knobs {
    fn default() -> Self {
        Self {
            seat_efforts: SeatEfforts::default(),
            width: 4,
            max_width: 4,
            max_waves: 1,
            wave_growth: 2.0,
            novelty_floor: 0.35,
            dissent_epsilon: 0.05,
            hard_factor: 2,
            refine_width: 6,
            layers: 2,
            always: false,
            engaged: false,
            mode: MoaMode::Auto,
            reflect: false,
            judge: false,
            judge_panel: 1,
            dims: false,
            keep: 0,
            samples: 1,
            verify: 0,
            verify_guard: false,
            research: false,
            cite: false,
            hedge: false,
            search_url: default_search_url(),
            delegate: false,
            max_tests: 3,
            dissent_gate: false,
            scale_threshold: 6,
            cluster: false,
            cluster_sim: 0.55,
            rep_mode: "detail".to_string(),
            pod_size: 6,
            pod_keep: 2,
            judge_fanout: 18,
            judge_weights: false,
            agg_fanin: usize::MAX,
            dominant_relax: 0.67,
            eclusters_hi: 4.0,
        }
    }
}
