use super::episode::{EpisodeEventKindV1, EpisodeEventV1};
use super::episode_reducer::{EpisodeDurabilityAuthorityV1, EpisodeStateV1};
use super::journal::ActionJournalEventV1;
use super::store::CompetitionStore;
use super::store_fs::{
    OpenKind, StoreLease, io_error, open_private, read_bounded, reject_symlink, secure_dir,
    sync_dir,
};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const MAX_EPISODE_EVENT_BYTES: usize = 1024 * 1024;
const MAX_EPISODE_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) struct DurableEpisodeJournalV1 {
    events: Vec<EpisodeEventV1>,
}

impl DurableEpisodeJournalV1 {
    pub(super) fn events(&self) -> &[EpisodeEventV1] {
        &self.events
    }
}

enum ActionAttestationV1 {
    Started(Box<ActionJournalEventV1>),
    Terminal { sequence: u64, head_sha256: String },
}

pub(crate) struct EpisodeStoreV1 {
    root: PathBuf,
    lock_path: PathBuf,
    action_store: CompetitionStore,
    pending_attestation: Option<ActionAttestationV1>,
}

struct EpisodeReadV1 {
    state: EpisodeStateV1,
    file_len: u64,
}

impl EpisodeStoreV1 {
    pub(crate) fn new(root: PathBuf, action_store: CompetitionStore) -> Self {
        Self {
            lock_path: root.join("episode-store.lock"),
            root,
            action_store,
            pending_attestation: None,
        }
    }

    pub(crate) fn recover_episode(&self, episode_id: &str) -> Result<EpisodeStateV1, String> {
        self.prepare_root()?;
        let _lease = StoreLease::acquire(&self.lock_path)?;
        Ok(self.read_episode(episode_id)?.state)
    }

    fn prepare_root(&self) -> Result<(), String> {
        if self.root.exists() {
            reject_symlink(&self.root)?;
        }
        fs::create_dir_all(&self.root).map_err(|error| io_error("create", &self.root, error))?;
        secure_dir(&self.root)
    }

    fn episode_path(&self, episode_id: &str) -> Result<PathBuf, String> {
        super::schema_validation::validate_id(episode_id, "invalid episode id")
            .map_err(str::to_string)?;
        Ok(self.root.join(format!(
            "episode-{}.jsonl",
            crate::knowledge::cut::sha256_hex(episode_id.as_bytes())
        )))
    }

    fn read_episode(&self, episode_id: &str) -> Result<EpisodeReadV1, String> {
        let path = self.episode_path(episode_id)?;
        if !path.exists() {
            return Ok(EpisodeReadV1 {
                state: EpisodeStateV1::default(),
                file_len: 0,
            });
        }
        reject_symlink(&path)?;
        let raw = read_bounded(&path, MAX_EPISODE_JOURNAL_BYTES)?;
        if !raw.is_empty() && !raw.ends_with(b"\n") {
            return Err("torn episode journal tail".into());
        }
        let mut events = Vec::new();
        let complete = raw.strip_suffix(b"\n").unwrap_or(&raw);
        if !raw.is_empty() && complete.is_empty() {
            return Err("blank episode journal event".into());
        }
        for line in complete.split(|byte| *byte == b'\n') {
            if line.is_empty() {
                return Err("blank episode journal event".into());
            }
            if line.len() > MAX_EPISODE_EVENT_BYTES {
                return Err("episode journal event exceeds bound".into());
            }
            let event: EpisodeEventV1 = serde_json::from_slice(line)
                .map_err(|error| format!("parse episode journal: {error}"))?;
            if event.episode_id != episode_id {
                return Err("episode journal identity does not match its durable key".into());
            }
            events.push(event);
        }
        let journal = DurableEpisodeJournalV1 { events };
        Ok(EpisodeReadV1 {
            state: EpisodeStateV1::recover_durable(&journal)?,
            file_len: raw.len() as u64,
        })
    }

    fn append_event(&self, event: &EpisodeEventV1) -> Result<(), String> {
        self.prepare_root()?;
        let _lease = StoreLease::acquire(&self.lock_path)?;
        let path = self.episode_path(&event.episode_id)?;
        let read = self.read_episode(&event.episode_id)?;
        read.state.validate_next(event)?;
        let mut line =
            serde_json::to_vec(event).map_err(|error| format!("encode episode event: {error}"))?;
        line.push(b'\n');
        if line.len() > MAX_EPISODE_EVENT_BYTES
            || read
                .file_len
                .checked_add(line.len() as u64)
                .is_none_or(|size| size > MAX_EPISODE_JOURNAL_BYTES)
        {
            return Err("episode journal exceeds bound".into());
        }
        let mut file = open_private(&path, OpenKind::Append)?;
        file.write_all(&line)
            .and_then(|_| file.sync_data())
            .map_err(|error| io_error("append", &path, error))?;
        sync_dir(&self.root)
    }

    #[cfg(test)]
    pub(crate) fn journal_path(&self, episode_id: &str) -> PathBuf {
        self.episode_path(episode_id)
            .expect("test episode id is valid")
    }
}

impl EpisodeDurabilityAuthorityV1 for EpisodeStoreV1 {
    fn verify_action_event_durable(&mut self, event: &ActionJournalEventV1) -> Result<(), String> {
        if self.pending_attestation.is_some() {
            return Err("episode action attestation is already pending".into());
        }
        let recovered = self.action_store.recover()?.state;
        let record = recovered
            .actions
            .get(&event.update.intent.action_key)
            .ok_or_else(|| "Started action is absent from durable journal".to_string())?;
        if record.update != event.update
            || record.last_seq != event.seq
            || record.last_event_sha256 != event.event_sha256
        {
            return Err("Started action event is not the durable canonical record".into());
        }
        self.pending_attestation = Some(ActionAttestationV1::Started(Box::new(event.clone())));
        Ok(())
    }

    fn verify_action_head_durable(
        &mut self,
        sequence: u64,
        head_sha256: &str,
    ) -> Result<(), String> {
        if self.pending_attestation.is_some() {
            return Err("episode action attestation is already pending".into());
        }
        let recovered = self.action_store.recover()?.state;
        let durable_sequence = recovered
            .next_seq
            .checked_sub(1)
            .ok_or_else(|| "durable action journal is empty".to_string())?;
        if durable_sequence != sequence || recovered.head_sha256 != head_sha256 {
            return Err("terminal action-journal head is not durable".into());
        }
        self.pending_attestation = Some(ActionAttestationV1::Terminal {
            sequence,
            head_sha256: head_sha256.to_string(),
        });
        Ok(())
    }

    fn append_and_sync(&mut self, event: &EpisodeEventV1) -> Result<(), String> {
        let attestation = self.pending_attestation.take();
        let attested = match (&event.event, attestation) {
            (EpisodeEventKindV1::Started(start), Some(ActionAttestationV1::Started(action))) => {
                *action == start.start_journal_event
            }
            (
                EpisodeEventKindV1::Terminal(terminal),
                Some(ActionAttestationV1::Terminal {
                    sequence,
                    head_sha256,
                }),
            ) => {
                sequence == terminal.action_journal_end_sequence
                    && head_sha256 == terminal.action_journal_head_sha256
            }
            (EpisodeEventKindV1::EvidenceLinked(_), None) => true,
            _ => false,
        };
        if !attested {
            return Err("episode event lacks matching durable action attestation".into());
        }
        self.append_event(event)
    }
}
