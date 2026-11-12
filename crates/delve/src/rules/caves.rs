//! The living cave on the host's `Run`: seed plus changed cells, stepped on
//! the host's tick. Guests draw the same holes from that list.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

pub(crate) mod mines;
pub mod world;

use super::world::{Dest, Entrance};
use super::{Hero, Input, Run, RoomKind, Tile, TILE_UNITS};
use world::Cave;

/// Courtyard mouth. The party stands on it and the existing stair hold
/// takes them in (`floors::travel` → `take_entrance`).
pub const YARD_MOUTH: Entrance = Entrance {
    id: "cave-yard",
    room: RoomKind::Gate,
    at: (2, 6, 2, 1),
    to: Dest::Cave,
    label: "A LIT MOUTH IN THE YARD",
    needs: None,
};

/// First delve floor, in the start room. Stamped as stairs when that floor
/// is entered, then taken by the same stair hold.
pub const FLOOR_MOUTH: Entrance = Entrance {
    id: "cave-floor",
    room: RoomKind::Start,
    at: (3, 11, 1, 1),
    to: Dest::Cave,
    label: "A LIT MOUTH IN THE FIRST FLOOR",
    needs: None,
};

/// Where a knight appears inside, and the lit way back out.
/// Both are a scan of the seed's generated cave, the same on host and guest.
pub fn spawn_cell(seed: u32) -> (i32, i32) {
    base(seed).spawn
}

/// Grip boots: open floor just before the frost region's first ice.
/// The same cells for a seed on host and guest, and reachable without digging.
pub(crate) fn grip_cell(seed: u32) -> (i32, i32) {
    boot_pair(seed).0
}

/// Blades: open floor in the frost region, past its first ice.
pub(crate) fn blades_cell(seed: u32) -> (i32, i32) {
    boot_pair(seed).1
}

fn boot_pair(seed: u32) -> ((i32, i32), (i32, i32)) {
    let kept = base(seed);
    (kept.grip, kept.blades)
}

fn place_boots(cave: &world::Cave, spawn: (i32, i32)) -> ((i32, i32), (i32, i32)) {
    let w = world::MAP_W;
    let h = world::MAP_H;
    let mut dist = vec![-1i32; (w * h) as usize];
    let mut q = Vec::new();
    let mut head = 0usize;
    let at = |x: i32, y: i32| -> Option<usize> {
        if x < 0 || y < 0 || x >= w || y >= h {
            None
        } else {
            Some((y * w + x) as usize)
        }
    };
    if let Some(i) = at(spawn.0, spawn.1) {
        dist[i] = 0;
        q.push(spawn);
    }
    while head < q.len() {
        let (x, y) = q[head];
        head += 1;
        let d = dist[at(x, y).unwrap()];
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let (nx, ny) = (x + dx, y + dy);
            let Some(i) = at(nx, ny) else { continue };
            if dist[i] >= 0 {
                continue;
            }
            if !cave.at(nx, ny).is_some_and(|c| c.open()) {
                continue;
            }
            dist[i] = d + 1;
            q.push((nx, ny));
        }
    }
    let d_of = |x: i32, y: i32| at(x, y).map(|i| dist[i]).unwrap_or(-1);
    let mut first: Option<(i32, i32, i32)> = None;
    for &(x, y) in &q {
        let cell = cave.at(x, y).unwrap();
        if world::region_of(x, y).0 == 3 && cell.ice() {
            let cand = (d_of(x, y), y, x);
            if first.is_none_or(|b| cand < b) {
                first = Some(cand);
            }
        }
    }
    let (idist, iy, ix) = first.unwrap_or((0, spawn.1, spawn.0));
    let mut grip: Option<(i32, i32, i32, i32)> = None;
    for &(x, y) in &q {
        let d = d_of(x, y);
        if d < 0 || d >= idist {
            continue;
        }
        let cell = cave.at(x, y).unwrap();
        if cell.ice() || !cell.open() {
            continue;
        }
        let man = (x - ix).abs() + (y - iy).abs();
        if man > 8 {
            continue;
        }
        let cand = (man, -d, y, x);
        if grip.is_none_or(|b| cand < b) {
            grip = Some(cand);
        }
    }
    let grip = grip.map(|(_, _, y, x)| (x, y)).unwrap_or((ix.saturating_sub(1), iy));
    let mut blades: Option<(i32, i32, i32)> = None;
    for &(x, y) in &q {
        if world::region_of(x, y).0 != 3 {
            continue;
        }
        if (x, y) == (ix, iy) || (x, y) == grip {
            continue;
        }
        let d = d_of(x, y);
        if d <= idist {
            continue;
        }
        if !cave.at(x, y).is_some_and(|c| c.open()) {
            continue;
        }
        let past = if d >= idist + 6 { 0 } else { 1 };
        let cand = (past, y, x);
        if blades.is_none_or(|b| cand < b) {
            blades = Some(cand);
        }
    }
    let blades = blades.map(|(_, y, x)| (x, y)).unwrap_or((ix.saturating_add(1), iy));
    (grip, blades)
}

pub fn exit_cell(seed: u32) -> (i32, i32) {
    base(seed).exit
}

/// A seed's cave as generated, with its arrival and its way out. Generating
/// and scanning it costs about a millisecond, so it is done once a seed and
/// every later reader shares that one.
struct Base {
    seed: u32,
    cave: Cave,
    spawn: (i32, i32),
    exit: (i32, i32),
    grip: (i32, i32),
    blades: (i32, i32),
}

fn base(seed: u32) -> Arc<Base> {
    static KEPT: Mutex<Vec<Arc<Base>>> = Mutex::new(Vec::new());
    let seed = seed.max(1);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = kept.iter().find(|b| b.seed == seed) {
        return found.clone();
    }
    let cave = Cave::generate(seed);
    let spawn = scan_arrival(&cave);
    let exit = scan_exit(&cave, spawn.0, spawn.1);
    let (grip, blades) = place_boots(&cave, spawn);
    let made = Arc::new(Base { seed, cave, spawn, exit, grip, blades });
    // A session plays one seed. Four are kept so tests on other seeds do not push it out.
    if kept.len() >= 4 {
        kept.remove(0);
    }
    kept.push(made.clone());
    made
}

fn mine_open(cave: &world::Cave, x: i32, y: i32) -> bool {
    world::region_of(x, y).0 == 0 && cave.at(x, y).is_some_and(|c| c.open())
}

fn largest_mine_mask(cave: &world::Cave) -> Vec<bool> {
    let w = world::MAP_W;
    let h = world::MAP_H;
    let n = (w * h) as usize;
    let mut seen = vec![false; n];
    let mut best_seed = (0, 0);
    let mut best_len = 0usize;
    let mut best_min = (h, w);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if seen[i] || !mine_open(cave, x, y) {
                continue;
            }
            let mut len = 0usize;
            let mut min = (y, x);
            let mut q = vec![(x, y)];
            seen[i] = true;
            while let Some((cx, cy)) = q.pop() {
                len += 1;
                if (cy, cx) < min {
                    min = (cy, cx);
                }
                for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let j = (ny * w + nx) as usize;
                    if !seen[j] && mine_open(cave, nx, ny) {
                        seen[j] = true;
                        q.push((nx, ny));
                    }
                }
            }
            if len > best_len || (len == best_len && min < best_min) {
                best_len = len;
                best_min = min;
                best_seed = (x, y);
            }
        }
    }
    let mut mask = vec![false; n];
    if best_len == 0 {
        return mask;
    }
    let mut q = vec![best_seed];
    if let Some(i) = cave.world.idx(best_seed.0, best_seed.1) {
        mask[i] = true;
    }
    while let Some((cx, cy)) = q.pop() {
        for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            let j = (ny * w + nx) as usize;
            if !mask[j] && mine_open(cave, nx, ny) {
                mask[j] = true;
                q.push((nx, ny));
            }
        }
    }
    mask
}

