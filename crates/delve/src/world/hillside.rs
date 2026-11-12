//! The hillside works as the renderers see them: the production sim's
//! buildings, roads, lines, carts and people read once a sim second into a
//! plain picture (`HillView`), shared by the flat view and the 3D one, so the
//! two draw the same works. Nothing here draws, and nothing here changes the
//! sim: it is a pure reading of `Run.home.industry`, kept for the second it was
//! read in (a client reads the same of the state it is sent).
//!
//! What moves (carts, line vehicles, people) is given as where it was and where
//! it is going with how far along, at the moment of the reading; [`Rider`] and
//! the like place it between two ticks.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::kingdom::{CartPhase, Kingdom, LineKind, LineState, Phase, Piece, Status, Tile, catalog};
use crate::render::overworld::works_kit::lines::{self, Dir, Kind as Way};
use crate::render::overworld::works_kit::{self as kit, Load, State, Trade};
use crate::rules::{HZ, Run};

/// One building, read.
#[derive(Clone, Debug)]
pub struct Placed {
    pub id: u32,
    /// The sim's plan id.
    pub plan: String,
    /// The kit piece that draws it (`""`: this one is drawn elsewhere, the Great Shaft's head).
    pub art: &'static str,
    /// The footprint in tiles, inclusive: `[x0, y0, x1, y1]`.
    pub rect: [i16; 4],
    pub state: State,
    pub status: Status,
    /// Hands at work, and standing idle at the post waiting on goods.
    pub crew: u16,
    pub waiting: u16,
    pub paused: bool,
    /// What it lacks (the good that starves it) or cannot send on (the good that backs it up).
    pub short: Option<String>,
    /// The way a swape faces (0 north, 1 east, 2 south, 3 west), and the one good it lifts.
    pub facing: u8,
    pub filter: Option<String>,
    /// A defence the Deep has battered: what it has left and what it began with.
    pub hurt: Option<(u16, u16)>,
}

/// A tile of rollerway, read: what it is, the way it faces, and the goods on it.
#[derive(Clone, Debug)]
pub struct Belt {
    pub tile: Tile,
    pub piece: Piece,
    /// 0 north, 1 east, 2 south, 3 west.
    pub dir: u8,
    /// The goods in its slots, entry end first; a trestle's second line is `across`.
    pub slots: [Option<Load>; 3],
    pub across: [Option<Load>; 3],
    /// Beats of the last second in which something on it moved.
    pub moved: u8,
    /// Whether a hand-chain tile has its labourer.
    pub crewed: bool,
    /// Seconds the good at its exit has stood.
    pub stuck: u16,
}

/// A tile of rail, read: the neighbours it joins, whether it is a switch, and the bay that stands on it.
#[derive(Clone, Debug)]
pub struct RailTile {
    pub tile: Tile,
    pub links: u8,
    pub switch: bool,
    /// For a switch, the way (0 north, 1 east, 2 south, 3 west) it is thrown for the wagon that is at it
    /// or coming to it, when one is: the way that wagon goes on from here. None: no wagon is near.
    pub throw: Option<u8>,
    pub bay: Option<RailBay>,
}

/// A named bay, and the way it faces the building it serves (0 north, 1 east, 2 south, 3 west).
#[derive(Clone, Debug)]
pub struct RailBay {
    pub name: String,
    pub side: u8,
    /// A wagon is working it now.
    pub held: bool,
}

/// A wagon, read: where it is going and how it is.
#[derive(Clone, Debug)]
pub struct Hauled {
    pub id: u32,
    pub rider: Rider,
    pub state: crate::kingdom::WagonState,
}

/// A tile of road and the neighbours it joins (`lines::N | E | S | W`).
#[derive(Clone, Copy, Debug)]
pub struct Way4 {
    pub tile: Tile,
    pub links: u8,
}

/// A thing on a line or a road, placed between two tiles.
#[derive(Clone, Copy, Debug)]
pub struct Rider {
    /// Where it was at the reading and where it is going, in tiles' middles.
    pub from: (f32, f32),
    pub to: (f32, f32),
    /// How far along from `from` to `to` it is at the reading, 0 to 1, and how
    /// much further along it goes in one second.
    pub at: f32,
    pub per_second: f32,
    pub load: Option<Load>,
    pub dir: Dir,
    pub moving: bool,
}

impl Rider {
    /// Where it is `frac` of the way through the second after the reading, in tiles.
    pub fn place(&self, frac: f32) -> (f32, f32) {
        let t = if self.moving { (self.at + self.per_second * frac).clamp(0.0, 1.0) } else { self.at };
        (self.from.0 + (self.to.0 - self.from.0) * t, self.from.1 + (self.to.1 - self.from.1) * t)
    }
}

