use super::adapters::AdapterFailureV1;
use super::board::{BoardEntryV1, SourceAccessClaimV1};
use super::schema::{ObjectiveComparatorV1, ScoreV1};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GlobalTargetV1 {
    pub(crate) entry_id: String,
    pub(crate) participant_id: String,
    pub(crate) submission_id: Option<String>,
    pub(crate) rank: Option<u64>,
    pub(crate) score: ScoreV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PersonalBestV1 {
    pub(crate) entry_id: String,
    pub(crate) participant_id: String,
    pub(crate) submission_id: Option<String>,
    pub(crate) rank: Option<u64>,
    pub(crate) score: ScoreV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceAccessibleBaseV1 {
    pub(crate) entry_id: String,
    pub(crate) participant_id: String,
    pub(crate) submission_id: Option<String>,
    pub(crate) score: ScoreV1,
    pub(crate) source: SourceAccessClaimV1,
    pub(crate) source_board_epoch: u64,
    pub(crate) selected_from_observation_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrontierV1 {
    pub(crate) global_target: Option<GlobalTargetV1>,
    pub(crate) personal_best: Option<PersonalBestV1>,
    pub(crate) source_accessible_base: Option<SourceAccessibleBaseV1>,
}

impl FrontierV1 {
    pub(crate) fn same_decision(&self, other: &Self) -> bool {
        self.global_target == other.global_target
            && self.personal_best == other.personal_best
            && source_decision(self.source_accessible_base.as_ref())
                == source_decision(other.source_accessible_base.as_ref())
    }

    pub(crate) fn decision_sha256(
        &self,
        comparator: &ObjectiveComparatorV1,
    ) -> Result<String, String> {
        let body = serde_json::to_vec(&(
            comparator,
            &self.global_target,
            &self.personal_best,
            source_decision(self.source_accessible_base.as_ref()),
        ))
        .map_err(|error| format!("encode frontier decision: {error}"))?;
        Ok(crate::cut::sha256_hex(&body))
    }
}

fn source_decision(
    source: Option<&SourceAccessibleBaseV1>,
) -> Option<(&str, &str, Option<&str>, &ScoreV1, &SourceAccessClaimV1)> {
    source.map(|base| {
        (
            base.entry_id.as_str(),
            base.participant_id.as_str(),
            base.submission_id.as_deref(),
            &base.score,
            &base.source,
        )
    })
}

pub(crate) fn reduce_frontier<F>(
    entries: &[BoardEntryV1],
    comparator: &ObjectiveComparatorV1,
    source_epoch: u64,
    observation_id: &str,
    compare: F,
) -> Result<FrontierV1, AdapterFailureV1>
where
    F: Fn(&ObjectiveComparatorV1, &ScoreV1, &ScoreV1) -> Result<Ordering, AdapterFailureV1>,
{
    let global = best(entries.iter(), comparator, &compare)?;
    let personal = best(
        entries.iter().filter(|entry| entry.personal),
        comparator,
        &compare,
    )?;
    let source = best(
        entries.iter().filter(|entry| entry.source.is_some()),
        comparator,
        &compare,
    )?;
    Ok(FrontierV1 {
        global_target: global.map(global_ref),
        personal_best: personal.map(personal_ref),
        source_accessible_base: source.map(|entry| SourceAccessibleBaseV1 {
            entry_id: entry.entry_id.clone(),
            participant_id: entry.participant_id.clone(),
            submission_id: entry.submission_id.clone(),
            score: entry.score.clone(),
            source: entry.source.clone().expect("source-filtered entry"),
            source_board_epoch: source_epoch,
            selected_from_observation_id: observation_id.to_string(),
        }),
    })
}

fn best<'a, I, F>(
    entries: I,
    comparator: &ObjectiveComparatorV1,
    compare: &F,
) -> Result<Option<&'a BoardEntryV1>, AdapterFailureV1>
where
    I: Iterator<Item = &'a BoardEntryV1>,
    F: Fn(&ObjectiveComparatorV1, &ScoreV1, &ScoreV1) -> Result<Ordering, AdapterFailureV1>,
{
    let mut selected: Option<&BoardEntryV1> = None;
    for candidate in entries {
        let replace = match selected {
            None => true,
            Some(current) => match compare(comparator, &candidate.score, &current.score)? {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => tie_key(candidate) < tie_key(current),
            },
        };
        if replace {
            selected = Some(candidate);
        }
    }
    Ok(selected)
}

fn tie_key(entry: &BoardEntryV1) -> (u64, &str) {
    (entry.rank.unwrap_or(u64::MAX), entry.entry_id.as_str())
}

fn global_ref(entry: &BoardEntryV1) -> GlobalTargetV1 {
    GlobalTargetV1 {
        entry_id: entry.entry_id.clone(),
        participant_id: entry.participant_id.clone(),
        submission_id: entry.submission_id.clone(),
        rank: entry.rank,
        score: entry.score.clone(),
    }
}

fn personal_ref(entry: &BoardEntryV1) -> PersonalBestV1 {
    PersonalBestV1 {
        entry_id: entry.entry_id.clone(),
        participant_id: entry.participant_id.clone(),
        submission_id: entry.submission_id.clone(),
        rank: entry.rank,
        score: entry.score.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::competition::board::SourceAccessClaimV1;
    use crate::competition::schema::ComparatorKindV1;

    fn comparator() -> ObjectiveComparatorV1 {
        ObjectiveComparatorV1 {
            objective_id: "score".into(),
            version: "1".into(),
            kind: ComparatorKindV1::HigherIsBetter,
        }
    }

    fn entry(id: &str, score: &str, personal: bool, source: bool) -> BoardEntryV1 {
        BoardEntryV1 {
            entry_id: id.into(),
            participant_id: format!("participant-{id}"),
            submission_id: Some(format!("submission-{id}")),
            rank: None,
            score: ScoreV1::new(score).unwrap(),
            personal,
            source: source.then(|| SourceAccessClaimV1 {
                source_id: format!("source-{id}"),
                commit_oid: format!("commit-{id}"),
                tree_oid: format!("tree-{id}"),
                workspace_sha256: crate::cut::sha256_hex(format!("workspace-{id}").as_bytes()),
                access_proof_sha256: crate::cut::sha256_hex(format!("proof-{id}").as_bytes()),
            }),
        }
    }

    fn reduce(entries: &[BoardEntryV1], epoch: u64, observation: &str) -> FrontierV1 {
        reduce_frontier(
            entries,
            &comparator(),
            epoch,
            observation,
            |contract, candidate, baseline| Ok(contract.compare(candidate, baseline).unwrap()),
        )
        .unwrap()
    }

    /// FG-BASE-020 / AC-04.
    #[test]
    fn global_personal_and_accessible_frontiers_remain_distinct() {
        let frontier = reduce(
            &[
                entry("external", "100", false, false),
                entry("personal", "90", true, false),
                entry("accessible", "80", false, true),
            ],
            1,
            "observation-one",
        );
        assert_eq!(frontier.global_target.unwrap().entry_id, "external");
        assert_eq!(frontier.personal_best.unwrap().entry_id, "personal");
        assert_eq!(
            frontier.source_accessible_base.unwrap().entry_id,
            "accessible"
        );
    }

    /// FG-BASE-021 / AC-04.
    #[test]
    fn inaccessible_global_leader_changes_target_not_source_base() {
        let first = reduce(
            &[
                entry("external-old", "100", false, false),
                entry("accessible", "80", false, true),
            ],
            1,
            "observation-one",
        );
        let moved = reduce(
            &[
                entry("external-new", "110", false, false),
                entry("accessible", "80", false, true),
            ],
            2,
            "observation-two",
        );
        assert_eq!(
            moved.global_target.as_ref().unwrap().entry_id,
            "external-new"
        );
        assert_eq!(
            first.source_accessible_base.as_ref().unwrap().source,
            moved.source_accessible_base.as_ref().unwrap().source
        );
        assert_eq!(
            moved.source_accessible_base.as_ref().unwrap().entry_id,
            "accessible"
        );
        assert!(
            !first.same_decision(&moved),
            "target movement advances decision"
        );
    }

    #[test]
    fn source_selection_metadata_is_not_part_of_decision_digest() {
        let entries = [entry("accessible", "80", true, true)];
        let first = reduce(&entries, 1, "observation-one");
        let refresh = reduce(&entries, 99, "observation-later");
        assert!(first.same_decision(&refresh));
        assert_eq!(
            first.decision_sha256(&comparator()).unwrap(),
            refresh.decision_sha256(&comparator()).unwrap()
        );
    }
}
