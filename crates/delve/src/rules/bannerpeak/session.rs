//! The run: every seated knight in one shared world, one tick at a time.
//!
//! Shared by the party: the foes, the blocks, the gold sparks lying about, the
//! gates, the banners, the ring of eight sparks, the doors, the clock. Each
//! knight has a body, a pie of wedges, a size and a camera of their own. A
//! tick runs the knights in seat order, then the foes, the items and the
//! rest, so the same keys on the same map replay the same run.
use super::body::{Body, Ev, Jump, Knobs, Move, Mover, Q, SHIELD_ID, TQ};
use super::foes::{
    BOLT_LIFE, BOLT_MOST, BOLT_SPEED, Bolt, Boulder, CARRY, CapState, Foe, FoeKind, Item, ItemKind,
};
use super::knight::{
    BACK_PIPS, BUBBLE_TICKS, Bubble, HURT_GRACE, Knight, PIPS, PIT_TICKS, POSE_TICKS, Size,
    WIPE_PIPS, follow,
};
use super::boss::{BOSS_ID, Beat as BossBeat, Boss};
use super::layout::{self, HEIGHT, SNOW, WIDTH, cell_centre, cell_feet, info, world};
use super::world::{Grid, TILE, Tile, World};
use crate::rules::{HZ, Hero, Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A run never runs past an hour; a longer one is not a run to play.
pub const LIMIT: u32 = 60 * 60 * HZ;
/// The banners it takes to open the summit door.
pub const SUMMIT_BANNERS: u32 = 7;
/// Ticks all knights stand down before the checkpoint takes them back.
pub const WIPE_TICKS: u32 = 20;
/// Ticks a banner select waits before the cursor's choice is taken.
pub const SELECT_TICKS: u32 = 240;
/// Ticks of the finale at the Great Pole before the run is done.
pub const FINALE_TICKS: u32 = 210;
/// Ticks a banner's name stays on every screen.
pub const ANNOUNCE_TICKS: u32 = 110;
/// Ticks of a block's hop, a coin's ping, a smoke puff.
pub const FX_LIFE: u32 = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Play,
    /// The course board is up: choose where to go on the mountain.
    Select,
    /// All eight banners are raised at the Great Pole.
    Finale,
}

/// A short effect, drawn from the sim so every screen shows the same one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxKind {
    Coin,
    Pop,
    Dust,
    Ring,
    Bump,
    Debris,
    Gleam,
    Smoke,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fx {
    pub k: FxKind,
    /// Pixels.
    pub x: i32,
    pub y: i32,
    pub t: u32,
}

/// The banner just raised, shown on every screen for a while.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announce {
    pub banner: u8,
    pub t: u32,
}

/// The names on the course board, and the banners they stand for.
pub const BOARD: [(&str, u8); 6] = [
    ("MARSH FLAG", 2),
    ("MILL RACE", 3),
    ("BELL SHAFT", 4),
    ("SNOW RUN", 5),
    ("EIGHT SPARKS", 6),
    ("KING CINDER", 7),
];
/// The gate (index into `info().gates`) each board line starts at, but for
/// the sparks, which start at the first section with one still to find.
const BOARD_GATE: [usize; 6] = [0, 2, 3, 5, 0, 7];
/// The banners' names, by number.
pub const BANNER_NAMES: [&str; 8] = [
    "TOWER TOP",
    "CELLAR",
    "MARSH FLAG",
    "MILL RACE",
    "BELL SHAFT",
    "SNOW RUN",
    "EIGHT SPARKS",
    "KING CINDER",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub seats: BTreeMap<u32, Knight>,
    pub elapsed: u32,
    /// Bit `n` is banner `n` raised.
    pub banners: u8,
    /// Bit `n` is ring spark `n` gathered.
    pub sparks: u8,
    /// Bit `n` is gate `n` raised.
    pub gates: u16,
    /// 0 for the start, else one more than the latest gate.
    pub checkpoint: u8,
    /// Bit 0: the courtyard's tapestry door is open. Bit 1: the summit door.
    pub doors: u8,
    pub phase: Phase,
    pub phase_t: u32,
    /// The line of the course board the cursor is on.
    pub cursor: u8,
    pub foes: Vec<Foe>,
    pub items: Vec<Item>,
    pub bolts: Vec<Bolt>,
    pub boulders: Vec<Boulder>,
    /// King Cinder, on the summit.
    pub boss: Boss,
    pub fx: Vec<Fx>,
    /// Gold sparks lying on the ground that have been taken (by mark number).
    pub taken: BTreeSet<u16>,
    /// Tiles a knight has changed: struck blocks, broken floor, opened doors.
    pub changed: BTreeMap<u32, u8>,
    pub announce: Option<Announce>,
    /// Ticks since every knight was down.
    pub wipe: u32,
    /// How many seats the foes were provisioned for (never goes down).
    pub scaled: u8,
    /// The board has been up once on this way in.
    pub seen: bool,
    pub completed: Option<u32>,
}

/// The map as it stands: the fixed tiles with the changed ones over them.
pub struct View<'a> {
    pub world: &'static World,
    pub changed: &'a BTreeMap<u32, u8>,
}

impl Grid for View<'_> {
    fn tile(&self, tx: i32, ty: i32) -> Tile {
        if tx >= 0 && tx < WIDTH && ty >= 0 && ty < HEIGHT {
            if let Some(&c) = self.changed.get(&((ty * WIDTH + tx) as u32)) {
                if let Some(t) = Tile::from_code(c) {
                    return t;
                }
            }
        }
        self.world.at(tx, ty)
    }
}

type Rect = (i32, i32, i32, i32);
fn overlap(a: Rect, b: Rect) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}
/// A body's box in whole pixels.
fn px(b: &Body) -> Rect {
    let (l, t, r, bt) = b.rect();
    (l.div_euclid(Q), t.div_euclid(Q), (r - 1).div_euclid(Q) + 1, bt.div_euclid(Q))
}

