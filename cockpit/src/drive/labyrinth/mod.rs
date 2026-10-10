//! Labyrinth Exploration's research map, adapted for Angel's measured loops.
//! Upstream: nasqret/labyrinth-exploration, Bartosz Naskręcki (MIT).
//! Maps are local and opt-in. Research states never award gameplay rewards.

pub(crate) mod campaign;
mod cli;
pub(crate) mod entry;
mod failed_routes;
mod projection;
mod resources;
pub(crate) mod routing;
mod store;
pub(crate) mod workflow;

use crate::agent::harness::book::{self, labyrinth_campaign as legend};
pub(crate) use cli::cli;
pub(crate) use failed_routes::has_negative_attempt;
pub(crate) use projection::Projection;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
pub(crate) use store::{
    initialize, load, observe_iteration, observe_measurement, observe_submission,
};
pub(crate) const CONTEXT_MARK: &str = "[labyrinth-context]";

pub(super) const MAX_NODES: usize = 4096;
const KINDS: &[&str] = &[
    "theorem",
    "exhaustive",
    "evidence",
    "conjecture",
    "hunch",
    "question",
    "deadend",
    "family",
    "method",
    "source",
];
const RELATIONS: &[&str] = &[
    "uses",
    "supports",
    "refutes",
    "modifies",
    "generalizes",
    "suggests",
    "answers",
    "tests",
    "instance-of",
    "cites",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Passage {
    Charted,
    Door,
    Dark,
    Review,
    Wall,
}

impl Passage {
    fn label(self) -> &'static str {
        match self {
            Self::Charted => "charted",
            Self::Door => "door",
            Self::Dark => "dark",
            Self::Review => "under-review",
            Self::Wall => "wall",
        }
    }
    fn cost(self, explore: bool) -> Option<u32> {
        match self {
            Self::Charted => Some(1),
            Self::Door if explore => Some(4),
            Self::Review if explore => Some(6),
            Self::Dark if explore => Some(8),
            _ => None,
        }
    }
}

pub(crate) fn passage(node: &Value) -> Passage {
    let kind = node["kind"].as_str().unwrap_or_default();
    let status = node["status"].as_str().unwrap_or_default();
    let has_evidence = node["evidence"]
        .as_array()
        .is_some_and(|items| !items.is_empty());
    if kind == "deadend" || status == "refuted" {
        return if has_evidence {
            Passage::Wall
        } else {
            Passage::Review
        };
    }
    let reviewed = matches!(
        node["review"]["state"].as_str(),
        Some("refereed" | "human-checked")
    ) && node["review"]["by"]
        .as_array()
        .is_some_and(|by| !by.is_empty())
        && matches!(
            node["review"]["verdict"]
                .as_str()
                .map(str::to_ascii_uppercase)
                .as_deref(),
            Some("ESTABLISHED" | "PROVED" | "CORRECT" | "VALID" | "PASS" | "COMPUTED")
        );
    if matches!(kind, "theorem" | "exhaustive") && matches!(status, "established" | "proved") {
        match node["tier"].as_str() {
            Some("T1") if has_evidence => return Passage::Charted,
            Some("T2" | "T3") if reviewed && has_evidence => return Passage::Charted,
            _ => return Passage::Review,
        }
    }
    match kind {
        "question" | "conjecture" => Passage::Door,
        "hunch" => Passage::Dark,
        "evidence" => Passage::Review,
        // These are usable tools/references, not proved claims.
        "method" | "source" | "family" => Passage::Charted,
        _ => Passage::Review,
    }
}

pub(crate) struct Map {
    pub(super) document: Value,
    pub(super) nodes: Vec<Value>,
    index: BTreeMap<String, usize>,
    charted: Vec<bool>,
}

