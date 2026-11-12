//! The Deep comes up (docs/KINGDOM-WORKS.md, "The Deep comes up"; ECONOMY.md,
//! "Pressure"). The sim holds the damage and the repair (`kingdom::defence`); this is
//! the raid: the realm's day and night, the night the Deep answers, the things that
//! climb the Great Shaft, where they go and what they do when they get there.
//!
//! - **When.** The realm's day is [`DAY`] seconds and its night the last of it. At dusk,
//!   with a knight on the hillside, the Deep is judged once: the stir its smoke, noise
//!   and digging make at the works, less the calm of the castles that stand there
//!   (`Kingdom::pressure_with`). From *uneasy* up it sends a raid, one a night.
//! - **Who.** Delve foes, the ordinary kinds, and a raid has a theme that teaches one
//!   thing: the first is two imps and fire; then thieves, sappers, boars, round again.
//!   The size is the level (2, 4, 6) and never more than [`RAID_MAX`], and the first
//!   raid is two whatever the level. The raiders come up the shaft one by one.
//! - **Where.** Each heads for the nearest smoke: a working smoky works or a lit brazier
//!   (else any working works; boars go for the Keep's wall). They walk a field of cheapest
//!   ways (`Field`): floor costs a step, a palisade or a wall costs what it will take to
//!   break, so they go round when going round is cheaper and break through when it is
//!   not, and a gap or a gate is a way in. A knight near them (six tiles) is fought as
//!   any foe is; otherwise they march.
//! - **What.** Imps set the works alight; thieves rob a store and run for the shaft
//!   (killed, they drop it); sappers cut the lines (road, rollerway, rail); boars wreck
//!   carts and batter the Keep's lift in hand: stones of an unfinished wall, true ones
//!   held longer, and what rested on them comes down (`world::works_siege`). Anything in
//!   their way that is a defence is battered. Every loss is laid out again as a plan for
//!   the camp's hands (`kingdom::defence`).
//! - **Against.** Watchtowers loose the arrows the roads bring them; walls and palisades
//!   hold; a well puts a fire out; knights mend with a hand on the work (the builder's
//!   MEND tool) and fight the raiders as they fight anything.
//! - **Bounds.** One raid a night, at most [`RAID_MAX`] raiders, none lives past
//!   [`LAST`] seconds or dawn, a raid is dropped when the party leaves the hillside, the
//!   camp, the shaft and a castle's yard are never struck, repair goods are never all
//!   taken, a lift's stones are knocked out only up to [`STONE_BUDGET`] a raid, and a
//!   party beaten on the hillside is knocked down, not killed.
//!
//! No randomness: everything is the tick, the sim's second and the state.
use super::*;
use crate::kingdom::{Blow, Stock, catalog};
use crate::world::works::{self, PX_PER_M};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::sync::Arc;

type KTile = crate::kingdom::Tile;

/// The realm's day, in sim seconds, and when its night falls.
pub const DAY: u64 = 600;
pub const DUSK: u64 = 400;
/// Seconds after dusk before the Deep is judged and the first raider climbs.
pub const JUDGE_AFTER: u64 = 6;
/// Ticks between raiders up the shaft.
pub const COME_EVERY: u64 = 3 * HZ as u64;
/// The most raiders in a raid.
pub const RAID_MAX: u8 = 8;
/// No raid outlasts this many seconds.
pub const LAST: u64 = 170;
/// A raider this near (units) a knight fights it; else it marches.
pub const AGGRO: f32 = 12.0;
/// How fast a raider marches, units a second.
pub const PACE: f32 = 4.0;
/// What a blow of each kind does to a defence, hit points a second.
pub const BLOW: u16 = 2;
pub const BOAR_BLOW: u16 = 8;
pub const SAPPER_BLOW: u16 = 3;
/// How far a tower's arrow hurts, and how often a tower looses (seconds).
pub const ARROW_DAMAGE: u32 = 14;
pub const ARROW_EVERY: u64 = 2;
/// Goods a thief carries off at a time.
pub const THIEF_TAKES: u32 = 4;
/// The lift in hand can be reached to this height, metres (a raider climbs the scaffold).
pub const SIEGE_Z: f32 = 4.8;
/// How far from a stone's face a raider reaches, tiles.
pub const SIEGE_REACH: f32 = 3.0;
/// Blows to knock a plain stone out of an unfinished wall; a true one takes more.
pub const STONE_BLOWS: u8 = 2;
pub const TRUE_BLOWS: u8 = 5;
/// The most stones a raid brings down (counting what rested on them), the most that are ever out at once, and the first raid's.
pub const STONE_BUDGET: u8 = 24;
pub const FIRST_STONES: u8 = 6;
/// What beating a raid takes off the Deep's stir below.
pub const RELIEF: u16 = 10;
/// A thief fleeing is away when it is this near the shaft's mouth, tiles.
const AWAY: i32 = 2;
/// The Keep, as a raider's goal.
const KEEP: u32 = 0;
/// A fleeing thief's goal, and the goal of a raider that has none yet.
const FLEE: u32 = u32::MAX;
const NONE: u32 = u32::MAX - 1;
/// A field of cheapest ways costs this many ticks before it is made again.
const FIELD_TICKS: u64 = 45;
const INF: u32 = u32::MAX;
/// What goes in a raid's books.
const MOST: usize = 16;

/// What a raid teaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Fire,
    Thieves,
    Sappers,
    Boars,
}

