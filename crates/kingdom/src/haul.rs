//! Hauling on infrastructure lines. Carts do not live here: they are a shared
//! pool on the road network (`carting.rs`). What is laid point to point is the
//! upgrade a road's flow outgrows into: a tramway, a ropeway, a chute, a barge.
//! A line is a real thing with a length, a fleet and a berth at each end. On
//! the hillside nothing hops by magic:
//!
//! - a vehicle carries `capacity` units one way, takes `speed` tenths of a
//!   second a tile, and holds a berth for `dwell` seconds to load and again to
//!   unload, so a dock serves only so many carts a minute;
//! - a line moves goods only from one building's store to another's, so a
//!   full buffer at either end stops it, and the player can see which;
//! - a junction is a depot: a store with berths that lines run into and out
//!   of, and it is as good as its buffer and its berths.
//!
//! Throughput is `fleet x capacity / cycle`, and every part of that is data in
//! the catalogue (`lines`), so finding the bottleneck means reading numbers
//! and fixing it means more vehicles, a faster kind of line, or another line.
//! Legacy valleys keep their abstract carriers; this runs on the hillside.
use crate::economy::{add, covers, neighbours, take};
use crate::*;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

/// Seconds a vehicle waits at its dock for a full load before it leaves with
/// what it has.
pub const PATIENCE: u32 = 15;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// At the origin dock, waiting for goods and a berth.
    #[default]
    Idle,
    Loading,
    Out,
    /// At the destination, waiting for a berth or for room.
    Waiting,
    Unloading,
    Back,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vehicle {
    pub phase: Phase,
    pub resource: String,
    pub amount: u32,
    /// Seconds left at a berth, seconds idle at the dock, or tenths of a
    /// second travelled, according to the phase.
    pub clock: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineStats {
    /// Units delivered since the line was laid.
    pub moved: u64,
    pub trips: u32,
    /// Vehicle-seconds (times eight, fading by an eighth each shift) spent
    /// idle at the origin with nothing to carry.
    pub starved: u32,
    /// The same for vehicles held up by a full destination, a busy berth or a
    /// dock with nothing the destination wants.
    pub queued: u32,
    /// Vehicle-seconds (times eight, fading alike) spent loading, travelling
    /// or unloading: the line at work.
    pub busy: u32,
    /// Units delivered in each of the last six ten-second shifts, oldest first.
    pub recent: [u16; 6],
    pub shift: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub kind: LineKind,
    pub from: u32,
    pub to: u32,
    /// The tiles it runs over, the first at the origin building and the last
    /// at the destination.
    pub path: Vec<Tile>,
    /// Tenths of a second to cross loaded, and to come back empty (zero when
    /// the vehicles return at once).
    pub forward: u32,
    pub back: u32,
    pub fleet: Vec<Vehicle>,
    pub stats: LineStats,
    /// Seconds the oxen's fodder lasts.
    #[serde(default)]
    pub fed: u16,
}

/// What the player reads off a line to find a bottleneck.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineRow {
    pub id: u32,
    pub kind: LineKind,
    pub from: u32,
    pub to: u32,
    pub length: usize,
    pub vehicles: usize,
    /// Units a minute the fleet could carry with goods always waiting and a
    /// berth always free.
    pub capacity_per_min: u32,
    /// Units a minute actually delivered over the last minute.
    pub observed_per_min: u32,
    pub cut: bool,
    pub state: LineState,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineState {
    /// The road is cut.
    Cut,
    /// The oxen have no fodder.
    Unfed,
    /// Vehicles wait at the origin: nothing to carry, so the cause is upstream.
    Starved,
    /// Vehicles wait at the destination: it is full or its berths are busy, so
    /// the cause is downstream.
    Backed,
    /// Every vehicle is out working: the line itself is the limit.
    Saturated,
    Flowing,
}
impl LineState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cut => "CUT",
            Self::Unfed => "NO FODDER",
            Self::Starved => "WAITING FOR GOODS",
            Self::Backed => "BACKED UP",
            Self::Saturated => "AT CAPACITY",
            Self::Flowing => "FLOWING",
        }
    }
}

