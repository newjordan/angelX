use super::director::CompetitionDirectorStateV1;
use super::migration::DirectorMigrationV1;
use super::migration_qualification::{
    DirectorMigrationEnvelopeV1, MigrationQualificationReceiptV1,
};
use super::scheduler::SchedulerStateV1;
use super::schema::COMPETITION_CONTRACT_V1;
use super::schema_validation::validate_sha256;
use super::store_fs::{
    OpenKind, StoreLease, io_error, open_private, read_bounded, reject_symlink, secure_dir,
    secure_file, sync_dir,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const DIRECTOR_SNAPSHOT_SCHEMA_V1: &str = "angel.competition-director-snapshot/v1";
const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectorSnapshotV1 {
    pub(crate) schema: String,
    pub(crate) contract: String,
    pub(crate) director: CompetitionDirectorStateV1,
    pub(crate) scheduler: SchedulerStateV1,
    pub(crate) action_journal_head_sha256: String,
    pub(crate) dossier_revision: u64,
    pub(crate) migration: DirectorMigrationEnvelopeV1,
    pub(crate) snapshot_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectorPublishDispositionV1 {
    Published,
    Replayed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectorCommitCutpointV1 {
    BeforeRename,
    AfterRenameAmbiguous,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DirectorStoreErrorV1 {
    Invalid(String),
    Commit {
        cutpoint: DirectorCommitCutpointV1,
        detail: String,
    },
}

impl fmt::Display for DirectorStoreErrorV1 {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(detail) => output.write_str(detail),
            Self::Commit { cutpoint, detail } => write!(output, "{cutpoint:?}: {detail}"),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DirectorStoreV1 {
    root: PathBuf,
    snapshot_path: PathBuf,
    lock_path: PathBuf,
    #[cfg(test)]
    failure: Option<DirectorCommitCutpointV1>,
    #[cfg(test)]
    fail_directory_sync: bool,
}

impl DirectorSnapshotV1 {
    pub(crate) fn new(
        director: CompetitionDirectorStateV1,
        scheduler: SchedulerStateV1,
        action_journal_head_sha256: String,
        dossier_revision: u64,
    ) -> Result<Self, String> {
        Self::new_with_migration(
            director,
            scheduler,
            action_journal_head_sha256,
            dossier_revision,
            None,
        )
    }

    pub(crate) fn new_migrated(
        migration: &DirectorMigrationV1,
        scheduler: SchedulerStateV1,
        action_journal_head_sha256: String,
        dossier_revision: u64,
    ) -> Result<Self, String> {
        Self::new(
            migration.native_director()?,
            scheduler,
            action_journal_head_sha256,
            dossier_revision,
        )
    }

    pub(crate) fn new_with_migration(
        director: CompetitionDirectorStateV1,
        scheduler: SchedulerStateV1,
        action_journal_head_sha256: String,
        dossier_revision: u64,
        migration: Option<MigrationQualificationReceiptV1>,
    ) -> Result<Self, String> {
        let migration = migration.map_or(DirectorMigrationEnvelopeV1::NativeV1, |receipt| {
            DirectorMigrationEnvelopeV1::Qualified {
                receipt: Box::new(receipt),
            }
        });
        let mut value = Self {
            schema: DIRECTOR_SNAPSHOT_SCHEMA_V1.into(),
            contract: COMPETITION_CONTRACT_V1.into(),
            director,
            scheduler,
            action_journal_head_sha256,
            dossier_revision,
            migration,
            snapshot_sha256: String::new(),
        };
        value.snapshot_sha256 = value.canonical_sha256()?;
        value.validate()?;
        Ok(value)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != DIRECTOR_SNAPSHOT_SCHEMA_V1
            || self.contract != COMPETITION_CONTRACT_V1
            || validate_sha256(&self.action_journal_head_sha256).is_err()
            || validate_sha256(&self.snapshot_sha256).is_err()
            || self.snapshot_sha256 != self.canonical_sha256()?
        {
            return Err("invalid director snapshot identity or digest".into());
        }
        self.director.validate()?;
        self.scheduler.validate()?;
        if let DirectorMigrationEnvelopeV1::Qualified { receipt } = &self.migration {
            receipt.validate_stored(
                &self.director,
                &self.action_journal_head_sha256,
                self.dossier_revision,
            )?;
        }
        match (&self.director.board, self.scheduler.last_board_revision) {
            (None, None) => Ok(()),
            (Some(board), Some(revision)) if revision == board.observation_revision => Ok(()),
            _ => Err("director and scheduler board revisions disagree".into()),
        }
    }

    fn canonical_sha256(&self) -> Result<String, String> {
        let mut value = self.clone();
        value.snapshot_sha256.clear();
        serde_json::to_vec(&value)
            .map(|body| crate::knowledge::cut::sha256_hex(&body))
            .map_err(|error| format!("encode director snapshot digest: {error}"))
    }
}

impl DirectorStoreV1 {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            snapshot_path: root.join("director.snapshot.json"),
            lock_path: root.join("director-store.lock"),
            root,
            #[cfg(test)]
            failure: None,
            #[cfg(test)]
            fail_directory_sync: false,
        }
    }

    pub(crate) fn recover(&self) -> Result<Option<DirectorSnapshotV1>, DirectorStoreErrorV1> {
        self.prepare_root()?;
        let _lease = StoreLease::acquire(&self.lock_path).map_err(invalid)?;
        self.read()
    }

    pub(crate) fn persist(
        &self,
        snapshot: &DirectorSnapshotV1,
    ) -> Result<DirectorPublishDispositionV1, DirectorStoreErrorV1> {
        self.prepare_root()?;
        let _lease = StoreLease::acquire(&self.lock_path).map_err(invalid)?;
        snapshot.validate().map_err(invalid)?;
        if let Some(current) = self.read()? {
            if current == *snapshot {
                secure_file(&self.snapshot_path).map_err(invalid)?;
                sync_dir(&self.root).map_err(invalid)?;
                return Ok(DirectorPublishDispositionV1::Replayed);
            }
            if current
                .director
                .revision
                .checked_add(1)
                .is_none_or(|revision| revision != snapshot.director.revision)
            {
                return Err(invalid("director snapshot revision is not append-ordered"));
            }
        }
        self.write_atomic(snapshot)?;
        Ok(DirectorPublishDispositionV1::Published)
    }

    fn prepare_root(&self) -> Result<(), DirectorStoreErrorV1> {
        if self.root.exists() {
            reject_symlink(&self.root).map_err(invalid)?;
        }
        fs::create_dir_all(&self.root)
            .map_err(|error| invalid(io_error("create", &self.root, error)))?;
        secure_dir(&self.root).map_err(invalid)
    }

    fn read(&self) -> Result<Option<DirectorSnapshotV1>, DirectorStoreErrorV1> {
        if !self.snapshot_path.exists() {
            return Ok(None);
        }
        reject_symlink(&self.snapshot_path).map_err(invalid)?;
        let raw = read_bounded(&self.snapshot_path, MAX_SNAPSHOT_BYTES).map_err(invalid)?;
        let value: DirectorSnapshotV1 = serde_json::from_slice(&raw)
            .map_err(|error| invalid(format!("parse director snapshot: {error}")))?;
        value.validate().map_err(invalid)?;
        Ok(Some(value))
    }

    fn write_atomic(&self, value: &DirectorSnapshotV1) -> Result<(), DirectorStoreErrorV1> {
        let body = serde_json::to_vec(value)
            .map_err(|error| invalid(format!("encode director snapshot: {error}")))?;
        if body.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err(invalid("director snapshot exceeds bound"));
        }
        let tmp = self.root.join(format!(
            ".director.{}.{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = open_private(&tmp, OpenKind::CreateNew).map_err(invalid)?;
        file.write_all(&body)
            .and_then(|_| file.sync_all())
            .map_err(|error| invalid(io_error("write", &tmp, error)))?;
        #[cfg(test)]
        if self.failure == Some(DirectorCommitCutpointV1::BeforeRename) {
            return Err(DirectorStoreErrorV1::Commit {
                cutpoint: DirectorCommitCutpointV1::BeforeRename,
                detail: "injected before director rename".into(),
            });
        }
        fs::rename(&tmp, &self.snapshot_path).map_err(|error| DirectorStoreErrorV1::Commit {
            cutpoint: DirectorCommitCutpointV1::BeforeRename,
            detail: io_error("publish", &self.snapshot_path, error),
        })?;
        #[cfg(test)]
        if self.failure == Some(DirectorCommitCutpointV1::AfterRenameAmbiguous) {
            return Err(DirectorStoreErrorV1::Commit {
                cutpoint: DirectorCommitCutpointV1::AfterRenameAmbiguous,
                detail: "injected after director rename".into(),
            });
        }
        secure_file(&self.snapshot_path).map_err(after_rename)?;
        #[cfg(test)]
        if self.fail_directory_sync {
            return Err(after_rename("injected director directory sync failure"));
        }
        sync_dir(&self.root).map_err(after_rename)
    }

    #[cfg(test)]
    pub(crate) fn inject_failure(&mut self, failure: DirectorCommitCutpointV1) {
        self.failure = Some(failure);
    }

    #[cfg(test)]
    pub(crate) fn inject_directory_sync_failure(&mut self) {
        self.fail_directory_sync = true;
    }

    #[cfg(test)]
    pub(crate) fn snapshot_path(&self) -> &std::path::Path {
        &self.snapshot_path
    }
}

fn invalid(detail: impl ToString) -> DirectorStoreErrorV1 {
    DirectorStoreErrorV1::Invalid(detail.to_string())
}

fn after_rename(detail: impl ToString) -> DirectorStoreErrorV1 {
    DirectorStoreErrorV1::Commit {
        cutpoint: DirectorCommitCutpointV1::AfterRenameAmbiguous,
        detail: detail.to_string(),
    }
}