impl Theme {
    pub const ALL: [Theme; 4] = [Theme::Fire, Theme::Thieves, Theme::Sappers, Theme::Boars];
    pub fn of(raids: u16) -> Theme {
        Self::ALL[usize::from(raids) % 4]
    }
    pub fn kind(self) -> EnemyKind {
        match self {
            Theme::Fire => EnemyKind::Imp,
            Theme::Thieves => EnemyKind::Goblin,
            Theme::Sappers => EnemyKind::Sapper,
            Theme::Boars => EnemyKind::Warboar,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Theme::Fire => "IMPS WITH TORCHES",
            Theme::Thieves => "THIEVES",
            Theme::Sappers => "SAPPERS",
            Theme::Boars => "WARBOARS",
        }
    }
    fn blow(self) -> u16 {
        match self {
            Theme::Boars => BOAR_BLOW,
            Theme::Sappers => SAPPER_BLOW,
            _ => BLOW,
        }
    }
}

/// A raid in hand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Raid {
    pub theme: Theme,
    /// How many are to come up, how many have.
    pub size: u8,
    pub came: u8,
    /// The sim second it began, the room it is in and the tick the hillside last saw it.
    pub began: u64,
    pub room: usize,
    pub seen: u64,
    /// The works the Deep's stir reaches, nearest the shaft first: what the raiders come for. A works
    /// inside a castle's calm is not among them.
    pub targets: Vec<u32>,
    /// The raiders' ids.
    pub band: Vec<u32>,
    /// Each raider's goal: a building's id, [`KEEP`], or [`FLEE`].
    pub goals: BTreeMap<u32, u32>,
    /// What each thief carries.
    pub loot: BTreeMap<u32, Stock>,
    /// Stones brought down so far, and the blows each stone has had.
    pub stones: u8,
    pub blows: BTreeMap<u32, u8>,
}

/// What the Deep has done over the whole game, and the raid in hand.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Watch {
    /// Raids sent, and beaten.
    pub raids: u16,
    pub beaten: u16,
    /// The night last judged (its day, plus one).
    pub judged: u64,
    pub on: Option<Raid>,
    #[serde(skip)]
    pub mind: Mind,
}

/// What the director keeps between ticks: not saved, not sent, never compared.
#[derive(Clone, Debug, Default)]
pub struct Mind {
    fields: BTreeMap<u32, (u64, Arc<Field>)>,
    /// The defence in the way of each raider this tick.
    blocked: BTreeMap<u32, u32>,
    /// The sim second the books were last read.
    stamp: u64,
}

impl PartialEq for Mind {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Mind {}

impl Watch {
    pub fn valid(&self) -> bool {
        self.on.as_ref().is_none_or(|r| {
            r.size <= RAID_MAX
                && r.came <= r.size
                && r.band.len() <= MOST
                && r.targets.len() <= MOST
                && r.goals.len() <= MOST
                && r.loot.len() <= MOST
                && r.loot.values().all(|s| s.len() <= MOST && s.values().all(|&n| n <= 64))
                && r.stones <= STONE_BUDGET
                && r.blows.len() <= 4 * usize::from(STONE_BUDGET)
                && r.blows.values().all(|&n| n < TRUE_BLOWS)
        })
    }
}

/// How many raiders the Deep sends at a level of stir, the `raids`th time: none while it sleeps; two,
/// four or six by level; the first raid two whatever the level; a raider more every third raid;
/// never more than [`RAID_MAX`].
pub fn raid_size(level: crate::kingdom::Stir, raids: u16) -> Option<u8> {
    use crate::kingdom::Stir;
    let base: u16 = match level {
        Stir::Calm => return None,
        Stir::Uneasy => 2,
        Stir::Stirring => 4,
        Stir::Raiding => 6,
    };
    Some(if raids == 0 { 2 } else { (base + raids / 3).min(u16::from(RAID_MAX)) as u8 })
}

/// One tick of a raider's march toward `goal` (a point, units): a step along it, slid along what is in
/// the way; a body wedged against stone is walked to the middle of its tile first, which always fits.
pub(super) fn march(enemy: &mut Enemy, goal: (f32, f32), grid: &Grid) {
    let pace = PACE * DT;
    let from = (enemy.x, enemy.y);
    let (ux, uy) = unit(goal.0 - enemy.x, goal.1 - enemy.y);
    (enemy.x, enemy.y) = grid.slide(from, (ux * pace, uy * pace), enemy.radius(), Mover::Walker);
    if (enemy.x, enemy.y) == from {
        let middle = centre_of(tile_of(enemy.x, enemy.y));
        let (mx, my) = unit(middle.0 - enemy.x, middle.1 - enemy.y);
        if (middle.0 - enemy.x).hypot(middle.1 - enemy.y) > pace {
            (enemy.x, enemy.y) = grid.slide(from, (mx * pace, my * pace), enemy.radius(), Mover::Walker);
        }
    }
    enemy.dir = (ux, uy);
}

/// How the realm's day stands: whether it is night, and the seconds till it changes.
pub fn clock(second: u64) -> (bool, u64) {
    let into = second % DAY;
    if into >= DUSK { (true, DAY - into) } else { (false, DUSK - into) }
}

// ------------------------------------------------------------- the field ----

/// The cheapest way to a goal from every tile of the room, in tenths of a step.
#[derive(Debug)]
pub struct Field {
    w: i32,
    h: i32,
    /// From each tile, the cost of the way to the goal (not counting the tile's own entering).
    cost: Vec<u32>,
    /// What entering each tile costs, [`INF`] where a raider cannot.
    entry: Vec<u32>,
}

impl Field {
    fn at(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h { INF } else { self.cost[(y * self.w + x) as usize] }
    }

