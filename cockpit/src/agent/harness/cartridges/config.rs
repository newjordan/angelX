//! `cartridge.toml`: one competition family or one tool, in data. A
//! config-only cartridge is just this file (and, for commands, its `skills/`);
//! a source cartridge adds Rust hooks beside it (`src/`).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) boards: Vec<String>,
    #[serde(default)]
    pub(crate) measures: Vec<String>,
    #[serde(default)]
    pub(crate) worker: String,
    #[serde(default)]
    pub(crate) status_check: Option<String>,
    #[serde(default)]
    pub(crate) fleet_interval_secs: Option<u64>,
    #[serde(default)]
    pub(crate) fleet_command: Option<String>,
    #[serde(default)]
    pub(crate) pages: Vec<Page>,
    #[serde(default)]
    pub(crate) commands: Vec<Command>,
    #[serde(default)]
    pub(crate) tools: Vec<ProgramTool>,
    #[serde(default)]
    pub(crate) plugin: Option<PluginMeta>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PluginMeta {
    pub(crate) version: String,
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) authors: Vec<String>,
    /// An SPDX license expression, such as `MIT`.
    #[serde(default)]
    pub(crate) license: String,
    #[serde(default)]
    pub(crate) repository: Option<String>,
    #[serde(default)]
    pub(crate) homepage: Option<String>,
    #[serde(default)]
    pub(crate) keywords: Vec<String>,
    #[serde(default)]
    pub(crate) angelx: Option<String>,
    #[serde(default)]
    pub(crate) setup: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Command {
    pub(crate) name: String,
    pub(crate) skills: Vec<String>,
    #[serde(default)]
    pub(crate) usage: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProgramTool {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) run: String,
    #[serde(default)]
    pub(crate) params: serde_json::Value,
    #[serde(default)]
    pub(crate) core: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Page {
    pub(crate) route: String,
    pub(crate) name: String,
    pub(crate) signal: String,
    #[serde(default)]
    pub(crate) action: String,
    pub(crate) pages: Vec<String>,
}

impl Config {
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if config.id.trim().is_empty() || config.label.trim().is_empty() {
            return Err("a cartridge needs an id and a label".into());
        }
        for page in &config.pages {
            let cells: Vec<char> = page.route.chars().collect();
            if cells.len() != 2 || !cells.iter().all(|c| ('\u{2800}'..='\u{28FF}').contains(c)) {
                return Err(format!("page route `{}` is not two braille cells", page.route));
            }
            if page.pages.is_empty() {
                return Err(format!("page route `{}` has no pages", page.route));
            }
        }
        for command in &config.commands {
            let word = !command.name.is_empty()
                && command
                    .name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !word {
                return Err(format!(
                    "command `{}` is not a lowercase word (a-z, 0-9, -)",
                    command.name
                ));
            }
            if command.skills.is_empty() {
                return Err(format!("command `{}` names no skills", command.name));
            }
        }
        for tool in &config.tools {
            let word = !tool.name.is_empty()
                && tool
                    .name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if !word {
                return Err(format!(
                    "tool `{}` is not a lowercase name (a-z, 0-9, _)",
                    tool.name
                ));
            }
            if tool.description.trim().is_empty() {
                return Err(format!("tool `{}` has no description", tool.name));
            }
            if !inside_folder(&tool.run) {
                return Err(format!(
                    "tool `{}` runs `{}`, not a path inside the cartridge",
                    tool.name, tool.run
                ));
            }
            if !(tool.params.is_null() || tool.params.is_object()) {
                return Err(format!("tool `{}` params are not a JSON schema object", tool.name));
            }
        }
        if let Some(setup) = config
            .plugin
            .as_ref()
            .and_then(|plugin| plugin.setup.as_deref())
            && !inside_folder(setup)
        {
            return Err(format!(
                "plugin setup `{setup}` is not a path inside the cartridge"
            ));
        }
        Ok(config)
    }
}

fn inside_folder(path: &str) -> bool {
    let run = Path::new(path);
    !path.is_empty()
        && run.is_relative()
        && run
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

pub(crate) fn roots() -> Vec<PathBuf> {
    match std::env::var_os("ANGEL_CARTRIDGES") {
        Some(list) => std::env::split_paths(&list).collect(),
        None => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".angelX").join("cartridges"))
            .into_iter()
            .collect(),
    }
}

pub(crate) fn folders(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in roots {
        if is_cartridge(root) {
            found.push(root.clone());
            continue;
        }
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| is_cartridge(path))
            .collect();
        dirs.sort();
        found.extend(dirs);
    }
    found
}

fn is_cartridge(path: &Path) -> bool {
    path.join("cartridge.toml").is_file()
}

pub(crate) fn has_source(path: &Path) -> bool {
    path.join("src").join("mod.rs").is_file()
}
