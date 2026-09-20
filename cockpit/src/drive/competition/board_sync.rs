use super::adapters::flywheel::FlywheelAdapterV1;
use super::adapters::flywheel_files::{canonical_score, invariant};
use super::adapters::flywheel_results::{
    FlywheelRewardSummaryV1, FlywheelSettleFailureV1, settle_fires,
};
use super::adapters::{AdapterFailureV1, CompetitionAdapterV1, CompetitionIdentityV1};
use super::board_reducer::BoardReducerV1;
use super::rewards::RewardLedgerV1;
use super::schema::ScoreV1;
use serde::Serialize;
use std::path::Path;

pub(crate) const BOARD_SYNC_SCHEMA_V1: &str = "angel.board-sync/v1";

/// One live path through the director: open the flywheel adapter, engage the
/// board reducer over an in-memory raw journal, observe personal submissions,
/// and bind official fire results through the reward join. Typed failures
/// (including phantom fires) are part of the report; only an unreadable state
/// dir is an error.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct BoardSyncReportV1 {
    pub(crate) schema: String,
    pub(crate) identity: CompetitionIdentityV1,
    pub(crate) canonical_board: Option<BoardSyncFrontierV1>,
    pub(crate) personal: BoardSyncPersonalV1,
    pub(crate) rewards: Vec<FlywheelRewardSummaryV1>,
    pub(crate) failures: Vec<FlywheelSettleFailureV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct BoardSyncFrontierV1 {
    pub(crate) score: ScoreV1,
    pub(crate) commit: Option<String>,
    pub(crate) solver: String,
    pub(crate) epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct BoardSyncPersonalV1 {
    pub(crate) in_flight: Vec<String>,
    pub(crate) best: Option<BoardSyncPersonalBestV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct BoardSyncPersonalBestV1 {
    pub(crate) submission_id: String,
    pub(crate) solver: String,
    pub(crate) score: ScoreV1,
    pub(crate) status: Option<String>,
}

pub(crate) fn run(state_dir: &Path) -> Result<String, AdapterFailureV1> {
    let mut adapter = FlywheelAdapterV1::open(state_dir)?;
    let mut failures: Vec<FlywheelSettleFailureV1> = Vec::new();
    let identity = adapter.identify_competition()?;
    let mut reducer = BoardReducerV1::default();
    let mut journal = super::adapters::fixture::MemoryRawBoardJournalV1::default();
    let now_ms = adapter.observed_at_ms();
    let canonical_board =
        match reducer.engage("flywheel-board-sync", &mut adapter, &mut journal, now_ms) {
            Ok(Ok(outcome)) => outcome.canonical.and_then(|board| {
                let target = board.frontier.global_target;
                target.map(|target| BoardSyncFrontierV1 {
                    score: target.score,
                    commit: adapter
                        .frontier_row()
                        .and_then(|row| row.commit())
                        .map(str::to_string),
                    solver: target.participant_id,
                    epoch: board.board_epoch,
                })
            }),
            Ok(Err(fetch_failure)) => {
                failures.push(FlywheelSettleFailureV1::Adapter {
                    failure: fetch_failure.failure,
                });
                None
            }
            Err(reduce_error) => {
                failures.push(FlywheelSettleFailureV1::Adapter {
                    failure: invariant(&format!("board reduce: {reduce_error:?}")),
                });
                None
            }
        };
    if let Err(failure) = adapter.fetch_personal_submissions(None) {
        failures.push(FlywheelSettleFailureV1::Adapter { failure });
    }
    let personal = BoardSyncPersonalV1 {
        in_flight: adapter
            .personal_in_flight()
            .into_iter()
            .map(|(submission_id, _)| submission_id)
            .collect(),
        best: adapter.personal_best().and_then(|row| {
            Some(BoardSyncPersonalBestV1 {
                submission_id: row.id.clone(),
                solver: row.solver_username.clone(),
                score: canonical_score(row.official_score?).ok()?,
                status: row.status.clone(),
            })
        }),
    };
    let owned = adapter.personal_submission_ids();
    let mut ledger = RewardLedgerV1::default();
    let settlement = settle_fires(
        state_dir,
        adapter.identity(),
        &identity,
        &owned,
        &mut ledger,
    );
    let rewards = settlement.rewards;
    failures.extend(settlement.failures);
    let report = BoardSyncReportV1 {
        schema: BOARD_SYNC_SCHEMA_V1.into(),
        identity,
        canonical_board,
        personal,
        rewards,
        failures,
    };
    serde_json::to_string(&report)
        .map_err(|error| invariant(&format!("board-sync report encode: {error}")))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/board_sync_tests.rs"]
mod tests;
