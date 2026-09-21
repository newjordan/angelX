//! Bounded read-only projection of external experiment observations.
//! A valid chain proves file continuity, NOT evaluator authority or a reward.
use super::{Entry, Place, State};
use serde::Deserialize;
use std::collections::{HashMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::SystemTime;

const MAX_FILE: u64 = 8 * 1024 * 1024;
const READ_BUDGET: u64 = 64 * 1024;
const MAX_LINE: usize = 16 * 1024;
const MAX_RECORDS: usize = 256;
const MAX_LINKS: usize = 16;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    sequence: u64,
    previous_sha256: String,
    payload_json: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Hypothesis,
    Worker,
    Candidate,
    Measurement,
    Decision,
    Submission,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Pending,
    Running,
    Completed,
    Failed,
    Inconclusive,
    Recorded,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    observed_unix_ms: Option<u64>,
    measurement: Option<super::measurement::Measurement>,
    project_root: String,
    id: String,
    experiment_id: String,
    kind: Kind,
    status: Status,
    title: String,
    summary: String,
    detail: String,
    links: Vec<String>,
    timestamp: String,
}

#[derive(Default)]
pub(crate) struct Journal {
    offset: u64,
    sequence: u64,
    previous: String,
    pending: Vec<u8>,
    records: VecDeque<Entry>,
    stamp: Option<(u64, Option<SystemTime>, u64)>,
    pub(crate) issue: Option<&'static str>,
    pub(crate) omitted: usize,
}

impl Journal {
    pub(crate) fn refresh(&mut self, workspace: &Path) -> Vec<Entry> {
        if let Err(issue) = self.read(workspace) {
            self.issue = Some(issue);
        }
        let mut entries: Vec<_> = self.records.iter().cloned().collect();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        for entry in &mut entries {
            if entry.state == State::Running
                && (self.issue.is_some()
                    || entry
                        .observed_unix_ms
                        .is_none_or(|at| now.saturating_sub(at) > 120_000 || at > now + 30_000))
            {
                entry.state = State::Inconclusive;
                entry.summary = format!(
                    "Heartbeat unavailable/stale · last reported: {}",
                    entry.summary
                );
            }
        }
        // Consumers are navigable in both directions. Only explicit links count.
        let mut consumers: HashMap<&str, Vec<usize>> = HashMap::with_capacity(self.records.len());
        for (idx, record) in self.records.iter().enumerate() {
            for (id, _) in &record.links {
                let list = consumers.entry(id.as_str()).or_default();
                if list.len() == MAX_LINKS || list.last() == Some(&idx) {
                    continue;
                }
                list.push(idx);
            }
        }
        for entry in &mut entries {
            if let Some(indexes) = consumers.get(entry.id.as_str()) {
                entry.links.extend(indexes.iter().map(|&idx| {
                    let record = &self.records[idx];
                    (record.id.clone(), format!("Consumer · {}", record.title))
                }));
            }
        }
        entries
    }

