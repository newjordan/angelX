//! Cartridges: competition families plugged into the harness.
//!
//! The harness ships the competition engine (the loop, the watcher, the fleet
//! HUD, truthful submit attribution, the book). A cartridge carries one
//! family's specifics: its board CLI, its loop worker's brief, its status API
//! and fleet sweep, its book pages. A cartridge is a folder with
//! `cartridge.toml`; with `src/mod.rs` beside it, `build.rs` compiles its Rust
//! hooks in, otherwise the toml alone is read at startup. Folders come from
//! `ANGEL_CARTRIDGES`, else `~/.angelX/cartridges`; `ANGEL_CARTRIDGE` picks the
//! active one by id, and the first found is the default. See docs/CARTRIDGES.md.

pub(crate) mod config;
pub(crate) mod fleet;
pub(crate) mod records;
pub(crate) mod submit_identity;

use crate::agent::harness::WatchNotify;
use crate::agent::harness::book::{Route, Sub};
use crate::agent::harness::comp_watch::StatusSource;
use config::Config;
use fleet::FleetSnapshot;
use records::{CandidateIdentity, TerminalEvidence};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

// `COMPILED`: each source cartridge's toml and hooks, written by build.rs.
include!(concat!(env!("OUT_DIR"), "/cartridges.rs"));

/// The Rust a source cartridge compiles in. Every hook is optional.
pub(crate) trait Hooks: Sync {
    /// Tools this family adds to every session.
    fn tools(&self, _workspace: &Path) -> Vec<Box<dyn crate::agent::harness::Tool>> {
        Vec::new()
    }
    /// Its tools that long research always sees (see `is_research_tool`).
    fn research_tools(&self) -> &'static [&'static str] {
        &[]
    }
    /// Book pages this family adds, or replaces by route.
    fn pages(&self) -> &'static [Sub] {
        &[]
    }
    /// The submission watch operator configuration adopts, if any.
    fn configured_watch(&self) -> Result<Option<(String, Box<dyn StatusSource + Send>)>, String> {
        Ok(None)
    }
    /// One fleet sweep, run off the UI thread; `None` when the family has no
    /// fleet in Rust.
    fn fleet_sweep(&self) -> Option<Result<FleetSnapshot, String>> {
        None
    }
    /// Whether `fleet_sweep` returns one.
    fn has_fleet(&self) -> bool {
        false
    }
    /// Evidence carried inline when a competition turn engages.
    fn seat_entry_focus(&self, _workspace: &Path) -> Option<String> {
        None
    }
    /// Persist a terminal result the watcher saw.
    fn record_terminal(&self, _workspace: &Path, _notify: &WatchNotify) -> Result<(), String> {
        Ok(())
    }
    /// Persist a submission the board accepted for evaluation.
    #[allow(clippy::too_many_arguments)]
    fn record_dispatch(
        &self,
        _workspace: &Path,
        _submission_id: &str,
        _identity: CandidateIdentity,
        _tool: &str,
        _note_sha256: Option<&str>,
        _model: Option<&str>,
        _harness: Option<&str>,
    ) -> Result<(), String> {
        Ok(())
    }
    /// Persisted terminal results, for loop reconciliation.
    fn terminal_results(&self, _workspace: &Path) -> Result<Vec<TerminalEvidence>, String> {
        Ok(Vec::new())
    }
    /// `angel --comp-status ARGS…`; `None` when the family has no such CLI.
    fn status_cli(&self, _args: &[String]) -> Option<Result<(), String>> {
        None
    }
}

/// A config-only cartridge's hooks: none.
struct NoHooks;
impl Hooks for NoHooks {}

/// One plugged-in competition family.
pub(crate) struct Cartridge {
    pub(crate) config: Config,
    hooks: &'static dyn Hooks,
    /// The hooks' pages, then the toml's, made once for the process.
    pages: &'static [Sub],
    /// A config-only cartridge's folder, which its commands run beside.
    folder: Option<std::path::PathBuf>,
}

impl Cartridge {
    pub(crate) fn new(config: Config, hooks: Option<&'static dyn Hooks>) -> Self {
        let hooks = hooks.unwrap_or(&NoHooks);
        let leak = |text: &str| -> &'static str { Box::leak(text.to_owned().into_boxed_str()) };
        let mut pages: Vec<Sub> = Vec::new();
        for page in &config.pages {
            let mut cells = page.route.chars();
            let (Some(primary), Some(sub)) = (cells.next(), cells.next()) else {
                continue;
            };
            let words: Vec<&'static str> = page.pages.iter().map(|p| leak(p)).collect();
            pages.push(Sub {
                route: Route::new(primary, sub),
                name: leak(&page.name),
                signal: leak(&page.signal),
                action: leak(&page.action),
                ideas: "",
                pages: Box::leak(words.into_boxed_slice()),
            });
        }
        let pages: &'static [Sub] = if pages.is_empty() {
            hooks.pages()
        } else {
            pages.splice(0..0, hooks.pages().iter().map(|sub| Sub { ..*sub }));
            Box::leak(pages.into_boxed_slice())
        };
        Self {
            config,
            hooks,
            pages,
            folder: None,
        }
    }

    /// Book pages this cartridge adds, or replaces by route.
    pub(crate) fn pages(&self) -> &'static [Sub] {
        self.pages
    }

    pub(crate) fn id(&self) -> &str {
        &self.config.id
    }

    pub(crate) fn label(&self) -> &str {
        &self.config.label
    }

    pub(crate) fn hooks(&self) -> &'static dyn Hooks {
        self.hooks
    }

    /// Whether `program operand` is this family's own local measurement; a
    /// loop counts it as a measured candidate like any `bench*` script.
    pub(crate) fn measures(&self, program: &str, operand: Option<&str>) -> bool {
        self.config.boards.iter().any(|board| board == program)
            && operand.is_some_and(|op| self.config.measures.iter().any(|m| m == op))
    }

    /// The loop worker's brief.
    pub(crate) fn worker(&self) -> &str {
        &self.config.worker
    }

    pub(crate) fn has_fleet(&self) -> bool {
        self.hooks.has_fleet() || self.config.fleet_command.is_some()
    }

    pub(crate) fn fleet_interval(&self) -> Duration {
        self.config
            .fleet_interval_secs
            .map_or(fleet::POLL_INTERVAL, Duration::from_secs)
    }

    pub(crate) fn fleet_sweep(&self) -> Result<FleetSnapshot, String> {
        if let Some(result) = self.hooks.fleet_sweep() {
            return result;
        }
        match &self.config.fleet_command {
            Some(command) => fleet::sweep_command(command, self.folder.as_deref()),
            None => Err(format!("{} has no fleet sweep", self.label())),
        }
    }
}