impl Session {
    pub fn new(players: &BTreeMap<u32, Hero>) -> Session {
        let mut s = Session {
            seats: BTreeMap::new(),
            elapsed: 0,
            banners: 0,
            sparks: 0,
            gates: 0,
            checkpoint: 0,
            doors: 0,
            phase: Phase::Play,
            phase_t: 0,
            cursor: 0,
            foes: Vec::new(),
            items: Vec::new(),
            bolts: Vec::new(),
            boulders: Vec::new(),
            boss: Boss::new(1),
            fx: Vec::new(),
            taken: BTreeSet::new(),
            changed: BTreeMap::new(),
            announce: None,
            wipe: 0,
            scaled: 0,
            seen: false,
            completed: None,
        };
        let ids: Vec<u32> = players
            .iter()
            .filter(|(_, h)| !h.away && !h.stone)
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            s.seat(id, 0);
        }
        s.provision();
        s
    }

    pub fn view(&self) -> View<'_> {
        View {
            world: world(),
            changed: &self.changed,
        }
    }
    pub fn tile(&self, tx: i32, ty: i32) -> Tile {
        self.view().tile(tx, ty)
    }
    fn set_tile(&mut self, tx: i32, ty: i32, t: Tile) {
        if tx >= 0 && tx < WIDTH && ty >= 0 && ty < HEIGHT {
            self.changed.insert((ty * WIDTH + tx) as u32, t.code());
        }
    }

    /// Where the latest checkpoint stands: its feet, in pixels.
    pub fn station(&self) -> (i32, i32) {
        let i = info();
        let cell = match self.checkpoint {
            0 => i.start,
            n => i.gates[usize::from(n) - 1],
        };
        cell_feet(cell)
    }

    /// Sit a knight down at the checkpoint, spread a little by seat.
    fn seat(&mut self, id: u32, joined: u32) {
        let (x, y) = self.station();
        let n = self.seats.len() as i32;
        // Alternate either side of the post; stay out of the walls.
        let off = ((n + 1) / 2) * 7 * if n % 2 == 0 { 1 } else { -1 };
        let mut x = x + off;
        let g = self.view();
        let (cx, cy) = (x.div_euclid(TILE), (y - 1).div_euclid(TILE));
        if g.tile(cx, cy).solid() || !g.tile(cx, cy + 1).solid() && !g.tile(cx, cy + 1).one_way() {
            x -= off;
        }
        let mut k = Knight::at(joined, x, y);
        k.cam = follow((0, 0), x, y, 1, 0, 0);
        self.seats.insert(id, k);
    }

    /// Foes for the seats at the table: the map's own, then more as the table
    /// fills. Never fewer, and never weaker.
    pub fn provision(&mut self) {
        let seats = self.seats.len().max(1) as u8;
        let first = self.scaled == 0;
        let tier = |s: u8| u8::from(s >= 3) + u8::from(s >= 5);
        let (was, now) = (tier(self.scaled), tier(seats));
        let station_x = self.station().0;
        for (n, m) in info().foes.iter().enumerate() {
            let (kind, wanted) = match m.k {
                's' => (FoeKind::Snuffer, first),
                'c' => (FoeKind::Capped, first),
                'w' => (FoeKind::Wisp, first),
                'e' => (FoeKind::Snuffer, was < 1 && now >= 1),
                'f' => (FoeKind::Capped, was < 2 && now >= 2),
                _ => continue,
            };
            // A mid-run extra behind the party is not worth a late arrival.
            let behind = !first && m.x * TILE < station_x;
            if wanted && !behind {
                self.foes.push(Foe::new(kind, (m.x, m.y), n as u16));
            }
        }
        if self.scaled == 0 {
            self.boss.plates = seats;
        } else {
            self.boss.seated(self.scaled, seats.max(self.scaled));
        }
        self.scaled = self.scaled.max(seats);
    }

    fn fx(&mut self, k: FxKind, x: i32, y: i32) {
        if self.fx.len() >= 48 {
            self.fx.remove(0);
        }
        self.fx.push(Fx { k, x, y, t: FX_LIFE });
    }

    /// The bounds a save, the wire or a repair holds a session to.
    pub fn valid(&self) -> bool {
        let (wp, hp) = (WIDTH * TILE, HEIGHT * TILE);
        self.seats.len() <= MAX_PLAYERS as usize
            && self.elapsed <= LIMIT
            && self.seats.iter().all(|(&id, k)| {
                (1..=MAX_PLAYERS).contains(&id) && k.valid() && k.joined <= self.elapsed
            })
            && self.checkpoint as usize <= info().gates.len()
            && u32::from(self.gates) < 1 << info().gates.len()
            && self.doors <= 3
            && self.phase_t <= LIMIT
            && (self.cursor as usize) < BOARD.len()
            && self.foes.len() <= 128
            && self.foes.iter().all(|f| f.valid(wp, hp))
            && self.items.len() <= 48
            && self.items.iter().all(|i| i.valid(wp, hp))
            && self.bolts.len() <= 16
            && self.bolts.iter().all(|b| b.valid(wp, hp))
            && self.boulders.len() <= 8
            && self.boulders.iter().all(|b| (0..=wp * Q).contains(&b.x) && b.t < 1 << 24)
            && self.boss.valid(MAX_PLAYERS as u8)
            && self.fx.len() <= 48
            && self.fx.iter().all(|f| f.t <= FX_LIFE)
            && self.taken.len() <= info().coins.len()
            && self.taken.iter().all(|&c| (c as usize) < info().coins.len())
            && self.changed.len() <= 2048
            && self.changed.iter().all(|(&k, &c)| {
                (k as i32) < WIDTH * HEIGHT && Tile::from_code(c).is_some()
            })
            && self.announce.is_none_or(|a| a.banner < 8 && a.t <= ANNOUNCE_TICKS)
            && self.wipe <= WIPE_TICKS
            && self.scaled <= MAX_PLAYERS as u8
            && self.completed.is_none_or(|t| t <= self.elapsed)
            && (self.phase != Phase::Finale || self.banners == 0xFF)
    }

    /// Mend what a bad save broke: out-of-range things go, the rest stays.
    pub fn repair(&mut self) {
        self.seats.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id));
        self.elapsed = self.elapsed.min(LIMIT);
        let elapsed = self.elapsed;
        let (wp, hp) = (WIDTH * TILE, HEIGHT * TILE);
        let station = self.station();
        for k in self.seats.values_mut() {
            if !k.valid() || k.joined > elapsed {
                let joined = k.joined.min(elapsed);
                *k = Knight::at(joined, station.0, station.1);
            }
        }
        self.checkpoint = self.checkpoint.min(info().gates.len() as u8);
        self.gates &= ((1u32 << info().gates.len()) - 1) as u16;
        self.doors &= 3;
        self.phase_t = self.phase_t.min(LIMIT);
        self.cursor = self.cursor.min(BOARD.len() as u8 - 1);
        self.foes.retain(|f| f.valid(wp, hp));
        self.foes.truncate(128);
        self.items.retain(|i| i.valid(wp, hp));
        self.items.truncate(48);
        self.bolts.retain(|b| b.valid(wp, hp));
        self.bolts.truncate(16);
        self.boulders.retain(|b| (0..=wp * Q).contains(&b.x));
        self.boulders.truncate(8);
        if !self.boss.valid(MAX_PLAYERS as u8) {
            self.boss = Boss::new(self.seats.len().max(1) as u8);
        }
        self.fx.retain(|f| f.t <= FX_LIFE);
        self.fx.truncate(48);
        let coins = info().coins.len();
        self.taken.retain(|&c| (c as usize) < coins);
        self.changed
            .retain(|&k, &mut c| (k as i32) < WIDTH * HEIGHT && Tile::from_code(c).is_some());
        if self.changed.len() > 2048 {
            self.changed.clear();
        }
        if self.announce.is_some_and(|a| a.banner >= 8 || a.t > ANNOUNCE_TICKS) {
            self.announce = None;
        }
        self.wipe = self.wipe.min(WIPE_TICKS);
        self.scaled = self.scaled.min(MAX_PLAYERS as u8);
        if self.completed.is_some_and(|t| t > elapsed) {
            self.completed = None;
        }
        if self.phase == Phase::Finale && self.banners != 0xFF {
            self.phase = Phase::Play;
        }
    }

    /// Banners raised.
    pub fn count(&self) -> u32 {
        self.banners.count_ones()
    }

    /// Whether the summit door's count is met.
    pub fn door_count(&self) -> u32 {
        (self.banners & 0x7F).count_ones()
    }

    /// The knights standing and playing: not in a bubble or a pit.
    pub fn alive(&self) -> Vec<u32> {
        self.seats
            .iter()
            .filter(|(_, k)| k.alive())
            .map(|(&id, _)| id)
            .collect()
    }

    /// The gate a ring spark not yet found sends a knight to on the board.
    fn spark_gate(&self) -> usize {
        let i = info();
        let found = (0..i.sparks.len()).find(|&n| self.sparks & (1 << n) == 0);
        let Some(n) = found else { return 0 };
        let at = i.sparks[n].0;
        match layout::section_of(at) {
            1 => 0,
            2 => 2,
            3 => 3,
            _ => 5,
        }
    }

    // ------------------------------------------------------------ the tick

    /// One tick for the whole table; the sounds it makes are pushed to `say`.
    pub fn step(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        heroes: &BTreeMap<u32, Hero>,
        knobs: &Knobs,
        say: &mut Vec<&'static str>,
    ) {
        self.elapsed = (self.elapsed + 1).min(LIMIT);
        // Seats are whoever is here and awake; a late knight sits at the gate.
        self.seats
            .retain(|id, _| heroes.get(id).is_some_and(|h| !h.away && !h.stone));
        let late: Vec<u32> = heroes
            .iter()
            .filter(|(id, h)| !h.away && !h.stone && !self.seats.contains_key(id))
            .map(|(&id, _)| id)
            .collect();
        for id in late {
            let at = self.elapsed;
            self.seat(id, at);
            say.push("door_pass");
        }
        if self.seats.len() as u8 > self.scaled {
            self.provision();
        }
        let input_of = |id: u32| {
            inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default()
        };
        self.fx.iter_mut().for_each(|f| f.t = f.t.saturating_sub(1));
        self.fx.retain(|f| f.t > 0);
        if let Some(a) = &mut self.announce {
            a.t = a.t.saturating_sub(1);
            if a.t == 0 {
                self.announce = None;
            }
        }
        match self.phase {
            Phase::Select => {
                self.select_step(&input_of, say);
                return;
            }
            Phase::Finale => {
                self.finale_step(say);
                return;
            }
            Phase::Play => {}
        }
        // Moving platforms, and each raised shield a friend may stand on.
        let mut now = layout::movers(self.elapsed);
        let mut before = layout::movers(self.elapsed.saturating_sub(1));
        if !self.boss.defeated() {
            now.push(Mover {
                id: BOSS_ID,
                p: self.boss.plank(false),
            });
            before.push(Mover {
                id: BOSS_ID,
                p: self.boss.plank(true),
            });
        }
        let shields: Vec<Mover> = self
            .seats
            .iter()
            .filter_map(|(&id, k)| k.body.shield(SHIELD_ID + id as u16))
            .collect();
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for &id in &ids {
            let Some(mut k) = self.seats.remove(&id) else {
                continue;
            };
            let mut plank_now = now.clone();
            let mut plank_before = before.clone();
            for s in &shields {
                if s.id != SHIELD_ID + id as u16 {
                    plank_now.push(*s);
                    plank_before.push(*s);
                }
            }
            self.step_knight(id, &mut k, input_of(id), knobs, &plank_now, &plank_before, say);
            self.seats.insert(id, k);
        }
        self.step_things(knobs, say);
        self.step_rescue(say);
        self.step_doors(say);
        // The camera of every seat.
        for k in self.seats.values_mut() {
            let (fx, fy) = k.body.foot();
            let (x, y, face, vx, vy) = match k.bubble {
                Some(b) => (b.x.div_euclid(Q), b.y.div_euclid(Q) + 10, 1, 0, 0),
                None => (fx, fy, k.body.face, k.body.vx, k.body.vy),
            };
            if k.pit == 0 {
                k.cam = follow(k.cam, x, y, face, vx, vy);
            }
        }
        // Everyone down at once: the checkpoint takes them back.
        if !self.seats.is_empty() && self.seats.values().all(|k| !k.alive()) {
            self.wipe += 1;
            if self.wipe >= WIPE_TICKS {
                self.wipe = 0;
                self.retry(say);
            }
        } else {
            self.wipe = 0;
        }
        if self.banners == 0xFF && self.phase == Phase::Play && self.announce.is_none() {
            self.start_finale(say);
        }
    }

    /// Everyone back at the latest gate, mended a little, the world as it is.
    fn retry(&mut self, say: &mut Vec<&'static str>) {
        let (x, y) = self.station();
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for (n, id) in ids.iter().enumerate() {
            let off = ((n as i32 + 1) / 2) * 7 * if n % 2 == 0 { 1 } else { -1 };
            if let Some(k) = self.seats.get_mut(id) {
                let joined = k.joined;
                let coins = k.coins;
                let score = k.score;
                let pips = k.pips.max(WIPE_PIPS);
                *k = Knight::at(joined, x + off, y);
                k.coins = coins;
                k.score = score;
                k.pips = pips;
                k.hurt = HURT_GRACE;
                k.cam = follow(k.cam, x, y, 1, 0, 0);
            }
        }
        self.boulders.clear();
        self.bolts.clear();
        say.push("door_open");
    }

    // ------------------------------------------------------------ a knight

    #[allow(clippy::too_many_arguments)]
    fn step_knight(
        &mut self,
        id: u32,
        k: &mut Knight,
        input: Input,
        knobs: &Knobs,
        now: &[Mover],
        before: &[Mover],
        say: &mut Vec<&'static str>,
    ) {
        k.hurt = k.hurt.saturating_sub(1);
        k.cooldown = k.cooldown.saturating_sub(1);
        k.shock = k.shock.saturating_sub(1);
        k.landed = k.landed.saturating_add(1);
        // A knight in a bubble steers it; the rest of the tick is for friends.
        if let Some(mut b) = k.bubble {
            b.t += 1;
            let (dx, dy) = (i32::from(input.move_x), i32::from(input.move_y));
            b.x += dx * 190;
            b.y += dy * 150 - 40;
            b.x = b.x.clamp(8 * TQ / 16, (WIDTH * TILE - 8) * Q);
            b.y = b.y.clamp(0, (HEIGHT * TILE) * Q);
            if b.t >= BUBBLE_TICKS {
                // Too long alone: the gate takes the knight back.
                let (x, y) = self.station();
                let joined = k.joined;
                let (coins, score) = (k.coins, k.score);
                *k = Knight::at(joined, x, y);
                k.coins = coins;
                k.score = score;
                k.pips = BACK_PIPS;
                k.hurt = HURT_GRACE;
                k.cam = follow(k.cam, x, y, 1, 0, 0);
                say.push("door_pass");
            } else {
                k.bubble = Some(b);
            }
            let (_, bits) = k.cmd(input);
            k.prev = bits;
            return;
        }
        // Out of sight in a pit until the safe ledge takes it back.
        if k.pit > 0 {
            k.pit -= 1;
            if k.pit == 0 {
                let (x, y) = k.safe;
                k.body.place(x, y);
                k.hurt = HURT_GRACE;
                k.cam = follow(k.cam, x, y, k.body.face, 0, 0);
            }
            let (_, bits) = k.cmd(input);
            k.prev = bits;
            return;
        }
        if k.pole > 0 {
            self.step_pole(k, say);
            return;
        }
        let (mut cmd, bits) = k.cmd(input);
        if k.pose > 0 {
            // The item-get pose: held still, the world goes on.
            k.pose -= 1;
            cmd = Default::default();
        }
        k.body.big = k.big();
        let was = k.body.y;
        let mut evs = Vec::new();
        {
            let view = View {
                world: world(),
                changed: &self.changed,
            };
            k.body.step(cmd, knobs, &view, now, before, &mut evs);
        }
        for e in evs {
            self.body_event(id, k, e, knobs, say);
        }
        if k.pose == 0 {
            self.touch(id, k, bits, was, say);
        }
        // Landing clears the stomp chain.
        if k.body.ground {
            k.chain = 0;
        }
        // The last safe footing: still on plain ground, not by a hazard.
        self.note_safe(k);
        k.prev = bits;
    }

    fn note_safe(&mut self, k: &mut Knight) {
        let b = &k.body;
        let (fx, fy) = b.foot();
        let g = self.view();
        let ok = b.ground
            && b.plank.is_none()
            && b.mode == Move::Ground
            && g.tile((fx - 7).div_euclid(TILE), fy.div_euclid(TILE)).solid()
            && g.tile((fx + 7).div_euclid(TILE), fy.div_euclid(TILE)).solid()
            && !matches!(g.tile(fx.div_euclid(TILE), (fy - 1).div_euclid(TILE)), Tile::Spikes);
        if ok {
            k.safe_t += 1;
            if k.safe_t >= 8 {
                k.safe = (fx, fy);
            }
        } else {
            k.safe_t = 0;
        }
    }

    fn body_event(
        &mut self,
        id: u32,
        k: &mut Knight,
        e: Ev,
        knobs: &Knobs,
        say: &mut Vec<&'static str>,
    ) {
        let (fx, fy) = k.body.foot();
        match e {
            Ev::Jump(kind) => match kind {
                Jump::Second => say.push("swing"),
                Jump::Triple => say.push("spell_cast"),
                Jump::Long | Jump::Back | Jump::Kick => say.push("roll"),
                Jump::Spring => say.push("shield_up"),
                _ => {}
            },
            Ev::Land { fell } => {
                k.landed = 0;
                if fell > 1700 {
                    self.fx(FxKind::Dust, fx, fy);
                    say.push("step_stone");
                }
            }
            Ev::Skid => self.fx(FxKind::Dust, fx, fy),
            Ev::Spring => {}
            Ev::Hatch => {
                let tile = (fx.div_euclid(TILE), fy.div_euclid(TILE));
                if let Some(to) = layout::hatch_to(tile) {
                    let (x, y) = cell_feet(to);
                    k.body.place(x, y);
                    k.cam = follow(k.cam, x, y, k.body.face, 0, 0);
                    k.safe = (x, y);
                    say.push("door_pass");
                }
            }
            Ev::Bump { x, y } => self.bump(id, k, x, y, say),
            Ev::Slam { x, y } => {
                k.shock = 10;
                k.chain = 0;
                self.slam(id, k, x, y, knobs.slam, say);
            }
        }
    }

    /// A head struck the tile `(tx, ty)` from below.
    fn bump(&mut self, id: u32, k: &mut Knight, tx: i32, ty: i32, say: &mut Vec<&'static str>) {
        let t = self.tile(tx, ty);
        let (cx, top) = (tx * TILE + TILE / 2, ty * TILE);
        match t {
            Tile::Coin => {
                self.set_tile(tx, ty, Tile::Spent);
                self.coin(k, say);
                self.fx(FxKind::Coin, cx, top);
                self.fx(FxKind::Bump, cx, top);
                say.push("clang");
            }
            Tile::Many(n) => {
                self.set_tile(tx, ty, if n > 1 { Tile::Many(n - 1) } else { Tile::Spent });
                self.coin(k, say);
                self.fx(FxKind::Coin, cx, top);
                self.fx(FxKind::Bump, cx, top);
                say.push("clang");
            }
            Tile::Power => {
                self.set_tile(tx, ty, Tile::Spent);
                let kind = if k.size == Size::Small {
                    ItemKind::Loaf
                } else {
                    ItemKind::Flame
                };
                let mut item = Item::new(kind, cx, top + TILE);
                item.dir = if id % 2 == 0 { -1 } else { 1 };
                self.items.push(item);
                self.fx(FxKind::Bump, cx, top);
                say.push("clang");
            }
            Tile::Brick => {
                if k.big() {
                    self.set_tile(tx, ty, Tile::Air);
                    for d in [-5, 5] {
                        self.fx(FxKind::Debris, cx + d, top + 8);
                    }
                    say.push("crack_open");
                } else {
                    self.fx(FxKind::Bump, cx, top);
                    say.push("clang");
                }
            }
            _ => return,
        }
        // Whatever stood on the block is thrown off it.
        for f in &mut self.foes {
            let (l, _, r, b) = f.rect();
            if f.alive() && b == top && l < tx * TILE + TILE && r > tx * TILE {
                f.defeat();
                k.score += 100;
            }
        }
        for i in &mut self.items {
            let (l, _, r, b) = i.rect();
            if b == top && l < tx * TILE + TILE && r > tx * TILE && i.kind == ItemKind::Cap {
                i.vy = -1500;
            }
        }
    }

    fn coin(&mut self, k: &mut Knight, say: &mut Vec<&'static str>) {
        k.coins += 1;
        k.score += 10;
        say.push("gold");
        if k.coins % 20 == 0 {
            k.pips = (k.pips + 1).min(PIPS);
        }
        if k.coins % 100 == 0 {
            // A hundred sparks is a reward: the whole pie.
            k.pips = PIPS;
            k.score += 500;
            say.push("wish_grant");
        }
    }

    /// A slam landed at `(x, y)` pixels: the floor cracks, small foes tumble.
    fn slam(
        &mut self,
        _id: u32,
        k: &mut Knight,
        x: i32,
        y: i32,
        radius: i32,
        say: &mut Vec<&'static str>,
    ) {
        self.fx(FxKind::Ring, x, y);
        say.push("rock_land");
        let phase = self.boss.phase;
        if self.boss.slam(x, y) {
            k.score += 500;
            self.fx(FxKind::Debris, self.boss.x, y - 8);
            self.fx(FxKind::Gleam, self.boss.x, y - 14);
            say.push(if self.boss.defeated() {
                "boss_death"
            } else if self.boss.phase > phase {
                "boss_rage"
            } else {
                "boss_roar"
            });
        } else if !self.boss.defeated() {
            // Not the moment: the shock rings off his head.
            let head = self.boss.plank(false);
            if (x - self.boss.x).abs() <= 24 && (y - head.y).abs() <= 3 {
                say.push("shield_block");
            }
        }
        let row = y.div_euclid(TILE);
        let (c0, c1) = (
            (x - radius).div_euclid(TILE),
            (x + radius).div_euclid(TILE),
        );
        let mut broke = false;
        for ty in row..=row + 1 {
            for tx in c0..=c1 {
                if self.tile(tx, ty) == Tile::Cracked {
                    let centre = tx * TILE + TILE / 2;
                    if (centre - x).abs() <= radius + TILE / 2 {
                        self.set_tile(tx, ty, Tile::Air);
                        for d in [-4, 4] {
                            self.fx(FxKind::Debris, centre + d, ty * TILE + 8);
                        }
                        broke = true;
                    }
                }
            }
        }
        if broke {
            say.push("crack_open");
        }
        for f in &mut self.foes {
            let (l, _, r, b) = f.rect();
            let cx = (l + r) / 2;
            if f.alive() && (cx - x).abs() <= radius && (b - y).abs() <= 8 {
                f.defeat();
                k.score += 200;
                say.push("kill");
            }
        }
        for it in &mut self.items {
            if it.kind == ItemKind::Cap && it.cap == CapState::Slide {
                let (l, _, r, b) = it.rect();
                if ((l + r) / 2 - x).abs() <= radius && (b - y).abs() <= 8 {
                    it.cap = CapState::Rest;
                    it.vx = 0;
                }
            }
        }
    }

    // --------------------------------------------------------- what a knight touches

    fn touch(&mut self, id: u32, k: &mut Knight, bits: u8, was: i32, say: &mut Vec<&'static str>) {
        let me = px(&k.body);
        let (fx, fy) = k.body.foot();
        let i = info();
        let (wp, hp) = (WIDTH * TILE, HEIGHT * TILE);
        // Off the bottom of the world.
        if fy > hp + 24 || fx < -8 || fx > wp + 8 {
            self.fall(k, say);
            return;
        }
        // Water and spikes.
        {
            let g = self.view();
            let deep = (me.0 + 1, me.1, me.2 - 1, me.3 - 4);
            let mut drowned = false;
            let mut pricked = false;
            for ty in deep.1.div_euclid(TILE)..=(me.3 - 1).div_euclid(TILE) {
                for tx in me.0.div_euclid(TILE)..=(me.2 - 1).div_euclid(TILE) {
                    match g.tile(tx, ty) {
                        Tile::Water => {
                            let cell = (tx * TILE, ty * TILE, tx * TILE + TILE, ty * TILE + TILE);
                            drowned |= overlap(deep, cell);
                        }
                        Tile::Spikes => {
                            let cell = (
                                tx * TILE + 3,
                                ty * TILE + 6,
                                tx * TILE + TILE - 3,
                                ty * TILE + TILE,
                            );
                            pricked |= overlap(me, cell);
                        }
                        _ => {}
                    }
                }
            }
            if drowned {
                self.fall(k, say);
                return;
            }
            if pricked && k.hit(-k.body.face) {
                say.push("knight_hit");
                self.after_hit(k);
            }
        }
        // Gold sparks lying about.
        for (n, &c) in i.coins.iter().enumerate() {
            let n16 = n as u16;
            if self.taken.contains(&n16) {
                continue;
            }
            let (cx, cy) = cell_centre(c);
            if overlap(me, (cx - 5, cy - 5, cx + 5, cy + 5)) {
                self.taken.insert(n16);
                self.coin(k, say);
                self.fx(FxKind::Coin, cx, cy);
            }
        }
        // The ring of eight.
        for (n, &c) in i.sparks.iter().enumerate() {
            if self.sparks & (1 << n) != 0 {
                continue;
            }
            let (cx, cy) = cell_centre(c);
            if overlap(me, (cx - 7, cy - 7, cx + 7, cy + 7)) {
                self.sparks |= 1 << n;
                self.fx(FxKind::Gleam, cx, cy);
                say.push("spell_cast");
                if self.sparks == 0xFF {
                    self.award(6, Some(k), say);
                }
            }
        }
        // Banners lying about.
        for b in 0..8usize {
            if self.banners & (1 << b) != 0 {
                continue;
            }
            let Some(c) = i.banners[b] else { continue };
            // The last waits for the crown to fall.
            if b == 7 && !self.crown_down() {
                continue;
            }
            let (cx, cy) = cell_centre(c);
            if overlap(me, (cx - 9, cy - 12, cx + 9, cy + 12)) {
                self.award(b as u8, Some(k), say);
            }
        }
        // Gates.
        for (n, &c) in i.gates.iter().enumerate() {
            let (cx, cy) = cell_centre(c);
            if overlap(me, (cx - 8, cy - 24, cx + 8, cy + 8)) {
                if self.gates & (1 << n) == 0 {
                    self.gates |= 1 << n;
                    self.fx(FxKind::Gleam, cx, cy);
                    say.push("shield_up");
                }
                if (n as u8) + 1 > self.checkpoint {
                    self.checkpoint = n as u8 + 1;
                }
            }
        }
        // The flagpole.
        let (pcx, pcy) = i.pole;
        let (pole_x, base) = (pcx * TILE + TILE / 2, (pcy + 1) * TILE);
        if k.pole == 0
            && !k.poled
            && (fx - pole_x).abs() <= 6
            && me.3 > base - 9 * TILE
            && me.1 < base
        {
            let height = ((base - fy) / TILE).clamp(0, 9);
            k.grip = height;
            k.pole = 1;
            k.score += [100, 200, 400, 800, 1000, 2000, 4000, 5000, 8000, 10000][height as usize];
            k.body.x = (pole_x - 6 * i32::from(k.body.face)) * Q;
            k.body.vx = 0;
            k.body.vy = 0;
            k.body.mode = Move::Air;
            k.body.ground = false;
            k.body.plank = None;
            say.push("wish_grant");
            if self.banners & 4 == 0 {
                self.banners |= 4;
                self.announce = Some(Announce {
                    banner: 2,
                    t: ANNOUNCE_TICKS,
                });
            }
        }
        // The courtyard's door and the board: E at either.
        if k.use_edge(bits) && k.free() {
            let (ax, ay) = cell_centre(i.arch);
            let near_arch = (fx - ax).abs() < 26 && (fy - ay).abs() < 24;
            let (bx, by) = cell_centre(i.board);
            let near_board = (fx - bx).abs() < 30 && (fy - by).abs() < 24;
            let near_gate = i.gates.iter().any(|&g| {
                let (gx, gy) = cell_centre(g);
                (fx - gx).abs() < 20 && (fy - gy).abs() < 24
            });
            if (near_arch && self.doors & 1 != 0) || near_board || near_gate {
                self.open_board(say);
            }
        }
        // The board shows itself to the first knight to come in from the hub.
        if !self.seen && self.doors & 1 != 0 {
            let (bx, by) = cell_centre(i.board);
            if (fx - bx).abs() < 22 && (fy - by).abs() < 24 {
                self.open_board(say);
            }
        }
        self.touch_things(id, k, bits, was, say);
    }

    /// The knight went into a pit or the water: a wedge, and back to the ledge.
    fn fall(&mut self, k: &mut Knight, say: &mut Vec<&'static str>) {
        if k.pit > 0 {
            return;
        }
        k.pips = k.pips.saturating_sub(1);
        k.chain = 0;
        k.carry = false;
        say.push("hit");
        self.fx(FxKind::Smoke, k.body.foot().0, k.body.foot().1.min(HEIGHT * TILE - 8));
        if k.pips == 0 {
            self.go_down(k);
        } else {
            k.pit = PIT_TICKS;
            k.body.vx = 0;
            k.body.vy = 0;
        }
    }

    /// After a hit that took the last wedge, a bubble.
    fn after_hit(&mut self, k: &mut Knight) {
        if k.pips == 0 {
            self.go_down(k);
        }
    }

    fn go_down(&mut self, k: &mut Knight) {
        let (fx, fy) = k.body.foot();
        k.bubble = Some(Bubble {
            x: fx * Q,
            y: (fy.min(HEIGHT * TILE - 24) - 14) * Q,
            t: 0,
        });
        k.size = Size::Small;
        k.body.big = false;
        k.carry = false;
        k.pit = 0;
        k.pole = 0;
        k.pose = 0;
    }

    /// A banner is raised for the whole table.
    pub fn award(&mut self, banner: u8, who: Option<&mut Knight>, say: &mut Vec<&'static str>) {
        if self.banners & (1 << banner) != 0 {
            return;
        }
        self.banners |= 1 << banner;
        self.announce = Some(Announce {
            banner,
            t: ANNOUNCE_TICKS,
        });
        say.push("wish_grant");
        if let Some(k) = who {
            k.pose = POSE_TICKS;
            k.score += 1000;
            let (fx, fy) = k.body.foot();
            self.fx(FxKind::Gleam, fx, fy - 24);
        }
    }

    /// Whether King Cinder's crown has fallen: his banner may be taken.
    pub fn crown_down(&self) -> bool {
        self.boss.defeated()
    }

    // ------------------------------------------------------------ the board

    fn open_board(&mut self, say: &mut Vec<&'static str>) {
        if self.phase != Phase::Play {
            return;
        }
        self.phase = Phase::Select;
        self.phase_t = 0;
        self.seen = true;
        // The line of the first banner not yet raised.
        self.cursor = BOARD
            .iter()
            .position(|&(_, b)| self.banners & (1 << b) == 0)
            .unwrap_or(0) as u8;
        say.push("play_card");
    }

    fn select_step(&mut self, input_of: &dyn Fn(u32) -> Input, say: &mut Vec<&'static str>) {
        self.phase_t = (self.phase_t + 1).min(LIMIT);
        let mut go = false;
        // Any seat may move the cursor or take the choice; the lowest seat
        // that presses wins the tick.
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let i = input_of(id);
            let Some(k) = self.seats.get_mut(&id) else { continue };
            let (_, bits) = k.cmd(i);
            let press = |b: u8| bits & b != 0 && k.prev & b == 0;
            if press(super::knight::B_DOWN) {
                self.cursor = (self.cursor + 1) % BOARD.len() as u8;
                say.push("card_pickup");
            } else if i.move_y < 0 && k.prev & super::knight::B_JUMP == 0 {
                self.cursor = (self.cursor + BOARD.len() as u8 - 1) % BOARD.len() as u8;
                say.push("card_pickup");
            }
            if press(super::knight::B_BRACE) {
                go = true;
            }
            k.prev = bits;
        }
        if self.phase_t >= SELECT_TICKS {
            go = true;
        }
        if !go {
            return;
        }
        let line = usize::from(self.cursor);
        // The summit waits for its door.
        if line == 5 && self.doors & 2 == 0 {
            self.phase_t = 0;
            say.push("mana_empty");
            return;
        }
        let gate = if line == 4 { self.spark_gate() } else { BOARD_GATE[line] };
        self.checkpoint = gate as u8 + 1;
        self.gates |= 1 << gate;
        let (x, y) = self.station();
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for (n, id) in ids.iter().enumerate() {
            let off = ((n as i32 + 1) / 2) * 7 * if n % 2 == 0 { 1 } else { -1 };
            if let Some(k) = self.seats.get_mut(id) {
                if k.bubble.is_some() {
                    k.pips = BACK_PIPS;
                }
                k.bubble = None;
                k.pit = 0;
                k.pole = 0;
                k.body.place(x + off, y);
                k.hurt = HURT_GRACE;
                k.safe = (x + off, y);
                k.cam = follow(k.cam, x, y, 1, 0, 0);
            }
        }
        self.boulders.clear();
        self.bolts.clear();
        self.phase = Phase::Play;
        self.phase_t = 0;
        say.push("door_open");
    }

    fn start_finale(&mut self, say: &mut Vec<&'static str>) {
        self.phase = Phase::Finale;
        self.phase_t = 0;
        let (fx, fy) = cell_feet(info().finale);
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for (n, id) in ids.iter().enumerate() {
            let off = (n as i32 - ids.len() as i32 / 2) * 14;
            if let Some(k) = self.seats.get_mut(id) {
                k.bubble = None;
                k.pit = 0;
                k.pole = 0;
                k.pose = 0;
                k.body.place(fx + off, fy);
                k.body.face = if n % 2 == 0 { 1 } else { -1 };
                k.safe = (fx + off, fy);
                k.cam = follow(k.cam, fx, fy, 1, 0, 0);
            }
        }
        self.bolts.clear();
        self.boulders.clear();
        say.push("play_card");
    }

    fn finale_step(&mut self, say: &mut Vec<&'static str>) {
        self.phase_t = (self.phase_t + 1).min(LIMIT);
        let t = self.phase_t;
        if t == 30 || t == 60 || t == 90 {
            say.push("shield_up");
        }
        if t == 120 {
            say.push("wish_grant");
            let (px, py) = cell_centre(info().great_pole);
            self.fx(FxKind::Gleam, px, py - 150);
        }
        for k in self.seats.values_mut() {
            let (fx, fy) = k.body.foot();
            k.cam = follow(k.cam, fx, fy, k.body.face, 0, 0);
        }
        if t >= FINALE_TICKS && self.completed.is_none() {
            self.completed = Some(self.elapsed);
            say.push("play_card");
        }
    }

    // ------------------------------------------------------------ doors

    fn step_doors(&mut self, say: &mut Vec<&'static str>) {
        if self.doors & 1 == 0 && self.banners & 3 == 3 {
            self.doors |= 1;
            self.open_gates(0);
            say.push("door_open");
        }
        if self.doors & 2 == 0 && self.door_count() >= SUMMIT_BANNERS {
            self.doors |= 2;
            self.open_gates(1);
            say.push("door_open");
        }
    }

    fn open_gates(&mut self, which: u8) {
        let w = world();
        for ty in 0..HEIGHT {
            for tx in 0..WIDTH {
                if w.at(tx, ty) == Tile::Gate(which) {
                    self.set_tile(tx, ty, Tile::Air);
                }
            }
        }
    }

    // --------------------------------------------------------- the pole

    fn step_pole(&mut self, k: &mut Knight, say: &mut Vec<&'static str>) {
        let (_, pcy) = info().pole;
        let base = (pcy + 1) * TILE * Q;
        k.pole += 1;
        k.body.vx = 0;
        if k.body.y < base {
            k.body.y = (k.body.y + 640).min(base);
            if k.body.y == base {
                k.body.ground = true;
                k.body.mode = Move::Ground;
                k.body.vy = 0;
                k.pole = k.pole.max(40);
            }
        } else {
            // Down: let go, face the way on, and walk off to the fanfare.
            if k.pole >= 60 && k.pole < 100 {
                k.body.face = 1;
                k.body.x += 560;
                let g = self.view();
                let (fx, fy) = k.body.foot();
                if g.tile((fx + 8).div_euclid(TILE), (fy - 4).div_euclid(TILE)).solid() {
                    k.body.x -= 560;
                }
            }
            if k.pole == 60 {
                say.push("door_pass");
            }
            if k.pole >= 100 {
                k.pole = 0;
                k.poled = true;
                k.body.mode = Move::Ground;
            }
        }
    }

    // ------------------------------------------------------ foes and things

    fn step_things(&mut self, knobs: &Knobs, say: &mut Vec<&'static str>) {
        let _ = knobs;
        let near: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|k| k.alive())
            .map(|k| k.body.foot())
            .collect();
        // A foe whose pop has run its course is gone for good.
        self.foes.retain(|f| f.dead != 1);
        {
            let view = View {
                world: world(),
                changed: &self.changed,
            };
            for f in &mut self.foes {
                let fx = f.x.div_euclid(Q);
                let fy = f.y.div_euclid(Q);
                let close = near
                    .iter()
                    .any(|&(x, y)| (x - fx).abs() < 340 && (y - fy).abs() < 260);
                f.step(&view, close);
            }
            for i in &mut self.items {
                i.step(&view);
            }
            let mut spent = Vec::new();
            for (n, b) in self.bolts.iter_mut().enumerate() {
                if !b.step(&view) {
                    spent.push(n);
                }
            }
            for n in spent.into_iter().rev() {
                self.bolts.remove(n);
            }
        }
        // Things that fell off the world.
        let limit = (HEIGHT * TILE + 64) * Q;
        self.foes.retain(|f| f.y < limit);
        self.items.retain(|i| i.y < limit);
        // Bolts meet foes.
        let mut kills = Vec::new();
        for (bn, b) in self.bolts.iter().enumerate() {
            for (fnn, f) in self.foes.iter().enumerate() {
                if f.alive() && overlap(b.rect(), f.rect()) {
                    kills.push((bn, fnn));
                }
            }
        }
        for (bn, fnn) in kills {
            if self.foes[fnn].alive() {
                self.foes[fnn].defeat();
                let (x, y) = (self.foes[fnn].x.div_euclid(Q), self.foes[fnn].y.div_euclid(Q));
                self.fx(FxKind::Pop, x, y);
                say.push("kill");
                if let Some(k) = self.seats.get_mut(&self.bolts[bn].owner) {
                    k.score += 100;
                }
            }
            self.bolts[bn].t = BOLT_LIFE + 1;
        }
        self.bolts.retain(|b| b.t <= BOLT_LIFE);
        // A cap on the slide flips whatever it meets.
        let mut flipped = Vec::new();
        for it in &self.items {
            if it.kind == ItemKind::Cap && it.cap == CapState::Slide {
                for (n, f) in self.foes.iter().enumerate() {
                    if f.alive() && overlap(it.rect(), f.rect()) {
                        flipped.push(n);
                    }
                }
            }
        }
        for n in flipped {
            if self.foes[n].alive() {
                self.foes[n].defeat();
                let (x, y) = (self.foes[n].x.div_euclid(Q), self.foes[n].y.div_euclid(Q));
                self.fx(FxKind::Pop, x, y);
                say.push("kill");
            }
        }
        // King Cinder, while a knight is on the summit.
        let feet: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|k| k.alive())
            .map(|k| k.body.foot())
            .collect();
        let seats = self.seats.len().max(1) as u8;
        let beat = self.boss.beat;
        self.boss.step(&feet, seats, self.elapsed);
        if beat == BossBeat::Dormant && self.boss.beat != BossBeat::Dormant {
            say.push("boss_rise");
        }
        if beat == BossBeat::Rest && self.boss.beat == BossBeat::Tell {
            say.push("boss_rage");
        }
        if matches!(self.boss.beat, BossBeat::Strike) && beat == BossBeat::Tell {
            say.push("rock_land");
        }
        self.step_boulders(&feet, say);
    }

    /// The bell boulders on the snow ledge: they roll down it toward the
    /// knights on a beat, faster and more of them for a fuller table.
    fn step_boulders(&mut self, feet: &[(i32, i32)], say: &mut Vec<&'static str>) {
        let (lo, hi) = ((SNOW.ox + 6) * TILE, (SNOW.ox + 97) * TILE);
        let seats = self.seats.len().max(1) as u32;
        let interval = 300u32.saturating_sub(24 * (seats - 1)).max(180);
        let speed = 800 + 32 * (seats as i32 - 1);
        let someone = feet.iter().any(|&(x, y)| {
            x > lo - 80 && x < hi + 80 && y > SNOW.oy * TILE && y < (SNOW.oy + 14) * TILE
        });
        for b in &mut self.boulders {
            b.x -= speed;
            b.t += 1;
        }
        self.boulders.retain(|b| b.x > (SNOW.ox + 4) * TILE * Q);
        if someone && self.phase == Phase::Play {
            let phase = self.elapsed % interval;
            let second = seats >= 4 && phase == 80;
            // The bell tolls shortly before one rolls out.
            if phase == interval - 36 || (seats >= 4 && phase == 80 - 36) {
                say.push("circle_toll");
            }
            if (phase == 0 || second) && self.boulders.len() < 4 {
                self.boulders.push(Boulder {
                    x: hi * Q,
                    t: 0,
                });
            }
        } else if !someone {
            self.boulders.clear();
        }
    }

    /// A knight meets the foes, the items and the caps.
    fn touch_things(
        &mut self,
        id: u32,
        k: &mut Knight,
        bits: u8,
        was: i32,
        say: &mut Vec<&'static str>,
    ) {
        let me = px(&k.body);
        let (fx, _fy) = k.body.foot();
        // Foes: a stomp, a slam, a dive or a belly slide beats them; a touch
        // from the side hurts.
        for n in 0..self.foes.len() {
            if !self.foes[n].alive() {
                continue;
            }
            let fr = self.foes[n].rect();
            if !overlap(me, fr) {
                continue;
            }
            // From above: the feet were over its head a tick ago.
            let falling = k.body.vy > 0 && was <= (fr.1 + 4) * Q;
            let striking = matches!(k.body.mode, Move::Pound | Move::Dive | Move::Belly);
            if striking || falling {
                let kind = self.foes[n].kind;
                let (cx, cy) = ((fr.0 + fr.2) / 2, fr.3);
                self.foes[n].defeat();
                self.fx(FxKind::Pop, cx, cy);
                say.push("kill");
                let value = 100 << k.chain.min(6);
                k.score += value;
                if falling && !striking {
                    k.chain = k.chain.saturating_add(1);
                    k.body.bounce(bits & super::knight::B_JUMP != 0);
                }
                if kind == FoeKind::Capped && !striking {
                    let mut cap = Item::new(ItemKind::Cap, cx, cy);
                    cap.dir = if k.body.face > 0 { 1 } else { -1 };
                    self.items.push(cap);
                }
            } else {
                let from = if self.foes[n].x.div_euclid(Q) > fx { 1 } else { -1 };
                if k.hit(-from) {
                    say.push("knight_hit");
                    self.after_hit(k);
                }
            }
        }
        // Bell boulders: a hit from the side, a springboard from above.
        let floor = layout::SNOW_FLOOR * TILE;
        for n in 0..self.boulders.len() {
            let bx = self.boulders[n].x.div_euclid(Q);
            let r = (bx - 14, floor - 28, bx + 14, floor);
            if overlap(me, r) {
                if k.body.vy > 0 && was <= (r.1 + 4) * Q {
                    k.body.bounce(bits & super::knight::B_JUMP != 0);
                    say.push("clang");
                } else if k.hit(1) {
                    say.push("knight_hit_big");
                    self.after_hit(k);
                }
            }
        }
        // King Cinder: his waves and embers hurt, and so does his side.
        if self.boss.beat != BossBeat::Dormant && !self.boss.defeated() {
            let mut hurt = false;
            for h in &self.boss.hazards {
                hurt |= h.hurts() && overlap(me, h.rect());
            }
            let body = self.boss.body();
            if self.boss.beat != BossBeat::Rage && overlap(me, body) {
                hurt = true;
            }
            if hurt {
                let from = if self.boss.x > fx { 1 } else { -1 };
                if k.hit(-from) {
                    say.push("knight_hit_big");
                    self.after_hit(k);
                }
            }
        }
        // Items.
        let mut gone = Vec::new();
        for n in 0..self.items.len() {
            let it = self.items[n].clone();
            let r = it.rect();
            match it.kind {
                ItemKind::Loaf | ItemKind::Flame => {
                    if !it.rising() && overlap(me, r) {
                        gone.push(n);
                        say.push("potion");
                        k.score += 1000;
                        match (it.kind, k.size) {
                            (ItemKind::Loaf, Size::Small) | (ItemKind::Flame, Size::Small) => {
                                k.size = Size::Big;
                                k.body.big = true;
                                let view = self.view();
                                k.body.regrow(&view);
                            }
                            (ItemKind::Flame, _) => k.size = Size::Flame,
                            (ItemKind::Loaf, _) => k.pips = (k.pips + 2).min(PIPS),
                            _ => {}
                        }
                    }
                }
                ItemKind::Cap => self.touch_cap(id, k, n, bits, was, say),
            }
        }
        for n in gone.into_iter().rev() {
            self.items.remove(n);
        }
        // Carried caps ride with their carrier; a long carry wakes it.
        if k.carry {
            let held = self
                .items
                .iter()
                .position(|i| i.kind == ItemKind::Cap && i.cap == CapState::Carried && i.owner == id);
            match held {
                Some(n) => {
                    if bits & super::knight::B_ACT == 0 {
                        // Let go: thrown the way the knight faces.
                        let face = k.body.face;
                        let it = &mut self.items[n];
                        it.cap = CapState::Slide;
                        it.dir = face;
                        it.t = 0;
                        k.carry = false;
                        say.push("swing");
                    } else {
                        let it = &mut self.items[n];
                        it.x = k.body.x + i32::from(k.body.face) * 10 * Q;
                        it.y = k.body.y - 2 * Q;
                        it.t += 1;
                        if it.t > CARRY {
                            self.items.remove(n);
                            k.carry = false;
                            if k.hit(-k.body.face) {
                                say.push("knight_hit");
                                self.after_hit(k);
                            }
                        }
                    }
                }
                None => k.carry = false,
            }
        }
        // A lantern flame throws bolts of light.
        if k.size == Size::Flame
            && k.act_edge(bits)
            && k.cooldown == 0
            && k.free()
            && self.bolts.iter().filter(|b| b.owner == id).count() < BOLT_MOST
        {
            let face = i32::from(k.body.face);
            self.bolts.push(Bolt {
                x: k.body.x + face * 10 * Q,
                y: k.body.y - 12 * Q,
                vx: face * BOLT_SPEED,
                vy: 200,
                t: 0,
                owner: id,
            });
            k.cooldown = 9;
            say.push("shot_spark");
        }
    }

    /// A knight and a cap: kick it, carry it, or stop it.
    fn touch_cap(
        &mut self,
        id: u32,
        k: &mut Knight,
        n: usize,
        bits: u8,
        was: i32,
        say: &mut Vec<&'static str>,
    ) {
        let me = px(&k.body);
        let it = self.items[n].clone();
        if it.cap == CapState::Carried || !overlap(me, it.rect()) {
            return;
        }
        let on_top = k.body.vy > 0 && was <= (it.rect().1 + 4) * Q;
        let from = if it.x.div_euclid(Q) > k.body.foot().0 { 1 } else { -1 };
        match it.cap {
            CapState::Rest => {
                if on_top {
                    // Stomp it away.
                    let c = &mut self.items[n];
                    c.cap = CapState::Slide;
                    c.dir = if k.body.face > 0 { 1 } else { -1 };
                    c.owner = id;
                    c.t = 0;
                    k.body.bounce(bits & super::knight::B_JUMP != 0);
                    say.push("kill");
                } else if bits & super::knight::B_ACT != 0 && !k.carry && k.free() {
                    let c = &mut self.items[n];
                    c.cap = CapState::Carried;
                    c.owner = id;
                    c.t = 0;
                    k.carry = true;
                    say.push("shield_up");
                } else {
                    // Run into it: it flies off the way you ran.
                    let c = &mut self.items[n];
                    c.cap = CapState::Slide;
                    c.dir = from as i8;
                    c.owner = id;
                    c.t = 0;
                    say.push("clang");
                }
            }
            CapState::Slide => {
                if on_top {
                    let c = &mut self.items[n];
                    c.cap = CapState::Rest;
                    c.vx = 0;
                    k.body.bounce(bits & super::knight::B_JUMP != 0);
                    say.push("kill");
                } else if it.owner != id || it.t > 14 {
                    if k.hit(-(from as i8)) {
                        say.push("knight_hit");
                        self.after_hit(k);
                    }
                }
            }
            CapState::Carried => {}
        }
    }

    // ---------------------------------------------------------- the rescue

    fn step_rescue(&mut self, say: &mut Vec<&'static str>) {
        let alive: Vec<(u32, (i32, i32, i32, i32), (i32, i32))> = self
            .seats
            .iter()
            .filter(|(_, k)| k.alive() && k.free())
            .map(|(&id, k)| (id, px(&k.body), k.body.foot()))
            .collect();
        let downed: Vec<(u32, Bubble)> = self
            .seats
            .iter()
            .filter_map(|(&id, k)| k.bubble.map(|b| (id, b)))
            .collect();
        for (id, b) in downed {
            let (bx, by) = (b.x.div_euclid(Q), b.y.div_euclid(Q));
            let hit = alive
                .iter()
                .find(|(_, r, _)| overlap(*r, (bx - 12, by - 12, bx + 12, by + 12)));
            if let Some(&(_, _, (fx, fy))) = hit {
                if let Some(k) = self.seats.get_mut(&id) {
                    k.bubble = None;
                    k.pips = BACK_PIPS;
                    k.hurt = HURT_GRACE;
                    k.body.place(fx, fy);
                    k.safe = (fx, fy);
                    k.cam = follow(k.cam, fx, fy, k.body.face, 0, 0);
                }
                self.fx(FxKind::Gleam, fx, fy - 12);
                say.push("wish_grant");
            }
        }
        // A bubble with no friend near drifts toward the nearest one, slowly.
        for (&id, k) in self.seats.iter_mut() {
            let _ = id;
            if let Some(b) = &mut k.bubble {
                if let Some(&(_, _, (fx, fy))) = alive.iter().min_by_key(|(_, _, (x, _))| {
                    (x - b.x.div_euclid(Q)).abs()
                }) {
                    b.x += (fx * Q - b.x).clamp(-24, 24);
                    b.y += ((fy - 20) * Q - b.y).clamp(-16, 16);
                }
            }
        }
        let _ = HZ;
    }
}