fn against_wall(cave: &world::Cave, x: i32, y: i32) -> bool {
    [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .any(|(dx, dy)| cave.at(x + dx, y + dy).is_some_and(|c| c.wall()))
}

fn open_depth(cave: &world::Cave, x: i32, y: i32, dx: i32, dy: i32) -> i32 {
    let mut n = 0;
    let mut cx = x + dx;
    let mut cy = y + dy;
    while n < 3 && cave.at(cx, cy).is_some_and(|c| c.open()) {
        n += 1;
        cx += dx;
        cy += dy;
    }
    n
}

fn lamp_beside(cave: &world::Cave, x: i32, y: i32) -> bool {
    for dy in -2..=2i32 {
        for dx in -2..=2i32 {
            if dx.abs() + dy.abs() != 2 {
                continue;
            }
            let nx = x + dx;
            let ny = y + dy;
            if cave.at(nx, ny).is_some_and(|c| c.open()) && against_wall(cave, nx, ny) {
                return true;
            }
        }
    }
    false
}

fn arrival_site(cave: &world::Cave, mask: &[bool], x: i32, y: i32) -> bool {
    if x < 12 || y < 12 || x >= world::MAP_W - 12 || y >= world::MAP_H - 12 {
        return false;
    }
    let Some(i) = cave.world.idx(x, y) else {
        return false;
    };
    if !mask[i] || world::region_of(x, y).0 != 0 {
        return false;
    }
    [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .all(|(dx, dy)| open_depth(cave, x, y, dx, dy) >= 3)
        && lamp_beside(cave, x, y)
}

fn scan_arrival(cave: &world::Cave) -> (i32, i32) {
    let mask = largest_mine_mask(cave);
    let mut best: Option<(i32, i32, i32)> = None;
    for y in 0..world::MAP_H {
        for x in 0..world::MAP_W {
            if !arrival_site(cave, &mask, x, y) {
                continue;
            }
            let centered = i32::from(
                x >= 24 && x < world::MAP_W - 24 && y >= 14 && y < world::MAP_H - 14,
            );
            let score = centered * 10000 - ((x - 40).abs() + (y - 40).abs());
            if best.is_none_or(|(s, _, _)| score > s) {
                best = Some((score, x, y));
            }
        }
    }
    if let Some((_, x, y)) = best {
        return (x, y);
    }
    let w = world::MAP_W;
    let mut fallback = (8, 36);
    for y in 0..world::MAP_H {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if mask[i] {
                fallback = (x, y);
                return fallback;
            }
        }
    }
    fallback
}

fn scan_exit(cave: &world::Cave, sx: i32, sy: i32) -> (i32, i32) {
    let mut found: Option<(i32, i32, i32)> = None;
    for dy in -2..=2 {
        for dx in -2..=2 {
            if i32::abs(dx) + i32::abs(dy) != 2 {
                continue;
            }
            let ex = sx + dx;
            let ey = sy + dy;
            if cave.at(ex, ey).is_some_and(|c| c.open()) && against_wall(cave, ex, ey) {
                let cand_dx = dx;
                if found.is_none() || cand_dx < found.unwrap().0 {
                    found = Some((cand_dx, ex, ey));
                }
            }
        }
    }
    found.map(|(_, ex, ey)| (ex, ey)).unwrap_or((sx, sy))
}

/// Ticks a knight takes over one cell: a pixel a tick.
pub const MOVE_EVERY: u8 = 8;
/// Ticks between one knight's shovel hits while the key is held.
const SWING_EVERY: u8 = 10;
/// Ticks a knight rests after a blast.
const BLAST_EVERY: u8 = 24;
const CHANGE_CAP: usize = 4096;

/// A shovel hit that did not break the cell.
pub const BURST_CHIP: u8 = 0;
/// A cell breaking into rubble.
pub const BURST_CRUMBLE: u8 = 1;
/// A magic blast, at the cell it was aimed at.
pub const BURST_BLAST: u8 = 2;
/// A burst is dropped this many ticks after it starts. The longest picture of
/// one runs 67 ticks.
const BURST_KEPT: u32 = 72;
const BURST_CAP: usize = 32;

/// Rock struck or broken a moment ago, for the draw: when, where and which.
/// Not part of the cave's cells; a guest who joins late misses only the dust.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Burst {
    /// The tick it starts on. A blast's outer breaks start a little after it.
    pub at: u32,
    pub x: i16,
    pub y: i16,
    pub kind: u8,
}

const STONE_NAME: [&str; 4] = ["limestone", "flint", "basalt", "freestone"];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Seat {
    pub x: i16,
    pub y: i16,
    pub face: u8,
    pub stone: [u8; 4],
    pub magic: u8,
    pub cool: u8,
    pub back_room: u16,
    pub back_x: i16,
    pub back_y: i16,
    pub from_yard: bool,
    /// Extra cells ice has carried this knight. A guest reads the same count.
    #[serde(default)]
    pub slides: u32,
    #[serde(default)]
    coast: i8,
    #[serde(default)]
    coast_dx: i8,
    #[serde(default)]
    coast_dy: i8,
    /// 0 plain, 1 grip, 2 blades. Grip does not coast.
    #[serde(default)]
    footwear: u8,
    #[serde(default)]
    skid_x: i16,
    #[serde(default)]
    skid_y: i16,
    #[serde(default)]
    skid_dx: i8,
    #[serde(default)]
    skid_dy: i8,
    /// 255 means no skid. Otherwise ticks since the slide was last refreshed.
    #[serde(default = "skid_none")]
    skid_age: u8,
    /// The last step taken, so the draw can carry the knight across the cell
    /// while `cool` counts down.
    #[serde(default)]
    step_dx: i8,
    #[serde(default)]
    step_dy: i8,
    /// That step was ice carrying the knight, not a stride.
    #[serde(default)]
    glide: bool,
    /// The tick after the last shovel hit or blast (0 before the first), and
    /// the ticks to rest from it.
    #[serde(default)]
    acted: u32,
    #[serde(default)]
    rest: u8,
}

impl Seat {
    fn at(back_room: u16, back_x: f32, back_y: f32, from_yard: bool) -> Self {
        Self {
            x: 0,
            y: 0,
            face: 0,
            stone: [0; 4],
            magic: world::MAGIC_MAX,
            cool: 0,
            back_room,
            back_x: (back_x * 16.0) as i16,
            back_y: (back_y * 16.0) as i16,
            from_yard,
            slides: 0,
            coast: 0,
            coast_dx: 0,
            coast_dy: 0,
            footwear: 0,
            skid_x: 0,
            skid_y: 0,
            skid_dx: 0,
            skid_dy: 0,
            skid_age: 255,
            step_dx: 0,
            step_dy: 0,
            glide: false,
            acted: 0,
            rest: 0,
        }
    }

    /// How far behind its cell the knight is still drawn, in pixels: the step
    /// it is taking, a pixel a tick.
    pub(crate) fn stride(&self) -> (i32, i32) {
        let left = i32::from(self.cool.min(MOVE_EVERY));
        (-i32::from(self.step_dx) * left, -i32::from(self.step_dy) * left)
    }

    /// Mid-step: walking or carried by ice.
    pub(crate) fn stepping(&self) -> bool {
        self.cool > 0 && (self.step_dx != 0 || self.step_dy != 0)
    }

    /// The step being taken is ice carrying the knight.
    pub(crate) fn gliding(&self) -> bool {
        self.stepping() && self.glide
    }

