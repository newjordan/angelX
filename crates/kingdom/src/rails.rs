//! Rails as a network, not a string between two buildings.
//!
//! Rail is laid tile by tile and every tile that touches another joins it: a
//! tile with three or four neighbours is a **switch**, and the wagons set it
//! themselves as they go, each by its own route. Where a wagon loads and
//! unloads is a **bay**: a rail tile beside a building, with a name. A
//! **route** is a list of stops, each a bay and what to do there (take on goods,
//! put them off, or both), and the **wagons** that run it round and round, a
//! horse to each.
//!
//! Wagons are kept apart by **blocks**. A plain tile of track is two lanes, one
//! each way, and holds a wagon in each; a switch, a bay and the end of a track
//! hold one wagon and nobody else. A wagon takes the next tile only when it
//! may: so wagons meeting at a switch take turns, a wagon at a bay holds it
//! while it loads and the ones behind queue on the rail, and a ring of wagons
//! each waiting on the next stands where it is. **A jam is real**: a blocked
//! wagon waits, looks for another way round, and if there is none it stands and
//! says so (`rail_jams`). The cure is a bypass round the bay, a bay on a spur
//! of its own, a second track, and the wagons find their way into it by themselves.
//!
//! The numbers (tile and wagon prices, speed, load, how long a bay takes, the
//! horse's fodder) are the tramway's in the catalogue (`lines.tram`): this
//! replaces it.
use crate::economy::{add, covers, neighbours, take};
use crate::rollers::Dir;
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The most tiles of rail, routes and wagons a kingdom may have.
pub const MAX_RAILS: usize = 3_000;
pub const MAX_ROUTES: usize = 40;
pub const MAX_WAGONS: usize = 120;
/// Units a wagon's hands move at a bay in a second.
const HANDS: u32 = 2;
/// Seconds a wagon waits at a bay with nothing moving before it goes on.
const PATIENCE: u8 = 10;
/// Seconds blocked before a wagon looks for another way round, and before it
/// is reported as jammed.
const LOOK_AROUND: u16 = 4;
pub const JAMMED: u16 = 20;

/// A tile of rail.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rail {
    /// The name of the bay that stands here, if one does.
    pub bay: Option<String>,
}

/// A named place to load and unload: a rail tile beside a building.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bay {
    pub tile: Tile,
    /// The way from the rail to the building it serves.
    pub side: Dir,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Act {
    /// Take on goods.
    #[default]
    Load,
    /// Put goods off.
    Unload,
    /// Put off what the place takes, then take on.
    Both,
}
impl Act {
    pub fn name(self) -> &'static str {
        match self {
            Act::Load => "LOAD",
            Act::Unload => "UNLOAD",
            Act::Both => "LOAD AND UNLOAD",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stop {
    pub bay: String,
    pub act: Act,
    /// The one good it loads or unloads; none takes whatever there is.
    #[serde(default)]
    pub good: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Route {
    pub name: String,
    pub stops: Vec<Stop>,
    /// Seconds the horses' fodder lasts.
    pub fed: u16,
    /// Goods delivered by this route's wagons, and trips made.
    pub moved: u64,
    pub trips: u32,
    /// Units unloaded in each of the last six ten-second shifts, oldest first, and the shift in hand.
    pub recent: [u16; 6],
    pub shift: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// On the way to its stop.
    #[default]
    Travel,
    /// At the bay, working.
    Dwell,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wagon {
    pub route: u32,
    pub tile: Tile,
    /// The tile it is moving on to, and tenths of a second it has been on the way.
    pub to: Option<Tile>,
    pub clock: u16,
    pub face: Dir,
    /// The stop it is bound for, or working at.
    pub stop: u8,
    pub phase: Phase,
    /// Seconds at the bay, and seconds of them with nothing moving.
    pub dwell: u8,
    pub idle: u8,
    pub good: String,
    pub amount: u32,
    /// Seconds it has been held up by another wagon.
    pub waiting: u16,
    /// The tile of the wagon that holds it up, for the report.
    pub behind: Option<Tile>,
}

mod as_list {
    use super::*;
    pub fn serialize<S: serde::Serializer>(m: &BTreeMap<Tile, Rail>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(m.iter())
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<Tile, Rail>, D::Error> {
        let v: Vec<(Tile, Rail)> = Vec::deserialize(d)?;
        Ok(v.into_iter().collect())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rails {
    #[serde(with = "as_list")]
    pub cells: BTreeMap<Tile, Rail>,
    pub bays: BTreeMap<String, Bay>,
    pub routes: BTreeMap<u32, Route>,
    pub wagons: BTreeMap<u32, Wagon>,
    pub next_route: u32,
    pub next_wagon: u32,
}

impl Rails {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.wagons.is_empty()
    }
    /// The neighbours of a tile that are rail: the ways out of it.
    pub fn links(&self, t: Tile) -> Vec<Tile> {
        neighbours(t).into_iter().filter(|n| self.cells.contains_key(n)).collect()
    }
    /// Whether a tile is a switch: three or four ways out.
    pub fn is_switch(&self, t: Tile) -> bool {
        self.cells.contains_key(&t) && self.links(t).len() >= 3
    }
    /// The shortest way over rail from `from` to `to`, avoiding `blocked`, as the tiles after `from`.
    pub fn path(&self, from: Tile, to: Tile, blocked: &BTreeSet<Tile>) -> Option<Vec<Tile>> {
        if from == to {
            return Some(Vec::new());
        }
        let mut parent: BTreeMap<Tile, Tile> = BTreeMap::from([(from, from)]);
        let mut queue = VecDeque::from([from]);
        while let Some(t) = queue.pop_front() {
            for n in neighbours(t) {
                if !self.cells.contains_key(&n) || parent.contains_key(&n) || (blocked.contains(&n) && n != to) {
                    continue;
                }
                parent.insert(n, t);
                if n == to {
                    let mut way = vec![n];
                    let mut at = t;
                    while at != from {
                        way.push(at);
                        at = parent[&at];
                    }
                    way.reverse();
                    return Some(way);
                }
                queue.push_back(n);
            }
        }
        None
    }
    /// The tiles a wagon holds: where it is and where it is going.
    fn held(&self) -> BTreeMap<Tile, u32> {
        let mut m = BTreeMap::new();
        for (&id, w) in &self.wagons {
            m.insert(w.tile, id);
            if let Some(t) = w.to {
                m.insert(t, id);
            }
        }
        m
    }
}

/// What a wagon is doing, for the report and the picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WagonState {
    Moving,
    Loading,
    Unloading,
    /// Held up by another wagon.
    Queued,
    /// Held up for so long that it is a jam.
    Jammed,
    /// Its horse has no fodder.
    Unfed,
}
impl WagonState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Moving => "RUNNING",
            Self::Loading => "LOADING",
            Self::Unloading => "UNLOADING",
            Self::Queued => "WAITING FOR THE TRACK",
            Self::Jammed => "JAMMED",
            Self::Unfed => "NO FODDER",
        }
    }
}

/// One route, as the player reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteRow {
    pub id: u32,
    pub name: String,
    pub stops: usize,
    pub wagons: usize,
    /// Units a minute unloaded over the last minute.
    pub per_min: u32,
    /// Units a minute the wagons could carry with goods always waiting: a rough ceiling.
    pub capacity_per_min: u32,
    pub state: WagonState,
}

/// A knot of wagons that cannot go on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailJam {
    pub tile: Tile,
    pub wagons: Vec<u32>,
    pub seconds: u16,
    pub why: String,
}

