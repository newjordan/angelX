use super::flywheel_files::{IN_FLIGHT_STATUSES, canonical_score, invariant, malformed, transport};
use super::{AdapterFailureV1, CompetitionIdentityV1};
use crate::competition::profile::{DeepCutProfileIdentityV1, DeepCutProfileV1};
use crate::competition::rewards::{
    OfficialProvenanceV1, OfficialResultSourceV1, OfficialResultV1, RewardContextV1, RewardLedgerV1,
};
use crate::competition::schema::AdapterIdentityV1;
use FlywheelSettleFailureV1::{Adapter, Phantom};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

/// Official flywheel results: `fires.jsonl` is our submission journal. Every
/// fire with a terminal `outcome` is official board evidence (INV-08: only
/// official results bind reward); in-flight fires bind nothing.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct FlywheelFireV1 {
    #[serde(default)]
    pub(crate) sub: String,
    #[serde(default)]
    pub(crate) base: Option<String>,
    #[serde(default)]
    pub(crate) base_score: Option<f64>,
    #[serde(default)]
    pub(crate) digest: Option<String>,
    #[serde(default)]
    pub(crate) outcome: Option<FlywheelFireOutcomeV1>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct FlywheelFireOutcomeV1 {
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) promoted: bool,
    #[serde(default)]
    pub(crate) score: Option<f64>,
    #[serde(default)]
    pub(crate) delta_pct: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct FlywheelRewardSummaryV1 {
    pub(crate) candidate: String,
    pub(crate) base: String,
    pub(crate) delta_pct: Option<f64>,
    pub(crate) promoted: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum FlywheelSettleFailureV1 {
    Adapter { failure: AdapterFailureV1 },
    Phantom { submission_id: String },
}

#[derive(Default)]
pub(crate) struct FlywheelSettlementV1 {
    pub(crate) rewards: Vec<FlywheelRewardSummaryV1>,
    pub(crate) failures: Vec<FlywheelSettleFailureV1>,
}

struct ParsedFireV1 {
    fire: FlywheelFireV1,
    receipt_sha256: String,
}

fn parse_fires(state_dir: &Path) -> Result<Vec<ParsedFireV1>, AdapterFailureV1> {
    // A track that has never fired has no fires.jsonl yet: that is the normal
    // fresh state, not a transport failure. Only an unreadable existing file is.
    let body = match std::fs::read_to_string(state_dir.join("fires.jsonl")) {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(transport(&format!("fires.jsonl unreadable: {error}"))),
    };
    let mut fires = Vec::new();
    for (index, line) in body.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fire: FlywheelFireV1 = serde_json::from_str(line).map_err(|_| {
            malformed(&format!(
                "fires.jsonl line {} is not a fire record",
                index + 1
            ))
        })?;
        fires.push(ParsedFireV1 {
            fire,
            receipt_sha256: crate::cut::sha256_hex(line.as_bytes()),
        });
    }
    Ok(fires)
}

pub(crate) fn settle_fires(
    state_dir: &Path,
    identity: &AdapterIdentityV1,
    competition: &CompetitionIdentityV1,
    owned_submission_ids: &BTreeSet<String>,
    ledger: &mut RewardLedgerV1,
) -> FlywheelSettlementV1 {
    let mut settlement = FlywheelSettlementV1::default();
    let profile = match DeepCutProfileV1::embedded().identity() {
        Ok(profile) => profile,
        Err(error) => {
            settlement.failures.push(Adapter {
                failure: invariant(&format!("embedded deep-cut profile: {error}")),
            });
            return settlement;
        }
    };
    let fires = match parse_fires(state_dir) {
        Ok(fires) => fires,
        Err(failure) => {
            settlement.failures.push(Adapter { failure });
            return settlement;
        }
    };
    for parsed in fires {
        bind_fire(
            parsed,
            identity,
            competition,
            owned_submission_ids,
            &profile,
            ledger,
            &mut settlement,
        );
    }
    settlement
}

#[allow(clippy::too_many_arguments)]
fn bind_fire(
    parsed: ParsedFireV1,
    identity: &AdapterIdentityV1,
    competition: &CompetitionIdentityV1,
    owned_submission_ids: &BTreeSet<String>,
    profile: &DeepCutProfileIdentityV1,
    ledger: &mut RewardLedgerV1,
    settlement: &mut FlywheelSettlementV1,
) {
    let fire = parsed.fire;
    let sub = fire.sub.clone();
    let reject = |settlement: &mut FlywheelSettlementV1, failure: AdapterFailureV1| {
        settlement.failures.push(Adapter { failure });
    };
    let Some(outcome) = fire.outcome else {
        return; // in flight: produces nothing
    };
    if IN_FLIGHT_STATUSES.contains(&outcome.status.as_str()) {
        return;
    }
    if sub.is_empty() {
        reject(settlement, invariant("fire lacks a submission id"));
        return;
    }
    let Some(digest) = fire.digest.filter(|value| !value.is_empty()) else {
        reject(settlement, invariant("fire lacks a tree digest"));
        return;
    };
    let Some(base) = fire.base.filter(|value| !value.is_empty()) else {
        reject(
            settlement,
            invariant(&format!("fire {digest} lacks a base commit")),
        );
        return;
    };
    let Some(base_score) = fire.base_score else {
        reject(
            settlement,
            invariant(&format!("fire {digest} lacks a base score")),
        );
        return;
    };
    let Some(candidate_score_value) = outcome.score else {
        reject(
            settlement,
            invariant(&format!("fire {digest} outcome lacks a score")),
        );
        return;
    };
    if !owned_submission_ids.contains(&sub) {
        settlement.failures.push(Phantom { submission_id: sub });
        return;
    }
    let (base_score, candidate_score) = match (
        canonical_score(base_score),
        canonical_score(candidate_score_value),
    ) {
        (Ok(base), Ok(candidate)) => (base, candidate),
        (Err(failure), _) | (_, Err(failure)) => {
            reject(settlement, failure);
            return;
        }
    };
    let context = RewardContextV1 {
        episode_id: format!("flywheel-episode-{digest}"),
        candidate_id: digest.clone(),
        submission_id: sub,
        comparable_base_id: base.clone(),
        competition: competition.competition.clone(),
        objective: competition.comparator.clone(),
        profile: profile.clone(),
    };
    let result = OfficialResultV1 {
        result_id: format!("flywheel-result-{digest}"),
        result_revision: 1,
        episode_id: context.episode_id.clone(),
        candidate_id: context.candidate_id.clone(),
        submission_id: context.submission_id.clone(),
        comparable_base_id: context.comparable_base_id.clone(),
        competition: context.competition.clone(),
        objective: context.objective.clone(),
        profile: context.profile.clone(),
        candidate_score,
        base_score,
        provenance: OfficialProvenanceV1 {
            source: OfficialResultSourceV1::OfficialSubmissionResult,
            adapter: identity.clone(),
            receipt_sha256: parsed.receipt_sha256,
        },
        corrects_binding_id: None,
    };
    if let Err(error) = ledger.bind_official(&context, result) {
        reject(
            settlement,
            invariant(&format!("official binding rejected: {error}")),
        );
        return;
    }
    settlement.rewards.push(FlywheelRewardSummaryV1 {
        candidate: digest,
        base,
        delta_pct: outcome.delta_pct,
        promoted: outcome.promoted,
    });
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/adapters__flywheel_results_tests.rs"]
mod tests;
