//! `angelX plugin check`: whether a folder loads as a plugin, and whether the
//! registry would take it.
//!
//! Two levels. [`Level::Load`] is what an install or a link needs: the
//! cartridge parses, it is config-only, every command's skills and every
//! tool's program are there, nothing points outside the folder.
//! [`Level::Publish`] adds what the registry lists: a `[plugin]` table with a
//! version, a description, authors and a license.

use super::{Version, digest, install::RECEIPT, valid_id};
use crate::agent::harness::cartridges::config::Config;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Level {
    Load,
    Publish,
}

/// What a check found.
#[derive(Debug, Default)]
pub(crate) struct Report {
    pub(crate) errors: Vec<String>,
    pub(crate) warnings: Vec<String>,
    /// The parsed `cartridge.toml`, when it parsed.
    pub(crate) config: Option<Config>,
}

impl Report {
    pub(crate) fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// The findings, one per line, errors first.
    pub(crate) fn lines(&self) -> Vec<String> {
        self.errors
            .iter()
            .map(|error| format!("  ✗ {error}"))
            .chain(self.warnings.iter().map(|warning| format!("  ! {warning}")))
            .collect()
    }
}

/// Check the plugin in `dir`.
pub(crate) fn folder(dir: &Path, level: Level) -> Report {
    let mut report = Report::default();
    let toml = dir.join("cartridge.toml");
    let text = match std::fs::read_to_string(&toml) {
        Ok(text) => text,
        Err(error) => {
            report.errors.push(format!("cartridge.toml: {error}"));
            return report;
        }
    };
    let config = match Config::parse(&text) {
        Ok(config) => config,
        Err(error) => {
            report.errors.push(format!("cartridge.toml: {error}"));
            // A top-level key written below `[plugin]` belongs to that table.
            if error.contains("expected one of `version`, `description`") {
                report.errors.push(
                    "cartridge.toml: every key under a [table] belongs to it; put [plugin] after the top-level keys".into(),
                );
            }
            return report;
        }
    };
    let errors = &mut report.errors;
    let warnings = &mut report.warnings;
    if !valid_id(&config.id) {
        errors.push(format!(
            "id `{}`: a plugin id is 2-32 lowercase letters, digits and `-`, starting with a letter",
            config.id
        ));
    }
    if crate::agent::harness::cartridges::config::has_source(dir) {
        errors.push("src/mod.rs makes this a source cartridge, which only a rebuild compiles in; a plugin is config-only".into());
    }
    if dir.join(RECEIPT).exists() && level == Level::Publish {
        errors.push(format!(
            "{RECEIPT} is the install receipt's name; a plugin cannot carry one"
        ));
    }
    let carries = !config.commands.is_empty()
        || !config.tools.is_empty()
        || !config.pages.is_empty()
        || !config.boards.is_empty()
        || !config.worker.is_empty();
    if !carries {
        errors.push(
            "it carries nothing: no [[commands]], [[tools]], [[pages]], boards or worker".into(),
        );
    }
    let setup = config
        .plugin
        .as_ref()
        .and_then(|plugin| plugin.setup.clone());
    match &config.plugin {
        Some(plugin) => {
            if Version::parse(&plugin.version).is_none() {
                errors.push(format!(
                    "[plugin] version `{}` is not MAJOR.MINOR.PATCH",
                    plugin.version
                ));
            }
            if let Some(problem) = super::registry::description_problem(&plugin.description) {
                errors.push(format!("[plugin] {problem}"));
            }
            if let Some(required) = &plugin.angelx {
                match Version::parse(required) {
                    None => errors.push(format!("[plugin] angelx `{required}` is not a version")),
                    Some(required)
                        if Version::parse(super::running_version())
                            .is_some_and(|running| running < required) =>
                    {
                        warnings.push(format!(
                            "[plugin] angelx = \"{}\": this angelX is {}, so it would refuse to install",
                            plugin.angelx.as_deref().unwrap_or(""),
                            super::running_version()
                        ));
                    }
                    Some(_) => {}
                }
            }
            if level == Level::Publish {
                if plugin.license.trim().is_empty() {
                    errors.push("[plugin] license: name one, such as \"MIT\"".into());
                }
                if plugin.authors.iter().all(|author| author.trim().is_empty()) {
                    errors.push("[plugin] authors: name at least one".into());
                }
            }
            if let Some(repository) = &plugin.repository
                && !repository.starts_with("https://")
            {
                errors.push(format!(
                    "[plugin] repository `{repository}` is not an https URL"
                ));
            }
        }
        None if level == Level::Publish => {
            errors.push("no [plugin] table: the registry needs its version, description, authors and license".into());
        }
        None => {}
    }
    // Commands: each skill is in skills/, and no built-in hides the word.
    let skills = crate::agent::harness::load_skills_from(&dir.join("skills"));
    let mut words = std::collections::BTreeSet::new();
    for command in &config.commands {
        if !words.insert(command.name.as_str()) {
            errors.push(format!("/{} is declared twice", command.name));
        }
        if crate::ui::input::builtin_command(&command.name) {
            errors.push(format!(
                "/{} is one of angelX's own commands, which always wins; choose another word",
                command.name
            ));
        }
        for skill in &command.skills {
            if skills.iter().any(|found| &found.name == skill) {
                continue;
            }
            if setup.is_some() {
                warnings.push(format!(
                    "/{}: skill `{skill}` is not in skills/; the setup must make it",
                    command.name
                ));
            } else {
                errors.push(format!(
                    "/{}: skill `{skill}` is not in skills/ (skills/{skill}/SKILL.md or skills/{skill}.md)",
                    command.name
                ));
            }
        }
        if command.usage.trim().is_empty() {
            warnings.push(format!(
                "/{} has no usage line; the composer shows it after the word",
                command.name
            ));
        }
    }
    // Tools: each program is a file in the folder that may run.
    let mut names = std::collections::BTreeSet::new();
    for tool in &config.tools {
        if !names.insert(tool.name.as_str()) {
            errors.push(format!("tool `{}` is declared twice", tool.name));
        }
        program(
            dir,
            &tool.run,
            &format!("tool `{}`", tool.name),
            setup.is_some(),
            errors,
            warnings,
        );
    }
    if let Some(setup) = &setup {
        program(dir, setup, "[plugin] setup", false, errors, warnings);
    }
    // Links stay inside; nothing in the tree is a device or a pipe.
    match digest::read_folder(dir) {
        Ok(entries) => {
            for entry in entries
                .iter()
                .filter(|entry| entry.kind == digest::Kind::Link)
            {
                let target = String::from_utf8_lossy(&entry.bytes);
                if !link_inside(&entry.path, &target) {
                    errors.push(format!(
                        "{} links to `{target}`, outside the plugin",
                        entry.path
                    ));
                }
            }
        }
        Err(error) => errors.push(error),
    }
    if level == Level::Publish {
        if !dir.join("README.md").is_file() {
            warnings
                .push("no README.md: say what it does, what it needs, and how to use it".into());
        }
        if !["LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"]
            .iter()
            .any(|name| dir.join(name).is_file())
        {
            warnings.push("no LICENSE file beside cartridge.toml".into());
        }
    }
    report.config = Some(config);
    report
}

fn program(
    dir: &Path,
    run: &str,
    what: &str,
    made_by_setup: bool,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let path = dir.join(run);
    match std::fs::metadata(&path) {
        Ok(meta) if meta.is_file() => {
            if !runnable(&meta) {
                errors.push(format!(
                    "{what}: {run} is not executable (chmod +x {run}, and commit the mode)"
                ));
            }
        }
        Ok(_) => errors.push(format!("{what}: {run} is not a file")),
        Err(_) if made_by_setup => warnings.push(format!(
            "{what}: {run} is not in the folder; the setup must make it"
        )),
        Err(_) => errors.push(format!("{what}: {run} is not in the folder")),
    }
}

#[cfg(unix)]
fn runnable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o100 != 0
}

#[cfg(not(unix))]
fn runnable(_meta: &std::fs::Metadata) -> bool {
    true
}

/// Whether a link at `path` (relative to the plugin) to `target` stays in the
/// plugin, by its words alone.
pub(crate) fn link_inside(path: &str, target: &str) -> bool {
    if target.is_empty() || target.starts_with('/') {
        return false;
    }
    let mut depth: i64 = path.split('/').count() as i64 - 1;
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => depth += 1,
        }
    }
    true
}
