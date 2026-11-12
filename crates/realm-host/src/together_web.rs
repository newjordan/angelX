//! The Delve's browser page, built and embedded with this binary
//! (`build.rs`): the angel-web shell and the Delve's wasm client, served by
//! the delve's own server at `/` and `/a/<hash>`, so a page and its host
//! always match.
//!
//! The music is not in the binary. It ships beside it (the package carries
//! `cockpit/assets/dungeon/music`), and the host reads it from there when it
//! opens its door: each track joins the page's sounds manifest and is served
//! like any other file. With no music folder found the page has no music,
//! a line says so in the log, and the game plays on.
//!
//! Where a shipped folder sits, and how a missing one is said, are the
//! cockpit's to know: it lends them as [`Places`].

use std::path::{Path, PathBuf};

use angel_serve::ClientBundle;

include!(concat!(env!("OUT_DIR"), "/delve_web.rs"));

/// What the host lends the page: where a shipped folder sits and how a
/// missing file is said.
pub struct Places {
    /// The shipped folder `place` (relative to the repository, as
    /// `cockpit/assets/dungeon/music`), under the first root that has it.
    pub resource: fn(place: &str) -> Option<PathBuf>,
    /// Say that a shipped file is missing: to the log, and to stderr when
    /// that is not the screen the cockpit draws on.
    pub missing: fn(line: &str),
}

/// The page for a hosted delve's server with the music found where the
/// package ships it (`together_audio`), or `None` in a build without one.
pub fn shipped() -> Option<ClientBundle> {
    bundle(&Places {
        resource: crate::together_audio::resource,
        missing: crate::together_audio::missing,
    })
}

/// The page for a hosted delve's server, or `None` in a build without one.
pub fn bundle(places: &Places) -> Option<ClientBundle> {
    let (index, files) = BUNDLE?;
    let mut files: Vec<(String, Vec<u8>, String)> = files
        .iter()
        .map(|(name, bytes)| {
            (
                name.to_string(),
                bytes.to_vec(),
                angel_serve::http::mime_for(name).to_string(),
            )
        })
        .collect();
    let mut sounds: Vec<String> = Some(SOUNDS)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .into_iter()
        .collect();
    for (prefix, place) in BESIDE {
        match (places.resource)(place) {
            Some(dir) => sounds.extend(add_folder(&dir, prefix, &mut files, places)),
            None => (places.missing)(&format!(
                "no {place} beside this binary: browser seats play without it"
            )),
        }
    }
    Some(ClientBundle {
        index_html: index.replace("@@SOUNDS@@", &format!("{{{}}}", sounds.join(","))),
        files,
    })
}

/// Every file in `dir` as `<prefix><file>`, added to `files`; their entries
/// in the sounds manifest (`"name":"{{asset:name.ext}}"`), in name order. A
/// file the page could not name, or that cannot be read, is left out.
fn add_folder(
    dir: &Path,
    prefix: &str,
    files: &mut Vec<(String, Vec<u8>, String)>,
    places: &Places,
) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        (places.missing)(&format!("could not read the sound folder {}", dir.display()));
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    let mut sounds = Vec::new();
    for path in paths {
        let file = format!(
            "{prefix}{}",
            path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
        );
        if !file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            (places.missing)(&format!("could not read the sound {}", path.display()));
            continue;
        };
        let stem = file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s);
        sounds.push(format!("\"{stem}\":\"{{{{asset:{file}}}}}\""));
        let mime = angel_serve::http::mime_for(&file).to_string();
        files.push((file, bytes, mime));
    }
    sounds
}

#[cfg(test)]
mod tests;

/// Admit one browser generation without reading directories or compressing on
/// the session frame. Lookup configuration is fixed now; source bytes become
/// immutable only inside the worker, before the first page can be served.
pub fn shipped_prepared() -> std::io::Result<Option<angel_serve::http::PreparedAssets>> {
    if BUNDLE.is_none() { return Ok(None); }
    // The tests of a process share its two generations: whoever admits one holds `tests::browser_capacity()`.
    #[cfg(test)]
    assert!(crate::tests::capacity_held(), "a test that admits a browser generation holds `tests::browser_capacity()`");
    crate::lend_source_tree();
    let resource = std::env::var_os("ANGEL_RESOURCE_DIR").filter(|v| !v.is_empty()).map(PathBuf::from);
    let exe = std::env::current_exe().ok();
    let source = angel_live::host::runtime_paths::source_cockpit_dir().map(Path::to_path_buf);
    let identity = angel_live::host::runtime_paths::bundle_identity();
    angel_serve::http::PreparedAssets::start_static(move |cancel| {
        let root = angel_live::host::runtime_paths::resolve_root(resource.clone(), exe.as_deref(), source.as_deref(), identity);
        let roots: Vec<_> = resource.into_iter().chain(source.and_then(|p| p.parent().map(Path::to_path_buf))).chain(Some(root)).collect();
        capture(&roots, cancel)
    }, PREPARED_FILES).map(Some)
}

