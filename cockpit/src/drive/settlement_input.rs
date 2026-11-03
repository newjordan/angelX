//! The cockpit side of settlement research planning: reads the labyrinth map
//! and derives the plain `together_settlement::planning::Input` the realm
//! side stores, replays and digests. `planning` keeps the data types and the
//! pure excavation planner; the labyrinth never leaks into it. The old
//! `planning::input` path still resolves through a re-export there.
use crate::drive::labyrinth::{self, Passage};
use crate::drive::together_settlement::planning::{Input, MAX_ROUTE, Outcome, ResearchPassage};
use crate::knowledge::cut::sha256_hex;
use std::path::Path;

/// Select one eligible research frontier. An explicit campaign choice can use
/// input_for_door; both paths derive the route and review outcome from the map.
pub(crate) fn input(root: &Path, task: &str) -> Result<Option<Input>, String> {
    let Some(map) = labyrinth::load(root)? else {
        return Ok(None);
    };
    let task_id = task_id(task);
    let doors = map.frontier(Some(task), MAX_ROUTE);
    let Some(door) = doors
        .as_array()
        .into_iter()
        .flatten()
        .find(|door| door["id"] != task_id)
        .or_else(|| doors.as_array().and_then(|doors| doors.first()))
        .and_then(|door| door["id"].as_str())
    else {
        return Ok(None);
    };
    map_input(&map, task, door).map(Some)
}

#[cfg_attr(not(test), allow(dead_code))] // an explicit door choice; tests and a campaign pick call it
pub(crate) fn input_for_door(root: &Path, task: &str, door: &str) -> Result<Option<Input>, String> {
    labyrinth::load(root)?
        .map(|map| map_input(&map, task, door))
        .transpose()
}

fn task_id(task: &str) -> String {
    let task = task.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("q.angel-{}", &sha256_hex(task.as_bytes())[..24])
}

fn map_input(map: &labyrinth::Map, task: &str, door: &str) -> Result<Input, String> {
    let frontier = map.frontier(Some(task), MAX_ROUTE);
    let eligible = frontier
        .as_array()
        .into_iter()
        .flatten()
        .find(|node| node["id"] == door)
        .ok_or("settlement research door is closed or outside the current frontier")?;
    let task_id = task_id(task);
    let node = map
        .nodes
        .iter()
        .find(|node| node["id"] == door)
        .ok_or("settlement research door is missing")?;
    let passage = match eligible["passage"].as_str() {
        Some("door") => ResearchPassage::Door,
        Some("dark") => ResearchPassage::Dark,
        Some("under-review") => ResearchPassage::Review,
        _ => return Err("settlement research door is not open".into()),
    };
    let origin = map
        .nodes
        .iter()
        .find(|node| node["id"] == task_id)
        .or_else(|| {
            map.nodes
                .iter()
                .find(|node| labyrinth::passage(node) == Passage::Charted)
        })
        .and_then(|node| node["id"].as_str())
        .unwrap_or(door);
    let route = map.route(origin, door, true)?;
    if route["path"]
        .as_array()
        .is_some_and(|path| path.len() > MAX_ROUTE)
    {
        return Err("settlement research route exceeds the recorded planning budget".into());
    }
    // A disconnected door has no invented connection to another research
    // node. Its singleton route still records the exact chosen frontier.
    let route: Vec<String> = route["path"]
        .as_array()
        .map(|path| {
            path.iter()
                .filter_map(|part| part["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_else(|| vec![door.into()]);
    let verdict = node["review"]["verdict"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let outcome = if labyrinth::has_negative_attempt(node)
        || node["angel_measurement"]["passed"] == false
        || node["angel_submission"]["official_metrics"]["verified"] == false
        || matches!(
            node["angel_submission"]["status"]
                .as_str()
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("failed" | "error" | "timeout" | "timed_out" | "timed-out")
        )
        || matches!(verdict.as_str(), "FALSE" | "GAP" | "FAIL" | "INVALID")
    {
        Outcome::Failed
    } else {
        match node["review"]["state"].as_str() {
            Some("refereed" | "human-checked") => Outcome::Refereed,
            Some("under-review") => Outcome::UnderReview,
            _ => Outcome::Unreviewed,
        }
    };
    let input = Input {
        map_sha256: sha256_hex(
            &serde_json::to_vec(&map.document).map_err(|error| error.to_string())?,
        ),
        task_id,
        door_id: door.into(),
        route,
        passage,
        outcome,
    };
    if !input.valid() {
        return Err("invalid settlement research provenance".into());
    }
    Ok(input)
}
