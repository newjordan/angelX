//! `angelX plugin …` in a shell, and the reports `/plugins` shows in the
//! cockpit. Every report is plain text built here, so the two say the same.

use super::check::{self, Level};
use super::install::{self, Installed, Origin, Output};
use super::registry::{self, Entry, Registry};
use super::{Paths, scaffold, submit};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// Who reads a report: hints name the command they would type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Surface {
    Shell,
    Cockpit,
}

impl Surface {
    fn command(self, rest: &str) -> String {
        match self {
            Surface::Shell => format!("`angelX plugin {rest}`"),
            Surface::Cockpit => format!("`/plugins {rest}`"),
        }
    }
}

pub(crate) const USAGE: &str = "\
angelX plugin — find, install and publish angelX plugins (docs/PLUGINS.md)

  angelX plugin                         the plugins installed here
  angelX plugin search [words]          the registry, or the plugins matching every word
  angelX plugin info <id>               fetch one and show what it carries, before installing
  angelX plugin install <id>… [--force] [--no-setup]
  angelX plugin update [<id>…] [--no-setup]
  angelX plugin remove <id>… [--force]
  angelX plugin setup <id>              run a plugin's setup again

  angelX plugin new <id> [dir]          a new plugin that works, to grow from
  angelX plugin check [dir]             whether it loads, and whether the registry takes it
  angelX plugin link [dir] [--force]    load the plugin from the folder it is edited in
  angelX plugin submit [dir] [--commit <sha>] [--repository <url>] [--print | --into <registry.toml>]
                                        open the registry pull request for a pushed plugin

Plugins install into ~/.angelX/cartridges (ANGEL_PLUGIN_DIR). The registry is
ANGEL_PLUGIN_REGISTRY: a URL or a file; by default angelX's own.
";

/// `angel --plugin ARGS…`; returns the exit status.
pub(crate) fn main(args: impl IntoIterator<Item = OsString>) -> i32 {
    let args: Vec<String> = match args.into_iter().map(OsString::into_string).collect() {
        Ok(args) => args,
        Err(_) => {
            eprintln!("angelX plugin: arguments must be UTF-8");
            return 2;
        }
    };
    let paths = Paths::from_env();
    let progress = |line: &str| eprintln!("{line}");
    match run(
        &paths,
        &args,
        Surface::Shell,
        &progress,
        Output::Inherit,
        None,
    ) {
        Ok(text) => {
            if !text.is_empty() {
                println!("{text}");
            }
            0
        }
        Err(Failure::Usage(text)) => {
            eprintln!("{text}");
            2
        }
        Err(Failure::Failed(text)) => {
            eprintln!("{text}");
            1
        }
    }
}

/// Why a command did not finish.
#[derive(Debug)]
pub(crate) enum Failure {
    /// The words were wrong.
    Usage(String),
    /// The command ran and failed.
    Failed(String),
}

impl From<String> for Failure {
    fn from(text: String) -> Self {
        Failure::Failed(text)
    }
}

/// Split flags from words: `--name value` for `valued`, `--name` otherwise.
struct Words {
    words: Vec<String>,
    flags: Vec<String>,
    values: Vec<(String, String)>,
}

fn parse(args: &[String], switches: &[&str], valued: &[&str]) -> Result<Words, Failure> {
    let mut words = Vec::new();
    let mut flags = Vec::new();
    let mut values = Vec::new();
    let mut at = 0;
    while at < args.len() {
        let arg = &args[at];
        if let Some(name) = arg.strip_prefix("--") {
            let (name, inline) = match name.split_once('=') {
                Some((name, value)) => (name, Some(value.to_owned())),
                None => (name, None),
            };
            if valued.contains(&name) {
                let value = match inline {
                    Some(value) => value,
                    None => {
                        at += 1;
                        args.get(at)
                            .cloned()
                            .ok_or_else(|| Failure::Usage(format!("--{name} needs a value")))?
                    }
                };
                values.push((name.to_owned(), value));
            } else if switches.contains(&name) && inline.is_none() {
                flags.push(name.to_owned());
            } else {
                return Err(Failure::Usage(format!(
                    "unknown option --{name}\n\n{USAGE}"
                )));
            }
        } else {
            words.push(arg.clone());
        }
        at += 1;
    }
    Ok(Words {
        words,
        flags,
        values,
    })
}