impl Map {
    pub(super) fn parse(document: Value) -> Result<Self, String> {
        if document.get("schema").is_some_and(|schema| schema != 1) {
            return Err("unsupported labyrinth schema (expected 1)".into());
        }
        let nodes = document["nodes"]
            .as_array()
            .ok_or("labyrinth needs a nodes array")?
            .clone();
        if nodes.len() > MAX_NODES {
            return Err("labyrinth node budget exceeded".into());
        }
        let mut index = BTreeMap::new();
        for (i, node) in nodes.iter().enumerate() {
            let id = node["id"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 240 && !id.chars().any(char::is_control))
                .ok_or("labyrinth node needs a bounded nonempty id")?;
            if index.insert(id.to_string(), i).is_some() {
                return Err(format!("duplicate node {id}"));
            }
            let kind = node["kind"].as_str().unwrap_or_default();
            if !KINDS.contains(&kind) {
                return Err(format!("{id}: unknown kind"));
            }
            if matches!(
                kind,
                "theorem" | "exhaustive" | "evidence" | "conjecture" | "hunch"
            ) && !matches!(
                node["tier"].as_str(),
                Some("T1" | "T2" | "T3" | "T4" | "T5" | "T6")
            ) {
                return Err(format!("{id}: missing or invalid tier"));
            }
            if kind == "hunch" && node["tier"] != "T6" {
                return Err(format!("{id}: hunch must be T6"));
            }
            if kind == "theorem" && matches!(node["tier"].as_str(), Some("T5" | "T6")) {
                return Err(format!("{id}: theorem cannot be a conjecture or hunch"));
            }
            if kind == "conjecture"
                && node["status"] == "open"
                && node["test"]
                    .as_str()
                    .is_none_or(|test| test.trim().is_empty())
            {
                return Err(format!("{id}: open conjecture needs a test"));
            }
            if kind == "deadend" && node["lesson"].as_str().is_none_or(|s| s.trim().is_empty()) {
                return Err(format!("{id}: dead end needs a lesson"));
            }
            for key in ["title", "statement", "test", "lesson"] {
                if node
                    .get(key)
                    .is_some_and(|v| v.as_str().is_none_or(|s| s.len() > 16_000))
                {
                    return Err(format!("{id}: invalid or oversized {key}"));
                }
            }
            if let Some(review) = node.get("review") {
                if !review.is_object()
                    || !matches!(
                        review["state"].as_str(),
                        Some("unreviewed" | "under-review" | "refereed" | "human-checked")
                    )
                {
                    return Err(format!("{id}: invalid review"));
                }
                if matches!(review["state"].as_str(), Some("refereed" | "human-checked"))
                    && review["by"].as_array().is_none_or(|by| {
                        by.is_empty()
                            || by
                                .iter()
                                .any(|by| by.as_str().is_none_or(|s| s.trim().is_empty()))
                    })
                {
                    return Err(format!("{id}: review must name its referees"));
                }
            }
            failed_routes::validate(node, id)?;
        }
        let mut dependency_counts = vec![0; nodes.len()];
        let mut dependents = vec![Vec::new(); nodes.len()];
        let mut dependencies = vec![Vec::new(); nodes.len()];
        let mut link_count = 0usize;
        for (i, node) in nodes.iter().enumerate() {
            if let Some(raw) = node.get("links") {
                let links = raw
                    .as_array()
                    .filter(|links| links.len() <= MAX_NODES)
                    .ok_or("invalid links array")?;
                link_count += links.len();
                if link_count > 32768 {
                    return Err("labyrinth link budget exceeded".into());
                }
                for link in links {
                    let target = link["to"]
                        .as_str()
                        .and_then(|id| index.get(id))
                        .copied()
                        .ok_or_else(|| format!("{}: dangling link", node["id"]))?;
                    let rel = link["rel"].as_str().unwrap_or_default();
                    if !RELATIONS.contains(&rel) {
                        return Err("unknown labyrinth relation".into());
                    }
                    if rel == "uses" {
                        dependency_counts[i] += 1;
                        dependents[target].push(i);
                        dependencies[i].push(target);
                    }
                }
            }
        }
        let mut ready: VecDeque<_> = dependency_counts
            .iter()
            .enumerate()
            .filter_map(|(i, &count)| (count == 0).then_some(i))
            .collect();
        let mut resolved = 0;
        let mut charted = vec![false; nodes.len()];
        while let Some(at) = ready.pop_front() {
            resolved += 1;
            charted[at] = passage(&nodes[at]) == Passage::Charted
                && dependencies[at]
                    .iter()
                    .all(|&dependency| charted[dependency]);
            for &next in &dependents[at] {
                dependency_counts[next] -= 1;
                if dependency_counts[next] == 0 {
                    ready.push_back(next);
                }
            }
        }
        if resolved != nodes.len() {
            return Err("cyclic labyrinth prerequisites".into());
        }
        Ok(Self {
            document,
            nodes,
            index,
            charted,
        })
    }