    /// Ticks since the last shovel hit or blast, while the knight still rests from it.
    pub(crate) fn swinging(&self, tick: u32) -> Option<u32> {
        let age = (tick + 1).checked_sub(self.acted)?;
        (self.acted > 0 && age < u32::from(self.rest)).then_some(age)
    }

    /// Age of a shovel hit only. A blast rests longer, and must keep its fist.
    pub(crate) fn digging(&self, tick: u32) -> Option<u32> {
        let age = self.swinging(tick)?;
        (self.rest == SWING_EVERY).then_some(age)
    }

    fn rested(&self, tick: u32) -> bool {
        self.swinging(tick).is_none()
    }

    fn act(&mut self, tick: u32, rest: u8) {
        self.acted = tick + 1;
        self.rest = rest;
    }

    #[cfg(test)]
    pub(crate) fn pose_north_dig(&mut self, tick: u32) {
        self.face = 3;
        self.acted = tick.max(1);
        self.rest = SWING_EVERY;
    }

    pub(crate) fn pack(&self) -> world::Pack {
        world::Pack {
            magic: self.magic,
            stone: self.stone,
            footwear: boots_of(self.footwear),
        }
    }

    /// A slide's wake, while it still shows: the cell it reached, the way it
    /// went, and the ticks since it stopped. Gone at 15.
    pub(crate) fn skid(&self) -> Option<((i32, i32), (i32, i32), u8)> {
        if self.skid_age >= 15 || (self.skid_dx == 0 && self.skid_dy == 0) {
            return None;
        }
        Some((
            (i32::from(self.skid_x), i32::from(self.skid_y)),
            (i32::from(self.skid_dx), i32::from(self.skid_dy)),
            self.skid_age,
        ))
    }

    fn store_pack(&mut self, pack: &world::Pack) {
        self.magic = pack.magic;
        self.stone = pack.stone;
    }
}

fn skid_none() -> u8 {
    255
}

fn boots_of(n: u8) -> world::Footwear {
    match n {
        1 => world::Footwear::Grip,
        2 => world::Footwear::Blades,
        _ => world::Footwear::Plain,
    }
}

/// Awake cells kept across ticks. Equal to any other diet so a field compare
/// still means the guest log, not the scratch.
#[derive(Clone, Debug, Default)]
struct Diet {
    ready: bool,
    seed: u32,
    awake: Vec<u32>,
    sealed: Vec<u32>,
    stirred: Vec<u32>,
}

impl PartialEq for Diet {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Diet {}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Field {
    #[serde(default, skip_serializing_if = "u32_zero")]
    pub seed: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub changes: BTreeMap<u32, u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub seats: BTreeMap<u32, Seat>,
    /// Host step count. Countdowns are derived from it, so a fire is two wire
    /// writes. Not serialized: a changing clock would republish the cave every tick.
    #[serde(skip)]
    clock: u32,
    /// Tick a cell caught fire. Fire is FIRE_LIFE minus the ticks since, while above 0.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    fires: BTreeMap<u32, u32>,
    /// Tick a cell's steam ends.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    steams: BTreeMap<u32, u32>,
    /// Integrity left on a wall acid has bitten. Untouched walls are not listed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    acids: BTreeMap<u32, u8>,
    /// Tick a melting cell refreezes, if its water is still there.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    melts: BTreeMap<u32, u32>,
    /// Rock struck or broken in the last few ticks. Empty in a quiet cave.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bursts: Vec<Burst>,
    /// Who took the grip boots. 0 means they still lie on the floor.
    #[serde(default, skip_serializing_if = "u32_zero")]
    pub grip_by: u32,
    /// Who took the blades. 0 means they still lie on the floor.
    #[serde(default, skip_serializing_if = "u32_zero")]
    pub blades_by: u32,
    /// Ticks a knight has stood on the mouth. Not world state; each host counts its own.
    #[serde(skip)]
    mouth_hold: BTreeMap<u32, u32>,
    /// Vein cells a knight has proved. Sorted. Not written into the shared cave.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proven: Vec<(i32, i32)>,
    /// Whether interact was held last tick. The sink is the press, not the hold.
    #[serde(skip)]
    interact_down: BTreeMap<u32, bool>,
    /// Cells the fluid step may touch, kept across ticks. Not part of the guest log.
    #[serde(skip)]
    diet: Diet,
}

fn u32_zero(n: &u32) -> bool {
    *n == 0
}

impl Field {
    pub fn quiet(&self) -> bool {
        self.seed == 0 && self.changes.is_empty() && self.seats.is_empty()
    }

    pub fn open(&self) -> bool {
        !self.seats.is_empty()
    }

    pub fn inside(&self, id: u32) -> bool {
        self.seats.contains_key(&id)
    }

    /// One more tick stood on the mouth. The count is this host's, not the cave.
    pub fn mouth_tick(&mut self, id: u32) -> u32 {
        let n = self.mouth_hold.entry(id).or_default();
        *n = n.saturating_add(1);
        *n
    }

    pub fn clear_mouth(&mut self, id: u32) {
        self.mouth_hold.remove(&id);
    }

    fn burst(&mut self, at: u32, x: i32, y: i32, kind: u8) {
        if self.bursts.len() >= BURST_CAP {
            self.bursts.remove(0);
        }
        self.bursts.push(Burst { at, x: x as i16, y: y as i16, kind });
    }

    fn drop_old_bursts(&mut self, tick: u32) {
        if !self.bursts.is_empty() {
            self.bursts.retain(|b| tick < b.at.saturating_add(BURST_KEPT));
        }
    }

    pub fn view_live(&self) -> world::Cave {
        let mut cave = self.view();
        self.wake_hot(&mut cave);
        cave
    }

    /// One host step of water and refreezing. Changed cells are stored so a
    /// second seat, reading the same field, draws the same fluids.
    /// The whole-map path, kept so a test can prove the awake path matches it.
    pub(crate) fn step_fluids_scan(&mut self) {
        if self.seats.is_empty() {
            return;
        }
        let before = self.view_live();
        let mut after = before.clone();
        for y in 0..world::MAP_H {
            for x in 0..world::MAP_W {
                if after.at(x, y).is_some_and(|c| c.fluid != 0 || c.melt > 0 || c.fire > 0 || c.steam > 0) {
                    after.wake(x, y);
                }
            }
        }
        after.step_sim();
        self.clock = self.clock.saturating_add(1);
        for y in 0..world::MAP_H {
            for x in 0..world::MAP_W {
                let a = after.at(x, y).unwrap();
                let b = before.at(x, y).unwrap();
                if a != b {
                    self.absorb(x, y, b, a);
                }
            }
        }
    }

    pub fn step_fluids(&mut self) {
        if self.seats.is_empty() {
            return;
        }
        self.prepare_diet();
        let touch: Vec<usize> = self.diet.awake.iter().map(|i| *i as usize).collect();
        let sealed: Vec<usize> = self.diet.sealed.iter().map(|i| *i as usize).collect();
        if touch.is_empty() && sealed.is_empty() {
            return;
        }
        let after = {
            let field = &*self;
            world::react_touch(world::MAP_W, world::MAP_H, 0, |i| field.cell_live_idx(i), &touch, &sealed)
        };
        self.store_react(&after);
        self.retarget_awake(&after);
    }

    fn store_react(&mut self, after: &[(usize, world::Cell)]) {
        let before: Vec<world::Cell> = after.iter().map(|&(i, _)| self.cell_live_idx(i)).collect();
        self.clock = self.clock.saturating_add(1);
        for (n, &(i, a)) in after.iter().enumerate() {
            let x = (i as i32) % world::MAP_W;
            let y = (i as i32) / world::MAP_W;
            if before[n] != a {
                self.absorb(x, y, &before[n], &a);
            }
        }
    }

