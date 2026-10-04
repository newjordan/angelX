//! `cartridge.toml`: one competition family, in data. A config-only cartridge
//! is just this file; a source cartridge adds Rust hooks beside it (`src/`).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// Stable id; `ANGEL_CARTRIDGE` selects the active cartridge by it.
    pub(crate) id: String,
    /// Name on operator surfaces.
    pub(crate) label: String,
    /// The board CLI's program names. `<board> submit` carries angelX's own
    /// `--model` and `--harness`; `<board> submit|submissions|status|list`
    /// count as competition outcomes.
    #[serde(default)]
    pub(crate) boards: Vec<String>,
    /// Board subcommands that are this family's own local measurement
    /// (`<board> run`): a loop counts them as a measured candidate.
    #[serde(default)]
    pub(crate) measures: Vec<String>,
    /// The loop worker's brief: book page cells or plain words.
    #[serde(default)]
    pub(crate) worker: String,
    /// How a submission's result is read, as the prompts name it.
    #[serde(default)]
    pub(crate) status_check: Option<String>,
    /// Seconds between fleet sweeps.
    #[serde(default)]
    pub(crate) fleet_interval_secs: Option<u64>,
    /// A config-only fleet: a shell command whose stdout is one snapshot,
    /// `{"benchmarks": 2, "failed": 0, "submissions": [{"benchmark", "id",
    /// "status", "score"}]}`. `ANGEL_CARTRIDGE_DIR` names the cartridge's folder.
    #[serde(default)]
    pub(crate) fleet_command: Option<String>,
    /// Book pages, as data: a config-only cartridge speaks in pages too.
    #[serde(default)]
    pub(crate) pages: Vec<Page>,
}

/// One book page set a cartridge adds or replaces, by its two-cell route.
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
        Ok(config)
    }
}

/// Where cartridges are found: `ANGEL_CARTRIDGES` (a path list; each entry is
/// a cartridge or a folder of them), else `~/.angelX/cartridges`. `build.rs`
/// reads the same places to compile source cartridges in.
pub(crate) fn roots() -> Vec<PathBuf> {
    match std::env::var_os("ANGEL_CARTRIDGES") {
        Some(list) => std::env::split_paths(&list).collect(),
        None => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".angelX").join("cartridges"))
            .into_iter()
            .collect(),
    }
}

/// Every cartridge folder under `roots`, in order.
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

/// A folder whose Rust the build compiles in; at runtime only config-only
/// folders are read, so a source cartridge never runs without its hooks.
pub(crate) fn has_source(path: &Path) -> bool {
    path.join("src").join("mod.rs").is_file()
}