/// A laid line, read.
#[derive(Clone, Debug)]
pub struct Laid {
    pub id: u32,
    pub kind: Way,
    pub path: Vec<Tile>,
    pub state: LineState,
    /// The good it mostly carries, for its colour.
    pub load: Option<Load>,
    pub per_min: u32,
    pub riders: Vec<Rider>,
}

/// One of the sim's people, read.
#[derive(Clone, Copy, Debug)]
pub struct Folk {
    pub id: u32,
    pub rider: Rider,
    pub trade: Option<Trade>,
    pub working: bool,
    pub left: bool,
}

/// A plan laid on the ground, read.
#[derive(Clone, Debug)]
pub struct PlanView {
    pub id: u32,
    pub plan: String,
    /// The kit piece that draws it, or "" for a road, a bridge or a plan with no piece.
    pub art: &'static str,
    /// The footprint in tiles, inclusive.
    pub rect: [i16; 4],
    pub priority: u8,
    /// What holds it back, in a few words (upper case), or none: a crew can take it.
    pub wait: Option<String>,
    /// How far a crew has got with raising it, 0 to 1, while one is at it.
    pub raising: Option<f32>,
    /// A crew is walking out to it with the materials.
    pub fetched: bool,
    /// What the Deep took, laid out again (the camp's hands, or a knight's, will set it right).
    pub deep: bool,
}

/// One crew of the camp's hands, read: a labourer carrying and a carpenter raising.
#[derive(Clone, Copy, Debug)]
pub struct CrewView {
    pub labourer: Rider,
    pub carpenter: Rider,
    pub working: bool,
    pub idle: bool,
}

/// What a flow arrow's heat says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heat {
    /// Goods moving.
    Flowing,
    /// Something starves here.
    Starved,
    /// Something backs up here.
    Backed,
}

/// An arrow of flow between two works, its thickness the throughput.
#[derive(Clone, Copy, Debug)]
pub struct Arrow {
    pub from: (f32, f32),
    pub to: (f32, f32),
    /// 1 to 4.
    pub weight: u8,
    pub heat: Heat,
}

/// A mark over a works: starved (red) or backed up (amber).
#[derive(Clone, Debug)]
pub struct Mark {
    pub at: (f32, f32),
    pub heat: Heat,
    pub words: String,
}

/// How near the Keep's stockyard (tiles) a cart bound for it comes to be drawn in the bailey, on the lane to
/// the crane's foot, instead of on the road: the lane is this long (`world3d::works_yard`).
pub const RELAY: f32 = 14.0;

/// Where a cart bound for the Keep's yard is in its delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Loaded and on the road to the yard.
    Hauling,
    /// At the yard waiting for a berth, or for room.
    Waiting,
    /// At a berth, unloading.
    Unloading,
}

/// A cart of the sim bound for the Keep's yard.
#[derive(Clone, Copy, Debug)]
pub struct YardCart {
    /// Its place in [`HillView::carts`], where it is drawn on the road.
    pub index: usize,
    /// Tiles to the yard, as the cart goes (a part of a tile for the one it is on).
    pub dist: f32,
    pub phase: Delivery,
    /// Seconds of its unloading left, and how long a berth keeps a cart in all.
    pub left: u32,
    pub dwell: u32,
    pub load: Option<Load>,
}

/// The Keep's stockyard as the sim has it: what it holds of what the wall is paid in, whether the masons
/// could be paid for another stone, and the carts bringing more. What the crane lifts is this stock.
#[derive(Clone, Debug, Default)]
pub struct KeepYard {
    pub rect: [i16; 4],
    /// Dressed stone (ashlar, quoins, voussoirs), rubble for the wall's core, mortar, poles and joists.
    pub cut: u32,
    pub rough: u32,
    pub mortar: u32,
    pub timber: u32,
    /// The yard can pay for at least one more stone (`Kingdom::site_allowance`).
    pub can_pay: bool,
    pub carts: Vec<YardCart>,
}

impl HillView {
    /// The places in [`HillView::carts`] of the carts bound for the Keep's yard that are near enough to it to be
    /// drawn in the bailey (`RELAY`) and not on the road.
    pub fn relayed(&self) -> impl Iterator<Item = usize> + '_ {
        self.keep_yard.iter().flat_map(|y| y.carts.iter().filter(|c| c.dist <= RELAY).map(|c| c.index))
    }
}

