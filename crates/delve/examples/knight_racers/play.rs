//! Knight racers, played: a looped circuit (the dungeon or the canyon bent into
//! an oval), four knights on it, a title screen to pick yours, one lap from a
//! countdown, a finish table. The art is the example's own; this file is the
//! rules, the rivals, the camera, the lettering and the terminal.
//!
//! Live: raw keys (Kitty key release where the terminal reports it; without it
//! the throttle is automatic and keys are taps). Headless, for checking:
//!
//!   --autopilot [--knight N] [--frames N] [--shots DIR [--every N]]   a driver plays your knight
//!   --script FILE [...]                                                lines of `FRAME keys...` (left right up down boost)

use super::{BENT, CELLS, Cell, Course, GUN, H_HALF, H_LAVA, IRON, RACERS, Shot, canyon_span, dungeon_span, gate, racer};
use angel_delve::render::world3d::math::{V3, v3};
use angel_delve::render::world3d::mesh::{Mesh, mat};
use angel_delve::render::world3d::raster::View3;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use crossterm::terminal;
use std::error::Error;
use std::f32::consts::PI;
use std::io::{self, IsTerminal, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- the circuit

/// One tick of the game, in seconds: input is read and applied every one.
const DT: f32 = 1.0 / 30.0;
/// The lap, in track units. A multiple of 48, which the pillars and cages keep to.
const LAP: f32 = 864.0;
/// The oval: two straights and two half-circle bends of this radius, bending left.
const BEND: f32 = 50.0;
const STRAIGHT: f32 = (LAP - 2.0 * PI * BEND) / 2.0;
const LAPS: u32 = 1;
/// The racers are built big for the cinematic; on the circuit they are this much smaller.
const SCALE: f32 = 0.72;
/// A racer's box on the track: half its length, and where the middle of it lies behind the racer's own origin.
const HALF_LEN: f32 = 5.5 * SCALE;
const CENTRE: f32 = -1.9 * SCALE;
/// Seconds on the grid before the start.
const COUNTDOWN: f32 = 3.2;
const NAMES: [&str; 4] = ["EXCALIBUR", "BASTION", "WRAITH", "CROWN"];

/// Where the track's centre line is at distance `s` round the lap, and which way it points.
fn pose(s: f32) -> (f32, f32, f32) {
    let s = s.rem_euclid(LAP);
    let arc = PI * BEND;
    if s < STRAIGHT {
        (s, 0.0, 0.0)
    } else if s < STRAIGHT + arc {
        let f = (s - STRAIGHT) / BEND;
        (STRAIGHT + BEND * f.sin(), BEND * (1.0 - f.cos()), f)
    } else if s < 2.0 * STRAIGHT + arc {
        (STRAIGHT - (s - STRAIGHT - arc), 2.0 * BEND, PI)
    } else {
        let f = (s - 2.0 * STRAIGHT - arc) / BEND;
        (-BEND * f.sin(), BEND * (1.0 + f.cos()), PI + f)
    }
}

/// The world point `y` to the left of the centre line at `s`, `z` up.
fn at(s: f32, y: f32, z: f32) -> V3 {
    let (px, py, th) = pose(s);
    v3(px - y * th.sin(), py + y * th.cos(), z)
}

/// How hard the track bends at `s` (1/radius): all the bends bend left.
fn curve(s: f32) -> f32 {
    let s = s.rem_euclid(LAP);
    if (STRAIGHT..STRAIGHT + PI * BEND).contains(&s) || s >= 2.0 * STRAIGHT + PI * BEND { 1.0 / BEND } else { 0.0 }
}

/// The distance between two places on the lap, the short way round, signed.
fn wrap(d: f32) -> f32 {
    (d + LAP * 0.5).rem_euclid(LAP) - LAP * 0.5
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// A straight dungeon or canyon bent along the oval, tile by tile. The hell
/// rim light is lit across the track, so each tile carries its pre-bend frame.
fn bend(m: &Mesh) -> Mesh {
    let mut out = Mesh::new();
    for t in &m.tris {
        let cen = (t.v[0] + t.v[1] + t.v[2]) / 3.0;
        out.push_tri(t.v.map(|p| at(p.x, p.y, p.z)), [[cen.y, cen.z], [t.normal.y, t.normal.z], BENT], t.mat);
    }
    out
}

/// A barricade across part of the track: slow down or steer round.
struct Block {
    s: f32,
    y: f32,
    hw: f32,
    hl: f32,
    /// Which side the racing line takes it: -1 right, +1 left.
    pass: f32,
}

fn blocks() -> Vec<Block> {
    const AT: [(f32, f32); 14] = [(-4.0, 1.0), (4.0, -1.0), (1.5, -1.0), (4.2, -1.0), (-4.2, 1.0), (1.5, -1.0), (-3.6, 1.0), (3.6, -1.0), (1.5, -1.0), (-4.0, 1.0), (4.0, -1.0), (1.5, -1.0), (4.0, -1.0), (-4.0, 1.0)];
    AT.iter().enumerate().map(|(k, &(y, pass))| Block { s: 120.0 + 52.0 * k as f32, y, hw: 2.2, hl: 1.4, pass }).collect()
}

/// What the rules of a course need to know, besides its looks.
struct Track {
    hell: bool,
    blocks: Vec<Block>,
    /// The racing line: the lateral place to be, a sample every unit of the lap.
    line: Vec<f32>,
    /// How free of barricades each unit of the lap is (0 beside one, 1 clear).
    free: Vec<f32>,
}

impl Track {
    fn new(hell: bool) -> Track {
        let blocks = blocks();
        let way: Vec<(f32, f32)> = blocks.iter().map(|b| (b.s, if b.pass < 0.0 { (b.y - b.hw - 3.9).max(-6.2) } else { (b.y + b.hw + 3.9).min(6.2) })).collect();
        let arc = |s: f32, a0: f32, a1: f32| (((s - a0 + 25.0) / 25.0).clamp(0.0, 1.0)).min(((a1 + 25.0 - s) / 25.0).clamp(0.0, 1.0));
        let (mut line, mut free) = (vec![], vec![]);
        for i in 0..LAP as usize {
            let s = i as f32;
            let k = way.iter().rposition(|w| w.0 <= s);
            let (a, b) = match k {
                Some(k) if k + 1 < way.len() => (way[k], way[k + 1]),
                Some(k) => (way[k], (way[0].0 + LAP, way[0].1)),
                None => ((way[way.len() - 1].0 - LAP, way[way.len() - 1].1), way[0]),
            };
            let w = 0.5 - 0.5 * (PI * (s - a.0) / (b.0 - a.0)).cos();
            let near = blocks.iter().map(|b| wrap(s - b.s).abs()).fold(f32::MAX, f32::min);
            let f = smooth(8.0, 20.0, near);
            let inside = arc(s, STRAIGHT, STRAIGHT + PI * BEND).max(arc(s, 2.0 * STRAIGHT + PI * BEND, LAP)).max(arc(s + LAP, 2.0 * STRAIGHT + PI * BEND, LAP));
            line.push(a.1 + (b.1 - a.1) * w + 2.2 * inside * f);
            free.push(f);
        }
        Track { hell, blocks, line, free }
    }

    fn line_at(&self, s: f32) -> f32 {
        let s = s.rem_euclid(LAP);
        let (i, f) = (s as usize % self.line.len(), s.fract());
        self.line[i] + (self.line[(i + 1) % self.line.len()] - self.line[i]) * f
    }

    fn free_at(&self, s: f32) -> f32 {
        self.free[s.rem_euclid(LAP) as usize % self.free.len()]
    }
}

/// A course built into a bent world, and the rules that go with it.
struct Circuit {
    world: Mesh,
    track: Track,
}

impl Circuit {
    fn new(course: Course) -> Circuit {
        let straight = match course {
            Course::Hell => dungeon_span(0.0, LAP, LAP),
            Course::Canyon => canyon_span(0.0, LAP, LAP),
        };
        let mut world = bend(&straight);
        let track = Track::new(course == Course::Hell);
        let mut props = Mesh::new();
        for b in &track.blocks {
            barricade(&mut props, b);
        }
        gate(&mut props, course);
        world.merge(bend(&props));
        Circuit { world, track }
    }
}

fn cuboid(m: &mut Mesh, lo: V3, hi: V3, k: u8) {
    let p = |x: bool, y: bool, z: bool| v3(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z });
    let (f, t) = (false, true);
    m.push_quad(p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t), k);
    m.push_quad(p(f, f, f), p(t, f, f), p(t, t, f), p(f, t, f), k);
    m.push_quad(p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t), k);
    m.push_quad(p(f, t, f), p(t, t, f), p(t, t, t), p(f, t, t), k);
    m.push_quad(p(f, f, f), p(f, t, f), p(f, t, t), p(f, f, t), k);
    m.push_quad(p(t, f, f), p(t, t, f), p(t, t, t), p(t, f, t), k);
}

