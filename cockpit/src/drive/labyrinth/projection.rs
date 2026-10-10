//! Cached native navigation over claims, never a new verification authority.
use super::{Map, load};
use crate::drive::research_workspace::{Entry, Place, State};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

#[derive(Default)]
pub(crate) struct Projection {
    root: PathBuf,
    checked: Option<Instant>,
    stamp: Option<(Option<SystemTime>, Option<SystemTime>)>,
    rows: Vec<Entry>,
}

impl Projection {
    pub(crate) fn refresh(&mut self, root: &Path) -> Vec<Entry> {
        if self.root != root {
            *self = Self {
                root: root.into(),
                ..Self::default()
            };
        }
        if self.checked.is_some_and(|at| at.elapsed().as_secs() < 2) {
            return self.rows.clone();
        }
        self.checked = Some(Instant::now());
        let modified = |relative: &str| {
            std::fs::symlink_metadata(root.join(relative))
                .ok()
                .and_then(|meta| meta.modified().ok())
        };
        let stamp = (
            modified("labyrinth/knowledge.json"),
            modified("labyrinth/angel/observations"),
        );
        if self.stamp == Some(stamp) {
            return self.rows.clone();
        }
        self.stamp = Some(stamp);
        self.rows = match load(root) {
            Ok(Some(map)) => map.entries(),
            Ok(None) => Vec::new(),
            Err(error) => vec![Entry::new(
                "labyrinth:diagnostic".into(),
                Place::Library,
                "Labyrinth unavailable",
                State::Recorded,
                &error,
                &error,
                "Labyrinth diagnostic",
            )],
        };
        self.rows.clone()
    }
}

impl Map {
    pub(crate) fn entries(&self) -> Vec<Entry> {
        let mut rows = vec![Entry::new(
            "labyrinth:overview".into(),
            Place::Library,
            "Labyrinth map",
            State::Recorded,
            &format!("{} nodes · showing at most 128 claims", self.nodes.len()),
            &format!(
                "{}\n\nAuthored tiers and review labels are claims; this view does not certify them. Use the labyrinth tool's plan/frontier/route actions to choose tests and inspect dependencies.",
                self.status()
            ),
            "Labyrinth research map",
        )];
        rows.extend(self.nodes.iter().enumerate().take(128).map(|(i, node)| {
            let id = node["id"].as_str().unwrap();
            let mut entry = Entry::new(
                format!("labyrinth:{id}"),
                Place::Library,
                node["title"].as_str().unwrap_or(id),
                State::Recorded,
                &format!(
                    "{} · {} · research claim",
                    self.node_passage(i).label(),
                    node["tier"].as_str().unwrap_or("question")
                ),
                &serde_json::to_string_pretty(node).unwrap_or_default(),
                "Labyrinth research map",
            );
            entry.parent = node["angel_task"]
                .as_str()
                .map(|id| format!("labyrinth:{id}"));
            entry.experiment_id = node["angel_measurement"]["id"]
                .as_str()
                .or_else(|| node["angel_submission"]["id"].as_str())
                .map(str::to_owned);
            entry.links = node["links"]
                .as_array()
                .into_iter()
                .flatten()
                .take(16)
                .map(|link| {
                    (
                        format!("labyrinth:{}", link["to"].as_str().unwrap()),
                        link["rel"].as_str().unwrap().to_owned(),
                    )
                })
                .collect();
            entry
        }));
        rows
    }
}
