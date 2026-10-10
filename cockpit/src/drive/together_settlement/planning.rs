//! Research doors choose real excavation fronts; durable loop receipts pay for
//! the work. A recorded plan is replayed without rereading a mutable map.

use super::{COLS, ROWS, RoomKind, Site};
use crate::drive::labyrinth::{self, Passage, routing::shortest_path};
use crate::knowledge::cut::sha256_hex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

const MAX_ROUTE: usize = 24;
const RADIUS: i32 = 6;
const WIDTH: i32 = RADIUS * 2 + 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResearchPassage {
    Door,
    Dark,
    Review,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Outcome {
    Unreviewed,
    UnderReview,
    Refereed,
    Failed,
}

/// Bounded provenance, not an authorization, proof certificate, or reward.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub(crate) map_sha256: String,
    pub(crate) task_id: String,
    pub(crate) door_id: String,
    pub(crate) route: Vec<String>,
    pub(crate) passage: ResearchPassage,
    pub(crate) outcome: Outcome,
}

impl Input {
    pub(crate) fn valid(&self) -> bool {
        let id = |text: &str| {
            !text.is_empty() && text.len() <= 240 && !text.chars().any(char::is_control)
        };
        self.map_sha256.len() == 64
            && self
                .map_sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            && id(&self.task_id)
            && id(&self.door_id)
            && (1..=MAX_ROUTE).contains(&self.route.len())
            && self.route.iter().all(|part| id(part))
            && self.route.iter().collect::<BTreeSet<_>>().len() == self.route.len()
            && self.route.last() == Some(&self.door_id)
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Axis {
    North,
    East,
    South,
    West,
}

impl Axis {
    pub(super) fn includes(self, col: usize, row: usize, slice: usize) -> bool {
        let (at, length) = match self {
            Self::North => (row - 1, ROWS - 2),
            Self::East => (COLS - 2 - col, COLS - 2),
            Self::South => (ROWS - 2 - row, ROWS - 2),
            Self::West => (col - 1, COLS - 2),
        };
        at * 3 / length <= slice
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Plan {
    schema: u32,
    pub(super) room: usize,
    pub(super) input: Input,
    pub(super) target: (i32, i32),
    pub(super) parent: usize,
    pub(super) direction: usize,
    pub(super) kind: RoomKind,
    pub(super) axis: Axis,
    pub(super) rock_route: Vec<(usize, usize)>,
    digest: String,
}

pub(super) fn choose(site: &Site, room: usize, input: &Input) -> Result<Plan, String> {
    if !input.valid() || room >= super::ROOMS.len() {
        return Err("invalid settlement research plan input".into());
    }
    let key = sha256_hex(
        &serde_json::to_vec(&(site.seed, room, input)).map_err(|error| error.to_string())?,
    );
    let bits = u64::from_str_radix(&key[..16], 16).map_err(|error| error.to_string())?;
    let home = site.floor.rooms[0].cell;
    let reserved: BTreeSet<_> = site.floor.rooms[..4]
        .iter()
        .map(|room| room.cell)
        .chain([(home.0 - 1, home.1 + 1)]) // the separately paid tavern
        .collect();
    let occupied: BTreeSet<_> = site.floor.rooms.iter().map(|room| room.cell).collect();
    let origin = (home.0 - 1, home.1);
    let index = |(x, y): (i32, i32)| ((y - home.1 + RADIUS) * WIDTH + x - home.0 + RADIUS) as usize;
    let cell = |i: usize| {
        (
            i as i32 % WIDTH + home.0 - RADIUS,
            i as i32 / WIDTH + home.1 - RADIUS,
        )
    };
    let bounded = |(x, y): (i32, i32)| (x - home.0).abs() <= RADIUS && (y - home.1).abs() <= RADIUS;
    let mut frontier = Vec::new();
    let parents: Vec<_> = if room == 0 {
        vec![0]
    } else {
        (4..4 + room).collect()
    };
    for parent in parents {
        for (direction, (dx, dy)) in super::layout::DIRS.into_iter().enumerate() {
            if parent == 0 && direction != 3 {
                continue;
            }
            let at = site.floor.rooms[parent].cell;
            let target = (at.0 + dx, at.1 + dy);
            if !bounded(target) || reserved.contains(&target) || occupied.contains(&target) {
                continue;
            }
            // The shared search counts rock as expensive and existing mining
            // galleries as cheap. Only an adjacent connected front is cut.
            let Some(route) = shortest_path(
                (WIDTH * WIDTH) as usize,
                index(origin),
                index(target),
                |at| {
                    let at = cell(at);
                    super::layout::DIRS
                        .into_iter()
                        .map(|(dx, dy)| (at.0 + dx, at.1 + dy))
                        .filter(|next| bounded(*next) && !reserved.contains(next))
                        .map(|next| (index(next), if occupied.contains(&next) { 1 } else { 5 }))
                        .collect()
                },
            ) else {
                continue;
            };
            let heading = (bits % 4) as usize;
            let turn = (direction + 4 - heading) % 4;
            let degree = super::layout::DIRS
                .into_iter()
                .filter(|&(dx, dy)| occupied.contains(&(target.0 + dx, target.1 + dy)))
                .count();
            let shape = match input.passage {
                ResearchPassage::Door => degree,
                ResearchPassage::Dark => route.len(),
                ResearchPassage::Review => 4 - degree,
            };
            frontier.push((
                (turn, shape, route.len(), target, parent),
                parent,
                direction,
                target,
            ));
        }
    }
    frontier.sort_by_key(|entry| entry.0);
    let Some((_, parent, direction, target)) = frontier.first().copied() else {
        return Err("settlement has no bounded connected excavation frontier".into());
    };
    let axis = match (bits >> 3) % 4 {
        0 => Axis::North,
        1 => Axis::East,
        2 => Axis::South,
        _ => Axis::West,
    };
    let kind = match input.outcome {
        Outcome::Failed => RoomKind::Workshop,
        Outcome::Refereed => RoomKind::Quarters,
        _ => match input.passage {
            ResearchPassage::Door => RoomKind::Workshop,
            ResearchPassage::Dark => RoomKind::Stockpile,
            ResearchPassage::Review => RoomKind::Hall,
        },
    };
    let ingress = match (direction + 2) % 4 {
        0 => (COLS / 2 - 1, 1),
        1 => (COLS - 2, ROWS / 2 - 1),
        2 => (COLS / 2 - 1, ROWS - 2),
        _ => (1, ROWS / 2 - 1),
    };
    let waypoint = (
        if bits & 32 == 0 {
            COLS / 4
        } else {
            COLS * 3 / 4
        },
        if bits & 64 == 0 {
            ROWS / 4
        } else {
            ROWS * 3 / 4
        },
    );
    let tile_index = |(col, row): (usize, usize)| row * COLS + col;
    let tile = |at: usize| (at % COLS, at / COLS);
    let rock_path = |from, to| {
        shortest_path(COLS * ROWS, tile_index(from), tile_index(to), |at| {
            let (col, row) = tile(at);
            super::layout::DIRS
                .into_iter()
                .map(|(dx, dy)| (col as i32 + dx, row as i32 + dy))
                .filter(|&(col, row)| {
                    (1..COLS as i32 - 1).contains(&col) && (1..ROWS as i32 - 1).contains(&row)
                })
                .map(|(col, row)| (tile_index((col as usize, row as usize)), 1))
                .collect()
        })
        .ok_or_else(|| "settlement rock ingress is unreachable".to_string())
    };
    let mut rock_route = rock_path(ingress, waypoint)?;
    rock_route.extend(
        rock_path(waypoint, (COLS / 2 - 1, ROWS / 2 - 1))?
            .into_iter()
            .skip(1),
    );
    let mut plan = Plan {
        schema: 1,
        room,
        input: input.clone(),
        target,
        parent,
        direction,
        kind,
        axis,
        rock_route: rock_route.into_iter().map(tile).collect(),
        digest: String::new(),
    };
    plan.digest = sha256_hex(&serde_json::to_vec(&plan).map_err(|error| error.to_string())?);
    Ok(plan)
}
