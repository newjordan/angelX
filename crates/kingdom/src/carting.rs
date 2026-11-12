//! The roads: carts are a shared pool, not a vehicle for every pair of works.
//!
//! A carter's yard keeps carts and oxen and serves every road it touches. A
//! cart owns no route. The planner looks at what each building lacks (a
//! workshop's inputs, the castle's stockyard, the camp's shelves) and what
//! each store holds (a maker's output, a warehouse, the camp), and writes
//! orders: a load, a place to fetch it and a place to leave it. The nearest
//! free cart takes the order, and because it is free where it last unloaded,
//! the cart that has just delivered is the one that picks up the next load
//! near it: the way back is carrying goods, not running empty.
//!
//! - A cart carries a tonne (`lines.cart.capacity`, in kilograms). Every good
//!   has a weight, so a cart takes ten blocks, twenty sacks, hundreds of
//!   nails, or a mixed load of whatever one place wants from another.
//! - Warehouses and the camp are real stores. A maker whose bench is filling
//!   pushes its goods to the nearest store with room; a building that is
//!   running short pulls from the nearest place that holds the good.
//! - Roads are tiles. A tile takes `road_lanes` carts at once; past that each
//!   extra cart slows the one entering it. Where carts were held up is
//!   remembered, so a new trip plans its route round a jammed street and a
//!   second road helps.
//! - Tramways, ropeways, chutes and barges stay point to point (`haul.rs`):
//!   they are the infrastructure you lay when a road's flow outgrows it, and a
//!   building they feed asks the roads for nothing.
use crate::economy::{add, neighbours, take};
use crate::*;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

/// Seconds between one planning pass and the next.
pub const PLAN_EVERY: u64 = 2;
/// Seconds an idle cart stands where it unloaded before heading home.
pub const IDLE_HOME: u32 = 30;
/// Seconds a cart waits with goods a full store will not take before it
/// carries them to the camp.
pub const PATIENCE: u32 = 20;
const NONE: u16 = u16::MAX;
/// Carts that can cross the camp's open yard abreast.
pub const CAMP_LANES: u16 = 6;
/// Tiles the camp is reckoned further than it stands when a work chooses where to fetch from.
pub const CAMP_DETOUR: u16 = 8;
/// The farthest, in tiles, a maker pushes its goods to a store.
pub const PUSH_RANGE: u16 = 14;
/// A want waiting for a cart grows more urgent: one point every this many seconds.
pub const AGE_SECONDS: u64 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartPhase {
    /// Standing, free to take an order.
    #[default]
    Idle,
    /// Free and heading back to its yard; it can still be given an order.
    Return,
    /// Going to fetch an order.
    Fetch,
    /// At the pickup, waiting for a berth.
    Queue,
    Loading,
    /// Loaded, going to the drop.
    Haul,
    /// At the drop, waiting for a berth or for room.
    Wait,
    Unloading,
}

/// One cart and its ox.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cart {
    /// A road tile, or the yard's own tile while it is parked there.
    pub tile: Tile,
    pub phase: CartPhase,
    /// The building it fetches from and the one it delivers to.
    pub from: u32,
    pub to: u32,
    /// What it was sent to fetch.
    pub order: Stock,
    /// What is aboard.
    pub load: Stock,
    /// The tiles still to cross, the next one last.
    pub route: Vec<Tile>,
    /// Tenths of a second into the tile in hand, or seconds left at a berth.
    pub clock: u32,
    /// Seconds spent standing idle, or held up at a berth.
    pub idle: u32,
}

/// Maps with tuple keys cannot be JSON object keys: they are saved as lists of pairs.
mod pairs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;
    pub fn serialize<S: Serializer, K: Serialize + Ord, V: Serialize>(map: &BTreeMap<K, V>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(map.iter())
    }
    pub fn deserialize<'de, D: Deserializer<'de>, K: Deserialize<'de> + Ord, V: Deserialize<'de>>(d: D) -> Result<BTreeMap<K, V>, D::Error> {
        Ok(Vec::<(K, V)>::deserialize(d)?.into_iter().collect())
    }
}

/// An order no cart could take at the last planning pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unserved {
    pub from: u32,
    pub to: u32,
    pub good: String,
    pub units: u32,
    /// No road joins the two places, rather than no free cart.
    pub no_road: bool,
}

/// What the roads have done lately, for the Clerk and the charts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Roadwork {
    /// Where carts have been held up lately, by road tile: thirty times the
    /// carts that have been waiting there, so thirty is one cart held there
    /// all the time.
    #[serde(with = "pairs")]
    pub crowd: BTreeMap<Tile, u16>,
    /// Orders left over at the last pass, the most pressing first.
    pub unserved: Vec<Unserved>,
    /// Carts free at the last pass.
    pub free: u16,
    pub moved: u64,
    /// Kilograms delivered over the whole game, and trips made.
    pub kilos: u64,
    pub trips: u32,
    /// Units delivered in each of the last six ten-second shifts, oldest first.
    pub recent: [u16; 6],
    pub shift: u16,
    /// Cart-seconds (times eight, fading by an eighth each shift) at work, standing
    /// free, and held up in a jam or at a berth.
    pub busy: u32,
    pub free_for: u32,
    pub held: u32,
    /// Kilograms carried between each pair of buildings lately (from, to): about
    /// seventy times a kilogram a second, fading by an eighth each shift.
    #[serde(with = "pairs")]
    pub flow: BTreeMap<(u32, u32), u32>,
    /// The second each want (building, good) was first left waiting for a cart.
    #[serde(with = "pairs")]
    pub waiting: BTreeMap<(u32, String), u64>,
}

/// One carter's yard, as the player reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YardRow {
    pub id: u32,
    pub carts: usize,
    pub stalls: u16,
    pub free: usize,
    pub fed: bool,
    pub crewed: bool,
}

/// The roads, as the player reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoadReport {
    pub yards: Vec<YardRow>,
    pub carts: usize,
    pub free: usize,
    /// Units a minute actually delivered over the last minute.
    pub observed_per_min: u32,
    /// Units a minute the carts could carry on a haul of ten tiles.
    pub capacity_per_min: u32,
    pub unserved: Vec<Unserved>,
    /// Tiles where carts are held up, with how much (thirty is one cart held all the time).
    pub jams: Vec<(Tile, u16)>,
}