    fn node_passage(&self, index: usize) -> Passage {
        let state = passage(&self.nodes[index]);
        if state == Passage::Charted && !self.charted[index] {
            Passage::Review
        } else {
            state
        }
    }

    pub(crate) fn status(&self) -> Value {
        let mut counts = BTreeMap::<&str, usize>::new();
        for i in 0..self.nodes.len() {
            *counts.entry(self.node_passage(i).label()).or_default() += 1;
        }
        json!({"schema":1,"nodes":self.nodes.len(),"passages":counts,
            "title":self.document["meta"]["title"],"initialized":true})
    }

    fn dependencies_ready(&self, node: &Value) -> bool {
        node["links"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|link| link["rel"] == "uses")
            .all(|link| {
                self.index
                    .get(link["to"].as_str().unwrap_or_default())
                    .is_some_and(|&i| self.charted[i])
            })
    }

    pub(crate) fn frontier(&self, task: Option<&str>, limit: usize) -> Value {
        let root = task.map(store::task_id);
        let mut candidates: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| {
                matches!(
                    self.node_passage(self.index[node["id"].as_str().unwrap()]),
                    Passage::Door | Passage::Dark | Passage::Review
                ) && task.is_none_or(|_| {
                    node["angel_task"] == root.as_deref().unwrap_or_default()
                        || node["id"] == root.as_deref().unwrap_or_default()
                        || (node["angel_task"].is_null() && node["source"] != "angel")
                }) && !matches!(
                    node["status"].as_str(),
                    Some("answered" | "dropped" | "refuted")
                )
            })
            .collect();
        candidates.sort_by_key(|node| {
            (
                !self.dependencies_ready(node),
                match passage(node) {
                    Passage::Door => 0,
                    Passage::Review => 1,
                    _ => 2,
                },
                node["id"].as_str().unwrap_or_default(),
            )
        });
        json!(candidates.into_iter().take(limit.clamp(1, 24)).map(|node| json!({
            "id":node["id"],"title":node["title"],"statement":node["statement"],
            "passage":self.node_passage(self.index[node["id"].as_str().unwrap()]).label(),"tier":node["tier"],"test":node["test"],
            "dependencies_ready":self.dependencies_ready(node),"review":node["review"],
            "failed_routes":failed_routes::projection(node,8,false)
        })).collect::<Vec<_>>())
    }

    pub(crate) fn route(&self, from: &str, to: &str, explore: bool) -> Result<Value, String> {
        let start = *self.index.get(from).ok_or("unknown starting node")?;
        let goal = *self.index.get(to).ok_or("unknown destination node")?;
        if self.node_passage(start).cost(explore).is_none()
            || self.node_passage(goal).cost(explore).is_none()
        {
            return Ok(json!({"path":null,"reason":"endpoint is closed for this route policy"}));
        }
        let mut edges = vec![BTreeSet::new(); self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate() {
            for link in node["links"].as_array().into_iter().flatten() {
                if matches!(link["rel"].as_str(), Some("refutes" | "cites")) {
                    continue;
                }
                let j = self.index[link["to"].as_str().unwrap()];
                edges[i].insert(j);
                edges[j].insert(i);
            }
        }
        let path = routing::shortest_path(self.nodes.len(), start, goal, |at| {
            edges[at]
                .iter()
                .filter_map(|&i| self.node_passage(i).cost(explore).map(|cost| (i, cost)))
                .collect()
        });
        Ok(json!({"policy":if explore {"explore"} else {"established"},
            "path":path.map(|path| path.into_iter().map(|i| json!({"id":self.nodes[i]["id"],
                "passage":self.node_passage(i).label(),"tier":self.nodes[i]["tier"]})).collect::<Vec<_>>())}))
    }

    /// Selection advice is derived from the map; it grants no reward and does
    /// not silently reclassify a failed attempt as a refutation.
    pub(crate) fn plan(&self, task: Option<&str>, limit: usize) -> Value {
        let root = task.map(store::task_id);
        let attempts: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| {
                (node.get("angel_measurement").is_some() || node.get("angel_submission").is_some())
                    && root.as_ref().is_none_or(|root| node["angel_task"] == *root)
            })
            .take(12)
            .map(|node| {
                json!({"id":node["id"],"measurement":node["angel_measurement"],
            "submission":node["angel_submission"],"evidence":node["evidence"]})
            })
            .collect();
        let doors = self.frontier(task, limit);
        let closed_routes: Vec<_> = doors
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|door| {
                door["failed_routes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(move |attempt| {
                        let mut attempt = attempt.clone();
                        attempt["door_id"] = door["id"].clone();
                        attempt
                    })
            })
            .take(12)
            .collect();
        json!({"doors":doors,"recorded_attempts":attempts,"closed_routes":closed_routes,
            "warpath":book::d3_roles::pages(legend::MAP,[3,5,6,7])})
    }
}

