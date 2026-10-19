//! Surface travel, bounded to the overview's geography and four seats.
use super::*;
use super::map::{self, MAP_H, MAP_W};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub wishes: std::sync::Arc<Vec<crate::realm::Wish>>,
    pub return_to: Option<Return>,
    pub door_held: bool,
    /// The knights whose E is still down from before the door was there
    /// for them (`tick_kingdom`): the server's own, never saved nor sent.
    #[serde(skip)]
    pub door_latched: std::collections::BTreeSet<u32>,
    /// In a seat's own view only (`Run::view_for_seat`): this knight waits
    /// outside while the party plays on inside. Never saved nor sent.
    #[serde(skip)]
    pub waiting: Option<Waiting>,
    /// The places the party has stepped into this outing, so the goal
    /// marker moves on to somewhere new.
    pub seen: Vec<RoomKind>,
    /// Knights holding E since it last changed their class, so one press
    /// is one change.
    pub class_held: std::collections::BTreeSet<u32>,
    /// The same, at a Sanctuary's field banner below ground.
    pub field_held: std::collections::BTreeSet<u32>,
    /// The kingdom's little things: who rang, kicked or bowed, and when.
    pub moments: super::trinkets::Moments,
    /// The biome each knight last came into on the surface, and the tick
    /// it did (`Biome::index`): the corner of that knight's view names it
    /// for a moment.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub entered: BTreeMap<u32, (u8, u64)>,
    /// The run's discoverables found so far, one bit each
    /// (`crate::world::finds`).
    #[serde(skip_serializing_if = "is_zero")]
    pub found: u32,
    /// The standing stones last woken, and when: they glow a moment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub woke: Option<(u8, u64)>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A knight on a kingdom road goes at a trot, twice the walking pace, so
/// crossing the realm is a run, not a slog.
pub const ROAD_PACE: f32 = 2.0;
/// How much of the way to its knight the kingdom camera eases a tick
/// (`Tune::camera_follow` until moved).
pub const CAMERA_FOLLOW: f32 = 0.22;

/// Whether a knight standing at `(x, y)` (units) is on a road.
pub fn on_road(geo: &map::Geography, x: f32, y: f32) -> bool {
    map::is_road(geo, ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32))
}

/// Where the party is while this knight waits outside, and the spot by
/// the door where E takes them back in (`Run::detached_seats`).
#[derive(Clone, Debug, PartialEq)]
pub struct Waiting {
    pub place: String,
    pub entry: (f32, f32),
}