impl Line {
    pub fn length(&self) -> usize {
        self.path.len().saturating_sub(1)
    }
    /// Seconds for one vehicle to go out, unload, come back and load.
    pub fn cycle(&self, spec: &LineSpec) -> u32 {
        (self.forward + self.back).div_ceil(10) + 2 * u32::from(spec.dwell)
    }
    pub fn capacity_per_min(&self, spec: &LineSpec) -> u32 {
        let cycle = self.cycle(spec).max(1);
        self.fleet.len() as u32 * u32::from(spec.capacity) * 60 / cycle
    }
    pub fn observed_per_min(&self) -> u32 {
        self.stats.recent.iter().map(|&n| u32::from(n)).sum()
    }
}

/// Shared scratch for one second of hauling, so no vehicle sees stale room.
struct Yard {
    reserved: BTreeMap<(u32, String), u32>,
    busy: BTreeMap<u32, u8>,
    targets: BTreeMap<u32, Option<Stock>>,
    capacity: u32,
}

impl Kingdom {
    pub fn line_spec<'a>(&self, c: &'a Catalog, line: &Line) -> &'a LineSpec {
        &c.lines[line.kind.key()]
    }

    // Stores: the camp keeps its goods in the kingdom's stock; depots keep
    // them in their output store; workshops and yards take deliveries into
    // their input buffer and give out of their output buffer.

    /// Goods a building will accept, as target levels, or `None` for the camp
    /// and depots that accept anything up to their cap.
    pub(crate) fn target_levels(&self, c: &Catalog, id: u32) -> Option<Stock> {
        let b = self.buildings.get(&id)?;
        let p = &c.plans[&b.plan];
        if id == 1 || p.hub {
            return None;
        }
        if b.construction > 0 || b.paused {
            return Some(Stock::new());
        }
        if p.stalls > 0 {
            // A carter's yard asks only for its oxen's fodder, which the roads bring.
            return Some(c.lines["cart"].upkeep.iter().map(|(g, n)| (g.clone(), (n * b.carts.len() as u32 * 2).max(2))).collect());
        }
        if p.watch > 0 {
            // A watchtower asks the roads for a full quiver.
            return Some(Stock::from([(crate::defence::ARROW.to_string(), u32::from(p.quiver).min(p.input_cap()))]));
        }
        if p.yard {
            return Some(
                self.site_of_yard(id)
                    .map(|(_, s)| self.site_wants(c, s))
                    .unwrap_or_default(),
            );
        }
        let cap = p.input_cap();
        Some(
            self.delivery_request(c, b)
                .map(|(input, _)| {
                    input
                        .into_iter()
                        .map(|(r, n)| (r, (n * 8).max(8).min(cap)))
                        .collect()
                })
                .unwrap_or_default(),
        )
    }

    pub(crate) fn have_at(&self, c: &Catalog, id: u32, r: &str) -> u32 {
        if id == 1 {
            self.amount(r)
        } else if let Some(b) = self.buildings.get(&id) {
            let store = if c.plans[&b.plan].hub { &b.output } else { &b.input };
            store.get(r).copied().unwrap_or(0)
        } else {
            0
        }
    }
    pub(crate) fn cap_at(&self, c: &Catalog, id: u32, ctx_capacity: u32) -> u32 {
        if id == 1 {
            ctx_capacity
        } else {
            self.buildings
                .get(&id)
                .map_or(0, |b| c.plans[&b.plan].input_cap())
        }
    }

    /// How many more units of `r` a destination can take, counting what is
    /// already on its way.
    fn room(&self, c: &Catalog, yard: &Yard, id: u32, r: &str) -> u32 {
        let reserved = yard.reserved.get(&(id, r.to_string())).copied().unwrap_or(0);
        let have = self.have_at(c, id, r);
        let cap = self.cap_at(c, id, yard.capacity);
        let limit = match yard.targets.get(&id) {
            Some(Some(levels)) => levels.get(r).copied().unwrap_or(0).min(cap),
            Some(None) => cap,
            None => 0,
        };
        limit.saturating_sub(have + reserved)
    }

    pub(crate) fn avail(&self, c: &Catalog, id: u32, r: &str, to: u32) -> u32 {
        if id == 1 {
            let reserve = if r == "food" && to != 1 { self.food_delivery_target(c) } else { 0 };
            self.amount(r).saturating_sub(reserve)
        } else if let Some(b) = self.buildings.get(&id) {
            if c.plans[&b.plan].yard || b.construction > 0 {
                0
            } else {
                b.output.get(r).copied().unwrap_or(0)
            }
        } else {
            0
        }
    }
    pub(crate) fn store_resources(&self, c: &Catalog, id: u32) -> Vec<String> {
        if id == 1 {
            self.stock.keys().cloned().collect()
        } else {
            self.buildings
                .get(&id)
                .filter(|b| !c.plans[&b.plan].yard)
                .map(|b| b.output.keys().cloned().collect())
                .unwrap_or_default()
        }
    }
    pub(crate) fn berths_of(&self, c: &Catalog, id: u32) -> u8 {
        self.buildings
            .get(&id)
            .map_or(1, |b| c.plans[&b.plan].berth_count())
    }

    pub(crate) fn give(&mut self, c: &Catalog, id: u32, r: &str, n: u32) {
        if id == 1 {
            add(&mut self.stock, r, n);
            crate::economy::capped_add(&mut self.delivered, r, n);
        } else if let Some(b) = self.buildings.get_mut(&id) {
            if c.plans[&b.plan].hub {
                add(&mut b.output, r, n);
            } else {
                add(&mut b.input, r, n);
            }
        }
    }
    pub(crate) fn draw(&mut self, id: u32, r: &str, n: u32) {
        let store = if id == 1 {
            &mut self.stock
        } else {
            &mut self.buildings.get_mut(&id).expect("origin exists").output
        };
        take(store, &Stock::from([(r.into(), n)]));
    }

    /// A line is intact while whatever it rides is there: a cart line needs
    /// every road tile it uses.
    pub fn line_intact(&self, c: &Catalog, line: &Line) -> bool {
        // Oxen that have not been fed do not pull.
        if !c.lines[line.kind.key()].upkeep.is_empty() && line.fed == 0 {
            return false;
        }
        match line.kind {
            LineKind::Cart => line
                .path
                .iter()
                .skip(1)
                .take(line.path.len().saturating_sub(2))
                .all(|t| self.roads.contains(t)),
            _ => self.buildings.contains_key(&line.from) && self.buildings.contains_key(&line.to),
        }
    }

    pub(crate) fn haul(&mut self, c: &Catalog) {
        if self.lines.is_empty() && !self.buildings.values().any(|b| !b.carts.is_empty()) {
            return;
        }
        let report = self.report(c);
        let capacity = report.capacity;
        self.haul_roads(c, capacity, &report.status);
        if self.lines.is_empty() {
            return;
        }
        let mut yard = Yard {
            reserved: BTreeMap::new(),
            busy: BTreeMap::new(),
            targets: BTreeMap::new(),
            capacity,
        };
        for line in self.lines.values() {
            for v in &line.fleet {
                if v.amount > 0 {
                    *yard.reserved.entry((line.to, v.resource.clone())).or_default() += v.amount;
                }
                match v.phase {
                    Phase::Loading => *yard.busy.entry(line.from).or_default() += 1,
                    Phase::Unloading => *yard.busy.entry(line.to).or_default() += 1,
                    _ => {}
                }
            }
            if !yard.targets.contains_key(&line.to) {
                yard.targets.insert(line.to, self.target_levels(c, line.to));
            }
        }
        // Whoever goes first this second changes with the second, so no line
        // owns the cargo at a shared depot.
        let mut ids: Vec<u32> = self.lines.keys().copied().collect();
        let turn = (self.second % ids.len() as u64) as usize;
        ids.rotate_left(turn);
        for id in ids {
            let Some(mut line) = self.lines.remove(&id) else { continue };
            let intact = self.line_intact(c, &line);
            let spec = c.lines[line.kind.key()].clone();
            for i in 0..line.fleet.len() {
                self.step(c, &mut yard, &mut line, i, &spec, intact);
            }
            if self.second % 10 == 0 {
                line.stats.recent.rotate_left(1);
                line.stats.recent[5] = std::mem::take(&mut line.stats.shift);
                // Waiting is remembered for about a minute and a half, so the
                // report describes the line as it is, not as it was built.
                line.stats.starved -= line.stats.starved.div_ceil(8);
                line.stats.queued -= line.stats.queued.div_ceil(8);
                line.stats.busy -= line.stats.busy.div_ceil(8);
            }
            self.lines.insert(id, line);
        }
    }

    fn step(&mut self, c: &Catalog, yard: &mut Yard, line: &mut Line, i: usize, spec: &LineSpec, intact: bool) {
        let (from, to) = (line.from, line.to);
        let dwell = u32::from(spec.dwell);
        let mut v = std::mem::take(&mut line.fleet[i]);
        match v.phase {
            Phase::Idle => {
                v.clock += 1;
                let free = yard.busy.get(&from).copied().unwrap_or(0) < self.berths_of(c, from);
                let mut best: Option<(u32, String)> = None;
                // Goods waiting that the destination cannot take is a blocked
                // line, not a starved one: the cure lies downstream.
                let mut blocked = false;
                if free && intact {
                    for r in self.store_resources(c, from) {
                        let have = self.avail(c, from, &r, to);
                        let take_n = have.min(self.room(c, yard, to, &r)).min(u32::from(spec.capacity));
                        blocked |= have > 0 && take_n == 0;
                        if take_n > best.as_ref().map_or(0, |b| b.0) {
                            best = Some((take_n, r));
                        }
                    }
                }
                match best {
                    Some((n, r)) if n == u32::from(spec.capacity) || v.clock >= PATIENCE => {
                        self.draw(from, &r, n);
                        *yard.reserved.entry((to, r.clone())).or_default() += n;
                        v.resource = r;
                        v.amount = n;
                        if dwell > 0 {
                            v.phase = Phase::Loading;
                            v.clock = dwell;
                            *yard.busy.entry(from).or_default() += 1;
                        } else {
                            v.phase = Phase::Out;
                            v.clock = 0;
                        }
                    }
                    Some(_) => {}
                    None if free && !blocked => line.stats.starved += 8,
                    None => line.stats.queued += 8,
                }
            }
            Phase::Loading => {
                v.clock = v.clock.saturating_sub(1);
                if v.clock == 0 {
                    v.phase = Phase::Out;
                    if let Some(n) = yard.busy.get_mut(&from) {
                        *n = n.saturating_sub(1);
                    }
                }
            }
            Phase::Out => {
                if intact {
                    v.clock += u32::from(crate::pace::travel_clock());
                    if v.clock >= line.forward {
                        v.phase = Phase::Waiting;
                        v.clock = 0;
                    }
                }
            }
            Phase::Waiting => {
                let free = dwell == 0 || yard.busy.get(&to).copied().unwrap_or(0) < self.berths_of(c, to);
                let space = self
                    .cap_at(c, to, yard.capacity)
                    .saturating_sub(self.have_at(c, to, &v.resource))
                    >= v.amount;
                if free && space && self.buildings.contains_key(&to) {
                    if dwell > 0 {
                        v.phase = Phase::Unloading;
                        v.clock = dwell;
                        *yard.busy.entry(to).or_default() += 1;
                    } else {
                        self.deliver(c, yard, line, &mut v);
                    }
                } else {
                    line.stats.queued += 8;
                }
            }
            Phase::Unloading => {
                v.clock = v.clock.saturating_sub(1);
                if v.clock == 0 {
                    let space = self
                        .cap_at(c, to, yard.capacity)
                        .saturating_sub(self.have_at(c, to, &v.resource))
                        >= v.amount;
                    if space {
                        if let Some(n) = yard.busy.get_mut(&to) {
                            *n = n.saturating_sub(1);
                        }
                        self.deliver(c, yard, line, &mut v);
                    } else {
                        line.stats.queued += 8;
                    }
                }
            }
            Phase::Back => {
                if intact {
                    v.clock += u32::from(crate::pace::travel_clock());
                    if v.clock >= line.back {
                        v.phase = Phase::Idle;
                        v.clock = 0;
                    }
                }
            }
        }
        if matches!(v.phase, Phase::Loading | Phase::Out | Phase::Unloading | Phase::Back) {
            line.stats.busy += 8;
        }
        line.fleet[i] = v;
    }

    fn deliver(&mut self, c: &Catalog, yard: &mut Yard, line: &mut Line, v: &mut Vehicle) {
        self.give(c, line.to, &v.resource, v.amount);
        if let Some(n) = yard.reserved.get_mut(&(line.to, v.resource.clone())) {
            *n = n.saturating_sub(v.amount);
        }
        line.stats.moved += u64::from(v.amount);
        line.stats.trips += 1;
        line.stats.shift = line.stats.shift.saturating_add(v.amount as u16);
        v.resource.clear();
        v.amount = 0;
        v.clock = 0;
        v.phase = if line.back == 0 { Phase::Idle } else { Phase::Back };
    }

    /// Goods riding a line's vehicles or a cart, for the supply ledger.
    pub(crate) fn hauled(&self, resource: &str) -> u32 {
        self.lines
            .values()
            .flat_map(|l| &l.fleet)
            .filter(|v| v.resource == resource)
            .map(|v| v.amount)
            .sum::<u32>()
            + self.carted(resource)
            + self.rollers.holding(resource)
            + self.railed(resource)
    }

    // ---- planning and laying -------------------------------------------

    /// Tiles a line may not cross: deep water, anything built that cannot be
    /// walked through, a castle's ground. Bridges count as land.
    fn line_open(&self, c: &Catalog) -> Vec<bool> {
        let (w, _) = self.world.bounds();
        let w = w as usize;
        let mut open = self.world.walkable_grid();
        for &t in &self.bridges {
            open[t.1 as usize * w + t.0 as usize] = true;
        }
        for b in self.buildings.values() {
            if !c.plans[&b.plan].passable {
                for t in self.covered(b) {
                    open[t.1 as usize * w + t.0 as usize] = false;
                }
            }
        }
        for s in self.sites.values() {
            let (x0, y0, x1, y1) = s.rect();
            for y in y0.max(0)..=y1.min(self.world.bounds().1 - 1) {
                for x in x0.max(0)..=x1.min(self.world.bounds().0 - 1) {
                    open[y as usize * w + x as usize] = false;
                }
            }
        }
        open
    }

    /// Tiles carrying some line's track, which nothing else may be built on.
    pub fn track_tiles(&self) -> BTreeSet<Tile> {
        self.lines
            .values()
            .filter(|l| matches!(l.kind, LineKind::Tram | LineKind::Chute))
            .flat_map(|l| l.path.iter().skip(1).take(l.path.len().saturating_sub(2)).copied())
            .collect()
    }

    /// The shortest way a line of this kind can go between two buildings, or
    /// why there is none. Carts prefer roads, trams and chutes ride new track
    /// or shared track, a chute only goes downhill, a barge stays on the river
    /// between two quays, and a rope goes straight over anything.
    pub fn plan_path(&self, c: &Catalog, kind: LineKind, from: u32, to: u32) -> Result<Vec<Tile>, String> {
        let (building_a, building_b) = (self.buildings.get(&from).ok_or("no building to start from")?, self.buildings.get(&to).ok_or("no building to end at")?);
        let (a, b) = (building_a.tile, building_b.tile);
        let (docks_a, docks_b) = (self.docks_of(building_a), self.docks_of(building_b));
        if from == to {
            return Err("a line needs two different buildings".into());
        }
        let spec = c.lines.get(kind.key()).ok_or("this catalogue has no such line")?;
        let (width, height) = self.world.bounds();
        let w = width as usize;
        let at = |t: Tile| t.1 as usize * w + t.0 as usize;
        let inside = |t: Tile| t.0 >= 0 && t.1 >= 0 && t.0 < width && t.1 < height;
        let path = if kind == LineKind::Rope {
            // The straight staircase: x first then y, which the renderer can
            // draw as one cable with a pylon every few tiles.
            let mut path = vec![a];
            let mut cur = a;
            while cur != b {
                if cur.0 != b.0 {
                    cur.0 += (b.0 - cur.0).signum();
                } else {
                    cur.1 += (b.1 - cur.1).signum();
                }
                path.push(cur);
            }
            path
        } else {
            let open = self.line_open(c);
            let track = self.track_tiles();
            let barge = kind == LineKind::Barge;
            let allowed = |t: Tile| {
                if !inside(t) {
                    return false;
                }
                if barge {
                    self.world.terrain(t) == Terrain::River
                } else {
                    open[at(t)]
                }
            };
            let cost = |t: Tile| -> u32 {
                match kind {
                    LineKind::Cart => if self.roads.contains(&t) { 1 } else { 3 },
                    LineKind::Tram => if track.contains(&t) { 1 } else { 2 },
                    _ => 1,
                }
            };
            let height_of = |t: Tile| self.world.height(t);
            // A cart line between buildings the camp already reaches stays on the
            // camp's roads at both docks, so it can never strand a crew on an
            // island of road; one end that is not yet connected joins the other.
            let net = if kind == LineKind::Cart { self.camp_road_net() } else { BTreeSet::new() };
            let on_net = |t: &Tile| net.contains(t);
            let a_net = docks_a.iter().any(on_net);
            let b_net = docks_b.iter().any(on_net);
            // Dijkstra from every tile touching the origin to any tile touching
            // the destination.
            let mut best = vec![u32::MAX; open.len()];
            let mut parent: Vec<Option<Tile>> = vec![None; open.len()];
            let mut queue = BinaryHeap::new();
            for &t in &docks_a {
                if allowed(t) && (kind != LineKind::Chute || height_of(t) <= height_of(a)) && (!a_net || on_net(&t)) {
                    best[at(t)] = cost(t);
                    queue.push(Reverse((cost(t), t)));
                }
            }
            let docks: BTreeSet<Tile> = docks_b.iter().copied().filter(|t| !b_net || on_net(t)).collect();
            let mut found = None;
            while let Some(Reverse((d, t))) = queue.pop() {
                if d != best[at(t)] {
                    continue;
                }
                if docks.contains(&t) && (kind != LineKind::Chute || height_of(b) <= height_of(t)) {
                    found = Some(t);
                    break;
                }
                for n in neighbours(t) {
                    if !allowed(n) || (kind == LineKind::Chute && height_of(n) > height_of(t)) {
                        continue;
                    }
                    let nd = d + cost(n);
                    if nd < best[at(n)] {
                        best[at(n)] = nd;
                        parent[at(n)] = Some(t);
                        queue.push(Reverse((nd, n)));
                    }
                }
            }
            let end = found.ok_or_else(|| {
                match kind {
                    LineKind::Chute => "no downhill way: a chute only runs from high ground to low",
                    LineKind::Barge => "no river between those two quays",
                    _ => "no way through between those buildings",
                }
                .to_string()
            })?;
            let mut path = vec![end];
            let mut cursor = end;
            while let Some(p) = parent[at(cursor)] {
                path.push(p);
                cursor = p;
            }
            // The building's own tile each end of the way touches.
            let touching = |building: &Building, dock: Tile| self.covered(building).find(|t| neighbours(*t).contains(&dock)).unwrap_or(building.tile);
            let first = *path.last().expect("a way");
            path.push(touching(building_a, first));
            path.reverse();
            path.push(touching(building_b, end));
            path
        };
        if path.len() as u32 > u32::from(spec.max_length) + 1 {
            return Err(format!(
                "a {} runs at most {} tiles; this one needs {}",
                spec.name,
                spec.max_length,
                path.len() - 1
            ));
        }
        Ok(path)
    }

    /// The road tiles the camp's walkers can reach.
    pub fn camp_road_net(&self) -> BTreeSet<Tile> {
        let mut seen = BTreeSet::new();
        let mut queue: std::collections::VecDeque<Tile> =
            neighbours(self.camp()).into_iter().filter(|t| self.roads.contains(t)).collect();
        while let Some(t) = queue.pop_front() {
            if seen.insert(t) {
                queue.extend(neighbours(t).into_iter().filter(|n| self.roads.contains(n) && !seen.contains(n)));
            }
        }
        seen
    }

    /// Tenths of a second to cross `path`, either way.
    fn crossing(&self, spec: &LineSpec, path: &[Tile], reverse: bool) -> u32 {
        let against = if spec.against == 0 { spec.speed } else { spec.against };
        let mut total = 0u32;
        for pair in path.windows(2) {
            let (p, q) = if reverse { (pair[1], pair[0]) } else { (pair[0], pair[1]) };
            // The river runs south, so +y is with the current; a lateral step
            // costs the mean of the two.
            let step = match q.1.cmp(&p.1) {
                std::cmp::Ordering::Greater => spec.speed,
                std::cmp::Ordering::Less => against,
                std::cmp::Ordering::Equal => (spec.speed + against).div_ceil(2),
            };
            total += u32::from(step);
        }
        total
    }

    fn check_line_path(&self, c: &Catalog, kind: LineKind, from: u32, to: u32, path: &[Tile]) -> Result<(), String> {
        // A line runs from a tile of one building to a tile of the other (a building
        // has its footprint: the line's first and last tiles are what it covers).
        let a: Vec<Tile> = self.covered(&self.buildings[&from]).collect();
        let b: Vec<Tile> = self.covered(&self.buildings[&to]).collect();
        if path.len() < 2 || !a.contains(&path[0]) || !b.contains(&path[path.len() - 1]) {
            return Err("a line runs from one building to the other".into());
        }
        let spec = &c.lines[kind.key()];
        if path.len() as u32 > u32::from(spec.max_length) + 1 {
            return Err(format!("a {} runs at most {} tiles", spec.name, spec.max_length));
        }
        let mut seen = BTreeSet::new();
        for pair in path.windows(2) {
            if pair[0].0.abs_diff(pair[1].0) + pair[0].1.abs_diff(pair[1].1) != 1 {
                return Err("a line moves one tile at a time".into());
            }
        }
        let open = self.line_open(c);
        let w = self.world.bounds().0 as usize;
        for &t in &path[1..path.len() - 1] {
            if !self.world.contains(t) || !seen.insert(t) {
                return Err("a line cannot cross itself".into());
            }
            let ok = match kind {
                LineKind::Rope => true,
                LineKind::Barge => self.world.terrain(t) == Terrain::River,
                _ => open[t.1 as usize * w + t.0 as usize],
            };
            if !ok {
                return Err(format!("the {} cannot go over ({}, {})", spec.name, t.0, t.1));
            }
        }
        match kind {
            LineKind::Barge => {
                if !(c.plans[&self.buildings[&from].plan].quay && c.plans[&self.buildings[&to].plan].quay) {
                    return Err("barges run between two quays".into());
                }
            }
            LineKind::Chute => {
                let h: Vec<u8> = path.iter().map(|&t| self.world.height(t)).collect();
                if h.windows(2).any(|p| p[1] > p[0]) || h[0] < h[h.len() - 1] + 2 {
                    return Err("a chute only runs downhill, and must fall at least two steps".into());
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn lay_line(
        &mut self,
        c: &Catalog,
        kind: LineKind,
        from: u32,
        to: u32,
        path: Vec<Tile>,
        vehicles: u16,
    ) -> Result<String, String> {
        let spec = c.lines.get(kind.key()).ok_or("this catalogue has no such line")?;
        if !self.world.is_hillside() {
            return Err("lines run on the hillside; the valley keeps its carriers".into());
        }
        if kind == LineKind::Cart {
            return Err("carts ride the roads: raise a carter's yard and add carts to it".into());
        }
        if kind == LineKind::Tram {
            return Err("rails are laid tile by tile now: lay the rail, name a bay at each building and run a route (or ask for a Railway, which does all three at once)".into());
        }
        if !spec.requires.iter().all(|r| self.researched.contains(r)) {
            return Err(format!("research {} first", spec.requires.join(", ")));
        }
        if !self.buildings.contains_key(&from) || !self.buildings.contains_key(&to) || from == to {
            return Err("a line needs two different buildings".into());
        }
        if self.lines.len() >= MAX_LINES {
            return Err("too many lines".into());
        }
        if vehicles == 0 || vehicles > spec.max_vehicles {
            return Err(format!("a {} takes 1 to {} {}s", spec.name, spec.max_vehicles, spec.vehicle));
        }
        let path = if path.is_empty() { self.plan_path(c, kind, from, to)? } else { path };
        self.check_line_path(c, kind, from, to, &path)?;
        let mut cost = Stock::new();
        let inner = &path[1..path.len() - 1];
        let new_tiles = match kind {
            LineKind::Tram => {
                let track = self.track_tiles();
                inner.iter().filter(|t| !track.contains(t)).count()
            }
            LineKind::Cart => 0,
            _ => inner.len(),
        };
        for (r, n) in &spec.tile_cost {
            add(&mut cost, r, n * new_tiles as u32);
        }
        for (r, n) in &spec.vehicle_cost {
            add(&mut cost, r, n * u32::from(vehicles));
        }
        // A cart line rides roads, laying the missing ones at the road's price.
        let mut roads = Vec::new();
        if kind == LineKind::Cart {
            for &t in inner {
                if !self.roads.contains(&t) {
                    self.check_road_site(t)?;
                    roads.push(t);
                }
            }
            add(&mut cost, "timber", roads.len() as u32);
        }
        if !covers(&self.stock, &cost) {
            return Err(format!("a {} line needs {}", spec.name, label(&cost)));
        }
        take(&mut self.stock, &cost);
        for t in roads {
            self.world.clear_forest(t);
            self.roads.insert(t);
        }
        let forward = self.crossing(spec, &path, false);
        let back = if spec.instant_return { 0 } else { self.crossing(spec, &path, true) };
        let id = self.next_line.max(1);
        self.next_line = id + 1;
        self.lines.insert(
            id,
            Line {
                kind,
                from,
                to,
                path,
                forward,
                back,
                fleet: vec![Vehicle::default(); usize::from(vehicles)],
                stats: LineStats::default(),
                fed: 0,
            },
        );
        Ok(format!("{} line laid", spec.name))
    }

    pub(crate) fn set_fleet(&mut self, c: &Catalog, id: u32, vehicles: u16) -> Result<String, String> {
        let line = self.lines.get(&id).ok_or("no such line")?;
        let spec = &c.lines[line.kind.key()];
        if vehicles == 0 || vehicles > spec.max_vehicles {
            return Err(format!("a {} takes 1 to {} {}s", spec.name, spec.max_vehicles, spec.vehicle));
        }
        let have = line.fleet.len() as u16;
        if vehicles > have {
            let mut cost = Stock::new();
            for (r, n) in &spec.vehicle_cost {
                add(&mut cost, r, n * u32::from(vehicles - have));
            }
            if !covers(&self.stock, &cost) {
                return Err(format!("{} more {}s need {}", vehicles - have, spec.vehicle, label(&cost)));
            }
            take(&mut self.stock, &cost);
            let line = self.lines.get_mut(&id).unwrap();
            line.fleet.resize(usize::from(vehicles), Vehicle::default());
        } else if vehicles < have {
            // Only empty vehicles waiting at the dock can be sold.
            let line = self.lines.get_mut(&id).unwrap();
            let mut sell = usize::from(have - vehicles);
            let mut kept = Vec::new();
            let mut sold = 0u32;
            for v in std::mem::take(&mut line.fleet) {
                if sell > 0 && v.phase == Phase::Idle && v.amount == 0 {
                    sell -= 1;
                    sold += 1;
                } else {
                    kept.push(v);
                }
            }
            line.fleet = kept;
            if sold == 0 {
                return Err("every vehicle is on the road; sell when some wait at the dock".into());
            }
            for (r, n) in &spec.vehicle_cost {
                add(&mut self.stock, r, n * sold / 2);
            }
        }
        Ok(format!("{} {}s on the line", self.lines[&id].fleet.len(), spec.vehicle))
    }

    /// Takes a line down: its cargo goes to the camp, never into the void.
    pub(crate) fn remove_line(&mut self, id: u32) -> bool {
        let Some(line) = self.lines.remove(&id) else { return false };
        for v in line.fleet {
            add(&mut self.stock, &v.resource, v.amount);
        }
        true
    }
    pub(crate) fn remove_lines_of(&mut self, building: u32) {
        let ids: Vec<u32> = self
            .lines
            .iter()
            .filter(|(_, l)| l.from == building || l.to == building)
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.remove_line(id);
        }
    }

    /// Every line, read as a bottleneck hunter would: capacity, what it really
    /// moved and what its vehicles spent their time doing.
    pub fn line_report(&self, c: &Catalog) -> Vec<LineRow> {
        self.lines
            .iter()
            .map(|(&id, l)| {
                let spec = &c.lines[l.kind.key()];
                let cut = !self.line_intact(c, l) && !(!spec.upkeep.is_empty() && l.fed == 0);
                // Judged on where the vehicles' time has gone lately: waiting at
                // the origin for goods, waiting at the destination for room, or
                // working. A line is at capacity when it has no slack at all.
                let (s, q, b) = (l.stats.starved, l.stats.queued, l.stats.busy);
                let unfed = !spec.upkeep.is_empty() && l.fed == 0;
                let state = if unfed {
                    LineState::Unfed
                } else if cut {
                    LineState::Cut
                } else if q > b / 4 && q >= s {
                    LineState::Backed
                } else if s > b / 4 {
                    LineState::Starved
                } else if s + q < b / 8 {
                    LineState::Saturated
                } else {
                    LineState::Flowing
                };
                LineRow {
                    id,
                    kind: l.kind,
                    from: l.from,
                    to: l.to,
                    length: l.length(),
                    vehicles: l.fleet.len(),
                    capacity_per_min: l.capacity_per_min(spec),
                    observed_per_min: l.observed_per_min(),
                    cut,
                    state,
                }
            })
            .collect()
    }
}
