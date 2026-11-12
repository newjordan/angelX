//! Surface travel, bounded to the overview's geography and four seats.
use super::*;
use super::map::{self, MAP_H, MAP_W};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// The company is walking the replaceable player-built surface.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub frontier: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub industry: BTreeMap<u32, super::industry::Board>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub claim_bosses: BTreeMap<String, super::claim_bosses::Encounter>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ore_sentries: BTreeMap<String, super::ore_threats::Encounter>,
    /// How stirred the Deep is by the party's digging, the faces cut this
    /// expedition and the claims under raid (`stir`).
    #[serde(default, skip_serializing_if = "super::stir::Stir::is_calm")]
    pub stir: super::stir::Stir,
    /// The knights whose E was down last tick, so a press that takes a block
    /// from a friend is one press (`quarrying`). Never saved nor sent.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub carry_held: std::collections::BTreeSet<u32>,
    /// Capacity notices are once per seam/room entry, never save evidence.
    #[serde(skip)]
    pub claim_capacity_notice: super::frontier_claims::CapacityNotice,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub squire_yard: BTreeMap<u32, super::squires_town::Ledger>,
    pub wishes: std::sync::Arc<Vec<crate::realm::Wish>>,
    pub return_to: Option<Return>,
    pub door_held: bool,
    /// The knights whose E is still down from before the door was there
    /// for them (`tick_kingdom`): the server's own, never saved nor sent.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub door_latched: std::collections::BTreeSet<u32>,
    /// In a seat's own view only (`Run::view_for_seat`): this knight waits
    /// outside while the party plays on inside. Never saved nor sent.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
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
    /// The waystations: who talks with a keeper, the rally standing, the
    /// last coach to run.
    #[serde(skip_serializing_if = "super::waystation::Stops::is_idle")]
    pub stops: super::waystation::Stops,
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
    /// Each knight's surveyor's rod, while it is up (`raisings`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub survey: BTreeMap<u32, super::raisings::Survey>,
    /// Knights holding the next-card key since it last moved their rod, so
    /// one press is one plan.
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub survey_held: std::collections::BTreeSet<u32>,
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

/// How near a knight stands to a kingdom door for E to open it: the
/// reach its ENTER sign shows at (`render::overworld::arena::kingdom::
/// footer_sign`). A friend joins 2.4 units beside the first knight
/// (`Run::join_seat`), so a party arriving at a door is within it.
pub const DOOR_REACH: f32 = 4.5;

/// The kingdom door (`Geography::sites` index) a knight at `(x, y)` stands
/// at, as its sign says: the nearest within `DOOR_REACH`, unless a trinket
/// is nearer (there E is for that). The sign and the key ask this one
/// question, so ENTER on screen is a door E opens.
pub fn door_at(geo: &map::Geography, x: f32, y: f32) -> Option<usize> {
    let banner = super::trinkets::things(geo)
        .iter()
        .map(|&(_, p)| p)
        .map(|(bx, by)| (x - (bx as f32 + 0.5) * TILE_UNITS).hypot(y - (by as f32 + 0.5) * TILE_UNITS))
        .fold(f32::MAX, f32::min);
    geo.sites
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let (dx, dy) = ((s.stand.0 as f32 + 0.5) * TILE_UNITS, (s.stand.1 as f32 + 0.7) * TILE_UNITS);
            (i, (x - dx).hypot(y - dy))
        })
        .filter(|&(_, d)| d < DOOR_REACH && d <= banner)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Where the party is while this knight waits outside, and the spot by
