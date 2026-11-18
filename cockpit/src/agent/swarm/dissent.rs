//! The dissent gate: proposer disagreement as a free uncertainty signal.
//!
//! Independent proposers that converge on the same answer are the pipeline's
//! own evidence that the answer is easy — judging, refining, and verifying it
//! is spend without payoff. Proposers that *diverge* mark exactly the turns
//! where the judge panel and an adversarial verify round earn their cost. The
//! gauge is pure local text similarity (zero model calls), and the gate turns
//! it into per-turn knob overrides: relax the pipeline when the drafts agree,
//! escalate it when they don't.
//!
//! The metric is deliberately surface-level — content-word vocabulary overlap
//! adjusted for polarity, not semantics — so the thresholds are heuristics, not
//! truths. The MoA ledger (`swarm/ledger.rs`) records every turn's dissent score
//! precisely so the thresholds can be calibrated from observed data (`/moa`).
//!
//! One correction is baked in because pure overlap got it exactly backwards:
//! contradictory drafts share their vocabulary. "We should ship the change" and
//! "We should not ship the change" have identical content words, scored as
//! total agreement, and made the gate *relax* on the turns where its proposers
//! most disagreed. Polarity divergence now suppresses similarity — see
//! [`draft_similarity`].

use super::*;

fn content_words(text: &str) -> std::collections::HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4)
        .map(str::to_lowercase)
        .collect()
}

const NEGATION_MARKERS: [&str; 12] = [
    "not",
    "no",
    "nor",
    "never",
    "cannot",
    "none",
    "neither",
    "without",
    "fails",
    "unable",
    "incorrect",
    "false",
];

const POLARITY_SCALE: f64 = 3.0;

fn negation_count(text: &str) -> usize {
    let lower = text.to_lowercase();
    let contractions = lower.matches("n't").count();
    let words = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| NEGATION_MARKERS.contains(w))
        .count();
    contractions + words
}

struct DraftSignature<'a> {
    words: std::collections::HashSet<String>,
    negation_rate: f64,
    text: &'a str,
}

impl<'a> DraftSignature<'a> {
    fn new(text: &'a str) -> Self {
        let words = content_words(text);
        let negation_rate = negation_count(text) as f64 / words.len().max(1) as f64;
        Self {
            words,
            negation_rate,
            text,
        }
    }

    fn similarity(&self, other: &DraftSignature<'_>) -> f64 {
        if self.words.is_empty() && other.words.is_empty() {
            return if self
                .text
                .split_whitespace()
                .eq(other.text.split_whitespace())
            {
                1.0
            } else {
                0.0
            };
        }
        let inter = self.words.intersection(&other.words).count() as f64;
        let union = (self.words.len() + other.words.len()) as f64 - inter;
        let jaccard = if union > 0.0 { inter / union } else { 0.0 };
        let polarity_gap = (self.negation_rate - other.negation_rate).abs();
        jaccard * (1.0 - (polarity_gap * POLARITY_SCALE).min(1.0))
    }
}

pub(crate) fn draft_similarity(a: &str, b: &str) -> f64 {
    DraftSignature::new(a).similarity(&DraftSignature::new(b))
}

pub(crate) fn draft_dissent(drafts: &[String]) -> Option<f64> {
    if drafts.len() < 2 {
        return None;
    }
    let sigs: Vec<DraftSignature<'_>> = drafts.iter().map(|d| DraftSignature::new(d)).collect();
    let mut total = 0.0;
    let mut pairs = 0usize;
    for i in 0..sigs.len() {
        for j in (i + 1)..sigs.len() {
            total += 1.0 - sigs[i].similarity(&sigs[j]);
            pairs += 1;
        }
    }
    Some(total / pairs as f64)
}

pub(crate) fn count_novel_against(pool: &[String], wave: &[String], threshold: f64) -> usize {
    let mut seen: Vec<&String> = pool.iter().collect();
    let mut novel = 0usize;
    for draft in wave {
        if !seen
            .iter()
            .any(|prior| near_identical_at(prior, draft, threshold))
        {
            novel += 1;
            seen.push(draft);
        }
    }
    novel
}

pub(crate) fn contention_summary(drafts: &[String]) -> String {
    if drafts.len() < 2 {
        return String::new();
    }
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for words in drafts.iter().map(|d| content_words(d)) {
        for word in words {
            *counts.entry(word).or_default() += 1;
        }
    }
    let n = drafts.len() as f64;
    let mut ranked: Vec<(String, usize, f64)> = counts
        .into_iter()
        .filter_map(|(word, count)| {
            let df = count as f64 / n;
            (0.25..=0.75)
                .contains(&df)
                .then_some((word, count, df * (1.0 - df)))
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    if ranked.is_empty() {
        return crate::agent::harness::book::d3_roles::pages(
            crate::agent::harness::book::d4_angles::WAVE,
            [4],
        );
    }
    ranked
        .into_iter()
        .take(12)
        .map(|(word, count, _)| format!("- {word} ({count}/{})", drafts.len()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn dissent_hi() -> f64 {
    env_f64("ANGEL_MOA_DISSENT_HI", 0.7)
}

pub(crate) fn dissent_lo() -> f64 {
    env_f64("ANGEL_MOA_DISSENT_LO", 0.45)
}

pub(crate) fn dissent_verify_rounds() -> usize {
    env_usize("ANGEL_MOA_DISSENT_VERIFY", 1)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GateAction {
    Escalate,
    Hold,
    Relax,
}

impl GateAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            GateAction::Escalate => "escalate",
            GateAction::Hold => "hold",
            GateAction::Relax => "relax",
        }
    }
}

pub(crate) fn gate_knobs(k: &Knobs, dissent: Option<f64>) -> (Knobs, Option<GateAction>) {
    gate_knobs_at(
        k,
        dissent,
        dissent_hi(),
        dissent_lo(),
        dissent_verify_rounds(),
    )
}

pub(crate) fn gate_knobs_at(
    k: &Knobs,
    dissent: Option<f64>,
    hi: f64,
    lo: f64,
    rounds: usize,
) -> (Knobs, Option<GateAction>) {
    let mut gated = k.clone();
    let Some(d) = dissent else {
        return (gated, None);
    };
    let action = if d >= hi {
        gated.judge = true;
        gated.verify = gated.verify.max(rounds);
        GateAction::Escalate
    } else if d <= lo {
        gated.layers = 1;
        gated.reflect = false;
        gated.judge = false;
        gated.verify = 0;
        gated.samples = 1;
        GateAction::Relax
    } else {
        GateAction::Hold
    };
    (gated, Some(action))
}

pub(crate) fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(default)
}