/// An iron barricade with a glowing hazard band and teeth.
fn barricade(m: &mut Mesh, b: &Block) {
    let (lo, hi) = (v3(b.s - b.hl, b.y - b.hw, 0.0), v3(b.s + b.hl, b.y + b.hw, 1.3));
    cuboid(m, lo, hi, IRON);
    cuboid(m, v3(lo.x - 0.05, lo.y, 0.8), v3(hi.x + 0.05, hi.y, 1.05), mat::FIRE);
    for i in 0..5 {
        let y = b.y - b.hw + (i as f32 + 0.5) * 2.0 * b.hw / 5.0;
        for (a, c) in [(1.0f32, -1.0f32), (-1.0, 1.0)] {
            let _ = c;
            let tip = v3(b.s + a * (b.hl + 0.9), y, 1.9);
            let base = v3(b.s + a * b.hl, y, 0.5);
            let w = 0.28;
            m.push_tri([base + v3(0.0, -w, 0.0), base + v3(0.0, w, 0.0), tip], [[0.0; 2]; 3], GUN);
        }
        cuboid(m, v3(b.s - 0.2, y - 0.14, 1.3), v3(b.s + 0.2, y + 0.14, 1.9), GUN);
    }
}

// ---------------------------------------------------------------- the cars

/// What a driver asks of a car in one tick.
#[derive(Clone, Copy, Default)]
struct Input {
    /// -1 right .. 1 left.
    steer: f32,
    throttle: f32,
    brake: bool,
    boost: bool,
}

/// How a knight's racer drives: top speed (units/s), pull, steering (units/s
/// sideways), mass, how much a crash hurts it (lower is tougher), boost thirst.
struct Stats {
    top: f32,
    accel: f32,
    grip: f32,
    mass: f32,
    hurt: f32,
    thirst: f32,
    /// The rival's skill when it is not you.
    skill: f32,
    bias: f32,
    tag: &'static str,
}

const STATS: [Stats; 4] = [
    Stats { top: 40.0, accel: 26.0, grip: 13.0, mass: 1.0, hurt: 1.0, thirst: 1.0, skill: 0.955, bias: -0.8, tag: "Balanced. A lance, a blue plume, no weakness." },
    Stats { top: 38.0, accel: 30.0, grip: 10.5, mass: 1.6, hurt: 0.6, thirst: 1.0, skill: 0.945, bias: 0.9, tag: "A tower shield on wheels. Heavy, hard to hurt, slow to turn." },
    Stats { top: 43.0, accel: 21.0, grip: 14.5, mass: 0.8, hurt: 1.25, thirst: 1.15, skill: 0.97, bias: -0.5, tag: "Fast and slippery. Lovely in the open, a glass jaw in a wall." },
    Stats { top: 40.0, accel: 25.0, grip: 12.0, mass: 1.1, hurt: 0.9, thirst: 0.75, skill: 0.955, bias: 0.6, tag: "A thrifty boost. The crown stays on through a rough race." },
];

#[derive(Clone)]
struct Car {
    id: usize,
    /// Distance driven since the line (negative on the grid).
    d: f32,
    /// Sideways from the centre line, left positive.
    y: f32,
    vy: f32,
    v: f32,
    psi: f32,
    boost: f32,
    boost_idle: f32,
    boosting: bool,
    locked: bool,
    guard: f32,
    hurt: f32,
    note: &'static str,
    note_ttl: f32,
    lap_t0: f32,
    /// Seconds left on the banner for a lap just done.
    lap_flash: f32,
    laps: Vec<f32>,
    finish: Option<f32>,
    walls: u32,
    blocks: u32,
    bumps: u32,
    lava: f32,
}

impl Car {
    fn new(id: usize, d: f32, y: f32) -> Car {
        Car { id, d, y, vy: 0.0, v: 0.0, psi: 0.0, boost: 1.0, boost_idle: 0.0, boosting: false, locked: false, guard: 0.0, hurt: 0.0, note: "", note_ttl: 0.0, lap_t0: 0.0, lap_flash: 0.0, laps: vec![], finish: None, walls: 0, blocks: 0, bumps: 0, lava: 0.0 }
    }

    fn s(&self) -> f32 {
        self.d.rem_euclid(LAP)
    }

    /// How far out the outer engine reaches, and how far out the engines' centres are.
    fn reach(&self) -> f32 {
        (RACERS[self.id].gap + 2.0 * RACERS[self.id].r) * SCALE
    }

    fn mid(&self) -> f32 {
        (RACERS[self.id].gap + RACERS[self.id].r) * SCALE
    }

    /// Sideways speed the steering can make at this speed.
    fn lat(&self) -> f32 {
        STATS[self.id].grip * (0.4 + 0.6 * (self.v / 16.0).min(1.0)) * if self.boosting { 0.8 } else { 1.0 }
    }

    fn notice(&mut self, what: &'static str, ttl: f32) {
        self.note = what;
        self.note_ttl = ttl;
    }

    /// A knock: speed lost (scaled by how tough the knight is), once per `guard`.
    fn crash(&mut self, k: f32, what: &'static str) -> bool {
        if self.guard > 0.0 {
            return false;
        }
        self.v *= (1.0 - k * STATS[self.id].hurt).max(0.12);
        self.guard = 0.4;
        self.hurt = 0.45;
        self.boost *= 0.5;
        self.notice(what, 0.9);
        true
    }

