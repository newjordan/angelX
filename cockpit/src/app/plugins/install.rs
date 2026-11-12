//! Install, update, link and remove plugins in the cartridge folder.
//!
//! A registry install leaves `.angelx-plugin.toml` in the plugin's folder: the
//! entry it came from and the files it wrote. An update writes the new files,
//! drops the old ones and keeps whatever else is there (what the setup
//! fetched or built, an index the plugin keeps), so a setup that finds its
//! work done can skip it.

use super::check::{self, Level};
use super::digest::{self, Entry as File};
use super::registry::Entry;
use super::{Paths, Version, git};
use crate::agent::harness::cartridges::config::Config;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The install receipt, in the plugin's folder.
pub(crate) const RECEIPT: &str = ".angelx-plugin.toml";

/// What a registry install recorded.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) repository: String,
    pub(crate) commit: String,
    #[serde(default)]
    pub(crate) path: String,
    pub(crate) sha256: String,
    /// The registry the entry came from.
    pub(crate) registry: String,
    /// Seconds since the epoch.
    pub(crate) installed: u64,
    /// Whether the last setup finished; absent when there is no setup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) setup_ok: Option<bool>,
    /// The files the install wrote, relative to the folder.
    pub(crate) files: Vec<String>,
}

impl Receipt {
    fn read(folder: &Path) -> Option<Self> {
        toml::from_str(&std::fs::read_to_string(folder.join(RECEIPT)).ok()?).ok()
    }

    fn write(&self, folder: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::platform::workspace_store::write_atomic(&folder.join(RECEIPT), text.as_bytes())
    }
}

/// How a plugin in the cartridge folder got there.
#[derive(Clone, Debug)]
pub(crate) enum Origin {
    /// From a registry; its receipt.
    Registry(Receipt),
    /// `angelX plugin link`: a symlink to a folder being worked on.
    Linked(PathBuf),
    /// Put there by hand or by a cartridge's own install script.
    Placed,
}

/// One cartridge in the plugin folder.
#[derive(Clone, Debug)]
pub(crate) struct Installed {
    pub(crate) id: String,
    pub(crate) folder: PathBuf,
    pub(crate) origin: Origin,
    /// Its `cartridge.toml`, or why it does not load.
    pub(crate) config: Result<Config, String>,
}

impl Installed {
    pub(crate) fn version(&self) -> Option<String> {
        match (&self.origin, &self.config) {
            (_, Ok(config)) if config.plugin.is_some() => {
                config.plugin.as_ref().map(|plugin| plugin.version.clone())
            }
            (Origin::Registry(receipt), _) => Some(receipt.version.clone()),
            _ => None,
        }
    }
}

/// Every cartridge folder under `paths.cartridges`, by id.
pub(crate) fn installed(paths: &Paths) -> Vec<Installed> {
    let Ok(listing) = std::fs::read_dir(&paths.cartridges) else {
        return Vec::new();
    };
    let mut found: Vec<Installed> = listing
        .flatten()
        .filter(|item| item.path().join("cartridge.toml").is_file())
        .filter_map(|item| {
            let id = item.file_name().into_string().ok()?;
            Some(describe(&id, &item.path()))
        })
        .collect();
    found.sort_by(|left, right| left.id.cmp(&right.id));
    found
}

/// A name that is one folder directly in the plugin folder: no separators,
/// nothing hidden, not `.` or `..`.
pub(crate) fn folder_name(id: &str) -> bool {
    !id.is_empty() && !id.starts_with('.') && !id.contains(['/', '\\', '\0'])
}

/// The cartridge `id` in the plugin folder, if there is one.
pub(crate) fn find(paths: &Paths, id: &str) -> Option<Installed> {
    if !folder_name(id) {
        return None;
    }
    let folder = paths.cartridges.join(id);
    (std::fs::symlink_metadata(&folder).is_ok()).then(|| describe(id, &folder))
}