pub(crate) struct Net {
    w: usize,
    h: usize,
    road: Vec<bool>,
}
impl Net {
    fn at(&self, t: Tile) -> usize {
        t.1 as usize * self.w + t.0 as usize
    }
    fn has(&self, t: Tile) -> bool {
        t.0 >= 0 && t.1 >= 0 && (t.0 as usize) < self.w && (t.1 as usize) < self.h && self.road[self.at(t)]
    }
    /// Road tiles a cart standing at `t` can start from.
    fn starts(&self, t: Tile) -> Vec<Tile> {
        if self.has(t) {
            vec![t]
        } else {
            neighbours(t).into_iter().filter(|n| self.has(*n)).collect()
        }
    }
    /// Steps from the nearest of `sources` to every road tile.
    fn bfs(&self, sources: &[Tile]) -> Vec<u16> {
        let mut dist = vec![NONE; self.road.len()];
        let mut queue = VecDeque::new();
        for &s in sources {
            if self.has(s) && dist[self.at(s)] == NONE {
                dist[self.at(s)] = 0;
                queue.push_back(s);
            }
        }
        while let Some(t) = queue.pop_front() {
            let d = dist[self.at(t)];
            for n in neighbours(t) {
                if self.has(n) && dist[self.at(n)] == NONE {
                    dist[self.at(n)] = d.saturating_add(1);
                    queue.push_back(n);
                }
            }
        }
        dist
    }
    fn near(&self, dist: &[u16], tiles: &[Tile]) -> u16 {
        tiles.iter().filter(|t| self.has(**t)).map(|t| dist[self.at(*t)]).min().unwrap_or(NONE)
    }
}

/// A want: a building is short of a good.
struct Want {
    to: u32,
    good: String,
    deficit: u32,
    threshold: u32,
    prio: i32,
}
struct Order {
    from: u32,
    to: u32,
    goods: Stock,
    prio: i32,
}

struct Tick {
    net: Net,
    /// Carts on each road tile, moving, at this moment.
    occ: BTreeMap<Tile, u16>,
    /// Cart-seconds held up on each tile this second.
    crowd: BTreeMap<Tile, u16>,
    /// Carts at a berth of each building.
    busy: BTreeMap<u32, u8>,
    capacity: u32,
    lanes: u16,
    /// The camp's yard takes this many carts abreast; a road tile takes `lanes`.
    camp: Tile,
    jam: u32,
    /// Cart-seconds this second: at work, free, held up.
    work: u32,
    free: u32,
    held: u32,
    delivered: u32,
}

impl Kingdom {
    // ---- reading ---------------------------------------------------------

    /// Kilograms one cart takes.
    pub fn cart_capacity(c: &Catalog) -> u32 {
        u32::from(c.lines["cart"].capacity)
    }
    /// Units of `good` that fill a cart.
    pub fn cart_lot(c: &Catalog, good: &str) -> u32 {
        (Self::cart_capacity(c) / c.resources.get(good).map_or(50, |r| r.kilos())).max(1)
    }
    pub(crate) fn kilos(c: &Catalog, stock: &Stock) -> u32 {
        stock.iter().map(|(g, n)| n * c.resources.get(g).map_or(50, |r| r.kilos())).sum()
    }

    pub(crate) fn road_net(&self) -> Net {
        let (w, h) = self.world.bounds();
        let (w, h) = (w as usize, h as usize);
        let mut road = vec![false; w * h];
        for t in &self.roads {
            if t.0 >= 0 && t.1 >= 0 && (t.0 as usize) < w && (t.1 as usize) < h {
                road[t.1 as usize * w + t.0 as usize] = true;
            }
        }
        // The camp is a yard the carts drive through, so the roads on either side of it meet.
        let camp = self.camp();
        road[camp.1 as usize * w + camp.0 as usize] = true;
        Net { w, h, road }
    }

    /// Road tiles beside a building, where carts stand to load and unload.
    fn docks(&self, c: &Catalog, net: &Net, id: u32) -> Vec<Tile> {
        let Some(b) = self.buildings.get(&id) else { return vec![] };
        let _ = c;
        let mut docks = BTreeSet::new();
        for t in self.covered(b) {
            for n in neighbours(t) {
                if net.has(n) {
                    docks.insert(n);
                }
            }
        }
        docks.into_iter().collect()
    }

    /// Carts that have lately been held up on a tile, on average.
    fn heat(&self, t: Tile) -> u32 {
        u32::from(self.roadwork.crowd.get(&t).copied().unwrap_or(0)) / 30
    }

    /// Every cart in the realm.
    pub fn cart_count(&self) -> usize {
        self.buildings.values().map(|b| b.carts.len()).sum()
    }

    /// Whether a yard's oxen are fed and its carter at work.
    fn yard_active(&self, c: &Catalog, status: &BTreeMap<u32, Status>, id: u32) -> bool {
        let Some(b) = self.buildings.get(&id) else { return false };
        c.plans[&b.plan].stalls > 0 && b.fed > 0 && status.get(&id) == Some(&Status::Working)
    }

    // ---- the pool --------------------------------------------------------

