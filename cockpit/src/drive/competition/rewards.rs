use super::profile::DeepCutProfileIdentityV1;
use super::rewards_decimal::subtract_score;
use super::rewards_validation::{
    same_official_event, same_reward_identity, validate_binding, validate_context, validate_ledger,
    validate_result,
};
use super::schema::{
    AdapterIdentityV1, ComparatorKindV1, CompetitionKeyV1, ObjectiveComparatorV1, ScoreV1,
};
use super::schema_validation::{validate_id, validate_sha256};
use crate::agent::harness::live_policy;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub(crate) const OFFICIAL_REWARD_SCHEMA_V1: &str = "angel.deep-cut-official-reward/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RewardContextV1 {
    pub(crate) episode_id: String,
    pub(crate) candidate_id: String,
    pub(crate) submission_id: String,
    pub(crate) comparable_base_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) profile: DeepCutProfileIdentityV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OfficialResultSourceV1 {
    OfficialSubmissionResult,
    ModelSelfReport,
    ControlGate,
    UnrelatedRankMovement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialProvenanceV1 {
    pub(crate) source: OfficialResultSourceV1,
    pub(crate) adapter: AdapterIdentityV1,
    pub(crate) receipt_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialResultV1 {
    pub(crate) result_id: String,
    pub(crate) result_revision: u64,
    pub(crate) episode_id: String,
    pub(crate) candidate_id: String,
    pub(crate) submission_id: String,
    pub(crate) comparable_base_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) profile: DeepCutProfileIdentityV1,
    pub(crate) candidate_score: ScoreV1,
    pub(crate) base_score: ScoreV1,
    pub(crate) provenance: OfficialProvenanceV1,
    pub(crate) corrects_binding_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrientedComparisonV1 {
    Improvement,
    Tie,
    Regression,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialRewardBindingV1 {
    pub(crate) schema: String,
    pub(crate) binding_id: String,
    pub(crate) episode_id: String,
    pub(crate) candidate_id: String,
    pub(crate) submission_id: String,
    pub(crate) official_result_id: String,
    pub(crate) official_result_revision: u64,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) profile: DeepCutProfileIdentityV1,
    pub(crate) comparable_base_id: String,
    pub(crate) candidate_score: ScoreV1,
    pub(crate) base_score: ScoreV1,
    pub(crate) oriented_comparison: OrientedComparisonV1,
    pub(crate) oriented_delta: ScoreV1,
    pub(crate) primary_reward: i8,
    pub(crate) official_provenance: OfficialProvenanceV1,
    pub(crate) corrects_binding_id: Option<String>,
}

impl OfficialRewardBindingV1 {
    pub(crate) fn validate(&self) -> Result<(), String> {
        validate_binding(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProvisionalShapingV1 {
    pub(crate) shaping_id: String,
    pub(crate) episode_id: String,
    pub(crate) value_millis: i64,
    pub(crate) provenance_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindDispositionV1 {
    Appended,
    Duplicate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RewardSelectionV1<'a> {
    Official(&'a OfficialRewardBindingV1),
    Provisional(&'a ProvisionalShapingV1),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RewardLedgerV1 {
    shaping: Vec<ProvisionalShapingV1>,
    bindings: Vec<OfficialRewardBindingV1>,
}

impl RewardLedgerV1 {
    pub(crate) fn record_shaping(&mut self, shaping: ProvisionalShapingV1) -> Result<bool, String> {
        validate_id(&shaping.shaping_id, "invalid shaping id").map_err(str::to_string)?;
        validate_id(&shaping.episode_id, "invalid episode id").map_err(str::to_string)?;
        validate_sha256(&shaping.provenance_sha256).map_err(str::to_string)?;
        let rows = shaping_rows(&self.shaping, &shaping);
        let (decision, leg) = decide_shaping(&rows);
        tracing::debug!(
            leg = LEG,
            source = leg.as_str(),
            rows = rows.len(),
            "provisional shaping event"
        );
        match decision.outcome {
            ShapingOutcome::Append => {
                self.shaping.push(shaping);
                Ok(true)
            }
            ShapingOutcome::Duplicate => Ok(false),
            ShapingOutcome::Conflict => Err("conflicting shaping identity".into()),
        }
    }

    pub(crate) fn bind_official(
        &mut self,
        context: &RewardContextV1,
        result: OfficialResultV1,
    ) -> Result<(BindDispositionV1, OfficialRewardBindingV1), String> {
        validate_context(context)?;
        validate_result(context, &result)?;
        let binding = make_binding(result)?;
        let (decision, leg) = decide_binding(&self.bindings, &binding);
        tracing::debug!(
            leg = LEG,
            source = leg.as_str(),
            bindings = self.bindings.len(),
            "official reward binding"
        );
        match decision.outcome {
            BindingOutcome::Append => {
                self.bindings.push(binding.clone());
                Ok((BindDispositionV1::Appended, binding))
            }
            BindingOutcome::Duplicate => decision
                .index
                .and_then(|index| self.bindings.get(index))
                .cloned()
                .map(|existing| (BindDispositionV1::Duplicate, existing))
                .ok_or_else(|| "conflicting official result identity".into()),
            BindingOutcome::ConflictId => Err("conflicting official result identity".into()),
            BindingOutcome::ConflictReceipt => Err("conflicting official result receipt".into()),
            BindingOutcome::RejectFirstCorrection => {
                Err("first official result cannot be a correction".into())
            }
            BindingOutcome::RejectCorrection => {
                Err("correction must advance the canonical binding".into())
            }
        }
    }

    pub(crate) fn canonical_binding(&self, episode_id: &str) -> Option<&OfficialRewardBindingV1> {
        self.bindings
            .iter()
            .rev()
            .find(|entry| entry.episode_id == episode_id)
    }

    pub(crate) fn shaping(&self) -> &[ProvisionalShapingV1] {
        &self.shaping
    }

    pub(crate) fn bindings(&self) -> &[OfficialRewardBindingV1] {
        &self.bindings
    }

    pub(crate) fn effective_reward(&self, episode_id: &str) -> Option<RewardSelectionV1<'_>> {
        let bindings: Vec<bool> = self
            .bindings
            .iter()
            .map(|entry| entry.episode_id == episode_id)
            .collect();
        let shaping: Vec<bool> = self
            .shaping
            .iter()
            .map(|entry| entry.episode_id == episode_id)
            .collect();
        let (selection, leg) = decide_selection(&bindings, &shaping);
        tracing::debug!(
            leg = LEG,
            source = leg.as_str(),
            bindings = bindings.len(),
            shaping = shaping.len(),
            "effective reward selection"
        );
        match selection.kind {
            SelectionKind::Official => selection
                .index
                .and_then(|index| self.bindings.get(index))
                .map(RewardSelectionV1::Official),
            SelectionKind::Provisional => selection
                .index
                .and_then(|index| self.shaping.get(index))
                .map(RewardSelectionV1::Provisional),
            SelectionKind::Absent => None,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        validate_ledger(&self.shaping, &self.bindings)
    }
}

pub(super) fn make_binding(result: OfficialResultV1) -> Result<OfficialRewardBindingV1, String> {
    let (left, right) = match result.objective.kind {
        ComparatorKindV1::HigherIsBetter => (&result.candidate_score, &result.base_score),
        ComparatorKindV1::LowerIsBetter => (&result.base_score, &result.candidate_score),
        ComparatorKindV1::AdapterDefined { .. } => {
            return Err("adapter-defined reward comparison requires its adapter".into());
        }
    };
    let ordering = result
        .objective
        .compare(&result.candidate_score, &result.base_score)
        .map_err(|_| "unsupported objective comparison".to_string())?;
    let oriented_comparison = match ordering {
        Ordering::Greater => OrientedComparisonV1::Improvement,
        Ordering::Equal => OrientedComparisonV1::Tie,
        Ordering::Less => OrientedComparisonV1::Regression,
    };
    let oriented_delta = subtract_score(left, right)?;
    let mut binding = OfficialRewardBindingV1 {
        schema: OFFICIAL_REWARD_SCHEMA_V1.into(),
        binding_id: String::new(),
        episode_id: result.episode_id,
        candidate_id: result.candidate_id,
        submission_id: result.submission_id,
        official_result_id: result.result_id,
        official_result_revision: result.result_revision,
        competition: result.competition,
        objective: result.objective,
        profile: result.profile,
        comparable_base_id: result.comparable_base_id,
        candidate_score: result.candidate_score,
        base_score: result.base_score,
        oriented_comparison,
        oriented_delta,
        primary_reward: match ordering {
            Ordering::Greater => 1,
            Ordering::Equal => 0,
            Ordering::Less => -1,
        },
        official_provenance: result.provenance,
        corrects_binding_id: result.corrects_binding_id,
    };
    binding.binding_id = binding_sha(&binding)?;
    Ok(binding)
}

pub(super) fn binding_sha(binding: &OfficialRewardBindingV1) -> Result<String, String> {
    let mut value = binding.clone();
    value.binding_id.clear();
    serde_json::to_vec(&value)
        .map(|body| crate::knowledge::cut::sha256_hex(&body))
        .map_err(|error| format!("encode official reward binding: {error}"))
}

/// The shipped Bend leg: one evaluation answers one ledger decision. The row
/// scans below stay as the native fallback and as the parity oracle.
const LEG: &str = "rewards";

/// Which row is the episode's effective reward, as the leg reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SelectionKind {
    Official,
    Provisional,
    Absent,
}

/// An index into the ledger's own rows: `official` and `provisional` carry one,
/// `absent` carries none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    pub(super) kind: SelectionKind,
    pub(super) index: Option<usize>,
}

impl Selection {
    /// The native rule's own guarantees, which a hot edit must not break: a
    /// named row exists and is a row the claim is about.
    pub(super) fn check(&self, bindings: &[bool], shaping: &[bool]) -> Result<(), String> {
        let (name, flags) = match self.kind {
            SelectionKind::Absent => {
                return match self.index {
                    None => Ok(()),
                    Some(index) => Err(format!(
                        "live rewards leg reported no selection with row index {index}"
                    )),
                };
            }
            SelectionKind::Official => ("official", bindings),
            SelectionKind::Provisional => ("provisional", shaping),
        };
        let index = self
            .index
            .ok_or_else(|| format!("live rewards leg chose {name} without a row index"))?;
        match flags.get(index) {
            Some(true) => Ok(()),
            Some(false) => Err(format!(
                "live rewards leg chose {name} index {index}, which is not this episode's row"
            )),
            None => Err(format!(
                "live rewards leg chose {name} index {index} beyond {} rows",
                flags.len()
            )),
        }
    }
}

/// One existing shaping row as the leg reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ShapingRow {
    pub(super) id_equal: bool,
    pub(super) entry_equal: bool,
}

/// One existing binding as the leg reads it. Rust decides the text equalities
/// and the identity/lifecycle comparisons; the leg decides what they mean.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct BindingRow {
    pub(super) result_id_equal: bool,
    pub(super) entry_equal: bool,
    pub(super) provenance_equal: bool,
    pub(super) same_event: bool,
    pub(super) episode_equal: bool,
    pub(super) identity_equal: bool,
    pub(super) correction_targets: bool,
    pub(super) revision_hi: u32,
    pub(super) revision_lo: u32,
}

/// The incoming binding's own facts. The revision travels as two `u32` halves
/// so the comparison is exact for every `u64`, not just the exactly
/// representable ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Incoming {
    pub(super) has_correction: bool,
    pub(super) revision_hi: u32,
    pub(super) revision_lo: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ShapingOutcome {
    Append,
    Duplicate,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShapingDecision {
    pub(super) outcome: ShapingOutcome,
    pub(super) index: Option<usize>,
}

impl ShapingDecision {
    pub(super) fn check(&self, rows: &[ShapingRow]) -> Result<(), String> {
        match self.outcome {
            ShapingOutcome::Append => match self.index {
                Some(index) => Err(format!(
                    "live rewards leg appended a shaping event with row index {index}"
                )),
                None if rows.iter().any(|row| row.id_equal) => {
                    Err("live rewards leg appended a shaping event that already exists".into())
                }
                None => Ok(()),
            },
            ShapingOutcome::Duplicate | ShapingOutcome::Conflict => {
                let name = match self.outcome {
                    ShapingOutcome::Duplicate => "duplicate",
                    _ => "conflict",
                };
                let index = self.index.ok_or_else(|| {
                    format!("live rewards leg reported {name} without a row index")
                })?;
                let row = rows.get(index).ok_or_else(|| {
                    format!(
                        "live rewards leg named shaping row {index} beyond {} rows",
                        rows.len()
                    )
                })?;
                if !row.id_equal {
                    return Err(format!(
                        "live rewards leg called shaping row {index} {name}, but it holds another id"
                    ));
                }
                if row.entry_equal == (self.outcome == ShapingOutcome::Duplicate) {
                    Ok(())
                } else {
                    Err(format!(
                        "live rewards leg called shaping row {index} {name}, which it is not"
                    ))
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BindingOutcome {
    Append,
    Duplicate,
    ConflictId,
    ConflictReceipt,
    RejectFirstCorrection,
    RejectCorrection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BindingDecision {
    pub(super) outcome: BindingOutcome,
    pub(super) index: Option<usize>,
}

impl BindingDecision {
    pub(super) fn check(&self, rows: &[BindingRow], incoming: &Incoming) -> Result<(), String> {
        let named = |index: Option<usize>| -> Result<&BindingRow, String> {
            let index = index.ok_or_else(|| {
                format!(
                    "live rewards leg reported {:?} without a row index",
                    self.outcome
                )
            })?;
            rows.get(index).ok_or_else(|| {
                format!(
                    "live rewards leg named binding row {index} beyond {} rows",
                    rows.len()
                )
            })
        };
        match self.outcome {
            BindingOutcome::Append => match self.index {
                None => Ok(()),
                Some(index) => Err(format!(
                    "live rewards leg appended an official result with row index {index}"
                )),
            },
            BindingOutcome::Duplicate => {
                let row = named(self.index)?;
                if (row.result_id_equal && row.entry_equal)
                    || (row.provenance_equal && row.same_event)
                {
                    Ok(())
                } else {
                    Err(format!(
                        "live rewards leg called binding row {} a duplicate, which it is not",
                        self.index.unwrap_or_default()
                    ))
                }
            }
            BindingOutcome::ConflictId => {
                let row = named(self.index)?;
                if row.result_id_equal && !row.entry_equal {
                    Ok(())
                } else {
                    Err(format!(
                        "live rewards leg called binding row {} a conflicting result id, which it is not",
                        self.index.unwrap_or_default()
                    ))
                }
            }
            BindingOutcome::ConflictReceipt => {
                let row = named(self.index)?;
                if row.provenance_equal && !row.same_event {
                    Ok(())
                } else {
                    Err(format!(
                        "live rewards leg called binding row {} a conflicting receipt, which it is not",
                        self.index.unwrap_or_default()
                    ))
                }
            }
            BindingOutcome::RejectFirstCorrection => {
                if !incoming.has_correction {
                    return Err(
                        "live rewards leg refused a first official result that is not a correction"
                            .into(),
                    );
                }
                match self.index {
                    Some(index) => Err(format!(
                        "live rewards leg refused a first correction while naming row {index}"
                    )),
                    None => Ok(()),
                }
            }
            BindingOutcome::RejectCorrection => {
                let row = named(self.index)?;
                if row.episode_equal {
                    Ok(())
                } else {
                    Err(format!(
                        "live rewards leg named binding row {} canonical, which is another episode",
                        self.index.unwrap_or_default()
                    ))
                }
            }
        }
    }
}

/// The existing shaping rows, and the one `record_shaping` is holding.
pub(super) fn shaping_rows(
    shaping: &[ProvisionalShapingV1],
    candidate: &ProvisionalShapingV1,
) -> Vec<ShapingRow> {
    shaping
        .iter()
        .map(|entry| ShapingRow {
            id_equal: entry.shaping_id == candidate.shaping_id,
            entry_equal: entry == candidate,
        })
        .collect()
}

pub(super) fn selection_input(bindings: &[bool], shaping: &[bool]) -> serde_json::Value {
    serde_json::json!({"op": "select", "bindings": bindings, "shaping": shaping})
}

fn decide_selection(bindings: &[bool], shaping: &[bool]) -> (Selection, live_policy::LegSource) {
    let input = selection_input(bindings, shaping);
    live_policy::decide_shipped(LEG, &input, |selection: &Selection| {
        selection.check(bindings, shaping)
    })
}

pub(super) fn shaping_input(rows: &[ShapingRow]) -> serde_json::Value {
    serde_json::json!({"op": "record_shaping", "rows": rows})
}

fn decide_shaping(rows: &[ShapingRow]) -> (ShapingDecision, live_policy::LegSource) {
    let input = shaping_input(rows);
    live_policy::decide_shipped(LEG, &input, |decision: &ShapingDecision| {
        decision.check(rows)
    })
}

/// The existing bindings, with every text comparison the native scans make
/// already decided, and the one `bind_official` is holding.
pub(super) fn binding_rows(
    bindings: &[OfficialRewardBindingV1],
    binding: &OfficialRewardBindingV1,
) -> Vec<BindingRow> {
    bindings
        .iter()
        .map(|entry| BindingRow {
            result_id_equal: entry.official_result_id == binding.official_result_id,
            entry_equal: entry == binding,
            provenance_equal: entry.official_provenance == binding.official_provenance,
            same_event: same_official_event(entry, binding),
            episode_equal: entry.episode_id == binding.episode_id,
            identity_equal: same_reward_identity(entry, binding),
            correction_targets: binding.corrects_binding_id.as_deref()
                == Some(entry.binding_id.as_str()),
            revision_hi: (entry.official_result_revision >> 32) as u32,
            revision_lo: entry.official_result_revision as u32,
        })
        .collect()
}

pub(super) fn incoming(binding: &OfficialRewardBindingV1) -> Incoming {
    Incoming {
        has_correction: binding.corrects_binding_id.is_some(),
        revision_hi: (binding.official_result_revision >> 32) as u32,
        revision_lo: binding.official_result_revision as u32,
    }
}

pub(super) fn binding_input(rows: &[BindingRow], incoming: &Incoming) -> serde_json::Value {
    serde_json::json!({"op": "bind_official", "rows": rows, "incoming": incoming})
}

fn decide_binding(
    bindings: &[OfficialRewardBindingV1],
    binding: &OfficialRewardBindingV1,
) -> (BindingDecision, live_policy::LegSource) {
    let rows = binding_rows(bindings, binding);
    let facts = incoming(binding);
    let input = binding_input(&rows, &facts);
    live_policy::decide_shipped(LEG, &input, |decision: &BindingDecision| {
        decision.check(&rows, &facts)
    })
}
