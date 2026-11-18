//! `plugins/registry.toml`: every published plugin, one table per id.
//!
//! ```toml
//! [vfx]
//! label = "Bend Pixel VFX"
//! version = "1.0.0"
//! description = "Pixel-art VFX sheets rendered locally with Bend."
//! repository = "https://github.com/newjordan/angelX"
//! commit = "<40 hex>"
//! path = "docs/examples/cartridges/vfx"
//! sha256 = "<the plugin tree digest at that commit>"
//! license = "MIT"
//! authors = ["Jordan Newman"]
//! ```
//!
//! A table's key is the plugin's id; TOML refuses a key twice, so an id is
//! taken once. Entries stay in id order so that two submissions touch two
//! places in the file. [`upsert`] edits the text and keeps every comment.

use super::{Paths, Version, git, valid_id};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    #[serde(skip)]
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) repository: String,
    pub(crate) commit: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) license: String,
    pub(crate) authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) homepage: Option<String>,
}

impl Entry {
    pub(crate) fn problems(&self, public: bool) -> Vec<String> {
        let mut problems = Vec::new();
        let id = &self.id;
        if !valid_id(id) {
            problems.push(format!(
                "`{id}`: an id is 2-32 lowercase letters, digits and `-`, starting with a letter"
            ));
        }
        if self.label.trim().is_empty() {
            problems.push(format!("{id}: no label"));
        }
        if Version::parse(&self.version).is_none() {
            problems.push(format!(
                "{id}: version `{}` is not MAJOR.MINOR.PATCH",
                self.version
            ));
        }
        problems.extend(
            description_problem(&self.description).map(|problem| format!("{id}: {problem}")),
        );
        if public {
            if !self.repository.starts_with("https://") || !git::fetchable(&self.repository) {
                problems.push(format!(
                    "{id}: repository `{}` is not a public https URL",
                    self.repository
                ));
            } else if self
                .repository
                .split("://")
                .nth(1)
                .is_some_and(|rest| rest.split('/').next().unwrap_or("").contains('@'))
            {
                problems.push(format!(
                    "{id}: repository `{}` carries credentials",
                    self.repository
                ));
            }
        } else if !git::fetchable(&self.repository) {
            problems.push(format!(
                "{id}: repository `{}` is not an https, http or file URL, or an absolute path",
                self.repository
            ));
        }
        if !git::valid_commit(&self.commit) {
            problems.push(format!(
                "{id}: commit `{}` is not a full 40-character commit id",
                self.commit
            ));
        }
        if !self.path.is_empty() && !super::digest::safe_path(&self.path) {
            problems.push(format!(
                "{id}: path `{}` is not a folder inside the repository",
                self.path
            ));
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        {
            problems.push(format!(
                "{id}: sha256 `{}` is not 64 lowercase hex digits",
                self.sha256
            ));
        }
        if self.license.trim().is_empty() {
            problems.push(format!("{id}: no license"));
        }
        if self.authors.iter().all(|author| author.trim().is_empty()) {
            problems.push(format!("{id}: no authors"));
        }
        problems
    }

    pub(crate) fn matches(&self, query: &str) -> bool {
        let haystack = format!(
            "{} {} {} {}",
            self.id,
            self.label,
            self.description,
            self.keywords.join(" ")
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|word| haystack.contains(&word.to_lowercase()))
    }

    pub(crate) fn source(&self) -> String {
        if self.path.is_empty() {
            self.repository.clone()
        } else {
            format!("{} ({})", self.repository, self.path)
        }
    }
}

pub(crate) fn description_problem(description: &str) -> Option<String> {
    if description.trim().is_empty() {
        Some("no description".into())
    } else if description.contains('\n') {
        Some("the description is more than one line".into())
    } else if description.chars().count() > 200 {
        Some("the description is longer than 200 characters".into())
    } else {
        None
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Registry {
    pub(crate) entries: Vec<Entry>,
    pub(crate) source: String,
    pub(crate) stale: Option<String>,
}

impl Registry {
    pub(crate) fn parse(text: &str, source: &str) -> Result<Self, String> {
        let tables: BTreeMap<String, Entry> =
            toml::from_str(text).map_err(|e| format!("{source}: {e}"))?;
        let entries = tables
            .into_iter()
            .map(|(id, entry)| Entry { id, ..entry })
            .collect();
        Ok(Self {
            entries,
            source: source.to_owned(),
            stale: None,
        })
    }

    pub(crate) fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub(crate) fn search(&self, query: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.matches(query))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn problems(&self, public: bool) -> Vec<String> {
        let mut problems: Vec<String> = self
            .entries
            .iter()
            .flat_map(|entry| entry.problems(public))
            .collect();
        let mut seen: BTreeMap<(String, String), &str> = BTreeMap::new();
        for entry in &self.entries {
            let key = (
                entry.repository.trim_end_matches(".git").to_lowercase(),
                entry.path.clone(),
            );
            if let Some(first) = seen.insert(key, &entry.id) {
                problems.push(format!(
                    "{} and {first} are the same folder of the same repository",
                    entry.id
                ));
            }
        }
        problems
    }
}

pub(crate) fn load(paths: &Paths) -> Result<Registry, String> {
    let source = paths.registry.as_str();
    let cache = paths.state.join("registry.toml");
    let fetched = read_source(source);
    match fetched {
        Ok(text) => {
            let registry = Registry::parse(&text, source)?;
            if source.starts_with("https://") || source.starts_with("http://") {
                let _ = std::fs::create_dir_all(&paths.state);
                let _ = crate::platform::workspace_store::write_atomic(&cache, text.as_bytes());
            }
            Ok(registry)
        }
        Err(error) => match std::fs::read_to_string(&cache) {
            Ok(text) if source.starts_with("https://") || source.starts_with("http://") => {
                let mut registry = Registry::parse(&text, &cache.display().to_string())?;
                registry.stale = Some(error);
                Ok(registry)
            }
            _ => Err(error),
        },
    }
}

fn read_source(source: &str) -> Result<String, String> {
    if source.starts_with("https://") || source.starts_with("http://") {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(20))
            .build();
        let response = agent
            .get(source)
            .set(
                "User-Agent",
                concat!("angelX/", env!("CARGO_PKG_VERSION"), " plugins"),
            )
            .call()
            .map_err(|e| format!("cannot read the plugin registry at {source}: {e}"))?;
        return response
            .into_string()
            .map_err(|e| format!("cannot read the plugin registry at {source}: {e}"));
    }
    let path = source.strip_prefix("file://").unwrap_or(source);
    std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read the plugin registry at {path}: {e}"))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Added,
    Updated(String),
    Unchanged,
}

pub(crate) fn upsert(text: &str, entry: &Entry) -> Result<(String, Change), String> {
    let current = Registry::parse(text, super::REGISTRY_FILE)?;
    let rendered = render(entry)?;
    let same_repository = |left: &str, right: &str| {
        left.trim_end_matches('/')
            .trim_end_matches(".git")
            .eq_ignore_ascii_case(right.trim_end_matches('/').trim_end_matches(".git"))
    };
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let header = |line: &str| table_key(line);
    if let Some(existing) = current.get(&entry.id) {
        if !same_repository(&existing.repository, &entry.repository) {
            return Err(format!(
                "the id `{}` is taken by {}; choose another id in cartridge.toml",
                entry.id, existing.repository
            ));
        }
        if existing == entry {
            return Ok((text.to_owned(), Change::Unchanged));
        }
        let start = lines
            .iter()
            .position(|line| header(line).as_deref() == Some(entry.id.as_str()))
            .ok_or_else(|| format!("cannot find the [{}] table in the registry text", entry.id))?;
        let end = section_end(&lines, start);
        let mut out: String = lines[..start].concat();
        out.push_str(&rendered);
        out.push_str(&lines[end..].concat());
        return Ok((out, Change::Updated(existing.version.clone())));
    }
    let at = lines
        .iter()
        .position(|line| header(line).is_some_and(|key| key.as_str() > entry.id.as_str()))
        .map(|index| comment_start(&lines, index));
    let mut out = String::new();
    match at {
        Some(index) => {
            out.push_str(&lines[..index].concat());
            out.push_str(&rendered);
            out.push('\n');
            out.push_str(&lines[index..].concat());
        }
        None => {
            out.push_str(text);
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
            out.push_str(&rendered);
        }
    }
    Registry::parse(&out, super::REGISTRY_FILE)?;
    Ok((out, Change::Added))
}

fn table_key(line: &str) -> Option<String> {
    let line = line.trim();
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    (!inner.starts_with('[')).then(|| inner.trim().trim_matches('"').to_owned())
}

fn section_end(lines: &[&str], start: usize) -> usize {
    let next = lines[start + 1..]
        .iter()
        .position(|line| table_key(line).is_some())
        .map_or(lines.len(), |offset| start + 1 + offset);
    let mut end = comment_start(lines, next);
    while end > start + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    end
}

fn comment_start(lines: &[&str], index: usize) -> usize {
    let mut at = index;
    while at > 0 && lines[at - 1].trim_start().starts_with('#') {
        at -= 1;
    }
    at
}

pub(crate) fn render(entry: &Entry) -> Result<String, String> {
    let mut table = BTreeMap::new();
    table.insert(entry.id.clone(), entry.clone());
    let body = toml::to_string(&table).map_err(|e| e.to_string())?;
    Ok(body)
}