    /// Sets the number of carts in a yard, buying or selling the difference.
    pub(crate) fn set_carts(&mut self, c: &Catalog, yard: u32, carts: u16) -> Result<String, String> {
        let b = self.buildings.get(&yard).ok_or("no such building")?;
        let p = &c.plans[&b.plan];
        if p.stalls == 0 {
            return Err("only a carter's yard keeps carts".into());
        }
        if b.construction > 0 {
            return Err("the yard is still being built".into());
        }
        if carts > p.stalls {
            return Err(format!("a {} has stalls for {} carts; raise another yard", p.name.to_lowercase(), p.stalls));
        }
        let spec = &c.lines["cart"];
        let have = b.carts.len() as u16;
        if carts > have {
            let mut cost = Stock::new();
            for (r, n) in &spec.vehicle_cost {
                add(&mut cost, r, n * u32::from(carts - have));
            }
            if !crate::economy::covers(&self.stock, &cost) {
                return Err(format!("{} more carts need {}", carts - have, label(&cost)));
            }
            take(&mut self.stock, &cost);
            let tile = b.tile;
            let b = self.buildings.get_mut(&yard).unwrap();
            for _ in have..carts {
                b.carts.push(Cart { tile, ..Cart::default() });
            }
        } else if carts < have {
            // Only carts standing idle in the yard can be sold.
            let b = self.buildings.get_mut(&yard).unwrap();
            let mut sell = usize::from(have - carts);
            let mut kept = Vec::new();
            let mut sold = 0u32;
            for cart in std::mem::take(&mut b.carts) {
                if sell > 0 && cart.order.is_empty() && cart.load.is_empty() && matches!(cart.phase, CartPhase::Idle | CartPhase::Return) {
                    sell -= 1;
                    sold += 1;
                } else {
                    kept.push(cart);
                }
            }
            b.carts = kept;
            if sold == 0 {
                return Err("every cart is on the road; sell when some stand free".into());
            }
            for (r, n) in &spec.vehicle_cost {
                add(&mut self.stock, r, n * sold / 2);
            }
        }
        Ok(format!("{} carts in the yard", self.buildings[&yard].carts.len()))
    }

    /// A yard pulled down: its cargo goes to the camp and its carts are sold.
    pub(crate) fn release_carts(&mut self, c: &Catalog, carts: Vec<Cart>) {
        let spec = &c.lines["cart"];
        for cart in &carts {
            for (g, n) in &cart.load {
                add(&mut self.stock, g, *n);
            }
            for (r, n) in &spec.vehicle_cost {
                add(&mut self.stock, r, *n);
            }
        }
    }

    /// Old saves laid cart lines between pairs of buildings. The roads carry
    /// that traffic now: the lines come down, their cargo goes to the camp and
    /// their vehicles go back into the stock.
    pub(crate) fn retire_cart_lines(&mut self, c: &Catalog) {
        let old: Vec<u32> = self.lines.iter().filter(|(_, l)| l.kind == LineKind::Cart).map(|(&id, _)| id).collect();
        for id in old {
            let n = self.lines[&id].fleet.len() as u32;
            self.remove_line(id);
            if let Some(spec) = c.lines.get("cart") {
                for (r, per) in &spec.vehicle_cost {
                    add(&mut self.stock, r, per * n);
                }
            }
        }
    }

    // ---- one second of the roads ------------------------------------------

    pub(crate) fn haul_roads(&mut self, c: &Catalog, capacity: u32, status: &BTreeMap<u32, Status>) {
        let yards: Vec<u32> = self.buildings.iter().filter(|(_, b)| !b.carts.is_empty()).map(|(&id, _)| id).collect();
        if yards.is_empty() {
            return;
        }
        let spec = &c.lines["cart"];
        let mut t = Tick {
            net: self.road_net(),
            occ: BTreeMap::new(),
            crowd: BTreeMap::new(),
            busy: BTreeMap::new(),
            capacity,
            lanes: u16::from(c.works.road_lanes),
            camp: self.camp(),
            jam: u32::from(c.works.jam_percent),
            work: 0,
            free: 0,
            held: 0,
            delivered: 0,
        };
        for &y in &yards {
            if !self.yard_active(c, status, y) {
                continue;
            }
            for cart in &self.buildings[&y].carts {
                match cart.phase {
                    CartPhase::Fetch | CartPhase::Haul | CartPhase::Return => *t.occ.entry(cart.tile).or_default() += 1,
                    CartPhase::Loading => *t.busy.entry(cart.from).or_default() += 1,
                    CartPhase::Unloading => *t.busy.entry(cart.to).or_default() += 1,
                    _ => {}
                }
            }
        }
        // Whoever steps first changes with the second, so no yard has first call
        // on a berth.
        let turn = (self.second % yards.len() as u64) as usize;
        let mut order = yards.clone();
        order.rotate_left(turn);
        for y in order {
            if !self.yard_active(c, status, y) {
                // A yard that stands still (no carter, no fodder) keeps its loads aboard but
                // gives up the orders it had not yet filled, so the goods are not held for carts
                // that will not come.
                for cart in &mut self.buildings.get_mut(&y).unwrap().carts {
                    if matches!(cart.phase, CartPhase::Fetch | CartPhase::Queue | CartPhase::Loading) {
                        cart.order.clear();
                        cart.from = 0;
                        cart.to = 0;
                        cart.route.clear();
                        cart.clock = 0;
                        cart.idle = 0;
                        cart.phase = CartPhase::Idle;
                    }
                }
                continue;
            }
            let mut carts = std::mem::take(&mut self.buildings.get_mut(&y).unwrap().carts);
            for cart in &mut carts {
                self.cart_step(c, &mut t, y, cart, spec);
            }
            self.buildings.get_mut(&y).unwrap().carts = carts;
        }
        // A jam is remembered: where carts were held up this second.
        for (&tile, &n) in &t.crowd {
            let e = self.roadwork.crowd.entry(tile).or_default();
            *e = e.saturating_add(n);
        }
        self.roadwork.busy += 8 * t.work;
        self.roadwork.free_for += 8 * t.free;
        self.roadwork.held += 8 * t.held;
        self.roadwork.shift = self.roadwork.shift.saturating_add(t.delivered as u16);
        if self.second % PLAN_EVERY == 0 {
            self.plan_roads(c, &t, &yards, status);
        }
        if self.second % 10 == 0 {
            let r = &mut self.roadwork;
            r.recent.rotate_left(1);
            r.recent[5] = std::mem::take(&mut r.shift);
            for v in r.crowd.values_mut() {
                *v -= (*v).div_ceil(4);
            }
            r.crowd.retain(|_, v| *v > 0);
            for v in r.flow.values_mut() {
                *v -= (*v).div_ceil(8);
            }
            r.flow.retain(|_, v| *v > 0);
            r.busy -= r.busy.div_ceil(8);
            r.free_for -= r.free_for.div_ceil(8);
            r.held -= r.held.div_ceil(8);
        }
    }