/// the door where E takes them back in (`Run::detached_seats`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
        self.ore_sentries.len() <= super::claim_bosses::MAX_ENCOUNTERS
            && self.ore_sentries.iter().all(|(id, encounter)| !id.is_empty() && id.len() <= 96 && encounter.valid())
            && self.claim_bosses.len() <= super::claim_bosses::MAX_ENCOUNTERS
            && self.claim_bosses.iter().all(|(id, encounter)| !id.is_empty() && id.len() <= 96 && encounter.valid())
            && self.industry.len() <= MAX_PLAYERS as usize
            && self.industry.iter().all(|(id, board)| (1..=MAX_PLAYERS).contains(id) && board.valid())
            && self.wishes.len() <= crate::realm::MAX_WISHES
            && self.squire_yard.len() <= MAX_PLAYERS as usize
            && self.squire_yard.iter().all(|(id, l)| {
                (1..=MAX_PLAYERS).contains(id)
                    && l.owner.len() <= 256
                    && l.page.iter().all(|&p| p <= super::squires_lodge::MAX_ROSTER)
                    && l.holding.is_none_or(|(s, c)| s < 3 && c < 3)
            })
            && self.seen.len() <= 128
            && self.class_held.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.field_held.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.survey_held.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.survey.iter().all(|(id, s)| {
                (1..=MAX_PLAYERS).contains(id) && usize::from(s.pick) < super::raisings::PLANS.len()
            })
            && self.moments.valid()
            && self.stops.valid()
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
                (RoomKind::Gate, "alehouse-out" | "bazaar-out")
                    | (RoomKind::CollegeGrounds, "college-out")
                    | (RoomKind::ColosseumHall, "colosseum-out")
                    | (RoomKind::Ranch, "ranch-out")
                    | (RoomKind::Pasture, "fields-out")
                    | (world::bomrekedan::TOP, "bomrekedan-out")
            )
        })
    }
    pub fn raise_kingdom(&mut self) {
        if self.dungeon.depth != 0 {
            return;
        }
        if self.kingdom.frontier {
            self.refresh_frontier();
            return;
        }
        let geo = map::geography(&self.kingdom.wishes);
        // The map holds the Keep's site closed; what stands there open or
        // shut is the Works' (`site::patch_room`).
        let mut room = geo.room.clone();
        if crate::world::works::on_map() {
            super::site::patch_room(&mut room, &self.sites);
        }
        if let Some(i) = world::room_of(&self.dungeon, RoomKind::Kingdom) {
            if self.dungeon.rooms[i].fingerprint() != room.fingerprint() {
                // A save from before the kingdom opened carries the old,
                // smaller map: its spots on it mean nothing on this one.
                let resized = (self.dungeon.rooms[i].cols, self.dungeon.rooms[i].rows) != (geo.room.cols, geo.room.rows);
                self.dungeon.rooms[i] = room;
                self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
                if resized {
                    self.rehome_on_new_map(&geo);
                } else {
                    self.rehome_on_changed_ground(&geo);
                }
            }
        } else {
            self.dungeon.rooms.push(room);
            self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        }
        if world::room_of(&self.dungeon, RoomKind::RealmHall).is_none() {
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
        self.forget_what_is_gone();
        if self.room().kind == RoomKind::Kingdom {
            let at = unit(keep);
            for hero in self.players.values_mut() {
                (hero.x, hero.y) = at;
                hero.camera = Some((at.0, at.1 - 1.0));
            }
            self.safe_kingdom_arrival();
        }
    }
    /// After the blank-slate plan (`crate::world::plan`) has taken places off
    /// the map from under a saved run: what pointed at one of them points at
    /// nothing now. The errand and the rumour that named a place no door
    /// leads to, a coach's rally at a halt that is gone, a traveller walking
    /// a road that is gone, and the goose on its green are let go; the
    /// knights' standing and everything the realm owns stay.
    fn forget_what_is_gone(&mut self) {
        if !crate::world::plan::tabula() {
            return;
        }
        if self.kingdom.moments.errand.is_some_and(|room| !self.on_the_way(room)) {
            self.kingdom.moments.errand = None;
        }
        if self.wayfarers.rumoured.is_some_and(|room| !self.on_the_way(room)) {
            self.wayfarers.rumoured = None;
        }
        self.kingdom.stops = Default::default();
        self.kingdom.moments.goose = None;
        self.kingdom.moments.goose_at = None;
        if self.wayfarers.on.as_ref().is_some_and(|w| w.stage != super::wayfarers::Stage::Following) {
            self.wayfarers.on = None;
            self.wayfarers.walked = 0;
        }
        self.wayfarers.talks.clear();
        self.wayfarers.keys.clear();
        // Where a knight stood on the surface when it went into a place, if
        // the journal has it, may be closed ground now: it comes out at the
        // Keep's stand instead.
        if let Some(kingdom) = world::room_of(&self.dungeon, RoomKind::Kingdom) {
            let geo = map::geography(&self.kingdom.wishes);
            let stand = crate::world::kingdom::keep_stand();
            let keep = ((stand.0 as f32 + 0.5) * TILE_UNITS, (stand.1 as f32 + 0.7) * TILE_UNITS);
            let open = |&(x, y): &(f32, f32)| {
                x.is_finite() && y.is_finite() && geo.room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor
            };
            let entries = self.exits.entry.iter_mut().chain(self.exits.outside.values_mut());
            for entry in entries.filter(|e| e.at == kingdom) {
                for spot in entry.party.values_mut().filter(|spot| !open(spot)) {
                    *spot = keep;
                }
            }
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
                // On the blank-slate plan (`crate::world::plan`) most of the old
                // places are off the map: a return door to one of them (a save
                // taken inside the Village, say) comes out at the Keep.
                // The Delve's gate, whose name an older save may spell otherwise,
                // is where its own door now stands.
                None if crate::world::plan::tabula() => Some(
                    geo.sites
                        .iter()
                        .find(|s| s.room == back.root)
                        .map_or_else(crate::world::kingdom::keep_stand, |s| s.stand),
                ),
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
        self.forget_what_is_gone();
        if self.room().kind == RoomKind::Kingdom {
            self.safe_kingdom_arrival();
        }
    }
    pub fn walk_kingdom(&mut self, tile: Option<(i32, i32)>) {
        self.kingdom.frontier = false;
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
    pub(super) fn safe_kingdom_arrival(&mut self) {
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
        } else if world::bomrekedan::is_level(kind) {
            world::bomrekedan::TOP
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
        self.rumour_comes_true(&geo, site);
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
            world::bomrekedan::TOP => world::bomrekedan::ARRIVAL,
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
        self.sounds.push("area_leave");
    }
    /// Whether a place can be walked to from the surface: always in the full
    /// plan, where every place has its door on the map; on the blank-slate
    /// plan (`crate::world::plan`) only the Delve's gate is on the map, so a
    /// place is "on the way" only when the home floor's doors and stairs lead
    /// to it from there. What points a knight at a place (an errand, a
    /// gossip's line, a herald's call) asks this first, so nobody is sent
    /// where no door goes.
    pub fn on_the_way(&self, room: RoomKind) -> bool {
        if !crate::world::plan::tabula() {
            return true;
        }
        let geo = map::geography(&self.kingdom.wishes);
        if geo.sites.iter().any(|s| s.room == room) {
            return true;
        }
        // Out from the Delve's gate by the home floor's doors and stairs.
        let rooms = &self.dungeon.rooms;
        let (Some(gate), Some(goal)) = (world::room_of(&self.dungeon, RoomKind::Gate), world::room_of(&self.dungeon, room)) else {
            return false;
        };
        let mut seen = vec![false; rooms.len()];
        let mut queue = std::collections::VecDeque::from([gate]);
        seen[gate] = true;
        while let Some(i) = queue.pop_front() {
            if i == goal {
                return true;
            }
            let kind = rooms[i].kind;
            let doors = (0..4).filter_map(|dir| self.dungeon.neighbour(i, dir));
            let stairs = world::ENTRANCES.iter().filter_map(|e| match e.to {
                world::Dest::Room(to, _) if e.room == kind => world::room_of(&self.dungeon, to),
                _ => None,
            });
            for j in doors.chain(stairs).collect::<Vec<_>>() {
                if !seen[j] {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        false
    }
    /// Where knight `id` should head on the surface: the Delve until the
    /// party has been down once, then the nearest place nobody has stepped
    /// into yet this outing, and the Delve again once all are seen.
    pub fn kingdom_goal(&self, id: u32) -> Option<Goal> {
        if self.room().kind != RoomKind::Kingdom {
            return None;
        }
        let hero = self.players.get(&id)?;
        // A rally called at a halt calls every knight not yet near it.
        if let Some((halt, stand)) = self.rally_call(id) {
            return Some(Goal { name: format!("RALLY: {}", halt.to_uppercase()), stand });
        }
        // A knight who has carried a block up from below has one place to go: the stockyard,
        // where it is set down (`quarrying`).
        if crate::world::plan::tabula() && hero.block.is_some() {
            let (x, y) = super::quarrying::yard_spot();
            return Some(Goal { name: "THE STOCKYARD".into(), stand: ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) });
        }
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
    pub(super) fn gateway<'g>(&self, room: RoomKind, geo: &'g map::Geography) -> Option<&'g map::Site> {
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
        if self.room().kind != RoomKind::Kingdom {
            // A knight who has gone below carries nothing of the Keep's.
            self.release_all_hands();
            self.kingdom.stops.talks.clear();
            self.kingdom.stops.held.clear();
            return;
        }
        // The masons at the Keep's wall, and the tiles under it.
        self.tick_sites(inputs);
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
        self.tick_survey(inputs);
        // E wakes a knight still stone from its arrival, and that same
        // press is for the door it stands at.
        for (id, hero) in &mut self.players {
            if let Some(input) = inputs.get(id).filter(|i| i.valid() && i.interact) {
                Run::wake_on_first_move(hero, input);
            }
        }
        self.tick_stops(inputs);
        // The door each knight's sign shows (`door_at`), the first knight
        // pressing a fresh E at one. A knight with the rod up is building.
        let near = self.players.iter().find_map(|(id, h)| {
            let free = !self.kingdom.door_latched.contains(id) && !self.kingdom.survey.contains_key(id);
            (down(id) && free && h.hp > 0 && !h.stone)
                .then(|| door_at(&geo, h.x, h.y))
                .flatten()
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

#[cfg(test)]
#[path = "../../tests/rules/tabula_sweep__tests.rs"]
mod tabula_sweep_tests;