/// Where a knight on the surface heads next, and what it is called.
#[derive(Clone, Debug, PartialEq)]
pub struct Goal {
    pub name: String,
    pub stand: (i32, i32),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Return {
    pub x: f32,
    pub y: f32,
    pub name: String,
    pub root: RoomKind,
    #[serde(default)]
    pub party: BTreeMap<u32, (f32, f32)>,
}
impl State {
    pub fn valid(&self) -> bool {
        self.wishes.len() <= crate::realm::MAX_WISHES
            && self.seen.len() <= 128
            && self.class_held.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.field_held.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.moments.valid()
            && self.found < 1 << crate::world::finds::COUNT
            && self.woke.is_none_or(|(i, _)| usize::from(i) < crate::world::finds::COUNT)
            && self.entered.len() <= MAX_PLAYERS as usize
            && self.entered.iter().all(|(id, &(biome, _))| {
                (1..=MAX_PLAYERS).contains(id) && usize::from(biome) < crate::world::biome::Biome::ALL.len()
            })
            && self.wishes.iter().all(|w| {
                w.art.len() <= 32
                    && w.art.iter().all(|r| r.chars().count() <= 32)
                    && w.name.len() <= 256
            })
            && self.return_to.as_ref().is_none_or(|r| {
                r.x.is_finite()
                    && r.y.is_finite()
                    && r.x >= 0.0
                    && r.y >= 0.0
                    && r.x < MAP_W as f32 * TILE_UNITS
                    && r.y < MAP_H as f32 * TILE_UNITS
                    && r.name.len() <= 256
                    && r.party.len() <= MAX_PLAYERS as usize
                    && r.party.iter().all(|(id, (x, y))| {
                        (1..=MAX_PLAYERS).contains(id)
                            && x.is_finite()
                            && y.is_finite()
                            && *x >= 0.0
                            && *y >= 0.0
                            && *x < MAP_W as f32 * TILE_UNITS
                            && *y < MAP_H as f32 * TILE_UNITS
                    })
            })
    }
}
impl Run {
    pub fn returns_to_kingdom(&self, entrance: &str) -> bool {
        self.kingdom.return_to.as_ref().is_some_and(|r| {
            matches!(
                (r.root, entrance),
                (RoomKind::CollegeGrounds, "college-out")
                    | (RoomKind::ColosseumHall, "colosseum-out")
                    | (RoomKind::Ranch, "ranch-out")
                    | (RoomKind::Pasture, "fields-out")
            )
        })
    }
    pub fn raise_kingdom(&mut self) {
        if self.dungeon.depth != 0 {
            return;
        }
        let geo = map::geography(&self.kingdom.wishes);
        if let Some(i) = world::room_of(&self.dungeon, RoomKind::Kingdom) {
            if self.dungeon.rooms[i].fingerprint() != geo.room.fingerprint() {
                // A save from before the kingdom opened carries the old,
                // smaller map: its spots on it mean nothing on this one.
                let resized = (self.dungeon.rooms[i].cols, self.dungeon.rooms[i].rows) != (geo.room.cols, geo.room.rows);
                self.dungeon.rooms[i] = geo.room.clone();
                self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
                if resized {
                    self.rehome_on_new_map(&geo);
                } else {
                    self.rehome_on_changed_ground(&geo);
                }
            }
        } else if self.dungeon.rooms.len() < world::HOME_ROOM_LIMIT {
            self.dungeon.rooms.push(geo.room.clone());
            self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        }
        if world::room_of(&self.dungeon, RoomKind::RealmHall).is_none()
            && self.dungeon.rooms.len() < world::HOME_ROOM_LIMIT
        {
            let mut hall = Room::solid_rock((21, -20));
            hall.kind = RoomKind::RealmHall;
            for y in 1..ROWS - 1 {
                for x in 1..COLS - 1 {
                    hall.set(x, y, Tile::Floor);
                }
            }
            hall.open_door(2);
            self.dungeon.rooms.push(hall);
            self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        }
    }
    /// After the kingdom's map changed size under a saved run: the return
    /// door goes to its place's stand on the new map (the Keep's, if the
    /// place is gone), the party with it, and a party out on the surface
    /// stands at the Keep.
    fn rehome_on_new_map(&mut self, geo: &map::Geography) {
        let keep = crate::world::kingdom::keep_stand();
        let unit = |(x, y): (i32, i32)| ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.7) * TILE_UNITS);
        if let Some(back) = &mut self.kingdom.return_to {
            let stand = geo.sites.iter().find(|s| s.room == back.root).map_or(keep, |s| s.stand);
            (back.x, back.y) = unit(stand);
            for spot in back.party.values_mut() {
                *spot = unit(stand);
            }
        }
        self.kingdom.moments.goose_at = None;
        self.kingdom.entered.clear();
        if self.room().kind == RoomKind::Kingdom {
            let at = unit(keep);
            for hero in self.players.values_mut() {
                (hero.x, hero.y) = at;
                hero.camera = Some((at.0, at.1 - 1.0));
            }
            self.safe_kingdom_arrival();
        }
    }
    /// After the kingdom's ground changed under a saved run at the same
    /// size (the castles grown into great walled exteriors,
    /// `docs/KINGDOM-OPEN.md`): a return door whose place's door has moved
    /// goes with it, the party with it, and a knight out on the surface
    /// who now stands on closed ground steps onto the nearest open ground.
    fn rehome_on_changed_ground(&mut self, geo: &map::Geography) {
        let unit = |(x, y): (i32, i32)| ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.7) * TILE_UNITS);
        if let Some(back) = &mut self.kingdom.return_to {
            let tile = ((back.x / TILE_UNITS) as i32, (back.y / TILE_UNITS) as i32);
            let moved = match geo.sites.iter().find(|s| s.room == back.root && s.name == back.name) {
                Some(site) => (site.stand != tile).then_some(site.stand),
                None if geo.room.tile(tile.0, tile.1) != Tile::Floor => Some(
                    geo.sites
                        .iter()
                        .find(|s| s.room == back.root)
                        .map_or_else(crate::world::kingdom::keep_stand, |s| s.stand),
                ),
                None => None,
            };
            if let Some(stand) = moved {
                (back.x, back.y) = unit(stand);
                for spot in back.party.values_mut() {
                    *spot = unit(stand);
                }
            }
        }
        if self.room().kind == RoomKind::Kingdom {
            self.safe_kingdom_arrival();
        }
    }
    pub fn walk_kingdom(&mut self, tile: Option<(i32, i32)>) {
        self.raise_kingdom();
        let Some(i) = world::room_of(&self.dungeon, RoomKind::Kingdom) else {
            return;
        };
        let stand = tile.unwrap_or_else(|| {
            map::gate_stand(&self.kingdom.wishes)
        });
        self.kingdom.return_to = None;
        self.arrive(i, (stand.0 as f32 + 0.5, stand.1 as f32 + 0.7));
        self.kingdom.door_held = true;
        self.safe_kingdom_arrival();
        for h in self.players.values_mut() {
            h.camera = Some((h.x, h.y - 1.0));
        }
    }
    fn safe_kingdom_arrival(&mut self) {
        let room = &self.dungeon.rooms[self.at];
        let grid = Grid {
            room,
            barred: false,
        };
        for hero in self.players.values_mut() {
            if grid.clear(hero.x, hero.y, HERO_RADIUS, Mover::Hero) {
                continue;
            }
            let tile = ((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
            let at = (0..room.rows as i32)
                .flat_map(|y| (0..room.cols as i32).map(move |x| (x, y)))
                .filter(|&(x, y)| room.tile(x, y) == Tile::Floor)
                .min_by_key(|&(x, y)| (x - tile.0).pow(2) + (y - tile.1).pow(2));
            if let Some((x, y)) = at {
                (hero.x, hero.y) = ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS);
            }
        }
    }
    pub fn migrate_kingdom(&mut self) {
        self.raise_kingdom();
        self.record_kingdom_visit();
    }
    pub fn record_kingdom_visit(&mut self) {
        // Legacy surface rooms become visits with a real surface return door.
        let kind = self.room().kind;
        let root = if college::is_room(kind) {
            RoomKind::CollegeGrounds
        } else if colosseum::is_room(kind) {
            RoomKind::ColosseumHall
        } else if kind == RoomKind::KingsHall {
            RoomKind::MineHead
        } else {
            kind
        };
        if map::outdoor(root)
            && self.kingdom.return_to.is_none()
            && world::room_of(&self.dungeon, RoomKind::Kingdom).is_some()
        {
            let geo = map::geography(&self.kingdom.wishes);
            if let Some(site) = geo.sites.iter().find(|s| s.room == root) {
                self.kingdom.return_to = Some(Return {
                    x: (site.stand.0 as f32 + 0.5) * TILE_UNITS,
                    y: (site.stand.1 as f32 + 0.7) * TILE_UNITS,
                    name: site.name.clone(),
                    root: site.room,
                    party: BTreeMap::new(),
                });
            }
        }
    }
    pub fn enter_kingdom_place(&mut self, site_index: usize) {
        let geo = map::geography(&self.kingdom.wishes);
        let Some(site) = geo.sites.get(site_index) else {
            return;
        };
        let Some(i) = world::room_of(&self.dungeon, site.room) else {
            return;
        };
        if !self.kingdom.seen.contains(&site.room) && self.kingdom.seen.len() < 128 {
            self.kingdom.seen.push(site.room);
        }
        if self.kingdom.moments.errand == Some(site.room) {
            self.kingdom.moments.errand = None;
        }
        self.kingdom.return_to = Some(Return {
            x: (site.stand.0 as f32 + 0.5) * TILE_UNITS,
            y: (site.stand.1 as f32 + 0.7) * TILE_UNITS,
            name: site.name.clone(),
            root: site.room,
            party: self
                .players
                .iter()
                .map(|(&id, h)| (id, (h.x, h.y)))
                .collect(),
        });
        let arrival = match site.room {
            RoomKind::Gate => world::GATE_FRONT,
            RoomKind::CollegeGrounds => (12.0, 10.5),
            RoomKind::Ranch => (3.0, 10.5),
            RoomKind::Pasture => (12.0, 10.5),
            RoomKind::ColosseumHall => colosseum::ARRIVAL,
            _ => (12.0, 11.0),
        };
        self.arrive(i, arrival);
        self.safe_kingdom_arrival();
        self.sounds.push("door_open");
    }
    pub fn leave_kingdom_place(&mut self) {
        let Some(back) = self.kingdom.return_to.take() else {
            return;
        };
        self.raise_kingdom();
        let Some(i) = world::room_of(&self.dungeon, RoomKind::Kingdom) else {
            return;
        };
        self.arrive(i, (back.x / TILE_UNITS, back.y / TILE_UNITS));
        self.kingdom.door_held = true;
        for (&id, hero) in &mut self.players {
            if let Some(&(x, y)) = back.party.get(&id) {
                (hero.x, hero.y) = (x, y);
            }
            if hero.camera.is_none() {
                hero.camera = Some((hero.x, hero.y - 1.0));
            }
        }
        self.safe_kingdom_arrival();
        self.sounds.push("door_open");
    }
    /// Where knight `id` should head on the surface: the Delve until the
    /// party has been down once, then the nearest place nobody has stepped
    /// into yet this outing, and the Delve again once all are seen.
    pub fn kingdom_goal(&self, id: u32) -> Option<Goal> {
        if self.room().kind != RoomKind::Kingdom {
            return None;
        }
        let hero = self.players.get(&id)?;
        let geo = map::geography(&self.kingdom.wishes);
        let delve = geo.sites.iter().find(|s| s.room == RoomKind::Gate)?;
        let far = |s: &&map::Site| {
            let (x, y) = (s.stand.0 as f32 + 0.5, s.stand.1 as f32 + 0.7);
            (hero.x / TILE_UNITS - x).hypot(hero.y / TILE_UNITS - y)
        };
        let errand = self.kingdom.moments.errand.and_then(|room| {
            geo.sites
                .iter()
                .find(|s| s.room == room)
                .or_else(|| self.gateway(room, &geo))
        });
        let site = if let Some(errand) = errand {
            errand
        } else if self.home.deepest == 0 {
            delve
        } else if let Some(eyrie) = geo
            .sites
            .iter()
            .find(|s| s.room == RoomKind::Eyrie)
            .filter(|_| !self.race_won())
        {
            // Until somebody has won a race, the griffins are the goal.
            eyrie
        } else {
            geo.sites
                .iter()
                .filter(|s| {
                    !matches!(s.room, RoomKind::Gate | RoomKind::RealmHall)
                        && !self.kingdom.seen.contains(&s.room)
                })
                .min_by(|a, b| far(a).total_cmp(&far(b)))
                .unwrap_or(delve)
        };
        Some(Goal {
            name: if site.room == RoomKind::Gate {
                "The Delve".into()
            } else {
                site.name.clone()
            },
            stand: site.stand,
        })
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn gateway_for_test(&self, room: RoomKind, geo: &map::Geography) -> bool {
        geo.sites.iter().any(|s| s.room == room) || self.gateway(room, geo).is_some()
    }

    /// The kingdom site whose road leads to `room`: walk the home floor's
    /// doors out from it to the nearest place the map shows.
    fn gateway<'g>(&self, room: RoomKind, geo: &'g map::Geography) -> Option<&'g map::Site> {
        // The Colosseum's arenas are reached through its door.
        if colosseum::is_room(room) {
            return geo.sites.iter().find(|s| s.room == RoomKind::ColosseumHall);
        }
        let start = super::world::room_of(&self.dungeon, room)?;
        let mut seen = vec![false; self.dungeon.rooms.len()];
        let mut queue = std::collections::VecDeque::from([start]);
        seen[start] = true;
        while let Some(i) = queue.pop_front() {
            let kind = self.dungeon.rooms[i].kind;
            if let Some(site) = geo
                .sites
                .iter()
                .find(|s| s.room == kind && !matches!(kind, RoomKind::Gate | RoomKind::RealmHall))
            {
                return Some(site);
            }
            for dir in 0..4 {
                if let Some(j) = self.dungeon.neighbour(i, dir).filter(|&j| !seen[j]) {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        None
    }

    pub(super) fn tick_kingdom(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.tick_forest_north(inputs) || self.forest_road_showing() { return; }
        if self.tick_roundtable_entry(inputs) || self.roundtable_showing() { return; }
        self.tick_class_stones(inputs);
        self.tick_school(inputs);
        if self.room().kind != RoomKind::Kingdom {
            return;
        }
        let geo = map::geography(&self.kingdom.wishes);
        // Each knight crossing into another biome: its name, for a moment.
        let tick = self.tick;
        for (&id, hero) in &self.players {
            let here = crate::world::biome::at((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32).index();
            if self.kingdom.entered.get(&id).map(|&(b, _)| b) != Some(here) {
                self.kingdom.entered.insert(id, (here, tick));
            }
        }
        self.kingdom.entered.retain(|id, _| self.players.contains_key(id));
        // A door takes a fresh press: an E still down from before (the one
        // that came through a door) opens nothing until it is let go. Each
        // knight's own, so one knight holding E holds no one else's door.
        if self.kingdom.door_held {
            self.kingdom.door_held = false;
            self.kingdom.door_latched = self.players.keys().copied().collect();
        }
        let down = |id: &u32| inputs.get(id).is_some_and(|i| i.interact);
        self.kingdom.door_latched.retain(down);
        let near = geo.sites.iter().enumerate().find_map(|(index, site)| {
            let x = (site.stand.0 as f32 + 0.5) * TILE_UNITS;
            let y = (site.stand.1 as f32 + 0.7) * TILE_UNITS;
            self.players
                .iter()
                .find(|(id, h)| {
                    down(id)
                        && !self.kingdom.door_latched.contains(id)
                        && h.hp > 0
                        && !h.stone
                        && (h.x - x).hypot(h.y - y) < 2.1
                })
                .map(|_| index)
        });
        if let Some(index) = near {
            self.kingdom.door_held = true;
            self.enter_kingdom_place(index);
        }
    }
    pub fn follow_kingdom_cameras(&mut self) {
        if self.room().kind != RoomKind::Kingdom {
            return;
        }
        let (w, h) = (self.room().width(), self.room().height());
        let ease = self.tune.camera_follow;
        for hero in self.players.values_mut() {
            follow_camera(hero, w, h, ease);
        }
    }
}

/// One tick of a knight's kingdom camera following them, in a `w`x`h`-unit
/// map: a 48x28-unit viewport, a 5x3 dead zone, 180 ms easing at 30 Hz. A
/// client walks its own knight's camera with its own keys by the same step.
pub fn follow_camera(hero: &mut Hero, w: f32, h: f32, ease: f32) {
    let focus = (hero.x, hero.y - 1.0);
    let cam = hero.camera.unwrap_or(focus);
    let axis = |c: f32, f: f32, dead: f32, half: f32, bound: f32| {
        let target = if f > c + dead {
            f - dead
        } else if f < c - dead {
            f + dead
        } else {
            c
        };
        (c + (target - c) * ease).clamp(half, bound - half)
    };
    hero.camera = Some((
        axis(cam.0, focus.0, 5.0, WIDTH / 2.0, w),
        axis(cam.1, focus.1, 3.0, HEIGHT / 2.0, h),
    ));
}

#[cfg(test)]
#[path = "../../tests/rules/together_kingdom__tests.rs"]
mod tests;
