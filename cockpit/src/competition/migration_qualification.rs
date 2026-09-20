use super::director::CompetitionDirectorStateV1;
use super::director_services::DirectorServiceKindV1;
use super::dossier::DossierV1;
use super::journal::{ActionJournalStateV1, canonical_action_key};
use super::migration::{DirectorMigrationV1, MigrationDispositionV1};
use super::schema::{
    ActionIntentV1, ActionKindV1, ActionPhaseV1, COMPETITION_CONTRACT_V1, CompetitionKeyV1,
    DirectorHealthStateV1,
};
use super::schema_validation::validate_sha256;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const MIGRATION_QUALIFICATION_SCHEMA_V1: &str =
    "angel.competition-migration-qualification/v1";
const MIGRATION_INTENT_VERSION_V1: &str = "competition-director-migration/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DirectorMigrationEnvelopeV1 {
    NativeV1,
    Qualified {
        receipt: Box<MigrationQualificationReceiptV1>,
    },
}

impl DirectorMigrationEnvelopeV1 {
    pub(crate) fn receipt(&self) -> Option<&MigrationQualificationReceiptV1> {
        match self {
            Self::NativeV1 => None,
            Self::Qualified { receipt } => Some(receipt),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MigrationQualificationReceiptV1 {
    pub(crate) schema: String,
    pub(crate) migration_id: String,
    pub(crate) migration_action_key: String,
    pub(crate) source_sha256: String,
    pub(crate) target_sha256: String,
    pub(crate) journal_head_sha256: String,
    pub(crate) dossier_revision: u64,
    pub(crate) pending_action_provenance: BTreeMap<String, String>,
    pub(crate) resumed_revision: u64,
    pub(crate) resumed_director_sha256: String,
    pub(crate) receipt_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QualifiedDirectorMigrationV1 {
    pub(crate) state: CompetitionDirectorStateV1,
    pub(crate) receipt: MigrationQualificationReceiptV1,
}

pub(crate) struct QualificationPermitV1(String);

impl QualificationPermitV1 {
    fn new(migration_id: &str) -> Self {
        Self(migration_id.into())
    }

    pub(super) fn migration_id(&self) -> &str {
        &self.0
    }
}

pub(crate) fn migration_action_intent(
    migration: &DirectorMigrationV1,
    competition: CompetitionKeyV1,
) -> Result<ActionIntentV1, String> {
    competition.validate().map_err(str::to_string)?;
    if migration
        .board()
        .as_ref()
        .is_some_and(|board| board.competition != competition)
    {
        return Err("migration action competition differs from last-good board".into());
    }
    let payload_sha256 = crate::cut::sha256_hex(
        serde_json::to_vec(&(
            COMPETITION_CONTRACT_V1,
            &migration.source_sha256,
            &migration.target_sha256,
            &migration.pending_action_provenance,
        ))
        .map_err(|error| format!("encode migration action payload: {error}"))?
        .as_slice(),
    );
    let mut intent = ActionIntentV1 {
        action_key: String::new(),
        campaign_id: migration.campaign_id().into(),
        competition,
        kind: ActionKindV1::MigrateState,
        subject_id: migration.migration_id.clone(),
        payload_sha256,
        intent_version: MIGRATION_INTENT_VERSION_V1.into(),
    };
    intent.action_key = canonical_action_key(&intent).map_err(|error| error.to_string())?;
    Ok(intent)
}

pub(crate) fn qualify_migration(
    migration: &DirectorMigrationV1,
    journal: &ActionJournalStateV1,
    dossier: &DossierV1,
    competition: CompetitionKeyV1,
    resume_at_ms: u64,
) -> Result<QualifiedDirectorMigrationV1, String> {
    if migration.disposition != MigrationDispositionV1::MigratedLegacyV0
        || !migration.qualification_required()?
    {
        return Err("migration does not require legacy persistence qualification".into());
    }
    let intent = migration_action_intent(migration, competition)?;
    let record = journal
        .actions
        .get(&intent.action_key)
        .ok_or_else(|| "durable migration action is absent".to_string())?;
    if record.update.intent != intent
        || record.update.phase != ActionPhaseV1::Completed
        || record.update.receipt_sha256.as_ref() != Some(&migration.target_sha256)
        || journal.campaign_id.as_deref() != Some(migration.campaign_id())
    {
        return Err("migration action is not durably completed".into());
    }
    dossier.validate()?;
    if dossier.journal_head_sha256 != journal.head_sha256
        || dossier
            .checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.journal_head_sha256 != journal.head_sha256)
    {
        return Err("migration dossier is not on the durable journal head".into());
    }
    let mut state =
        migration.release_qualified(QualificationPermitV1::new(&migration.migration_id))?;
    if !state.services.iter().any(|service| {
        matches!(
            service.kind,
            DirectorServiceKindV1::RefreshBoard { full: true }
        )
    }) {
        return Err("qualified migration lacks authoritative full refresh".into());
    }
    state.upsert_service(super::director_services::DirectorServiceIntentV1::new(
        &state.campaign_id,
        DirectorServiceKindV1::RefreshBoard { full: true },
        resume_at_ms,
    )?);
    let next = state.clone();
    state.commit(
        next,
        DirectorHealthStateV1::Retrying,
        Some("legacy persistence qualified; full board refresh remains required"),
        "full_board_refresh",
        resume_at_ms,
    )?;
    let resumed_director_sha256 = director_sha(&state)?;
    let mut receipt = MigrationQualificationReceiptV1 {
        schema: MIGRATION_QUALIFICATION_SCHEMA_V1.into(),
        migration_id: migration.migration_id.clone(),
        migration_action_key: intent.action_key,
        source_sha256: migration.source_sha256.clone(),
        target_sha256: migration.target_sha256.clone(),
        journal_head_sha256: journal.head_sha256.clone(),
        dossier_revision: dossier.dossier_revision,
        pending_action_provenance: migration.pending_action_provenance.clone(),
        resumed_revision: state.revision,
        resumed_director_sha256,
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = receipt.canonical_sha256()?;
    receipt.validate_for(&state, &journal.head_sha256, dossier.dossier_revision)?;
    Ok(QualifiedDirectorMigrationV1 { state, receipt })
}

impl MigrationQualificationReceiptV1 {
    pub(crate) fn validate_for(
        &self,
        director: &CompetitionDirectorStateV1,
        journal_head_sha256: &str,
        dossier_revision: u64,
    ) -> Result<(), String> {
        self.validate_stored(director, journal_head_sha256, dossier_revision)?;
        if self.resumed_revision != director.revision
            || self.resumed_director_sha256 != director_sha(director)?
        {
            return Err("migration qualification does not bind resumed director".into());
        }
        Ok(())
    }

    pub(crate) fn validate_stored(
        &self,
        director: &CompetitionDirectorStateV1,
        journal_head_sha256: &str,
        dossier_revision: u64,
    ) -> Result<(), String> {
        if self.schema != MIGRATION_QUALIFICATION_SCHEMA_V1
            || validate_sha256(&self.migration_id).is_err()
            || validate_sha256(&self.migration_action_key).is_err()
            || validate_sha256(&self.source_sha256).is_err()
            || validate_sha256(&self.target_sha256).is_err()
            || self.journal_head_sha256 != journal_head_sha256
            || self.dossier_revision != dossier_revision
            || self.resumed_revision == 0
            || director.revision < self.resumed_revision
            || validate_sha256(&self.resumed_director_sha256).is_err()
            || self.pending_action_provenance.iter().any(|(id, digest)| {
                validate_sha256(id).is_err() || validate_sha256(digest).is_err()
            })
            || self.receipt_sha256 != self.canonical_sha256()?
        {
            return Err("invalid migration qualification receipt".into());
        }
        Ok(())
    }

    fn canonical_sha256(&self) -> Result<String, String> {
        let mut value = self.clone();
        value.receipt_sha256.clear();
        serde_json::to_vec(&value)
            .map(|body| crate::cut::sha256_hex(&body))
            .map_err(|error| format!("encode migration qualification: {error}"))
    }
}

fn director_sha(director: &CompetitionDirectorStateV1) -> Result<String, String> {
    director.validate()?;
    serde_json::to_vec(director)
        .map(|body| crate::cut::sha256_hex(&body))
        .map_err(|error| format!("encode qualified director: {error}"))
}