    fn drive(&mut self, inp: &Input, scale: f32) {
        let st = &STATS[self.id];
        if self.locked && self.boost > 0.25 {
            self.locked = false;
        }
        self.boosting = inp.boost && !inp.brake && !self.locked && self.boost > 0.0;
        let gas = if self.boosting { 1.0 } else { inp.throttle };
        if self.boosting {
            self.boost = (self.boost - 0.30 * st.thirst * DT).max(0.0);
            self.boost_idle = 0.0;
            self.locked = self.boost == 0.0;
        } else {
            self.boost_idle += DT;
            if self.boost_idle > 0.7 {
                self.boost = (self.boost + 0.11 * DT).min(1.0);
            }
        }
        let top = st.top * scale * if self.boosting { 1.4 } else { 1.0 };
        let acc = st.accel * if self.boosting { 2.4 } else { 1.0 };
        if inp.brake {
            self.v = (self.v - 46.0 * DT).max(0.0);
        } else if gas > 0.0 {
            if self.v < top {
                self.v += acc * gas * (1.0 - self.v / top) * DT;
            }
        } else {
            self.v = (self.v - (3.0 + 0.004 * self.v * self.v) * DT).max(0.0);
        }
        if self.v > top {
            self.v = (self.v - 16.0 * DT).max(top);
        }
        // steering is snappy: the sideways speed is chased hard
        let cmd = inp.steer * self.lat();
        let rate = if cmd.abs() >= self.vy.abs() || cmd * self.vy < 0.0 { 90.0 } else { 55.0 };
        self.vy += (cmd - self.vy).clamp(-rate * DT, rate * DT);
        let k = curve(self.s());
        let slide = SLIDE * self.v * self.v * k;
        self.y += (self.vy - slide) * DT;
        let lean = (self.vy / self.v.max(12.0)).atan();
        self.psi += (lean * 0.9 - self.psi) * 0.3;
        self.d += self.v * DT / (1.0 - k * self.y).max(0.4);
        self.guard = (self.guard - DT).max(0.0);
        self.hurt = (self.hurt - DT).max(0.0);
        self.note_ttl = (self.note_ttl - DT).max(0.0);
        self.lap_flash = (self.lap_flash - DT).max(0.0);
    }

    /// The pillars, the lava, the walls and the barricades.
    fn hit_track(&mut self, tr: &Track) {
        let (r, s, sign) = (&RACERS[self.id], self.s(), self.y.signum());
        let (mid, reach) = (self.mid(), self.reach());
        if tr.hell {
            if self.y.abs() + mid > H_HALF {
                self.v = (self.v - 30.0 * DT).max(0.0);
                self.lava += DT;
                self.notice("LAVA", 0.3);
            }
            if self.y.abs() + reach > H_HALF + 0.3 {
                let (lo, hi) = (s - 0.5 * r.len * SCALE, s + (0.5 * r.len + 2.2 * r.r) * SCALE);
                let p = 4.0 + 12.0 * ((s - 4.0) / 12.0).round();
                if [p - 12.0, p, p + 12.0].iter().any(|&q| hi >= q - 1.0 && lo <= q + 1.0) {
                    if self.crash(0.5, "PILLAR") {
                        self.walls += 1;
                    }
                    self.y = sign * (H_HALF + 0.3 - reach);
                    self.vy = -sign * 3.0;
                }
            }
        }
        let wall = if tr.hell { H_HALF + H_LAVA } else { 11.0 };
        if self.y.abs() + reach > wall {
            if self.crash(0.4, "WALL") {
                self.walls += 1;
            } else {
                self.v = (self.v - 16.0 * DT).max(0.0);
            }
            self.y = sign * (wall - reach);
            self.vy = -sign * self.vy.abs() * 0.4;
        }
        // barricades: the car spans from its cart (behind) to its rams (ahead)
        for b in &tr.blocks {
            let ds = wrap(s + CENTRE - b.s);
            let (pen_s, pen_y) = (HALF_LEN + b.hl - ds.abs(), reach * 0.9 + b.hw - (self.y - b.y).abs());
            if pen_s > 0.0 && pen_y > 0.0 {
                if pen_s < pen_y * 1.5 {
                    self.d += -ds.signum() * pen_s;
                } else {
                    self.y += (self.y - b.y).signum() * pen_y;
                    self.vy = (self.y - b.y).signum() * 4.0;
                }
                if self.crash(0.55, "BARRICADE") {
                    self.blocks += 1;
                }
            }
        }
    }
}

/// How hard a bend throws a car out (units/s of slide per unit of v^2/R).
const SLIDE: f32 = 0.075;

/// Two cars touching: bump them apart and trade a little speed.
fn bump(a: &mut Car, b: &mut Car) {
    let ds = wrap(b.d - a.d);
    let (wa, wb) = (a.reach() * 0.9, b.reach() * 0.9);
    let dy = b.y - a.y;
    let (pen_s, pen_y) = (2.0 * HALF_LEN - ds.abs(), wa + wb - dy.abs());
    if pen_s <= 0.0 || pen_y <= 0.0 {
        return;
    }
    let (ma, mb) = (STATS[a.id].mass, STATS[b.id].mass);
    let (sa, sb) = (mb / (ma + mb), ma / (ma + mb));
    if pen_s < pen_y * 1.6 {
        // nose to tail: the rear car can go no further in, and gives up most of what it closed by
        let (rear, front) = if ds > 0.0 { (a as &mut Car, b as &mut Car) } else { (b as &mut Car, a as &mut Car) };
        let closing = (rear.v - front.v).max(0.0);
        let (mr, mf) = (STATS[rear.id].mass, STATS[front.id].mass);
        rear.d -= pen_s;
        rear.v = front.v + closing * 0.2 * mf / (mr + mf);
        front.v += closing * 0.25 * mr / (mr + mf);
    } else {
        let dir = if dy >= 0.0 { 1.0 } else { -1.0 };
        a.y -= dir * pen_y * sa;
        b.y += dir * pen_y * sb;
        a.vy -= dir * 4.0 * sa;
        b.vy += dir * 4.0 * sb;
        a.v *= 0.993;
        b.v *= 0.993;
    }
    for c in [a, b] {
        c.bumps += 1;
        c.hurt = c.hurt.max(0.15);
        if c.note_ttl < 0.3 {
            c.notice("BUMP", 0.5);
        }
    }
}

/// A race: four cars, a clock that starts on the grid, and whose car is yours.
#[derive(Clone)]
struct Race {
    cars: [Car; 4],
    player: usize,
    /// Seconds since the grid; the start is at `COUNTDOWN`.
    clock: f32,
    cam_y: f32,
    fov: f32,
}

/// The grid: (distance, sideways) for each of four slots, two abreast.
const GRID: [(f32, f32); 4] = [(-6.0, -3.6), (-10.0, 3.6), (-22.0, -3.6), (-26.0, 3.6)];

/// The rivals' speed against how far they are ahead of you (negative: behind),
/// mildly: a long way ahead eases them a few percent, a long way behind eggs them on.
fn rubber(gap: f32) -> f32 {
    if gap > 0.0 { 1.0 - 0.06 * (gap / 100.0).min(1.0) } else { 1.0 + 0.07 * (-gap / 100.0).min(1.0) }
}

impl Race {
    fn new(player: usize) -> Race {
        let mut slots = [0usize, 1, 2].into_iter();
        let cars = std::array::from_fn(|id| {
            let slot = if id == player { 3 } else { slots.next().unwrap_or(3) };
            Car::new(id, GRID[slot].0, GRID[slot].1)
        });
        let cam_y = cars[player].y * 0.85;
        Race { cars, player, clock: 0.0, cam_y, fov: 0.96 }
    }

    /// Seconds into the race (negative on the grid).
    fn time(&self) -> f32 {
        self.clock - COUNTDOWN
    }

    fn order(&self) -> [usize; 4] {
        let key = |c: &Car| c.finish.unwrap_or(1.0e6 - c.d);
        let mut o = [0, 1, 2, 3];
        o.sort_by(|&a, &b| key(&self.cars[a]).total_cmp(&key(&self.cars[b])));
        o
    }

    fn place(&self, id: usize) -> usize {
        self.order().iter().position(|&i| i == id).unwrap_or(0)
    }

