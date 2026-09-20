use super::board::{
    BoardEntryV1, BoardFreshnessV1, CANONICAL_BOARD_SCHEMA_V1, CanonicalBoardV1,
    SourceAccessClaimV1,
};
use super::frontier::{GlobalTargetV1, PersonalBestV1, SourceAccessibleBaseV1};
use super::schema::{AdapterCapabilityV1, ComparatorKindV1};
use super::schema_validation::{validate_id, validate_sha256};
use std::cmp::Ordering;
use std::collections::BTreeSet;

pub(super) fn validate_board(board: &CanonicalBoardV1, campaign_id: &str) -> Result<(), String> {
    if board.schema != CANONICAL_BOARD_SCHEMA_V1
        || board.campaign_id != campaign_id
        || board.board_epoch == 0
        || board.observation_revision == 0
        || board.predecessor_epoch != (board.board_epoch > 1).then_some(board.board_epoch - 1)
    {
        return Err("invalid canonical board identity or ancestry".into());
    }
    board.competition.validate().map_err(str::to_string)?;
    board.comparator.validate().map_err(str::to_string)?;
    validate_id(&board.latest_observation_id, "invalid latest observation")
        .map_err(str::to_string)?;
    board
        .latest_provenance
        .adapter
        .validate()
        .map_err(str::to_string)?;
    if !board
        .latest_provenance
        .adapter
        .capabilities
        .contains(&AdapterCapabilityV1::Board)
    {
        return Err("canonical board provenance lacks board capability".into());
    }
    validate_sha256(&board.latest_provenance.raw_sha256).map_err(str::to_string)?;
    validate_freshness(&board.freshness)?;
    let mut ids = BTreeSet::new();
    for entry in &board.entries {
        validate_entry(entry)?;
        if !ids.insert(&entry.entry_id) {
            return Err("canonical board entry identity is duplicated".into());
        }
    }
    validate_frontier(board)?;
    if board.frontier.decision_sha256(&board.comparator)? != board.decision_sha256 {
        return Err("canonical board decision digest mismatch".into());
    }
    validate_sha256(&board.decision_sha256).map_err(str::to_string)
}

fn validate_entry(entry: &BoardEntryV1) -> Result<(), String> {
    validate_id(&entry.entry_id, "invalid board entry").map_err(str::to_string)?;
    validate_id(&entry.participant_id, "invalid board participant").map_err(str::to_string)?;
    if let Some(id) = &entry.submission_id {
        validate_id(id, "invalid board submission").map_err(str::to_string)?;
    }
    if entry.rank == Some(0) {
        return Err("board rank must be positive".into());
    }
    if let Some(source) = &entry.source {
        validate_source(source)?;
    }
    Ok(())
}

fn validate_source(source: &SourceAccessClaimV1) -> Result<(), String> {
    validate_id(&source.source_id, "invalid source id").map_err(str::to_string)?;
    validate_object_oid(&source.commit_oid)?;
    validate_object_oid(&source.tree_oid)?;
    validate_sha256(&source.workspace_sha256).map_err(str::to_string)?;
    validate_sha256(&source.access_proof_sha256).map_err(str::to_string)
}

