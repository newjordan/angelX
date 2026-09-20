use super::journal::{
    ActionJournalEventV1, ActionJournalStateV1, ActionUpdateV1, JournalError, PrepareActionV1,
};
use super::lease_store;
use super::leases::{LeaseBookV1, LeaseError, WorkLeaseKeyV1};
use super::store_fs::{
    OpenKind, StoreLease, io_error, open_private, read_bounded, reject_symlink, secure_dir,
    secure_file, sync_dir,
};
use serde::Serialize;
use std::fs;
use std::io::Write;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_EVENT_BYTES: usize = 1024 * 1024;
const MAX_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
const MAX_LEASE_BYTES: u64 = 8 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub(crate) struct CompetitionStore {
    root: PathBuf,
    journal_path: PathBuf,
    snapshot_path: PathBuf,
    lease_path: PathBuf,
    lock_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppendReceiptV1 {
    pub(crate) appended: bool,
    pub(crate) event_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StoreRecoveryV1 {
    pub(crate) state: ActionJournalStateV1,
    pub(crate) torn_tail_discarded: bool,
}

pub(crate) type StoreError = String;

struct JournalRead {
    state: ActionJournalStateV1,
    complete_len: usize,
    file_len: usize,
}

impl CompetitionStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            journal_path: root.join("actions.jsonl"),
            snapshot_path: root.join("actions.snapshot.json"),
            lease_path: root.join("leases.snapshot.json"),
            lock_path: root.join("store.lock"),
            root,
        }
    }

    pub(crate) fn append_action(
        &self,
        _update: ActionUpdateV1,
    ) -> Result<AppendReceiptV1, StoreError> {
        Err("authoritative action append requires worker lease fencing".into())
    }

    pub(crate) fn append_action_fenced(
        &self,
        lease_key: &WorkLeaseKeyV1,
        generation: u64,
        update: ActionUpdateV1,
    ) -> Result<AppendReceiptV1, StoreError> {
        self.prepare_root()?;
        let _lease = self.acquire_lease()?;
        if lease_key.campaign_id != update.intent.campaign_id {
            return Err("action campaign does not match worker lease".into());
        }
        self.read_leases()?
            .accept_landing(lease_key, generation)
            .map_err(lease_error)?;
        self.append_action_locked(update)
    }

    fn append_action_locked(&self, update: ActionUpdateV1) -> Result<AppendReceiptV1, StoreError> {
        let mut journal = self.read_journal()?;
        self.discard_torn_tail(&journal)?;
        match journal.state.prepare(update).map_err(journal_error)? {
            PrepareActionV1::Replay { event_sha256 } => {
                self.save_snapshot(&journal.state)?;
                Ok(AppendReceiptV1 {
                    appended: false,
                    event_sha256,
                })
            }
            PrepareActionV1::Append(event) => {
                self.append_event(&event)?;
                // The event is authoritative. A snapshot failure is recoverable by
                // replaying the journal, and an exact caller retry will not reissue it.
                journal.state.apply(&event).map_err(journal_error)?;
                self.save_snapshot(&journal.state)?;
                Ok(AppendReceiptV1 {
                    appended: true,
                    event_sha256: event.event_sha256,
                })
            }
        }
    }

    pub(crate) fn update_leases<T>(
        &self,
        mutation: impl FnOnce(&mut LeaseBookV1) -> Result<T, LeaseError>,
    ) -> Result<T, StoreError> {
        self.prepare_root()?;
        let _lease = self.acquire_lease()?;
        let mut leases = self.read_leases()?;
        let result = mutation(&mut leases).map_err(lease_error)?;
        self.save_leases(&leases)?;
        Ok(result)
    }

    pub(crate) fn recover_leases(&self) -> Result<LeaseBookV1, StoreError> {
        self.prepare_root()?;
        let _lease = self.acquire_lease()?;
        self.read_leases()
    }

    pub(crate) fn recover(&self) -> Result<StoreRecoveryV1, StoreError> {
        self.prepare_root()?;
        let _lease = self.acquire_lease()?;
        let journal = self.read_journal()?;
        let torn_tail_discarded = self.discard_torn_tail(&journal)?;
        self.save_snapshot(&journal.state)?;
        Ok(StoreRecoveryV1 {
            state: journal.state,
            torn_tail_discarded,
        })
    }

    fn prepare_root(&self) -> Result<(), StoreError> {
        if self.root.exists() {
            reject_symlink(&self.root)?;
        }
        fs::create_dir_all(&self.root).map_err(|error| io_error("create", &self.root, error))?;
        secure_dir(&self.root)
    }

    fn acquire_lease(&self) -> Result<StoreLease, StoreError> {
        StoreLease::acquire(&self.lock_path)
    }

    fn read_journal(&self) -> Result<JournalRead, StoreError> {
        if !self.journal_path.exists() {
            return Ok(JournalRead {
                state: ActionJournalStateV1::default(),
                complete_len: 0,
                file_len: 0,
            });
        }
        reject_symlink(&self.journal_path)?;
        let raw = read_bounded(&self.journal_path, MAX_JOURNAL_BYTES)?;
        let complete_len = raw
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        let mut state = ActionJournalStateV1::default();
        for line in raw[..complete_len]
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            if line.len() > MAX_EVENT_BYTES {
                return Err("action journal event exceeds bound".into());
            }
            let event: ActionJournalEventV1 = serde_json::from_slice(line)
                .map_err(|error| format!("parse action journal: {error}"))?;
            state.apply(&event).map_err(journal_error)?;
        }
        Ok(JournalRead {
            state,
            complete_len,
            file_len: raw.len(),
        })
    }

    fn read_leases(&self) -> Result<LeaseBookV1, StoreError> {
        if !self.lease_path.exists() {
            return Ok(LeaseBookV1::default());
        }
        reject_symlink(&self.lease_path)?;
        let raw = read_bounded(&self.lease_path, MAX_LEASE_BYTES)?;
        lease_store::decode(&raw)
    }

    fn discard_torn_tail(&self, journal: &JournalRead) -> Result<bool, StoreError> {
        if journal.complete_len == journal.file_len {
            return Ok(false);
        }
        let file = open_private(&self.journal_path, OpenKind::WriteExisting)?;
        file.set_len(journal.complete_len as u64)
            .map_err(|error| io_error("truncate", &self.journal_path, error))?;
        file.sync_data()
            .map_err(|error| io_error("sync", &self.journal_path, error))?;
        Ok(true)
    }

    fn append_event(&self, event: &ActionJournalEventV1) -> Result<(), StoreError> {
        let mut line =
            serde_json::to_vec(event).map_err(|error| format!("encode action event: {error}"))?;
        line.push(b'\n');
        if line.len() > MAX_EVENT_BYTES {
            return Err("action journal event exceeds bound".into());
        }
        let current = fs::metadata(&self.journal_path)
            .map(|meta| meta.len())
            .unwrap_or(0);
        if current
            .checked_add(line.len() as u64)
            .is_none_or(|size| size > MAX_JOURNAL_BYTES)
        {
            return Err("action journal exceeds bound".into());
        }
        let mut file = open_private(&self.journal_path, OpenKind::Append)?;
        file.write_all(&line)
            .and_then(|_| file.sync_data())
            .map_err(|error| io_error("append", &self.journal_path, error))?;
        sync_dir(&self.root)
    }

    fn save_snapshot(&self, state: &ActionJournalStateV1) -> Result<(), StoreError> {
        #[derive(Serialize)]
        #[serde(deny_unknown_fields)]
        struct Snapshot<'a> {
            schema: &'static str,
            state: &'a ActionJournalStateV1,
        }
        let body = serde_json::to_vec_pretty(&Snapshot {
            schema: "angel.competition-action-snapshot/v1",
            state,
        })
        .map_err(|error| format!("encode action snapshot: {error}"))?;
        if body.len() > MAX_SNAPSHOT_BYTES {
            return Err("action snapshot exceeds bound".into());
        }
        self.save_atomic(&self.snapshot_path, "actions", &body)
    }

    fn save_leases(&self, leases: &LeaseBookV1) -> Result<(), StoreError> {
        let body = lease_store::encode(leases)?;
        if body.len() as u64 > MAX_LEASE_BYTES {
            return Err("lease snapshot exceeds bound".into());
        }
        self.save_atomic(&self.lease_path, "leases", &body)
    }

    fn save_atomic(&self, target: &PathBuf, tag: &str, body: &[u8]) -> Result<(), StoreError> {
        let tmp = self.root.join(format!(
            ".{tag}.snapshot.{}.{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = open_private(&tmp, OpenKind::CreateNew)?;
        if let Err(error) = file.write_all(body).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&tmp);
            return Err(io_error("write", &tmp, error));
        }
        fs::rename(&tmp, target).map_err(|error| {
            let _ = fs::remove_file(&tmp);
            io_error("publish", target, error)
        })?;
        secure_file(target)?;
        sync_dir(&self.root)
    }

    #[cfg(test)]
    pub(crate) fn journal_path(&self) -> &Path {
        &self.journal_path
    }
}

fn journal_error(error: JournalError) -> StoreError {
    error.to_string()
}

fn lease_error(error: LeaseError) -> StoreError {
    error.to_string()
}
