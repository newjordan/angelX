//! Directory-wide JSONL rotation. A stable private flock serializes all writers
//! across processes; descriptor-relative operations preserve store confinement.
use serde_json::{Value, json};
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

pub(super) fn append(path: &Path, record: &Value, cap: u64, age: Duration) -> io::Result<()> {
    let (directory, name) = crate::platform::workspace_store::private_io::parent(path)?;
    let lock = directory.lock_file(std::ffi::OsStr::new(".rotation.lock"))?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: lock owns this live fd; closing releases the advisory lock.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let mut line = crate::platform::secrets::to_redacted_vec(record).map_err(io::Error::other)?;
    // Reserve space for the bounded rotation receipt before deleting anything.
    if (line.len() as u64).saturating_add(1024) > cap {
        return Err(io::Error::from(io::ErrorKind::FileTooLarge));
    }
    let now = SystemTime::now();
    let mut files = Vec::new();
    for entry in directory.names()? {
        if Path::new(&entry)
            .extension()
            .is_none_or(|ext| ext != "jsonl")
        {
            continue;
        }
        if let Some(file) = directory.existing(&entry)? {
            let metadata = file.metadata()?;
            files.push((metadata.modified()?, entry, metadata.len(), file));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let before: u64 = files.iter().map(|f| f.2).sum();
    let mut total = before;
    let mut removed = 0_u64;
    let mut aged_files = 0_u64;
    for (modified, entry, bytes, file) in files {
        let expired = now.duration_since(modified).unwrap_or_default() >= age;
        if expired || total.saturating_add(line.len() as u64 + 1024) > cap {
            directory.remove_owned(&entry, &file)?;
            total = total.saturating_sub(bytes);
            removed += 1;
            aged_files += u64::from(expired);
        }
    }
    let event = (removed > 0).then(|| {
        json!({
            "store":"trajectories", "reason":"size_or_age", "ts_ms":super::now_ms(),
            "removed_files":removed, "aged_files":aged_files, "removed_bytes":before-total,
            "before_bytes":before, "max_bytes":cap, "max_age_secs":age.as_secs()
        })
    });
    if let Some(event) = &event {
        let mut record = record.clone();
        if !record["store_rotations"].is_array() {
            record["store_rotations"] = json!([]);
        }
        record["store_rotations"]
            .as_array_mut()
            .unwrap()
            .push(event.clone());
        line = crate::platform::secrets::to_redacted_vec(&record).map_err(io::Error::other)?;
    }
    line.push(b'\n');
    if total.saturating_add(line.len() as u64) > cap {
        return Err(io::Error::from(io::ErrorKind::FileTooLarge));
    }
    directory.append(name)?.write_all(&line)?;
    if let Some(event) = event {
        super::TURN_LEDGER.with(|cell| cell.borrow_mut().store_rotations.push(event));
    }
    drop(lock);
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/trajectory__rotation__tests.rs"]
mod tests;
