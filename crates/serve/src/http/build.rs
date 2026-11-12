//! Build-time preparation for compiled browser bytes. The same Body code is
//! used here and at runtime, so content URLs, ETags and gzip remain identical.
use super::preparation::{MAX_FILE_BYTES, MAX_FILES, MAX_TOTAL_BYTES};
use super::{Body, mime_for};
use crate::hash::{hex, sha1};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

#[derive(PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: std::time::SystemTime,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}
fn stamp(metadata: &fs::Metadata) -> io::Result<Stamp> {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Ok(Stamp {
        len: metadata.len(),
        modified: metadata.modified()?,
        #[cfg(unix)]
        identity: (
            metadata.dev(),
            metadata.ino(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ),
    })
}

/// Capture original files into content-named immutable build outputs and return
/// their paths plus Rust defining PREPARED_FILES. The caller still watches its
/// original sources. Compiler includes refer only to captured output bytes.
pub fn capture_static(files: &[(String, PathBuf)], out: &Path) -> io::Result<StaticBuild> {
    capture_with_hook(files, out, |_| {})
}
fn capture_with_hook(
    files: &[(String, PathBuf)],
    out: &Path,
    mut after_read: impl FnMut(&Path),
) -> io::Result<StaticBuild> {
    if files.len() > MAX_FILES {
        return Err(io::Error::other("embedded browser file limit"));
    }
    fs::create_dir_all(out)?;
    // Exclusive Cargo output directory: one published build plus its proposed
    // replacement. Failed proposals remove only the files they created.
    let mut entries = 0usize;
    let mut resident = 0u64;
    for entry in fs::read_dir(out)? {
        entries += 1;
        resident = resident
            .checked_add(entry?.metadata()?.len())
            .ok_or_else(|| io::Error::other("build size overflow"))?;
        if entries > MAX_FILES * 4 || resident > MAX_TOTAL_BYTES as u64 * 4 {
            return Err(io::Error::other("embedded browser output limit"));
        }
    }
    let mut result = StaticBuild {
        files: Vec::new(),
        rust: String::new(),
        out: out.to_path_buf(),
        created: Vec::new(),
        keep: std::collections::HashSet::new(),
    };
    let mut total = 0usize;
    let mut captured = Vec::new();
    let mut stamps = Vec::new();
    let mut rust =
        String::from("pub static PREPARED_FILES: &[angel_serve::http::StaticFile] = &[\n");
    let mut definitions =
        String::from("const _: () = assert!(angel_serve::http::StaticFile::FORMAT == 1);\n");
    for (index, (name, path)) in files.iter().enumerate() {
        let file = fs::File::open(path)?;
        let before = file.metadata()?;
        let remaining = MAX_FILE_BYTES.min(MAX_TOTAL_BYTES.saturating_sub(total));
        if before.len() > remaining as u64 {
            return Err(io::Error::other("embedded browser byte limit"));
        }
        let mut bytes = Vec::new();
        file.take(remaining as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > remaining {
            return Err(io::Error::other("embedded browser byte limit"));
        }
        total += bytes.len();
        stamps.push((path.clone(), stamp(&before)?));
        after_read(path);
        let digest_bytes = sha1(&bytes);
        let digest = hex(&digest_bytes);
        let plain = out.join(format!("{digest}.plain"));
        result.retain(&plain, &bytes, &mut resident)?;
        definitions.push_str(&format!(
            "static BROWSER_FILE_{index}: &[u8] = include_bytes!({:?});\n",
            plain.to_string_lossy()
        ));
        let mime = mime_for(name);
        let body = Body::with_etag(mime, bytes, format!("\"{}\"", &digest[..16]))?;
        let gzip = if let Some(bytes) = body.gzip {
            let gzip_digest = hex(&sha1(&bytes));
            let path = out.join(format!("{gzip_digest}.gzip"));
            result.retain(&path, &bytes, &mut resident)?;
            format!("Some(include_bytes!({:?}))", path.to_string_lossy())
        } else {
            "None".into()
        };
        rust.push_str(&format!("angel_serve::http::StaticFile::generated({name:?}, {mime:?}, BROWSER_FILE_{index}, {digest_bytes:?}, {gzip}),\n"));
        captured.push((name.clone(), plain));
    }
    for (path, observed) in stamps {
        let now = fs::metadata(path)?;
        if stamp(&now)? != observed {
            return Err(io::Error::other(
                "embedded browser source changed during build capture",
            ));
        }
    }
    rust.push_str("];\n");
    definitions.push_str(&rust);
    result.files = captured;
    result.rust = definitions;
    Ok(result)
}
/// A proposed captured build owns new files until its generated source is
/// atomically published. Dropping a failed proposal cleans those files; older
/// successful outputs survive. Publication prunes retired content, bounding
/// repeated edits without touching already-linked binaries.
pub struct StaticBuild {
    files: Vec<(String, PathBuf)>,
    rust: String,
    out: PathBuf,
    created: Vec<PathBuf>,
    keep: std::collections::HashSet<PathBuf>,
}
impl StaticBuild {
    pub fn files(&self) -> &[(String, PathBuf)] {
        &self.files
    }
    pub fn rust(&self) -> &str {
        &self.rust
    }
    pub fn publish(mut self, path: &Path, source: &str) -> io::Result<()> {
        let staging = path.with_extension("browser-publish.tmp");
        if let Err(error) = fs::write(&staging, source).and_then(|()| fs::rename(&staging, path)) {
            let _ = fs::remove_file(staging);
            return Err(error);
        }
        self.created.clear();
        for entry in fs::read_dir(&self.out)? {
            let path = entry?.path();
            if !self.keep.contains(&path) && owned_output(&path) {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }
    fn retain(&mut self, path: &Path, bytes: &[u8], resident: &mut u64) -> io::Result<()> {
        if path.exists() {
            let file = fs::File::open(path)?;
            if file.metadata()?.len() != bytes.len() as u64 {
                return Err(io::Error::other("embedded browser output collision"));
            }
            let mut existing = Vec::new();
            file.take(bytes.len() as u64 + 1)
                .read_to_end(&mut existing)?;
            if existing != bytes {
                return Err(io::Error::other("embedded browser output collision"));
            }
        } else {
            if *resident + bytes.len() as u64 > MAX_TOTAL_BYTES as u64 * 4 {
                return Err(io::Error::other("embedded browser output limit"));
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            self.created.push(path.to_path_buf());
            file.write_all(bytes)?;
            *resident += bytes.len() as u64;
        }
        self.keep.insert(path.to_path_buf());
        Ok(())
    }
}
impl Drop for StaticBuild {
    fn drop(&mut self) {
        for path in &self.created {
            let _ = fs::remove_file(path);
        }
    }
}
fn owned_output(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    stem.len() == 40
        && stem.bytes().all(|b| b.is_ascii_hexdigit())
        && matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("plain" | "gzip")
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "browser-build-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn captured_bytes_publish_atomically_and_retire_prior_disk_outputs() {
        let root = root();
        let input = root.join("input.js");
        let out = root.join("out");
        let code = root.join("generated.rs");
        fs::write(&input, b"old exact compiled source".repeat(100)).unwrap();
        let first = capture_static(&[("script.js".into(), input.clone())], &out).unwrap();
        let original = first.files()[0].1.clone();
        let saved = fs::read(&original).unwrap();
        assert!(first.rust().contains("BROWSER_FILE_0"));
        first.publish(&code, "first generation").unwrap();
        fs::write(&input, b"new exact compiled source".repeat(100)).unwrap();
        let replacement = capture_static(&[("script.js".into(), input.clone())], &out).unwrap();
        let newer = replacement.files()[0].1.clone();
        assert_ne!(newer, original);
        assert_eq!(fs::read(&original).unwrap(), saved);
        drop(replacement);
        assert!(!newer.exists());
        assert!(original.exists());
        assert_eq!(fs::read(&code).unwrap(), b"first generation");
        let replacement = capture_static(&[("script.js".into(), input)], &out).unwrap();
        replacement.publish(&code, "second generation").unwrap();
        assert!(!original.exists());
        assert_eq!(fs::read(&code).unwrap(), b"second generation");
        assert!(fs::read_dir(&out).unwrap().count() <= 2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn changed_source_and_publication_failure_clean_new_files_without_losing_old_build() {
        let root = root();
        let input = root.join("input.js");
        let out = root.join("out");
        let code = root.join("generated.rs");
        fs::write(&input, b"first").unwrap();
        let first = capture_static(&[("script.js".into(), input.clone())], &out).unwrap();
        let old = first.files()[0].1.clone();
        first.publish(&code, "old published source").unwrap();
        fs::write(&input, b"another source").unwrap();
        assert!(
            capture_with_hook(&[("script.js".into(), input.clone())], &out, |p| fs::write(
                p,
                b"changed while capturing"
            )
            .unwrap())
            .is_err()
        );
        assert!(old.exists());
        assert_eq!(fs::read(&code).unwrap(), b"old published source");
        assert!(
            capture_static(&[("script.js".into(), input)], &out)
                .unwrap()
                .publish(&root.join("missing/code.rs"), "new")
                .is_err()
        );
        assert_eq!(fs::read_dir(&out).unwrap().count(), 1);
        assert!(old.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn byte_and_entry_limits_and_corrupted_content_are_rejected() {
        let root = root();
        let input = root.join("input.js");
        let out = root.join("out");
        fs::File::create(&input)
            .unwrap()
            .set_len(MAX_FILE_BYTES as u64 + 1)
            .unwrap();
        assert!(capture_static(&[("script.js".into(), input.clone())], &out).is_err());
        assert!(
            capture_static(
                &vec![("script.js".into(), input.clone()); MAX_FILES + 1],
                &out
            )
            .is_err()
        );
        fs::write(&input, b"exact source").unwrap();
        let first = capture_static(&[("script.js".into(), input.clone())], &out).unwrap();
        let stored = first.files()[0].1.clone();
        first.publish(&root.join("code.rs"), "published").unwrap();
        fs::write(&stored, b"stale corrupted bytes").unwrap();
        assert!(capture_static(&[("script.js".into(), input)], &out).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