    fn cart_step(&mut self, c: &Catalog, t: &mut Tick, yard: u32, cart: &mut Cart, spec: &LineSpec) {
        let dwell = u32::from(spec.dwell);
        let berths = |k: &Kingdom, id: u32| k.berths_of(c, id);
        match cart.phase {
            CartPhase::Idle => {
                cart.idle += 1;
                t.free += 1;
                let home = self.buildings[&yard].tile;
                if cart.idle >= IDLE_HOME && cart.tile != home {
                    if let Some(route) = self.route_to(t, cart.tile, &self.docks(c, &t.net, yard)) {
                        cart.route = route;
                        cart.clock = 0;
                        cart.phase = CartPhase::Return;
                    } else {
                        // Stranded off the roads: it walks home.
                        cart.tile = home;
                    }
                }
            }
            CartPhase::Return => {
                t.free += 1;
                if self.advance(t, cart, spec) {
                    cart.tile = self.buildings[&yard].tile;
                    cart.phase = CartPhase::Idle;
                    cart.idle = 0;
                    cart.route.clear();
                }
            }
            CartPhase::Fetch => {
                t.work += 1;
                if !self.buildings.contains_key(&cart.from) || !self.buildings.contains_key(&cart.to) {
                    self.drop_order(cart);
                } else if self.advance(t, cart, spec) {
                    cart.phase = CartPhase::Queue;
                    cart.idle = 0;
                }
            }
            CartPhase::Queue => {
                if !self.buildings.contains_key(&cart.from) || !self.buildings.contains_key(&cart.to) {
                    return self.drop_order(cart);
                }
                let there = cart.order.keys().any(|g| self.avail(c, cart.from, g, cart.to) > 0);
                if !there {
                    // Someone else took what it came for.
                    return self.drop_order(cart);
                }
                if t.busy.get(&cart.from).copied().unwrap_or(0) < berths(self, cart.from) {
                    *t.busy.entry(cart.from).or_default() += 1;
                    cart.phase = CartPhase::Loading;
                    cart.clock = dwell;
                    t.work += 1;
                } else {
                    cart.idle += 1;
                    t.held += 1;
                }
            }
            CartPhase::Loading => {
                t.work += 1;
                cart.clock = cart.clock.saturating_sub(1);
                if cart.clock == 0 {
                    if let Some(n) = t.busy.get_mut(&cart.from) {
                        *n = n.saturating_sub(1);
                    }
                    let order = std::mem::take(&mut cart.order);
                    let mut room = Self::cart_capacity(c);
                    for (g, want) in order {
                        if !self.buildings.contains_key(&cart.from) {
                            break;
                        }
                        let kilos = c.resources.get(&g).map_or(50, |r| r.kilos());
                        let n = want.min(self.avail(c, cart.from, &g, cart.to)).min(room / kilos);
                        if n > 0 {
                            self.draw(cart.from, &g, n);
                            add(&mut cart.load, &g, n);
                            room -= n * kilos;
                        }
                    }
                    if cart.load.is_empty() {
                        return self.drop_order(cart);
                    }
                    let docks = self.docks(c, &t.net, cart.to);
                    match self.route_to(t, cart.tile, &docks) {
                        Some(route) => {
                            cart.route = route;
                            cart.clock = 0;
                            cart.phase = CartPhase::Haul;
                        }
                        // No road to the drop: the load goes back where it came from.
                        None => {
                            cart.to = cart.from;
                            let docks = self.docks(c, &t.net, cart.to);
                            cart.route = self.route_to(t, cart.tile, &docks).unwrap_or_default();
                            cart.phase = if cart.route.is_empty() { CartPhase::Wait } else { CartPhase::Haul };
                        }
                    }
                }
            }
            CartPhase::Haul => {
                t.work += 1;
                if !self.buildings.contains_key(&cart.to) {
                    cart.to = 1;
                    let docks = self.docks(c, &t.net, 1);
                    cart.route = self.route_to(t, cart.tile, &docks).unwrap_or_default();
                }
                if self.advance(t, cart, spec) {
                    cart.phase = CartPhase::Wait;
                    cart.idle = 0;
                }
            }
            CartPhase::Wait => {
                if !self.buildings.contains_key(&cart.to) {
                    cart.to = 1;
                }
                let space = cart.to == 1 || cart.load.iter().any(|(g, _)| self.unloadable(c, t, cart.to, g) > 0);
                if !space {
                    // The store is full: a load held up for long enough goes to the camp,
                    // which takes anything.
                    cart.idle += 1;
                    t.held += 1;
                    if cart.idle >= PATIENCE {
                        cart.to = 1;
                        let docks = self.docks(c, &t.net, 1);
                        if let Some(route) = self.route_to(t, cart.tile, &docks) {
                            cart.route = route;
                            cart.clock = 0;
                            cart.phase = CartPhase::Haul;
                        }
                    }
                } else if t.busy.get(&cart.to).copied().unwrap_or(0) < berths(self, cart.to) {
                    *t.busy.entry(cart.to).or_default() += 1;
                    cart.phase = CartPhase::Unloading;
                    cart.clock = dwell;
                    cart.idle = 0;
                    t.work += 1;
                } else {
                    // Waiting for a berth is only a queue.
                    t.held += 1;
                }
            }
            CartPhase::Unloading => {
                t.work += 1;
                cart.clock = cart.clock.saturating_sub(1);
                if cart.clock == 0 {
                    if let Some(n) = t.busy.get_mut(&cart.to) {
                        *n = n.saturating_sub(1);
                    }
                    let load = std::mem::take(&mut cart.load);
                    let mut left = Stock::new();
                    for (g, n) in load {
                        let d = if cart.to == 1 { n } else { n.min(self.unloadable(c, t, cart.to, &g)) };
                        if d > 0 {
                            self.give(c, cart.to, &g, d);
                            t.delivered += d;
                            self.roadwork.moved += u64::from(d);
                            let kilos = d * c.resources.get(&g).map_or(50, |r| r.kilos());
                            self.roadwork.kilos += u64::from(kilos);
                            let e = self.roadwork.flow.entry((cart.from, cart.to)).or_default();
                            *e = e.saturating_add(kilos);
                        }
                        if n > d {
                            add(&mut left, &g, n - d);
                        }
                    }
                    cart.load = left;
                    if cart.load.is_empty() {
                        self.roadwork.trips += 1;
                        cart.to = 0;
                        cart.from = 0;
                        cart.phase = CartPhase::Idle;
                        cart.idle = 0;
                    } else {
                        cart.phase = CartPhase::Wait;
                        cart.idle = 0;
                    }
                }
            }
        }
    }

