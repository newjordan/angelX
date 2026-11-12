//! Persistent visual commuting, independent from goods output and staffing.
//! The settlement adapter chooses doorways; this module only walks people over
//! bounded, deterministic routes. Travel does not yet reduce work credit.
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

pub type Tile = (i16, i16);
pub const MAX_ACTORS: usize = 256;
pub const MAX_PATH: usize = 512;
/// The valley is 3,888 cells; the hillside, the whole 240 x 176 realm, is 42,240.
pub const MAX_NAVIGATION_CELLS: usize = 65_536;

/// Owned, passable ground in row-major order. Roads are preferred, while land
/// remains available for builders carrying supplies to a new wall or courtyard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Navigation {
    pub width: i16,
    pub height: i16,
    pub walkable: Vec<bool>,
    pub roads: BTreeSet<Tile>,
    pub spawn: Tile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Destination {
    pub citizen: u32,
    pub tile: Tile,
    /// Select a work animation after reaching this workplace entrance.
    pub working: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Walker {
    pub tile: Tile,
    pub previous_tile: Tile,
    pub destination: Tile,
    /// Next adjacent steps, excluding the current tile. Long routes retain a
    /// bounded prefix and replan when that prefix has been walked.
    pub path: Vec<Tile>,
    pub walking: bool,
    pub working: bool,
    pub blocked: bool,
    pub facing_left: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Motion {
    pub actors: BTreeMap<u32, Walker>,
    /// Hash of actual geometry, never the economy's per-second revision.
    pub geometry: u64,
}

fn inside((x, y): Tile, width: i16, height: i16) -> bool {
    x >= 0 && y >= 0 && x < width && y < height
}
fn distance(a: Tile, b: Tile) -> u32 {
    u32::from(a.0.abs_diff(b.0)) + u32::from(a.1.abs_diff(b.1))
}
fn dimensions(width: i16, height: i16) -> Option<usize> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let area = usize::try_from(width).ok()? * usize::try_from(height).ok()?;
    (area <= MAX_NAVIGATION_CELLS).then_some(area)
}
impl Navigation {
    fn index(&self, tile: Tile) -> usize {
        tile.1 as usize * self.width as usize + tile.0 as usize
    }
    pub fn valid(&self) -> bool {
        dimensions(self.width, self.height) == Some(self.walkable.len())
            && inside(self.spawn, self.width, self.height)
            && self.walkable[self.index(self.spawn)]
            && self.roads.len() <= MAX_NAVIGATION_CELLS
            && self
                .roads
                .iter()
                .all(|&t| inside(t, self.width, self.height))
    }
    pub fn open(&self, tile: Tile) -> bool {
        inside(tile, self.width, self.height)
            && self
                .walkable
                .get(self.index(tile))
                .copied()
                .unwrap_or(false)
    }
    fn fingerprint(&self) -> u64 {
        // Specified byte order keeps native and WASM caches identical.
        let mut hash = 0xcbf29ce484222325u64;
        let mut push = |byte: u8| {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        };
        for n in [self.width, self.height, self.spawn.0, self.spawn.1] {
            for byte in n.to_le_bytes() {
                push(byte);
            }
        }
        for &open in &self.walkable {
            push(u8::from(open));
        }
        for &(x, y) in &self.roads {
            for byte in x.to_le_bytes().into_iter().chain(y.to_le_bytes()) {
                push(byte);
            }
        }
        hash
    }

    fn route(&self, from: Tile, to: Tile) -> Option<Vec<Tile>> {
        if from == to {
            return Some(Vec::new());
        }
        if !self.open(to) {
            return None;
        }
        let mut costs = vec![u32::MAX; self.walkable.len()];
        let mut parent = vec![None; self.walkable.len()];
        costs[self.index(from)] = 0;
        let mut queue = BinaryHeap::from([Reverse((0u32, from))]);
        while let Some(Reverse((cost, tile))) = queue.pop() {
            if costs[self.index(tile)] != cost {
                continue;
            }
            if tile == to {
                let mut path = Vec::new();
                let mut cursor = to;
                while cursor != from {
                    path.push(cursor);
                    cursor = parent[self.index(cursor)]?;
                }
                path.reverse();
                path.truncate(MAX_PATH);
                return Some(path);
            }
            for next in [
                (tile.0 - 1, tile.1),
                (tile.0 + 1, tile.1),
                (tile.0, tile.1 - 1),
                (tile.0, tile.1 + 1),
            ] {
                if !self.open(next) {
                    continue;
                }
                let next_cost = cost + if self.roads.contains(&next) { 10 } else { 24 };
                let index = self.index(next);
                if next_cost < costs[index] {
                    costs[index] = next_cost;
                    parent[index] = Some(tile);
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }
        None
    }
}

impl Motion {
    /// Geometry changes may leave a person temporarily standing on a newly
    /// blocked plot. Save validation checks bounds and path shape, not current
    /// passability; the next sync repairs that route without teleporting.
    pub fn valid(&self, width: i16, height: i16) -> bool {
        dimensions(width, height).is_some()
            && self.actors.len() <= MAX_ACTORS
            && self.actors.iter().all(|(&id, actor)| {
                id > 0
                    && id < u32::MAX
                    && [actor.tile, actor.previous_tile, actor.destination]
                        .into_iter()
                        .all(|t| inside(t, width, height))
                    && distance(actor.tile, actor.previous_tile) <= 1
                    && actor.walking == (actor.tile != actor.previous_tile)
                    && actor.path.len() <= MAX_PATH
                    && actor.path.iter().all(|&t| inside(t, width, height))
                    && actor
                        .path
                        .first()
                        .is_none_or(|&t| distance(actor.tile, t) == 1)
                    && actor.path.windows(2).all(|p| distance(p[0], p[1]) == 1)
                    && (!actor.working || actor.tile == actor.destination)
                    && (!actor.blocked || (actor.path.is_empty() && !actor.working))
            })
    }

    fn check(&self, destinations: &[Destination], nav: &Navigation) -> Result<(), String> {
        if !nav.valid() || !self.valid(nav.width, nav.height) {
            return Err("Invalid citizen navigation or saved movement".into());
        }
        let mut ids = BTreeSet::new();
        if destinations.len() > MAX_ACTORS
            || destinations.iter().any(|d| {
                d.citizen == 0
                    || d.citizen == u32::MAX
                    || !ids.insert(d.citizen)
                    || !inside(d.tile, nav.width, nav.height)
            })
        {
            return Err("Invalid citizen destinations".into());
        }
        Ok(())
    }

    /// Reconcile after commands and roster changes without moving anyone.
    /// Invalid input is rejected before modifying the saved actor ledger.
    pub fn sync(&mut self, destinations: &[Destination], nav: &Navigation) -> Result<(), String> {
        self.check(destinations, nav)?;
        let ids: BTreeSet<_> = destinations.iter().map(|d| d.citizen).collect();
        self.actors.retain(|id, _| ids.contains(id));
        let geometry = nav.fingerprint();
        let changed = self.geometry != geometry;
        for d in destinations {
            let actor = self.actors.entry(d.citizen).or_insert_with(|| Walker {
                tile: nav.spawn,
                previous_tile: nav.spawn,
                destination: nav.spawn,
                path: Vec::new(),
                walking: false,
                working: false,
                blocked: false,
                facing_left: false,
            });
            actor.previous_tile = actor.tile;
            actor.walking = false;
            let redirected = actor.destination != d.tile;
            actor.destination = d.tile;
            if changed
                || redirected
                || (actor.path.is_empty() && actor.tile != d.tile && !actor.blocked)
            {
                match nav.route(actor.tile, d.tile) {
                    Some(path) => {
                        actor.path = path;
                        actor.blocked = false;
                    }
                    None => {
                        actor.path.clear();
                        actor.blocked = true;
                    }
                }
            }
            actor.working = d.working && actor.tile == d.tile && !actor.blocked;
        }
        self.geometry = geometry;
        Ok(())
    }

    /// One authoritative second: one adjacent plot per citizen. The renderer
    /// may interpolate previous_tile→tile and use walking before working when
    /// a person has just arrived, so the final step finishes naturally.
    pub fn advance_second(
        &mut self,
        destinations: &[Destination],
        nav: &Navigation,
    ) -> Result<(), String> {
        self.sync(destinations, nav)?;
        for d in destinations {
            let actor = self.actors.get_mut(&d.citizen).unwrap();
            if let Some(&next) = actor.path.first() {
                // A stale or hand-edited route cannot cross a blocked tile.
                if !nav.open(next) {
                    actor.path.clear();
                    actor.blocked = true;
                    actor.working = false;
                    continue;
                }
                actor.path.remove(0);
                actor.previous_tile = actor.tile;
                actor.tile = next;
                actor.walking = true;
                if next.0 != actor.previous_tile.0 {
                    actor.facing_left = next.0 < actor.previous_tile.0;
                }
            }
            actor.working = d.working && actor.tile == d.tile && !actor.blocked;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn map(width: i16, height: i16) -> Navigation {
        Navigation {
            width,
            height,
            walkable: vec![true; width as usize * height as usize],
            roads: BTreeSet::new(),
            spawn: (0, 0),
        }
    }
    fn target(tile: Tile) -> [Destination; 1] {
        [Destination {
            citizen: 1,
            tile,
            working: true,
        }]
    }

    #[test]
    fn citizens_prefer_roads_reach_entrances_and_switch_from_walk_to_work() {
        let mut nav = map(5, 3);
        nav.roads = (0..5).map(|x| (x, 1)).collect();
        let mut motion = Motion::default();
        motion.sync(&target((4, 0)), &nav).unwrap();
        assert_eq!(motion.actors[&1].path[0], (0, 1));
        assert!(!motion.actors[&1].working);
        for _ in 0..6 {
            motion.advance_second(&target((4, 0)), &nav).unwrap();
        }
        let actor = &motion.actors[&1];
        assert_eq!(actor.tile, (4, 0));
        assert!(actor.walking && actor.working);
        motion.advance_second(&target((4, 0)), &nav).unwrap();
        assert!(!motion.actors[&1].walking);
        assert!(motion.actors[&1].working);
        assert!(motion.valid(5, 3));
    }

    #[test]
    fn closing_a_road_replans_from_the_current_person_without_teleporting() {
        let mut nav = map(5, 3);
        nav.spawn = (0, 1);
        nav.roads = (0..5).map(|x| (x, 1)).collect();
        let mut motion = Motion::default();
        let goal = target((4, 1));
        motion.advance_second(&goal, &nav).unwrap();
        assert_eq!(motion.actors[&1].tile, (1, 1));
        nav.walkable[7] = false;
        motion.sync(&goal, &nav).unwrap();
        assert_eq!(motion.actors[&1].tile, (1, 1));
        assert!(!motion.actors[&1].path.contains(&(2, 1)));
        for _ in 0..5 {
            motion.advance_second(&goal, &nav).unwrap();
        }
        assert_eq!(motion.actors[&1].tile, (4, 1));
    }

    #[test]
    fn unreachable_workers_wait_until_geometry_reopens_and_ids_stay_bounded() {
        let mut nav = map(3, 1);
        nav.walkable[1] = false;
        let mut motion = Motion::default();
        let goal = target((2, 0));
        motion.advance_second(&goal, &nav).unwrap();
        assert!(motion.actors[&1].blocked);
        assert_eq!(motion.actors[&1].tile, nav.spawn);
        nav.walkable[1] = true;
        motion.advance_second(&goal, &nav).unwrap();
        assert_eq!(motion.actors[&1].tile, (1, 0));
        assert!(!motion.actors[&1].blocked);
        motion.sync(&[], &nav).unwrap();
        assert!(motion.actors.is_empty());
        let newcomers = [Destination {
            citizen: 8,
            tile: (2, 0),
            working: false,
        }];
        motion.sync(&newcomers, &nav).unwrap();
        assert_eq!(motion.actors[&8].tile, nav.spawn);
    }

    #[test]
    fn saves_replay_identical_paths_and_goal_changes_preserve_position() {
        let nav = map(12, 4);
        let goal = target((11, 3));
        let mut a = Motion::default();
        for _ in 0..5 {
            a.advance_second(&goal, &nav).unwrap();
        }
        let mut b: Motion = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(a, b);
        let changed = target((1, 2));
        let before = a.actors[&1].tile;
        a.sync(&changed, &nav).unwrap();
        b.sync(&changed, &nav).unwrap();
        assert_eq!(a.actors[&1].tile, before);
        for _ in 0..20 {
            a.advance_second(&changed, &nav).unwrap();
            b.advance_second(&changed, &nav).unwrap();
            assert_eq!(a, b);
            assert!(a.valid(12, 4));
        }
    }

    #[test]
    fn long_winding_routes_replan_bounded_prefixes_until_arrival() {
        let mut nav = map(32, 33);
        nav.walkable.fill(false);
        for y in 0..33 {
            for x in 0..32 {
                if y % 2 == 0 || x == if (y / 2) % 2 == 0 { 31 } else { 0 } {
                    nav.walkable[y * 32 + x] = true;
                }
            }
        }
        let mut motion = Motion::default();
        let goal = target((31, 32));
        motion.sync(&goal, &nav).unwrap();
        assert_eq!(motion.actors[&1].path.len(), MAX_PATH);
        for _ in 0..600 {
            motion.advance_second(&goal, &nav).unwrap();
            assert!(motion.valid(32, 33));
        }
        assert_eq!(motion.actors[&1].tile, (31, 32));
        assert!(motion.actors[&1].working);
    }

    #[test]
    fn invalid_geometry_rosters_and_paths_are_atomic() {
        let nav = map(4, 4);
        let mut motion = Motion::default();
        motion.sync(&target((3, 3)), &nav).unwrap();
        let before = motion.clone();
        assert!(motion.sync(&[target((2, 2))[0]; 2], &nav).is_err());
        assert_eq!(motion, before);
        let mut invalid = nav.clone();
        invalid.walkable.pop();
        assert!(motion.advance_second(&target((3, 3)), &invalid).is_err());
        assert_eq!(motion, before);
        motion.actors.get_mut(&1).unwrap().path[0] = (3, 3);
        assert!(!motion.valid(4, 4));
        let broken = motion.clone();
        assert!(motion.advance_second(&target((3, 3)), &nav).is_err());
        assert_eq!(motion, broken);
    }
}