    fn cell_live_idx(&self, i: usize) -> world::Cell {
        let x = (i as i32) % world::MAP_W;
        let y = (i as i32) / world::MAP_W;
        let mut cell = self.cell(x, y).unwrap_or(world::Cell {
            flags: 0,
            fire: 0,
            fluid: 0,
            amount: 0,
            integrity: 0,
            steam: 0,
            height: y as u8,
            stone: 0,
            melt: 0,
            swings: 0,
        });
        cell
    }

    fn prepare_diet(&mut self) {
        if !self.diet.ready || self.diet.seed != self.seed {
            self.seed_diet();
        }
        let stirred = std::mem::take(&mut self.diet.stirred);
        self.note_awake(stirred);
    }

    fn seed_diet(&mut self) {
        let base = base(self.seed);
        let mut awake = Vec::new();
        let mut sealed = Vec::new();
        for (i, c) in base.cave.world.cells.iter().enumerate() {
            if c.fluid != world::FLUID_NONE || c.melt > 0 || c.fire > 0 || c.steam > 0 {
                awake.push(i as u32);
            }
            if c.sealed() {
                sealed.push(i as u32);
            }
        }
        for &k in self
            .changes
            .keys()
            .chain(self.melts.keys())
            .chain(self.fires.keys())
            .chain(self.steams.keys())
            .chain(self.acids.keys())
        {
            let x = (k & 0x0fff) as i32;
            let y = ((k >> 12) & 0x0fff) as i32;
            if let Some(i) = index_of(x, y) {
                awake.push(i);
            }
        }
        self.diet.awake.clear();
        self.diet.sealed = sealed;
        self.diet.seed = self.seed;
        self.diet.ready = true;
        self.note_awake(awake);
        let sealed = std::mem::take(&mut self.diet.sealed);
        self.diet.sealed = sealed.into_iter().filter(|&i| self.cell_live_idx(i as usize).sealed()).collect();
    }

    fn note_awake(&mut self, extra: Vec<u32>) {
        let w = world::MAP_W;
        let h = world::MAP_H;
        let seeds = extra.clone();
        self.diet.awake.extend(extra);
        for i in seeds {
            let x = (i as i32) % w;
            let y = (i as i32) / w;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x + dx;
                let ny = y + dy;
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                self.diet.awake.push((ny * w + nx) as u32);
            }
        }
        self.diet.awake.sort_unstable();
        self.diet.awake.dedup();
    }

    fn retarget_awake(&mut self, after: &[(usize, world::Cell)]) {
        let mut next = Vec::new();
        for &(i, c) in after {
            if c.fluid != world::FLUID_NONE || c.melt > 0 || c.fire > 0 || c.steam > 0 {
                next.push(i as u32);
            }
        }
        next.extend(std::mem::take(&mut self.diet.stirred));
        self.diet.awake.clear();
        self.note_awake(next);
        let sealed = std::mem::take(&mut self.diet.sealed);
        self.diet.sealed = sealed
            .into_iter()
            .filter(|&i| self.cell_live_idx(i as usize).sealed())
            .collect();
    }

    fn stir(&mut self, x: i32, y: i32) {
        if let Some(i) = index_of(x, y) {
            self.diet.stirred.push(i);
        }
    }


    fn wake_hot(&self, cave: &mut world::Cave) {
        let mut keys: Vec<u32> = self
            .fires
            .keys()
            .chain(self.steams.keys())
            .chain(self.melts.keys())
            .copied()
            .collect();
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            let x = (key & 0x0fff) as i32;
            let y = ((key >> 12) & 0x0fff) as i32;
            if cave.at(x, y).is_some_and(|c| c.fire > 0 || c.steam > 0 || c.melt > 0) {
                cave.wake(x, y);
            }
        }
    }

    fn paint_hot(&self, cave: &mut world::Cave) {
        let mut keys: Vec<u32> = self
            .fires
            .keys()
            .chain(self.steams.keys())
            .chain(self.acids.keys())
            .chain(self.melts.keys())
            .copied()
            .collect();
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            let x = (key & 0x0fff) as i32;
            let y = ((key >> 12) & 0x0fff) as i32;
            if let Some(cell) = cave.at_mut(x, y) {
                self.paint_cell(key, cell);
            }
        }
    }

    fn paint_cell(&self, key: u32, cell: &mut world::Cell) {
        let now = self.clock;
        if let Some(&caught) = self.fires.get(&key) {
            cell.fire = fire_left(caught, now);
        }
        if let Some(&end) = self.steams.get(&key) {
            cell.steam = ticks_left(end, now);
        }
        if let Some(&left) = self.acids.get(&key) {
            if cell.wall() {
                cell.integrity = left;
            }
        }
        if let Some(&at) = self.melts.get(&key) {
            cell.melt = ticks_left(at, now);
        }
    }

    fn absorb(&mut self, x: i32, y: i32, before: &world::Cell, after: &world::Cell) {
        if !same_pack(before, after) {
            let packed = key(x, y);
            if self.changes.len() < CHANGE_CAP || self.changes.contains_key(&packed) {
                self.changes.insert(packed, pack_cell(x, y, after));
                self.stir(x, y);
            }
        }
        self.keep_ephemeral(x, y, after);
    }

    fn keep_ephemeral(&mut self, x: i32, y: i32, cell: &world::Cell) {
        let k = key(x, y);
        self.keep_fire(k, cell.fire);
        Self::keep_until(&mut self.steams, self.clock, k, cell.steam);
        Self::keep_until(&mut self.melts, self.clock, k, cell.melt);
        self.keep_acid(x, y, cell);
    }

    fn keep_fire(&mut self, k: u32, fire: u8) {
        let now = self.clock;
        let predicted = self.fires.get(&k).copied().map(|c| fire_left(c, now)).unwrap_or(0);
        if fire == predicted {
            if fire == 0 {
                self.fires.remove(&k);
            }
            return;
        }
        if fire == 0 {
            self.fires.remove(&k);
            return;
        }
        let age = u32::from(world::FIRE_LIFE.saturating_sub(fire));
        self.fires.insert(k, now.saturating_sub(age));
    }

