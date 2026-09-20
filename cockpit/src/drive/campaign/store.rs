use super::schema::{
    CampaignRecord, CampaignStatus, MAX_RECORD_BYTES, ProjectBinding, validate_campaign_id,
};
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static EVENT_LOCK: Mutex<()> = Mutex::new(());
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) enum LoadedCampaign {
    Missing,
    Active(Box<CampaignRecord>),
    Inert(String),
}

#[derive(Clone, Debug)]
pub(crate) struct CampaignStore {
    workspace: PathBuf,
    binding: ProjectBinding,
    root: PathBuf,
    project_dir: PathBuf,
    record_path: PathBuf,
    lock_path: PathBuf,
    events_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct CampaignLease {
    file: File,
}

impl Drop for CampaignLease {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: CampaignLease owns this descriptor until Drop returns.
            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct CampaignEvent<'a> {
    schema: &'static str,
    ts_ms: u64,
    campaign_id: &'a str,
    revision: u64,
    old_status: CampaignStatus,
    new_status: CampaignStatus,
    event: &'a str,
    detail: String,
}

impl CampaignStore {
    pub(crate) fn for_workspace(workspace: &Path) -> Self {
        let root = std::env::var_os("ANGEL_CAMPAIGN_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::platform::workspace_store::angel_subdir("campaigns"));
        Self::in_root(workspace, root)
    }

    pub(crate) fn in_root(workspace: &Path, root: PathBuf) -> Self {
        let binding = ProjectBinding::for_workspace(workspace);
        let project_dir = root.join(&binding.project_key);
        Self {
            workspace: workspace.to_path_buf(),
            binding,
            root,
            record_path: project_dir.join("campaign.json"),
            lock_path: project_dir.join("campaign.lock"),
            events_path: project_dir.join("events.jsonl"),
            project_dir,
        }
    }

    pub(crate) fn binding(&self) -> &ProjectBinding {
        &self.binding
    }

    #[cfg(test)]
    pub(crate) fn record_path(&self) -> &Path {
        &self.record_path
    }

    pub(crate) fn load(&self) -> LoadedCampaign {
        match self.load_strict() {
            Ok(Some(record)) => LoadedCampaign::Active(Box::new(record)),
            Ok(None) => LoadedCampaign::Missing,
            Err(error) => LoadedCampaign::Inert(error),
        }
    }

    fn load_strict(&self) -> Result<Option<CampaignRecord>, String> {
        if !self.record_path.exists() {
            return Ok(None);
        }
        check_private_dir(&self.root)?;
        check_private_dir(&self.project_dir)?;
        reject_symlink(&self.project_dir)?;
        reject_symlink(&self.record_path)?;
        check_private_file(&self.record_path)?;
        let mut file = File::open(&self.record_path)
            .map_err(|error| format!("read {}: {error}", self.record_path.display()))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("stat {}: {error}", self.record_path.display()))?;
        if !metadata.is_file() || metadata.len() > MAX_RECORD_BYTES {
            return Err("campaign record is not a bounded regular file".to_string());
        }
        let mut raw = Vec::with_capacity(metadata.len() as usize);
        Read::by_ref(&mut file)
            .take(MAX_RECORD_BYTES + 1)
            .read_to_end(&mut raw)
            .map_err(|error| format!("read {}: {error}", self.record_path.display()))?;
        if raw.len() as u64 > MAX_RECORD_BYTES {
            return Err("campaign record exceeds the 1 MiB safety cap".to_string());
        }
        let record: CampaignRecord = serde_json::from_slice(&raw)
            .map_err(|error| format!("parse {}: {error}", self.record_path.display()))?;
        record.validate_for(&self.workspace)?;
        if record.project != self.binding {
            return Err("campaign record project binding changed".to_string());
        }
        Ok(Some(record))
    }