/// Every plugged-in cartridge: the compiled ones, then config-only folders.
pub(crate) fn all() -> &'static [Cartridge] {
    static ALL: OnceLock<Vec<Cartridge>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut found: Vec<Cartridge> = Vec::new();
        // Tests play the fixture board first, whatever else is plugged in.
        #[cfg(test)]
        found.push(Cartridge::new(test_board(), Some(&TestBoard)));
        for (text, hooks) in COMPILED {
            match Config::parse(text) {
                Ok(config) => found.push(Cartridge::new(config, Some(*hooks))),
                Err(error) => eprintln!("cartridge: a compiled cartridge.toml is invalid: {error}"),
            }
        }
        for folder in config::folders(&config::roots()) {
            if config::has_source(&folder) {
                continue;
            }
            let path = folder.join("cartridge.toml");
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| Config::parse(&text))
            {
                Ok(config) if found.iter().any(|c| c.id() == config.id) => {}
                Ok(config) => found.push(Cartridge {
                    folder: Some(folder.clone()),
                    ..Cartridge::new(config, None)
                }),
                Err(error) => eprintln!("cartridge: {}: {error}", path.display()),
            }
        }
        found
    })
}

/// The fixture cartridge engine tests run against: board CLI `board`.
#[cfg(test)]
pub(crate) fn test_board() -> Config {
    Config {
        id: "board".into(),
        label: "Board".into(),
        boards: vec!["board".into()],
        measures: vec!["run".into(), "validate".into()],
        ..Config::default()
    }
}

/// The fixture board's hooks: `ANGEL_WATCH_BOARD_SUBMISSION` configures a
/// watch whose board never answers, so the slot stays in flight.
#[cfg(test)]
struct TestBoard;

#[cfg(test)]
impl Hooks for TestBoard {
    fn configured_watch(&self) -> Result<Option<(String, Box<dyn StatusSource + Send>)>, String> {
        struct Offline;
        impl StatusSource for Offline {
            fn probe(
                &mut self,
                _id: &str,
            ) -> Result<crate::agent::harness::comp_watch::SlotSnapshot, String> {
                Err("the fixture board is offline".into())
            }
        }
        Ok(std::env::var("ANGEL_WATCH_BOARD_SUBMISSION")
            .ok()
            .map(|id| (id, Box::new(Offline) as Box<dyn StatusSource + Send>)))
    }
}

fn selected_id() -> Option<String> {
    ["ANGEL_CARTRIDGE", "ANGEL_COMP_PACKAGE"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok())
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
}

/// The active cartridge: `ANGEL_CARTRIDGE` by id, else the first plugged in.
/// `None` when no cartridge is plugged in; the engine then runs without one.
pub(crate) fn active() -> Option<&'static Cartridge> {
    let all = all();
    selected_id()
        .and_then(|id| all.iter().find(|c| c.id() == id))
        .or_else(|| all.first())
}

/// Set when `ANGEL_CARTRIDGE` names no plugged-in cartridge; the /loop start
/// notice carries it.
pub(crate) fn selection_error() -> Option<String> {
    let id = selected_id()?;
    if all().iter().any(|c| c.id() == id) {
        return None;
    }
    Some(match active() {
        Some(fallback) => format!("unknown cartridge `{id}`; using `{}`", fallback.id()),
        None => format!("unknown cartridge `{id}`; no cartridge is plugged in"),
    })
}

/// Every plugged-in board CLI's program name.
pub(crate) fn boards() -> impl Iterator<Item = &'static str> {
    all()
        .iter()
        .flat_map(|c| c.config.boards.iter().map(String::as_str))
}

/// How the prompts name reading a submission's result.
pub(crate) fn status_check() -> &'static str {
    active()
        .and_then(|c| c.config.status_check.as_deref())
        .unwrap_or("the platform CLI")
}

/// The active cartridge's book page for `route`, if it supplies one.
pub(crate) fn page(route: Route) -> Option<&'static Sub> {
    active()?.pages().iter().find(|sub| sub.route == route)
}

/// The active cartridge's pages on the primary `cell`.
pub(crate) fn pages_on(cell: char) -> impl Iterator<Item = &'static Sub> {
    active()
        .map_or(&[][..], |c| c.pages())
        .iter()
        .filter(move |sub| sub.route.primary == cell)
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/cartridges__tests.rs"]
mod tests;
