//! Plugins: cartridges anyone can publish, find and install (docs/PLUGINS.md).
//!
//! A plugin is a config-only cartridge (`cartridge.toml` with a `[plugin]`
//! table, its `skills/`, its programs) in a public git repository. The
//! registry, `plugins/registry.toml` in the angelX repository, pins each one
//! to a commit and the digest of its files there; a submission is a pull
//! request adding an entry, which `angelX plugin submit` opens.
//!
//! An install fetches only the plugin's folder at the pinned commit, checks
//! the digest before anything in it runs, writes it into
//! `~/.angelX/cartridges/<id>` beside a receipt, then runs the plugin's
//! setup, if it has one. The cartridge loader does the rest.

pub(crate) mod check;
pub(crate) mod cli;
pub(crate) mod digest;
pub(crate) mod git;
pub(crate) mod install;
pub(crate) mod registry;
pub(crate) mod scaffold;
pub(crate) mod submit;

use std::path::PathBuf;

/// The registry angelX reads when `ANGEL_PLUGIN_REGISTRY` names no other.
pub(crate) const DEFAULT_REGISTRY: &str =
    "https://raw.githubusercontent.com/newjordan/angelX/main/plugins/registry.toml";
/// The repository submissions open their pull requests on.
pub(crate) const UPSTREAM: &str = "newjordan/angelX";
/// The registry's path inside that repository.
pub(crate) const REGISTRY_FILE: &str = "plugins/registry.toml";

/// Where plugins go and where the manager keeps its own files.
#[derive(Clone, Debug)]
pub(crate) struct Paths {
    /// The folder plugins are installed into: one cartridge folder each.
    pub(crate) cartridges: PathBuf,
    /// The registry cache and the staging area for fetches.
    pub(crate) state: PathBuf,
    /// `ANGEL_PLUGIN_REGISTRY`, else the default: a URL or a local file.
    pub(crate) registry: String,
}

impl Paths {
    /// `ANGEL_PLUGIN_DIR` (else `~/.angelX/cartridges`), `~/.angelX/plugins`
    /// and `ANGEL_PLUGIN_REGISTRY` (else [`DEFAULT_REGISTRY`]).
    pub(crate) fn from_env() -> Self {
        let home = crate::platform::workspace_store::angel_dir();
        let cartridges = std::env::var_os("ANGEL_PLUGIN_DIR")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("cartridges"));
        let registry = std::env::var("ANGEL_PLUGIN_REGISTRY")
            .ok()
            .map(|source| source.trim().to_owned())
            .filter(|source| !source.is_empty())
            .unwrap_or_else(|| DEFAULT_REGISTRY.to_owned());
        Self {
            cartridges,
            state: home.join("plugins"),
            registry,
        }
    }

    /// Whether the cartridge loader reads [`Self::cartridges`]; with
    /// `ANGEL_CARTRIDGES` pointing elsewhere, installed plugins would not load.
    pub(crate) fn loaded(&self) -> bool {
        let canonical = |path: &std::path::Path| path.canonicalize().unwrap_or(path.to_path_buf());
        let target = canonical(&self.cartridges);
        crate::agent::harness::cartridges::config::roots()
            .iter()
            .any(|root| canonical(root) == target)
    }
}

/// A version: `MAJOR.MINOR.PATCH`, with an optional `-pre` suffix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Version {
    numbers: [u64; 3],
    pre: Option<String>,
}

impl Version {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        if pre.is_some_and(|pre| {
            pre.is_empty()
                || !pre
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        }) {
            return None;
        }
        let mut numbers = [0u64; 3];
        let mut parts = core.split('.');
        for slot in &mut numbers {
            let part = parts.next()?;
            if part.is_empty()
                || !part.chars().all(|c| c.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
            {
                return None;
            }
            *slot = part.parse().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            numbers,
            pre: pre.map(str::to_owned),
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    /// Numbers first; a pre-release comes before its release.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.numbers
            .cmp(&other.numbers)
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(left), Some(right)) => left.cmp(right),
            })
    }
}

/// This binary's version, which a plugin's `angelx` requirement is held to.
pub(crate) fn running_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// A plugin id: a lowercase word of 2 to 32 letters, digits and `-`,
/// starting with a letter. It names the plugin's folder.
pub(crate) fn valid_id(id: &str) -> bool {
    (2..=32).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.ends_with('-')
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/plugins/plugins__tests.rs"]
mod tests;