/// The works, read.
#[derive(Debug, Default)]
pub struct HillView {
    /// The Keep's yard, when the sim has staked the Keep.
    pub keep_yard: Option<KeepYard>,
    pub rev: u64,
    pub second: u64,
    pub buildings: Vec<Placed>,
    pub roads: Vec<Way4>,
    pub shafts: Vec<Way4>,
    pub leylines: Vec<Way4>,
    pub lines: Vec<Laid>,
    /// The rollerways, tile by tile, and the heads of their jams (tile and why).
    pub belts: Vec<Belt>,
    pub jams: Vec<(Tile, String)>,
    /// The rail network: its tiles, the wagons on them and the knots where wagons wait (tile and why).
    pub rails: Vec<RailTile>,
    pub wagons: Vec<Hauled>,
    pub rail_jams: Vec<(Tile, String)>,
    pub carts: Vec<Rider>,
    pub folk: Vec<Folk>,
    pub arrows: Vec<Arrow>,
    pub marks: Vec<Mark>,
    /// The party's plans on the ground, and the crews at them.
    pub plans: Vec<PlanView>,
    pub crews: Vec<CrewView>,
    /// The Clerk of Works' sentence for the Keep, upper case, or empty.
    pub clerk: String,
    /// The tiles the works fill, inclusive: `[x0, y0, x1, y1]`.
    pub bounds: [i16; 4],
}

/// The kit piece that draws a plan, if one does. The carter's yard, the
/// castle's stockyard, the founding camp and the shaft heads each have a piece
/// of their own; the Keep and the smelter have none yet.
pub fn art_of(plan: &str) -> &'static str {
    match plan {
        "keep" | "smelter" => "",
        p => kit::plans().find(|k| *k == p).unwrap_or(""),
    }
}

/// Which way the kit draws a kind of line.
pub fn way_of(kind: LineKind) -> Way {
    match kind {
        LineKind::Cart => Way::Cart,
        LineKind::Tram => Way::Tram,
        LineKind::Rope => Way::Rope,
        LineKind::Chute => Way::Chute,
        LineKind::Barge => Way::Barge,
        LineKind::Mule => Way::Mule,
    }
}

fn mid(t: Tile) -> (f32, f32) {
    (f32::from(t.0) + 0.5, f32::from(t.1) + 0.5)
}

fn dir_of(a: (f32, f32), b: (f32, f32)) -> Dir {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 { Dir::East } else { Dir::West }
    } else if dy >= 0.0 {
        Dir::South
    } else {
        Dir::North
    }
}

/// The heaviest good of a stock, as the look of a load.
fn load_of(stock: &crate::kingdom::Stock) -> Option<Load> {
    let c = catalog();
    stock
        .iter()
        .max_by_key(|(r, n)| c.resources.get(*r).map_or(1, |x| u32::from(x.weight.max(1))) * **n)
        .map(|(r, _)| Load::of(r))
}

fn links_of(set: &std::collections::BTreeSet<Tile>, t: Tile, near: impl Fn(Tile) -> bool) -> u8 {
    let mut links = 0;
    for (bit, n) in [(lines::N, (t.0, t.1 - 1)), (lines::E, (t.0 + 1, t.1)), (lines::S, (t.0, t.1 + 1)), (lines::W, (t.0 - 1, t.1))] {
        if set.contains(&n) || near(n) {
            links |= bit;
        }
    }
    links
}

/// The building states from the sim's status.
fn state_of(k: &Kingdom, id: u32, status: &Status, short: &Option<String>) -> State {
    let c = catalog();
    let b = &k.buildings[&id];
    let p = &c.plans[&b.plan];
    match status {
        Status::Construction => {
            let total = f32::from(p.build_seconds.max(1));
            let done = 1.0 - f32::from(b.construction) / total;
            State::Building(((done * f32::from(kit::STEPS)) as u8).min(kit::STEPS - 1))
        }
        Status::Working | Status::Target => State::Working,
        // A swape has nothing to lift, or nowhere to put it: the good it waits on is the one it is set to.
        Status::Inputs if p.swape > 0 => State::Starved(b.filter.as_deref().map_or(Load::Crate, Load::of)),
        Status::Storage if p.swape > 0 => State::BackedUp(b.filter.as_deref().map_or(Load::Crate, Load::of)),
        Status::Inputs => State::Starved(short.as_deref().map_or(Load::Crate, Load::of)),
        Status::Storage => State::BackedUp(short.as_deref().map_or(Load::Crate, Load::of)),
        _ => State::Idle,
    }
}

