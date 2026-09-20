//! Scale coordination for large MoA draft sets.

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DraftCluster {
    pub(crate) indices: Vec<usize>,
    pub(crate) medoid: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RepMode {
    Detail,
    Medoid,
}

impl RepMode {
    pub(crate) fn from_env_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "medoid" => Self::Medoid,
            _ => Self::Detail,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScaleSignal {
    pub(crate) clusters: usize,
    pub(crate) dominant: f64,
    pub(crate) eclusters: f64,
    pub(crate) gate: GateAction,
}

pub(crate) fn cluster_drafts(drafts: &[String], threshold: f64) -> Vec<DraftCluster> {
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for idx in 0..drafts.len() {
        let mut placed = false;
        for cluster in &mut clusters {
            if cluster
                .iter()
                .any(|&j| draft_similarity(&drafts[idx], &drafts[j]) >= threshold)
            {
                cluster.push(idx);
                placed = true;
                break;
            }
        }
        if !placed {
            clusters.push(vec![idx]);
        }
    }
    clusters
        .into_iter()
        .map(|indices| {
            let medoid = medoid_index(drafts, &indices);
            DraftCluster { indices, medoid }
        })
        .collect()
}

fn medoid_index(drafts: &[String], indices: &[usize]) -> usize {
    indices
        .iter()
        .copied()
        .max_by(|&a, &b| {
            avg_similarity(drafts, a, indices)
                .partial_cmp(&avg_similarity(drafts, b, indices))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0)
}

fn avg_similarity(drafts: &[String], idx: usize, indices: &[usize]) -> f64 {
    if indices.len() <= 1 {
        return 1.0;
    }
    let total: f64 = indices
        .iter()
        .copied()
        .filter(|&j| j != idx)
        .map(|j| draft_similarity(&drafts[idx], &drafts[j]))
        .sum();
    total / (indices.len() - 1) as f64
}

pub(crate) fn pick_representative(
    drafts: &[String],
    cluster: &DraftCluster,
    mode: RepMode,
) -> usize {
    match mode {
        RepMode::Medoid => cluster.medoid,
        RepMode::Detail => cluster
            .indices
            .iter()
            .copied()
            .max_by_key(|&idx| drafts[idx].chars().count())
            .unwrap_or(cluster.medoid),
    }
}

pub(crate) fn cluster_representatives(
    drafts: &[String],
    clusters: &[DraftCluster],
    mode: RepMode,
) -> Vec<(String, usize)> {
    clusters
        .iter()
        .map(|cluster| {
            let idx = pick_representative(drafts, cluster, mode);
            (drafts[idx].clone(), cluster.indices.len())
        })
        .collect()
}

pub(crate) fn seed_pods(n: usize, pod_size: usize) -> Vec<Vec<usize>> {
    let pod_size = pod_size.max(1);
    (0..n)
        .collect::<Vec<_>>()
        .chunks(pod_size)
        .map(|c| c.to_vec())
        .collect()
}

pub(crate) fn scale_signal_at(
    clusters: &[DraftCluster],
    total: usize,
    dominant_relax: f64,
    eclusters_hi: f64,
) -> Option<ScaleSignal> {
    if total == 0 || clusters.is_empty() {
        return None;
    }
    let dominant =
        clusters.iter().map(|c| c.indices.len()).max().unwrap_or(0) as f64 / total as f64;
    let entropy = clusters
        .iter()
        .map(|c| c.indices.len() as f64 / total as f64)
        .filter(|&p| p > 0.0)
        .map(|p| -p * p.ln())
        .sum::<f64>();
    let eclusters = entropy.exp();
    let gate = if dominant >= dominant_relax {
        GateAction::Relax
    } else if eclusters >= eclusters_hi || (dominant < 0.55 && eclusters >= 2.0) {
        GateAction::Escalate
    } else {
        GateAction::Hold
    };
    Some(ScaleSignal {
        clusters: clusters.len(),
        dominant,
        eclusters,
        gate,
    })
}

pub(crate) fn gate_knobs_scaled_at(
    k: &Knobs,
    signal: Option<&ScaleSignal>,
    rounds: usize,
) -> (Knobs, Option<GateAction>) {
    let mut gated = k.clone();
    let Some(signal) = signal else {
        return (gated, None);
    };
    match signal.gate {
        GateAction::Escalate => {
            gated.judge = true;
            gated.verify = gated.verify.max(rounds);
            (gated, Some(GateAction::Escalate))
        }
        GateAction::Relax => {
            gated.layers = 1;
            gated.reflect = false;
            gated.judge = false;
            gated.verify = 0;
            gated.samples = 1;
            (gated, Some(GateAction::Relax))
        }
        GateAction::Hold => (gated, Some(GateAction::Hold)),
    }
}

pub(crate) fn stage_char_budget(club: &dyn Club) -> usize {
    let detected = club.metadata().map(|m| m.context_window).filter(|&n| n > 0);
    let configured = env_usize("ANGEL_MOA_CTX_TOKENS", 0);
    let Some(ctx) = detected.or_else(|| (configured > 0).then_some(configured)) else {
        // An unknown route does not inherit an invented 32K/128K ceiling.
        // The provider still owns its physical request limit; operators can
        // supply ANGEL_MOA_CTX_TOKENS when the surface cannot report metadata.
        return usize::MAX;
    };
    let reserve = env_usize("ANGEL_MOA_CTX_RESERVE_OUT", 8192);
    ctx.saturating_sub(reserve).saturating_mul(4)
}

pub(crate) fn draft_min_chars() -> usize {
    env_usize("ANGEL_MOA_DRAFT_MIN_CHARS", 700)
}

pub(crate) fn per_draft_cap(n: usize, budget_chars: usize) -> Option<usize> {
    if n == 0 {
        return Some(moa_draft_max_chars());
    }
    let per = budget_chars / n.max(1);
    if per < draft_min_chars() {
        None
    } else {
        let configured = moa_draft_max_chars();
        // Zero means there is no product-authored draft ceiling. The only
        // remaining bound is the actual downstream route context.
        Some(if configured == 0 {
            per
        } else {
            per.min(configured)
        })
    }
}

pub(crate) fn per_draft_cap_for(club: &dyn Club, n: usize) -> Option<usize> {
    per_draft_cap(n, stage_char_budget(club))
}

/// Whether a stage can consume every draft without product- or context-driven
/// truncation. A false result asks the pipeline to spend another map/reduce
/// round instead of silently throwing away proposer output.
pub(crate) fn drafts_fit_untruncated_for(club: &dyn Club, drafts: &[String]) -> bool {
    let Some(cap) = per_draft_cap_for(club, drafts.len()) else {
        return false;
    };
    cap == 0 || drafts.iter().all(|draft| draft.chars().count() <= cap)
}