    /// A rival's hands: the racing line, round the cars in the way, a boost on a clear straight.
    fn ai(&self, tr: &Track, i: usize) -> Input {
        let c = &self.cars[i];
        let s = c.s();
        let look = (c.v * 0.2).max(4.0);
        let mut target = tr.line_at(s + look) + STATS[c.id].bias * tr.free_at(s + look);
        // cars near it push it off the line, each the other way, but never into a barricade
        if [0.0, 8.0, 16.0, 24.0].iter().all(|k| tr.free_at(s + k) > 0.95) {
            let mut push = 0.0;
            for o in self.cars.iter().filter(|o| o.id != c.id) {
                let (ahead, dy) = (wrap(o.d - c.d), c.y - o.y);
                if (-7.0..16.0).contains(&ahead) && dy.abs() < 4.6 {
                    let side = if dy.abs() > 0.3 { dy.signum() } else if c.id < o.id { 1.0 } else { -1.0 };
                    push += side * (4.6 - dy.abs()) * 1.1 * if ahead > 0.0 { 1.0 } else { 0.6 };
                }
            }
            target = (target + push).clamp(-6.0, 6.0);
        }
        // a car just ahead in its lane: lift, so it does not ride the car's tail
        let tailing = self.cars.iter().filter(|o| o.id != c.id).any(|o| {
            let (ahead, dy) = (wrap(o.d - c.d), (c.y - o.y).abs());
            (0.0..11.5).contains(&ahead) && dy < 3.9 && c.v > o.v - 1.5
        });
        let lat = c.lat().max(4.0);
        let cmd = (4.0 * (target - c.y) + SLIDE * c.v * c.v * curve(s)).clamp(-lat, lat);
        let clear = curve(s) == 0.0 && curve(s + 40.0) == 0.0 && tr.free_at(s + 30.0) > 0.99 && (target - c.y).abs() < 1.2;
        let behind = c.d < self.cars[self.player].d + 40.0;
        let turn = (self.clock * 0.37) as usize + i * 3;
        Input { steer: cmd / lat, throttle: if tailing { 0.0 } else { 1.0 }, brake: false, boost: !tailing && clear && c.boost > 0.5 && self.time() > 4.0 && (behind || turn.is_multiple_of(4)) }
    }

    fn tick(&mut self, tr: &Track, human: &Input) {
        self.clock += DT;
        let (rt, p) = (self.time(), self.player);
        let pd = self.cars[p].d;
        for i in 0..4 {
            let inp = if rt < 0.0 {
                Input::default()
            } else if i == p && self.cars[p].finish.is_none() {
                *human
            } else {
                self.ai(tr, i)
            };
            let scale = if i == p { 1.0 } else { STATS[self.cars[i].id].skill * rubber(self.cars[i].d - pd) };
            let c = &mut self.cars[i];
            c.drive(&inp, scale);
            c.hit_track(tr);
        }
        for a in 0..4 {
            for b in a + 1..4 {
                let (head, tail) = self.cars.split_at_mut(b);
                bump(&mut head[a], &mut tail[0]);
            }
        }
        for c in &mut self.cars {
            // a lap is counted as the line is crossed, timed to the tick it was crossed in
            let done = c.laps.len() as i32;
            if (c.d / LAP).floor() as i32 > done && c.d > 0.0 {
                let t = rt - (c.d - (done + 1) as f32 * LAP) / c.v.max(1.0);
                c.laps.push(t - c.lap_t0);
                c.lap_t0 = t;
                c.lap_flash = 2.5;
                if c.laps.len() as u32 == LAPS {
                    c.finish = Some(t);
                }
            }
        }
        // the camera
        let c = &self.cars[p];
        self.cam_y += (c.y * 0.85 - self.cam_y) * 0.2;
        self.fov += ((if c.boosting { 1.1 } else { 0.96 }) - self.fov) * 0.12;
    }

    /// The chase camera behind the player's racer, looking down the track ahead of it.
    fn view(&self) -> View3 {
        let c = &self.cars[self.player];
        let s = c.s();
        let shake = c.hurt * 1.4;
        let jolt = v3((self.clock * 61.0).sin() * shake * 0.3, (self.clock * 47.0).cos() * shake * 0.3, (self.clock * 53.0).sin() * shake * 0.25);
        // behind the racer along its own heading (not along the bend), looking a little into the bend
        let (anchor, th) = (at(s, self.cam_y, 0.0), pose(s).2);
        let look = th + 0.25 * curve(s + 8.0) * 16.0;
        let pos = anchor + v3(-19.0 * th.cos(), -19.0 * th.sin(), 6.0) + jolt;
        let aim = anchor + v3(6.0 * look.cos(), 6.0 * look.sin(), 1.2);
        let (dx, dy) = (aim.x - pos.x, aim.y - pos.y);
        View3 { pos, heading_rad: dy.atan2(dx), pitch: ((aim.z - pos.z) / (dx * dx + dy * dy).sqrt()).atan(), fov_rad: self.fov }
    }
}

// ---------------------------------------------------------------- lettering

/// A 5x7 face: seven rows of five bits, the leftmost bit highest.
fn face(ch: char) -> Option<[u8; 7]> {
    Some(match ch.to_ascii_uppercase() {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        _ => return None,
    })
}

/// A cell's lettering as filled rectangles in a `cw` x `chh` pixel cell, for the
/// PNG of a frame (a terminal draws these with its own font).
pub fn glyph(ch: char, cw: usize, chh: usize, mut f: impl FnMut(usize, usize, usize, usize)) {
    match ch {
        '█' => f(0, 0, cw, chh),
        '▀' => f(0, 0, cw, chh / 2),
        '▄' => f(0, chh / 2, cw, chh / 2),
        '░' | '▒' => {
            for (n, gy) in (0..chh).step_by(4).enumerate() {
                for gx in (0..cw).step_by(4) {
                    f(gx + n % 2 * 2, gy, 2, 2);
                }
            }
        }
        _ => {
            if let Some(rows) = face(ch) {
                // 5 columns drawn 8 pixels wide, a pixel of air either side
                const X: [(usize, usize); 5] = [(1, 2), (3, 1), (4, 2), (6, 1), (7, 2)];
                let (px, top) = ((cw.saturating_sub(10)) / 2, chh.saturating_sub(14) / 2);
                for (gy, row) in rows.iter().enumerate() {
                    for (gx, &(x, w)) in X.iter().enumerate() {
                        if row >> (4 - gx) & 1 == 1 {
                            f(px + x, top + gy * 2, w, 2);
                        }
                    }
                }
            }
        }
    }
}

const INK: [u8; 3] = [188, 196, 208];
const DIM: [u8; 3] = [104, 110, 122];
const PANEL: [u8; 3] = [8, 8, 12];
const GOLD: [u8; 3] = [214, 164, 52];
const EMBER: [u8; 3] = [236, 116, 44];
const AZURE: [u8; 3] = [61, 139, 255];

fn mix(a: [u8; 3], b: [u8; 3], k: f32) -> [u8; 3] {
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * k) as u8)
}

/// The cells of a frame, for drawing letters and panels over.
struct Canvas<'a> {
    cells: &'a mut [Cell],
}