/// The works as the run has them this second; `None` off the hillside.
pub fn of(run: &Run) -> Option<Arc<HillView>> {
    if !run.sim_hillside() || run.dungeon.depth != 0 {
        return None;
    }
    let k = &run.home.industry;
    // Kept for the second it was read in, and a few runs' worth (a host's run and a
    // seat's view of it read alike).
    type Kept = Mutex<Vec<((u64, u64, usize, usize, usize, u64), Arc<HillView>)>>;
    static KEPT: Kept = Mutex::new(Vec::new());
    let tiles: u64 = k.buildings.values().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b.tile.0 as u64 * 977 + b.tile.1 as u64));
    let key = (k.rev, k.second, k.buildings.len(), k.lines.len(), k.roads.len(), tiles ^ ((k.rollers.cells.len() as u64 + k.rails.cells.len() as u64 * 5 + k.rails.wagons.len() as u64 * 7) << 40));
    if let Ok(kept) = KEPT.lock()
        && let Some((_, v)) = kept.iter().find(|(kk, _)| *kk == key)
    {
        return Some(Arc::clone(v));
    }
    let view = Arc::new(read(run, k));
    if let Ok(mut kept) = KEPT.lock() {
        if kept.len() >= 6 {
            kept.remove(0);
        }
        kept.push((key, Arc::clone(&view)));
    }
    Some(view)
}

