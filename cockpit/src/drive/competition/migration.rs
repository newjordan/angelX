use super::board::{BoardFreshnessV1, CanonicalBoardV1};
use super::candidate_store::CandidateRepositoryV1;
use super::director::{CompetitionDirectorStateV1, DIRECTOR_STATE_SCHEMA_V1};
use super::director_services::{
    DirectorEpisodeRefV1, DirectorServiceIntentV1, DirectorServiceKindV1, DirectorWorkerRefV1,
};
use super::migration_state::{MigratedDirectorStateV1, MigrationQualificationMarkerV1};
use super::patterns::FieldPatternMemoryV1;
use super::rewards::RewardLedgerV1;
use super::schema::{
    COMPETITION_CONTRACT_V1, COMPETITION_SCHEMA_V1, DirectorHealthStateV1, DirectorHealthV1,
    ScheduledActionV1,
};
use super::schema_validation::validate_id;
use super::submission::SubmissionSpoolV1;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const LEGACY_DIRECTOR_SCHEMA_V0: &str = "angel.competition-director-state/v0";
pub(crate) const MIGRATION_QUALIFICATION_REQUIRED_REASON: &str =
    "legacy-v0 state awaits journal and dossier qualification";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyPersistenceV0 {
    Absent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyPendingActionV0 {
    pub(crate) intent: DirectorServiceIntentV1,
    pub(crate) provenance_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyDirectorStateV0 {
    pub(crate) schema: String,
    pub(crate) revision: u64,
    pub(crate) campaign_id: String,
    pub(crate) last_good_board: Option<CanonicalBoardV1>,
    pub(crate) candidates: Option<CandidateRepositoryV1>,
    pub(crate) submissions: SubmissionSpoolV1,
    pub(crate) rewards: RewardLedgerV1,
    pub(crate) patterns: FieldPatternMemoryV1,
    pub(crate) episodes: BTreeMap<String, DirectorEpisodeRefV1>,
    pub(crate) workers: BTreeMap<String, DirectorWorkerRefV1>,
    pub(crate) pending_actions: Vec<LegacyPendingActionV0>,
    pub(crate) action_journal: LegacyPersistenceV0,
    pub(crate) dossier: LegacyPersistenceV0,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MigrationDispositionV1 {
    MigratedLegacyV0,
    AlreadyV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirectorMigrationV1 {
    pub(crate) disposition: MigrationDispositionV1,
    pub(crate) source_sha256: String,
    pub(crate) target_sha256: String,
    pub(crate) migration_id: String,
    pub(crate) pending_action_provenance: BTreeMap<String, String>,
    state: MigratedDirectorStateV1,
}

pub(crate) fn migrate_director_state(
    raw: &[u8],
    resume_at_ms: u64,
) -> Result<DirectorMigrationV1, String> {
    let source_sha256 = crate::knowledge::cut::sha256_hex(raw);
    let value: serde_json::Value = serde_json::from_slice(raw)
        .map_err(|error| format!("parse director migration: {error}"))?;
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "director migration source lacks an explicit schema".to_string())?;
    let (disposition, state, pending_action_provenance, requires_reconciliation) = match schema {
        LEGACY_DIRECTOR_SCHEMA_V0 => {
            let legacy: LegacyDirectorStateV0 = serde_json::from_value(value)
                .map_err(|error| format!("parse legacy director v0: {error}"))?;
            let (state, provenance) = migrate_v0(legacy, resume_at_ms)?;
            (
                MigrationDispositionV1::MigratedLegacyV0,
                state,
                provenance,
                true,
            )
        }
        DIRECTOR_STATE_SCHEMA_V1 => {
            let state: CompetitionDirectorStateV1 = serde_json::from_value(value)
                .map_err(|error| format!("parse director v1: {error}"))?;
            state.validate()?;
            (
                MigrationDispositionV1::AlreadyV1,
                state,
                BTreeMap::new(),
                false,
            )
        }
        _ => return Err("unsupported or newer director migration schema".into()),
    };
    let target_body =
        serde_json::to_vec(&state).map_err(|error| format!("encode migrated director: {error}"))?;
    let target_sha256 = crate::knowledge::cut::sha256_hex(&target_body);
    let migration_id = crate::knowledge::cut::sha256_hex(
        serde_json::to_vec(&(
            COMPETITION_CONTRACT_V1,
            &source_sha256,
            &target_sha256,
            resume_at_ms,
        ))
        .map_err(|error| format!("encode migration receipt: {error}"))?
        .as_slice(),
    );
    let marker = if requires_reconciliation {
        MigrationQualificationMarkerV1::QualificationRequired {
            migration_id: migration_id.clone(),
            source_sha256: source_sha256.clone(),
            target_sha256: target_sha256.clone(),
        }
    } else {
        MigrationQualificationMarkerV1::NativeV1
    };
    Ok(DirectorMigrationV1 {
        disposition,
        source_sha256,
        target_sha256,
        migration_id,
        pending_action_provenance,
        state: MigratedDirectorStateV1 {
            marker,
            director: state,
        },
    })
}

impl DirectorMigrationV1 {
    pub(crate) fn qualification_required(&self) -> Result<bool, String> {
        match (&self.disposition, &self.state.marker) {
            (
                MigrationDispositionV1::MigratedLegacyV0,
                MigrationQualificationMarkerV1::QualificationRequired {
                    migration_id,
                    source_sha256,
                    target_sha256,
                },
            ) if migration_id == &self.migration_id
                && source_sha256 == &self.source_sha256
                && target_sha256 == &self.target_sha256 =>
            {
                Ok(true)
            }
            (MigrationDispositionV1::AlreadyV1, MigrationQualificationMarkerV1::NativeV1) => {
                Ok(false)
            }
            _ => Err("migration qualification marker conflicts with migration metadata".into()),
        }
    }

    pub(crate) fn board(&self) -> Option<&CanonicalBoardV1> {
        self.state.director.board.as_ref()
    }

    pub(crate) fn campaign_id(&self) -> &str {
        &self.state.director.campaign_id
    }

    pub(crate) fn health(&self) -> &DirectorHealthV1 {
        &self.state.director.health
    }

    pub(crate) fn services(&self) -> &[DirectorServiceIntentV1] {
        &self.state.director.services
    }

    pub(crate) fn matches_director(&self, director: &CompetitionDirectorStateV1) -> bool {
        &self.state.director == director
    }

    pub(crate) fn native_director(&self) -> Result<CompetitionDirectorStateV1, String> {
        if self.qualification_required()? {
            return Err("legacy migration requires durable qualification".into());
        }
        Ok(self.state.director.clone())
    }

    pub(crate) fn release_qualified(
        &self,
        permit: super::migration_qualification::QualificationPermitV1,
    ) -> Result<CompetitionDirectorStateV1, String> {
        self.state.clone().release(permit)
    }

    #[cfg(test)]
    pub(crate) fn commit_health_drift_for_test(&mut self) -> Result<(), String> {
        self.state.commit_health_drift_for_test()
    }
}

fn migrate_v0(
    legacy: LegacyDirectorStateV0,
    resume_at_ms: u64,
) -> Result<(CompetitionDirectorStateV1, BTreeMap<String, String>), String> {
    if legacy.schema != LEGACY_DIRECTOR_SCHEMA_V0
        || legacy.revision == 0
        || validate_id(&legacy.campaign_id, "invalid legacy campaign").is_err()
    {
        return Err("invalid legacy director identity".into());
    }
    let had_board = legacy.last_good_board.is_some();
    let mut board = legacy.last_good_board;
    if let Some(board) = &mut board {
        board.freshness = BoardFreshnessV1::Stale {
            since_ms: resume_at_ms,
            reason: "legacy-v0 migration requires source refresh".into(),
        };
    }
    let mut provenance = BTreeMap::new();
    let mut services = Vec::new();
    for pending in legacy.pending_actions {
        pending.intent.validate()?;
        super::schema_validation::validate_sha256(&pending.provenance_sha256)
            .map_err(str::to_string)?;
        if provenance
            .insert(pending.intent.intent_id.clone(), pending.provenance_sha256)
            .is_some()
        {
            return Err("duplicate legacy pending action provenance".into());
        }
        services.push(pending.intent);
    }
    let mut state = CompetitionDirectorStateV1 {
        schema: DIRECTOR_STATE_SCHEMA_V1.into(),
        revision: legacy.revision,
        campaign_id: legacy.campaign_id.clone(),
        board,
        candidates: legacy.candidates,
        submissions: legacy.submissions,
        rewards: legacy.rewards,
        patterns: legacy.patterns,
        episodes: legacy.episodes,
        workers: legacy.workers,
        services,
        health: migration_health(legacy.revision, had_board, resume_at_ms)?,
    };
    ensure_service(
        &mut state,
        DirectorServiceKindV1::RefreshBoard { full: true },
        resume_at_ms,
    )?;
    ensure_service(
        &mut state,
        DirectorServiceKindV1::ContinueContext,
        resume_at_ms,
    )?;
    state.validate()?;
    Ok((state, provenance))
}

fn ensure_service(
    state: &mut CompetitionDirectorStateV1,
    kind: DirectorServiceKindV1,
    due_at_ms: u64,
) -> Result<(), String> {
    let wanted = DirectorServiceIntentV1::new(&state.campaign_id, kind, due_at_ms)?;
    if !state.services.iter().any(|service| service == &wanted) {
        state.upsert_service(wanted);
    }
    Ok(())
}

fn migration_health(
    revision: u64,
    had_board: bool,
    at_ms: u64,
) -> Result<DirectorHealthV1, String> {
    let value = DirectorHealthV1 {
        schema: COMPETITION_SCHEMA_V1.into(),
        state: DirectorHealthStateV1::NeedsAttention,
        last_good_revision: had_board.then_some(revision),
        reason: Some(MIGRATION_QUALIFICATION_REQUIRED_REASON.into()),
        next: ScheduledActionV1 {
            action: "full_board_refresh".into(),
            next_attempt_at_ms: at_ms,
        },
        updated_at_ms: at_ms,
    };
    value.validate().map_err(str::to_string)?;
    Ok(value)
}