    /// Units of `good` the place can take this moment, whatever is on the way.
    fn unloadable(&self, c: &Catalog, t: &Tick, id: u32, good: &str) -> u32 {
        self.cap_at(c, id, t.capacity).saturating_sub(self.have_at(c, id, good))
    }

    /// An order that cannot be filled: the cart is free again.
    fn drop_order(&self, cart: &mut Cart) {
        cart.order.clear();
        cart.from = 0;
        cart.to = 0;
        cart.route.clear();
        cart.clock = 0;
        cart.idle = 0;
        cart.phase = if cart.load.is_empty() { CartPhase::Idle } else { CartPhase::Wait };
    }

    /// Takes the cart along its route for one second. Each tile costs its
    /// time, longer when more carts are on it than the road takes.
    fn advance(&self, t: &mut Tick, cart: &mut Cart, spec: &LineSpec) -> bool {
        cart.clock += u32::from(crate::pace::travel_clock());
        loop {
            let Some(&next) = cart.route.last() else {
                cart.clock = 0;
                return true;
            };
            if !t.net.has(next) {
                // The road is cut ahead: it waits for it to be mended.
                t.held += 1;
                return false;
            }
            let lanes = if next == t.camp { CAMP_LANES } else { t.lanes };
            let over = u32::from(t.occ.get(&next).copied().unwrap_or(0).saturating_sub(lanes));
            let cost = u32::from(spec.speed) * (100 + t.jam * over) / 100;
            if over > 0 {
                t.held += 1;
                *t.crowd.entry(next).or_default() += 1;
            }
            if cart.clock < cost {
                return false;
            }
            cart.clock -= cost;
            cart.route.pop();
            if let Some(n) = t.occ.get_mut(&cart.tile) {
                *n = n.saturating_sub(1);
            }
            cart.tile = next;
            *t.occ.entry(next).or_default() += 1;
            if cart.route.is_empty() {
                cart.clock = 0;
                return true;
            }
        }
    }

    /// The cheapest way along the roads, counting what traffic has done to
    /// each tile; the route is returned with its next tile last.
    fn route_to(&self, t: &Tick, from: Tile, goals: &[Tile]) -> Option<Vec<Tile>> {
        self.route_net(&t.net, from, goals)
    }

    pub(crate) fn route_net(&self, net: &Net, from: Tile, goals: &[Tile]) -> Option<Vec<Tile>> {
        let goal: BTreeSet<Tile> = goals.iter().copied().filter(|g| net.has(*g)).collect();
        if goal.is_empty() {
            return None;
        }
        let mut best = vec![u32::MAX; net.road.len()];
        let mut parent: Vec<Option<Tile>> = vec![None; net.road.len()];
        let mut queue = BinaryHeap::new();
        for s in net.starts(from) {
            best[net.at(s)] = 0;
            queue.push(Reverse((0u32, s)));
        }
        while let Some(Reverse((d, tile))) = queue.pop() {
            if d != best[net.at(tile)] {
                continue;
            }
            if goal.contains(&tile) {
                let mut path = vec![tile];
                let mut cursor = tile;
                while let Some(p) = parent[net.at(cursor)] {
                    path.push(p);
                    cursor = p;
                }
                // `path` runs goal back to start. A start that is the tile the
                // cart already stands on is not a step to take.
                if path.last() == Some(&from) {
                    path.pop();
                }
                return Some(path);
            }
            for n in neighbours(tile) {
                if !net.has(n) {
                    continue;
                }
                let nd = d + 10 + 10 * self.heat(n);
                if nd < best[net.at(n)] {
                    best[net.at(n)] = nd;
                    parent[net.at(n)] = Some(tile);
                    queue.push(Reverse((nd, n)));
                }
            }
        }
        None
    }

    // ---- planning ---------------------------------------------------------

    /// What a building asks the roads to bring it, as the level it likes to
    /// keep and the priority of the asking.
    fn pull_levels(&self, c: &Catalog, id: u32) -> (Stock, i32) {
        if id == 1 {
            return (c.camp_stock.iter().map(|(g, n)| (g.clone(), *n)).collect(), 60);
        }
        let Some(b) = self.buildings.get(&id) else { return (Stock::new(), 0) };
        let p = &c.plans[&b.plan];
        if p.hub {
            return (Stock::new(), 0);
        }
        let base = 100 + 40 * i32::from(b.priority);
        if p.stalls > 0 {
            if b.construction > 0 || b.paused {
                return (Stock::new(), 0);
            }
            let fodder: Stock = c.lines["cart"].upkeep.iter().map(|(g, n)| (g.clone(), (n * b.carts.len() as u32 * 2).max(2))).collect();
            return (fodder, base + 40);
        }
        // Never ask for more than the store holds: a cart that cannot unload goes round again.
        let cap = p.input_cap();
        let levels = self.target_levels(c, id).unwrap_or_default().into_iter().map(|(g, n)| (g, n.min(cap))).collect();
        (levels, if p.yard { base + 20 } else { base })
    }