fn keep_until(map: &mut BTreeMap<u32, u32>, now: u32, k: u32, left: u8) {
        let predicted = map.get(&k).copied().map(|end| ticks_left(end, now)).unwrap_or(0);
        if left == predicted {
            if left == 0 {
                map.remove(&k);
            }
            return;
        }
        if left == 0 {
            map.remove(&k);
        } else {
            map.insert(k, now.saturating_add(u32::from(left)));
        }
    }

    fn keep_acid(&mut self, x: i32, y: i32, cell: &world::Cell) {
        let k = key(x, y);
        if !cell.wall() || cell.integrity == 0 {
            self.acids.remove(&k);
            return;
        }
        let born = world::cell_from(world::terrain(self.seed, x, y), x, y).integrity;
        if cell.integrity == born {
            self.acids.remove(&k);
        } else if self.acids.get(&k) != Some(&cell.integrity) {
            self.acids.insert(k, cell.integrity);
        }
    }

    /// A stand beside soft rock, and the aim that faces it. Fixed scan, west-to-east.
    pub fn soft_aim(&self) -> Option<(i16, i16, i8, i8)> {
        let cave = self.view();
        for y in 1..world::MAP_H - 1 {
            for x in 1..world::MAP_W - 1 {
                let Some(cell) = cave.at(x, y) else { continue };
                if !(cell.wall() && !cell.hard() && !cell.ice()) {
                    continue;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let ox = x - dx;
                    let oy = y - dy;
                    if cave.at(ox, oy).is_some_and(|c| c.open() && !c.wall()) {
                        return Some((ox as i16, oy as i16, dx as i8, dy as i8));
                    }
                }
            }
        }
        None
    }

    pub fn fingerprint(&self) -> u64 {
        let mut n = u64::from(self.seed);
        for (k, v) in &self.changes {
            n = n.wrapping_mul(0x100000001b3).wrapping_add(u64::from(*k));
            n = n.wrapping_mul(0x100000001b3).wrapping_add(*v);
        }
        // Hot entries, not the clock. A fire republishes on catch and out only.
        for (k, v) in &self.fires {
            n = n.wrapping_mul(0x100000001b3).wrapping_add(u64::from(*k));
            n = n.wrapping_mul(0x100000001b3).wrapping_add(u64::from(*v));
        }
        for (k, v) in &self.steams {
            n = n.wrapping_mul(0x100000001b3).wrapping_add(u64::from(*k) ^ 0x51);
            n = n.wrapping_mul(0x100000001b3).wrapping_add(u64::from(*v));
        }
        for (k, v) in &self.acids {
            n = n.wrapping_mul(0x100000001b3).wrapping_add(u64::from(*k) ^ 0xa3);
            n = n.wrapping_mul(0x100000001b3).wrapping_add(u64::from(*v));
        }
        for (k, v) in &self.melts {
            n = n.wrapping_mul(0x100000001b3).wrapping_add(u64::from(*k) ^ 0x1d);
            n = n.wrapping_mul(0x100000001b3).wrapping_add(u64::from(*v));
        }
        n
    }

    /// The whole cave as it stands: the seed's cave with every changed cell laid over it.
    pub fn view(&self) -> Cave {
        self.view_as(self.seed)
    }

    /// `view`, for a cave nobody has entered yet and so has no seed of its own.
    pub(crate) fn view_as(&self, seed: u32) -> Cave {
        let mut cave = base(seed).cave.clone();
        for (key, packed) in &self.changes {
            apply(&mut cave, *key, *packed);
        }
        self.paint_hot(&mut cave);
        cave
    }

    /// One cell as it stands, without building the whole cave.
    pub(crate) fn cell(&self, x: i32, y: i32) -> Option<world::Cell> {
        let base = base(self.seed);
        let mut cell = *base.cave.at(x, y)?;
        if let Some(packed) = self.changes.get(&key(x, y)) {
            lay(&mut cell, *packed);
        }
        self.paint_cell(key(x, y), &mut cell);
        Some(cell)
    }

    /// Change the cave through `f` and keep what cell `(x, y)` became.
    fn edit(&mut self, x: i32, y: i32, f: impl FnOnce(&mut Cave)) {
        let mut cave = self.view();
        f(&mut cave);
        if let Some(cell) = cave.at(x, y).copied() {
            self.record(x, y, &cell);
        }
    }

    /// Keep one changed cell. A long walk fills the log with melt ticks, and a
    /// dig still has to land, so a full log gives up another cell for it.
    fn record(&mut self, x: i32, y: i32, cell: &world::Cell) {
        let k = key(x, y);
        if self.changes.len() >= CHANGE_CAP && !self.changes.contains_key(&k) {
            if let Some(drop) = self.changes.keys().copied().find(|key| *key != k) {
                self.changes.remove(&drop);
            }
        }
        self.changes.insert(k, pack_cell(x, y, cell));
        self.keep_ephemeral(x, y, cell);
        self.stir(x, y);
    }
}

fn key(x: i32, y: i32) -> u32 {
    ((x as u32) & 0x0fff) | (((y as u32) & 0x0fff) << 12)
}

fn index_of(x: i32, y: i32) -> Option<u32> {
    if x < 0 || y < 0 || x >= world::MAP_W || y >= world::MAP_H {
        None
    } else {
        Some((y * world::MAP_W + x) as u32)
    }
}


fn fire_left(caught: u32, now: u32) -> u8 {
    let age = now.saturating_sub(caught);
    if age < u32::from(world::FIRE_LIFE) {
        (u32::from(world::FIRE_LIFE) - age) as u8
    } else {
        0
    }
}

fn ticks_left(end: u32, now: u32) -> u8 {
    if end > now {
        (end - now).min(255) as u8
    } else {
        0
    }
}

fn same_pack(a: &world::Cell, b: &world::Cell) -> bool {
    a.flags == b.flags
        && a.fluid == b.fluid
        && a.amount == b.amount
        && a.stone == b.stone
        && a.swings == b.swings
}

fn pack_cell(x: i32, y: i32, cell: &world::Cell) -> u64 {
    let flags = cell.flags as u64;
    let fluid = u64::from(cell.fluid);
    let amount = u64::from(cell.amount);
    let stone = u64::from(cell.stone);
    (u64::from(key(x, y)))
        | (flags << 24)
        | (fluid << 40)
        | (amount << 48)
        | (stone << 56)
        | (u64::from(cell.swings & 0x0f) << 60)
}

fn apply(cave: &mut Cave, key: u32, packed: u64) {
    let x = (key & 0x0fff) as i32;
    let y = ((key >> 12) & 0x0fff) as i32;
    if let Some(cell) = cave.at_mut(x, y) {
        lay(cell, packed);
    }
}

/// A changed cell's packed state laid over the generated one.
fn lay(cell: &mut world::Cell, packed: u64) {
    cell.flags = ((packed >> 24) & 0xffff) as u16;
    cell.fluid = ((packed >> 40) & 0xff) as u8;
    cell.amount = ((packed >> 48) & 0xff) as u8;
    cell.stone = ((packed >> 56) & 0x0f) as u8;
    cell.swings = ((packed >> 60) & 0x0f) as u8;
    cell.integrity = 0;
}

impl Run {
    pub fn stamp_cave_mouth(&mut self) {
        if self.dungeon.depth != 1 {
            return;
        }
        let at = self.at;
        let Some(room) = self.dungeon.rooms.get_mut(at) else {
            return;
        };
        if room.kind != RoomKind::Start {
            return;
        }
        let (col, row, _, _) = FLOOR_MOUTH.at;
        if col >= 0 && row >= 0 {
            room.set(col as usize, row as usize, Tile::Stairs);
        }
    }

    /// The seed of this run's cave: the one it has, or the one it will be
    /// given when the first knight goes in.
    pub(crate) fn cave_seed(&self) -> u32 {
        if self.caves.seed != 0 {
            return self.caves.seed;
        }
        let seed = self.seed as u32;
        if seed == 0 { world::DEMO_SEED } else { seed }
    }