fn read(run: &Run, k: &Kingdom) -> HillView {
    let c = catalog();
    let report = k.report(c);
    let machines: BTreeMap<u32, crate::kingdom::MachineRow> = k.machine_report(c).into_iter().map(|m| (m.id, m)).collect();
    let mut v = HillView { rev: k.rev, second: k.second, bounds: [i16::MAX, i16::MAX, i16::MIN, i16::MIN], ..HillView::default() };
    let mut grow = |v: &mut HillView, x: i16, y: i16| {
        v.bounds = [v.bounds[0].min(x), v.bounds[1].min(y), v.bounds[2].max(x), v.bounds[3].max(y)];
    };
    // Buildings.
    for (&id, b) in &k.buildings {
        let p = &c.plans[&b.plan];
        let status = report.status.get(&id).cloned().unwrap_or(Status::Idle);
        let short = machines.get(&id).and_then(|m| match status {
            Status::Inputs => m.input_fill.iter().find(|(_, have, need)| have < need).map(|(g, _, _)| g.clone()),
            Status::Storage => m.output_fill.iter().max_by_key(|(_, have, cap)| (have * 100 / (*cap).max(1), *have)).map(|(g, _, _)| g.clone()),
            _ => None,
        }).or_else(|| (status == Status::Inputs && p.watch > 0).then(|| crate::kingdom::defence::ARROW.to_string()));
        let covered: Vec<Tile> = k.covered(b).collect();
        let x0 = covered.iter().map(|t| t.0).min().unwrap_or(b.tile.0);
        let y0 = covered.iter().map(|t| t.1).min().unwrap_or(b.tile.1);
        let x1 = covered.iter().map(|t| t.0).max().unwrap_or(b.tile.0);
        let y1 = covered.iter().map(|t| t.1).max().unwrap_or(b.tile.1);
        grow(&mut v, x0, y0);
        grow(&mut v, x1, y1);
        let waiting = report.population.waiting.get(&id).copied().unwrap_or(0);
        v.buildings.push(Placed {
            id,
            plan: b.plan.clone(),
            art: art_of(&b.plan),
            rect: [x0, y0, x1, y1],
            state: state_of(k, id, &status, &short),
            crew: report.population.assigned.get(&id).copied().unwrap_or(0),
            waiting,
            paused: b.paused,
            short,
            status: status.clone(),
            facing: b.facing,
            filter: b.filter.clone(),
            hurt: k.defence_of(c, id).filter(|(left, max)| left < max),
        });
        let _ = p;
    }
    // Roads, shafts and leylines, each with what it joins.
    let near_building = |t: Tile| k.at(t).is_some_and(|id| id != 1 || true);
    for &t in &k.roads {
        v.roads.push(Way4 { tile: t, links: links_of(&k.roads, t, |n| near_building(n) && false) });
        grow(&mut v, t.0, t.1);
    }
    for &t in &k.power.shafts {
        v.shafts.push(Way4 { tile: t, links: links_of(&k.power.shafts, t, |n| k.at(n).is_some()) });
    }
    for &t in &k.power.leylines {
        v.leylines.push(Way4 { tile: t, links: links_of(&k.power.leylines, t, |n| k.at(n).is_some()) });
    }
    // Lines and what runs on them.
    let rows: BTreeMap<u32, crate::kingdom::LineRow> = k.line_report(c).into_iter().map(|r| (r.id, r)).collect();
    for (&id, line) in &k.lines {
        let spec = k.line_spec(c, line);
        let load = line.fleet.iter().find(|x| x.amount > 0).map(|x| Load::of(&x.resource));
        let mut riders = Vec::new();
        let n = line.path.len();
        if n >= 2 {
            let last = (n - 1) as f32;
            for veh in &line.fleet {
                let out = !matches!(veh.phase, Phase::Back);
                let cross = if out { line.forward } else { line.back }.max(1) as f32;
                // How far along the whole way, 0 at the origin and 1 at the destination.
                let progress = match veh.phase {
                    Phase::Out => (veh.clock as f32 / cross).clamp(0.0, 1.0),
                    Phase::Back => 1.0 - (veh.clock as f32 / cross).clamp(0.0, 1.0),
                    Phase::Waiting | Phase::Unloading => 1.0,
                    Phase::Idle | Phase::Loading => 0.0,
                };
                let moving = matches!(veh.phase, Phase::Out | Phase::Back);
                let along = progress * last;
                let i0 = (along.floor() as usize).min(n - 2);
                let t = along - i0 as f32;
                let (a, b) = (mid(line.path[i0]), mid(line.path[i0 + 1]));
                // Ten tenths a second over `cross` tenths for the whole way of `last` tiles.
                let per_second = 10.0 * last / cross;
                let (from, to, at) = if out { (a, b, t) } else { (b, a, 1.0 - t) };
                riders.push(Rider {
                    from,
                    to,
                    at,
                    per_second,
                    load: (veh.amount > 0).then(|| Load::of(&veh.resource)),
                    dir: dir_of(from, to),
                    moving,
                });
            }
        }
        let _ = spec;
        v.lines.push(Laid {
            id,
            kind: way_of(line.kind),
            path: line.path.clone(),
            state: rows.get(&id).map_or(LineState::Flowing, |r| r.state),
            load,
            per_min: rows.get(&id).map_or(0, |r| r.observed_per_min),
            riders,
        });
        for t in &line.path {
            grow(&mut v, t.0, t.1);
        }
    }
    // Rollerways: tile by tile, with what rides them.
    {
        let need = k.rollers.chain_tiles();
        let mut hands = usize::from(report.population.assigned.get(&1).copied().unwrap_or(0)).min(need);
        let load = |g: u8| k.rollers.good_name(g).map(Load::of);
        for (&t, cell) in &k.rollers.cells {
            let crewed = cell.piece == Piece::Chain && hands > 0;
            if crewed {
                hands -= 1;
            }
            v.belts.push(Belt {
                tile: t,
                piece: cell.piece,
                dir: cell.dir.index(),
                slots: cell.slots.map(load),
                across: cell.across.map(load),
                moved: cell.moved,
                crewed,
                stuck: cell.stuck,
            });
            grow(&mut v, t.0, t.1);
        }
        for row in k.roller_report(c) {
            // A jam is a line nothing can get on to; a full branch the rest goes round is not one.
            for jam in row.jams.into_iter().filter(|j| j.blocking) {
                v.jams.push((jam.tile, jam.why.to_uppercase()));
            }
        }
    }
    // Rails: the network, the bays and the wagons.
    if !k.rails.is_empty() {
        let held: std::collections::BTreeSet<Tile> = k.rails.wagons.values().filter(|w| w.phase == crate::kingdom::rails::Phase::Dwell).map(|w| w.tile).collect();
        for (&t, r) in &k.rails.cells {
            let mut links = 0;
            for (bit, n) in [(lines::N, (t.0, t.1 - 1)), (lines::E, (t.0 + 1, t.1)), (lines::S, (t.0, t.1 + 1)), (lines::W, (t.0 - 1, t.1))] {
                if k.rails.cells.contains_key(&n) {
                    links |= bit;
                }
            }
            let bay = r.bay.as_ref().and_then(|name| k.rails.bays.get(name).map(|b| RailBay { name: name.clone(), side: b.side.index(), held: held.contains(&t) }));
            let switch = k.rails.is_switch(t);
            v.rails.push(RailTile { tile: t, links, switch, throw: if switch { switch_throw(k, t) } else { None }, bay });
            grow(&mut v, t.0, t.1);
        }
        let spec = &c.lines["tram"];
        let speed = f32::from(spec.speed.max(1));
        for (&id, w) in &k.rails.wagons {
            let here = mid(w.tile);
            let (to, moving) = match w.to {
                Some(n) => (mid(n), true),
                None => (here, false),
            };
            let dir = if moving { dir_of(here, to) } else { match w.face { crate::kingdom::Dir::N => Dir::North, crate::kingdom::Dir::E => Dir::East, crate::kingdom::Dir::S => Dir::South, crate::kingdom::Dir::W => Dir::West } };
            v.wagons.push(Hauled {
                id,
                rider: Rider { from: here, to, at: (f32::from(w.clock) / speed).clamp(0.0, 1.0), per_second: 10.0 / speed, load: (w.amount > 0).then(|| Load::of(&w.good)), dir, moving },
                state: k.wagon_state(c, id),
            });
        }
        for j in k.rail_jams(c) {
            v.rail_jams.push((j.tile, j.why.to_uppercase()));
        }
    }
    // The Keep's yard, and the carts bound for it.
    let yard = run.keep_live().and_then(|site| {
        let y = k.sites.get(&site)?.yard;
        let yb = k.buildings.get(&y)?;
        let covered: Vec<Tile> = k.covered(yb).collect();
        let rect = [
            covered.iter().map(|t| t.0).min()?,
            covered.iter().map(|t| t.1).min()?,
            covered.iter().map(|t| t.0).max()?,
            covered.iter().map(|t| t.1).max()?,
        ];
        let have = |g: &str| yb.input.get(g).copied().unwrap_or(0);
        let allowance = k.site_allowance(c, site, u32::MAX / 200);
        Some((y, KeepYard {
            rect,
            cut: have("ashlar") + have("voussoir") + have("quoin"),
            rough: have("rubble"),
            mortar: have("mortar"),
            timber: have("pole") + have("joist") + have("beam") + have("plank"),
            can_pay: allowance.stones > 0,
            carts: Vec::new(),
        }))
    });
    let (yard_id, mut keep_yard) = match yard {
        Some((id, y)) => (Some(id), Some(y)),
        None => (None, None),
    };
    // Carts on the roads.
    let cart_cost = f32::from(c.lines["cart"].speed.max(1));
    for (id, b) in &k.buildings {
        if c.plans[&b.plan].stalls == 0 {
            continue;
        }
        let _ = id;
        for cart in &b.carts {
            if let (Some(y), Some(yid)) = (keep_yard.as_mut(), yard_id)
                && cart.to == yid
            {
                let phase = match cart.phase {
                    CartPhase::Haul => Some(Delivery::Hauling),
                    CartPhase::Wait => Some(Delivery::Waiting),
                    CartPhase::Unloading => Some(Delivery::Unloading),
                    _ => None,
                };
                if let Some(phase) = phase {
                    let apart = |t: Tile| {
                        let dx = (y.rect[0] - t.0).max(t.0 - y.rect[2]).max(0);
                        let dy = (y.rect[1] - t.1).max(t.1 - y.rect[3]).max(0);
                        f32::from(dx + dy)
                    };
                    let progress = if matches!(phase, Delivery::Hauling) { (cart.clock as f32 / cart_cost).clamp(0.0, 1.0) } else { 0.0 };
                    y.carts.push(YardCart {
                        index: v.carts.len(),
                        dist: (apart(cart.tile) - progress).max(0.0),
                        phase,
                        left: cart.clock,
                        dwell: u32::from(c.lines["cart"].dwell),
                        load: load_of(&cart.load),
                    });
                }
            }
            let here = mid(cart.tile);
            let (to, moving) = match cart.route.last() {
                Some(&next) if matches!(cart.phase, CartPhase::Fetch | CartPhase::Haul | CartPhase::Return) => (mid(next), true),
                _ => (here, false),
            };
            v.carts.push(Rider {
                from: here,
                to,
                at: (cart.clock as f32 / cart_cost).clamp(0.0, 1.0),
                per_second: 10.0 / cart_cost,
                load: load_of(&cart.load),
                dir: dir_of(here, to),
                moving,
            });
        }
    }
    v.keep_yard = keep_yard;
    // People, at their posts, walking between them.
    for (&id, w) in &k.motion.actors {
        let citizen = k.population.citizens.get(&id);
        let trade = citizen
            .and_then(|p| p.job.or(p.waiting))
            .and_then(|job| k.buildings.get(&job))
            .and_then(|b| Trade::from_id(&c.plans[&b.plan].trade));
        let (from, to) = (mid(w.previous_tile), mid(w.tile));
        v.folk.push(Folk {
            id,
            rider: Rider { from, to, at: 0.0, per_second: 1.0, load: None, dir: dir_of(from, to), moving: w.walking && from != to },
            trade,
            working: w.working,
            left: w.facing_left,
        });
    }
    // The party's plans, and the hands at them.
    read_plans(k, &mut v);
    // The overlay: arrows where goods moved lately, marks where something starves or backs up.
    let mut top: Vec<((u32, u32), u32)> = k.roadwork.flow.iter().map(|(p, kg)| (*p, *kg)).collect();
    top.sort_by_key(|(_, kg)| std::cmp::Reverse(*kg));
    let most = top.first().map_or(1, |(_, kg)| (*kg).max(1));
    let centre = |id: u32| {
        k.buildings.get(&id).map(|b| {
            let ts: Vec<Tile> = k.covered(b).collect();
            let (x0, y0) = (ts.iter().map(|t| t.0).min().unwrap_or(b.tile.0), ts.iter().map(|t| t.1).min().unwrap_or(b.tile.1));
            let (x1, y1) = (ts.iter().map(|t| t.0).max().unwrap_or(b.tile.0), ts.iter().map(|t| t.1).max().unwrap_or(b.tile.1));
            ((f32::from(x0) + f32::from(x1) + 1.0) / 2.0, (f32::from(y0) + f32::from(y1) + 1.0) / 2.0)
        })
    };
    for ((a, b), kg) in top.into_iter().take(48) {
        if let (Some(from), Some(to)) = (centre(a), centre(b)) {
            let weight = 1 + (3 * kg / most).min(3) as u8;
            v.arrows.push(Arrow { from, to, weight, heat: Heat::Flowing });
        }
    }
    for line in &v.lines {
        let (Some(first), Some(last)) = (line.path.first(), line.path.last()) else { continue };
        let heat = match line.state {
            LineState::Starved => Heat::Starved,
            LineState::Backed | LineState::Cut | LineState::Unfed => Heat::Backed,
            // A line that is the limit is thick with flow; the marks over the works say what it starves and what backs up.
            LineState::Saturated | LineState::Flowing => Heat::Flowing,
        };
        v.arrows.push(Arrow { from: mid(*first), to: mid(*last), weight: 1 + (line.per_min / 12).min(3) as u8, heat });
    }
    let name = |b: &Placed| c.plans[&b.plan].name.to_uppercase();
    for (t, why) in &v.jams {
        v.marks.push(Mark { at: mid(*t), heat: Heat::Backed, words: format!("JAMMED: {why}") });
    }
    for (t, why) in &v.rail_jams {
        v.marks.push(Mark { at: mid(*t), heat: Heat::Backed, words: format!("JAMMED: {why}") });
    }
    for b in &v.buildings {
        let at = ((f32::from(b.rect[0]) + f32::from(b.rect[2]) + 1.0) / 2.0, f32::from(b.rect[1]));
        let why = |good: &Option<String>| good.as_deref().map_or(String::new(), |g| c.resources.get(g).map_or(g.to_string(), |x| x.name.to_uppercase()));
        if b.plan == "swape" {
            // A swape says what it lifts and what stops it, in its own words.
            if matches!(b.status, Status::Inputs | Status::Storage | Status::Workers) {
                let heat = if b.status == Status::Storage { Heat::Backed } else { Heat::Starved };
                let words = if b.status == Status::Workers { "SWAPE: NEEDS A BOY".to_string() } else { format!("SWAPE {}", k.swape_words(c, b.id).to_uppercase()) };
                v.marks.push(Mark { at, heat, words });
            }
            continue;
        }
        match &b.status {
            Status::Inputs => v.marks.push(Mark { at, heat: Heat::Starved, words: format!("{} NEEDS {}", name(b), why(&b.short)) }),
            Status::Storage => v.marks.push(Mark { at, heat: Heat::Backed, words: format!("{} BACKED UP WITH {}", name(b), why(&b.short)) }),
            Status::Fire => v.marks.push(Mark { at, heat: Heat::Backed, words: format!("{} IS ON FIRE: A WELL OR A KNIGHT'S HANDS", name(b)) }),
            Status::Workers | Status::Disconnected | Status::Tools | Status::Fuel | Status::Fodder | Status::Shaft => {
                v.marks.push(Mark { at, heat: Heat::Starved, words: format!("{}: {}", name(b), b.status.label()) })
            }
            _ => {}
        }
    }
    v.clerk = run.keep_clerk().unwrap_or_default();
    if v.bounds[0] == i16::MAX {
        v.bounds = [0, 0, 0, 0];
    }
    let _ = (run, HZ);
    v
}