    /// Whether a point-to-point line already carries `good` to `to`. A line whose vehicles
    /// stand idle at its origin (nothing to carry), or that is cut or unfed, is not carrying
    /// anything: the roads fetch the good from wherever else it is held, so a work is never
    /// left short for want of the one maker its line happens to run from.
    fn line_feeds(&self, c: &Catalog, idle_lines: &BTreeSet<u32>, to: u32, good: &str) -> bool {
        self.lines.iter().filter(|(id, _)| !idle_lines.contains(id)).map(|(_, l)| l).any(|l| {
            l.to == to
                && l.kind != LineKind::Cart
                && (l.from == 1
                    || self.buildings.get(&l.from).is_some_and(|b| {
                        let p = &c.plans[&b.plan];
                        p.hub
                            || b.output.contains_key(good)
                            || if p.extractor {
                                b.claim.as_ref().and_then(|q| self.claims.get(q)).is_some_and(|q| q.output().contains_key(good))
                            } else {
                                b.recipe.as_ref().is_some_and(|r| c.recipes[r].outputs.contains_key(good))
                            }
                    }))
        })
    }

    /// Whether any building standing uses `good` in a recipe it can run.
    fn consumed_somewhere(&self, c: &Catalog, good: &str) -> bool {
        self.buildings.values().any(|b| {
            let p = &c.plans[&b.plan];
            p.recipes.iter().any(|r| c.recipes[r].inputs.contains_key(good)) || (p.yard && !self.sites.is_empty())
        })
    }

