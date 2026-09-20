use super::portability::PortableEntryV1;
use super::store_fs::{OpenKind, open_private, read_bounded, reject_symlink, secure_dir, sync_dir};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_COMPONENT_BYTES: u64 = 64 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn collect_entries(
    source_root: &Path,
    dossier_generation: &str,
) -> Result<Vec<PortableEntryV1>, String> {
    let mut entries = Vec::new();
    for name in ["actions.jsonl", "leases.snapshot.json"] {
        collect_file(
            source_root,
            &source_root.join("action").join(name),
            &mut entries,
        )?;
    }
    collect_file(
        source_root,
        &source_root.join("dossier/current.json"),
        &mut entries,
    )?;
    collect_tree(
        source_root,
        &source_root
            .join("dossier/generations")
            .join(dossier_generation),
        &mut entries,
    )?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn collect_tree(
    base: &Path,
    path: &Path,
    entries: &mut Vec<PortableEntryV1>,
) -> Result<(), String> {
    reject_symlink(path)?;
    let metadata =
        fs::metadata(path).map_err(|error| format!("inspect portable state: {error}"))?;
    if metadata.is_file() {
        return collect_file(base, path, entries);
    }
    if !metadata.is_dir() {
        return Err("portable state contains a non-file entry".into());
    }
    let mut children = fs::read_dir(path)
        .map_err(|error| format!("read portable state: {error}"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read portable state entry: {error}"))?;
    children.sort();
    for child in children {
        collect_tree(base, &child, entries)?;
    }
    Ok(())
}

fn collect_file(
    base: &Path,
    path: &Path,
    entries: &mut Vec<PortableEntryV1>,
) -> Result<(), String> {
    reject_symlink(path)?;
    let body = read_bounded(path, MAX_COMPONENT_BYTES)?;
    let relative = path
        .strip_prefix(base)
        .map_err(|_| "portable entry escaped state root".to_string())?;
    let path = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    entries.push(PortableEntryV1 {
        path,
        content_sha256: crate::knowledge::cut::sha256_hex(&body),
        body,
    });
    Ok(())
}

pub(super) fn install_staging(
    target_root: &Path,
    entries: &[PortableEntryV1],
) -> Result<PathBuf, String> {
    reject_symlink(target_root)?;
    if target_root.exists() {
        return Err("portable import target already exists".into());
    }
    let parent = target_root
        .parent()
        .ok_or_else(|| "portable import target has no parent".to_string())?;
    reject_symlink(parent)?;
    if !parent.is_dir() {
        return Err("portable import parent is missing or not a directory".into());
    }
    let name = target_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "invalid portable import target".to_string())?;
    let staging = parent.join(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&staging).map_err(|error| format!("create import staging: {error}"))?;
    secure_dir(&staging)?;
    for entry in entries {
        let path = staging.join(&entry.path);
        let parent = path
            .parent()
            .ok_or_else(|| "portable entry has no parent".to_string())?;
        create_private_dirs(&staging, parent)?;
        let mut file = open_private(&path, OpenKind::CreateNew)?;
        file.write_all(&entry.body)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("write portable entry: {error}"))?;
        if entry.path.ends_with("/manifest.json") {
            let anchors = path.parent().unwrap().join("anchors");
            if !anchors.exists() {
                fs::create_dir(&anchors)
                    .map_err(|error| format!("create import anchor root: {error}"))?;
                secure_dir(&anchors)?;
            }
        }
    }
    sync_tree(&staging)?;
    Ok(staging)
}

fn create_private_dirs(root: &Path, target: &Path) -> Result<(), String> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| "portable directory escaped staging".to_string())?;
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        if !path.exists() {
            fs::create_dir(&path).map_err(|error| format!("create import directory: {error}"))?;
        }
        reject_symlink(&path)?;
        secure_dir(&path)?;
    }
    Ok(())
}

fn sync_tree(root: &Path) -> Result<(), String> {
    let mut directories = Vec::new();
    collect_directories(root, &mut directories)?;
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        sync_dir(&directory)?;
    }
    Ok(())
}

fn collect_directories(path: &Path, directories: &mut Vec<PathBuf>) -> Result<(), String> {
    reject_symlink(path)?;
    directories.push(path.into());
    for entry in fs::read_dir(path).map_err(|error| format!("read import directory: {error}"))? {
        let path = entry
            .map_err(|error| format!("read import entry: {error}"))?
            .path();
        reject_symlink(&path)?;
        if path.is_dir() {
            collect_directories(&path, directories)?;
        }
    }
    Ok(())
}

pub(super) fn publish_staging(staging: &Path, target: &Path) -> Result<(), String> {
    fs::rename(staging, target).map_err(|error| format!("publish portable state: {error}"))?;
    sync_dir(
        target
            .parent()
            .ok_or_else(|| "portable target has no parent".to_string())?,
    )
}