impl Kingdom {
    pub(crate) fn rail_spec<'a>(&self, c: &'a Catalog) -> &'a LineSpec {
        &c.lines["tram"]
    }

    /// The building a bay serves, if one stands there.
    pub fn bay_building(&self, name: &str) -> Option<u32> {
        let bay = self.rails.bays.get(name)?;
        self.at(bay.side.step(bay.tile))
    }

    /// The state of one wagon.
    pub fn wagon_state(&self, c: &Catalog, id: u32) -> WagonState {
        let Some(w) = self.rails.wagons.get(&id) else { return WagonState::Moving };
        let spec = self.rail_spec(c);
        if !spec.upkeep.is_empty() && self.rails.routes.get(&w.route).is_none_or(|r| r.fed == 0) {
            return WagonState::Unfed;
        }
        if w.waiting >= JAMMED {
            WagonState::Jammed
        } else if w.waiting > 0 {
            WagonState::Queued
        } else if w.phase == Phase::Dwell {
            match self.rails.routes.get(&w.route).and_then(|r| r.stops.get(usize::from(w.stop))).map(|s| s.act) {
                Some(Act::Unload) => WagonState::Unloading,
                Some(Act::Both) if w.amount > 0 => WagonState::Unloading,
                _ => WagonState::Loading,
            }
        } else {
            WagonState::Moving
        }
    }

    // ---- laying -----------------------------------------------------------

    pub fn check_rail_site(&self, c: &Catalog, tile: Tile) -> Result<(), String> {
        let _ = c;
        if !self.world.is_hillside() {
            return Err("rails run on the hillside".into());
        }
        if !self.inside(tile) || !self.world.owned(tile) {
            return Err("that ground is not yours to lay on".into());
        }
        if !self.world.terrain(tile).walkable() {
            return Err("rail cannot cross the river".into());
        }
        if tile == self.gate() || tile == self.camp() {
            return Err("that ground is kept clear".into());
        }
        if let Some(why) = self.reserved_ground(tile) {
            return Err(why);
        }
        if self.at(tile).is_some() {
            return Err("a building stands here".into());
        }
        if self.roads.contains(&tile) {
            return Err("a road runs here".into());
        }
        if PowerKind::ALL.into_iter().any(|kind| self.power.contains(kind, tile)) {
            return Err("a drive shaft runs here".into());
        }
        if self.rollers.cells.get(&tile).is_some_and(|x| x.piece != crate::rollers::Piece::Trestle) {
            return Err("a rollerway runs here: a trestle can carry it over".into());
        }
        Ok(())
    }

    pub(crate) fn lay_rail(&mut self, c: &Catalog, tile: Tile) -> Result<String, String> {
        let spec = self.rail_spec(c).clone();
        if !spec.requires.iter().all(|r| self.researched.contains(r)) {
            return Err(format!("research {} first", spec.requires.join(", ")));
        }
        if self.rails.cells.contains_key(&tile) {
            return Err("rail is laid here already".into());
        }
        if self.rails.cells.len() >= MAX_RAILS {
            return Err("too much rail".into());
        }
        self.check_rail_site(c, tile)?;
        if !covers(&self.stock, &spec.tile_cost) {
            return Err(format!("a tile of rail needs {}", crate::economy::missing(&self.stock, &spec.tile_cost)));
        }
        take(&mut self.stock, &spec.tile_cost);
        self.world.clear_forest(tile);
        self.rails.cells.insert(tile, Rail::default());
        Ok("Rail laid".into())
    }

    /// Take a tile of rail up: a wagon on it goes to the camp's stable, a bay on it is gone.
    pub(crate) fn lift_rail(&mut self, tile: Tile) -> Result<String, String> {
        if !self.rails.cells.contains_key(&tile) {
            return Err("no rail here".into());
        }
        if let Some((&id, _)) = self.rails.wagons.iter().find(|(_, w)| w.tile == tile || w.to == Some(tile)) {
            return Err(format!("a wagon stands on it (wagon {id})"));
        }
        let rail = self.rails.cells.remove(&tile).unwrap();
        if let Some(name) = rail.bay {
            self.rails.bays.remove(&name);
            // The stops of that bay go with it.
            for r in self.rails.routes.values_mut() {
                r.stops.retain(|s| s.bay != name);
            }
        }
        self.reap_routes();
        Ok("Rail taken up".into())
    }

    /// A route with fewer than two stops runs nowhere: it is taken down and its wagons go home.
    fn reap_routes(&mut self) {
        let dead: Vec<u32> = self.rails.routes.iter().filter(|(_, r)| r.stops.len() < 2).map(|(&id, _)| id).collect();
        for id in dead {
            self.remove_route(id);
        }
    }

    /// A name for a bay beside a building: its plan's name, and a number when there is more than one.
    fn bay_name(&self, c: &Catalog, building: u32) -> String {
        let base = self.buildings.get(&building).map_or("Bay".to_string(), |b| c.plans[&b.plan].name.clone());
        let mut n = 1;
        loop {
            let name = if n == 1 { base.clone() } else { format!("{base} {n}") };
            if !self.rails.bays.contains_key(&name) {
                return name;
            }
            n += 1;
        }
    }

    /// Name the rail tile `tile` as a bay serving the building on `side` of it (or the first building beside it).
    pub(crate) fn make_bay(&mut self, c: &Catalog, tile: Tile, side: Option<Dir>, name: Option<String>) -> Result<String, String> {
        let rail = self.rails.cells.get(&tile).ok_or("a bay is a tile of rail")?;
        if rail.bay.is_some() {
            return Err("it is a bay already".into());
        }
        let side = side
            .filter(|d| self.at(d.step(tile)).is_some_and(|id| self.usable_rail_end(c, id)))
            .or_else(|| Dir::ALL.into_iter().find(|d| self.at(d.step(tile)).is_some_and(|id| self.usable_rail_end(c, id))))
            .ok_or("a bay stands beside a building")?;
        let id = self.at(side.step(tile)).unwrap();
        let name = name.filter(|n| !n.trim().is_empty()).map(|n| n.trim().to_string()).unwrap_or_else(|| self.bay_name(c, id));
        if name.len() > 32 || self.rails.bays.contains_key(&name) {
            return Err("a bay needs a name of its own".into());
        }
        self.rails.bays.insert(name.clone(), Bay { tile, side });
        self.rails.cells.get_mut(&tile).unwrap().bay = Some(name.clone());
        Ok(format!("The {name} bay"))
    }

    fn usable_rail_end(&self, c: &Catalog, id: u32) -> bool {
        self.buildings.get(&id).is_some_and(|b| c.plans[&b.plan].swape == 0)
    }

    /// Run `stops` with `wagons` wagons, which are bought and stood at the first bay.
    pub(crate) fn make_route(&mut self, c: &Catalog, name: Option<String>, stops: Vec<Stop>, wagons: u16) -> Result<String, String> {
        if stops.len() < 2 || stops.len() > 12 {
            return Err("a route takes two to twelve stops".into());
        }
        for s in &stops {
            if !self.rails.bays.contains_key(&s.bay) {
                return Err(format!("no bay named {}", s.bay));
            }
            if s.good.as_ref().is_some_and(|g| !c.resources.contains_key(g)) {
                return Err("no such good".into());
            }
        }
        if self.rails.routes.len() >= MAX_ROUTES {
            return Err("too many routes".into());
        }
        let id = self.rails.next_route.max(1);
        let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("{} to {}", stops[0].bay, stops[stops.len() - 1].bay));
        self.rails.routes.insert(id, Route { name: name.clone(), stops, ..Route::default() });
        self.rails.next_route = id + 1;
        if wagons > 0 {
            if let Err(e) = self.add_wagons(c, id, wagons) {
                self.rails.routes.remove(&id);
                return Err(e);
            }
        }
        Ok(format!("The {name} route runs"))
    }

    /// Buy `count` wagons for a route and stand them at its first bay and along the rail behind it.
    pub(crate) fn add_wagons(&mut self, c: &Catalog, route: u32, count: u16) -> Result<String, String> {
        let spec = self.rail_spec(c).clone();
        let r = self.rails.routes.get(&route).ok_or("no such route")?;
        let first = self.rails.bays.get(&r.stops[0].bay).ok_or("its first bay is gone")?.tile;
        let have = self.rails.wagons.values().filter(|w| w.route == route).count();
        if count == 0 || have + usize::from(count) > usize::from(spec.max_vehicles) || self.rails.wagons.len() + usize::from(count) > MAX_WAGONS {
            return Err(format!("a route takes 1 to {} wagons", spec.max_vehicles));
        }
        let mut cost = Stock::new();
        for (g, n) in &spec.vehicle_cost {
            add(&mut cost, g, n * u32::from(count));
        }
        if !covers(&self.stock, &cost) {
            return Err(format!("{count} wagons need {}", crate::economy::missing(&self.stock, &cost)));
        }
        // Room for them: the first bay's tile and the free rail nearest it.
        let held = self.rails.held();
        let mut places = Vec::new();
        let mut seen = BTreeSet::from([first]);
        let mut queue = VecDeque::from([first]);
        while let Some(t) = queue.pop_front() {
            if !held.contains_key(&t) {
                places.push(t);
                if places.len() == usize::from(count) {
                    break;
                }
            }
            for n in self.rails.links(t) {
                if seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        if places.len() < usize::from(count) {
            return Err("there is no room on the rail by the first bay".into());
        }
        take(&mut self.stock, &cost);
        for tile in places {
            let id = self.rails.next_wagon.max(1);
            self.rails.next_wagon = id + 1;
            self.rails.wagons.insert(id, Wagon { route, tile, ..Wagon::default() });
        }
        // Fed from the first second: the horses come with a day's hay.
        let upkeep = spec.upkeep_seconds;
        let rt = self.rails.routes.get_mut(&route).unwrap();
        if rt.fed == 0 {
            rt.fed = upkeep;
        }
        Ok(format!("{count} wagons on the route"))
    }

    /// Take a route down: its wagons are sold (half their price) and what they carry goes to the camp.
    pub(crate) fn remove_route(&mut self, id: u32) -> bool {
        if self.rails.routes.remove(&id).is_none() {
            return false;
        }
        let ids: Vec<u32> = self.rails.wagons.iter().filter(|(_, w)| w.route == id).map(|(&i, _)| i).collect();
        for i in ids {
            if let Some(w) = self.rails.wagons.remove(&i) {
                add(&mut self.stock, &w.good, w.amount);
            }
        }
        true
    }

    /// Sell one wagon of a route (the last), standing at its first bay or anywhere free to be sold.
    pub(crate) fn sell_wagon(&mut self, c: &Catalog, route: u32) -> Result<String, String> {
        let id = *self.rails.wagons.iter().rev().find(|(_, w)| w.route == route).map(|(i, _)| i).ok_or("no wagon on that route")?;
        let w = self.rails.wagons.remove(&id).unwrap();
        add(&mut self.stock, &w.good, w.amount);
        for (g, n) in &self.rail_spec(c).vehicle_cost.clone() {
            add(&mut self.stock, g, n / 2);
        }
        Ok("A wagon sold".into())
    }

    // ---- the day's work ---------------------------------------------------------

    /// One second of every wagon on the rails.
    pub(crate) fn rails_tick(&mut self, c: &Catalog, report: &Report) {
        if self.rails.wagons.is_empty() {
            return;
        }
        let spec = self.rail_spec(c).clone();
        let capacity = report.capacity;
        // The horses eat: a route's fodder lasts, then is drawn again from the camp.
        let ids: Vec<u32> = self.rails.routes.keys().copied().collect();
        for id in ids {
            let wagons = self.rails.wagons.values().filter(|w| w.route == id).count() as u32;
            let fed = self.rails.routes[&id].fed;
            if fed > 0 {
                self.rails.routes.get_mut(&id).unwrap().fed -= 1;
            } else if wagons > 0 {
                let mut cost = Stock::new();
                for (g, n) in &spec.upkeep {
                    add(&mut cost, g, n * wagons);
                }
                if covers(&self.stock, &cost) {
                    take(&mut self.stock, &cost);
                    self.rails.routes.get_mut(&id).unwrap().fed = spec.upkeep_seconds;
                }
            }
            if self.second % 10 == 0 {
                let r = self.rails.routes.get_mut(&id).unwrap();
                r.recent.rotate_left(1);
                r.recent[5] = std::mem::take(&mut r.shift);
            }
        }
        let mut order: Vec<u32> = self.rails.wagons.keys().copied().collect();
        let turn = (self.second % order.len() as u64) as usize;
        order.rotate_left(turn);
        for id in order {
            self.step_wagon(c, id, &spec, capacity);
        }
    }

    fn step_wagon(&mut self, c: &Catalog, id: u32, spec: &LineSpec, capacity: u32) {
        let Some(mut w) = self.rails.wagons.remove(&id) else { return };
        let Some(route) = self.rails.routes.get(&w.route).cloned() else {
            add(&mut self.stock, &w.good, w.amount);
            return;
        };
        let unfed = !spec.upkeep.is_empty() && route.fed == 0;
        let n = route.stops.len();
        w.stop %= n.max(1) as u8;
        let stop = route.stops[usize::from(w.stop)].clone();
        let bay = self.rails.bays.get(&stop.bay).cloned();
        let put_back = |k: &mut Kingdom, w: Wagon| {
            k.rails.wagons.insert(id, w);
        };
        if unfed {
            put_back(self, w);
            return;
        }
        let Some(bay) = bay else {
            // Its bay is gone: on to the next stop.
            w.stop = (w.stop + 1) % n as u8;
            put_back(self, w);
            return;
        };
        match w.phase {
            Phase::Dwell => {
                let route_id = w.route;
                self.dwell(c, &mut w, &stop, &bay, spec, capacity, route_id);
                if w.phase == Phase::Travel {
                    w.stop = (w.stop + 1) % n as u8;
                    w.waiting = 0;
                    w.behind = None;
                    if let Some(r) = self.rails.routes.get_mut(&w.route) {
                        if w.stop == 0 {
                            r.trips += 1;
                        }
                    }
                }
                put_back(self, w);
            }
            Phase::Travel => {
                // On the way between tiles.
                if let Some(to) = w.to {
                    w.clock += crate::pace::travel_clock();
                    if w.clock >= spec.speed {
                        w.face = Dir::between(w.tile, to).unwrap_or(w.face);
                        w.tile = to;
                        w.to = None;
                        w.clock = 0;
                    } else {
                        put_back(self, w);
                        return;
                    }
                }
                if w.tile == bay.tile {
                    w.phase = Phase::Dwell;
                    w.dwell = 0;
                    w.idle = 0;
                    w.waiting = 0;
                    w.behind = None;
                    put_back(self, w);
                    return;
                }
                // Choose the next tile: the shortest way, or another when the shortest is held.
                let mut way = self.rails.path(w.tile, bay.tile, &BTreeSet::new()).unwrap_or_default();
                if way.is_empty() {
                    // Cut off from its bay: it stands.
                    w.waiting = w.waiting.saturating_add(1);
                    w.behind = None;
                    put_back(self, w);
                    return;
                }
                // A wagon stands in the lane of the way it means to go (so that one coming the other
                // way can pass it), if that lane is free to it.
                if let Some(turn) = Dir::between(w.tile, way[0]) {
                    let others: Vec<Dir> = self.rails.wagons.values().filter(|o| o.tile == w.tile).map(|o| o.face).collect();
                    if others.iter().all(|f| *f == turn.back()) {
                        w.face = turn;
                    }
                }
                if let Err(blocker) = self.may_take(w.tile, way[0]) {
                    w.waiting = w.waiting.saturating_add(1);
                    w.behind = Some(blocker);
                    if w.waiting >= LOOK_AROUND {
                        // Another way round, if the rails have one: a bypass, a second track.
                        let mut shut = self.shut();
                        shut.remove(&w.tile);
                        if let Some(round) = self.rails.path(w.tile, bay.tile, &shut) {
                            way = round;
                        }
                    }
                }
                if let Some(&next) = way.first() {
                    if self.may_take(w.tile, next).is_ok() {
                        w.to = Some(next);
                        w.clock = 0;
                        w.waiting = 0;
                        w.behind = None;
                    }
                }
                put_back(self, w);
            }
        }
    }

    /// Whether wagon `id`, on `from`, may take the tile `next` now; if not, the tile of the wagon in the way.
    /// (The wagon is not in the map while it is stepped.)
    fn may_take(&self, from: Tile, next: Tile) -> Result<(), Tile> {
        let exclusive = self.rails.links(next).len() != 2 || self.rails.cells.get(&next).is_some_and(|r| r.bay.is_some());
        let mine = Dir::between(from, next);
        for o in self.rails.wagons.values() {
            let (there, coming) = (o.tile == next, o.to == Some(next));
            if !there && !coming {
                continue;
            }
            // A tile of plain track has a lane each way: a wagon coming the other way, not stopped to work, passes.
            let theirs = if coming { Dir::between(o.tile, next) } else { Some(o.face) };
            let passes = !exclusive && !(there && o.phase == Phase::Dwell) && matches!((mine, theirs), (Some(m), Some(t)) if t == m.back());
            if !passes {
                return Err(if there { o.tile } else { o.to.unwrap_or(o.tile) });
            }
        }
        Ok(())
    }

    /// The tiles that no wagon can go through now: those held by one, where it holds the whole of the tile.
    fn shut(&self) -> BTreeSet<Tile> {
        let mut shut = BTreeSet::new();
        for o in self.rails.wagons.values() {
            for t in [Some(o.tile), o.to].into_iter().flatten() {
                if self.rails.links(t).len() != 2 || self.rails.cells.get(&t).is_some_and(|r| r.bay.is_some()) || (t == o.tile && o.phase == Phase::Dwell) {
                    shut.insert(t);
                }
            }
        }
        shut
    }

    #[allow(clippy::too_many_arguments)]
    fn dwell(&mut self, c: &Catalog, w: &mut Wagon, stop: &Stop, bay: &Bay, spec: &LineSpec, capacity: u32, route: u32) {
        w.dwell = w.dwell.saturating_add(1);
        // It takes the berth's time to stand and set the brake.
        if u16::from(w.dwell) <= spec.dwell {
            return;
        }
        let place = self.at(bay.side.step(bay.tile)).filter(|b| self.usable_here(c, *b));
        let cap = u32::from(spec.capacity);
        let mut moved = 0u32;
        let ok = |g: &str| stop.good.as_deref().is_none_or(|x| x == g);
        if let Some(b) = place {
            if matches!(stop.act, Act::Unload | Act::Both) && w.amount > 0 && ok(&w.good) {
                // Put off what the place takes.
                let room = self.room_here(c, b, &w.good, capacity);
                let n = w.amount.min(HANDS).min(room);
                if n > 0 {
                    self.give(c, b, &w.good, n);
                    w.amount -= n;
                    moved += n;
                    if let Some(r) = self.rails.routes.get_mut(&route) {
                        r.moved += u64::from(n);
                        r.shift = r.shift.saturating_add(n as u16);
                    }
                }
            } else if matches!(stop.act, Act::Load | Act::Both) && w.amount < cap {
                // The good it already carries, or the one asked for, or the one there is most of.
                let take_good = if w.amount > 0 {
                    Some(w.good.clone())
                } else {
                    let mut best: Option<(u32, String)> = None;
                    for r in self.store_resources(c, b) {
                        if !ok(&r) {
                            continue;
                        }
                        let have = self.avail(c, b, &r, 0);
                        if have > best.as_ref().map_or(0, |x| x.0) {
                            best = Some((have, r));
                        }
                    }
                    best.map(|(_, r)| r)
                };
                if let Some(g) = take_good.filter(|g| ok(g)) {
                    let n = (cap - w.amount).min(HANDS).min(self.avail(c, b, &g, 0));
                    if n > 0 {
                        self.draw(b, &g, n);
                        w.good = g;
                        w.amount += n;
                        moved += n;
                    }
                }
            }
        }
        if moved > 0 {
            w.idle = 0;
        } else {
            w.idle = w.idle.saturating_add(1);
        }
        let done = match stop.act {
            Act::Load => w.amount >= cap || w.idle >= PATIENCE,
            Act::Unload => w.amount == 0 || w.idle >= PATIENCE,
            Act::Both => w.amount >= cap || w.idle >= PATIENCE,
        };
        if done {
            w.phase = Phase::Travel;
            w.dwell = 0;
            w.idle = 0;
        }
    }

    fn usable_here(&self, c: &Catalog, id: u32) -> bool {
        id == 1 || self.buildings.get(&id).is_some_and(|b| b.construction == 0 && c.plans[&b.plan].swape == 0)
    }

    fn room_here(&self, c: &Catalog, id: u32, r: &str, capacity: u32) -> u32 {
        if !self.usable_here(c, id) || (id != 1 && self.buildings[&id].paused) {
            return 0;
        }
        let have = self.have_at(c, id, r);
        let cap = self.cap_at(c, id, capacity);
        let limit = match self.target_levels(c, id) {
            Some(levels) => levels.get(r).copied().unwrap_or(0).min(cap),
            None => cap,
        };
        limit.saturating_sub(have)
    }

    // ---- reading ----------------------------------------------------------------

    /// Goods riding the wagons.
    pub fn railed(&self, resource: &str) -> u32 {
        self.rails.wagons.values().filter(|w| w.good == resource).map(|w| w.amount).sum()
    }

    pub fn route_report(&self, c: &Catalog) -> Vec<RouteRow> {
        let spec = self.rail_spec(c);
        self.rails
            .routes
            .iter()
            .map(|(&id, r)| {
                let ws: Vec<u32> = self.rails.wagons.iter().filter(|(_, w)| w.route == id).map(|(&i, _)| i).collect();
                let states: Vec<WagonState> = ws.iter().map(|i| self.wagon_state(c, *i)).collect();
                let state = [WagonState::Unfed, WagonState::Jammed, WagonState::Queued, WagonState::Loading, WagonState::Unloading]
                    .into_iter()
                    .find(|s| states.contains(s))
                    .unwrap_or(WagonState::Moving);
                // A wagon's round: the way round at its speed, and a berth at each stop.
                let length: u32 = r
                    .stops
                    .windows(2)
                    .filter_map(|p| {
                        let (a, b) = (self.rails.bays.get(&p[0].bay)?, self.rails.bays.get(&p[1].bay)?);
                        self.rails.path(a.tile, b.tile, &BTreeSet::new()).map(|w| w.len() as u32)
                    })
                    .sum::<u32>()
                    * 2;
                let cycle = (length * u32::from(spec.speed)).div_ceil(10) + r.stops.len() as u32 * (u32::from(spec.dwell) + 4);
                RouteRow {
                    id,
                    name: r.name.clone(),
                    stops: r.stops.len(),
                    wagons: ws.len(),
                    per_min: r.recent.iter().map(|&n| u32::from(n)).sum(),
                    capacity_per_min: ws.len() as u32 * u32::from(spec.capacity) * 60 / cycle.max(1),
                    state,
                }
            })
            .collect()
    }

    /// Wagons that cannot go on, and why: the knots of the track.
    pub fn rail_jams(&self, c: &Catalog) -> Vec<RailJam> {
        let mut jams = Vec::new();
        for (&id, w) in &self.rails.wagons {
            if w.waiting < JAMMED {
                continue;
            }
            let other = w.behind.and_then(|t| self.rails.wagons.iter().find(|(_, o)| o.tile == t || o.to == Some(t)));
            let why = match other {
                Some((&oid, o)) => {
                    let mutual = o.waiting > 0 && o.behind == Some(w.tile);
                    if mutual {
                        format!("wagons {id} and {oid} are nose to nose on a single track: a loop would let one pass")
                    } else if o.phase == Phase::Dwell {
                        let at = self.rails.cells.get(&o.tile).and_then(|r| r.bay.clone()).unwrap_or_else(|| "a bay".into());
                        format!("wagon {id} queues behind wagon {oid}, which holds the {at} bay")
                    } else {
                        format!("wagon {id} waits behind wagon {oid}")
                    }
                }
                None => format!("wagon {id} has no way on to its bay"),
            };
            jams.push(RailJam { tile: w.tile, wagons: vec![id], seconds: w.waiting, why });
        }
        let _ = c;
        jams
    }

    /// Whether the rails are sound: tiles on ground that suits them, bays and routes that refer to what stands.
    pub(crate) fn rails_valid(&self, c: &Catalog) -> bool {
        let r = &self.rails;
        if r.is_empty() && r.bays.is_empty() && r.routes.is_empty() {
            return true;
        }
        if r.cells.len() > MAX_RAILS || r.routes.len() > MAX_ROUTES || r.wagons.len() > MAX_WAGONS || !self.world.is_hillside() {
            return false;
        }
        let covered: BTreeSet<Tile> = self.buildings.values().flat_map(|b| self.covered(b)).collect();
        // A tile holds a wagon, or two going opposite ways (laying a switch where two stand leaves them there
        // until they have gone).
        let mut per: BTreeMap<Tile, Vec<&Wagon>> = BTreeMap::new();
        for w in r.wagons.values() {
            per.entry(w.tile).or_default().push(w);
        }
        if per.values().any(|ws| ws.len() > 2 || (ws.len() == 2 && ws[0].face != ws[1].face.back())) {
            return false;
        }
        r.cells.keys().all(|&t| self.inside(t) && self.world.terrain(t).walkable() && !covered.contains(&t) && !self.roads.contains(&t))
            && r.bays.iter().all(|(name, b)| name.len() <= 32 && r.cells.get(&b.tile).is_some_and(|x| x.bay.as_deref() == Some(name)))
            && r.cells.values().all(|x| x.bay.as_ref().is_none_or(|n| r.bays.contains_key(n)))
            && r.routes.values().all(|rt| rt.stops.len() >= 2 && rt.stops.iter().all(|s| r.bays.contains_key(&s.bay) && s.good.as_ref().is_none_or(|g| c.resources.contains_key(g))))
            && r.wagons.values().all(|w| {
                r.routes.contains_key(&w.route)
                    && r.cells.contains_key(&w.tile)
                    && w.to.is_none_or(|t| r.cells.contains_key(&t))
                    && (w.amount == 0 || c.resources.contains_key(&w.good))
                    && w.amount <= u32::from(self.rail_spec(c).capacity)
                    && usize::from(w.stop) < r.routes[&w.route].stops.len()
            })
    }

    // ---- the one-gesture railway ----------------------------------------------------

    /// The way a railway from one work to another would take: the rail tiles in order, the first beside `from` and the
    /// last beside `to`. The cheapest clear ground (rail already laid counts half what new rail does, so a railway joins
    /// the network it passes); an error says why there is none. The old tramway's planner, for the rail network.
    pub fn plan_rail_path(&self, c: &Catalog, from: u32, to: u32) -> Result<Vec<Tile>, String> {
        let (a, b) = (self.buildings.get(&from).ok_or("no work to start from")?, self.buildings.get(&to).ok_or("no work to end at")?);
        if from == to {
            return Err("a railway joins two different works".into());
        }
        if !self.usable_rail_end(c, from) || !self.usable_rail_end(c, to) {
            return Err("a railway runs from a work to a work, not to a swape".into());
        }
        let open = |t: Tile| self.rails.cells.contains_key(&t) || self.check_rail_site(c, t).is_ok();
        let beside = |id: u32| -> BTreeSet<Tile> {
            let own: BTreeSet<Tile> = self.covered(&self.buildings[&id]).collect();
            own.iter().flat_map(|t| neighbours(*t)).filter(|n| !own.contains(n) && open(*n)).collect()
        };
        let (starts, goals) = (beside(from), beside(to));
        let _ = (a, b);
        if starts.is_empty() || goals.is_empty() {
            return Err("there is no open ground beside one of the works to lay a bay on".into());
        }
        // Dijkstra over open ground: rail already laid costs 2, new rail 3 (so a path is not bent to reuse a few tiles).
        let cost = |t: Tile| if self.rails.cells.contains_key(&t) { 2u32 } else { 3 };
        let mut best: BTreeMap<Tile, u32> = BTreeMap::new();
        let mut parent: BTreeMap<Tile, Option<Tile>> = BTreeMap::new();
        let mut queue = std::collections::BinaryHeap::new();
        for &s in &starts {
            best.insert(s, cost(s));
            parent.insert(s, None);
            queue.push(std::cmp::Reverse((cost(s), s)));
        }
        let limit = self.rail_spec(c).max_length as usize;
        let mut found: Option<Tile> = None;
        while let Some(std::cmp::Reverse((d, t))) = queue.pop() {
            if best.get(&t) != Some(&d) {
                continue;
            }
            // Two bays need two tiles: a goal reached with no step taken does not count.
            if goals.contains(&t) && parent[&t].is_some() {
                found = Some(t);
                break;
            }
            for n in neighbours(t) {
                if !self.inside(n) || !open(n) {
                    continue;
                }
                let nd = d + cost(n);
                if best.get(&n).is_none_or(|&old| nd < old) && nd <= (limit as u32) * 3 {
                    best.insert(n, nd);
                    parent.insert(n, Some(t));
                    queue.push(std::cmp::Reverse((nd, n)));
                }
            }
        }
        let end = found.ok_or("no clear way between those two works")?;
        let mut way = vec![end];
        let mut at = end;
        while let Some(Some(p)) = parent.get(&at) {
            way.push(*p);
            at = *p;
        }
        way.reverse();
        if way.len() > limit {
            return Err(format!("a railway runs at most {limit} tiles"));
        }
        Ok(way)
    }

    /// Lay a railway from one work to another in one gesture: the rail along `path` (the shortest way when it is empty),
    /// a bay at each end named for the work it serves, a route that loads at the first and unloads at the second (the
    /// one `good`, or whatever there is), and `wagons` wagons. All of it or none: a railway the camp cannot pay for, or
    /// that would not run, changes nothing.
    pub(crate) fn lay_railway(&mut self, c: &Catalog, from: u32, to: u32, path: Vec<Tile>, wagons: u16, good: Option<String>) -> Result<String, String> {
        let spec = self.rail_spec(c).clone();
        if !spec.requires.iter().all(|r| self.researched.contains(r)) {
            return Err(format!("research {} first", spec.requires.join(", ")));
        }
        if wagons == 0 || wagons > spec.max_vehicles {
            return Err(format!("a route takes 1 to {} wagons", spec.max_vehicles));
        }
        if good.as_ref().is_some_and(|g| !c.resources.contains_key(g)) {
            return Err("no such good".into());
        }
        let path = if path.is_empty() { self.plan_rail_path(c, from, to)? } else { path };
        if path.len() < 2 || path.len() > usize::from(spec.max_length) {
            return Err(format!("a railway is 2 to {} tiles", spec.max_length));
        }
        if path.windows(2).any(|w| Dir::between(w[0], w[1]).is_none()) {
            return Err("a railway's tiles run one beside the next".into());
        }
        // The ends: each beside the work it serves.
        let facing = |k: &Kingdom, tile: Tile, id: u32| Dir::ALL.into_iter().find(|d| k.at(d.step(tile)) == Some(id));
        let mut trial = self.clone();
        for &t in &path {
            if !trial.rails.cells.contains_key(&t) {
                trial.lay_rail(c, t)?;
            }
        }
        let mut names = Vec::new();
        for (tile, id) in [(path[0], from), (path[path.len() - 1], to)] {
            let side = facing(&trial, tile, id).ok_or("the railway's ends must stand beside the two works")?;
            // A bay already there that serves this work is shared.
            let name = match trial.rails.cells[&tile].bay.clone() {
                Some(n) if trial.rails.bays[&n].side == side => n,
                Some(_) => return Err("a bay of another work stands on that tile".into()),
                None => {
                    let n = trial.bay_name(c, id);
                    trial.make_bay(c, tile, Some(side), Some(n.clone()))?;
                    n
                }
            };
            names.push(name);
        }
        if names[0] == names[1] {
            return Err("a railway needs a bay at each of two works".into());
        }
        let stops = vec![
            Stop { bay: names[0].clone(), act: Act::Load, good: good.clone() },
            Stop { bay: names[1].clone(), act: Act::Unload, good },
        ];
        trial.make_route(c, None, stops, wagons)?;
        // The new rail must join the bays: the route is reaped if its first stop cannot reach the last.
        let (a, b) = (trial.rails.bays[&names[0]].tile, trial.rails.bays[&names[1]].tile);
        if trial.rails.path(a, b, &BTreeSet::new()).is_none() {
            return Err("that rail does not join the two bays".into());
        }
        let laid = trial.rails.cells.len() - self.rails.cells.len();
        *self = trial;
        Ok(format!("The {} to {} railway runs: {laid} tiles laid, {wagons} wagons", names[0], names[1]))
    }

    // ---- the old tramways ----------------------------------------------------------

    /// Whether some tramway line of the old kind is still laid (a save from before the rails).
    pub fn has_tramways(&self) -> bool {
        self.lines.values().any(|l| l.kind == LineKind::Tram)
    }

    /// Bring every point-to-point tramway of an old save onto the network: its track becomes rail, a bay
    /// stands at each end and the wagons run between them as a route, as they did. A tramway with no track
    /// between its buildings (they touch, or there is a single tile) has no room for a bay at each end
    /// and stays as it was. Returns how many were brought over.
    pub fn migrate_tramways(&mut self, c: &Catalog) -> usize {
        let old: Vec<u32> = self.lines.iter().filter(|(_, l)| l.kind == LineKind::Tram && l.path.len() >= 4).map(|(&id, _)| id).collect();
        let mut moved = 0;
        for id in old {
            let line = self.lines[&id].clone();
            let inner: Vec<Tile> = line.path[1..line.path.len() - 1].to_vec();
            if inner.iter().any(|t| self.roads.contains(t) || self.at(*t).is_some() || self.rollers.cells.get(t).is_some_and(|x| x.piece != crate::rollers::Piece::Trestle)) {
                continue;
            }
            let (first, last) = (inner[0], inner[inner.len() - 1]);
            let (Some(a), Some(b)) = (Dir::between(first, line.path[0]), Dir::between(last, line.path[line.path.len() - 1])) else { continue };
            let (Some(from), Some(to)) = (self.at(line.path[0]), self.at(line.path[line.path.len() - 1])) else { continue };
            for t in &inner {
                self.rails.cells.entry(*t).or_default();
            }
            let na = self.rails.cells[&first].bay.clone().unwrap_or_else(|| self.bay_name(c, from));
            if self.rails.cells[&first].bay.is_none() {
                self.rails.bays.insert(na.clone(), Bay { tile: first, side: a });
                self.rails.cells.get_mut(&first).unwrap().bay = Some(na.clone());
            }
            let nb = self.rails.cells[&last].bay.clone().unwrap_or_else(|| self.bay_name(c, to));
            if self.rails.cells[&last].bay.is_none() {
                self.rails.bays.insert(nb.clone(), Bay { tile: last, side: b });
                self.rails.cells.get_mut(&last).unwrap().bay = Some(nb.clone());
            }
            let route = self.rails.next_route.max(1);
            self.rails.next_route = route + 1;
            self.rails.routes.insert(
                route,
                Route {
                    name: format!("{na} to {nb}"),
                    stops: vec![Stop { bay: na, act: Act::Load, good: None }, Stop { bay: nb, act: Act::Unload, good: None }],
                    fed: self.rail_spec(c).upkeep_seconds,
                    ..Route::default()
                },
            );
            // The wagons stand along the track from the first bay; what they carried goes to the camp.
            let mut at = VecDeque::from([first]);
            let mut seen = BTreeSet::from([first]);
            let mut places = Vec::new();
            while let Some(t) = at.pop_front() {
                if !self.rails.wagons.values().any(|w| w.tile == t) {
                    places.push(t);
                }
                for n in self.rails.links(t) {
                    if seen.insert(n) {
                        at.push_back(n);
                    }
                }
            }
            for (i, _v) in line.fleet.iter().enumerate() {
                let Some(&tile) = places.get(i) else { break };
                let wid = self.rails.next_wagon.max(1);
                self.rails.next_wagon = wid + 1;
                self.rails.wagons.insert(wid, Wagon { route, tile, ..Wagon::default() });
            }
            for v in &line.fleet {
                add(&mut self.stock, &v.resource, v.amount);
            }
            self.lines.remove(&id);
            moved += 1;
        }
        moved
    }
}