    fn plan_roads(&mut self, c: &Catalog, t: &Tick, yards: &[u32], status: &BTreeMap<u32, Status>) {
        // Who is free, and what is already promised.
        let mut out_res: BTreeMap<(u32, String), u32> = BTreeMap::new();
        let mut in_res: BTreeMap<(u32, String), u32> = BTreeMap::new();
        let mut available: Vec<(u32, usize)> = Vec::new();
        for &y in yards {
            let active = self.yard_active(c, status, y);
            for (i, cart) in self.buildings[&y].carts.iter().enumerate() {
                for (g, n) in &cart.order {
                    *out_res.entry((cart.from, g.clone())).or_default() += n;
                    *in_res.entry((cart.to, g.clone())).or_default() += n;
                }
                for (g, n) in &cart.load {
                    *in_res.entry((cart.to, g.clone())).or_default() += n;
                }
                if active && matches!(cart.phase, CartPhase::Idle | CartPhase::Return) {
                    available.push((y, i));
                }
            }
        }
        for l in self.lines.values() {
            for v in &l.fleet {
                if v.amount > 0 {
                    *in_res.entry((l.to, v.resource.clone())).or_default() += v.amount;
                }
            }
        }
        self.roadwork.free = available.len() as u16;
        // Without a free cart there is nothing to assign; the last pass's list stands.
        if available.is_empty() && !self.roadwork.unserved.is_empty() {
            return;
        }
        let net = &t.net;
        let mut docks: BTreeMap<u32, Vec<Tile>> = BTreeMap::new();
        let mut maps: BTreeMap<u32, Vec<u16>> = BTreeMap::new();
        let ids: Vec<u32> = std::iter::once(1).chain(self.buildings.keys().copied().filter(|&i| i != 1)).collect();
        for &id in &ids {
            docks.insert(id, self.docks(c, net, id));
        }
        // Who holds what, ready to be fetched.
        let mut holders: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
        for &id in &ids {
            let Some(b) = self.buildings.get(&id) else { continue };
            if b.construction > 0 || c.plans[&b.plan].yard || docks[&id].is_empty() {
                continue;
            }
            let goods: Vec<String> = if id == 1 { self.stock.keys().cloned().collect() } else { b.output.keys().cloned().collect() };
            for g in goods {
                let have = self.avail(c, id, &g, 0).saturating_sub(out_res.get(&(id, g.clone())).copied().unwrap_or(0));
                if have > 0 {
                    holders.entry(g).or_default().push((id, have));
                }
            }
        }
        let lot = |g: &str| Self::cart_lot(c, g);
        let idle_lines: BTreeSet<u32> = if self.lines.is_empty() {
            BTreeSet::new()
        } else {
            self.line_report(c)
                .into_iter()
                .filter(|r| matches!(r.state, crate::haul::LineState::Starved | crate::haul::LineState::Cut | crate::haul::LineState::Unfed))
                .map(|r| r.id)
                .collect()
        };
        // What the wall is waiting on is fetched however little there is: the last few
        // stones of a stage are never held up for want of a full cart.
        let gating: Vec<(u32, String)> = self
            .sites
            .iter()
            .filter_map(|(&sid, site)| match self.site_allowance(c, sid, 99).hold {
                Hold::Short { resource } => Some((site.yard, resource)),
                _ => None,
            })
            .collect();
        // What is wanted.
        let mut wants: Vec<Want> = Vec::new();
        for &id in &ids {
            let Some(b) = self.buildings.get(&id) else { continue };
            if docks[&id].is_empty() || (id != 1 && (b.paused || b.construction > 0)) {
                continue;
            }
            let (levels, prio) = self.pull_levels(c, id);
            for (g, target) in levels {
                if target == 0 || self.line_feeds(c, &idle_lines, id, &g) {
                    continue;
                }
                let have = self.have_at(c, id, &g);
                let coming = in_res.get(&(id, g.clone())).copied().unwrap_or(0);
                if have + coming >= target {
                    continue;
                }
                let deficit = target - have - coming;
                // A bench that is nearly out asks for whatever there is; a full one waits
                // until it is half empty.
                // The wall's yard and the camp send for a quarter of what they keep; a bench for half.
                let batch = (target / 8).max(1);
                let share = if id == 1 || c.plans.get(&self.buildings[&id].plan).is_some_and(|p| p.yard) { 4 } else { 2 };
                let gated = gating.iter().any(|(y, r)| *y == id && *r == g);
                let mut threshold = if have + coming < batch || gated { 1 } else { lot(&g).min((target / share).max(1)) };
                // The camp's level is what purchases are paid out of, and a purchase as dear as the level (twelve
                // iron ore for the second charter, thirty beams for the great crane) could never be made if the camp
                // stopped one lot short of it: a store of building goods is topped up to its level, a few at a time.
                if id == 1 && deficit < threshold && c.resources.get(&g).is_some_and(|r| r.shelf == 0) && g != "fodder" && !c.diet.labourer.iter().chain(&c.diet.skilled).chain(&c.diet.master).any(|d| d.split('|').any(|x| x == g)) {
                    threshold = deficit;
                }
                if deficit < threshold {
                    continue;
                }
                let urgency = (100 * (target - (have + coming).min(target)) / target) as i32;
                // People come first: the camp's bread and ale, and the oxen's fodder.
                let people = if id == 1 && c.diet.labourer.iter().chain(&c.diet.skilled).any(|d| d.split('|').any(|x| x == g)) { 80 } else { 0 };
                wants.push(Want { to: id, good: g, deficit, threshold, prio: prio + urgency + people });
            }
        }
        // The longer a bench has waited, the more urgent its goods: nothing starves behind
        // louder neighbours.
        let now = self.second;
        let mut waiting: BTreeMap<(u32, String), u64> = BTreeMap::new();
        for w in &mut wants {
            let since = self.roadwork.waiting.get(&(w.to, w.good.clone())).copied().unwrap_or(now);
            w.prio += ((now - since) / AGE_SECONDS).min(300) as i32;
            waiting.insert((w.to, w.good.clone()), since);
        }
        self.roadwork.waiting = waiting;
        wants.sort_by(|a, b| b.prio.cmp(&a.prio).then(a.to.cmp(&b.to)).then(a.good.cmp(&b.good)));
        let mut orders: BTreeMap<(u32, u32), Order> = BTreeMap::new();
        let mut unserved: Vec<Unserved> = Vec::new();
        let mut pulled: BTreeMap<(u32, String), u32> = BTreeMap::new();
        for w in &wants {
            let map = maps.entry(w.to).or_insert_with(|| net.bfs(&docks[&w.to]));
            let mut best: Option<(u16, u8, u32, u32)> = None;
            let mut cut = false;
            for &(s, have) in holders.get(&w.good).map(Vec::as_slice).unwrap_or(&[]) {
                if s == w.to {
                    continue;
                }
                let have = have.saturating_sub(pulled.get(&(s, w.good.clone())).copied().unwrap_or(0));
                if have == 0 {
                    continue;
                }
                let d = net.near(map, &docks[&s]);
                if d == NONE {
                    cut = true;
                    continue;
                }
                // A bench that holds all its shelf can hold is always worth a trip, whatever the
                // asker's lot: the camp asks bread in lots of thirty and a bakery's shelf holds
                // twenty-four, so without this the camp's bread was fetched only when it was nearly out.
                let full = s != 1 && self.buildings.get(&s).is_some_and(|hb| hb.output.get(&w.good).copied().unwrap_or(0) >= c.plans[&hb.plan].output_cap());
                if have < w.threshold.min(w.deficit) && !full {
                    continue;
                }
                // The nearest wins; a maker's own bench before a store, and the camp, which every
                // cart crosses, counts as a few tiles further than it is.
                let rank = if s == 1 { 2 } else if self.buildings[&s].output.contains_key(&w.good) && !c.plans[&self.buildings[&s].plan].hub { 0 } else { 1 };
                let key = (d + if s == 1 { CAMP_DETOUR } else { 0 }, rank, s, have);
                if best.is_none_or(|b| (key.0, key.1, key.2) < (b.0, b.1, b.2)) {
                    best = Some(key);
                }
            }
            match best {
                Some((_, _, s, have)) => {
                    let n = w.deficit.min(have);
                    *pulled.entry((s, w.good.clone())).or_default() += n;
                    let o = orders.entry((s, w.to)).or_insert(Order { from: s, to: w.to, goods: Stock::new(), prio: w.prio });
                    add(&mut o.goods, &w.good, n);
                    o.prio = o.prio.max(w.prio);
                }
                None if cut => unserved.push(Unserved { from: 0, to: w.to, good: w.good.clone(), units: w.deficit, no_road: true }),
                None => {}
            }
        }
        // Makers whose benches are filling push to the nearest store with room, but only
        // goods somebody wants and only a short way: a store is a stock held for the works
        // around it, not a place to dump whatever a bench makes.
        let wanted: BTreeSet<&str> = wants.iter().map(|w| w.good.as_str()).chain(c.camp_stock.keys().map(String::as_str)).collect();
        let hubs: Vec<u32> = ids
            .iter()
            .copied()
            .filter(|&id| id != 1 && self.buildings.get(&id).is_some_and(|b| c.plans[&b.plan].hub && b.construction == 0 && !docks[&id].is_empty()))
            .collect();
        for &p in &ids {
            if p == 1 {
                continue;
            }
            let Some(b) = self.buildings.get(&p) else { continue };
            let plan = &c.plans[&b.plan];
            if b.construction > 0 || plan.hub || plan.yard || docks[&p].is_empty() || plan.stalls > 0 {
                continue;
            }
            let cap = plan.output_cap();
            for (g, &held) in &b.output {
                if !wanted.contains(g.as_str()) && !self.consumed_somewhere(c, g) {
                    continue;
                }
                let spare = held.saturating_sub(out_res.get(&(p, g.clone())).copied().unwrap_or(0)).saturating_sub(pulled.get(&(p, g.clone())).copied().unwrap_or(0));
                if spare * 2 < cap || spare == 0 {
                    continue;
                }
                let map = maps.entry(p).or_insert_with(|| net.bfs(&docks[&p]));
                // Nearest store with room: a warehouse, or the camp while it is under its level.
                let mut best: Option<(u16, u32, u32)> = None;
                for &h in hubs.iter().chain(std::iter::once(&1)) {
                    let room = if h == 1 {
                        c.camp_stock.get(g).copied().unwrap_or(0).saturating_sub(self.have_at(c, 1, g) + in_res.get(&(1, g.clone())).copied().unwrap_or(0))
                    } else {
                        self.cap_at(c, h, t.capacity).saturating_sub(self.have_at(c, h, g) + in_res.get(&(h, g.clone())).copied().unwrap_or(0))
                    };
                    if room == 0 {
                        continue;
                    }
                    let d = net.near(map, &docks[&h]);
                    if d != NONE && d <= PUSH_RANGE && best.is_none_or(|b| (d, h) < (b.0, b.1)) {
                        best = Some((d, h, room));
                    }
                }
                if let Some((_, h, room)) = best {
                    let n = spare.min(room);
                    let o = orders.entry((p, h)).or_insert(Order { from: p, to: h, goods: Stock::new(), prio: 40 });
                    add(&mut o.goods, g, n);
                    *in_res.entry((h, g.clone())).or_default() += n;
                }
            }
        }
        // Orders in the order of their need; one cart each.
        let mut list: Vec<Order> = orders.into_values().collect();
        list.sort_by(|a, b| b.prio.cmp(&a.prio).then(a.from.cmp(&b.from)).then(a.to.cmp(&b.to)));
        let capacity = Self::cart_capacity(c);
        for mut o in list {
            // Trim to a cart's load, the most pressing goods first by name order.
            let mut room = capacity;
            let mut goods = Stock::new();
            for (g, n) in std::mem::take(&mut o.goods) {
                let kilos = c.resources.get(&g).map_or(50, |r| r.kilos());
                let take_n = n.min(room / kilos);
                if take_n > 0 {
                    room -= take_n * kilos;
                    goods.insert(g, take_n);
                }
            }
            if goods.is_empty() {
                continue;
            }
            // The nearest free cart that can reach both ends.
            let map = maps.entry(o.from).or_insert_with(|| net.bfs(&docks[&o.from]));
            let mut pick: Option<(u16, usize)> = None;
            let reach = net.near(map, &docks[&o.to]) != NONE;
            if reach {
                for (k, &(y, i)) in available.iter().enumerate() {
                    let cart = &self.buildings[&y].carts[i];
                    // A cart parked in its yard sets out from any road tile that touches the yard, not only
                    // from the neighbours of the yard's corner.
                    let starts = if cart.tile == self.buildings[&y].tile && !net.has(cart.tile) { docks[&y].clone() } else { net.starts(cart.tile) };
                    let d = net.near(map, &starts);
                    if d != NONE && pick.is_none_or(|p| d < p.0) {
                        pick = Some((d, k));
                    }
                }
            }
            match pick {
                Some((_, k)) => {
                    let (y, i) = available.swap_remove(k);
                    let mut start = self.buildings[&y].carts[i].tile;
                    if start == self.buildings[&y].tile && !net.has(start) {
                        if let Some(&d) = docks[&y].iter().min_by_key(|d| map[net.at(**d)]) {
                            start = d;
                        }
                    }
                    let route = self.route_to(t, start, &docks[&o.from]);
                    let cart = &mut self.buildings.get_mut(&y).unwrap().carts[i];
                    // A cart already at the dock has nothing to cross.
                    cart.route = route.unwrap_or_default();
                    cart.from = o.from;
                    cart.to = o.to;
                    cart.order = goods;
                    cart.clock = 0;
                    cart.idle = 0;
                    cart.phase = if cart.route.is_empty() { CartPhase::Queue } else { CartPhase::Fetch };
                }
                None => {
                    if unserved.len() < 24 {
                        for (g, n) in goods {
                            unserved.push(Unserved { from: o.from, to: o.to, good: g, units: n, no_road: !reach });
                        }
                    }
                }
            }
        }
        unserved.truncate(24);
        self.roadwork.unserved = unserved;
    }