    pub fn enter_cave(&mut self, from_yard: bool) {
        self.keep_cave_seed();
        let ids: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.enter_one(id, from_yard);
        }
        self.cues.push("entrance:cave".to_string());
    }

    /// One knight who has stood on the mouth. The rest of the party stays in the delve.
    pub fn enter_one(&mut self, id: u32, from_yard: bool) {
        if self.players.get(&id).is_none_or(|h| h.hp == 0 || h.stone) || self.caves.inside(id) {
            return;
        }
        self.keep_cave_seed();
        let back_room = self.at as u16;
        let (x, y) = self.players.get(&id).map(|h| (h.x, h.y)).unwrap_or((0.0, 0.0));
        let (sx, sy) = self.arrival_for_next();
        let seat = self
            .caves
            .seats
            .entry(id)
            .or_insert_with(|| Seat::at(back_room, x, y, from_yard));
        seat.x = sx as i16;
        seat.y = sy as i16;
        seat.magic = world::MAGIC_MAX;
        seat.from_yard = from_yard;
        if seat.footwear == 0 && id == self.caves.grip_by {
            seat.footwear = 1;
        } else if seat.footwear == 0 && id == self.caves.blades_by {
            seat.footwear = 2;
        }
        if let Some(hero) = self.players.get_mut(&id) {
            if from_yard {
                hero.x = (YARD_MOUTH.at.0 as f32 + 3.0) * TILE_UNITS;
                hero.y = YARD_MOUTH.at.1 as f32 * TILE_UNITS;
            } else {
                hero.x = (FLOOR_MOUTH.at.0 as f32) * TILE_UNITS;
                hero.y = (FLOOR_MOUTH.at.1 as f32 - 1.0) * TILE_UNITS;
            }
        }
        self.caves.clear_mouth(id);
    }

    /// Where the next knight in arrives: the arrival cell, or the first free
    /// place two cells from it, so a party stands apart and not in one spot.
    fn arrival_for_next(&self) -> (i32, i32) {
        let seed = self.caves.seed.max(1);
        let (sx, sy) = spawn_cell(seed);
        let exit = exit_cell(seed);
        for (dx, dy) in [(0, 0), (2, 0), (0, 2), (2, 2), (-2, 0), (0, -2)] {
            let at = (sx + dx, sy + dy);
            let taken = self.caves.seats.values().any(|s| (i32::from(s.x), i32::from(s.y)) == at);
            if at != exit && !taken && self.caves.cell(at.0, at.1).is_some_and(|c| c.open()) {
                return at;
            }
        }
        (sx, sy)
    }

    /// Any knight who stands on the mouth for the usual stair time goes in alone.
    pub(crate) fn stands_on_cave_mouth(&self, id: u32) -> bool {
        if self.caves.inside(id) {
            return false;
        }
        let Some(hero) = self.players.get(&id) else {
            return false;
        };
        if hero.hp == 0 || hero.stone {
            return false;
        }
        if self.caves.mouth_hold.get(&id).copied().unwrap_or(0) > 0 {
            return true;
        }
        let col = (hero.x / super::TILE_UNITS) as i32;
        let row = (hero.y / super::TILE_UNITS) as i32;
        if self.room().tile(col, row) != Tile::Stairs {
            return false;
        }
        let kind = self.room().kind;
        floor_mouth(kind, col, row).is_some()
            || (kind == RoomKind::Gate
                && col >= YARD_MOUTH.at.0
                && col < YARD_MOUTH.at.0 + YARD_MOUTH.at.2
                && row >= YARD_MOUTH.at.1
                && row < YARD_MOUTH.at.1 + YARD_MOUTH.at.3)
    }

    pub(super) fn offer_cave_mouth(&mut self) {
        let depth = self.dungeon.depth;
        let kind = self.room().kind;
        let ids: Vec<u32> = self.players.keys().copied().collect();
        let mut ready = Vec::new();
        for id in ids {
            let Some(hero) = self.players.get(&id) else { continue };
            if hero.hp == 0 || hero.stone || hero.absent || self.caves.inside(id) {
                self.caves.clear_mouth(id);
                continue;
            }
            let col = (hero.x / super::TILE_UNITS) as i32;
            let row = (hero.y / super::TILE_UNITS) as i32;
            let gate = if depth == 1 && self.room().tile(col, row) == super::Tile::Stairs {
                floor_mouth(kind, col, row)
            } else {
                None
            };
            let Some(gate) = gate else {
                self.caves.clear_mouth(id);
                continue;
            };
            if self.caves.mouth_tick(id) >= super::home::DESCEND_HOLD {
                ready.push((id, gate.room == super::RoomKind::Gate));
            }
        }
        for (id, from_yard) in ready {
            self.caves.clear_mouth(id);
            self.enter_one(id, from_yard);
        }
    }
    pub(super) fn cave_mouth_holds(
        &mut self,
        on_stairs: &BTreeMap<u32, Option<&str>>,
        gate: Option<&str>,
        from_yard: bool,
    ) {
        let standing: Vec<u32> = on_stairs
            .iter()
            .filter(|(_, g)| **g == gate)
            .map(|(id, _)| *id)
            .collect();
        let outside: Vec<u32> = self
            .players
            .iter()
            .filter(|(id, h)| h.hp > 0 && !h.stone && !self.caves.inside(**id))
            .map(|(id, _)| *id)
            .collect();
        for id in outside {
            if !standing.contains(&id) {
                self.caves.clear_mouth(id);
            }
        }
        let ready: Vec<u32> = standing
            .into_iter()
            .filter(|id| self.caves.mouth_tick(*id) >= super::home::DESCEND_HOLD)
            .collect();
        for id in ready {
            self.enter_one(id, from_yard);
        }
    }

    pub fn tick_caves(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.caves.seats.is_empty() {
            return;
        }
        let tick = self.tick as u32;
        self.caves.drop_old_bursts(tick);
        let ids: Vec<u32> = self.caves.seats.keys().copied().collect();
        for id in ids {
            let Some(input) = inputs.get(&id).cloned().or_else(|| self.players.get(&id).map(|_| Input::default())) else {
                continue;
            };
            self.tick_one_cave(id, &input);
        }
        self.caves.step_fluids();
    }

    fn tick_one_cave(&mut self, id: u32, input: &Input) {
        let Some(mut seat) = self.caves.seats.get(&id).copied() else {
            return;
        };
        let tick = self.tick as u32;
        if seat.cool > 0 {
            seat.cool -= 1;
        }
        if self.tick > 0 && self.tick.is_multiple_of(8) && seat.magic < world::MAGIC_MAX {
            seat.magic = seat.magic.saturating_add(1);
        }
        let (dx, dy) = facing(input);
        let old = (seat.x, seat.y);
        let walled = |field: &Field, x: i32, y: i32| field.cell(x, y).is_some_and(|c| c.wall() && !c.open());
        if seat.cool == 0 && (dx != 0 || dy != 0) {
            let nx = seat.x as i32 + dx;
            let ny = seat.y as i32 + dy;
            if !walled(&self.caves, nx, ny) {
                seat.x = nx as i16;
                seat.y = ny as i16;
                seat.cool = MOVE_EVERY;
                seat.step_dx = dx as i8;
                seat.step_dy = dy as i8;
                seat.glide = false;
                self.take_boots(id, &mut seat);
                self.note_ice(&mut seat, old, dx, dy);
            }
            seat.face = face_of(dx, dy);
        } else if seat.cool == 0 && seat.coast > 0 {
            let cx = i32::from(seat.coast_dx);
            let cy = i32::from(seat.coast_dy);
            let nx = seat.x as i32 + cx;
            let ny = seat.y as i32 + cy;
            if !walled(&self.caves, nx, ny) && (cx != 0 || cy != 0) {
                seat.x = nx as i16;
                seat.y = ny as i16;
                seat.cool = MOVE_EVERY;
                seat.step_dx = cx as i8;
                seat.step_dy = cy as i8;
                seat.glide = true;
                seat.coast -= 1;
                seat.slides = seat.slides.saturating_add(1);
                self.take_boots(id, &mut seat);
            } else {
                seat.coast = 0;
            }
        }
        if seat.coast > 0 {
            seat.skid_x = seat.x;
            seat.skid_y = seat.y;
            seat.skid_dx = seat.coast_dx;
            seat.skid_dy = seat.coast_dy;
            seat.skid_age = 0;
        } else if seat.skid_age < 255 {
            seat.skid_age = seat.skid_age.saturating_add(1);
        }
        let hero = self.players.get(&id).cloned();
        let mut pack = seat.pack();
        let (fx, fy) = if dx == 0 && dy == 0 { face_step(seat.face) } else { (dx, dy) };
        let (tx, ty) = aim_cell(seat.x as i32, seat.y as i32, input, fx, fy);
        // A held key is one hit every `SWING_EVERY` ticks, so the cracks show.
        if input.fire && seat.rested(tick) {
            let before: u8 = pack.stone.iter().sum();
            if self.fire_at(&mut pack, tx, ty) {
                seat.act(tick, SWING_EVERY);
                seat.face = face_of(tx - i32::from(seat.x), ty - i32::from(seat.y));
            }
            if pack.stone.iter().sum::<u8>() > before {
                self.seat_notices.push((id, "+1 STONE".to_string()));
            }
        }
        if (input.cast != 0 || input.play != 0) && seat.rested(tick) {
            let element = hero.as_ref().map(|h| element_of(h, input.cast)).unwrap_or(0);
            if self.magic_at(&mut pack, tx, ty, element) {
                seat.act(tick, BLAST_EVERY);
                seat.face = face_of(tx - i32::from(seat.x), ty - i32::from(seat.y));
            }
        }
        seat.store_pack(&pack);
        if self.caves.seats.contains_key(&id) {
            self.caves.seats.insert(id, seat);
        self.sink_on_press(id, input);
        self.mark_veins_seen(id);
        }
        if (i32::from(seat.x), i32::from(seat.y)) == exit_cell(self.caves.seed) {
            self.leave_cave(id);
        }
    }

    fn take_boots(&mut self, id: u32, seat: &mut Seat) {
        let seed = self.caves.seed;
        if seed == 0 {
            return;
        }
        let at = (i32::from(seat.x), i32::from(seat.y));
        let (grip, blades) = boot_pair(seed);
        if at == grip && self.caves.grip_by == 0 {
            self.caves.grip_by = id;
            seat.footwear = 1;
        } else if at == blades && self.caves.blades_by == 0 {
            self.caves.blades_by = id;
            seat.footwear = 2;
        }
    }

    fn note_ice(&mut self, seat: &mut Seat, old: (i16, i16), dx: i32, dy: i32) {
        let was = self.caves.cell(i32::from(old.0), i32::from(old.1)).is_some_and(|c| c.ice());
        let here = self.caves.cell(i32::from(seat.x), i32::from(seat.y));
        let now = here.is_some_and(|c| c.ice());
        let flags = here.map(|c| c.flags).unwrap_or(0);
        let boots = boots_of(seat.footwear);
        let slip = world::slippery(flags, boots);
        if now && was && slip {
            seat.slides = seat.slides.saturating_add(1);
        }
        if now && slip {
            let n = world::coast_cells(flags, boots);
            seat.coast = n.clamp(0, 8) as i8;
            seat.coast_dx = dx as i8;
            seat.coast_dy = dy as i8;
            seat.skid_x = seat.x;
            seat.skid_y = seat.y;
            seat.skid_dx = dx as i8;
            seat.skid_dy = dy as i8;
            seat.skid_age = 0;
        } else if now {
            seat.coast = 0;
        }
    }

    /// The shovel on the cell in front. True when it struck something.
    fn fire_at(&mut self, pack: &mut world::Pack, x: i32, y: i32) -> bool {
        let Some(cell) = self.caves.cell(x, y) else {
            return false;
        };
        let tick = self.tick as u32;
        // An open ice floor does not melt under a shovel: a dull chip, and
        // a few ice chips from the frost rock-chip sheet. Thin water still closes.
        if cell.ice() && !cell.wall() {
            self.caves.burst(tick, x, y, BURST_CHIP);
            self.sounds.push("rock_land");
            return true;
        }
        if cell.thin() && !cell.wall() {
            self.caves.edit(x, y, |cave| cave.melt_ice(x, y));
            return true;
        }
        if !cell.wall() {
            return false;
        }
        // One hit. A loose wall opens on hit 3; hits 1 and 2 crack it. Hard
        // rock only rings: it takes magic.
        let mut dig = world::Dig::Miss;
        self.caves.edit(x, y, |cave| dig = cave.shovel(pack, x, y));
        match dig {
            world::Dig::Miss => return false,
            world::Dig::Broke { .. } => self.caves.burst(tick, x, y, BURST_CRUMBLE),
            world::Dig::Swing(_) | world::Dig::Ring => self.caves.burst(tick, x, y, BURST_CHIP),
        }
        self.sounds.push("hit");
        true
    }

    /// Magic on the cell in front. True when it was spent.
    fn magic_at(&mut self, pack: &mut world::Pack, x: i32, y: i32, element: u8) -> bool {
        let Some(cell) = self.caves.cell(x, y) else {
            return false;
        };
        if element == 1 && (cell.ice() || cell.thin()) {
            self.caves.edit(x, y, |cave| cave.melt_ice(x, y));
            return true;
        }
        if element == 2 && cell.fluid == world::FLUID_WATER && cell.amount > 0 {
            let mut froze = false;
            self.caves.edit(x, y, |cave| froze = cave.frost_bridge(pack, x, y));
            return froze;
        }
        if (cell.wall() || cell.hard() || cell.rubble()) && self.blast_at(pack, x, y) {
            self.sounds.push("hit");
            self.fx.push(super::fx::Fx::Shake(8));
            return true;
        }
        false
    }

    fn blast_at(&mut self, pack: &mut world::Pack, x: i32, y: i32) -> bool {
        let mut cave = self.caves.view();
        let before: Vec<u16> = cave.world.cells.iter().map(|c| c.flags).collect();
        if !cave.blast(pack, x, y) {
            return false;
        }
        let tick = self.tick as u32;
        self.caves.burst(tick, x, y, BURST_BLAST);
        let w = world::MAP_W;
        for (i, cell) in cave.world.cells.iter().enumerate() {
            if cell.flags == before[i] {
                continue;
            }
            let cx = (i as i32) % w;
            let cy = (i as i32) / w;
            self.caves.record(cx, cy, cell);
            if before[i] & world::FL_WALL != 0 && !cell.wall() {
                // The aimed cell breaks with the blast, its neighbours two ticks after.
                let late = if (cx, cy) == (x, y) { 0 } else { 2 };
                self.caves.burst(tick + late, cx, cy, BURST_CRUMBLE);
            }
        }
        true
    }

    fn keep_cave_seed(&mut self) {
        if self.home.cave_seed == 0 {
            let today = if self.caves.seed != 0 {
                self.caves.seed
            } else {
                let seed = self.seed as u32;
                if seed == 0 { world::DEMO_SEED } else { seed }
            };
            self.home.cave_seed = u64::from(today);
        }
        self.caves.seed = self.home.cave_seed as u32;
    }

    fn mark_veins_seen(&mut self, id: u32) {
        let Some(seat) = self.caves.seats.get(&id).copied() else { return };
        let at = (i32::from(seat.x), i32::from(seat.y));
        let laid = mines::bodies(self.caves.seed);
        let mut extra = Vec::new();
        for body in &laid.bodies {
            for &cell in &body.cells {
                if mines::chebyshev(at, cell) > mines::SIGHT {
                    continue;
                }
                if self.caves.proven.binary_search(&cell).is_ok() {
                    continue;
                }
                if self.caves.cell(cell.0, cell.1).is_none() {
                    continue;
                }
                extra.push(cell);
            }
        }
        for cell in extra {
            if let Err(at) = self.caves.proven.binary_search(&cell) {
                self.caves.proven.insert(at, cell);
            }
        }
    }

    fn sink_on_press(&mut self, id: u32, input: &Input) {
        let was = self.caves.interact_down.get(&id).copied().unwrap_or(false);
        self.caves.interact_down.insert(id, input.interact);
        if !input.interact || was {
            return;
        }
        let Some(seat) = self.caves.seats.get(&id).copied() else { return };
        let at = (i32::from(seat.x), i32::from(seat.y));
        let seed = self.caves.seed;
        let laid = mines::bodies(seed);
        let Some(body) = laid.bodies.iter().find(|b| mines::beside(at, b.head) && !self.home.mines.contains_key(&b.index)) else {
            return;
        };
        if mines::proven(&self.caves, body) < mines::PROVE_AT {
            return;
        }
        let claim_id = format!("cave-{seed}-{}", body.index);
        let stone = mines::stone_name(body.region);
        let seam = crate::rules::geology::seam_at(mines::pack_of(body.region), u32::from(body.depth));
        Run::found_in(&mut self.home.industry, self.seed);
        self.home.industry.discover(
            crate::kingdom::catalog(),
            claim_id.clone(),
            crate::kingdom::Claim {
                resource: stone.to_ascii_lowercase(),
                name: format!("{stone} node {}", body.depth),
                source: "the caves".into(),
                secured: false,
                rate: seam.rate(),
                yields: seam.yields(),
                surface: true,
            },
        );
        self.home.industry.secure(&claim_id);
        let site = mines::SURFACE_SITE[usize::from(body.index)];
        match self.raise_quarry(site, &claim_id) {
            Ok(()) => {
                self.home.mines.insert(
                    body.index,
                    mines::SunkMine {
                        region: body.region,
                        head_x: body.head.0,
                        head_y: body.head.1,
                        who: id,
                        tick: self.tick,
                        claim: claim_id,
                    },
                );
                let line = format!("A MINE IS SUNK: {} NODE {}", stone.to_ascii_uppercase(), body.depth);
                let ids: Vec<u32> = self.players.keys().copied().collect();
                for pid in ids {
                    self.seat_notices.push((pid, line.clone()));
                }
            }
            Err(why) => {
                let wait = format!("THE QUARRY WAITS: {why}");
                let ids: Vec<u32> = self.players.keys().copied().collect();
                for pid in ids {
                    self.seat_notices.push((pid, wait.clone()));
                }
            }
        }
    }

    fn raise_quarry(&mut self, site: (i32, i32), claim: &str) -> Result<(), String> {
        use crate::kingdom::{Command, catalog};
        let k = &mut self.home.industry;
        let before: std::collections::BTreeSet<u32> = k.buildings.keys().copied().collect();
        k.apply(catalog(), Command::Build { plan: "claim-quarry".into(), tile: (site.0 as i16, site.1 as i16) })?;
        let id = k
            .buildings
            .keys()
            .copied()
            .find(|id| !before.contains(id))
            .ok_or_else(|| "the quarry was not raised".to_string())?;
        if let Err(why) = k
            .apply(catalog(), Command::Link { building: id, claim: claim.into() })
            .and_then(|_| k.apply(catalog(), Command::Connect(id)))
        {
            k.buildings.remove(&id);
            return Err(why);
        }
        Ok(())
    }

    fn leave_cave(&mut self, id: u32) {
        let Some(seat) = self.caves.seats.remove(&id) else {
            return;
        };
        for (i, name) in STONE_NAME.iter().enumerate() {
            let n = u32::from(seat.stone[i]);
            if n > 0 {
                let _ = self.home.stockyard.stack(name, n);
            }
        }
        if seat.from_yard {
            self.raise_home_world();
            if let Some(gate) = super::world::room_of(&self.dungeon, RoomKind::Gate) {
                let at = ((YARD_MOUTH.at.0 + 3) as f32, YARD_MOUTH.at.1 as f32);
                self.arrive(gate, at);
            }
        } else if let Some(hero) = self.players.get_mut(&id) {
            hero.x = f32::from(seat.back_x) / 16.0;
            hero.y = f32::from(seat.back_y) / 16.0;
        }
        self.cues.push("cave-leave".to_string());
    }
}