fn describe(id: &str, folder: &Path) -> Installed {
    let link = std::fs::symlink_metadata(folder).is_ok_and(|meta| meta.file_type().is_symlink());
    let origin = if link {
        Origin::Linked(std::fs::read_link(folder).unwrap_or_default())
    } else if let Some(receipt) = Receipt::read(folder) {
        Origin::Registry(receipt)
    } else {
        Origin::Placed
    };
    let config = std::fs::read_to_string(folder.join("cartridge.toml"))
        .map_err(|e| e.to_string())
        .and_then(|text| Config::parse(&text));
    Installed {
        id: id.to_owned(),
        folder: folder.to_path_buf(),
        origin,
        config,
    }
}

/// A scratch folder, removed when dropped. It sits in the plugin folder's
/// `.staging` (no cartridge.toml of its own, so the loader passes over it),
/// on the same filesystem as the installs it becomes.
pub(crate) struct Staging(PathBuf);

impl Staging {
    pub(crate) fn new(paths: &Paths, label: &str) -> Result<Self, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = paths
            .cartridges
            .join(".staging")
            .join(format!("{label}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Self(dir))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A plugin fetched and verified, not yet installed.
pub(crate) struct Fetched {
    pub(crate) staging: Staging,
    /// Its files, written out under the staging folder.
    pub(crate) folder: PathBuf,
    pub(crate) files: Vec<File>,
    pub(crate) config: Config,
    pub(crate) report: check::Report,
}

/// Fetch `entry`'s folder at its commit, check the digest, write the files
/// out and check that they load as the entry says. Nothing in them runs.
pub(crate) fn fetch(
    paths: &Paths,
    entry: &Entry,
    cancel: Option<&AtomicBool>,
) -> Result<Fetched, String> {
    let staging = Staging::new(paths, &entry.id)?;
    let files = git::fetch(
        &entry.repository,
        &entry.commit,
        &entry.path,
        &staging.path().join("repo.git"),
        cancel,
    )?;
    let got = digest::digest(&files);
    if got != entry.sha256 {
        return Err(format!(
            "{} at {} does not match the registry: its files digest to {got}, the registry says {}; nothing was installed",
            entry.source(),
            short(&entry.commit),
            entry.sha256
        ));
    }
    let folder = staging.path().join("plugin");
    digest::write_folder(&folder, &files)?;
    let report = check::folder(&folder, Level::Load);
    if !report.ok() {
        return Err(format!(
            "{} does not load as a plugin:\n{}",
            entry.id,
            report.lines().join("\n")
        ));
    }
    let config = report
        .config
        .clone()
        .expect("a passing check parsed the cartridge");
    if config.id != entry.id {
        return Err(format!(
            "the registry's `{}` is a cartridge with id `{}`",
            entry.id, config.id
        ));
    }
    if let Some(plugin) = &config.plugin
        && plugin.version != entry.version
    {
        return Err(format!(
            "the registry lists {} {}, but its cartridge.toml says {}",
            entry.id, entry.version, plugin.version
        ));
    }
    if let Some(required) = config
        .plugin
        .as_ref()
        .and_then(|plugin| plugin.angelx.as_deref())
        && let (Some(required), Some(running)) = (
            Version::parse(required),
            Version::parse(super::running_version()),
        )
        && running < required
    {
        return Err(format!(
            "{} {} needs angelX {} or newer; this is {} (`angelX update` brings the latest)",
            entry.id,
            entry.version,
            config
                .plugin
                .as_ref()
                .and_then(|plugin| plugin.angelx.as_deref())
                .unwrap_or(""),
            super::running_version()
        ));
    }
    Ok(Fetched {
        staging,
        folder,
        files,
        config,
        report,
    })
}

/// Where a setup's output goes: the terminal, or the cockpit's job panel.
pub(crate) enum Output<'a> {
    Inherit,
    Sink(&'a (dyn Fn(&[u8]) + Send + Sync)),
}

/// How an install went.
#[derive(Debug)]
pub(crate) struct Outcome {
    pub(crate) id: String,
    pub(crate) folder: PathBuf,
    /// The version it replaced, on an update.
    pub(crate) replaced: Option<String>,
    pub(crate) version: String,
    pub(crate) config: Config,
    /// `Some(Err)` when the setup failed; the files stay in place.
    pub(crate) setup: Option<Result<(), String>>,
    /// Anything else the operator should know.
    pub(crate) notes: Vec<String>,
}

/// Install `entry` from `registry` into the plugin folder, replacing a
/// previous registry install of it. `force` also replaces a linked or
/// hand-placed cartridge of that id. The setup runs unless `skip_setup`.
pub(crate) fn install(
    paths: &Paths,
    entry: &Entry,
    registry: &str,
    force: bool,
    skip_setup: bool,
    output: Output<'_>,
    cancel: Option<&AtomicBool>,
) -> Result<Outcome, String> {
    let dest = paths.cartridges.join(&entry.id);
    let previous = find(paths, &entry.id);
    let old_receipt = match &previous {
        None => None,
        Some(Installed {
            origin: Origin::Registry(receipt),
            ..
        }) => Some(receipt.clone()),
        Some(Installed {
            origin: Origin::Linked(target),
            ..
        }) if !force => {
            return Err(format!(
                "{} is linked to {}; `angelX plugin remove {}` unlinks it, or --force replaces the link",
                dest.display(),
                target.display(),
                entry.id
            ));
        }
        Some(Installed {
            origin: Origin::Placed,
            ..
        }) if !force => {
            return Err(format!(
                "{} is already there and was not installed from a registry; --force replaces it",
                dest.display()
            ));
        }
        Some(_) => None,
    };
    let fetched = fetch(paths, entry, cancel)?;
    if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Err("cancelled".into());
    }
    let files: Vec<String> = fetched.files.iter().map(|file| file.path.clone()).collect();
    let has_setup = fetched
        .config
        .plugin
        .as_ref()
        .and_then(|plugin| plugin.setup.as_ref())
        .is_some();
    let receipt = Receipt {
        id: entry.id.clone(),
        version: entry.version.clone(),
        repository: entry.repository.clone(),
        commit: entry.commit.clone(),
        path: entry.path.clone(),
        sha256: entry.sha256.clone(),
        registry: registry.to_owned(),
        installed: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        setup_ok: (has_setup && !skip_setup).then_some(false),
        files: files.clone(),
    };
    receipt.write(&fetched.folder)?;
    std::fs::create_dir_all(&paths.cartridges)
        .map_err(|e| format!("{}: {e}", paths.cartridges.display()))?;
    let aside = swap(&fetched.folder, &dest, fetched.staging.path())?;
    let mut notes = Vec::new();
    // Carry over what the old install's setup made: everything in its folder
    // that the old files list does not name and the new tree does not hold.
    // What cannot move is kept beside the plugins, never dropped.
    if let (Some(old), Some(aside)) = (&old_receipt, &aside) {
        let ours: BTreeSet<&str> = old.files.iter().map(String::as_str).collect();
        let theirs: BTreeSet<&str> = files.iter().map(String::as_str).collect();
        if let Err(error) = carry(aside, &dest, "", &ours, &theirs) {
            let kept = paths
                .cartridges
                .join(".staging")
                .join(format!("{}-previous-{}", entry.id, receipt.installed));
            let kept = match std::fs::rename(aside, &kept) {
                Ok(()) => kept.display().to_string(),
                Err(_) => aside.display().to_string(),
            };
            notes.push(format!(
                "could not carry everything over from the previous install ({error}); the rest is in {kept}"
            ));
        }
    }
    let mut outcome = Outcome {
        id: entry.id.clone(),
        folder: dest.clone(),
        replaced: previous.as_ref().and_then(Installed::version),
        version: entry.version.clone(),
        config: fetched.config.clone(),
        setup: None,
        notes,
    };
    if has_setup && !skip_setup {
        let result = run_setup(&dest, &fetched.config, output, cancel);
        let mut receipt = receipt;
        receipt.setup_ok = Some(result.is_ok());
        let _ = receipt.write(&dest);
        outcome.setup = Some(result);
    }
    Ok(outcome)
}

/// Move into `new` what `old` held beyond its install's own files.
fn carry(
    old: &Path,
    new: &Path,
    prefix: &str,
    ours: &BTreeSet<&str>,
    theirs: &BTreeSet<&str>,
) -> Result<(), String> {
    let Ok(listing) = std::fs::read_dir(old.join(prefix)) else {
        return Ok(());
    };
    for item in listing.flatten() {
        let Ok(name) = item.file_name().into_string() else {
            continue;
        };
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if prefix.is_empty() && name == RECEIPT {
            continue;
        }
        let under =
            |set: &BTreeSet<&str>| set.iter().any(|file| file.starts_with(&format!("{rel}/")));
        let is_dir = item.file_type().is_ok_and(|kind| kind.is_dir());
        if is_dir && (under(ours) || under(theirs)) {
            carry(old, new, &rel, ours, theirs)?;
            continue;
        }
        if ours.contains(rel.as_str())
            || theirs.contains(rel.as_str())
            || std::fs::symlink_metadata(new.join(&rel)).is_ok()
        {
            continue;
        }
        let target = new.join(&rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::rename(item.path(), &target)
            .map_err(|e| format!("keeping {}: {e}", item.path().display()))?;
    }
    Ok(())
}

/// Put `new` at `dest`, moving whatever was there aside into `staging` first.
/// Put `new` at `dest`, moving whatever was there aside into `staging`
/// first; returns where it went. On failure the old folder is put back.
fn swap(new: &Path, dest: &Path, staging: &Path) -> Result<Option<PathBuf>, String> {
    let aside = staging.join("previous");
    let had = std::fs::symlink_metadata(dest).is_ok();
    if had {
        std::fs::rename(dest, &aside)
            .map_err(|e| format!("moving {} aside: {e}", dest.display()))?;
    }
    if let Err(error) = std::fs::rename(new, dest) {
        if had {
            let _ = std::fs::rename(&aside, dest);
        }
        return Err(format!("installing into {}: {error}", dest.display()));
    }
    Ok(had.then_some(aside))
}

/// Run a plugin's setup in its folder: `<setup> <folder>`, with
/// `ANGEL_CARTRIDGE_DIR` set to the folder.
pub(crate) fn run_setup(
    folder: &Path,
    config: &Config,
    output: Output<'_>,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let Some(setup) = config
        .plugin
        .as_ref()
        .and_then(|plugin| plugin.setup.as_deref())
    else {
        return Ok(());
    };
    let program = folder.join(setup);
    let mut command = std::process::Command::new(&program);
    command
        .arg(folder)
        .current_dir(folder)
        .env("ANGEL_CARTRIDGE_DIR", folder)
        .env("ANGEL_PLUGIN_ID", &config.id)
        .stdin(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let status = match output {
        Output::Inherit => {
            let mut child = command.spawn().map_err(|e| format!("setup {setup}: {e}"))?;
            wait(&mut child, cancel)?
        }
        Output::Sink(sink) => {
            command
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            let mut child = command.spawn().map_err(|e| format!("setup {setup}: {e}"))?;
            let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
            let pump = |pipe: Option<Box<dyn Read + Send>>,
                        tx: std::sync::mpsc::Sender<Vec<u8>>| {
                std::thread::spawn(move || {
                    let Some(mut pipe) = pipe else { return };
                    let mut buffer = [0u8; 4096];
                    while let Ok(count) = pipe.read(&mut buffer) {
                        if count == 0 || tx.send(buffer[..count].to_vec()).is_err() {
                            break;
                        }
                    }
                })
            };
            let out = pump(
                child
                    .stdout
                    .take()
                    .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
                tx.clone(),
            );
            let err = pump(
                child
                    .stderr
                    .take()
                    .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
                tx,
            );
            let status = loop {
                while let Ok(chunk) = rx.try_recv() {
                    sink(&chunk);
                }
                if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                    git::kill(&mut child);
                    return Err("setup cancelled".into());
                }
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
                    Err(e) => return Err(format!("setup {setup}: {e}")),
                }
            };
            let _ = out.join();
            let _ = err.join();
            while let Ok(chunk) = rx.try_recv() {
                sink(&chunk);
            }
            status
        }
    };
    if status.success() {
        Ok(())
    } else {
        Err(format!("setup {setup} exited {status}"))
    }
}

fn wait(
    child: &mut std::process::Child,
    cancel: Option<&AtomicBool>,
) -> Result<std::process::ExitStatus, String> {
    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            git::kill(child);
            return Err("setup cancelled".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(e) => return Err(format!("setup: {e}")),
        }
    }
}

/// Run the setup of an installed plugin again.
pub(crate) fn setup_again(
    paths: &Paths,
    id: &str,
    output: Output<'_>,
    cancel: Option<&AtomicBool>,
) -> Result<String, String> {
    let found = find(paths, id)
        .ok_or_else(|| format!("{id} is not installed in {}", paths.cartridges.display()))?;
    let config = found.config.clone()?;
    if config
        .plugin
        .as_ref()
        .and_then(|plugin| plugin.setup.as_ref())
        .is_none()
    {
        return Ok(format!("{id} has no setup"));
    }
    let result = run_setup(&found.folder, &config, output, cancel);
    if let Origin::Registry(mut receipt) = found.origin {
        receipt.setup_ok = Some(result.is_ok());
        let _ = receipt.write(&found.folder);
    }
    result.map(|()| format!("{id}: setup finished"))
}

/// Remove an installed plugin: a registry install's folder, or a link (never
/// what it points to). `force` also removes a hand-placed cartridge.
pub(crate) fn remove(paths: &Paths, id: &str, force: bool) -> Result<String, String> {
    if !folder_name(id) {
        return Err(format!("`{id}` is not a plugin id"));
    }
    let found = find(paths, id)
        .ok_or_else(|| format!("{id} is not installed in {}", paths.cartridges.display()))?;
    match &found.origin {
        Origin::Linked(target) => {
            std::fs::remove_file(&found.folder)
                .map_err(|e| format!("{}: {e}", found.folder.display()))?;
            Ok(format!(
                "unlinked {id} (its folder {} is untouched)",
                target.display()
            ))
        }
        Origin::Placed if !force => Err(format!(
            "{} was not installed from a registry; --force removes the folder and everything in it",
            found.folder.display()
        )),
        Origin::Registry(_) | Origin::Placed => {
            std::fs::remove_dir_all(&found.folder)
                .map_err(|e| format!("{}: {e}", found.folder.display()))?;
            Ok(format!("removed {id} ({})", found.folder.display()))
        }
    }
}

/// Link the plugin being worked on at `dir` into the plugin folder, so angelX
/// loads it from where it is edited.
pub(crate) fn link(
    paths: &Paths,
    dir: &Path,
    force: bool,
) -> Result<(String, check::Report), String> {
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    let report = check::folder(&dir, Level::Load);
    let Some(config) = report.config.clone() else {
        return Err(format!(
            "{} does not load:\n{}",
            dir.display(),
            report.lines().join("\n")
        ));
    };
    if !report.ok() && !force {
        return Err(format!(
            "{} does not load as a plugin yet (--force links it anyway):\n{}",
            dir.display(),
            report.lines().join("\n")
        ));
    }
    let dest = paths.cartridges.join(&config.id);
    if let Some(found) = find(paths, &config.id) {
        match &found.origin {
            Origin::Linked(target) if target == &dir => {
                return Ok((
                    format!("{} is already linked to {}", config.id, dir.display()),
                    report,
                ));
            }
            Origin::Linked(_) => {
                std::fs::remove_file(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
            }
            _ if force => {
                std::fs::remove_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
            }
            _ => {
                return Err(format!(
                    "{} is already there; `angelX plugin remove {}` first, or --force replaces it",
                    dest.display(),
                    config.id
                ));
            }
        }
    }
    std::fs::create_dir_all(&paths.cartridges)
        .map_err(|e| format!("{}: {e}", paths.cartridges.display()))?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(&dir, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    #[cfg(not(unix))]
    return Err("linking needs a unix system".into());
    #[allow(unreachable_code)]
    Ok((
        format!("linked {} → {}", dest.display(), dir.display()),
        report,
    ))
}

/// The first 12 characters of a commit id.
pub(crate) fn short(commit: &str) -> &str {
    &commit[..commit.len().min(12)]
}