    // ---- reading the roads -------------------------------------------------

    pub fn road_report(&self, c: &Catalog) -> RoadReport {
        let report = self.report(c);
        let spec = &c.lines["cart"];
        let mut r = RoadReport::default();
        for (&id, b) in &self.buildings {
            if c.plans[&b.plan].stalls == 0 {
                continue;
            }
            let free = b.carts.iter().filter(|x| matches!(x.phase, CartPhase::Idle | CartPhase::Return)).count();
            r.carts += b.carts.len();
            r.free += free;
            r.yards.push(YardRow {
                id,
                carts: b.carts.len(),
                stalls: c.plans[&b.plan].stalls,
                free,
                fed: b.fed > 0,
                crewed: report.status.get(&id) != Some(&Status::Workers),
            });
        }
        r.observed_per_min = self.roadwork.recent.iter().map(|&n| u32::from(n)).sum();
        // A round trip of ten tiles each way, loaded one way.
        let cycle = 2 * 10 * u32::from(spec.speed) / 10 + 2 * u32::from(spec.dwell);
        r.capacity_per_min = r.carts as u32 * 10 * 60 / cycle.max(1);
        r.unserved = self.roadwork.unserved.clone();
        // Half a cart held up all the time is a jam.
        let mut jams: Vec<(Tile, u16)> = self
            .roadwork
            .crowd
            .iter()
            .filter(|(_, v)| **v >= 15)
            .map(|(&t, v)| (t, *v))
            .collect();
        jams.sort_by_key(|(t, n)| (Reverse(*n), *t));
        jams.truncate(8);
        r.jams = jams;
        r
    }

    /// The way a road runs from the camp, in a few words: "the east road".
    pub fn road_name(&self, tile: Tile) -> String {
        let camp = self.camp();
        let (dx, dy) = (i32::from(tile.0 - camp.0), i32::from(tile.1 - camp.1));
        let way = if dx.abs() >= 2 * dy.abs() {
            if dx > 0 { "east" } else { "west" }
        } else if dy.abs() >= 2 * dx.abs() {
            if dy > 0 { "south" } else { "north" }
        } else {
            match (dx > 0, dy > 0) {
                (true, true) => "south-east",
                (true, false) => "north-east",
                (false, true) => "south-west",
                (false, false) => "north-west",
            }
        };
        format!("the {way} road")
    }

    /// Goods on carts, for the ledger.
    pub(crate) fn carted(&self, resource: &str) -> u32 {
        self.buildings.values().flat_map(|b| &b.carts).map(|x| x.load.get(resource).copied().unwrap_or(0)).sum()
    }

    /// Kilograms aboard a cart, for tests and the charts.
    pub fn cart_load_kilos(c: &Catalog, cart: &Cart) -> u32 {
        Self::kilos(c, &cart.load)
    }
}