    fn entry(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h { INF } else { self.entry[(y * self.w + x) as usize] }
    }
}

/// A defence in the way: the building and what it costs to break through, tenths of a step.
type Defences = BTreeMap<(i32, i32), (u32, u32)>;

/// What stepping into a tile costs a raider, if it may: floor a step, bog two, a defence what
/// it will take to break it.
fn enter(room: &Room, defences: &Defences, x: i32, y: i32) -> Option<u32> {
    if let Some(&(_, cost)) = defences.get(&(x, y)) {
        return Some(cost);
    }
    match room.tile(x, y) {
        layout::Tile::Floor | layout::Tile::Stairs => Some(10),
        layout::Tile::Bog => Some(20),
        _ => None,
    }
}

fn make_field(room: &Room, defences: &Defences, sources: &[(i32, i32)]) -> Field {
    let (w, h) = (room.cols as i32, room.rows as i32);
    let mut cost = vec![INF; (w * h) as usize];
    let mut heap = BinaryHeap::new();
    for &(x, y) in sources {
        if x >= 0 && y >= 0 && x < w && y < h {
            cost[(y * w + x) as usize] = 0;
            heap.push(Reverse((0u32, (y * w + x) as usize)));
        }
    }
    while let Some(Reverse((d, i))) = heap.pop() {
        if d > cost[i] {
            continue;
        }
        let (x, y) = ((i as i32) % w, (i as i32) / w);
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            // The tile a raider steps from must be one it can stand on (or a defence it breaks).
            if enter(room, defences, nx, ny).is_none() {
                continue;
            }
            let diagonal = dx != 0 && dy != 0;
            // The way is entered through (x, y): no cutting a corner of stone.
            if diagonal && (enter(room, defences, x + dx, y).is_none() && !is_source(sources, x + dx, y) || enter(room, defences, x, y + dy).is_none() && !is_source(sources, x, y + dy)) {
                continue;
            }
            let into = enter(room, defences, x, y).unwrap_or(10);
            let step = into * if diagonal { 14 } else { 10 } / 10;
            let nd = d.saturating_add(step);
            let ni = (ny * w + nx) as usize;
            if nd < cost[ni] {
                cost[ni] = nd;
                heap.push(Reverse((nd, ni)));
            }
        }
    }
    let entry = (0..w * h).map(|i| enter(room, defences, i % w, i / w).unwrap_or(INF)).collect();
    Field { w, h, cost, entry }
}

fn is_source(sources: &[(i32, i32)], x: i32, y: i32) -> bool {
    sources.contains(&(x, y))
}

/// The tile a raider at `(x, y)` steps to next on `field`: the neighbour that costs least to enter and
/// go on from, if the way on is cheaper than here. (A defence's own entering is its cost: a wall is
/// stepped into only when that is the cheapest way.)
pub(super) fn step_of(field: &Field, x: i32, y: i32) -> Option<(i32, i32)> {
    let here = field.at(x, y);
    let mut best: Option<(u32, (i32, i32))> = None;
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)] {
        let (nx, ny) = (x + dx, y + dy);
        let (enter, ahead) = (field.entry(nx, ny), field.at(nx, ny));
        if enter == INF || ahead >= here {
            continue;
        }
        // No cutting the corner of stone.
        if dx != 0 && dy != 0 && (field.entry(x + dx, y) == INF || field.entry(x, y + dy) == INF) {
            continue;
        }
        let total = enter.saturating_mul(if dx != 0 && dy != 0 { 14 } else { 10 }) / 10 + ahead;
        if best.is_none_or(|(b, _)| total < b) {
            best = Some((total, (nx, ny)));
        }
    }
    best.map(|(_, at)| at)
}

fn tile_of(x: f32, y: f32) -> (i32, i32) {
    ((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32)
}

fn centre_of((x, y): (i32, i32)) -> (f32, f32) {
    ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS)
}

// ----------------------------------------------------------------- the run ----

impl Run {
    /// Whether the realm's night is on, and the seconds until it turns.
    pub fn deep_clock(&self) -> (bool, u64) {
        clock(self.home.industry.second)
    }

    /// The raid in hand and how many of its raiders are standing.
    pub fn raid_status(&self) -> Option<(Theme, usize, u8)> {
        let raid = self.kingdom.stir.watch.on.as_ref()?;
        let alive = self.enemies.iter().filter(|e| e.hp > 0 && raid.band.contains(&e.id)).count();
        Some((raid.theme, alive, raid.size))
    }

    /// Whether foe `id` is one of a raid's.
    pub fn is_raider(&self, id: u32) -> bool {
        self.kingdom.stir.watch.on.as_ref().is_some_and(|r| r.band.contains(&id))
    }

    /// Whether `e` is a raider with no knight near it: it marches for the works and does nothing else.
    pub(super) fn raider_marches(&self, e: &Enemy) -> bool {
        self.is_raider(e.id)
            && !self.players.values().any(|h| h.targetable() && (h.x - e.x).hypot(h.y - e.y) <= AGGRO)
    }