fn face_step(face: u8) -> (i32, i32) {
    match face {
        1 => (-1, 0),
        2 => (1, 0),
        3 => (0, -1),
        _ => (0, 1),
    }
}

fn facing(input: &Input) -> (i32, i32) {
    let dx = if input.move_x > 0 {
        1
    } else if input.move_x < 0 {
        -1
    } else {
        0
    };
    let dy = if input.move_y > 0 {
        1
    } else if input.move_y < 0 {
        -1
    } else {
        0
    };
    (dx, dy)
}

fn face_of(dx: i32, dy: i32) -> u8 {
    if dx < 0 {
        1
    } else if dx > 0 {
        2
    } else if dy < 0 {
        3
    } else {
        0
    }
}

fn aim_cell(x: i32, y: i32, input: &Input, dx: i32, dy: i32) -> (i32, i32) {
    // A zero on one axis is part of the aim, not a request to use facing.
    let aimed = input.aim_x != 0 || input.aim_y != 0;
    let ax = if aimed { i32::from(input.aim_x) } else { dx };
    let ay = if aimed { i32::from(input.aim_y) } else { dy };
    if ax == 0 && ay == 0 {
        return (x, y + 1);
    }
    let (sx, sy) = if ax.abs() >= ay.abs() {
        (ax.signum(), 0)
    } else {
        (0, ay.signum())
    };
    (x + sx, y + sy)
}

