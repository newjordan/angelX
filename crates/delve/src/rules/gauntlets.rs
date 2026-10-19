//! Gauntlets: the dungeon's own obstacle courses, built into its passages
//! and its fighting rooms, to be crossed with nerve and a sense of rhythm.
//!
//! - **The axe bridge.** A straight passage falls away into a gorge, and a
//!   bridge two tiles wide runs over it. Pendulum axes hang over the bridge
//!   and swing across it in a cascade. A blade high at the end of its arc
//!   passes over; one coming down among the knights strikes, and throws
//!   whoever it hits the way it swings. Kept to the middle, a knight is only
//!   shoved; at the edge, over they go: a fall costs `FALL_SHARE` of their
//!   health and puts them back where they set out from. A monster on the
//!   bridge is swept clean off it.
//! - **The hammer row.** Iron stampers hang over a passage, a row of them
//!   across its width at each gate, and come down one gate after another,
//!   so a knight who moves with the wave walks through untouched. Each
//!   shudders and glows underneath before it falls; whatever stands under
//!   it is struck and thrown out of its footprint.
//! - **The boulder run.** A lane across a fighting room, worn smooth: a
//!   boulder rumbles in its wall's mouth, then rolls the length of the lane
//!   and bursts on the far stone. It throws aside what it meets, and it
//!   rolls over monsters as gladly as knights, so lure them into its lane.
//! - **The saw rail.** A slot in the floor, a whirling blade that runs it
//!   end to end and back, revving at each end before it sets off. It bites
//!   knights and monsters alike and throws them off its rail.
//!
//! Every gauntlet reads its state from the run's tick and its own phase,
//! nothing else, so the host's picture and every friend's mirror agree;
//! their places are drawn from the floor's own seed and the room's place,
//! never from the run's dice. Each warns before it strikes, each knight's
//! ward after a hit carries them through the next blow, and no fall leaves
//! a body anywhere a friend cannot reach. The numbers are the constants at
//! the top.

use super::*;

/// How likely a passage is to be a gauntlet, and a fighting room to hold
/// one, by depth (1, 2, 3 and below), in percent.
const HALL_ODDS: [u32; 3] = [60, 70, 80];
const ROOM_ODDS: [u32; 3] = [22, 32, 42];
/// How near a doorway a gauntlet in a fighting room may come (units).
const DOOR_CLEAR: f32 = 5.0;

/// The axe bridge: half the bridge's width; how far either side of its
/// middle a blade swings; how low (of that reach) the blade comes among the
/// knights; half the blade's breadth along the bridge.
pub const BRIDGE_HALF: f32 = 2.0;
pub const AXE_REACH: f32 = 3.4;
pub const AXE_LOW: f32 = 0.62;
pub const AXE_EDGE: f32 = 0.9;
/// A swing's whole cycle, out and back, by depth.
pub const AXE_PERIOD: [u32; 3] = [84, 74, 64];
/// How long before the blade comes down the bridge shows its shadow.
pub const AXE_WARN: u32 = 18;
/// What a blade does to a knight (and how far it throws them), and to a
/// monster.
pub const AXE_HARM: u32 = 12;
pub const AXE_KNOCK: f32 = 1.3;
const AXE_FOE_HARM: u32 = 30;
/// What a fall into the gorge costs, in percent of a knight's most health.
pub const FALL_SHARE: u32 = 12;

/// The hammer row: the cycle; when a hammer starts to shudder, when it
/// falls, when it lifts again; half its footprint (it is two tiles square);
/// and what it does.
pub const HAMMER_PERIOD: u32 = 66;
pub const HAMMER_WARN: u32 = 36;
pub const HAMMER_SLAM: u32 = 56;
pub const HAMMER_LIFT: u32 = 62;
pub const HAMMER_HALF: f32 = 2.0;
pub const HAMMER_HARM: u32 = 15;
const HAMMER_FOE_HARM: u32 = 60;

/// The boulder run: the cycle by depth; the rumble before it rolls; how
/// fast it rolls (units a tick); its size; and what it does.
pub const BOULDER_PERIOD: [u32; 3] = [210, 190, 170];
pub const BOULDER_WARN: u32 = 30;
pub const BOULDER_SPEED: f32 = 0.5;
pub const BOULDER_RADIUS: f32 = 1.5;
pub const BOULDER_HARM: u32 = 15;
const BOULDER_FOE_HARM: u32 = 50;

/// The saw rail: how fast the blade runs (units a tick) by depth; how long
/// it revs at each end; its size; what it does a knight, and a monster each
/// `SAW_EVERY` ticks it stays in it.
pub const SAW_SPEED: [f32; 3] = [0.2, 0.24, 0.28];
pub const SAW_REST: u32 = 18;
pub const SAW_RADIUS: f32 = 1.0;
pub const SAW_HARM: u32 = 9;
pub const SAW_KNOCK: f32 = 1.4;
const SAW_FOE_HARM: u32 = 16;
const SAW_EVERY: u64 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gauntlet {
    /// A gorge across a passage, and the bridge over it: its middle line
    /// lies at `line` across the passage, and the gorge runs from `from` to
    /// `to` along it (along y when `upright`, the bridge running north and
    /// south; along x when not).
    Bridge {
        from: f32,
        to: f32,
        line: f32,
        upright: bool,
    },
    /// A pendulum axe over the bridge at `(x, y)`, swinging across it.
    Axe {
        x: f32,
        y: f32,
        upright: bool,
        period: u32,
        phase: u32,
    },
    /// An iron stamper two tiles square, centred at `(x, y)`, in a passage
    /// running north and south when `upright`.
    Hammer {
        x: f32,
        y: f32,
        upright: bool,
        phase: u32,
    },
    /// A boulder's lane along x at `y`, from the wall's mouth at `from` to
    /// the stone it bursts on at `to`.
    Boulder {
        from: f32,
        to: f32,
        y: f32,
        period: u32,
        phase: u32,
    },
    /// A saw's rail from `a` to `b`, and how fast it runs it.
    Saw {
        a: (f32, f32),
        b: (f32, f32),
        speed: f32,
        phase: u32,
    },
}

