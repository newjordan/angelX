use super::candidate::{CandidateCatalogV1, CandidateV1, validate_candidate_catalog};
use super::schema_validation::{validate_id, validate_sha256};
use super::store_fs::{
    OpenKind, StoreLease, io_error, open_private, read_bounded, reject_symlink, secure_dir,
    secure_file, sync_dir,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const CANDIDATE_REPOSITORY_SCHEMA_V1: &str = "angel.competition-candidate-repository/v1";
const MAX_REPOSITORY_BYTES: u64 = 64 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CandidateEligibilityStatusV1 {
    Eligible,
    StaleBoard,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateEligibilityV1 {
    pub(crate) candidate_id: String,
    pub(crate) current_board_epoch: u64,
    pub(crate) current_board_decision_sha256: String,
    pub(crate) status: CandidateEligibilityStatusV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateRepositoryV1 {
    pub(crate) schema: String,
    pub(crate) revision: u64,
    pub(crate) current_board_epoch: u64,
    pub(crate) current_board_decision_sha256: String,
    pub(crate) catalog: CandidateCatalogV1,
    pub(crate) eligibility: BTreeMap<String, CandidateEligibilityV1>,
}

impl CandidateRepositoryV1 {
    pub(crate) fn new(board_epoch: u64, decision_sha256: String) -> Result<Self, String> {
        let value = Self {
            schema: CANDIDATE_REPOSITORY_SCHEMA_V1.into(),
            revision: 0,
            current_board_epoch: board_epoch,
            current_board_decision_sha256: decision_sha256,
            catalog: CandidateCatalogV1::new(),
            eligibility: BTreeMap::new(),
        };
        value.validate()?;
        Ok(value)
    }

    pub(crate) fn insert(&mut self, candidate: CandidateV1) -> Result<(), String> {
        let mut next = self.clone();
        if next.catalog.contains_key(&candidate.candidate_id) {
            return Err("candidate id already exists".into());
        }
        let id = candidate.candidate_id.clone();
        next.catalog.insert(id.clone(), candidate);
        next.eligibility
            .insert(id.clone(), next.eligibility_for(&id)?);
        next.bump_revision()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub(crate) fn set_current_board(
        &mut self,
        board_epoch: u64,
        decision_sha256: String,
    ) -> Result<(), String> {
        if board_epoch <= self.current_board_epoch {
            return Err("current board epoch must advance".into());
        }
        validate_sha256(&decision_sha256).map_err(str::to_string)?;
        let mut next = self.clone();
        next.current_board_epoch = board_epoch;
        next.current_board_decision_sha256 = decision_sha256;
        next.eligibility = next
            .catalog
            .keys()
            .map(|id| Ok((id.clone(), next.eligibility_for(id)?)))
            .collect::<Result<_, String>>()?;
        next.bump_revision()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub(crate) fn require_eligible(&self, candidate_id: &str) -> Result<&CandidateV1, String> {
        self.validate()?;
        match self.eligibility.get(candidate_id) {
            Some(record) if record.status == CandidateEligibilityStatusV1::Eligible => self
                .catalog
                .get(candidate_id)
                .ok_or_else(|| "eligible candidate is absent".into()),
            _ => Err("candidate is not eligible on the current board".into()),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != CANDIDATE_REPOSITORY_SCHEMA_V1 || self.current_board_epoch == 0 {
            return Err("invalid candidate repository schema or epoch".into());
        }
        validate_sha256(&self.current_board_decision_sha256).map_err(str::to_string)?;
        validate_candidate_catalog(&self.catalog)?;
        if self.catalog.len() != self.eligibility.len() {
            return Err("every candidate requires named current-board eligibility".into());
        }
        for id in self.catalog.keys() {
            validate_id(id, "invalid candidate repository key").map_err(str::to_string)?;
            if self.eligibility.get(id) != Some(&self.eligibility_for(id)?) {
                return Err("candidate eligibility is stale or misbound".into());
            }
        }
        Ok(())
    }

    fn eligibility_for(&self, id: &str) -> Result<CandidateEligibilityV1, String> {
        let candidate = self
            .catalog
            .get(id)
            .ok_or_else(|| "eligibility candidate is absent".to_string())?;
        let exact = candidate.board.board_epoch == self.current_board_epoch
            && candidate.board.board_decision_sha256 == self.current_board_decision_sha256;
        Ok(CandidateEligibilityV1 {
            candidate_id: id.into(),
            current_board_epoch: self.current_board_epoch,
            current_board_decision_sha256: self.current_board_decision_sha256.clone(),
            status: if exact {
                CandidateEligibilityStatusV1::Eligible
            } else {
                CandidateEligibilityStatusV1::StaleBoard
            },
        })
    }

    fn bump_revision(&mut self) -> Result<(), String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| "candidate repository revision exhausted".to_string())?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CandidateStoreV1 {
    root: PathBuf,
    snapshot_path: PathBuf,
    lock_path: PathBuf,
    #[cfg(test)]
    failure: Option<CandidatePublishFailureV1>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CandidatePublishFailureV1 {
    PreRename,
    PostRename,
}

impl CandidateStoreV1 {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            snapshot_path: root.join("candidates.snapshot.json"),
            lock_path: root.join("candidate-store.lock"),
            root,
            #[cfg(test)]
            failure: None,
        }
    }

    pub(crate) fn recover(&self) -> Result<Option<CandidateRepositoryV1>, String> {
        self.prepare_root()?;
        let _lease = StoreLease::acquire(&self.lock_path)?;
        self.read()
    }

    pub(crate) fn persist(&self, repository: &CandidateRepositoryV1) -> Result<(), String> {
        self.prepare_root()?;
        let _lease = StoreLease::acquire(&self.lock_path)?;
        repository.validate()?;
        let current = self.read()?;
        if current.as_ref() == Some(repository) {
            secure_file(&self.snapshot_path)?;
            sync_dir(&self.root)?;
            return Ok(());
        }
        let expected = current.as_ref().map_or(Ok(1), |value| {
            value
                .revision
                .checked_add(1)
                .ok_or_else(|| "candidate repository revision exhausted".to_string())
        })?;
        if repository.revision != expected {
            return Err("candidate repository revision is not append-ordered".into());
        }
        let body = serde_json::to_vec(repository)
            .map_err(|error| format!("encode candidate repository: {error}"))?;
        self.write_atomic(&body)
    }

    fn prepare_root(&self) -> Result<(), String> {
        if self.root.exists() {
            reject_symlink(&self.root)?;
        }
        fs::create_dir_all(&self.root).map_err(|error| io_error("create", &self.root, error))?;
        secure_dir(&self.root)
    }

    fn read(&self) -> Result<Option<CandidateRepositoryV1>, String> {
        if !self.snapshot_path.exists() {
            return Ok(None);
        }
        reject_symlink(&self.snapshot_path)?;
        let raw = read_bounded(&self.snapshot_path, MAX_REPOSITORY_BYTES)?;
        let value: CandidateRepositoryV1 = serde_json::from_slice(&raw)
            .map_err(|error| format!("parse candidate repository: {error}"))?;
        value.validate()?;
        Ok(Some(value))
    }

    fn write_atomic(&self, body: &[u8]) -> Result<(), String> {
        if body.len() as u64 > MAX_REPOSITORY_BYTES {
            return Err("candidate repository exceeds bound".into());
        }
        let tmp = self.root.join(format!(
            ".candidates.{}.{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = open_private(&tmp, OpenKind::CreateNew)?;
        file.write_all(body)
            .and_then(|_| file.sync_all())
            .map_err(|error| io_error("write", &tmp, error))?;
        #[cfg(test)]
        if self.failure == Some(CandidatePublishFailureV1::PreRename) {
            return Err("injected candidate pre-rename failure".into());
        }
        fs::rename(&tmp, &self.snapshot_path)
            .map_err(|error| io_error("publish", &self.snapshot_path, error))?;
        #[cfg(test)]
        if self.failure == Some(CandidatePublishFailureV1::PostRename) {
            return Err("injected candidate post-rename failure".into());
        }
        secure_file(&self.snapshot_path)?;
        sync_dir(&self.root)
    }

    #[cfg(test)]
    pub(crate) fn snapshot_path(&self) -> &Path {
        &self.snapshot_path
    }

    #[cfg(test)]
    pub(crate) fn inject_failure(&mut self, failure: CandidatePublishFailureV1) {
        self.failure = Some(failure);
    }
}