    /// Once a tick on the hillside.
    pub(super) fn tick_raids(&mut self) {
        if !self.hill_world() || self.home.industry.is_empty() {
            return;
        }
        // The party left the hillside and came back: the raid is not here any more.
        let tick = self.tick;
        if let Some(raid) = self.kingdom.stir.watch.on.as_mut() {
            let away = raid.room != self.at || tick > raid.seen + 3 * u64::from(HZ);
            raid.seen = tick;
            if away {
                self.end_raid(None);
            }
        }
        let second = self.home.industry.second;
        if self.kingdom.stir.watch.on.is_none() {
            if self.tick.is_multiple_of(u64::from(HZ)) {
                self.judge(second);
            }
            return;
        }
        // Raiders come up the shaft.
        if self.tick.is_multiple_of(COME_EVERY) {
            self.come_up();
        }
        if !self.tick.is_multiple_of(u64::from(HZ)) {
            return;
        }
        self.raid_second(second);
    }

    /// At dusk, once: does the Deep answer?
    fn judge(&mut self, second: u64) {
        let (day, into) = (second / DAY, second % DAY);
        if into < DUSK + JUDGE_AFTER || self.kingdom.stir.watch.judged == day + 1 {
            return;
        }
        // Nobody to raid: the night is not judged.
        if !self.players.values().any(|h| !h.absent && h.hp > 0 && !h.stone) {
            return;
        }
        self.kingdom.stir.watch.judged = day + 1;
        let c = catalog();
        let k = &self.home.industry;
        let report = k.report(c);
        let lures = k.lures(c, &report);
        // The stir the works make, and at each works what is left of it after the calm that reaches it.
        let deep = u32::from(self.kingdom.stir.level) / 2;
        let base = k.pressure_with(c, deep, None);
        let raw = base.smoke + base.noise + base.digging;
        let mut exposed: Vec<(i32, u32, u32)> = lures
            .iter()
            .map(|l| (self.lure_distance(l.tile), l.id, raw.saturating_sub(k.calm_at(c, l.tile))))
            .filter(|&(_, _, stir)| stir >= crate::kingdom::pressure::UNEASY)
            .collect();
        exposed.sort();
        let Some(top) = exposed.iter().map(|&(_, _, stir)| stir).max() else { return };
        let raids = self.kingdom.stir.watch.raids;
        let Some(size) = raid_size(crate::kingdom::pressure::level_of(top), raids) else { return };
        let targets: Vec<u32> = exposed.iter().take(MOST).map(|&(_, id, _)| id).collect();
        let theme = Theme::of(raids);
        self.kingdom.stir.watch.raids = raids.saturating_add(1);
        self.kingdom.stir.watch.on = Some(Raid {
            theme,
            size,
            came: 0,
            began: second,
            room: self.at,
            seen: self.tick,
            targets,
            band: Vec::new(),
            goals: BTreeMap::new(),
            loot: BTreeMap::new(),
            stones: 0,
            blows: BTreeMap::new(),
        });
        self.sounds.push("door_seal");
        self.found = Some((self.tick, 0, format!("THE DEEP ANSWERS: {} CLIMB THE GREAT SHAFT", theme.name())));
        self.notices.push(format!(
            "NIGHT RAID: {size} {} are coming up the Great Shaft for the nearest smoke. The stir at the works is {}.",
            theme.name().to_lowercase(),
            top
        ));
    }

    /// How far a place is from the Great Shaft's mouth, tiles squared.
    fn lure_distance(&self, at: KTile) -> i32 {
        let (sx, sy) = crate::world::wishes::delve_stand();
        (i32::from(at.0) - sx).pow(2) + (i32::from(at.1) - sy).pow(2)
    }