impl Canvas<'_> {
    fn cell(&mut self, col: usize, row: usize) -> Option<&mut Cell> {
        (col < CELLS.0 && row < CELLS.1).then(|| &mut self.cells[row * CELLS.0 + col])
    }

    /// A solid ground, `w` x `h` cells.
    fn panel(&mut self, col: usize, row: usize, w: usize, h: usize, bg: [u8; 3]) {
        for r in row..row + h {
            for c in col..col + w {
                if let Some(x) = self.cell(c, r) {
                    *x = Cell { bits: 0, ch: ' ', fg: bg, bg };
                }
            }
        }
    }

    /// Letters from `col`; with a `bg` they sit on that ground, else on the frame.
    fn text(&mut self, col: usize, row: usize, s: &str, fg: [u8; 3], bg: Option<[u8; 3]>) {
        for (i, ch) in s.chars().enumerate() {
            if let Some(x) = self.cell(col + i, row) {
                x.bits = 0;
                x.ch = ch;
                x.fg = fg;
                if let Some(b) = bg {
                    x.bg = b;
                }
            }
        }
    }

    fn centred(&mut self, row: usize, s: &str, fg: [u8; 3], bg: Option<[u8; 3]>) {
        self.text(CELLS.0.saturating_sub(s.chars().count()) / 2, row, s, fg, bg);
    }

    /// Big letters from the 5x7 face: scale 1 is a column and half a row to a
    /// pixel (five columns, four rows a letter), scale 2 two columns and a row.
    /// Returns the width it took.
    fn big(&mut self, col: usize, row: usize, s: &str, scale: usize, fg: [u8; 3]) -> usize {
        let pitch = if scale == 1 { 6 } else { 12 };
        for (n, ch) in s.chars().enumerate() {
            let Some(rows) = face(ch) else { continue };
            let px = |gx: usize, gy: usize| gy < 7 && rows[gy] >> (4 - gx) & 1 == 1;
            for gx in 0..5 {
                if scale == 1 {
                    for r in 0..4 {
                        let (top, bot) = (px(gx, r * 2), px(gx, r * 2 + 1));
                        if let (Some(x), true) = (self.cell(col + n * pitch + gx, row + r), top || bot) {
                            x.bits = 0;
                            x.ch = if top && bot { '█' } else if top { '▀' } else { '▄' };
                            x.fg = fg;
                        }
                    }
                } else {
                    for gy in 0..7 {
                        for k in 0..2 {
                            if let (Some(x), true) = (self.cell(col + n * pitch + gx * 2 + k, row + gy), px(gx, gy)) {
                                x.bits = 0;
                                x.ch = '█';
                                x.fg = fg;
                            }
                        }
                    }
                }
            }
        }
        s.chars().count() * pitch - pitch / 6
    }

    /// Darken the whole frame, ink and ground.
    fn dim(&mut self, k: f32) {
        for c in self.cells.iter_mut() {
            c.fg = c.fg.map(|v| (v as f32 * k) as u8);
            c.bg = c.bg.map(|v| (v as f32 * k) as u8);
        }
    }

    /// A bar of `n` cells, `k` full.
    fn bar(&mut self, col: usize, row: usize, n: usize, k: f32, fg: [u8; 3], bg: [u8; 3]) {
        let full = (k.clamp(0.0, 1.0) * n as f32).round() as usize;
        let s: String = (0..n).map(|i| if i < full { '█' } else { '░' }).collect();
        for (i, ch) in s.chars().enumerate() {
            if let Some(x) = self.cell(col + i, row) {
                x.bits = 0;
                x.ch = ch;
                x.fg = if i < full { fg } else { mix(bg, DIM, 0.55) };
                x.bg = bg;
            }
        }
    }
}

fn clock(t: f32) -> String {
    let t = t.max(0.0);
    format!("{}:{:05.2}", (t / 60.0) as u32, t % 60.0)
}

fn ordinal(place: usize) -> &'static str {
    ["1ST", "2ND", "3RD", "4TH"][place.min(3)]
}

/// A racer's livery colour as it is drawn.
fn livery(course: Course, id: usize) -> [u8; 3] {
    course.paint(RACERS[id].cloth, true).0.map(|c| c as u8)
}

// ---------------------------------------------------------------- the game

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Title,
    Race,
    Results,
}

/// A key the game answers (what is held to drive is read separately).
#[derive(Clone, Copy, PartialEq)]
enum Key {
    Left,
    Right,
    Enter,
    Quit,
    Tab,
    Char(char),
}

/// Where a racer stands to be drawn: its knight, place on the lap, sideways, yaw off the track, size.
#[derive(Clone, Copy)]
struct Stand {
    id: usize,
    s: f32,
    y: f32,
    psi: f32,
    size: f32,
}

struct Game {
    course: Course,
    circuit: Circuit,
    shot: Shot,
    movers: [Mesh; 4],
    mode: Mode,
    sel: usize,
    race: Race,
    /// The finished order, once the player has crossed the line and the rest have followed.
    results: Option<Race>,
    frame: u64,
    paused: bool,
    quit: bool,
    /// Frames since the player crossed the line.
    after: u32,
}

impl Game {
    fn new(course: Course, knight: usize) -> Game {
        Game { course, circuit: Circuit::new(course), shot: Shot::new(course), movers: [Mesh::new(), Mesh::new(), Mesh::new(), Mesh::new()], mode: Mode::Title, sel: knight, race: Race::new(knight), results: None, frame: 0, paused: false, quit: false, after: 0 }
    }

    fn start(&mut self) {
        self.race = Race::new(self.sel);
        self.results = None;
        self.after = 0;
        self.mode = Mode::Race;
    }

    fn switch_course(&mut self) {
        self.course = if self.course == Course::Hell { Course::Canyon } else { Course::Hell };
        self.circuit = Circuit::new(self.course);
        self.shot.course = self.course;
    }

    fn press(&mut self, key: Key) {
        match (self.mode, key) {
            (_, Key::Quit | Key::Char('q')) => self.quit = true,
            (Mode::Title, Key::Left | Key::Char('a')) => self.sel = (self.sel + 3) % 4,
            (Mode::Title, Key::Right | Key::Char('d')) => self.sel = (self.sel + 1) % 4,
            (Mode::Title, Key::Char(c @ '1'..='4')) => self.sel = c as usize - '1' as usize,
            (Mode::Title, Key::Tab | Key::Char('c')) => self.switch_course(),
            (Mode::Title, Key::Enter | Key::Char(' ')) => self.start(),
            (Mode::Race, Key::Char('p')) => self.paused = !self.paused,
            (Mode::Results, Key::Enter | Key::Char('r')) => self.start(),
            (Mode::Results, Key::Char('t')) => self.mode = Mode::Title,
            _ => {}
        }
    }

    /// One tick of the game, with what is held.
    fn update(&mut self, input: &Input) {
        self.frame += 1;
        if self.mode != Mode::Race || self.paused {
            return;
        }
        self.race.tick(&self.circuit.track, input);
        if self.race.cars[self.race.player].finish.is_some() {
            self.after += 1;
            if self.after > 90 {
                // the others finish out of sight; the table is the settled race
                let mut rest = self.race.clone();
                for _ in 0..(60.0 / DT) as usize {
                    if rest.cars.iter().all(|c| c.finish.is_some()) {
                        break;
                    }
                    rest.tick(&self.circuit.track, &Input::default());
                }
                self.results = Some(rest);
                self.mode = Mode::Results;
            }
        }
    }