impl Gauntlet {
    /// Bounded and finite, as every mirrored thing must be.
    pub fn valid(&self) -> bool {
        let ok = |v: f32| v.is_finite() && v.abs() < 1.0e4;
        match *self {
            Gauntlet::Bridge { from, to, line, .. } => ok(from) && ok(to) && ok(line),
            Gauntlet::Axe { x, y, period, .. } => ok(x) && ok(y) && period > 0,
            Gauntlet::Hammer { x, y, .. } => ok(x) && ok(y),
            Gauntlet::Boulder { from, to, y, period, .. } => {
                ok(from) && ok(to) && ok(y) && period > BOULDER_WARN
            }
            Gauntlet::Saw { a, b, speed, .. } => {
                ok(a.0) && ok(a.1) && ok(b.0) && ok(b.1) && speed > 0.01 && speed < 2.0
            }
        }
    }

    /// Where it stands: an axe's pivot over the bridge, a hammer's middle,
    /// a lane's or a rail's middle, the bridge's middle.
    pub fn at(&self) -> (f32, f32) {
        match *self {
            Gauntlet::Bridge { from, to, line, upright } => {
                let mid = (from + to) / 2.0;
                if upright { (line, mid) } else { (mid, line) }
            }
            Gauntlet::Axe { x, y, .. } | Gauntlet::Hammer { x, y, .. } => (x, y),
            Gauntlet::Boulder { from, to, y, .. } => ((from + to) / 2.0, y),
            Gauntlet::Saw { a, b, .. } => ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0),
        }
    }

    /// Ticks into its cycle on this tick of the run.
    pub fn beat(&self, tick: u64) -> u32 {
        let (period, phase) = match *self {
            Gauntlet::Bridge { .. } => return 0,
            Gauntlet::Axe { period, phase, .. } => (period, phase),
            Gauntlet::Hammer { phase, .. } => (HAMMER_PERIOD, phase),
            Gauntlet::Boulder { period, phase, .. } => (period, phase),
            Gauntlet::Saw { phase, .. } => (self.saw_cycle(), phase),
        };
        let period = u64::from(period.max(1));
        ((tick % period + u64::from(phase) % period) % period) as u32
    }

    // ─── the axe ───────────────────────────────────────────────────────────

    /// Where an axe's blade is across the bridge at `tick`, from -1 to 1 of
    /// its reach: 0 is straight down.
    pub fn swing(&self, tick: u64) -> f32 {
        let Gauntlet::Axe { period, .. } = *self else { return 0.0 };
        (std::f32::consts::TAU * self.beat(tick) as f32 / period as f32).sin()
    }

    /// The blade is down among the knights: over the bridge, not high at
    /// the end of its arc.
    pub fn low(&self, tick: u64) -> bool {
        matches!(self, Gauntlet::Axe { .. }) && self.swing(tick).abs() < AXE_LOW
    }

    /// Which way the blade is swinging across the bridge: 1 toward +x (or
    /// +y on a bridge along x), -1 back.
    pub fn heading(&self, tick: u64) -> f32 {
        let Gauntlet::Axe { period, .. } = *self else { return 0.0 };
        let c = (std::f32::consts::TAU * self.beat(tick) as f32 / period as f32).cos();
        if c >= 0.0 { 1.0 } else { -1.0 }
    }

    /// Where the blade is now, in the room.
    pub fn blade(&self, tick: u64) -> (f32, f32) {
        let Gauntlet::Axe { x, y, upright, .. } = *self else { return self.at() };
        let off = AXE_REACH * self.swing(tick);
        if upright { (x + off, y) } else { (x, y + off) }
    }

    /// How near the blade is to coming down (0 to 1), in the last
    /// `AXE_WARN` ticks before it does: the bridge shows its shadow.
    pub fn warning(&self, tick: u64) -> Option<f32> {
        match self {
            Gauntlet::Axe { .. } => {
                if self.low(tick) {
                    return None;
                }
                (1..=AXE_WARN)
                    .find(|&k| self.low(tick + u64::from(k)))
                    .map(|k| 1.0 - (k - 1) as f32 / AXE_WARN as f32)
            }
            Gauntlet::Hammer { .. } => {
                let beat = self.beat(tick);
                (HAMMER_WARN..HAMMER_SLAM)
                    .contains(&beat)
                    .then(|| (beat - HAMMER_WARN) as f32 / (HAMMER_SLAM - HAMMER_WARN) as f32)
            }
            Gauntlet::Boulder { .. } => {
                let beat = self.beat(tick);
                (beat < BOULDER_WARN).then(|| beat as f32 / BOULDER_WARN as f32)
            }
            Gauntlet::Saw { .. } => {
                // It revs at either end before it sets off.
                let (_, resting) = self.saw_at(tick);
                resting
            }
            Gauntlet::Bridge { .. } => None,
        }
    }

    // ─── the hammer ────────────────────────────────────────────────────────

    /// How far a hammer has come down (0 raised high, 1 on the floor).
    pub fn drop(&self, tick: u64) -> f32 {
        if !matches!(self, Gauntlet::Hammer { .. }) {
            return 0.0;
        }
        let beat = self.beat(tick);
        if beat < HAMMER_WARN {
            0.0
        } else if beat < HAMMER_SLAM {
            // It sags a little as it shudders, then falls in the last ticks.
            let k = (beat - HAMMER_WARN) as f32 / (HAMMER_SLAM - HAMMER_WARN) as f32;
            if beat + 3 >= HAMMER_SLAM { 0.15 + 0.85 * (1.0 - (HAMMER_SLAM - beat) as f32 / 3.0) } else { 0.15 * k }
        } else if beat < HAMMER_LIFT {
            1.0
        } else {
            1.0 - (beat - HAMMER_LIFT) as f32 / (HAMMER_PERIOD - HAMMER_LIFT) as f32
        }
    }

    /// The hammer lies on the floor: from its fall until it lifts.
    pub fn down(&self, tick: u64) -> bool {
        matches!(self, Gauntlet::Hammer { .. }) && (HAMMER_SLAM..HAMMER_LIFT).contains(&self.beat(tick))
    }

    // ─── the boulder ───────────────────────────────────────────────────────

    /// Where a boulder is along its lane, while it rolls.
    pub fn boulder(&self, tick: u64) -> Option<(f32, f32)> {
        let Gauntlet::Boulder { from, to, y, .. } = *self else { return None };
        let beat = self.beat(tick);
        if beat < BOULDER_WARN {
            return None;
        }
        let gone = (beat - BOULDER_WARN) as f32 * BOULDER_SPEED;
        let long = (to - from).abs();
        (gone <= long).then(|| (from + gone * (to - from).signum(), y))
    }

    /// The tick the boulder bursts on the far stone.
    fn bursts(&self, tick: u64) -> bool {
        let Gauntlet::Boulder { from, to, .. } = *self else { return false };
        let roll = ((to - from).abs() / BOULDER_SPEED).floor() as u32;
        self.beat(tick) == BOULDER_WARN + roll + 1
    }

    // ─── the saw ───────────────────────────────────────────────────────────

    fn saw_travel(&self) -> u32 {
        let Gauntlet::Saw { a, b, speed, .. } = *self else { return 1 };
        (((b.0 - a.0).hypot(b.1 - a.1) / speed.max(0.01)).ceil() as u32).max(1)
    }

    fn saw_cycle(&self) -> u32 {
        2 * (self.saw_travel() + SAW_REST)
    }

    /// Where the saw's blade is, and how far through a rev at an end it is
    /// (none while it runs).
    pub fn saw_at(&self, tick: u64) -> ((f32, f32), Option<f32>) {
        let Gauntlet::Saw { a, b, .. } = *self else { return (self.at(), None) };
        let travel = self.saw_travel();
        let beat = self.beat(tick);
        let (s, rest) = if beat < SAW_REST {
            (0.0, Some(beat as f32 / SAW_REST as f32))
        } else if beat < SAW_REST + travel {
            ((beat - SAW_REST) as f32 / travel as f32, None)
        } else if beat < 2 * SAW_REST + travel {
            (1.0, Some((beat - SAW_REST - travel) as f32 / SAW_REST as f32))
        } else {
            (1.0 - (beat - 2 * SAW_REST - travel) as f32 / travel as f32, None)
        };
        ((a.0 + (b.0 - a.0) * s, a.1 + (b.1 - a.1) * s), rest)
    }
}