impl Words {
    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|flag| flag == name)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn dir(&self) -> Result<PathBuf, Failure> {
        match self.words.as_slice() {
            [] => Ok(PathBuf::from(".")),
            [dir] => Ok(PathBuf::from(dir)),
            _ => Err(Failure::Usage(format!("one folder at most\n\n{USAGE}"))),
        }
    }
}

/// Run one `plugin` command and return its report.
pub(crate) fn run(
    paths: &Paths,
    args: &[String],
    surface: Surface,
    progress: &dyn Fn(&str),
    output: Output<'_>,
    cancel: Option<&AtomicBool>,
) -> Result<String, Failure> {
    let (command, rest) = match args.split_first() {
        None => return Ok(list(paths, surface)),
        Some((command, rest)) => (command.as_str(), rest),
    };
    match command {
        "list" | "ls" => {
            parse(rest, &[], &[])?;
            Ok(list(paths, surface))
        }
        "search" | "find" => {
            let words = parse(rest, &[], &[])?;
            search(paths, &words.words.join(" "), surface).map_err(Failure::from)
        }
        "info" | "show" => {
            let words = parse(rest, &[], &[])?;
            let [id] = words.words.as_slice() else {
                return Err(Failure::Usage(format!(
                    "info takes one plugin id\n\n{USAGE}"
                )));
            };
            info(paths, id, surface, progress, cancel).map_err(Failure::from)
        }
        "install" | "add" => {
            let words = parse(rest, &["force", "no-setup"], &[])?;
            if words.words.is_empty() {
                return Err(Failure::Usage(format!(
                    "install takes plugin ids\n\n{USAGE}"
                )));
            }
            install_all(
                paths,
                &words.words,
                words.flag("force"),
                words.flag("no-setup"),
                surface,
                progress,
                output,
                cancel,
            )
            .map_err(Failure::from)
        }
        "update" | "upgrade" => {
            let words = parse(rest, &["no-setup"], &[])?;
            update(
                paths,
                &words.words,
                words.flag("no-setup"),
                surface,
                progress,
                output,
                cancel,
            )
            .map_err(Failure::from)
        }
        "remove" | "uninstall" | "rm" => {
            let words = parse(rest, &["force"], &[])?;
            if words.words.is_empty() {
                return Err(Failure::Usage(format!(
                    "remove takes plugin ids\n\n{USAGE}"
                )));
            }
            let mut lines = Vec::new();
            let mut failed = false;
            for id in &words.words {
                match install::remove(paths, id, words.flag("force")) {
                    Ok(line) => lines.push(line),
                    Err(error) => {
                        failed = true;
                        lines.push(error);
                    }
                }
            }
            if surface == Surface::Shell && !failed {
                lines.push(
                    "angelX sessions already running keep its tools until they next start".into(),
                );
            }
            let text = lines.join("\n");
            if failed {
                Err(Failure::Failed(text))
            } else {
                Ok(text)
            }
        }
        "setup" => {
            let words = parse(rest, &[], &[])?;
            let [id] = words.words.as_slice() else {
                return Err(Failure::Usage(format!(
                    "setup takes one plugin id\n\n{USAGE}"
                )));
            };
            install::setup_again(paths, id, output, cancel).map_err(Failure::from)
        }
        "new" | "init" => {
            let words = parse(rest, &[], &[])?;
            let (id, dir) = match words.words.as_slice() {
                [id] => (id, None),
                [id, dir] => (id, Some(Path::new(dir))),
                _ => {
                    return Err(Failure::Usage(format!(
                        "new takes an id and, if you like, a folder\n\n{USAGE}"
                    )));
                }
            };
            let author = git_author();
            let folder = scaffold::create(id, dir, &author)?;
            Ok(format!(
                "made {id} in {}\n  {}       load it from there\n  {}      what the registry would say\n  then push it to a public repository and {}",
                folder.display(),
                surface.command(&format!("link {}", folder.display())),
                surface.command(&format!("check {}", folder.display())),
                surface.command(&format!("submit {}", folder.display())),
            ))
        }
        "check" | "lint" => {
            let words = parse(rest, &[], &[])?;
            let dir = words.dir()?;
            let (text, ok) = check_report(&dir);
            if ok {
                Ok(text)
            } else {
                Err(Failure::Failed(text))
            }
        }
        "link" => {
            let words = parse(rest, &["force"], &[])?;
            let dir = words.dir()?;
            let (line, report) = install::link(paths, &dir, words.flag("force"))?;
            let mut lines = vec![line];
            lines.extend(report.lines());
            lines.push(loaded_note(paths, surface));
            Ok(lines
                .into_iter()
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "submit" | "publish" => {
            let words = parse(rest, &["print"], &["commit", "repository", "into"])?;
            let dir = words.dir()?;
            progress("fetching the plugin back from its repository, as an install will…");
            // Writing into a local registry file may name a private repository;
            // the public registry (a pull request, or --print for one) takes https.
            let public = words.value("into").is_none();
            let prepared = submit::prepare(
                paths,
                &dir,
                words.value("commit"),
                words.value("repository"),
                public,
                cancel,
            )?;
            let entry = &prepared.entry;
            let mut lines = vec![format!(
                "{} {} · {} · digest {}",
                entry.id,
                entry.version,
                submit::carries(&prepared.config),
                &entry.sha256[..16]
            )];
            lines.extend(prepared.report.lines());
            if words.flag("print") {
                lines.push(String::new());
                lines.push(registry::render(entry)?);
                return Ok(lines.join("\n"));
            }
            if let Some(file) = words.value("into") {
                lines.push(submit::into_file(&prepared, Path::new(file))?);
                return Ok(lines.join("\n"));
            }
            progress("opening the registry pull request…");
            let url = submit::open_pull_request(&prepared)?;
            lines.push(format!("pull request: {url}"));
            lines.push("a maintainer checks it, reads the code and merges".into());
            Ok(lines.join("\n"))
        }
        "help" | "-h" | "--help" => Ok(USAGE.trim_end().to_owned()),
        other => Err(Failure::Usage(format!(
            "unknown plugin command `{other}`\n\n{USAGE}"
        ))),
    }
}

fn git_author() -> String {
    std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// What `check` says, and whether it passed.
pub(crate) fn check_report(dir: &Path) -> (String, bool) {
    let load = check::folder(dir, Level::Load);
    let publish = check::folder(dir, Level::Publish);
    let name = load
        .config
        .as_ref()
        .map(|config| {
            let version = config
                .plugin
                .as_ref()
                .map(|plugin| format!(" {}", plugin.version))
                .unwrap_or_default();
            format!("{}{version}", config.id)
        })
        .unwrap_or_else(|| dir.display().to_string());
    let mut lines = Vec::new();
    if !load.ok() {
        lines.push(format!("{name} does not load as a plugin"));
        lines.extend(publish.lines());
        return (lines.join("\n"), false);
    }
    let carries = load
        .config
        .as_ref()
        .map(submit::carries)
        .unwrap_or_default();
    if publish.ok() {
        lines.push(format!(
            "{name} loads and is ready for the registry · {carries}"
        ));
    } else {
        lines.push(format!(
            "{name} loads, but the registry would refuse it · {carries}"
        ));
    }
    if let Ok(digest) = super::digest::digest_folder(dir) {
        lines.push(format!(
            "  folder digest {digest} (submit computes the registry's from the pushed commit)"
        ));
    }
    lines.extend(publish.lines());
    (lines.join("\n"), publish.ok())
}

fn loaded_note(paths: &Paths, surface: Surface) -> String {
    if paths.loaded() {
        String::new()
    } else {
        format!(
            "  ! ANGEL_CARTRIDGES does not include {}, so angelX will not load it{}",
            paths.cartridges.display(),
            if surface == Surface::Shell {
                " (add it to the list)"
            } else {
                ""
            }
        )
    }
}

/// The plugins installed here.
pub(crate) fn list(paths: &Paths, surface: Surface) -> String {
    let found = install::installed(paths);
    if found.is_empty() {
        return format!(
            "no plugins in {} · {} lists the registry · {} adds one",
            paths.cartridges.display(),
            surface.command("search"),
            surface.command("install <id>")
        );
    }
    let width = found.iter().map(|item| item.id.len()).max().unwrap_or(0);
    let mut lines = vec![format!("plugins in {}", paths.cartridges.display())];
    for item in &found {
        lines.push(format!("  {:width$}  {}", item.id, describe(item)));
    }
    lines.push(format!(
        "{} lists the registry · {} brings the latest",
        surface.command("search"),
        surface.command("update")
    ));
    lines.push(loaded_note(paths, surface));
    lines
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn describe(item: &Installed) -> String {
    let version = item.version().unwrap_or_else(|| "—".into());
    let what = match &item.config {
        Ok(config) => format!("{} · {}", config.label, submit::carries(config)),
        Err(error) => format!("does not load: {error}"),
    };
    let origin = match &item.origin {
        Origin::Registry(receipt) => {
            let setup = match receipt.setup_ok {
                Some(false) => " · setup did not finish",
                _ => "",
            };
            format!("registry at {}{setup}", install::short(&receipt.commit))
        }
        Origin::Linked(target) => format!("linked → {}", target.display()),
        Origin::Placed => "placed by hand".into(),
    };
    format!("{version:8} {what}  ({origin})")
}

fn load(paths: &Paths, progress: &dyn Fn(&str)) -> Result<Registry, String> {
    let registry = registry::load(paths)?;
    if let Some(error) = &registry.stale {
        progress(&format!("{error}; using the copy from the last fetch"));
    }
    Ok(registry)
}

/// The registry, or the entries matching every word of `query`.
pub(crate) fn search(paths: &Paths, query: &str, surface: Surface) -> Result<String, String> {
    let registry = load(paths, &|_| {})?;
    let hits = registry.search(query);
    let installed = install::installed(paths);
    let mut lines = Vec::new();
    if let Some(error) = &registry.stale {
        lines.push(format!("(offline: {error}; from the last fetch)"));
    }
    if hits.is_empty() {
        lines.push(if query.trim().is_empty() {
            format!("the registry ({}) lists no plugins yet", registry.source)
        } else {
            format!("no plugin in the registry matches `{}`", query.trim())
        });
        return Ok(lines.join("\n"));
    }
    lines.push(if query.trim().is_empty() {
        format!(
            "{} plugin{} in the registry",
            hits.len(),
            if hits.len() == 1 { "" } else { "s" }
        )
    } else {
        format!(
            "{} {} `{}`",
            hits.len(),
            if hits.len() == 1 {
                "plugin matches"
            } else {
                "plugins match"
            },
            query.trim()
        )
    });
    let width = hits.iter().map(|entry| entry.id.len()).max().unwrap_or(0);
    for entry in &hits {
        let here = installed
            .iter()
            .find(|item| item.id == entry.id)
            .map(|item| match (item.version(), &item.origin) {
                (Some(version), Origin::Registry(receipt)) if receipt.commit != entry.commit => {
                    format!("  · installed {version}, update available")
                }
                (_, Origin::Registry(_)) => "  · installed".to_owned(),
                _ => "  · a cartridge of that id is here".to_owned(),
            });
        lines.push(format!(
            "  {:width$}  {:8} {} — {}{}",
            entry.id,
            entry.version,
            entry.label,
            entry.description,
            here.unwrap_or_default()
        ));
    }
    lines.push(format!(
        "{} shows what one carries · {} installs it",
        surface.command("info <id>"),
        surface.command("install <id>")
    ));
    Ok(lines.join("\n"))
}

/// The registry's entry for `id`, held to the entry rules before anything is
/// fetched for it.
fn entry(registry: &Registry, id: &str, surface: Surface) -> Result<Entry, String> {
    let found = lookup(registry, id, surface)?;
    let problems = found.problems(false);
    if !problems.is_empty() {
        return Err(format!(
            "the registry's entry for {id} is malformed; nothing was fetched:\n  ✗ {}",
            problems.join("\n  ✗ ")
        ));
    }
    Ok(found)
}

fn lookup(registry: &Registry, id: &str, surface: Surface) -> Result<Entry, String> {
    registry.get(id).cloned().ok_or_else(|| {
        let near: Vec<&str> = registry
            .entries
            .iter()
            .filter(|entry| entry.id.contains(id) || id.contains(entry.id.as_str()))
            .map(|entry| entry.id.as_str())
            .collect();
        if near.is_empty() {
            format!(
                "no plugin `{id}` in the registry ({} searches it)",
                surface.command("search <words>")
            )
        } else {
            format!(
                "no plugin `{id}` in the registry; did you mean {}?",
                near.join(", ")
            )
        }
    })
}

/// Fetch a plugin and say what it carries, without installing it.
pub(crate) fn info(
    paths: &Paths,
    id: &str,
    surface: Surface,
    progress: &dyn Fn(&str),
    cancel: Option<&AtomicBool>,
) -> Result<String, String> {
    let registry = load(paths, progress)?;
    let entry = entry(&registry, id, surface)?;
    progress(&format!(
        "fetching {} {} from {}…",
        entry.id,
        entry.version,
        entry.source()
    ));
    let fetched = install::fetch(paths, &entry, cancel)?;
    let bytes: usize = fetched.files.iter().map(|file| file.bytes.len()).sum();
    let config = &fetched.config;
    let plugin = config.plugin.as_ref();
    let mut lines = vec![
        format!("{} {} · {}", entry.id, entry.version, entry.label),
        format!("  {}", entry.description),
        format!(
            "  source    {} at {}",
            entry.source(),
            install::short(&entry.commit)
        ),
        format!(
            "  files     {} ({}), digest matches the registry",
            fetched.files.len(),
            size(bytes)
        ),
        format!(
            "  license   {} · by {}",
            entry.license,
            entry.authors.join(", ")
        ),
        format!("  carries   {}", submit::carries(config)),
    ];
    if let Some(homepage) = &entry.homepage {
        lines.push(format!("  homepage  {homepage}"));
    }
    if let Some(required) = plugin.and_then(|plugin| plugin.angelx.as_deref()) {
        lines.push(format!(
            "  needs     angelX {required} or newer (this is {})",
            super::running_version()
        ));
    }
    if let Some(setup) = plugin.and_then(|plugin| plugin.setup.as_deref()) {
        lines.push(format!(
            "  setup     {setup} runs after install, in its folder — read it: {}",
            setup_link(&entry, setup)
        ));
    }
    for tool in &config.tools {
        lines.push(format!("  tool      {} — {}", tool.name, tool.description));
    }
    for warning in &fetched.report.warnings {
        lines.push(format!("  ! {warning}"));
    }
    lines.push(match install::find(paths, &entry.id) {
        Some(item) => match &item.origin {
            Origin::Registry(receipt) if receipt.commit == entry.commit => {
                "  installed, up to date".into()
            }
            Origin::Registry(receipt) => format!(
                "  installed {} · {} brings {}",
                receipt.version,
                surface.command(&format!("update {}", entry.id)),
                entry.version
            ),
            Origin::Linked(target) => format!("  linked here from {}", target.display()),
            Origin::Placed => format!(
                "  a cartridge `{}` is already here, placed by hand",
                entry.id
            ),
        },
        None => format!(
            "  {} installs it",
            surface.command(&format!("install {}", entry.id))
        ),
    });
    Ok(lines.join("\n"))
}

/// A web address for a file of the plugin, where the host has a known shape.
fn setup_link(entry: &Entry, file: &str) -> String {
    let path = if entry.path.is_empty() {
        file.to_owned()
    } else {
        format!("{}/{file}", entry.path)
    };
    let repository = entry
        .repository
        .trim_end_matches('/')
        .trim_end_matches(".git");
    if repository.starts_with("https://github.com/")
        || repository.starts_with("https://codeberg.org/")
        || repository.starts_with("https://gitlab.com/")
    {
        let blob = if repository.starts_with("https://gitlab.com/") {
            "-/blob"
        } else if repository.starts_with("https://codeberg.org/") {
            "src/commit"
        } else {
            "blob"
        };
        format!("{repository}/{blob}/{}/{path}", entry.commit)
    } else {
        format!("{path} at {}", install::short(&entry.commit))
    }
}

fn size(bytes: usize) -> String {
    match bytes {
        0..1_000 => format!("{bytes} B"),
        1_000..1_000_000 => format!("{:.1} KB", bytes as f64 / 1e3),
        _ => format!("{:.1} MB", bytes as f64 / 1e6),
    }
}

#[allow(clippy::too_many_arguments)]
fn install_all(
    paths: &Paths,
    ids: &[String],
    force: bool,
    skip_setup: bool,
    surface: Surface,
    progress: &dyn Fn(&str),
    output: Output<'_>,
    cancel: Option<&AtomicBool>,
) -> Result<String, String> {
    let registry = load(paths, progress)?;
    let entries = ids
        .iter()
        .map(|id| entry(&registry, id, surface))
        .collect::<Result<Vec<_>, _>>()?;
    let mut lines = Vec::new();
    let sink = match &output {
        Output::Sink(sink) => Some(*sink),
        Output::Inherit => None,
    };
    for entry in &entries {
        lines.push(install_one(
            paths,
            entry,
            &registry.source,
            force,
            skip_setup,
            surface,
            progress,
            sink,
            cancel,
        )?);
    }
    let note = loaded_note(paths, surface);
    if !note.is_empty() {
        lines.push(note);
    }
    Ok(lines.join("\n"))
}

#[allow(clippy::too_many_arguments)]
fn install_one(
    paths: &Paths,
    entry: &Entry,
    registry: &str,
    force: bool,
    skip_setup: bool,
    surface: Surface,
    progress: &dyn Fn(&str),
    sink: Option<&(dyn Fn(&[u8]) + Send + Sync)>,
    cancel: Option<&AtomicBool>,
) -> Result<String, String> {
    if crate::agent::harness::cartridges::compiled(&entry.id) {
        return Err(format!(
            "{} is compiled into this angelX as a source cartridge; a plugin cannot take its id",
            entry.id
        ));
    }
    progress(&format!(
        "fetching {} {} from {}…",
        entry.id,
        entry.version,
        entry.source()
    ));
    let output = match sink {
        Some(sink) => Output::Sink(sink),
        None => Output::Inherit,
    };
    let outcome = install::install(paths, entry, registry, force, skip_setup, output, cancel)?;
    let verb = match &outcome.replaced {
        Some(from) if from != &outcome.version => {
            format!("updated {} {from} → {}", outcome.id, outcome.version)
        }
        Some(_) => format!("reinstalled {} {}", outcome.id, outcome.version),
        None => format!("installed {} {}", outcome.id, outcome.version),
    };
    let mut lines = vec![
        format!("{verb} into {}", outcome.folder.display()),
        format!("  carries {}", submit::carries(&outcome.config)),
    ];
    match &outcome.setup {
        Some(Ok(())) => lines.push("  setup finished".into()),
        Some(Err(error)) => lines.push(format!(
            "  ! {error}; the files are in place — {} runs it again",
            surface.command(&format!("setup {}", outcome.id))
        )),
        None if skip_setup
            && outcome
                .config
                .plugin
                .as_ref()
                .and_then(|plugin| plugin.setup.as_ref())
                .is_some() =>
        {
            lines.push(format!(
                "  setup skipped; {} runs it",
                surface.command(&format!("setup {}", outcome.id))
            ))
        }
        None => {}
    }
    for note in &outcome.notes {
        lines.push(format!("  ! {note}"));
    }
    for clash in clashes(&outcome.config) {
        lines.push(format!("  ! {clash}"));
    }
    Ok(lines.join("\n"))
}

/// Commands and tools this cartridge shares with another plugged-in one.
fn clashes(config: &crate::agent::harness::cartridges::config::Config) -> Vec<String> {
    let mut found = Vec::new();
    for other in crate::agent::harness::cartridges::all() {
        if other.id() == config.id {
            continue;
        }
        for command in &config.commands {
            if other
                .config
                .commands
                .iter()
                .any(|theirs| theirs.name == command.name)
            {
                found.push(format!(
                    "/{} is also {}'s; the one loaded first answers",
                    command.name,
                    other.id()
                ));
            }
        }
        for tool in &config.tools {
            if other
                .config
                .tools
                .iter()
                .any(|theirs| theirs.name == tool.name)
            {
                found.push(format!(
                    "tool {} is also {}'s; the one loaded first keeps the name",
                    tool.name,
                    other.id()
                ));
            }
        }
    }
    found
}

#[allow(clippy::too_many_arguments)]
fn update(
    paths: &Paths,
    ids: &[String],
    skip_setup: bool,
    surface: Surface,
    progress: &dyn Fn(&str),
    output: Output<'_>,
    cancel: Option<&AtomicBool>,
) -> Result<String, String> {
    let registry = load(paths, progress)?;
    let installed = install::installed(paths);
    let targets: Vec<&Installed> = if ids.is_empty() {
        installed
            .iter()
            .filter(|item| matches!(item.origin, Origin::Registry(_)))
            .collect()
    } else {
        ids.iter()
            .map(|id| {
                installed
                    .iter()
                    .find(|item| &item.id == id)
                    .ok_or_else(|| format!("{id} is not installed"))
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    if targets.is_empty() {
        return Ok(format!(
            "no plugins from the registry are installed · {} lists it",
            surface.command("search")
        ));
    }
    let sink = match &output {
        Output::Sink(sink) => Some(*sink),
        Output::Inherit => None,
    };
    let mut lines = Vec::new();
    let mut failures = Vec::new();
    for item in targets {
        let Origin::Registry(receipt) = &item.origin else {
            lines.push(format!(
                "{} is {}; it updates where it came from",
                item.id,
                match &item.origin {
                    Origin::Linked(target) => format!("linked from {}", target.display()),
                    _ => "placed by hand".into(),
                }
            ));
            continue;
        };
        let Some(entry) = registry.get(&item.id) else {
            lines.push(format!(
                "{} is no longer in the registry; kept as it is",
                item.id
            ));
            continue;
        };
        if entry.commit == receipt.commit && entry.sha256 == receipt.sha256 {
            lines.push(format!("{} {} is up to date", item.id, receipt.version));
            continue;
        }
        match install_one(
            paths,
            entry,
            &registry.source,
            false,
            skip_setup,
            surface,
            progress,
            sink,
            cancel,
        ) {
            Ok(text) => lines.push(text),
            Err(error) => failures.push(format!("{}: {error}", item.id)),
        }
    }
    lines.extend(failures.iter().cloned());
    if failures.is_empty() {
        Ok(lines.join("\n"))
    } else {
        Err(lines.join("\n"))
    }
}