/// Fresh contexts see at most three doors. Missing maps do not alter old loops.
pub(crate) fn context(workspace: &Path, task: &str) -> String {
    match load(workspace) {
        Ok(Some(map)) => {
            let mut doors = map.frontier(Some(task), 3);
            if doors.as_array().is_none_or(|doors| doors.is_empty()) {
                return String::new();
            }
            for door in doors.as_array_mut().unwrap() {
                // Preserve exact identities in the tool view; fresh prompts
                // carry only two compact recent attempts for each open door.
                let history = door["id"]
                    .as_str()
                    .and_then(|id| map.index.get(id))
                    .map(|&index| failed_routes::projection(&map.nodes[index], 2, true))
                    .unwrap_or_default();
                door["failed_routes"] = json!(history);
                for key in ["title", "statement", "test"] {
                    if let Some(text) = door[key].as_str() {
                        door[key] = json!(text.chars().take(400).collect::<String>());
                    }
                }
                // Referee provenance can be large and include custom fields.
                // Carry a bounded cue here; the read-only tool retains the
                // complete authored record for deliberate inspection.
                let review = &door["review"];
                door["review"] = json!({
                    "state":review["state"].as_str(),
                    "verdict":review["verdict"].as_str().map(|text|text.chars().take(80).collect::<String>()),
                    "by":review["by"].as_array().into_iter().flatten().take(4)
                        .filter_map(|by|by.as_str()).map(|text|text.chars().take(160).collect::<String>()).collect::<Vec<_>>()
                });
            }
            let text = format!(
                "\n\n{CONTEXT_MARK}\n{}\n{}\n{doors}",
                legend::MAP.cells(),
                book::d3_roles::pages(legend::DATA, [9])
            );
            text
        }
        Ok(None) => String::new(),
        Err(error) => format!(
            "\n\n{CONTEXT_MARK}\n{}\n{}\n{}",
            legend::MAP.cells(),
            book::d3_roles::pages(legend::DATA, [10]),
            json!({"error":error})
        ),
    }
}

pub(crate) fn command(workspace: &Path, arg: Option<&str>) -> Result<String, String> {
    let parts: Vec<_> = arg.unwrap_or("status").split_whitespace().collect();
    if parts.as_slice() == ["init"] {
        initialize(workspace)?;
        return Ok("labyrinth initialized · /labyrinth frontier · python3 labyrinth/lab.py check && python3 labyrinth/lab.py build".into());
    }
    let Some(map) = load(workspace)? else {
        return Ok("No local labyrinth · /labyrinth init enables Deli, loop and RL observations in this workspace.".into());
    };
    let value = match parts.as_slice() {
        [] | ["status"] | ["check"] => map.status(),
        ["frontier"] => map.frontier(None, 12),
        ["plan"] => map.plan(None, 6),
        ["route", from, to] => map.route(from, to, true)?,
        ["route", from, to, "established"] => map.route(from, to, false)?,
        _ => return Err(
            "usage: /labyrinth [init|status|check|frontier|plan|route <from> <to> [established]]"
                .into(),
        ),
    };
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/labyrinth/labyrinth__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/labyrinth/native__tests.rs"]
mod native_tests;