fn element_of(hero: &Hero, cast: u8) -> u8 {
    let slot = usize::from(cast.saturating_sub(1));
    let Some(name) = hero.spells.get(slot).and_then(|s| s.as_deref()) else {
        return 0;
    };
    let n = name.to_ascii_lowercase();
    if n.contains("fire") || n.contains("flame") || n.contains("burn") || n.contains("ember") {
        return 1;
    }
    if n.contains("frost") || n.contains("ice") || n.contains("chill") || n.contains("cold") {
        return 2;
    }
    0
}

/// The yard mouth, if this tile is it. Other entrances win first.
pub fn yard_mouth(kind: RoomKind, col: i32, row: i32) -> Option<&'static Entrance> {
    let (x, y, w, h) = YARD_MOUTH.at;
    (kind == RoomKind::Gate && col >= x && col < x + w && row >= y && row < y + h).then_some(&YARD_MOUTH)
}

pub fn floor_mouth(kind: RoomKind, col: i32, row: i32) -> Option<&'static Entrance> {
    let (x, y, w, h) = FLOOR_MOUTH.at;
    (kind == RoomKind::Start && col >= x && col < x + w && row >= y && row < y + h).then_some(&FLOOR_MOUTH)
}

#[cfg(test)]
#[path = "../../tests/rules/caves__tests.rs"]
mod tests; // cf1 proof stills live in this module

#[cfg(test)]
mod proven_set {
    use super::*;

    #[test]
    fn proving_writes_the_set_and_spends_nothing_from_the_change_log() {
        let mut run = crate::rules::Run::new(11, 1, Some("Ash"));
        run.enter_one(1, false);
        let body = mines::bodies(run.caves.seed).bodies[0].clone();
        let (x, y) = body.cells[0];
        let seat = run.caves.seats.get_mut(&1).unwrap();
        seat.x = x as i16;
        seat.y = y as i16;
        let flags = run.caves.cell(x, y).unwrap().flags;
        let changes = run.caves.changes.clone();
        run.mark_veins_seen(1);
        assert_eq!(run.caves.cell(x, y).unwrap().flags, flags);
        assert_eq!(run.caves.changes, changes);
        assert!(run.caves.proven.binary_search(&(x, y)).is_ok());
        let back: Field = serde_json::from_value(serde_json::to_value(&run.caves).unwrap()).unwrap();
        assert_eq!(back.proven, run.caves.proven);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/caves_mines__tests.rs"]
mod mines_tests;

#[cfg(test)]
#[path = "../../tests/rules/caves_mt1__tests.rs"]
mod mt1_tests;

#[cfg(test)]
#[path = "../../tests/rules/caves_mt3__tests.rs"]
mod mt3_tests;