#[derive(PartialEq, Eq)]
struct SourceStamp {
    len: u64,
    modified: std::time::SystemTime,
    #[cfg(unix)] identity: (u64, u64, i64, i64),
}
fn source_stamp(metadata: std::fs::Metadata) -> std::io::Result<SourceStamp> {
    #[cfg(unix)] use std::os::unix::fs::MetadataExt;
    Ok(SourceStamp {
        len: metadata.len(), modified: metadata.modified()?,
        #[cfg(unix)] identity: (metadata.dev(), metadata.ino(), metadata.ctime(), metadata.ctime_nsec()),
    })
}

fn capture(roots: &[PathBuf], cancel: &std::sync::atomic::AtomicBool) -> std::io::Result<ClientBundle> {
    capture_with_hook(roots, cancel, |_| {})
}

fn capture_with_hook(roots: &[PathBuf], cancel: &std::sync::atomic::AtomicBool, mut after_read: impl FnMut(&Path)) -> std::io::Result<ClientBundle> {
    use angel_serve::http::preparation::{MAX_FILES, MAX_FILE_BYTES, MAX_TOTAL_BYTES};
    use std::io::Read;
    use std::sync::atomic::Ordering;
    let (index, embedded) = BUNDLE.ok_or_else(|| std::io::Error::other("browser bundle absent"))?;
    let mut total = index.len();
    let mut files = Vec::new();
    let mut sounds: Vec<String> = (!SOUNDS.is_empty()).then(|| SOUNDS.to_string()).into_iter().collect();
    let check = |count: usize, size: usize, total: usize| -> std::io::Result<()> {
        if cancel.load(Ordering::Acquire) { return Err(std::io::Error::other("browser capture cancelled")); }
        if count > MAX_FILES || size > MAX_FILE_BYTES || total > MAX_TOTAL_BYTES { return Err(std::io::Error::other("browser capture limit")); }
        Ok(())
    };
    check(0, index.len(), total)?;
    for (name, bytes) in embedded {
        total = total.checked_add(bytes.len()).ok_or_else(|| std::io::Error::other("browser size overflow"))?;
        check(files.len() + 1, bytes.len(), total)?;
        files.push((name.to_string(), bytes.to_vec(), angel_serve::http::mime_for(name).to_string()));
    }
    let mut observed = Vec::new();
    let mut folders = Vec::new();
    for (prefix, place) in BESIDE {
        let Some(dir) = roots.iter().map(|r| r.join(place)).find(|p| p.is_dir()) else {
            crate::together_audio::missing(&format!("no {place} beside this binary: browser seats play without it"));
            continue;
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            crate::together_audio::missing(&format!("could not read the sound folder {}", dir.display()));
            continue;
        };
        folders.push((dir.clone(), source_stamp(std::fs::metadata(&dir)?)?));
        let mut paths = Vec::new();
        for (i, entry) in entries.enumerate() {
            // Bound directory traversal too, including rejected/non-file entries.
            check(i + 1, 0, total)?;
            let Ok(entry) = entry else { continue; };
            // Match shipped bundle policy: symlinks to regular files count.
            if entry.path().is_file() { paths.push(entry.path()); }
        }
        paths.sort();
        for path in paths {
            check(files.len() + 1, 0, total)?;
            let file = format!("{prefix}{}", path.file_name().unwrap_or_default().to_string_lossy());
            if !file.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) { continue; }
            let remaining = MAX_FILE_BYTES.min(MAX_TOTAL_BYTES.saturating_sub(total));
            let mut bytes = Vec::new();
            let read = (|| -> std::io::Result<_> {
                let file = std::fs::File::open(&path)?;
                let before = file.metadata()?;
                if before.len() > remaining as u64 { return Err(std::io::Error::new(std::io::ErrorKind::FileTooLarge, "browser capture limit")); }
                file.take(remaining as u64 + 1).read_to_end(&mut bytes)?;
                source_stamp(before)
            })();
            let stamp = match read {
                Ok(stamp) => stamp,
                Err(error) if error.kind() == std::io::ErrorKind::FileTooLarge => return Err(error),
                Err(_) => { crate::together_audio::missing(&format!("could not read the sound {}", path.display())); continue; }
            };
            observed.push((path.clone(), stamp));
            after_read(&path);
            total = total.checked_add(bytes.len()).ok_or_else(|| std::io::Error::other("browser size overflow"))?;
            check(files.len() + 1, bytes.len(), total)?;
            let stem = file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s);
            sounds.push(format!("\"{stem}\":\"{{{{asset:{file}}}}}\""));
            let mime = angel_serve::http::mime_for(&file).to_string();
            files.push((file, bytes, mime));
        }
    }
    // A changed directory or read source never publishes a mixed manifest.
    // This detects ordinary writes/replacements during capture; publishers that
    // deliberately restore metadata must use an immutable version directory.
    for (path, stamp) in observed {
        if source_stamp(std::fs::metadata(path)?)? != stamp { return Err(std::io::Error::other("browser source changed during capture")); }
    }
    for (path, stamp) in folders {
        if source_stamp(std::fs::metadata(path)?)? != stamp { return Err(std::io::Error::other("browser folder changed during capture")); }
    }
    Ok(ClientBundle { index_html: index.replace("@@SOUNDS@@", &format!("{{{}}}", sounds.join(","))), files })
}
