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

/// Content words of a draft: lowercased alphanumeric runs of ≥4 chars. Short
/// tokens (articles, "the"/"is", list numbers) carry no signal about *what* a
/// draft concluded; the longer vocabulary does, and it survives rephrasing far
/// better than the word-3-gram shingles the dedup pass uses. Length is taken
/// on the original token so Unicode case-folding cannot change the ≥4 floor.
fn content_words(text: &str) -> std::collections::HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4)
        .map(str::to_lowercase)
        .collect()
}

/// Negation markers the content-word filter cannot see. `not`/`no`/`nor` are
/// under the 4-char floor, and every `n't` contraction is mangled by the
/// alphanumeric split (`isn't` → `isn` + `t`, both dropped). The longer markers
/// are included too: Jaccard does see them, but only as ordinary vocabulary —
/// it has no notion that they *invert* a claim.
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

/// How strongly a polarity gap suppresses measured similarity. A single flipped
/// conclusion in a short draft should read as total disagreement, so the gap is
/// amplified rather than used raw.
const POLARITY_SCALE: f64 = 3.0;

/// Count negation markers in `text`: whole-word matches from
/// [`NEGATION_MARKERS`] plus every `n't` contraction. Walks the fully
/// lowercased string (not the original-token split) so contractions and
/// case-folded markers stay visible.
fn negation_count(text: &str) -> usize {
    let lower = text.to_lowercase();
    let contractions = lower.matches("n't").count();
    let words = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| NEGATION_MARKERS.contains(w))
        .count();
    contractions + words
}

/// Per-draft vocabulary + polarity, built once and reused across a comparison
/// pool. Polarity density is negation count over content-word cardinality
/// (minimum 1), matching the former `negation_rate` path without retokenizing.
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

/// Vocabulary overlap between two drafts: Jaccard over content words, in
/// [0, 1], suppressed when the two carry materially different polarity.
///
/// Jaccard alone is blind to contradiction. "We should ship the change" and "We
/// should not ship the change" have *identical* content-word sets — `not` is
/// under the 4-char floor — so they scored 1.0, i.e. perfect agreement, and the
/// gate then relaxed the pipeline exactly when its proposers had reached
/// opposite conclusions. Two drafts built from the same vocabulary but carrying
/// different amounts of negation are asserting different things about the same
/// subject, so the polarity gap suppresses the score.
///
/// The adjustment only ever *lowers* similarity — it can raise measured dissent
/// and escalate verification, never lower it and skip verification that the
/// unadjusted metric would have triggered. A miscalibration here costs spend,
/// not correctness.
pub(crate) fn draft_similarity(a: &str, b: &str) -> f64 {
    DraftSignature::new(a).similarity(&DraftSignature::new(b))
}

/// Mean pairwise dissent (1 − similarity) across the draft set, or `None` when
/// there is nothing to disagree with (fewer than two drafts). The *mean* — not
/// the max — so one deliberately contrarian persona (red-team disagrees by
/// design) can't single-handedly escalate every turn.
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
        return "No clear lexical disagreement frontier yet.".to_string();
    }
    ranked
        .into_iter()
        .take(12)
        .map(|(word, count, _)| format!("- {word} ({count}/{})", drafts.len()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Dissent at or above this → escalate. `ANGEL_MOA_DISSENT_HI` overrides.
pub(crate) fn dissent_hi() -> f64 {
    env_f64("ANGEL_MOA_DISSENT_HI", 0.7)
}

/// Dissent at or below this → relax. `ANGEL_MOA_DISSENT_LO` overrides.
pub(crate) fn dissent_lo() -> f64 {
    env_f64("ANGEL_MOA_DISSENT_LO", 0.45)
}

/// Verify rounds an escalation buys (`ANGEL_MOA_DISSENT_VERIFY`, default 1).
pub(crate) fn dissent_verify_rounds() -> usize {
    env_usize("ANGEL_MOA_DISSENT_VERIFY", 1)
}

/// What the gate decided for one turn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GateAction {
    /// Drafts diverged: buy scrutiny — judge on, verify round(s) on.
    Escalate,
    /// No strong signal either way: run the configured pipeline unchanged.
    Hold,
    /// Drafts agree: skip refine layers, judge, verify, and extra samples.
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

/// Per-turn knob overrides from the dissent score, with env thresholds.
/// `None` dissent (gate off, or a single draft) leaves the knobs untouched.
pub(crate) fn gate_knobs(k: &Knobs, dissent: Option<f64>) -> (Knobs, Option<GateAction>) {
    gate_knobs_at(
        k,
        dissent,
        dissent_hi(),
        dissent_lo(),
        dissent_verify_rounds(),
    )
}

/// [`gate_knobs`] with the thresholds explicit (testable without env).
///
/// Escalate wins when a misconfigured `lo > hi` makes both bounds true —
/// over-scrutiny is the safer failure. Escalation sets `judge` and grants
/// `rounds` verify passes (never taking away configured rounds); at width 2
/// the judge is a free no-op (auto-keep ≥ draft count skips the reviewer
/// calls), so the verify round is what escalation actually buys there.
/// Relaxation drops the optional stages — refine layers, reflect, judge,
/// verify, extra samples — and leaves the synthesis, so agreeing turns still
/// end in one polished answer (and stream live, since nothing post-processes).
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