    fn read(&mut self, workspace: &Path) -> Result<(), &'static str> {
        let root = fs::canonicalize(workspace).map_err(|_| "research workspace unavailable")?;
        let dir = root.join(".angelX/research");
        for path in [root.join(".angelX"), dir.clone()] {
            match fs::symlink_metadata(path) {
                Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    *self = Self::default();
                    return Ok(());
                }
                _ => return Err("research journal directory is not a regular directory"),
            }
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let mut file = match options.open(dir.join("experiments.jsonl")) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                *self = Self::default();
                return Ok(());
            }
            Err(_) => return Err("research journal cannot be opened safely"),
        };
        let meta = file
            .metadata()
            .map_err(|_| "research journal metadata unavailable")?;
        if !meta.is_file() || meta.len() > MAX_FILE {
            return Err("research journal size/type limit");
        }
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            meta.ino()
        };
        #[cfg(not(unix))]
        let inode = 0;
        let stamp = (meta.len(), meta.modified().ok(), inode);
        if let Some(old) = self.stamp {
            if old.2 != inode
                || meta.len() < self.offset
                || (old.0 == meta.len() && old.1 != stamp.1)
            {
                *self = Self::default();
            } else if old == stamp && (self.issue.is_some() || self.offset == meta.len()) {
                return Ok(());
            }
        }
        self.stamp = Some(stamp);
        if self.issue.is_some() {
            return Ok(());
        } // Broken chains require replacement/recovery.
        file.seek(SeekFrom::Start(self.offset))
            .map_err(|_| "research journal seek failed")?;
        let mut bytes = Vec::new();
        file.take(READ_BUDGET)
            .read_to_end(&mut bytes)
            .map_err(|_| "research journal read failed")?;
        self.offset += bytes.len() as u64;
        self.pending.extend(bytes);
        let mut pending = std::mem::take(&mut self.pending);
        let mut start = 0;
        while let Some(rel) = pending[start..].iter().position(|b| *b == b'\n') {
            if rel > MAX_LINE {
                pending.copy_within(start.., 0);
                pending.truncate(pending.len() - start);
                self.pending = pending;
                return Err("research event exceeds line limit");
            }
            let end = start + rel;
            let accepted = self.accept(&pending[start..end], &root);
            start = end + 1;
            if let Err(issue) = accepted {
                pending.copy_within(start.., 0);
                pending.truncate(pending.len() - start);
                self.pending = pending;
                return Err(issue);
            }
        }
        if start != 0 {
            pending.copy_within(start.., 0);
            pending.truncate(pending.len() - start);
        }
        self.pending = pending;
        if self.pending.len() > MAX_LINE {
            return Err("research event exceeds line limit");
        }
        Ok(()) // A final partial line waits for the writer; it is never projected.
    }

    fn accept(&mut self, line: &[u8], root: &Path) -> Result<(), &'static str> {
        let envelope: Envelope =
            serde_json::from_slice(line).map_err(|_| "invalid research envelope")?;
        let previous = if self.sequence == 0 {
            "0".repeat(64)
        } else {
            self.previous.clone()
        };
        if envelope.schema != "angel.research-event/v1"
            || envelope.sequence != self.sequence + 1
            || envelope.previous_sha256 != previous
            || envelope.sha256
                != crate::knowledge::cut::sha256_hex(
                    format!(
                        "{}\n{}\n{}",
                        envelope.sequence, previous, envelope.payload_json
                    )
                    .as_bytes(),
                )
        {
            return Err("research journal continuity check failed");
        }
        let item: Observation = serde_json::from_str(&envelope.payload_json)
            .map_err(|_| "invalid research observation")?;
        if Path::new(&item.project_root) != root {
            return Err("research observation belongs to another workspace");
        }
        let valid_id = |id: &str| {
            !id.is_empty()
                && id.len() <= 160
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_:./".contains(&b))
        };
        if !valid_id(&item.id)
            || !valid_id(&item.experiment_id)
            || item.links.len() > MAX_LINKS
            || item.links.iter().any(|id| !valid_id(id))
        {
            return Err("invalid research identity/link");
        }
        let place = match item.kind {
            Kind::Hypothesis | Kind::Measurement => Place::Observatory,
            Kind::Worker => Place::Council,
            Kind::Candidate => Place::Smithy,
            Kind::Decision | Kind::Submission => Place::Library,
        };
        let state = match item.status {
            Status::Pending => State::Pending,
            Status::Running => State::Running,
            Status::Failed => State::Failed,
            Status::Inconclusive => State::Inconclusive,
            // An external report of passing is not a native verification receipt.
            Status::Completed | Status::Recorded => State::Recorded,
        };
        let mut entry = Entry::new(
            format!("experiment:{}", item.id),
            place,
            &item.title,
            state,
            &item.summary,
            &format!(
                "{}\n\nObserved at: {}\nJournal event: {}\n\nExternal research observation; not a native verification or reward binding.",
                item.detail, item.timestamp, envelope.sha256
            ),
            "experiment receipt journal",
        );
        if let Some(sample) = &item.measurement {
            sample.validate()?;
        }
        entry.measurement = item.measurement;
        entry.observed_unix_ms = item.observed_unix_ms;
        entry.experiment_id = Some(item.experiment_id.clone());
        entry.parent = Some(item.experiment_id);
        entry.links = item
            .links
            .iter()
            .map(|id| (format!("experiment:{id}"), format!("Dependency · {id}")))
            .collect();
        if let Some(existing) = self.records.iter_mut().find(|e| e.id == entry.id) {
            *existing = entry;
        } else {
            if self.records.len() == MAX_RECORDS {
                self.records.pop_front();
                self.omitted += 1;
            }
            self.records.push_back(entry);
        }
        self.sequence = envelope.sequence;
        self.previous = envelope.sha256;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/research_workspace__journal__tests.rs"]
mod tests;