    /// Set the frame's cells: the scene, then lettering over it.
    fn draw(&mut self) {
        let Game { shot, circuit, movers, mode, race, sel, frame, results, course, paused, .. } = self;
        let t = *frame as f32 * DT;
        let (view, clk, stand): (View3, f32, Vec<Stand>) = if *mode == Mode::Title {
            let (a, car) = (t * 0.55, at(60.0, 0.0, 0.0));
            // the car rides high in the frame: the title above and the roster below leave the middle
            let c = car + v3(0.0, 0.0, -1.2);
            let pos = car + v3(a.cos() * 30.0, a.sin() * 30.0, 7.0);
            let d = c - pos;
            (View3 { pos, heading_rad: d.y.atan2(d.x), pitch: (d.z / (d.x * d.x + d.y * d.y).sqrt()).atan(), fov_rad: 0.8 }, t, vec![Stand { id: *sel, s: 60.0, y: 0.0, psi: 0.0, size: 1.0 }])
        } else {
            (race.view(), race.clock, race.cars.iter().map(|c| Stand { id: c.id, s: c.s(), y: c.y, psi: c.psi, size: SCALE }).collect())
        };
        for (n, &Stand { id, s, y, psi, size }) in stand.iter().enumerate() {
            let m = &mut movers[n];
            m.tris.clear();
            racer(m, id, clk, V3::ZERO);
            let (sn, cs) = (pose(s).2 + psi).sin_cos();
            let o = at(s, y, 0.0);
            for tr in &mut m.tris {
                for p in &mut tr.v {
                    *p = v3(o.x + (p.x * cs - p.y * sn) * size, o.y + (p.x * sn + p.y * cs) * size, p.z * size);
                }
                tr.normal = v3(tr.normal.x * cs - tr.normal.y * sn, tr.normal.x * sn + tr.normal.y * cs, tr.normal.z);
            }
        }
        let mut scene: Vec<(&Mesh, Option<usize>)> = vec![(&circuit.world, None)];
        scene.extend(stand.iter().enumerate().map(|(n, st)| (&movers[n], Some(st.id))));
        shot.take(&scene, &view);
        let mut cv = Canvas { cells: &mut shot.cells };
        match *mode {
            Mode::Title => title(&mut cv, *course, *sel),
            Mode::Race => {
                hud(&mut cv, race, *course);
                if *paused {
                    cv.panel(CELLS.0 / 2 - 24, 14, 48, 8, PANEL);
                    cv.big((CELLS.0 - 35) / 2, 15, "PAUSED", 1, GOLD);
                    cv.centred(20, "P RESUME     Q QUIT", INK, Some(PANEL));
                }
            }
            Mode::Results => {
                cv.dim(0.4);
                finish(&mut cv, results.as_ref().unwrap_or(race), *course);
            }
        }
    }
}

// ---------------------------------------------------------------- screens

fn hud(cv: &mut Canvas, race: &Race, course: Course) {
    let (p, rt) = (race.player, race.time());
    let car = &race.cars[p];
    let place = race.place(p);
    let lap = (car.laps.len() as u32 + 1).min(LAPS);
    cv.panel(0, 0, CELLS.0, 1, PANEL);
    cv.text(2, 0, &format!("LAP {lap}/{LAPS}"), INK, None);
    cv.text(16, 0, &format!("TIME {}", clock(rt)), INK, None);
    cv.text(36, 0, &format!("THIS LAP {}", clock(rt - car.lap_t0)), INK, None);
    let best = car.laps.iter().copied().fold(f32::MAX, f32::min);
    cv.text(62, 0, &if car.laps.is_empty() { "BEST -:--.--".to_string() } else { format!("BEST {}", clock(best)) }, DIM, None);
    let knight = format!("{}  {}", NAMES[car.id], if course == Course::Hell { "HELL" } else { "CANYON" });
    cv.text(CELLS.0 - 2 - knight.len(), 0, &knight, livery(course, car.id), None);
    // place
    cv.panel(1, 2, 17, 6, PANEL);
    cv.big(3, 3, &format!("{}", place + 1), 1, GOLD);
    cv.text(10, 3, ordinal(place), GOLD, None);
    cv.text(10, 5, "OF 4", INK, None);
    // the order
    cv.panel(98, 2, 21, 6, PANEL);
    for (n, &id) in race.order().iter().enumerate() {
        let me = id == p;
        cv.text(100, 3 + n, &format!("{}", n + 1), if me { GOLD } else { DIM }, None);
        cv.text(102, 3 + n, "██", livery(course, id), None);
        cv.text(105, 3 + n, NAMES[id], if me { INK } else { DIM }, None);
        if me {
            cv.text(115, 3 + n, "<", GOLD, None);
        }
    }
    // speed and boost
    cv.panel(0, CELLS.1 - 1, CELLS.0, 1, PANEL);
    cv.text(2, CELLS.1 - 1, &format!("SPEED {:>3} KM/H", (car.v * 6.0).round() as u32), INK, None);
    cv.text(24, CELLS.1 - 1, "BOOST", INK, None);
    let tint = if car.boosting { EMBER } else if car.locked { mix(EMBER, DIM, 0.6) } else { AZURE };
    cv.bar(30, CELLS.1 - 1, 24, car.boost, tint, PANEL);
    cv.text(55, CELLS.1 - 1, &format!("{:>3}%", (car.boost * 100.0).round() as u32), INK, None);
    cv.text(66, CELLS.1 - 1, "ARROWS OR WASD DRIVE   SPACE BOOST   P PAUSE   Q QUIT", DIM, None);
    if car.lap_flash > 0.0 && car.finish.is_none() {
        let t = car.laps.last().copied().unwrap_or(0.0);
        cv.centred(11, &format!(" LAP {} DONE  {} ", car.laps.len(), clock(t)), GOLD, Some(PANEL));
    }
    if car.note_ttl > 0.0 && rt >= 0.0 {
        if car.note == "BUMP" {
            cv.centred(9, " BUMP! ", EMBER, Some(PANEL));
        } else {
            let w = car.note.len() * 6 - 1;
            cv.panel((CELLS.0 - w) / 2 - 2, 8, w + 4, 6, PANEL);
            cv.big((CELLS.0 - w) / 2, 9, car.note, 1, EMBER);
        }
    }
    if rt < 0.0 {
        let n = (-rt - 0.2).ceil().max(0.0) as u32;
        let s = if n == 0 { "GO!".to_string() } else { n.to_string() };
        let w = s.chars().count() * 12 - 2;
        cv.panel((CELLS.0 - w) / 2 - 3, 10, w + 6, 9, PANEL);
        cv.big((CELLS.0 - w) / 2, 11, &s, 2, if n == 0 { AZURE } else { GOLD });
    } else if rt < 0.8 {
        cv.panel((CELLS.0 - 34) / 2 - 3, 10, 40, 9, PANEL);
        cv.big((CELLS.0 - 34) / 2, 11, "GO!", 2, AZURE);
    }
    if let Some(t) = car.finish {
        cv.panel(CELLS.0 / 2 - 28, 11, 56, 8, PANEL);
        cv.big((CELLS.0 - 35) / 2, 12, "FINISH", 1, GOLD);
        cv.centred(17, &format!("{} PLACE   {}", ordinal(place), clock(t)), INK, Some(PANEL));
    }
    minimap(cv, race, course);
}

/// The oval in dots, bottom right, with a mark for each racer.
fn minimap(cv: &mut Canvas, race: &Race, course: Course) {
    let (col, row, w, h) = (100usize, 31usize, 18usize, 7usize);
    cv.panel(col - 1, row - 1, w + 2, h + 2, PANEL);
    let (sx, sy) = ((w * 2 - 3) as f32 / (STRAIGHT + 2.0 * BEND), (h * 4 - 3) as f32 / (2.0 * BEND));
    let dot = |cv: &mut Canvas, s: f32, ink: Option<[u8; 3]>| {
        let (x, y, _) = pose(s);
        let (dx, dy) = (((x + BEND) * sx) as usize + 1, ((2.0 * BEND - y) * sy) as usize + 1);
        if let Some(c) = cv.cell(col + dx / 2, row + dy / 4) {
            if c.ch != ' ' {
                *c = Cell { bits: 0, ch: ' ', fg: DIM, bg: PANEL };
            }
            c.bits |= super::braille_dot_bit(dx % 2, dy % 4);
            c.fg = ink.unwrap_or(if c.fg == PANEL { DIM } else { c.fg });
        }
    };
    for i in 0..120 {
        dot(cv, i as f32 * LAP / 120.0, None);
    }
    for id in (0..4).filter(|&i| i != race.player).chain([race.player]) {
        let c = &race.cars[id];
        let s = c.s();
        let ink = if id == race.player { INK } else { livery(course, id) };
        // a racer is the whole cell lit
        let (x, y, _) = pose(s);
        let (dx, dy) = (((x + BEND) * sx) as usize + 1, ((2.0 * BEND - y) * sy) as usize + 1);
        if let Some(c) = cv.cell(col + dx / 2, row + dy / 4) {
            c.bits = 0xFF;
            c.fg = ink;
        }
    }
}