/// The way switch `t` is thrown (0 north, 1 east, 2 south, 3 west): the way the nearest wagon within two
/// tiles of it goes on from it, by the way it means to go (the shortest way to its stop's bay, as the
/// sim sends it); none when no wagon is near. The sim keeps no lever, the wagons set the switch as they
/// go, so this is what a lever would be at.
fn switch_throw(k: &Kingdom, t: Tile) -> Option<u8> {
    let apart = |a: Tile, b: Tile| i32::from(a.0 - b.0).abs() + i32::from(a.1 - b.1).abs();
    let (mut best, mut out): (Option<(i32, u32)>, Option<u8>) = (None, None);
    for (&id, w) in &k.rails.wagons {
        let d = apart(w.tile, t).min(w.to.map_or(i32::MAX, |n| apart(n, t)));
        if d > 2 || best.is_some_and(|b| b <= (d, id)) {
            continue;
        }
        let bay = k
            .rails
            .routes
            .get(&w.route)
            .and_then(|r| r.stops.get(usize::from(w.stop) % r.stops.len().max(1)))
            .and_then(|s| k.rails.bays.get(&s.bay));
        let next = match (w.tile == t, w.to) {
            (true, Some(to)) => Some(to),
            _ => bay.and_then(|b| k.rails.path(t, b.tile, &Default::default())).and_then(|p| p.first().copied()),
        };
        if let Some(n) = next {
            let way = match (n.0 - t.0, n.1 - t.1) {
                (0, -1) => 0,
                (1, 0) => 1,
                (0, 1) => 2,
                (-1, 0) => 3,
                _ => continue,
            };
            best = Some((d, id));
            out = Some(way);
        }
    }
    out
}