fn validate_object_oid(value: &str) -> Result<(), String> {
    if validate_sha256(value).is_ok()
        || (value.len() == 40
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    {
        Ok(())
    } else {
        Err("invalid source object identity".into())
    }
}

fn validate_freshness(freshness: &BoardFreshnessV1) -> Result<(), String> {
    match freshness {
        BoardFreshnessV1::Fresh => Ok(()),
        BoardFreshnessV1::Stale { reason, .. } | BoardFreshnessV1::Unconfirmed { reason, .. } => {
            validate_id(reason, "invalid board freshness reason").map_err(str::to_string)
        }
    }
}

fn validate_frontier(board: &CanonicalBoardV1) -> Result<(), String> {
    validate_global(board, board.frontier.global_target.as_ref())?;
    validate_personal(board, board.frontier.personal_best.as_ref())?;
    validate_source_base(board, board.frontier.source_accessible_base.as_ref())?;
    if !matches!(
        board.comparator.kind,
        ComparatorKindV1::AdapterDefined { .. }
    ) {
        let best = |filter: fn(&BoardEntryV1) -> bool| -> Result<Option<&BoardEntryV1>, String> {
            board.entries.iter().filter(|entry| filter(entry)).try_fold(
                None::<&BoardEntryV1>,
                |selected, candidate| match selected {
                    None => Ok(Some(candidate)),
                    Some(current) => {
                        let order = board
                            .comparator
                            .compare(&candidate.score, &current.score)
                            .map_err(|_| "board comparator unavailable".to_string())?;
                        let better_tie = order == Ordering::Equal
                            && (
                                candidate.rank.unwrap_or(u64::MAX),
                                candidate.entry_id.as_str(),
                            ) < (current.rank.unwrap_or(u64::MAX), current.entry_id.as_str());
                        Ok(if order == Ordering::Greater || better_tie {
                            Some(candidate)
                        } else {
                            Some(current)
                        })
                    }
                },
            )
        };
        if board
            .frontier
            .global_target
            .as_ref()
            .map(|item| item.entry_id.as_str())
            != best(|_| true)?.map(|item| item.entry_id.as_str())
            || board
                .frontier
                .personal_best
                .as_ref()
                .map(|item| item.entry_id.as_str())
                != best(|item| item.personal)?.map(|item| item.entry_id.as_str())
            || board
                .frontier
                .source_accessible_base
                .as_ref()
                .map(|item| item.entry_id.as_str())
                != best(|item| item.source.is_some())?.map(|item| item.entry_id.as_str())
        {
            return Err("canonical board frontier is not comparator-selected".into());
        }
    }
    Ok(())
}

fn validate_global(
    board: &CanonicalBoardV1,
    selected: Option<&GlobalTargetV1>,
) -> Result<(), String> {
    match selected {
        None if board.entries.is_empty() => Ok(()),
        Some(value)
            if board.entries.iter().any(|entry| {
                entry.entry_id == value.entry_id
                    && entry.participant_id == value.participant_id
                    && entry.submission_id == value.submission_id
                    && entry.rank == value.rank
                    && entry.score == value.score
            }) =>
        {
            Ok(())
        }
        _ => Err("canonical global frontier is not an exact board entry".into()),
    }
}

fn validate_personal(
    board: &CanonicalBoardV1,
    selected: Option<&PersonalBestV1>,
) -> Result<(), String> {
    let any = board.entries.iter().any(|entry| entry.personal);
    match selected {
        None if !any => Ok(()),
        Some(value)
            if board.entries.iter().any(|entry| {
                entry.personal
                    && entry.entry_id == value.entry_id
                    && entry.participant_id == value.participant_id
                    && entry.submission_id == value.submission_id
                    && entry.rank == value.rank
                    && entry.score == value.score
            }) =>
        {
            Ok(())
        }
        _ => Err("canonical personal frontier is not an exact personal entry".into()),
    }
}

fn validate_source_base(
    board: &CanonicalBoardV1,
    selected: Option<&SourceAccessibleBaseV1>,
) -> Result<(), String> {
    let any = board.entries.iter().any(|entry| entry.source.is_some());
    match selected {
        None if !any => Ok(()),
        Some(value)
            if value.source_board_epoch == board.board_epoch
                && validate_id(
                    &value.selected_from_observation_id,
                    "invalid source selection observation",
                )
                .is_ok()
                && (matches!(board.freshness, BoardFreshnessV1::Unconfirmed { .. })
                    || value.selected_from_observation_id == board.latest_observation_id)
                && board.entries.iter().any(|entry| {
                    entry.entry_id == value.entry_id
                        && entry.participant_id == value.participant_id
                        && entry.submission_id == value.submission_id
                        && entry.score == value.score
                        && entry.source.as_ref() == Some(&value.source)
                }) =>
        {
            Ok(())
        }
        _ => Err("canonical source frontier is not an exact accessible entry".into()),
    }
}