fn title(cv: &mut Canvas, course: Course, sel: usize) {
    cv.panel(0, 0, CELLS.0, 8, PANEL);
    cv.big((CELLS.0 - 77) / 2, 1, "KNIGHT RACERS", 1, GOLD);
    cv.centred(6, "THREE LAPS OF HELL IN A CHARIOT TOWED BY TWO ENGINES", DIM, Some(PANEL));
    cv.panel(0, 28, CELLS.0, 12, PANEL);
    for (n, name) in NAMES.iter().enumerate() {
        let on = n == sel;
        let ground = if on { livery(course, n) } else { PANEL };
        cv.text(6 + n * 28, 29, &format!(" {} {:<9} ", n + 1, name), if on { PANEL } else { DIM }, Some(ground));
    }
    cv.text(6, 31, STATS[sel].tag, INK, None);
    let st = &STATS[sel];
    let rows = [("TOP SPEED", (st.top - 36.0) / 8.0), ("PULL", (st.accel - 19.0) / 12.0), ("STEERING", (st.grip - 9.0) / 7.0), ("TOUGHNESS", (1.0 / st.hurt - 0.7) / 1.0), ("BOOST", (1.0 / st.thirst - 0.7) / 0.8)];
    for (n, (label, k)) in rows.iter().enumerate() {
        cv.text(6, 33 + n, label, INK, None);
        cv.bar(18, 33 + n, 20, 0.15 + 0.85 * k.clamp(0.0, 1.0), livery(course, sel), PANEL);
    }
    cv.text(60, 33, "COURSE", INK, None);
    cv.text(68, 33, if course == Course::Hell { "HELL" } else { "CANYON" }, GOLD, None);
    cv.text(60, 34, "TAB SWITCHES COURSE", DIM, None);
    cv.text(60, 36, "BOOST DRAINS AND REFILLS. WALLS, PILLARS AND", DIM, None);
    cv.text(60, 37, "BARRICADES COST SPEED, LAVA COSTS MORE.", DIM, None);
    cv.text(6, 39, "1-4 OR LEFT/RIGHT PICK     ENTER RACE     Q QUIT", INK, None);
}

fn finish(cv: &mut Canvas, race: &Race, course: Course) {
    let (x, y, w) = (26usize, 6usize, 68usize);
    cv.panel(x, y, w, 21, PANEL);
    let won = race.place(race.player) == 0;
    let head = if won { "YOU WIN" } else { "RESULTS" };
    cv.big(x + (w - (head.len() * 6 - 1)) / 2, y + 1, head, 1, GOLD);
    cv.text(x + 4, y + 6, "PLACE  KNIGHT        TIME        BEST LAP", DIM, None);
    for (n, &id) in race.order().iter().enumerate() {
        let c = &race.cars[id];
        let me = id == race.player;
        let ink = if me { GOLD } else { INK };
        let row = y + 8 + n * 2;
        cv.text(x + 4, row, ordinal(n), ink, None);
        cv.text(x + 11, row, "██", livery(course, id), None);
        cv.text(x + 14, row, NAMES[id], ink, None);
        cv.text(x + 27, row, &c.finish.map_or("DNF".to_string(), clock), ink, None);
        cv.text(x + 39, row, &if c.laps.is_empty() { "-".to_string() } else { clock(c.laps.iter().copied().fold(f32::MAX, f32::min)) }, ink, None);
        if me {
            cv.text(x + 52, row, "YOU", GOLD, None);
        }
    }
    let p = &race.cars[race.player];
    cv.text(x + 4, y + 17, &format!("YOUR RACE   WALLS {}   BARRICADES {}   BUMPS {}   LAVA {:.1} S", p.walls, p.blocks, p.bumps, p.lava), DIM, None);
    cv.text(x + 4, y + 19, "R OR ENTER RACE AGAIN     T CHANGE KNIGHT     Q QUIT", INK, None);
}

// ---------------------------------------------------------------- the terminal

/// Whether the terminal is ours (so a panic or a signal knows to give it back).
static ACTIVE: AtomicBool = AtomicBool::new(false);
static PUSHED: AtomicBool = AtomicBool::new(false);

/// Give the terminal back: cooked mode, the cursor, the main screen. Safe to call twice.
fn restore() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut out = io::stdout();
    if PUSHED.swap(false, Ordering::SeqCst) {
        let _ = crossterm::execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = write!(out, "\x1b[0m\x1b[?25h\x1b[?1049l");
    let _ = out.flush();
    let _ = terminal::disable_raw_mode();
}

/// The terminal while the game has it: restored when this drops, however it drops.
struct Term {
    enhanced: bool,
}

impl Term {
    fn enter() -> io::Result<Term> {
        terminal::enable_raw_mode()?;
        ACTIVE.store(true, Ordering::SeqCst);
        let enhanced = terminal::supports_keyboard_enhancement().unwrap_or(false);
        let mut out = io::stdout();
        write!(out, "\x1b[?1049h\x1b[?25l\x1b[2J")?;
        if enhanced {
            crossterm::execute!(out, PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES))?;
            PUSHED.store(true, Ordering::SeqCst);
        }
        out.flush()?;
        Ok(Term { enhanced })
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        restore();
    }
}

/// A key that drives: held while down (with key release), or for a moment after each press.
#[derive(Clone, Copy, Default)]
struct Held {
    on: bool,
    stamp: Option<Instant>,
}

impl Held {
    fn active(&self, now: Instant, hold: Duration) -> bool {
        self.on || self.stamp.is_some_and(|t| now.duration_since(t) < hold)
    }
}

struct Keys {
    enhanced: bool,
    /// Left, right, throttle, brake, boost.
    held: [Held; 5],
}

impl Keys {
    fn event(&mut self, k: KeyEvent, game: &mut Game, now: Instant) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c' | 'C')) {
            game.quit = true;
            return;
        }
        let (slot, key) = match k.code {
            KeyCode::Left => (Some(0), Some(Key::Left)),
            KeyCode::Right => (Some(1), Some(Key::Right)),
            KeyCode::Up => (Some(2), None),
            KeyCode::Down => (Some(3), None),
            KeyCode::Enter => (None, Some(Key::Enter)),
            KeyCode::Esc => (None, Some(Key::Quit)),
            KeyCode::Tab => (None, Some(Key::Tab)),
            KeyCode::Char(c) => {
                let c = c.to_ascii_lowercase();
                (match c { 'a' => Some(0), 'd' => Some(1), 'w' => Some(2), 's' => Some(3), ' ' => Some(4), _ => None }, Some(Key::Char(c)))
            }
            _ => (None, None),
        };
        if let Some(i) = slot {
            let h = &mut self.held[i];
            if self.enhanced {
                h.on = k.kind != KeyEventKind::Release;
            } else if k.kind != KeyEventKind::Release {
                h.stamp = Some(now);
            }
        }
        if let (KeyEventKind::Press, Some(key)) = (k.kind, key) {
            game.press(key);
        }
    }

    fn input(&self, now: Instant) -> Input {
        let on = |i: usize, ms: u64| self.held[i].active(now, Duration::from_millis(ms));
        let (left, right, brake) = (on(0, 200), on(1, 200), on(3, 350));
        // without key release the gas is automatic (a held key would stutter on the repeat delay)
        let gas = if self.enhanced { on(2, 0) } else { !brake };
        Input { steer: left as i32 as f32 - right as i32 as f32, throttle: gas as i32 as f32, brake, boost: on(4, 400) }
    }
}