/// The sim's second inside the tick: 0 to 1, for placing what moves.
pub fn frac(run: &Run) -> f32 {
    (run.tick % u64::from(HZ)) as f32 / HZ as f32
}

/// The animation step of a working piece at this tick (eight a second).
pub fn frame(run: &Run) -> u32 {
    (run.tick / u64::from((HZ / 8).max(1))) as u32
}

/// The tile `p` (0 to 1) of the way along `way`, in tiles' middles.
fn along(way: &[Tile], p: f32) -> (f32, f32) {
    if way.len() < 2 {
        return way.first().map_or((0.0, 0.0), |t| mid(*t));
    }
    let at = p.clamp(0.0, 1.0) * (way.len() - 1) as f32;
    let i = (at.floor() as usize).min(way.len() - 2);
    let t = at - i as f32;
    let (a, b) = (mid(way[i]), mid(way[i + 1]));
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn read_plans(k: &Kingdom, v: &mut HillView) {
    use crate::kingdom::Leg;
    let c = catalog();
    for (&id, g) in &k.plans.ghosts {
        let tiles = k.ghost_tiles(c, g);
        let (x0, y0) = (tiles.iter().map(|t| t.0).min().unwrap_or(g.tile.0), tiles.iter().map(|t| t.1).min().unwrap_or(g.tile.1));
        let (x1, y1) = (tiles.iter().map(|t| t.0).max().unwrap_or(g.tile.0), tiles.iter().map(|t| t.1).max().unwrap_or(g.tile.1));
        let crew = k.plans.crews.iter().find(|w| w.ghost == id);
        v.plans.push(PlanView {
            id,
            plan: g.plan.clone(),
            art: art_of(&g.plan),
            rect: [x0, y0, x1, y1],
            priority: g.priority,
            wait: k.ghost_wait(c, id).map(|w| w.to_uppercase()),
            raising: crew.filter(|w| w.leg == Leg::Raising).map(|w| w.along()),
            fetched: crew.is_some_and(|w| w.leg == Leg::Out),
            deep: g.by == 0,
        });
    }
    if k.plans.ghosts.is_empty() {
        return;
    }
    let camp = k.camp();
    for w in &k.plans.crews {
        let way = if w.leg == Leg::Idle { vec![camp] } else { k.crew_way(w) };
        let steps = way.len().saturating_sub(1).max(1) as f32;
        // Where it is, and where a second on will have it; the carpenter a stride behind the labourer.
        let (p, per) = match w.leg {
            Leg::Out => (w.along(), 1.0 / f32::from(w.total.max(1))),
            Leg::Back => (1.0 - w.along(), -1.0 / f32::from(w.total.max(1))),
            Leg::Raising => (1.0, 0.0),
            Leg::Idle => (0.0, 0.0),
        };
        let rider = |p: f32, load: Option<Load>| {
            let (a, b) = (along(&way, p), along(&way, (p + per).clamp(0.0, 1.0)));
            Rider { from: a, to: b, at: 0.0, per_second: 1.0, load, dir: dir_of(a, b), moving: per != 0.0 && a != b }
        };
        let load = (w.leg == Leg::Out).then(|| load_of(&w.carrying)).flatten();
        let behind = (0.7 / steps).copysign(per);
        let lab = rider(p, load);
        let carp = rider((p - behind).clamp(0.0, 1.0), None);
        // Raising: the pair stand at the plan, one on each side of it.
        let (lab, carp) = if w.leg == Leg::Raising {
            let t = mid(w.tile);
            let mk = |dx: f32| Rider { from: (t.0 + dx, t.1 + 0.5), to: (t.0 + dx, t.1 + 0.5), at: 0.0, per_second: 0.0, load: None, dir: Dir::South, moving: false };
            (mk(-0.35), mk(0.35))
        } else {
            (lab, carp)
        };
        v.crews.push(CrewView { labourer: lab, carpenter: carp, working: w.leg == Leg::Raising, idle: w.leg == Leg::Idle });
    }
}
