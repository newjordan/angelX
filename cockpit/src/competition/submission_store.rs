use super::store_fs::{
    OpenKind, StoreLease, io_error, open_private, read_bounded, reject_symlink, secure_dir,
    secure_file, sync_dir,
};
use super::submission::SubmissionSpoolV1;
use std::fs;
use std::io::Write;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_SPOOL_BYTES: u64 = 64 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub(crate) struct SubmissionStoreV1 {
    root: PathBuf,
    snapshot_path: PathBuf,
    lock_path: PathBuf,
    #[cfg(test)]
    failure: Option<SubmissionPublishFailureV1>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubmissionPublishFailureV1 {
    PreRename,
    PostRename,
}

impl SubmissionStoreV1 {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            snapshot_path: root.join("submissions.snapshot.json"),
            lock_path: root.join("submission-store.lock"),
            root,
            #[cfg(test)]
            failure: None,
        }
    }

    pub(crate) fn recover(&self) -> Result<Option<SubmissionSpoolV1>, String> {
        self.prepare_root()?;
        let _lease = StoreLease::acquire(&self.lock_path)?;
        self.read()
    }

    pub(crate) fn persist(&self, spool: &SubmissionSpoolV1) -> Result<(), String> {
        self.prepare_root()?;
        let _lease = StoreLease::acquire(&self.lock_path)?;
        spool.validate()?;
        let current = self.read()?;
        if current.as_ref() == Some(spool) {
            secure_file(&self.snapshot_path)?;
            sync_dir(&self.root)?;
            return Ok(());
        }
        let expected = current.as_ref().map_or(Ok(1), |value| {
            value
                .revision
                .checked_add(1)
                .ok_or_else(|| "submission spool revision exhausted".to_string())
        })?;
        if spool.revision != expected {
            return Err("submission spool revision is not append-ordered".into());
        }
        let body = serde_json::to_vec(spool)
            .map_err(|error| format!("encode submission spool: {error}"))?;
        self.write_atomic(&body)
    }

    fn prepare_root(&self) -> Result<(), String> {
        if self.root.exists() {
            reject_symlink(&self.root)?;
        }
        fs::create_dir_all(&self.root).map_err(|error| io_error("create", &self.root, error))?;
        secure_dir(&self.root)
    }

    fn read(&self) -> Result<Option<SubmissionSpoolV1>, String> {
        if !self.snapshot_path.exists() {
            return Ok(None);
        }
        reject_symlink(&self.snapshot_path)?;
        let raw = read_bounded(&self.snapshot_path, MAX_SPOOL_BYTES)?;
        let value: SubmissionSpoolV1 = serde_json::from_slice(&raw)
            .map_err(|error| format!("parse submission spool: {error}"))?;
        value.validate()?;
        Ok(Some(value))
    }

    fn write_atomic(&self, body: &[u8]) -> Result<(), String> {
        if body.len() as u64 > MAX_SPOOL_BYTES {
            return Err("submission spool exceeds bound".into());
        }
        let tmp = self.root.join(format!(
            ".submissions.{}.{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = open_private(&tmp, OpenKind::CreateNew)?;
        file.write_all(body)
            .and_then(|_| file.sync_all())
            .map_err(|error| io_error("write", &tmp, error))?;
        #[cfg(test)]
        if self.failure == Some(SubmissionPublishFailureV1::PreRename) {
            return Err("injected submission pre-rename failure".into());
        }
        fs::rename(&tmp, &self.snapshot_path)
            .map_err(|error| io_error("publish", &self.snapshot_path, error))?;
        #[cfg(test)]
        if self.failure == Some(SubmissionPublishFailureV1::PostRename) {
            return Err("injected submission post-rename failure".into());
        }
        secure_file(&self.snapshot_path)?;
        sync_dir(&self.root)
    }

    #[cfg(test)]
    pub(crate) fn snapshot_path(&self) -> &Path {
        &self.snapshot_path
    }

    #[cfg(test)]
    pub(crate) fn inject_failure(&mut self, failure: SubmissionPublishFailureV1) {
        self.failure = Some(failure);
    }
}