/// Play in this terminal.
pub fn live(course: Course, knight: usize) -> Result<(), Box<dyn Error>> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("the game needs a terminal (try --autopilot to watch a race headless)".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM, signal_hook::consts::SIGHUP] {
        signal_hook::flag::register(sig, stop.clone())?;
    }
    let mut game = Game::new(course, knight);
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let was = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            was(info);
        }));
    });
    let term = Term::enter()?;
    let done = play(&mut game, term.enhanced, &stop);
    drop(term);
    done
}

fn play(game: &mut Game, enhanced: bool, stop: &AtomicBool) -> Result<(), Box<dyn Error>> {
    let tick = Duration::from_secs_f32(DT);
    let mut keys = Keys { enhanced, held: [Held::default(); 5] };
    let (mut prev, mut text, mut out) = (Vec::<String>::new(), String::new(), String::new());
    let mut size = (0u16, 0u16);
    let mut next = Instant::now();
    loop {
        // the rest of the tick goes to the keyboard
        loop {
            let left = next.saturating_duration_since(Instant::now());
            if !event::poll(left)? {
                break;
            }
            match event::read()? {
                Event::Key(k) => keys.event(k, game, Instant::now()),
                Event::Resize(..) => prev.clear(),
                _ => {}
            }
        }
        if game.quit || stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let now = Instant::now();
        let mut steps = 0;
        while next <= now && steps < 3 {
            game.update(&keys.input(now));
            next += tick;
            steps += 1;
        }
        if next <= now {
            next = now + tick;
        }
        let now_size = terminal::size()?;
        if now_size != size {
            size = now_size;
            prev.clear();
            out.clear();
            out.push_str("\x1b[2J");
        }
        let (cols, rows) = (size.0 as usize, size.1 as usize);
        if cols < CELLS.0 || rows < CELLS.1 {
            let msg = format!("KNIGHT RACERS needs a terminal of {}x{}; this one is {cols}x{rows}", CELLS.0, CELLS.1);
            out.push_str(&format!("\x1b[2J\x1b[1;1H\x1b[0m{msg}"));
        } else {
            game.draw();
            text.clear();
            game.shot.ansi(&mut text);
            let (c0, r0) = ((cols - CELLS.0) / 2 + 1, (rows - CELLS.1) / 2 + 1);
            for (i, line) in text.lines().enumerate() {
                if prev.get(i).is_none_or(|p| p != line) {
                    out.push_str(&format!("\x1b[{};{}H{line}", r0 + i, c0));
                    match prev.get_mut(i) {
                        Some(p) => line.clone_into(p),
                        None => prev.push(line.to_string()),
                    }
                }
            }
        }
        let mut so = io::stdout().lock();
        so.write_all(out.as_bytes())?;
        so.flush()?;
        out.clear();
    }
}

// ---------------------------------------------------------------- headless

pub struct Opts {
    pub course: Course,
    pub knight: usize,
    pub script: Option<String>,
    pub frames: Option<u32>,
    pub shots: Option<String>,
    pub every: u32,
}

/// `FRAME keys...` lines: from that tick on, those keys are held.
fn parse(script: &str) -> Result<Vec<(u32, Input)>, String> {
    let mut out = vec![];
    for (n, line) in script.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut words = line.split_whitespace();
        let frame = words.next().and_then(|w| w.parse().ok()).ok_or(format!("line {}: starts with a frame number", n + 1))?;
        let mut input = Input::default();
        for w in words {
            match w {
                "left" | "a" => input.steer += 1.0,
                "right" | "d" => input.steer -= 1.0,
                "up" | "w" => input.throttle = 1.0,
                "down" | "s" => input.brake = true,
                "boost" | "space" => input.boost = true,
                "none" => {}
                _ => return Err(format!("line {}: no key {w:?}", n + 1)),
            }
        }
        out.push((frame, input));
    }
    Ok(out)
}

/// Drive a race with no terminal: a script or a driver at the wheel, a report at the end.
pub fn headless(o: Opts) -> Result<(), Box<dyn Error>> {
    let script = o.script.as_deref().map(|p| std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}")).and_then(|s| parse(&s))).transpose()?;
    let mut game = Game::new(o.course, o.knight);
    let mut text = String::new();
    let mut snap = |game: &mut Game, name: &str| -> Result<(), Box<dyn Error>> {
        if let Some(dir) = &o.shots {
            std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
            game.draw();
            game.shot.save(dir, name, false, &mut text)?;
        }
        Ok(())
    };
    snap(&mut game, "title")?;
    game.start();
    let cap = o.frames.unwrap_or(200 * 30);
    let (go, mut held, mut top, mut shot_at_finish) = ((COUNTDOWN / DT) as u32, Input::default(), 0.0f32, false);
    for f in 0..cap {
        if let Some(s) = &script {
            if let Some(&(_, i)) = s.iter().rev().find(|(at, _)| *at <= f) {
                held = i;
            }
        } else {
            // the driver works the keys a player has: left, right or neither
            held = game.race.ai(&game.circuit.track, game.race.player);
            held.steer = if held.steer.abs() < 0.25 { 0.0 } else { held.steer.signum() };
        }
        game.update(&held);
        top = top.max(game.race.cars[game.race.player].v);
        let p = game.race.cars[game.race.player].clone();
        if f % 90 == 0 {
            println!("t {:>6.1}s  place {}  lap {}  {:>3} km/h  y {:>5.1}  boost {:>3.0}%  {}", game.race.time(), game.race.place(game.race.player) + 1, p.laps.len() + 1, (p.v * 6.0).round(), p.y, p.boost * 100.0, p.note);
        }
        if f == 15 || f == go + 10 || (f > go && (f - go) % o.every.max(1) == 0) {
            snap(&mut game, &if f == 15 { "grid".to_string() } else { format!("race_{f:04}") })?;
        }
        if p.finish.is_some() && !shot_at_finish {
            shot_at_finish = true;
            snap(&mut game, "finish")?;
        }
        if game.mode == Mode::Results {
            snap(&mut game, "results")?;
            break;
        }
    }
    let race = game.results.as_ref().unwrap_or(&game.race);
    println!("{}", if game.mode == Mode::Results { "finished" } else { "stopped before the finish" });
    for (n, &id) in race.order().iter().enumerate() {
        let c = &race.cars[id];
        let laps = c.laps.iter().map(|&t| clock(t)).collect::<Vec<_>>().join(" ");
        println!("{} {:<9} {}  laps {laps}{}", n + 1, NAMES[id], c.finish.map_or(format!("at {:.0} of {:.0}", c.d, LAPS as f32 * LAP), clock), if id == race.player { "   <- you" } else { "" });
    }
    for c in &race.cars {
        println!("{:<9} walls {} barricades {} bump-ticks {} lava {:.1} s", NAMES[c.id], c.walls, c.blocks, c.bumps, c.lava);
    }
    let p = &race.cars[race.player];
    println!("you: top {:.0} km/h, walls {}, barricades {}, bumps {}, lava {:.1} s", top * 6.0, p.walls, p.blocks, p.bumps, p.lava);
    Ok(())
}
