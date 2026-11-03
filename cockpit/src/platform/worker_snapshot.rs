//! Private, request-owned copies of a worker's declared source files.
//!
//! Two matching bounded reads catch concurrent edits.

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_MODULE: u64 = 1024 * 1024;
const MAX_GENERATION: usize = 8 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) struct Snapshot {
    pub(crate) dir: PathBuf,
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Snapshot {
    /// Copy all declared relative paths, then own the private directory until
    /// this snapshot is dropped. Failures never leave a partial snapshot live.
    pub(crate) fn capture(root: &Path, files: &[&str], label: &str) -> Result<Self, String> {
        if label.is_empty() || !label.bytes().all(|c| c.is_ascii_lowercase() || c == b'-') {
            return Err("invalid worker snapshot label".into());
        }
        let mut seen = std::collections::HashSet::new();
        for &file in files {
            let path = Path::new(file);
            if file.is_empty()
                || file.contains('\\')
                || file
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
                || path
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                || !seen.insert(file)
            {
                return Err(format!("invalid {label} module path"));
            }
        }
        if files.is_empty() || files.len() > 128 {
            return Err(format!("{label} module has an invalid file count"));
        }
        let read_all = || {
            let mut total = 0usize;
            let mut result = Vec::with_capacity(files.len());
            for path in files {
                let bytes = read(&root.join(path), label)?;
                total = total.saturating_add(bytes.len());
                if total > MAX_GENERATION {
                    return Err(format!("{label} module generation exceeds its byte limit"));
                }
                result.push(bytes);
            }
            Ok(result)
        };
        let first = read_all()?;
        if first != read_all()? {
            return Err(format!(
                "{label} module changed while preparing its generation"
            ));
        }
        let mut created = None;
        for _ in 0..128 {
            let name = format!(
                "angel-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let dir = std::env::temp_dir().join(name);
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&dir) {
                Ok(()) => {
                    created = Some(dir);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("{label} snapshot: {error}")),
            }
        }
        let snapshot = Self {
            dir: created.ok_or_else(|| format!("could not allocate {label} snapshot"))?,
        };
        for (path, bytes) in files.iter().zip(first) {
            let target = snapshot.dir.join(path);
            std::fs::create_dir_all(target.parent().ok_or("invalid snapshot path")?)
                .map_err(|error| error.to_string())?;
            std::fs::write(target, bytes).map_err(|error| format!("{label} snapshot: {error}"))?;
        }
        Ok(snapshot)
    }
}

pub(crate) fn read(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("{label} module {}: {error}", path.display()))?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err(format!(
            "{label} module {} is not a regular file",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_MODULE + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_MODULE {
        return Err(format!("{label} module exceeds its byte limit"));
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/worker_snapshot__tests.rs"]
mod tests;