    /// The next raider comes up the shaft.
    fn come_up(&mut self) {
        let Some(raid) = &self.kingdom.stir.watch.on else { return };
        if raid.came >= raid.size || raid.band.len() >= MOST {
            return;
        }
        let (kind, n) = (raid.theme.kind(), raid.came);
        let (sx, sy) = crate::world::wishes::delve_stand();
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: false };
        let (mut x, mut y) = centre_of((sx, sy));
        'find: for ring in 0..4i32 {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs().max(dy.abs()) != ring {
                        continue;
                    }
                    let (px, py) = centre_of((sx + dx, sy + dy));
                    if grid.clear(px, py, kind.radius() + 0.2, Mover::Walker) {
                        (x, y) = (px, py);
                        break 'find;
                    }
                }
            }
        }
        let Some(id) = self.spawn_body(kind, x, y) else { return };
        // It knows where it is going the moment it is up.
        let theme = self.kingdom.stir.watch.on.as_ref().map_or(Theme::Fire, |r| r.theme);
        let report = self.home.industry.report(catalog());
        let goal = self.pick_goal(theme, (sx, sy), &report, usize::from(n));
        if let Some(raid) = self.kingdom.stir.watch.on.as_mut() {
            raid.came = n + 1;
            raid.band.push(id);
            raid.goals.insert(id, goal);
        }
        self.sounds.push("door_seal");
    }

    /// The raiders' goals this tick, for the foes' turn: where each marches while no knight is
    /// near. Rebuilds the cheapest-way fields as they go stale.
    pub(super) fn raid_goals(&mut self) -> BTreeMap<u32, Option<(f32, f32)>> {
        let mut out = BTreeMap::new();
        let Some(raid) = &self.kingdom.stir.watch.on else { return out };
        if self.room().kind != RoomKind::Kingdom {
            return out;
        }
        self.kingdom.stir.watch.mind.blocked.clear();
        let band: Vec<(u32, f32, f32)> =
            self.enemies.iter().filter(|e| e.hp > 0 && raid.band.contains(&e.id)).map(|e| (e.id, e.x, e.y)).collect();
        let goals = raid.goals.clone();
        let mut blocked = BTreeMap::new();
        for (id, x, y) in band {
            let Some(&goal) = goals.get(&id).filter(|g| **g != NONE) else {
                // No goal yet: it waits where it came up.
                out.insert(id, None);
                continue;
            };
            let (tx, ty) = tile_of(x, y);
            let Some(field) = self.field_for(goal) else {
                out.insert(id, Some(centre_of((tx, ty))));
                continue;
            };
            let Some((nx, ny)) = step_of(&field, tx, ty) else {
                // Nowhere cheaper than here: it stands at the work.
                out.insert(id, Some(centre_of((tx, ty))));
                continue;
            };
            out.insert(id, Some(centre_of((nx, ny))));
            if let Some(did) = self.defence_at((nx, ny)) {
                blocked.insert(id, did);
            }
        }
        self.kingdom.stir.watch.mind.blocked = blocked;
        out
    }

    /// The building of a defence that stands on a room tile.
    fn defence_at(&self, (x, y): (i32, i32)) -> Option<u32> {
        let c = catalog();
        let k = &self.home.industry;
        let tile = (x as i16, y as i16);
        let id = k.at(tile)?;
        k.defence_of(c, id).map(|_| id)
    }

    /// The field of cheapest ways to goal `goal` (a building, the Keep or the shaft's mouth).
    fn field_for(&mut self, goal: u32) -> Option<Arc<Field>> {
        if let Some((at, f)) = self.kingdom.stir.watch.mind.fields.get(&goal)
            && self.tick.saturating_sub(*at) < FIELD_TICKS
        {
            return Some(Arc::clone(f));
        }
        let c = catalog();
        let k = &self.home.industry;
        let room = self.room();
        let mut defences = Defences::new();
        for (&id, b) in &k.buildings {
            let Some((left, _)) = k.defence_of(c, id) else { continue };
            for t in k.covered(b) {
                defences.insert((i32::from(t.0), i32::from(t.1)), (id, 10 + u32::from(left) * 2));
            }
        }
        let sources: Vec<(i32, i32)> = if goal == FLEE {
            let (sx, sy) = crate::world::wishes::delve_stand();
            vec![(sx, sy)]
        } else if goal == KEEP {
            self.keep_faces()
        } else {
            let b = k.buildings.get(&goal)?;
            k.covered(b).map(|t| (i32::from(t.0), i32::from(t.1))).collect()
        };
        if sources.is_empty() {
            return None;
        }
        let field = Arc::new(make_field(room, &defences, &sources));
        self.kingdom.stir.watch.mind.fields.insert(goal, (self.tick, Arc::clone(&field)));
        Some(field)
    }

    /// The open tiles that stand against the Keep's walls: where a siege is laid from.
    fn keep_faces(&self) -> Vec<(i32, i32)> {
        let Some(site) = self.sites.keep() else { return Vec::new() };
        let Some(castle) = site.castle() else { return Vec::new() };
        let (x0, y0, w, h) = castle.tiles();
        let room = self.room();
        let wall = |x: i32, y: i32| castle.blocked_without(x, y, site.laid, &site.lost) && !castle.is_ditch(x, y);
        let mut out = Vec::new();
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                if wall(x, y) || !matches!(room.tile(x, y), layout::Tile::Floor) {
                    continue;
                }
                if [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| wall(x + dx, y + dy)) {
                    out.push((x, y));
                }
            }
        }
        out
    }

    /// Picks a goal for raider `id` of theme `theme`, standing at `at`: the nearest of what it
    /// is after.
    fn pick_goal(&self, theme: Theme, at: (i32, i32), report: &crate::kingdom::Report, n: usize) -> u32 {
        let c = catalog();
        let k = &self.home.industry;
        let mut lures = k.lures(c, report);
        // Only the works the stir reaches; if none of them is left, whatever is.
        let targets = self.kingdom.stir.watch.on.as_ref().map(|r| r.targets.clone()).unwrap_or_default();
        if lures.iter().any(|l| targets.contains(&l.id)) {
            lures.retain(|l| targets.contains(&l.id));
        }
        let near = |l: &crate::kingdom::Lure| (i32::from(l.tile.0) - at.0).pow(2) + (i32::from(l.tile.1) - at.1).pow(2);
        let mut sorted: Vec<&crate::kingdom::Lure> = lures.iter().collect();
        sorted.sort_by_key(|l| (near(l), l.id));
        // Boars go for the Keep's wall while its lift in hand can be reached.
        if theme == Theme::Boars && self.keep_reachable() {
            return KEEP;
        }
        // Thieves go for a store: the camp when nothing else holds goods.
        if theme == Theme::Thieves {
            let store = sorted.iter().find(|l| k.buildings.get(&l.id).is_some_and(|b| !b.output.is_empty()));
            return store.map_or(1, |l| l.id);
        }
        let pool = sorted.len().min(3).max(1);
        sorted.get(n % pool).map_or_else(|| if self.keep_reachable() { KEEP } else { 1 }, |l| l.id)
    }

    /// Whether the Keep's lift in hand is low enough to be reached: an unfinished wall.
    fn keep_reachable(&self) -> bool {
        let Some(site) = self.sites.keep() else { return false };
        let Some(castle) = site.castle() else { return false };
        if site.laid >= castle.len() {
            return false;
        }
        let first = castle.stages[castle.stage_at(site.laid)].first.min(site.laid);
        (first..site.laid).any(|i| {
            castle.stones[i as usize].aabb()[2] as f32 / 100.0 <= SIEGE_Z && site.stands(i)
        })
    }

    /// Once a second while a raid is on: the raiders act, the towers loose, the raid ends.
    fn raid_second(&mut self, second: u64) {
        let c = catalog();
        let report = self.home.industry.report(c);
        self.kingdom.stir.watch.mind.stamp = second;
        let Some(raid) = self.kingdom.stir.watch.on.clone() else { return };
        // The dawn, or the end of its time: the rest slink back.
        let (night, _) = clock(second);
        if !night || second.saturating_sub(raid.began) > LAST {
            self.end_raid(Some("THE RAIDERS FALL BACK DOWN THE SHAFT: DAWN"));
            return;
        }
        let alive: Vec<u32> = self.enemies.iter().filter(|e| e.hp > 0 && raid.band.contains(&e.id)).map(|e| e.id).collect();
        // A thief that fell drops what it carried.
        for (id, loot) in &raid.loot {
            if !self.enemies.iter().any(|e| e.hp > 0 && e.id == *id) {
                // What was taken from the camp is the camp's again, whatever room its stores have.
                for (good, n) in loot {
                    *self.home.industry.stock.entry(good.clone()).or_default() += *n;
                }
                self.home.industry.rev = self.home.industry.rev.saturating_add(1);
                self.notices.push("The thief fell: what it carried is the camp's again.".into());
                if let Some(r) = self.kingdom.stir.watch.on.as_mut() {
                    r.loot.remove(id);
                }
            }
        }
        for &id in &alive {
            self.raider_acts(id, &raid, &report, second);
        }
        self.towers_loose(&report, second);
        // Beaten: all are up and none left.
        let Some(raid) = self.kingdom.stir.watch.on.clone() else { return };
        let left = self.enemies.iter().any(|e| e.hp > 0 && raid.band.contains(&e.id));
        if raid.came >= raid.size && !left {
            self.kingdom.stir.watch.beaten = self.kingdom.stir.watch.beaten.saturating_add(1);
            self.kingdom.stir.level = self.kingdom.stir.level.saturating_sub(RELIEF);
            self.kingdom.stir.watch.on = None;
            self.found = Some((self.tick, 0, "THE RAID IS BROKEN: THE DEEP IS QUIETER".into()));
            self.notices.push("The raid is broken. The Deep is quieter.".into());
            self.sounds.push("wish_grant");
        }
    }

    /// One raider's second.
    fn raider_acts(&mut self, id: u32, raid: &Raid, report: &crate::kingdom::Report, second: u64) {
        let c = catalog();
        let Some(me) = self.enemies.iter().find(|e| e.id == id && e.hp > 0) else { return };
        let (x, y) = (me.x, me.y);
        if me.age < TELEGRAPH {
            return;
        }
        let at = tile_of(x, y);
        // A goal, or a new one.
        let mut goal = raid.goals.get(&id).copied().unwrap_or(NONE);
        let stale = goal == NONE
            || (goal != KEEP && goal != FLEE && !self.home.industry.buildings.contains_key(&goal))
            || (goal == KEEP && !self.keep_reachable());
        if stale {
            let n = raid.band.iter().position(|b| *b == id).unwrap_or(0);
            goal = self.pick_goal(raid.theme, at, report, n);
            if let Some(r) = self.kingdom.stir.watch.on.as_mut() {
                r.goals.insert(id, goal);
            }
        }
        // A thief with its goods runs for the shaft.
        if goal == FLEE {
            let (sx, sy) = crate::world::wishes::delve_stand();
            if (at.0 - sx).abs() <= AWAY && (at.1 - sy).abs() <= AWAY {
                let n = raid.loot.get(&id).map_or(0, |s| s.values().sum::<u32>());
                self.enemies.retain(|e| e.id != id);
                if let Some(r) = self.kingdom.stir.watch.on.as_mut() {
                    r.loot.remove(&id);
                    r.band.retain(|b| *b != id);
                    r.goals.remove(&id);
                    // The slot is closed: it will not come again.
                    r.size = r.size.max(r.came);
                }
                self.found = Some((self.tick, 0, format!("A THIEF SLIPS DOWN THE SHAFT WITH {n} GOODS")));
                self.notices.push(format!("A thief got away down the Great Shaft with {n} goods."));
            }
            return;
        }
        let theme = raid.theme;
        let blow = theme.blow();
        // Sappers blow what lines they pass: a road, a rollerway or a rail within three tiles.
        if theme == Theme::Sappers && (second + u64::from(id)) % 4 == 0 {
            let t = (at.0 as i16, at.1 as i16);
            if let Some(cut) = self.home.industry.cut_near(c, t, 3) {
                self.found = Some((self.tick, 0, format!("A LINE IS CUT AT {}, {}: THE HANDS WILL MEND IT, OR MEND IT YOURSELF", cut.0, cut.1)));
                self.sounds.push("bomb");
                return;
            }
        }
        // A defence in the way is battered.
        let in_the_way = self.kingdom.stir.watch.mind.blocked.get(&id).copied();
        if let Some(did) = in_the_way
            && self.home.industry.defence_of(c, did).is_some()
        {
            self.batter(did, blow);
            return;
        }
        // At the goal?
        let field = self.field_for(goal);
        let here = field.as_ref().map_or(INF, |f| f.at(at.0, at.1));
        if here > 14 {
            return;
        }
        let beat = (second + u64::from(id)) % 4 == 0;
        if goal == KEEP {
            if beat {
                self.siege_blow(id, at);
            }
            return;
        }
        // A defence that is the goal (a brazier, a tower) is battered, not burnt.
        if self.home.industry.defence_of(c, goal).is_some() {
            self.batter(goal, blow);
            return;
        }
        if !beat {
            return;
        }
        match theme {
            Theme::Fire => {
                if self.home.industry.ignite(c, goal) {
                    let name = self.home.industry.buildings.get(&goal).map_or(String::new(), |b| c.plans[&b.plan].name.to_uppercase());
                    self.found = Some((self.tick, 0, format!("FIRE: THE {name} IS ALIGHT. A WELL OR A KNIGHT'S HANDS PUT IT OUT")));
                    self.sounds.push("impact_fire");
                }
            }
            Theme::Thieves => {
                let from = if self.home.industry.buildings.get(&goal).is_some_and(|b| !b.output.is_empty()) { goal } else { 1 };
                let got = self.home.industry.rob(c, from, THIEF_TAKES);
                if !got.is_empty() {
                    if let Some(r) = self.kingdom.stir.watch.on.as_mut() {
                        r.loot.insert(id, got);
                        r.goals.insert(id, FLEE);
                    }
                    self.found = Some((self.tick, 0, "A THIEF IS AWAY WITH GOODS: CATCH IT BEFORE THE SHAFT".into()));
                    self.sounds.push("hit");
                } else {
                    // Nothing to take here: it tries the camp.
                    if let Some(r) = self.kingdom.stir.watch.on.as_mut() {
                        r.goals.insert(id, 1);
                    }
                }
            }
            Theme::Sappers => {
                // At the goal with nothing to cut: they batter what is in reach.
                if let Some(did) = self.nearby_defence(at, 2) {
                    self.batter(did, blow);
                }
            }
            Theme::Boars => {
                if self.home.industry.buildings.get(&goal).is_some_and(|b| c.plans[&b.plan].stalls > 0) && self.home.industry.wreck_cart(goal) {
                    self.found = Some((self.tick, 0, "THE BOARS WRECK A CART".into()));
                    self.sounds.push("rock_land");
                }
            }
        }
    }

    /// The nearest defence within `reach` tiles of a tile.
    fn nearby_defence(&self, at: (i32, i32), reach: i32) -> Option<u32> {
        let c = catalog();
        let k = &self.home.industry;
        let mut best: Option<(i32, u32)> = None;
        for (&id, b) in &k.buildings {
            if k.defence_of(c, id).is_none() {
                continue;
            }
            for t in k.covered(b) {
                let d = (i32::from(t.0) - at.0).abs().max((i32::from(t.1) - at.1).abs());
                if d <= reach && best.is_none_or(|(bd, bi)| (d, id) < (bd, bi)) {
                    best = Some((d, id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// A raider's blow on defence `id`.
    fn batter(&mut self, id: u32, dmg: u16) {
        let c = catalog();
        let name = self.home.industry.buildings.get(&id).map_or(String::new(), |b| c.plans[&b.plan].name.to_uppercase());
        match self.home.industry.batter(c, id, dmg) {
            Some(Blow::Down) => {
                self.found = Some((self.tick, 0, format!("THE {name} IS DOWN: THE CAMP'S HANDS WILL RAISE IT AGAIN")));
                self.sounds.push("rock_land");
                self.shake_a_little();
            }
            Some(Blow::Held(_)) => {
                if self.tick.is_multiple_of(u64::from(HZ) * 2) {
                    self.sounds.push("clang");
                }
            }
            None => {}
        }
    }

    fn shake_a_little(&mut self) {
        self.shake = self.shake.max(6);
    }

    /// A raider batters the Keep's lift in hand: the stone nearest it that it can reach.
    fn siege_blow(&mut self, id: u32, at: (i32, i32)) {
        let Some(raid) = self.kingdom.stir.watch.on.clone() else { return };
        let budget = if raid.stones >= STONE_BUDGET || (self.kingdom.stir.watch.raids <= 1 && raid.stones >= FIRST_STONES) {
            return;
        } else if self.kingdom.stir.watch.raids <= 1 {
            FIRST_STONES
        } else {
            STONE_BUDGET
        };
        let Some(site) = self.sites.keep() else { return };
        let Some(castle) = site.castle() else { return };
        let Some(rests) = works::rests(&castle.id) else { return };
        let first = castle.stages[castle.stage_at(site.laid)].first.min(site.laid);
        // The nearest stone of the lift in hand within reach.
        let (ox, oz) = works::origin_px();
        let me = (at.0 as f32 + 0.5, at.1 as f32 + 0.5);
        let mut best: Option<(f32, u32)> = None;
        for i in first..site.laid {
            if !site.stands(i) {
                continue;
            }
            let b = castle.stones[i as usize].aabb().map(|v| f32::from(v) / 100.0);
            if b[2] > SIEGE_Z {
                continue;
            }
            let (cx, cy) = ((b[0] + b[3]) / 2.0, (b[1] + b[4]) / 2.0);
            let (tx, ty) = ((ox + cx * PX_PER_M) / 16.0, (oz - cy * PX_PER_M) / 16.0);
            let d = (tx - me.0).hypot(ty - me.1);
            if d <= SIEGE_REACH && best.is_none_or(|(bd, bi)| (d, i) < (bd, bi)) {
                best = Some((d, i));
            }
        }
        let Some((_, stone)) = best else { return };
        let blows = raid.blows.get(&stone).copied().unwrap_or(0) + 1;
        let stout = if site.is_true(stone) { TRUE_BLOWS } else { STONE_BLOWS };
        if blows < stout {
            if let Some(r) = self.kingdom.stir.watch.on.as_mut() {
                r.blows.insert(stone, blows);
            }
            self.sounds.push("clang");
            return;
        }
        let gone: BTreeSet<u32> = site.lost.iter().copied().collect();
        let fall = rests.held_by(stone, first, site.laid, &gone);
        let n = 1 + fall.len();
        // Too much would come down, in this raid or with what is out already: this stone holds.
        if usize::from(raid.stones) + n > usize::from(budget) || gone.len() + n > usize::from(STONE_BUDGET) {
            return;
        }
        let mut out = vec![stone];
        out.extend(fall);
        let (down, trues) = match self.sites.keep_mut() {
            Some(site) => {
                let trues = out.iter().filter(|&&i| site.is_true(i)).count();
                (site.lose(&out), trues)
            }
            None => (0, 0),
        };
        if let Some(r) = self.kingdom.stir.watch.on.as_mut() {
            r.stones = r.stones.saturating_add(down as u8);
            r.blows.remove(&stone);
        }
        let _ = id;
        self.sounds.push("rock_land");
        self.shake_a_little();
        self.found = Some((
            self.tick,
            0,
            format!("THE RAIDERS PULL {down} STONES OUT OF THE WALL{}: THE MASONS WILL LAY THEM AGAIN", if trues > 0 { format!(", {trues} TRUE") } else { String::new() }),
        ));
    }

    /// The towers loose: each manned, armed tower looses at the nearest raider in its reach.
    fn towers_loose(&mut self, report: &crate::kingdom::Report, second: u64) {
        let c = catalog();
        let towers = self.home.industry.towers(c, report);
        let Some(raid) = &self.kingdom.stir.watch.on else { return };
        let band = raid.band.clone();
        for t in towers {
            if (second + u64::from(t.id)) % ARROW_EVERY != 0 {
                continue;
            }
            let from = centre_of((i32::from(t.tile.0), i32::from(t.tile.1)));
            let reach = f32::from(t.reach) * TILE_UNITS;
            let Some(i) = (0..self.enemies.len())
                .filter(|&i| self.enemies[i].hp > 0 && band.contains(&self.enemies[i].id))
                .map(|i| (i, (self.enemies[i].x - from.0).hypot(self.enemies[i].y - from.1)))
                .filter(|&(_, d)| d <= reach)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
            else {
                continue;
            };
            if !self.home.industry.tower_loose(t.id) {
                continue;
            }
            let to = (self.enemies[i].x, self.enemies[i].y);
            let e = &mut self.enemies[i];
            let dealt = ARROW_DAMAGE.min(e.hp);
            e.hp -= dealt;
            self.feel.tracer(from, to, false);
            self.sounds.push("shot_bow");
        }
    }

    /// The raid ends, for a proof or a test to call it off.
    #[cfg(any(test, feature = "testing"))]
    pub fn end_raid_for_test(&mut self) {
        self.end_raid(None);
    }

    /// The raid ends: whoever is left slinks back down the shaft, what thieves carried is lost.
    fn end_raid(&mut self, why: Option<&str>) {
        let Some(raid) = self.kingdom.stir.watch.on.take() else { return };
        self.enemies.retain(|e| !raid.band.contains(&e.id));
        self.kingdom.stir.watch.mind.fields.clear();
        if let Some(why) = why {
            self.found = Some((self.tick, 0, why.into()));
            self.notices.push(why.to_lowercase());
        }
    }

    /// Beaten by the Deep's raiders on the hillside: the knights are knocked down, not killed. The
    /// raiders take their due from the camp's stores and go. True if it was this (and not a wipe).
    pub(super) fn deep_rout(&mut self) -> bool {
        if self.kingdom.stir.watch.on.is_none()
            || self.room().kind != RoomKind::Kingdom
            || !(self.players.values().all(|h| h.hp == 0 || h.stone) && self.players.values().any(|h| h.hp == 0))
        {
            return false;
        }
        let c = catalog();
        for hero in self.players.values_mut() {
            if hero.hp == 0 {
                hero.hp = (hero.max_hp / 4).max(1);
            }
        }
        self.projectiles.retain(|p| !p.hostile);
        let took = self.home.industry.rob(c, 1, THIEF_TAKES * 2);
        let n: u32 = took.values().sum();
        self.end_raid(None);
        self.sounds.push("door_open");
        self.found = Some((self.tick, 0, format!("THE RAIDERS TAKE THEIR DUE, {n} GOODS, AND GO BACK DOWN. YOU ARE LEFT STANDING")));
        true
    }
}

#[cfg(test)]
#[path = "../../tests/rules/deep_raid__tests.rs"]
mod tests;