    pub(crate) fn acquire_lease(&self) -> Result<CampaignLease, String> {
        secure_dir(&self.root)?;
        secure_dir(&self.project_dir)?;
        reject_symlink(&self.lock_path)?;
        let file = open_private(&self.lock_path, false)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: `file` owns a live descriptor for the lease lifetime.
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::WouldBlock {
                    return Err("campaign is already controlled by another process".to_string());
                }
                return Err(format!("lock {}: {error}", self.lock_path.display()));
            }
        }
        Ok(CampaignLease { file })
    }

    pub(crate) fn save(&self, record: &CampaignRecord) -> Result<(), String> {
        record.validate_for(&self.workspace)?;
        if record.project != self.binding {
            return Err("refusing to save a foreign campaign record".to_string());
        }
        if self.record_path.exists() {
            let existing = self
                .load_strict()?
                .ok_or_else(|| "campaign record disappeared during save".to_string())?;
            if existing.id != record.id {
                return Err("refusing to replace a different campaign".to_string());
            }
            if record.revision < existing.revision
                || (record.revision == existing.revision && record != &existing)
            {
                return Err("refusing a stale campaign record update".to_string());
            }
        }
        secure_dir(&self.root)?;
        secure_dir(&self.project_dir)?;
        reject_symlink(&self.record_path)?;
        let body = crate::platform::secrets::to_redacted_vec_pretty(record)
            .map_err(|error| format!("serialize campaign: {error}"))?;
        if body.len() as u64 > MAX_RECORD_BYTES {
            return Err("campaign record exceeds the 1 MiB safety cap".to_string());
        }
        let tmp = self.project_dir.join(format!(
            ".campaign.json.tmp.{}.{}.{}",
            std::process::id(),
            record.revision,
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        reject_symlink(&tmp)?;
        let mut file = open_private(&tmp, true)?;
        let write_result = file
            .write_all(&body)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("write {}: {error}", tmp.display()));
        if let Err(error) = write_result {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
        fs::rename(&tmp, &self.record_path).map_err(|error| {
            let _ = fs::remove_file(&tmp);
            format!("publish {}: {error}", self.record_path.display())
        })?;
        secure_file(&self.record_path)?;
        if let Ok(dir) = File::open(&self.project_dir) {
            dir.sync_all()
                .map_err(|error| format!("sync {}: {error}", self.project_dir.display()))?;
        }
        Ok(())
    }

    pub(crate) fn append_event(
        &self,
        record: &CampaignRecord,
        old_status: CampaignStatus,
        event: &str,
        detail: &str,
    ) -> Result<(), String> {
        validate_campaign_id(&record.id)?;
        if event.is_empty()
            || event.len() > 64
            || !event
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err("invalid campaign event name".to_string());
        }
        secure_dir(&self.root)?;
        secure_dir(&self.project_dir)?;
        reject_symlink(&self.events_path)?;
        let event = CampaignEvent {
            schema: "angel.campaign-event/v1",
            ts_ms: record.updated_ms,
            campaign_id: &record.id,
            revision: record.revision,
            old_status,
            new_status: record.status,
            event,
            detail: compact_scrubbed(detail, 2048),
        };
        let mut line = crate::platform::secrets::to_redacted_vec(&event)
            .map_err(|error| format!("encode campaign event: {error}"))?;
        line.push(b'\n');
        let _guard = EVENT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut file = open_private_append(&self.events_path)?;
        file.write_all(&line)
            .and_then(|_| file.sync_data())
            .map_err(|error| format!("append {}: {error}", self.events_path.display()))
    }

    /// Persist a bounded campaign-owned snapshot of the linked swarm proof.
    /// Campaign state references this immutable relative path and real SHA-256,
    /// never an unchecked model-supplied artifact path.
    pub(crate) fn save_proof_snapshot(
        &self,
        run_id: &str,
        receipt: &impl Serialize,
    ) -> Result<(PathBuf, String), String> {
        if !run_id.starts_with("swr-")
            || run_id.len() > 96
            || !run_id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
        {
            return Err("invalid swarm run id for campaign proof".to_string());
        }
        let body = crate::platform::secrets::to_redacted_vec_pretty(receipt)
            .map_err(|error| format!("serialize campaign proof: {error}"))?;
        if body.len() > 256 * 1024 {
            return Err("campaign proof snapshot exceeds 256 KiB".to_string());
        }
        secure_dir(&self.root)?;
        secure_dir(&self.project_dir)?;
        let proofs = self.project_dir.join("proofs");
        secure_dir(&proofs)?;
        let relative = PathBuf::from("proofs").join(format!("{run_id}.json"));
        let path = self.project_dir.join(&relative);
        reject_symlink(&path)?;
        if path.exists() {
            let existing =
                fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
            if existing != body {
                return Err("refusing to replace a different swarm proof snapshot".to_string());
            }
            return Ok((relative, crate::knowledge::cut::sha256_hex(&body)));
        }
        let tmp = proofs.join(format!(
            ".{run_id}.tmp.{}.{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = open_private(&tmp, true)?;
        let write_result = file
            .write_all(&body)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("write {}: {error}", tmp.display()));
        if let Err(error) = write_result {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
        fs::rename(&tmp, &path).map_err(|error| {
            let _ = fs::remove_file(&tmp);
            format!("publish {}: {error}", path.display())
        })?;
        secure_file(&path)?;
        if let Ok(dir) = File::open(&proofs) {
            dir.sync_all()
                .map_err(|error| format!("sync {}: {error}", proofs.display()))?;
        }
        Ok((relative, crate::knowledge::cut::sha256_hex(&body)))
    }

    /// Store one immutable alignment attempt separately from the technical
    /// swarm proof. The response digest makes retries append-only.
    pub(crate) fn save_alignment_snapshot(
        &self,
        run_id: &str,
        response_sha256: &str,
        receipt: &impl Serialize,
    ) -> Result<(PathBuf, String), String> {
        if !run_id.starts_with("swr-")
            || run_id.len() > 96
            || !run_id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
            || response_sha256.len() != 64
            || !response_sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err("invalid alignment proof identity".to_string());
        }
        let body = crate::platform::secrets::to_redacted_vec_pretty(receipt)
            .map_err(|error| format!("serialize alignment proof: {error}"))?;
        if body.len() > 256 * 1024 {
            return Err("alignment proof snapshot exceeds 256 KiB".to_string());
        }
        let snapshot_sha256 = crate::knowledge::cut::sha256_hex(&body);
        secure_dir(&self.root)?;
        secure_dir(&self.project_dir)?;
        let proofs = self.project_dir.join("proofs");
        secure_dir(&proofs)?;
        let suffix = &snapshot_sha256[..16];
        let relative = PathBuf::from("proofs").join(format!("{run_id}-alignment-{suffix}.json"));
        let path = self.project_dir.join(&relative);
        reject_symlink(&path)?;
        if path.exists() {
            let existing =
                fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
            if existing != body {
                return Err("refusing to replace a different alignment proof".to_string());
            }
            return Ok((relative, snapshot_sha256));
        }
        let tmp = proofs.join(format!(
            ".{run_id}-alignment-{suffix}.tmp.{}.{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = open_private(&tmp, true)?;
        let write_result = file
            .write_all(&body)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("write {}: {error}", tmp.display()));
        if let Err(error) = write_result {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
        fs::rename(&tmp, &path).map_err(|error| {
            let _ = fs::remove_file(&tmp);
            format!("publish {}: {error}", path.display())
        })?;
        secure_file(&path)?;
        if let Ok(dir) = File::open(&proofs) {
            dir.sync_all()
                .map_err(|error| format!("sync {}: {error}", proofs.display()))?;
        }
        Ok((relative, snapshot_sha256))
    }
}

fn compact_scrubbed(text: &str, max_chars: usize) -> String {
    let scrubbed = crate::knowledge::experience::scrub_secrets(text);
    if scrubbed.chars().count() <= max_chars {
        return scrubbed;
    }
    let mut out = scrubbed.chars().take(max_chars).collect::<String>();
    out.push_str(" …[truncated]");
    out
}

fn secure_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        reject_symlink(path)?;
    }
    fs::create_dir_all(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("secure {}: {error}", path.display()))?;
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing symlinked campaign path {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("inspect {}: {error}", path.display())),
    }
}

fn check_private_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(|error| format!("stat {}: {error}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "campaign record permissions are not private: {:o}",
                mode & 0o777
            ));
        }
    }
    Ok(())
}

fn check_private_dir(path: &Path) -> Result<(), String> {
    reject_symlink(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata =
            fs::metadata(path).map_err(|error| format!("stat {}: {error}", path.display()))?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(format!(
                "campaign directory permissions are not private: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn open_private(path: &Path, create_new: bool) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if create_new {
        options.create_new(true);
    } else {
        options.create(true).truncate(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    secure_file(path)?;
    Ok(file)
}

fn open_private_append(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    secure_file(path)?;
    Ok(file)
}

fn secure_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("secure {}: {error}", path.display()))?;
    }
    Ok(())
}