/// The tile under `(x, y)` is a pit, water or lava.
fn over_pit(room: &Room, x: f32, y: f32) -> bool {
    room.tile((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32) == Tile::Hazard
}

/// Stone a thrown body stops against (the gorge does not stop it).
fn stops(room: &Room, x: f32, y: f32) -> bool {
    matches!(
        room.tile((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32),
        Tile::Wall | Tile::Block | Tile::Door
    )
}

/// A body of half-size `r` at `from` thrown `dist` along `(dx, dy)` (a unit
/// direction): it flies over pits, and stone stops it.
pub(super) fn throw(room: &Room, from: (f32, f32), (dx, dy): (f32, f32), dist: f32, r: f32) -> (f32, f32) {
    let (mut x, mut y) = from;
    let mut left = dist.max(0.0);
    while left > 0.0 {
        let step = left.min(0.25);
        let (nx, ny) = (x + dx * step, y + dy * step);
        let blocked = [(-r, -r), (r, -r), (-r, r), (r, r)]
            .into_iter()
            .any(|(ox, oy)| stops(room, nx + ox, ny + oy));
        if blocked {
            break;
        }
        (x, y) = (nx, ny);
        left -= step;
    }
    (x, y)
}

/// The distance from `p` to the segment `a`-`b`.
fn to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (vx * vx + vy * vy).max(1.0e-6);
    let t = (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / len2).clamp(0.0, 1.0);
    (p.0 - a.0 - vx * t).hypot(p.1 - a.1 - vy * t)
}

/// What a gauntlet's blow does to a knight it lands on.
#[derive(Clone, Copy)]
struct Blow {
    harm: u32,
    /// The way they are thrown, and how far.
    dir: (f32, f32),
    dist: f32,
}

impl Run {
    /// A room's gauntlet, laid out when the party walks in: a passage's
    /// bridge or hammers, a fighting room's boulder or saw.
    pub(super) fn arm_gauntlets(&mut self) {
        self.works.gauntlets.clear();
        let depth = self.dungeon.depth;
        let kind = self.room().kind;
        if depth == 0
            || self.at_home_now()
            || self.side_on()
            || self.open_map()
            || self.mode == fortune::Mode::Gauntlet
            || self.dungeon.pack == Pack::Swamp
            // A hall a game is played in (the folly, the Keg Gantry, a race)
            // keeps to its game.
            || [self.folly_room, self.gantry_room, self.race3d_room].contains(&Some(self.at))
            || !matches!(kind, RoomKind::Hall | RoomKind::Fight)
        {
            return;
        }
        let tier = (depth.min(3) - 1) as usize;
        // Dedicated generator: nothing here draws from the run's dice.
        let mut rng = Rng::new(mix(self.seed
            ^ self.raid_id.rotate_left(29)
            ^ u64::from(depth) << 40
            ^ (self.at as u64) << 24
            ^ 0x6a17_9a11));
        if kind == RoomKind::Hall {
            if !rng.chance(HALL_ODDS[tier]) {
                return;
            }
            let d = self.room().doors;
            let across = d[1] && d[3] && !d[0] && !d[2];
            let upright = d[0] && d[2] && !d[1] && !d[3];
            let forge = self.dungeon.pack.kin() == Pack::Hellforge;
            let bridge = rng.chance(if forge { 50 } else { 70 });
            if (across || upright) && bridge {
                self.lay_bridge(upright, tier, &mut rng);
            } else if across || upright || !bridge || !self.lay_leg_bridge(tier, &mut rng) {
                self.lay_hammers(&mut rng);
            }
            if !self.works.gauntlets.is_empty() {
                // The gauntlet is the passage's danger: no plate or peril
                // besides it.
                self.traps.retain(|t| !matches!(t, Trap::Spikes { .. }));
                self.works.perils.clear();
            }
        } else if rng.chance(ROOM_ODDS[tier]) {
            self.lay_room_gauntlet(tier, &mut rng);
        }
        if let Some(cue) = self.works.gauntlets.iter().find_map(|g| match g {
            Gauntlet::Axe { .. } => Some("trap_axes"),
            Gauntlet::Hammer { .. } => Some("trap_hammers"),
            Gauntlet::Boulder { .. } => Some("trap_boulder"),
            Gauntlet::Saw { .. } => Some("trap_saw"),
            Gauntlet::Bridge { .. } => None,
        }) {
            self.cues.push(cue.into());
        }
        if self.works.gauntlets.is_empty() {
            return;
        }
        for hero in self.players.values_mut() {
            hero.footing = None;
        }
        self.keep_footing();
    }

    /// A straight passage's gorge and the bridge over it, with its axes.
    fn lay_bridge(&mut self, upright: bool, tier: usize, rng: &mut Rng) {
        let (cols, rows) = (self.room().cols as i32, self.room().rows as i32);
        // The bridge: the two middle tiles of the passage; the gorge: all
        // but the landings by each doorway.
        let (long, wide) = if upright { (rows, cols) } else { (cols, rows) };
        let landing = if upright { 3 } else { 4 };
        let (g0, g1) = (landing, long - 1 - landing);
        if g1 - g0 < 4 {
            return;
        }
        self.span_bridge(upright, (g0, g1), wide / 2, tier, rng);
    }

    /// A bent passage's longer leg turned to a gorge and its bridge: true if
    /// a leg is long enough.
    fn lay_leg_bridge(&mut self, tier: usize, rng: &mut Rng) -> bool {
        // Back in a passage already cut: the same gorge, the same bridge.
        if let Some((upright, span, mid)) = self.carved_gorge() {
            self.span_bridge(upright, span, mid, tier, rng);
            return true;
        }
        let legs = self.hall_legs();
        let Some((upright, lanes, steps)) = legs
            .into_iter()
            .filter(|(_, lanes, steps)| steps.len() >= 6 && lanes.len() % 2 == 0)
            .max_by_key(|(_, _, steps)| steps.len())
        else {
            return false;
        };
        // Two tiles kept by the doorway, one by the hub.
        let gorge = &steps[2..steps.len() - 1];
        let (g0, g1) = (*gorge.iter().min().unwrap_or(&0), *gorge.iter().max().unwrap_or(&0));
        self.span_bridge(upright, (g0, g1), lanes[lanes.len() / 2], tier, rng);
        true
    }

    /// The gorge a passage already has (a passage has no pits of its own):
    /// which way it runs, the tiles it spans along the passage, and the
    /// bridge's middle across it.
    fn carved_gorge(&self) -> Option<(bool, (i32, i32), i32)> {
        let room = self.room();
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        let pits: Vec<(i32, i32)> = (0..rows)
            .flat_map(|r| (0..cols).map(move |c| (c, r)))
            .filter(|&(c, r)| room.tile(c, r) == Tile::Hazard)
            .collect();
        let cmin = pits.iter().map(|p| p.0).min()?;
        let cmax = pits.iter().map(|p| p.0).max()?;
        let rmin = pits.iter().map(|p| p.1).min()?;
        let rmax = pits.iter().map(|p| p.1).max()?;
        let across = rmin == 1 && rmax == rows - 2 && !(cmin == 1 && cmax == cols - 2);
        if across {
            let mid = (1..rows - 1).filter(|&r| room.tile(cmin, r).ground()).max()?;
            Some((false, (cmin, cmax), mid))
        } else {
            let mid = (1..cols - 1).filter(|&c| room.tile(c, rmin).ground()).max()?;
            Some((true, (rmin, rmax), mid))
        }
    }

    /// The gorge across tiles `g0..=g1` along the passage, all but the
    /// bridge's two tiles either side of `mid` across it, and its axes.
    fn span_bridge(&mut self, upright: bool, (g0, g1): (i32, i32), mid: i32, tier: usize, rng: &mut Rng) {
        let at = self.at;
        let (cols, rows) = (self.room().cols as i32, self.room().rows as i32);
        let wide = if upright { cols } else { rows };
        let bridge = [mid - 1, mid];
        let mut changed = false;
        {
            let room = &mut self.dungeon.rooms[at];
            for a in g0..=g1 {
                for b in 1..wide - 1 {
                    let (c, r) = if upright { (b, a) } else { (a, b) };
                    let tile = if bridge.contains(&b) { Tile::Floor } else { Tile::Hazard };
                    if room.tile(c, r) != tile {
                        room.set(c as usize, r as usize, tile);
                        changed = true;
                    }
                }
            }
        }
        if changed {
            // The room's stones changed: friends' mirrors take the whole floor.
            self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        }
        let (from, to) = (g0 as f32 * TILE_UNITS, (g1 + 1) as f32 * TILE_UNITS);
        let line = mid as f32 * TILE_UNITS;
        self.works.gauntlets.push(Gauntlet::Bridge { from, to, line, upright });
        let span = to - from;
        let n = ((span / 7.5).round() as usize).clamp(2, 4) + usize::from(tier >= 2 && span > 20.0);
        let n = n.min(5);
        let period = AXE_PERIOD[tier];
        let base = rng.below(period as usize) as u32;
        // A cascade: each axe a little behind the one before, so the bridge
        // can be crossed with the swing, never all at once.
        let lag = period * 2 / 7;
        for i in 0..n {
            let along = from + span * (i as f32 + 0.5) / n as f32;
            let phase = (base + lag * i as u32) % period;
            let (x, y) = if upright { (line, along) } else { (along, line) };
            self.works.gauntlets.push(Gauntlet::Axe { x, y, upright, period, phase });
        }
        self.settle_off_pits();
    }

    /// Anyone standing where the gorge now is steps back onto firm ground.
    fn settle_off_pits(&mut self) {
        let room = &self.dungeon.rooms[self.at];
        let firm = |x: f32, y: f32| {
            let grid = Grid { room, barred: false };
            !over_pit(room, x, y) && grid.clear(x, y, HERO_RADIUS + 0.1, Mover::Hero)
        };
        let nearest = |x: f32, y: f32| -> (f32, f32) {
            let mut best = (x, y);
            let mut d = f32::MAX;
            for r in 0..room.rows as i32 {
                for c in 0..room.cols as i32 {
                    let (cx, cy) = ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
                    let e = (cx - x).hypot(cy - y);
                    if e < d && firm(cx, cy) {
                        (best, d) = ((cx, cy), e);
                    }
                }
            }
            best
        };
        let moves: Vec<(u32, (f32, f32))> = self
            .players
            .iter()
            .filter(|(_, h)| !firm(h.x, h.y))
            .map(|(&id, h)| (id, nearest(h.x, h.y)))
            .collect();
        let foes: Vec<(usize, (f32, f32))> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.kind.flies() && over_pit(room, e.x, e.y))
            .map(|(i, e)| (i, nearest(e.x, e.y)))
            .collect();
        for (id, at) in moves {
            if let Some(hero) = self.players.get_mut(&id) {
                (hero.x, hero.y) = at;
            }
        }
        for (i, at) in foes {
            (self.enemies[i].x, self.enemies[i].y) = at;
        }
    }

    /// The passage's longest leg, from its doorway to its hub: where it
    /// runs, along which axis, and its width.
    fn hall_legs(&self) -> Vec<(bool, Vec<i32>, Vec<i32>)> {
        let room = self.room();
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        let mut legs = Vec::new();
        for side in 0..4 {
            if !room.doors[side] {
                continue;
            }
            // `upright` legs run along y; `lanes` are the cross-coordinates
            // of the floor at the doorway's end, `steps` the tiles along it.
            let upright = side % 2 == 0;
            let (long, wide) = if upright { (rows, cols) } else { (cols, rows) };
            let tile = |a: i32, b: i32| if upright { room.tile(b, a) } else { room.tile(a, b) };
            let first = if side == 0 || side == 3 { 1 } else { long - 2 };
            let step = if first == 1 { 1 } else { -1 };
            let lanes: Vec<i32> = (1..wide - 1).filter(|&b| tile(first, b).ground()).collect();
            if lanes.len() < 2 {
                continue;
            }
            let mut steps = Vec::new();
            let mut a = first;
            while a > 0 && a < long - 1 {
                let floor: Vec<i32> = (1..wide - 1).filter(|&b| tile(a, b).ground()).collect();
                if floor != lanes {
                    break;
                }
                steps.push(a);
                a += step;
            }
            legs.push((upright, lanes, steps));
        }
        legs
    }

    /// Hammers across a passage's legs, a gate of them every three tiles.
    fn lay_hammers(&mut self, rng: &mut Rng) {
        let mut legs = self.hall_legs();
        // The longest leg first; a straight passage takes both.
        legs.sort_by_key(|(_, _, steps)| std::cmp::Reverse(steps.len()));
        let straight = legs.len() == 2 && legs[0].0 == legs[1].0;
        let base = rng.below(HAMMER_PERIOD as usize) as u32;
        let mut gate = 0u32;
        for (upright, lanes, steps) in legs.iter().take(if straight { 2 } else { 1 }) {
            // Tiles kept clear by the doorway (one in a short upright leg);
            // gates from the hub side.
            let usable = steps.len().saturating_sub(if *upright { 1 } else { 2 });
            let gates = ((usable + 1) / 3).min(3);
            if gates == 0 || lanes.len() % 2 != 0 {
                continue;
            }
            // Steps run from the doorway inward: the hub's end is last.
            let inner: Vec<i32> = steps.iter().rev().copied().collect();
            for g in 0..gates {
                let (a0, a1) = (inner[g * 3], inner[g * 3 + 1]);
                let along = (a0.min(a1) as f32 + 1.0) * TILE_UNITS;
                let phase = (base + gate * HAMMER_PERIOD / 3) % HAMMER_PERIOD;
                gate += 1;
                for pair in lanes.chunks(2) {
                    let across = (pair[0] as f32 + 1.0) * TILE_UNITS;
                    let (x, y) = if *upright { (across, along) } else { (along, across) };
                    self.works.gauntlets.push(Gauntlet::Hammer { x, y, upright: *upright, phase });
                }
            }
        }
    }

    /// A fighting room's boulder run or saw rail, by its pack.
    fn lay_room_gauntlet(&mut self, tier: usize, rng: &mut Rng) {
        // 0 a boulder, 1 a saw.
        let menu: Vec<(u8, u32)> = match self.dungeon.pack {
            Pack::Cavern | Pack::Fungal => vec![(0, 3), (1, 1)],
            Pack::Crypt | Pack::Archive => vec![(1, 3), (0, 1)],
            Pack::Hellforge | Pack::Unknown => vec![(0, 2), (1, 2)],
            Pack::Swamp => return,
        };
        let pick = rng.pick(&menu);
        let laid = if pick == 0 { self.lay_boulder(tier, rng) } else { self.lay_saw(tier, rng) };
        // A great hall far down may take a second saw.
        if laid && self.dungeon.depth >= 4 && self.room().great() && rng.chance(50) {
            self.lay_saw(tier, rng);
        }
    }

    /// Clear of the room's doorways, its perils and its spike plates.
    fn clear_of_others(&self, near: impl Fn((f32, f32)) -> f32) -> bool {
        let room = self.room();
        let doors = (0..4).filter(|&d| room.doors[d]).all(|d| near(hazards::doorway(room, d).0) > DOOR_CLEAR);
        let perils = self.works.perils.iter().all(|p| near(p.at()) > 4.5);
        let plates = self.traps.iter().all(|t| match *t {
            Trap::Spikes { col, row, .. } => near(((col + 1) as f32 * TILE_UNITS, (row + 1) as f32 * TILE_UNITS)) > 4.0,
            _ => true,
        });
        let others = self.works.gauntlets.iter().all(|g| near(g.at()) > 6.0);
        doors && perils && plates && others
    }

    /// A boulder's lane across the room, two tiles wide, out of a wall.
    fn lay_boulder(&mut self, tier: usize, rng: &mut Rng) -> bool {
        let room = self.room();
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        let mut lanes: Vec<(f32, f32, f32)> = Vec::new();
        for r in 2..rows - 3 {
            let y = (r + 1) as f32 * TILE_UNITS;
            for west in [true, false] {
                // It comes out of solid wall, never a doorway.
                let wall = if west { 0 } else { cols - 1 };
                if room.tile(wall, r) != Tile::Wall || room.tile(wall, r + 1) != Tile::Wall {
                    continue;
                }
                let mut run = 0;
                let mut c = if west { 1 } else { cols - 2 };
                while c > 0 && c < cols - 1 && room.tile(c, r).ground() && room.tile(c, r + 1).ground() {
                    run += 1;
                    c += if west { 1 } else { -1 };
                }
                if run < 9 {
                    continue;
                }
                let from = if west { TILE_UNITS + BOULDER_RADIUS } else { (cols - 1) as f32 * TILE_UNITS - BOULDER_RADIUS };
                let to = if west {
                    (1 + run) as f32 * TILE_UNITS - BOULDER_RADIUS
                } else {
                    (cols - 1 - run) as f32 * TILE_UNITS + BOULDER_RADIUS
                };
                let lane = ((from.min(to), y), (from.max(to), y));
                if self.clear_of_others(|p| to_segment(p, lane.0, lane.1)) {
                    lanes.push((from, to, y));
                }
            }
        }
        if lanes.is_empty() {
            return false;
        }
        let (from, to, y) = lanes[rng.below(lanes.len())];
        // Room for the whole roll and a breath after it, in a great hall too.
        let roll = ((to - from).abs() / BOULDER_SPEED).floor() as u32;
        let period = BOULDER_PERIOD[tier].max(BOULDER_WARN + roll + 45);
        let phase = rng.below(period as usize) as u32;
        self.works.gauntlets.push(Gauntlet::Boulder { from, to, y, period, phase });
        true
    }

    /// A saw's rail, four to six tiles long, through open floor.
    fn lay_saw(&mut self, tier: usize, rng: &mut Rng) -> bool {
        let room = self.room();
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        for _ in 0..60 {
            let long = 4 + rng.below(3) as i32;
            let upright = rng.chance(40);
            let (c, r) = if upright {
                (2 + rng.below((cols - 4).max(1) as usize) as i32, 2 + rng.below((rows - long - 4).max(1) as usize) as i32)
            } else {
                (2 + rng.below((cols - long - 4).max(1) as usize) as i32, 2 + rng.below((rows - 4).max(1) as usize) as i32)
            };
            let cells: Vec<(i32, i32)> = (0..=long)
                .flat_map(|k| (-1..=1).map(move |w| if upright { (c + w, r + k) } else { (c + k, r + w) }))
                .collect();
            if !cells.iter().all(|&(cc, rr)| room.tile(cc, rr).ground()) {
                continue;
            }
            let centre = |cc: i32, rr: i32| ((cc as f32 + 0.5) * TILE_UNITS, (rr as f32 + 0.5) * TILE_UNITS);
            let a = centre(c, r);
            let b = if upright { centre(c, r + long) } else { centre(c + long, r) };
            if !self.clear_of_others(|p| to_segment(p, a, b)) {
                continue;
            }
            let speed = SAW_SPEED[tier];
            let saw = Gauntlet::Saw { a, b, speed, phase: 0 };
            let phase = rng.below(saw.saw_cycle() as usize) as u32;
            self.works.gauntlets.push(Gauntlet::Saw { a, b, speed, phase });
            return true;
        }
        false
    }

    /// Each knight standing on firm ground (off any bridge's span) is
    /// remembered there: a fall puts them back on it.
    fn keep_footing(&mut self) {
        let room = &self.dungeon.rooms[self.at];
        let spans: Vec<(f32, f32, bool)> = self
            .works
            .gauntlets
            .iter()
            .filter_map(|g| match *g {
                Gauntlet::Bridge { from, to, upright, .. } => Some((from, to, upright)),
                _ => None,
            })
            .collect();
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.absent) {
            let r = HERO_RADIUS + 0.3;
            let firm = [(-r, -r), (r, -r), (-r, r), (r, r), (0.0, 0.0)]
                .into_iter()
                .all(|(ox, oy)| room.tile(((hero.x + ox) / TILE_UNITS).floor() as i32, ((hero.y + oy) / TILE_UNITS).floor() as i32).ground());
            let on_span = spans.iter().any(|&(from, to, upright)| {
                let along = if upright { hero.y } else { hero.x };
                along > from - 0.5 && along < to + 0.5
            });
            if firm && !on_span {
                hero.footing = Some((hero.x, hero.y));
            }
        }
    }

    /// Where a knight who fell with no footing remembered is put: the end
    /// of the bridge nearer to them, else the nearest firm floor.
    fn landing_for(&self, x: f32, y: f32) -> (f32, f32) {
        for g in &self.works.gauntlets {
            if let Gauntlet::Bridge { from, to, line, upright } = *g {
                let along = if upright { y } else { x };
                let end = if (along - from).abs() <= (along - to).abs() { from - 1.5 } else { to + 1.5 };
                return if upright { (line, end) } else { (end, line) };
            }
        }
        let room = self.room();
        let mut best = (x, y);
        let mut d = f32::MAX;
        for r in 0..room.rows as i32 {
            for c in 0..room.cols as i32 {
                let (cx, cy) = ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
                if room.tile(c, r).ground() && (cx - x).hypot(cy - y) < d {
                    (best, d) = ((cx, cy), (cx - x).hypot(cy - y));
                }
            }
        }
        best
    }

    /// A gauntlet's blow on knight `id`: hurt, thrown, and over the edge
    /// if the throw ends over a pit. Nothing while their ward holds.
    fn gauntlet_blow(&mut self, id: u32, blow: Blow) -> bool {
        let at = self.at;
        let Some(hero) = self.players.get_mut(&id) else { return false };
        if hero.hp == 0 || hero.stone || hero.absent || hero.privy > 0 || hero.invulnerable > 0 {
            return false;
        }
        let before = hero.hp;
        hero.hurt(blow.harm);
        self.room_hurt |= hero.hp < before;
        if hero.hp == 0 {
            // They fall where they were struck, on firm ground for a friend.
            return true;
        }
        let room = &self.dungeon.rooms[at];
        (hero.x, hero.y) = throw(room, (hero.x, hero.y), blow.dir, blow.dist, HERO_RADIUS);
        if over_pit(room, hero.x, hero.y) {
            self.fall(id);
        }
        true
    }

    /// Knight `id` goes over the edge: a share of their health, and back
    /// on their footing, warded a moment.
    fn fall(&mut self, id: u32) {
        let Some(hero) = self.players.get(&id) else { return };
        let (fx, fy) = (hero.x, hero.y);
        let back = hero.footing.unwrap_or_else(|| self.landing_for(fx, fy));
        let Some(hero) = self.players.get_mut(&id) else { return };
        let lost = (hero.max_hp * FALL_SHARE / 100).max(1);
        hero.hp = hero.hp.saturating_sub(lost);
        (hero.x, hero.y) = back;
        hero.dash_ticks = 0;
        hero.invulnerable = hero.invulnerable.max(HZ);
        self.room_hurt = true;
        self.boom(fx, fy, skills::BoomKind::Dust, 1.4);
        self.sounds.push("rock_land");
        self.cues.push("gorge_fall".into());
    }

    /// A gauntlet's blow on monster `i`: hurt, thrown, and over the edge
    /// (the end of it) if the throw ends over a pit. A guardian stands its
    /// ground. True if it went over.
    fn gauntlet_foe(&mut self, i: usize, harm: u32, dir: (f32, f32), dist: f32) -> bool {
        let at = self.at;
        let enemy = &mut self.enemies[i];
        enemy.hp = enemy.hp.saturating_sub(harm);
        if enemy.hp == 0 || enemy.is_boss() {
            return false;
        }
        let room = &self.dungeon.rooms[at];
        let to = throw(room, (enemy.x, enemy.y), dir, dist, enemy.radius() * 0.5);
        if !enemy.kind.flies() && over_pit(room, to.0, to.1) {
            // It goes over; what it carried stays on the edge it left.
            enemy.hp = 0;
            self.boom(to.0, to.1, skills::BoomKind::Dust, 1.4);
            return true;
        }
        (enemy.x, enemy.y) = to;
        false
    }

    /// The knights a gauntlet can reach now, and where they stand.
    fn gauntlet_knights(&self) -> Vec<(u32, f32, f32)> {
        self.players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && !h.absent && h.privy == 0)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect()
    }

    /// The monsters a gauntlet can reach now: index, place, half-size.
    fn gauntlet_foes(&self) -> Vec<(usize, f32, f32, f32, bool)> {
        self.enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.hp > 0 && e.kind != EnemyKind::Dummy && !e.dormant)
            .map(|(i, e)| (i, e.x, e.y, e.radius() * 0.5, e.kind.flies()))
            .collect()
    }

    /// Axes swing, hammers fall, boulders roll and saws run: whatever they
    /// meet is struck, knight or monster.
    pub(super) fn tick_gauntlets(&mut self) {
        if self.works.gauntlets.is_empty() {
            return;
        }
        let tick = self.tick;
        self.keep_footing();
        let mut slammed = false;
        let mut swept = false;
        for g in self.works.gauntlets.clone() {
            match g {
                Gauntlet::Bridge { .. } => {}
                Gauntlet::Axe { x, y, upright, .. } => {
                    if !g.low(tick) {
                        continue;
                    }
                    // Where the blade was and is across the bridge: what it
                    // passed through this tick, it struck.
                    let then = AXE_REACH * g.swing(tick.saturating_sub(1));
                    let now = AXE_REACH * g.swing(tick);
                    let heading = g.heading(tick);
                    let (lo, hi) = (then.min(now), then.max(now));
                    let split = |px: f32, py: f32| if upright { (py - y, px - x) } else { (px - x, py - y) };
                    let dir = if upright { (heading, 0.0) } else { (0.0, heading) };
                    if (then < 0.0) != (now < 0.0)
                        && self.gauntlet_knights().iter().any(|&(_, hx, hy)| (hx - x).hypot(hy - y) < 6.0)
                    {
                        self.sounds.push("swing");
                    }
                    for (id, hx, hy) in self.gauntlet_knights() {
                        let (along, across) = split(hx, hy);
                        let r = HERO_RADIUS + 0.3;
                        if along.abs() <= AXE_EDGE + HERO_RADIUS && across >= lo - r && across <= hi + r {
                            if self.gauntlet_blow(id, Blow { harm: AXE_HARM, dir, dist: AXE_KNOCK }) {
                                self.sounds.push("hit_axe");
                            }
                        }
                    }
                    for (i, ex, ey, r, _) in self.gauntlet_foes() {
                        let (along, across) = split(ex, ey);
                        if along.abs() <= AXE_EDGE + r && across >= lo - r - 0.3 && across <= hi + r + 0.3 {
                            // Swept the length of the swing: off the bridge.
                            let dist = (AXE_REACH + r + 0.4 - across * heading).max(0.5);
                            swept |= self.gauntlet_foe(i, AXE_FOE_HARM, dir, dist);
                            self.sounds.push("hit_axe");
                        }
                    }
                }
                Gauntlet::Hammer { x, y, upright, .. } => {
                    let beat = g.beat(tick);
                    let slam = beat == HAMMER_SLAM;
                    if !g.down(tick) {
                        continue;
                    }
                    // The floor shakes for the knights beside it, not the room.
                    slammed |= slam && self.gauntlet_knights().iter().any(|&(_, hx, hy)| (hx - x).hypot(hy - y) < 8.0);
                    // Out of the footprint along the passage, the short way.
                    let out = |px: f32, py: f32, r: f32| -> ((f32, f32), f32) {
                        let d = if upright { py - y } else { px - x };
                        let s = if d >= 0.0 { 1.0 } else { -1.0 };
                        let dir = if upright { (0.0, s) } else { (s, 0.0) };
                        (dir, HAMMER_HALF + r + 0.3 - d.abs())
                    };
                    let inside = |px: f32, py: f32, r: f32| (px - x).abs() < HAMMER_HALF + r && (py - y).abs() < HAMMER_HALF + r;
                    for (id, hx, hy) in self.gauntlet_knights() {
                        if !inside(hx, hy, HERO_RADIUS) {
                            continue;
                        }
                        let (dir, dist) = out(hx, hy, HERO_RADIUS);
                        let warded = self.players.get(&id).is_some_and(|h| h.invulnerable > 0);
                        if slam && !warded {
                            self.gauntlet_blow(id, Blow { harm: HAMMER_HARM, dir, dist });
                        } else if let Some(hero) = self.players.get_mut(&id) {
                            // An iron block on the floor: no standing in it.
                            let room = &self.dungeon.rooms[self.at];
                            (hero.x, hero.y) = throw(room, (hero.x, hero.y), dir, dist, HERO_RADIUS);
                        }
                    }
                    for (i, ex, ey, r, flies) in self.gauntlet_foes() {
                        if !inside(ex, ey, r) || (flies && !slam) {
                            continue;
                        }
                        let (dir, dist) = out(ex, ey, r);
                        swept |= self.gauntlet_foe(i, if slam { HAMMER_FOE_HARM } else { 0 }, dir, dist);
                    }
                }
                Gauntlet::Boulder { y, .. } => {
                    if g.bursts(tick) {
                        let Gauntlet::Boulder { to, .. } = g else { continue };
                        self.boom(to, y, skills::BoomKind::Dust, BOULDER_RADIUS + 0.8);
                        self.fx.push(Fx::Shake(6));
                        self.sounds.push("rock_land");
                        continue;
                    }
                    if g.beat(tick) == 0 {
                        self.sounds.push("rock_land");
                    }
                    let Some((bx, by)) = g.boulder(tick) else { continue };
                    let aside = |py: f32, r: f32| -> ((f32, f32), f32) {
                        let s = if py >= by { 1.0 } else { -1.0 };
                        ((0.0, s), BOULDER_RADIUS + r + 0.4 - (py - by).abs())
                    };
                    let room = &self.dungeon.rooms[self.at];
                    // Thrown to the open side; the other if stone stands there.
                    let side = |px: f32, py: f32, r: f32| -> ((f32, f32), f32) {
                        let ((dx, dy), dist) = aside(py, r);
                        let to = throw(room, (px, py), (dx, dy), dist, r);
                        if (to.1 - py).abs() + 0.05 < dist {
                            ((-dx, -dy), BOULDER_RADIUS + r + 0.4 + (py - by).abs())
                        } else {
                            ((dx, dy), dist)
                        }
                    };
                    let mut blows = Vec::new();
                    for (id, hx, hy) in self.gauntlet_knights() {
                        if (hx - bx).abs() < BOULDER_RADIUS + HERO_RADIUS && (hy - by).abs() < BOULDER_RADIUS + HERO_RADIUS {
                            let (dir, dist) = side(hx, hy, HERO_RADIUS);
                            blows.push((id, dir, dist));
                        }
                    }
                    let mut foes = Vec::new();
                    for (i, ex, ey, r, flies) in self.gauntlet_foes() {
                        if !flies && (ex - bx).abs() < BOULDER_RADIUS + r && (ey - by).abs() < BOULDER_RADIUS + r {
                            let (dir, dist) = side(ex, ey, r);
                            foes.push((i, dir, dist));
                        }
                    }
                    for (id, dir, dist) in blows {
                        if self.gauntlet_blow(id, Blow { harm: BOULDER_HARM, dir, dist }) {
                            self.fx.push(Fx::Shake(4));
                        }
                    }
                    for (i, dir, dist) in foes {
                        let boss = self.enemies[i].is_boss();
                        swept |= self.gauntlet_foe(i, if boss { BOULDER_FOE_HARM / 4 } else { BOULDER_FOE_HARM }, dir, dist);
                    }
                }
                Gauntlet::Saw { a, b, .. } => {
                    let ((sx, sy), _) = g.saw_at(tick);
                    // Off the rail, to the side the body stands on.
                    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
                    let len = vx.hypot(vy).max(0.01);
                    let (nx, ny) = (-vy / len, vx / len);
                    let away = |px: f32, py: f32| {
                        let s = if (px - sx) * nx + (py - sy) * ny >= 0.0 { 1.0 } else { -1.0 };
                        (nx * s, ny * s)
                    };
                    for (id, hx, hy) in self.gauntlet_knights() {
                        if (hx - sx).hypot(hy - sy) < SAW_RADIUS + HERO_RADIUS {
                            let dir = away(hx, hy);
                            if self.gauntlet_blow(id, Blow { harm: SAW_HARM, dir, dist: SAW_KNOCK }) {
                                self.sounds.push("hit_blade");
                            }
                        }
                    }
                    if tick.is_multiple_of(SAW_EVERY) {
                        for (i, ex, ey, r, flies) in self.gauntlet_foes() {
                            if !flies && (ex - sx).hypot(ey - sy) < SAW_RADIUS + r {
                                let dir = away(ex, ey);
                                swept |= self.gauntlet_foe(i, SAW_FOE_HARM, dir, 1.0);
                                self.sounds.push("hit_blade");
                            }
                        }
                    }
                }
            }
        }
        if slammed {
            self.fx.push(Fx::Shake(3));
            self.sounds.push("clang");
        }
        if swept {
            self.notice("mind_the_gap");
        }
    }
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    /// Lay this room's gauntlet of `kind` afresh from `seed`: "bridge" (a
    /// straight passage's), "hammers" (a passage's), "boulder" or "saw" (a
    /// fighting room's). False if the room has no place for it.
    pub fn lay_gauntlet_for_test(&mut self, kind: &str, seed: u64) -> bool {
        let tier = (self.dungeon.depth.clamp(1, 3) - 1) as usize;
        let mut rng = Rng::new(seed);
        let d = self.room().doors;
        let before = self.works.gauntlets.len();
        match kind {
            "bridge" if d == [false, true, false, true] => self.lay_bridge(false, tier, &mut rng),
            "bridge" if d == [true, false, true, false] => self.lay_bridge(true, tier, &mut rng),
            "bridge" => {
                self.lay_leg_bridge(tier, &mut rng);
            }
            "hammers" => self.lay_hammers(&mut rng),
            "boulder" => {
                self.lay_boulder(tier, &mut rng);
            }
            "saw" => {
                self.lay_saw(tier, &mut rng);
            }
            _ => {}
        }
        self.works.gauntlets.len() > before
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_gauntlets__tests.rs"]
mod tests;
