use super::adapters::AdapterFailureV1;
use super::board::{BoardEntryV1, SourceAccessClaimV1};
use super::schema::{ObjectiveComparatorV1, ScoreV1};
use crate::agent::harness::live_policy;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// The shipped Bend leg: one evaluation fills all three seats. The decision
/// lives in `cockpit/live/frontier/frontier.bend`; Rust builds the leg's input
/// and rejects an answer the scan could not have made.
const LEG: &str = "frontier";

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
        Ok(crate::knowledge::cut::sha256_hex(&body))
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
    let input = frontier_input(entries, comparator, &compare)?;
    let (selection, leg) = live_policy::decide_shipped(LEG, &input, |selection: &Selection| {
        selection.check(entries)
    });
    tracing::debug!(
        leg = LEG,
        source = leg.as_str(),
        entries = entries.len(),
        "frontier selection"
    );
    let selected = |index: Option<usize>| index.and_then(|index| entries.get(index));
    Ok(FrontierV1 {
        global_target: selected(selection.global.index()).map(global_ref),
        personal_best: selected(selection.personal.index()).map(personal_ref),
        source_accessible_base: selected(selection.source.index()).map(|entry| {
            SourceAccessibleBaseV1 {
                entry_id: entry.entry_id.clone(),
                participant_id: entry.participant_id.clone(),
                submission_id: entry.submission_id.clone(),
                score: entry.score.clone(),
                source: entry.source.clone().expect("source-filtered entry"),
                source_board_epoch: source_epoch,
                selected_from_observation_id: observation_id.to_string(),
            }
        }),
    })
}

/// One seat as the leg reports it: a present index or an explicit emptiness.
/// A missing key is not the same claim as `null`, so it is rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
struct Seat(Option<usize>);

impl Seat {
    fn index(self) -> Option<usize> {
        self.0
    }
}

/// The three seats the selection rule fills, as indices into the board slice.
/// The leg returns these; Rust builds every frontier struct from them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct Selection {
    global: Seat,
    personal: Seat,
    source: Seat,
}

impl Selection {
    /// The native rule's own guarantees, which a hot edit must not break: each
    /// seat names a real entry, the personal seat is a personal entry and the
    /// source seat carries a source access claim.
    fn check(&self, entries: &[BoardEntryV1]) -> Result<(), String> {
        fn seat(
            entries: &[BoardEntryV1],
            name: &str,
            index: Option<usize>,
            scope: fn(&BoardEntryV1) -> bool,
        ) -> Result<(), String> {
            let Some(index) = index else {
                return Ok(());
            };
            let entry = entries.get(index).ok_or_else(|| {
                format!(
                    "live frontier leg chose {name} index {index} beyond {} entries",
                    entries.len()
                )
            })?;
            if !scope(entry) {
                return Err(format!(
                    "live frontier leg chose {name} index {index}, which the native rule cannot select there"
                ));
            }
            Ok(())
        }
        seat(entries, "global", self.global.index(), |_| true)?;
        seat(entries, "personal", self.personal.index(), |entry| {
            entry.personal
        })?;
        seat(entries, "source", self.source.index(), |entry| {
            entry.source.is_some()
        })?;
        Ok(())
    }
}

/// Everything the selection compares, priced so the leg only compares integers.
/// `score` is the entry's rank under the objective comparator (higher is
/// better), the tie key's rank travels as two `u32` halves with a missing rank
/// priced as `u64::MAX` — the native `tie_key` sentinel — and `id` is the entry
/// id's position among the sorted distinct ids, so the leg's integer compare is
/// the native string compare.
fn frontier_input<F>(
    entries: &[BoardEntryV1],
    comparator: &ObjectiveComparatorV1,
    compare: &F,
) -> Result<serde_json::Value, AdapterFailureV1>
where
    F: Fn(&ObjectiveComparatorV1, &ScoreV1, &ScoreV1) -> Result<Ordering, AdapterFailureV1>,
{
    let scores = score_ranks(entries, comparator, compare)?;
    let mut ids: Vec<&str> = entries
        .iter()
        .map(|entry| entry.entry_id.as_str())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let rows: Vec<_> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let tie = entry.rank.unwrap_or(u64::MAX);
            serde_json::json!({
                "index": index,
                "score": scores[index],
                "rank_hi": (tie >> 32) as u32,
                "rank_lo": tie as u32,
                "id": ids
                    .binary_search(&entry.entry_id.as_str())
                    .expect("every entry id is priced") as u32,
                "personal": entry.personal,
                "source": entry.source.is_some(),
            })
        })
        .collect();
    Ok(serde_json::json!({ "rows": rows }))
}

/// The comparator's order as ranks: equal-comparing scores share a rank and a
/// better score always ranks higher, which is all the replace rule reads.
fn score_ranks<F>(
    entries: &[BoardEntryV1],
    comparator: &ObjectiveComparatorV1,
    compare: &F,
) -> Result<Vec<u32>, AdapterFailureV1>
where
    F: Fn(&ObjectiveComparatorV1, &ScoreV1, &ScoreV1) -> Result<Ordering, AdapterFailureV1>,
{
    let mut order: Vec<usize> = (0..entries.len()).collect();
    for position in 1..order.len() {
        let mut at = position;
        while at > 0 {
            let candidate = &entries[order[at]];
            let current = &entries[order[at - 1]];
            match compare(comparator, &candidate.score, &current.score)? {
                Ordering::Greater => {
                    order.swap(at - 1, at);
                    at -= 1;
                }
                _ => break,
            }
        }
    }
    let mut ranks = vec![0u32; entries.len()];
    let mut rank = 0u32;
    for (position, index) in order.iter().enumerate() {
        let same = position > 0
            && compare(
                comparator,
                &entries[*index].score,
                &entries[order[position - 1]].score,
            )? == Ordering::Equal;
        if !same {
            rank = (entries.len() - position) as u32;
        }
        ranks[*index] = rank;
    }
    Ok(ranks)
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
#[path = "../../../../tests/cockpit/competition/frontier__tests.rs"]
mod tests;
