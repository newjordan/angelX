//! Knight Racers courses: sampled centrelines with real elevation, banking
//! and width, the land beyond the walls, hazards, forks and set pieces.
//!
//! This file knows nothing about the rest of the Delve (std and serde only),
//! so a course can be authored, plotted and raced on its own. A course is a
//! `Line` (one sample per metre of centreline) plus the lists the renderer and
//! the sim both read. Everything here is deterministic: no clocks, no random
//! state, the same course on every seat.
//!
//! Conventions. The world is x east, y north, z up, in metres. A heading `th` is
//! radians counter-clockwise from +x, and the left of the road at `th` is
//! `(-sin th, cos th)`. Across the road, `y` is metres left of the centre line
//! (right is negative). Curvature is positive in a left-hand bend. Bank is positive
//! when the road rolls to the left: the left edge is lower, a banked left bend, and
//! `surface(s, y) = height(s) - y * tan(bank)`. Distances `s` are centre-line metres from
//! the start line and wrap each lap; a car's `d` runs on past the lap.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::OnceLock;

/// Where the road is, so the renderer can change the world without guessing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stretch { Open, Canyon, Tunnel, Bowl }

/// The courses of the game. `Proving` is the old Lanternward oval, kept as
/// the short practice run; the other three are the cup.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum CourseId { #[default] Proving, Emberfall, Glassway, Moatwalk }

/// The land a stretch of road runs through. The renderer picks palette, props
/// and sky from this; the sim never reads it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome { Proving, Moor, Forge, Foundry, Magma, Crystal, Cavern, Castle, Town, Moat }

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Proving => "proving ground", Biome::Moor => "moor", Biome::Forge => "forge canyon",
            Biome::Foundry => "foundry", Biome::Magma => "magma flats", Biome::Crystal => "crystal reach",
            Biome::Cavern => "deep cavern", Biome::Castle => "castle", Biome::Town => "lantern town",
            Biome::Moat => "moat",
        }
    }
}

/// What stands on the road. Slag and rockfalls are blocks, jets burn, slicks
/// take the grip away. The renderer draws them; the sim hits with them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HazardKind { Slag, Rockfall, Jet, Slick }

/// A hazard on the road. `period == 0` means always there; otherwise it is
/// live for `on` ticks of every `period` (offset by `phase`), at the fixed 30 Hz.
#[derive(Clone, Copy, Debug)]
pub struct Hazard {
    pub kind: HazardKind,
    pub s: f32,
    pub y: f32,
    pub half_l: f32,
    pub half_w: f32,
    pub period: u32,
    pub on: u32,
    pub phase: u32,
}
impl Hazard {
    pub const fn block(kind: HazardKind, s: f32, y: f32, half_l: f32, half_w: f32) -> Self {
        Self { kind, s, y, half_l, half_w, period: 0, on: 0, phase: 0 }
    }
    pub const fn timed(kind: HazardKind, s: f32, y: f32, half_l: f32, half_w: f32, period: u32, on: u32, phase: u32) -> Self {
        Self { kind, s, y, half_l, half_w, period, on, phase }
    }
    /// Whether it hurts at this race tick.
    pub fn live(&self, tick: u32) -> bool { self.period == 0 || (tick + self.phase) % self.period < self.on }
    /// 0 when it rests, rising to 1 as the next strike nears: the telegraph.
    pub fn warn(&self, tick: u32) -> f32 {
        if self.period == 0 { return 1.; }
        let at = (tick + self.phase) % self.period;
        if at < self.on { 1. } else { 1. - ((self.period - at) as f32 / 36.).min(1.) }
    }
}

/// A slag block. Kept for the first renderer, which reads only these four numbers.
#[derive(Clone, Copy, Debug)]
pub struct Slag { pub s: f32, pub y: f32, pub half_l: f32, pub half_w: f32 }

/// A boost strip down the racing line. `y` is its centre across the road.
#[derive(Clone, Copy, Debug)]
pub struct Strip { pub s0: f32, pub s1: f32, pub half_w: f32, pub y: f32 }

/// Authored set pieces the renderer builds out of the course's own metres.
/// `side` is -1 or +1 for beside the road (right is -1) and 0 for a piece
/// that spans or encloses it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceKind {
    Bridge, Viaduct, Tunnel, Arch, Grandstand, CastleGate, Foundry, Waterfall, CrystalVault,
    ChasmJump, Drawbridge, LavaFall, Statues, Lanterns, Pylons, Gantry, Rampart, Spires,
}
#[derive(Clone, Copy, Debug)]
pub struct Piece { pub kind: PieceKind, pub s0: f32, pub s1: f32, pub side: f32, pub seed: u32 }

/// A stretch where the road is missing. A car that reaches the lip fast
/// enough clears it; one that is too slow falls and is set back. `ramp` is
/// the slope of the launch ramp, which sets how high the car is thrown.
#[derive(Clone, Copy, Debug)]
pub struct Gap { pub s0: f32, pub s1: f32, pub ramp: f32 }

/// One named section of road: its land, its look, and the shape of the land beside it.
#[derive(Clone, Copy, Debug)]
pub struct Zone {
    pub s0: f32,
    pub s1: f32,
    pub biome: Biome,
    pub stretch: Stretch,
    pub name: &'static str,
    pub shape: Shape,
}

/// The land beside the road: a verge `shoulder` metres wide, then a wall of
/// `wall` metres rising over `rise`, or a drop of `sink` metres (chasm, moat).
/// `amp` scales the far hills and mountains.
#[derive(Clone, Copy, Debug)]
pub struct Shape { pub wall: f32, pub rise: f32, pub shoulder: f32, pub sink: f32, pub amp: f32 }
impl Shape {
    pub const fn new(wall: f32, rise: f32, shoulder: f32, sink: f32, amp: f32) -> Self { Self { wall, rise, shoulder, sink, amp } }
    pub const OPEN: Shape = Shape::new(6., 30., 6., 0., 22.);
    pub const CANYON: Shape = Shape::new(34., 14., 2., 0., 70.);
    pub const TUNNEL: Shape = Shape::new(60., 6., 1., 0., 60.);
    pub const CHASM: Shape = Shape::new(0., 20., 2., 70., 90.);
}

/// One road sample: centre, heading, curvature (+left), height, grade, roll
/// (+left, radians: the left edge is lower) and the two half-widths.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample { pub x: f32, pub y: f32, pub th: f32, pub k: f32, pub z: f32, pub g: f32, pub bank: f32, pub wl: f32, pub wr: f32 }

/// A control point of an authored centreline: world x/y, height, half-widths
/// left and right, and roll in degrees (+left: left edge down, a left-hand banked turn).
#[derive(Clone, Copy, Debug)]
pub struct Cp { pub x: f32, pub y: f32, pub z: f32, pub wl: f32, pub wr: f32, pub bank: f32 }
pub const fn cp(x: f32, y: f32, z: f32, w: f32, bank: f32) -> Cp { Cp { x, y, z, wl: w, wr: w, bank } }
pub const fn cpw(x: f32, y: f32, z: f32, wl: f32, wr: f32, bank: f32) -> Cp { Cp { x, y, z, wl, wr, bank } }

/// A sampled centreline, closed (a lap) or open (a fork's own road).
#[derive(Clone, Debug)]
pub struct Line {
    pts: Vec<Sample>,
    step: f32,
    len: f32,
    closed: bool,
    turn: f32,
    /// Distance along the line of each control point it was built from.
    pub knots: Vec<f32>,
}

type V = [f32; 6];

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }
fn smooth(t: f32) -> f32 { let t = t.clamp(0., 1.); t * t * (3. - 2. * t) }

fn mix(a: &V, b: &V, wa: f32, wb: f32) -> V {
    let mut o = [0.; 6];
    for i in 0..6 { o[i] = a[i] * wa + b[i] * wb; }
    o
}

/// Centripetal Catmull-Rom through four control values (Barry-Goldman), at `u` in 0..1 of the middle span.
fn catmull(p: [&V; 4], u: f32) -> V {
    let d = |a: &V, b: &V| ((a[0] - b[0]).hypot(a[1] - b[1])).sqrt().max(1e-3);
    let t0 = 0.;
    let t1 = t0 + d(p[0], p[1]);
    let t2 = t1 + d(p[1], p[2]);
    let t3 = t2 + d(p[2], p[3]);
    let t = lerp(t1, t2, u);
    let a1 = mix(p[0], p[1], (t1 - t) / (t1 - t0), (t - t0) / (t1 - t0));
    let a2 = mix(p[1], p[2], (t2 - t) / (t2 - t1), (t - t1) / (t2 - t1));
    let a3 = mix(p[2], p[3], (t3 - t) / (t3 - t2), (t - t2) / (t3 - t2));
    let b1 = mix(&a1, &a2, (t2 - t) / (t2 - t0), (t - t0) / (t2 - t0));
    let b2 = mix(&a2, &a3, (t3 - t) / (t3 - t1), (t - t1) / (t3 - t1));
    mix(&b1, &b2, (t2 - t) / (t2 - t1), (t - t1) / (t2 - t1))
}

fn v_of(c: &Cp) -> V { [c.x, c.y, c.z, c.wl, c.wr, c.bank.to_radians()] }


/// Monotone cubic interpolation (no overshoot between keys), periodic for a lap.
struct Pchip { x: Vec<f32>, y: Vec<f32>, m: Vec<f32> }
impl Pchip {
    fn new(keys: &[(f32, f32)], period: Option<f32>) -> Pchip {
        assert!(keys.len() >= 3, "a profile needs at least three keys");
        let n = keys.len();
        let mut e: Vec<(f32, f32)> = Vec::new();
        if let Some(p) = period {
            for k in &keys[n - 2..] { e.push((k.0 - p, k.1)); }
            e.extend_from_slice(keys);
            for k in &keys[..2] { e.push((k.0 + p, k.1)); }
        } else {
            e.extend_from_slice(keys);
        }
        for w in e.windows(2) { assert!(w[1].0 > w[0].0, "profile keys must rise: {} then {}", w[0].0, w[1].0); }
        let d: Vec<f32> = e.windows(2).map(|w| (w[1].1 - w[0].1) / (w[1].0 - w[0].0)).collect();
        let h: Vec<f32> = e.windows(2).map(|w| w[1].0 - w[0].0).collect();
        let mut m = vec![0.; e.len()];
        for i in 1..e.len() - 1 {
            if d[i - 1] * d[i] > 0. {
                let (w1, w2) = (2. * h[i] + h[i - 1], h[i] + 2. * h[i - 1]);
                m[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i]);
            }
        }
        m[0] = d[0];
        let last = e.len() - 1;
        m[last] = d[last - 1];
        Pchip { x: e.iter().map(|k| k.0).collect(), y: e.iter().map(|k| k.1).collect(), m }
    }
    fn at(&self, x: f32) -> f32 {
        let i = self.x.partition_point(|k| *k <= x).clamp(1, self.x.len() - 1) - 1;
        let h = self.x[i + 1] - self.x[i];
        let t = ((x - self.x[i]) / h).clamp(0., 1.);
        let (t2, t3) = (t * t, t * t * t);
        (2. * t3 - 3. * t2 + 1.) * self.y[i] + (t3 - 2. * t2 + t) * h * self.m[i] + (-2. * t3 + 3. * t2) * self.y[i + 1] + (t3 - t2) * h * self.m[i + 1]
    }
}

impl Line {
    pub fn len(&self) -> f32 { self.len }
    pub fn samples(&self) -> &[Sample] { &self.pts }
    pub fn is_closed(&self) -> bool { self.closed }
    pub fn step(&self) -> f32 { self.step }

    /// The distance of a fractional control point: `knot(7.5)` is half way from control 7 to 8.
    pub fn knot(&self, u: f32) -> f32 {
        let n = self.knots.len();
        let i = (u.floor().max(0.) as usize).min(n - 1);
        let f = u - i as f32;
        let a = self.knots[i];
        let b = if i + 1 < n { self.knots[i + 1] } else { self.len };
        a + (b - a) * f.max(0.)
    }

    /// A closed lap through the control points.
    pub fn lap(cps: &[Cp]) -> Line {
        let v: Vec<V> = cps.iter().map(v_of).collect();
        let n = v.len();
        let mut segs = Vec::new();
        for i in 0..n {
            segs.push([v[(i + n - 1) % n], v[i], v[(i + 1) % n], v[(i + 2) % n]]);
        }
        Line::build(&segs, true)
    }

    /// An open road from `start` to `end`, leaving and arriving on the given headings.
    pub fn road(start: (Cp, f32), mids: &[Cp], end: (Cp, f32)) -> Line {
        let mut pts: Vec<V> = vec![v_of(&start.0)];
        pts.extend(mids.iter().map(v_of));
        pts.push(v_of(&end.0));
        let reach = |a: &V, b: &V| ((a[0] - b[0]).hypot(a[1] - b[1])) * 0.5 + 6.;
        let (l0, l1) = (reach(&pts[0], &pts[1]), reach(&pts[pts.len() - 2], &pts[pts.len() - 1]));
        let mut before = pts[0];
        before[0] -= start.1.cos() * l0;
        before[1] -= start.1.sin() * l0;
        let mut after = *pts.last().unwrap();
        after[0] += end.1.cos() * l1;
        after[1] += end.1.sin() * l1;
        let mut all = vec![before];
        all.extend(pts);
        all.push(after);
        let segs: Vec<[V; 4]> = (1..all.len() - 2).map(|i| [all[i - 1], all[i], all[i + 1], all[i + 2]]).collect();
        Line::build(&segs, false)
    }


    /// A closed lap through bare plan points (x, y); height, width and roll come from the key methods.
    pub fn plan(pts: &[(f32, f32)]) -> Line {
        let cps: Vec<Cp> = pts.iter().map(|p| cp(p.0, p.1, 0., 8., 0.)).collect();
        Line::lap(&cps)
    }

    fn period(&self) -> Option<f32> { if self.closed { Some(self.len) } else { None } }

    /// Road height by `(s, z)` keys, blended without overshoot: climbs, crests and drops are where the keys say.
    pub fn z_keys(mut self, keys: &[(f32, f32)]) -> Line {
        let mut keys = keys.to_vec();
        keys.sort_by(|a, b| a.0.total_cmp(&b.0));
        keys.dedup_by(|a, b| a.0 - b.0 < 1.0);
        let p = Pchip::new(&keys, self.period());
        for (i, pt) in self.pts.iter_mut().enumerate() { pt.z = p.at(i as f32 * self.step); }
        self.finish()
    }
    /// Half-widths by `(s, left, right)` keys, blended without overshoot.
    pub fn w_keys(mut self, keys: &[(f32, f32, f32)]) -> Line {
        let mut keys = keys.to_vec();
        keys.sort_by(|a, b| a.0.total_cmp(&b.0));
        keys.dedup_by(|a, b| a.0 - b.0 < 1.0);
        let l: Vec<(f32, f32)> = keys.iter().map(|k| (k.0, k.1)).collect();
        let r: Vec<(f32, f32)> = keys.iter().map(|k| (k.0, k.2)).collect();
        let (pl, pr) = (Pchip::new(&l, self.period()), Pchip::new(&r, self.period()));
        for (i, pt) in self.pts.iter_mut().enumerate() {
            let s = i as f32 * self.step;
            pt.wl = pl.at(s);
            pt.wr = pr.at(s);
        }
        self
    }
    /// Roll from the bend: the road leans into every corner, `gain` radians per unit curvature, capped at `max`.
    pub fn auto_bank(mut self, gain: f32, max: f32) -> Line {
        let n = self.pts.len();
        let w = ((28. / self.step) as isize).max(1);
        let ks: Vec<f32> = (0..n).map(|i| {
            let mut acc = 0.;
            for j in -w..=w {
                let at = if self.closed { (i as isize + j).rem_euclid(n as isize) as usize } else { (i as isize + j).clamp(0, n as isize - 1) as usize };
                acc += self.pts[at].k;
            }
            acc / (2 * w + 1) as f32
        }).collect();
        for (pt, k) in self.pts.iter_mut().zip(ks) { pt.bank = (k * gain).clamp(-max, max); }
        self
    }
    /// Straighten the roll to flat over `s0..s1` (a bridge deck, a tunnel floor), easing in and out over `ease` metres.
    pub fn flatten_bank(mut self, s0: f32, s1: f32, ease: f32) -> Line {
        for (i, pt) in self.pts.iter_mut().enumerate() {
            let s = i as f32 * self.step;
            let t = smooth((s - s0) / ease) * smooth((s1 - s) / ease);
            pt.bank *= 1. - t;
        }
        self
    }

    /// A closed lap from explicit samples (the old arc-and-straight circuit).
    pub fn from_samples(pts: Vec<Sample>, len: f32, turn: f32) -> Line {
        let n = pts.len() as f32;
        Line { pts, step: len / n, len, closed: true, turn, knots: Vec::new() }.finish()
    }

    fn build(segs: &[[V; 4]], closed: bool) -> Line {
        // Dense points along the spline, with plan arc length.
        let mut dense: Vec<(f32, V)> = Vec::new();
        let mut cum = 0.;
        let mut knots = Vec::new();
        let mut last: Option<V> = None;
        for seg in segs {
            let chord = (seg[1][0] - seg[2][0]).hypot(seg[1][1] - seg[2][1]);
            let n = ((chord / 0.5).ceil() as usize).max(8);
            knots.push(cum);
            for j in 0..n {
                let v = catmull([&seg[0], &seg[1], &seg[2], &seg[3]], j as f32 / n as f32);
                if let Some(l) = last { cum += (v[0] - l[0]).hypot(v[1] - l[1]); }
                dense.push((cum, v));
                last = Some(v);
            }
        }
        // The seam: back to the first point (closed) or the last control point (open).
        let end = if closed { dense[0].1 } else { segs.last().unwrap()[2] };
        let l = last.unwrap();
        cum += (end[0] - l[0]).hypot(end[1] - l[1]);
        dense.push((cum, end));
        if !closed { knots.push(cum); }
        let len = cum;
        let count = if closed { (len / 1.0).round() as usize } else { (len / 1.0).round() as usize + 1 };
        let step = if closed { len / count as f32 } else { len / (count - 1) as f32 };
        let mut pts = Vec::with_capacity(count);
        let mut at = 0usize;
        for i in 0..count {
            let s = i as f32 * step;
            while at + 2 < dense.len() && dense[at + 1].0 < s { at += 1; }
            let (a, b) = (&dense[at], &dense[at + 1]);
            let f = if b.0 > a.0 { ((s - a.0) / (b.0 - a.0)).clamp(0., 1.) } else { 0. };
            let v = mix(&a.1, &b.1, 1. - f, f);
            pts.push(Sample { x: v[0], y: v[1], z: v[2], wl: v[3], wr: v[4], bank: v[5], ..Sample::default() });
        }
        let mut line = Line { pts, step, len, closed, turn: 0., knots };
        line.heading();
        line.finish()
    }

    /// Headings from the positions, unwrapped so they lerp, and the closing turn.
    fn heading(&mut self) {
        let n = self.pts.len();
        let closed = self.closed;
        let xy: Vec<(f32, f32)> = self.pts.iter().map(|p| (p.x, p.y)).collect();
        let at = |i: isize| -> (f32, f32) {
            let j = if closed { i.rem_euclid(n as isize) as usize } else { i.clamp(0, n as isize - 1) as usize };
            xy[j]
        };
        let raw = |i: isize| { let (a, b) = (at(i - 1), at(i + 1)); (b.1 - a.1).atan2(b.0 - a.0) };
        let unwrap = |mut th: f32, prev: f32| { while th - prev > TAU / 2. { th -= TAU; } while th - prev < -TAU / 2. { th += TAU; } th };
        let mut prev = raw(0);
        self.pts[0].th = prev;
        for i in 1..n {
            prev = unwrap(raw(i as isize), prev);
            self.pts[i].th = prev;
        }
        if closed {
            // The heading the first sample would have after one lap.
            let next = unwrap(raw(n as isize), prev);
            self.turn = ((next - self.pts[0].th) / TAU).round() * TAU;
        }
    }

    /// Curvature and grade from the finished samples, lightly smoothed.
    fn finish(mut self) -> Line {
        let n = self.pts.len();
        let idx = |i: isize, closed: bool| -> usize { if closed { i.rem_euclid(n as isize) as usize } else { i.clamp(0, n as isize - 1) as usize } };
        let th_at = |pts: &Vec<Sample>, i: isize, closed: bool, turn: f32| -> f32 {
            let w = if closed { i.div_euclid(n as isize) } else { 0 };
            pts[idx(i, closed)].th + w as f32 * turn
        };
        let raw_k: Vec<f32> = (0..n).map(|i| {
            let i = i as isize;
            (th_at(&self.pts, i + 1, self.closed, self.turn) - th_at(&self.pts, i - 1, self.closed, self.turn)) / (2. * self.step)
        }).collect();
        let raw_g: Vec<f32> = (0..n).map(|i| {
            let i = i as isize;
            let (a, b) = (self.pts[idx(i - 1, self.closed)].z, self.pts[idx(i + 1, self.closed)].z);
            (b - a) / (2. * self.step)
        }).collect();
        let w = ((5. / self.step) as isize).max(1);
        for i in 0..n {
            let (mut k, mut g, mut c) = (0., 0., 0.);
            for j in -w..=w {
                let at = idx(i as isize + j, self.closed);
                k += raw_k[at];
                g += raw_g[at];
                c += 1.;
            }
            self.pts[i].k = k / c;
            self.pts[i].g = g / c;
        }
        self
    }

    /// The road at distance `s`. Closed lines wrap; open lines clamp to their ends.
    pub fn at(&self, s: f32) -> Sample {
        let n = self.pts.len();
        let s = if self.closed { s.rem_euclid(self.len) } else { s.clamp(0., self.len) };
        let u = s / self.step;
        let i = (u.floor() as usize).min(if self.closed { n - 1 } else { n - 2 });
        let f = u - i as f32;
        let a = &self.pts[i];
        let (b, turn) = if self.closed {
            let j = (i + 1) % n;
            (&self.pts[j], if j == 0 { self.turn } else { 0. })
        } else { (&self.pts[i + 1], 0.) };
        Sample {
            x: lerp(a.x, b.x, f), y: lerp(a.y, b.y, f), th: lerp(a.th, b.th + turn, f),
            k: lerp(a.k, b.k, f), z: lerp(a.z, b.z, f), g: lerp(a.g, b.g, f),
            bank: lerp(a.bank, b.bank, f), wl: lerp(a.wl, b.wl, f), wr: lerp(a.wr, b.wr, f),
        }
    }
}

/// An alternate road between two points of the main line. `s0..s1` is the
/// stretch of main line it replaces; `line` is its own centreline. A car on
/// the fork carries the same `s` as on the main line, so the order of the
/// field never changes: a shorter fork simply covers its `s` range faster.
/// It leaves from the `side` half of the main road, the way a gore splits.
#[derive(Clone, Debug)]
pub struct Fork {
    pub name: &'static str,
    pub s0: f32,
    pub s1: f32,
    /// -1 right, +1 left: the side of the main road it leaves from.
    pub side: f32,
    /// Where the fork road sits across the main road when it leaves and when it rejoins.
    pub off0: f32,
    pub off1: f32,
    pub shortcut: bool,
    pub line: Line,
    pub biome: Biome,
    pub shape: Shape,
    pub hazards: Vec<Hazard>,
    pub strips: Vec<Strip>,
    pub pieces: Vec<Piece>,
}
impl Fork {
    /// Road length of the fork against the main line's length over the same range.
    pub fn scale(&self) -> f32 { self.line.len() / (self.s1 - self.s0) }
    fn local(&self, s: f32) -> f32 { (s - self.s0) * self.scale() }
    pub fn sample(&self, s: f32) -> Sample { self.line.at(self.local(s)) }
    pub fn pose(&self, s: f32) -> (f32, f32, f32) { let p = self.sample(s); (p.x, p.y, p.th) }
    pub fn height(&self, s: f32) -> f32 { self.sample(s).z }
    pub fn width(&self, s: f32) -> (f32, f32) { let p = self.sample(s); (p.wl, p.wr) }
    pub fn bank(&self, s: f32) -> f32 { self.sample(s).bank }
    pub fn curve(&self, s: f32) -> f32 { self.sample(s).k }
    /// Whether the main-line distance `s` is inside this fork.
    pub fn spans(&self, s: f32) -> bool { s >= self.s0 && s < self.s1 }
}


impl Fork {
    /// A fork leaving the `side` half of the main road at `s0` and rejoining at `s1`,
    /// through the world points `mids`. It leaves and arrives on the main road's heading,
    /// sitting across the middle of that half, as wide as that half is.
    pub fn new(main: &Line, name: &'static str, s0: f32, s1: f32, side: f32, mids: &[Cp], shortcut: bool, biome: Biome, shape: Shape) -> Fork {
        let (a, b) = (main.at(s0), main.at(s1));
        let half = |p: &Sample| if side > 0. { p.wl } else { p.wr } * 0.5;
        let (off0, off1) = (half(&a), half(&b));
        let put = |p: &Sample, off: f32| Cp { x: p.x - side * off * p.th.sin(), y: p.y + side * off * p.th.cos(), z: p.z, wl: off, wr: off, bank: 0. };
        // A lead-in and a lead-out hold the fork close to the main road's heading at both ends;
        // where the main road is bending away they lean half way toward the chord, so the
        // fork leaves like a gore, not like a kink.
        let wrap = |d: f32| (d + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let (p0, p1) = (put(&a, off0), put(&b, off1));
        let first = mids.first().map_or((p1.x, p1.y), |m| (m.x, m.y));
        let last = mids.last().map_or((p0.x, p0.y), |m| (m.x, m.y));
        let h0 = a.th + 0.45 * wrap((first.1 - p0.y).atan2(first.0 - p0.x) - a.th);
        let h1 = b.th + 0.45 * wrap((p1.y - last.1).atan2(p1.x - last.0) - b.th);
        let reach = ((s1 - s0) * 0.1).clamp(14., 36.);
        let mut all = vec![Cp { x: p0.x + reach * h0.cos(), y: p0.y + reach * h0.sin(), z: a.z + reach * a.g, wl: off0, wr: off0, bank: 0. }];
        all.extend_from_slice(mids);
        all.push(Cp { x: p1.x - reach * h1.cos(), y: p1.y - reach * h1.sin(), z: b.z - reach * b.g, wl: off1, wr: off1, bank: 0. });
        let line = Line::road((p0, h0), &all, (p1, h1)).auto_bank(30., 0.24);
        Fork { name, s0, s1, side, off0, off1, shortcut, line, biome, shape, hazards: Vec::new(), strips: Vec::new(), pieces: Vec::new() }
    }
}

/// Everything about the road at one distance, on the main line or a fork.
#[derive(Clone, Copy, Debug)]
pub struct Frame { pub x: f32, pub y: f32, pub th: f32, pub k: f32, pub z: f32, pub g: f32, pub bank: f32, pub wl: f32, pub wr: f32, pub scale: f32 }

/// One candidate for `ground`: a road sample with its land.
#[derive(Clone, Copy)]
struct Cand { x: f32, y: f32, c: f32, s: f32, z: f32, g: f32, bank: f32, wl: f32, wr: f32, shape: Shape }

/// A bilinear grid of the course's blurred elevation and mountain scale.
struct Far { x0: f32, y0: f32, cell: f32, nx: usize, ny: usize, z: Vec<f32>, amp: Vec<f32> }
impl Far {
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        let fx = ((x - self.x0) / self.cell).clamp(0., (self.nx - 1) as f32 - 1e-3);
        let fy = ((y - self.y0) / self.cell).clamp(0., (self.ny - 1) as f32 - 1e-3);
        let (ix, iy) = (fx as usize, fy as usize);
        let (tx, ty) = (fx - ix as f32, fy - iy as f32);
        let g = |v: &Vec<f32>, a: usize, b: usize| v[b * self.nx + a];
        let bl = |v: &Vec<f32>| lerp(lerp(g(v, ix, iy), g(v, ix + 1, iy), tx), lerp(g(v, ix, iy + 1), g(v, ix + 1, iy + 1), tx), ty);
        (bl(&self.z), bl(&self.amp))
    }
}

fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca6b) ^ 0x27d4_eb2f;
    h ^= h >> 15; h = h.wrapping_mul(0x2c1b_3c6d); h ^= h >> 12; h = h.wrapping_mul(0x297a_2d39); h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.
}
fn noise(x: f32, y: f32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (smooth(x - ix), smooth(y - iy));
    let (ix, iy) = (ix as i32, iy as i32);
    lerp(lerp(hash(ix, iy), hash(ix + 1, iy), fx), lerp(hash(ix, iy + 1), hash(ix + 1, iy + 1), fx), fy)
}
/// Ridged value noise in 0..1: ridges and spurs, not blobs.
fn ridges(x: f32, y: f32) -> f32 {
    let mut total = 0.;
    let mut amp = 0.5;
    let mut norm = 0.;
    let (mut fx, mut fy) = (x, y);
    for o in 0..4 {
        let n = 1. - (noise(fx + o as f32 * 17., fy - o as f32 * 9.) * 2. - 1.).abs();
        total += n * n * amp;
        norm += amp;
        amp *= 0.5;
        fx *= 2.07;
        fy *= 2.07;
    }
    total / norm
}

/// How far from the road the near profile reaches, metres, and the grid cell that finds it.
const REACH: f32 = 72.;
const CELL: f32 = REACH;

/// The authored road, in the order the author lists it: a name, its length and
/// its extras. Built once; every method is a lookup.
pub struct Course {
    pub id: CourseId,
    name: &'static str,
    blurb: &'static str,
    laps: u32,
    pub(super) line: Line,
    pub(super) zones: Vec<Zone>,
    pub(super) hazards: Vec<Hazard>,
    pub(super) slag: Vec<Slag>,
    pub(super) strips: Vec<Strip>,
    pub(super) pieces: Vec<Piece>,
    pub(super) forks: Vec<Fork>,
    pub(super) gaps: Vec<Gap>,
    pub(super) landmarks: Vec<f32>,
    cands: Vec<Cand>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    far: Far,
    /// (x0, y0, x1, y1) of everything the course touches.
    bounds: (f32, f32, f32, f32),
}

/// What an author hands over; `Course::new` builds the lookups.
pub struct Spec {
    pub id: CourseId,
    pub name: &'static str,
    pub blurb: &'static str,
    pub laps: u32,
    pub line: Line,
    pub zones: Vec<Zone>,
    pub hazards: Vec<Hazard>,
    pub strips: Vec<Strip>,
    pub pieces: Vec<Piece>,
    pub forks: Vec<Fork>,
    pub gaps: Vec<Gap>,
    pub landmarks: Vec<f32>,
}

impl std::fmt::Debug for Course { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Course({:?}, {:.0} m)", self.id, self.line.len) } }
impl std::fmt::Debug for Far { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Far({}x{})", self.nx, self.ny) } }

impl Course {
    pub fn new(spec: Spec) -> Course {
        let Spec { id, name, blurb, laps, line, zones, hazards, strips, pieces, forks, gaps, landmarks } = spec;
        let slag = hazards.iter().filter(|h| h.kind == HazardKind::Slag)
            .map(|h| Slag { s: h.s, y: h.y, half_l: h.half_l, half_w: h.half_w }).collect();
        let mut course = Course {
            id, name, blurb, laps, line, zones, hazards, slag, strips, pieces, forks, gaps, landmarks,
            cands: Vec::new(), grid: HashMap::new(),
            far: Far { x0: 0., y0: 0., cell: 64., nx: 2, ny: 2, z: vec![0.; 4], amp: vec![0.; 4] },
            bounds: (0., 0., 0., 0.),
        };
        course.index();
        course
    }

    fn shape_at(&self, s: f32) -> Shape { self.zone_at(s).shape }
    fn zone_at(&self, s: f32) -> &Zone {
        let s = s.rem_euclid(self.line.len);
        self.zones.iter().find(|z| s >= z.s0 && s < z.s1).unwrap_or_else(|| self.zones.last().unwrap())
    }

    /// Build the spatial lookups for `ground` from every road the course has.
    fn index(&mut self) {
        let mut cands = Vec::new();
        let mut push = |line: &Line, s_of: &dyn Fn(f32) -> f32, shape_of: &dyn Fn(f32) -> Shape, spacing: f32| {
            let mut u = 0.;
            while u <= line.len() {
                let p = line.at(u);
                cands.push(Cand { x: p.x, y: p.y, c: p.th.cos(), s: p.th.sin(), z: p.z, g: p.g, bank: p.bank, wl: p.wl, wr: p.wr, shape: shape_of(s_of(u)) });
                u += spacing;
            }
        };
        let main_shape = |s: f32| self.shape_at(s);
        push(&self.line, &|u| u, &main_shape, 5.);
        for f in &self.forks {
            let (s0, scale, shape) = (f.s0, f.scale(), f.shape);
            push(&f.line, &|u| s0 + u / scale, &|_| shape, 5.);
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for c in &cands { x0 = x0.min(c.x); y0 = y0.min(c.y); x1 = x1.max(c.x); y1 = y1.max(c.y); }
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, c) in cands.iter().enumerate() {
            grid.entry(((c.x / CELL).floor() as i32, (c.y / CELL).floor() as i32)).or_default().push(i as u32);
        }
        // The far land: blurred elevation and mountain scale on a coarse grid.
        let margin = 480.;
        let cell = 64.;
        let (gx0, gy0) = (x0 - margin, y0 - margin);
        let nx = (((x1 - x0) + 2. * margin) / cell).ceil() as usize + 2;
        let ny = (((y1 - y0) + 2. * margin) / cell).ceil() as usize + 2;
        let coarse: Vec<&Cand> = cands.iter().step_by(4).collect();
        let sigma2 = 2. * 190. * 190.;
        let (mut zs, mut amps) = (vec![0.; nx * ny], vec![0.; nx * ny]);
        for j in 0..ny {
            for i in 0..nx {
                let (px, py) = (gx0 + i as f32 * cell, gy0 + j as f32 * cell);
                let (mut sw, mut sz, mut sa) = (1e-9, 0., 0.);
                for c in &coarse {
                    let d2 = (c.x - px).powi(2) + (c.y - py).powi(2);
                    if d2 > 9. * sigma2 { continue; }
                    let w = (-d2 / sigma2).exp();
                    sw += w; sz += w * c.z; sa += w * c.shape.amp;
                }
                zs[j * nx + i] = sz / sw;
                amps[j * nx + i] = sa / sw;
            }
        }
        self.far = Far { x0: gx0, y0: gy0, cell, nx, ny, z: zs, amp: amps };
        self.bounds = (x0, y0, x1, y1);
        self.cands = cands;
        self.grid = grid;
    }

    pub fn name(&self) -> &'static str { self.name }
    pub fn blurb(&self) -> &'static str { self.blurb }
    pub fn laps(&self) -> u32 { self.laps }
    /// Centreline length of one lap, metres.
    pub fn lap(&self) -> f32 { self.line.len }
    /// Where the world lies, as (min x, min y, max x, max y).
    pub fn bounds(&self) -> (f32, f32, f32, f32) { self.bounds }
    pub fn line(&self) -> &Line { &self.line }

    pub fn sample(&self, s: f32) -> Sample { self.line.at(s) }
    pub fn pose(&self, s: f32) -> (f32, f32, f32) { let p = self.line.at(s); (p.x, p.y, p.th) }
    pub fn height(&self, s: f32) -> f32 { self.line.at(s).z }
    pub fn grade(&self, s: f32) -> f32 { self.line.at(s).g }
    /// Road roll in radians, +left (the left edge is lower).
    pub fn bank(&self, s: f32) -> f32 { self.line.at(s).bank }
    /// Half-widths of the racing surface, left then right.
    pub fn width(&self, s: f32) -> (f32, f32) { let p = self.line.at(s); (p.wl, p.wr) }
    pub fn curve(&self, s: f32) -> f32 { self.line.at(s).k }
    /// Surface height `y` metres left of the centre line at `s`.
    pub fn surface(&self, s: f32, y: f32) -> f32 { let p = self.line.at(s); p.z - y * p.bank.tan() }

    /// Inside line of the next corner, in track-left metres. Straights stay central.
    pub fn racing_line(&self, s: f32) -> f32 {
        let p = self.line.at(s + 22.);
        let room = (p.wl.min(p.wr) - 3.4).max(1.5);
        (p.k * 240.).clamp(-room, room)
    }

    pub fn zone(&self, s: f32) -> (Biome, &'static str) { let z = self.zone_at(s); (z.biome, z.name) }
    pub fn zones(&self) -> &[Zone] { &self.zones }
    pub fn stretch(&self, s: f32) -> Stretch { self.zone_at(s).stretch }
    pub fn hazards(&self) -> &[Hazard] { &self.hazards }
    pub fn slag(&self) -> &[Slag] { &self.slag }
    pub fn strips(&self) -> &[Strip] { &self.strips }
    pub fn pieces(&self) -> &[Piece] { &self.pieces }
    pub fn forks(&self) -> &[Fork] { &self.forks }
    pub fn gaps(&self) -> &[Gap] { &self.gaps }
    pub fn gap_at(&self, s: f32) -> Option<&Gap> { let s = s.rem_euclid(self.line.len); self.gaps.iter().find(|g| s >= g.s0 && s < g.s1) }
    /// Section starts, excluding the grid line (the start gate already marks it).
    pub fn landmarks(&self) -> Vec<f32> { self.landmarks.clone() }

    /// The fork whose range contains the main-line distance `s`, if any.
    pub fn fork_at(&self, s: f32) -> Option<(usize, &Fork)> {
        let s = s.rem_euclid(self.line.len);
        self.forks.iter().enumerate().find(|(_, f)| f.spans(s))
    }

    /// The road at `s` on `route` (0 is the main line, `n` the n-th fork), with the
    /// metres of that road per metre of `s`.
    pub fn frame(&self, route: u8, s: f32) -> Frame {
        let (p, scale) = match route.checked_sub(1).and_then(|i| self.forks.get(i as usize)) {
            Some(f) => (f.sample(s.rem_euclid(self.line.len)), f.scale()),
            None => (self.line.at(s), 1.),
        };
        Frame { x: p.x, y: p.y, th: p.th, k: p.k, z: p.z, g: p.g, bank: p.bank, wl: p.wl, wr: p.wr, scale }
    }

    /// Terrain height anywhere in the world, road or not. On the road it is the
    /// road bed; beside it, the verge, then the canyon wall or the drop into the
    /// chasm (over a chasm the bed under the road is the chasm floor: the bridge
    /// deck is the renderer's); far off, ridges and mountains rising from the
    /// course's own level. Continuous everywhere.
    pub fn ground(&self, x: f32, y: f32) -> f32 {
        let (cx, cy) = ((x / CELL).floor() as i32, (y / CELL).floor() as i32);
        let cells = || (cy - 1..=cy + 1).flat_map(move |j| (cx - 1..=cx + 1).filter_map(move |i| self.grid.get(&(i, j)))).flatten();
        let dist = |c: &Cand| (x - c.x).hypot(y - c.y);
        let d_min = cells().map(|&n| dist(&self.cands[n as usize])).fold(f32::MAX, f32::min);
        let (mut sw, mut sv, mut off_min) = (0f32, 0f32, f32::MAX);
        if d_min <= REACH {
            for &n in cells() {
                let c = &self.cands[n as usize];
                let d = dist(c);
                if d > d_min + 12. { continue; }
                let (dx, dy) = (x - c.x, y - c.y);
                let along = dx * c.c + dy * c.s;
                let lat = -dx * c.s + dy * c.c;
                let half = lerp(c.wr, c.wl, 0.5 + 0.5 * (lat / 2.).clamp(-1., 1.));
                let off = (d - half).max(0.);
                let sh = &c.shape;
                let fall = -sh.sink * smooth(off / 6.);
                let up = sh.wall * smooth((off - sh.shoulder) / sh.rise);
                let v = c.z + c.g * along - c.bank.tan() * lat.clamp(-half, half) + fall + up;
                let w = (-((d - d_min) / 3.5).powi(2)).exp();
                sw += w; sv += w * v;
                off_min = off_min.min(off);
            }
        }
        let (zb, amp) = self.far.at(x, y);
        let far = zb + amp * (0.12 + 1.05 * ridges(x * 0.0032, y * 0.0032)) - 6.;
        if sw == 0. { return far; }
        let near = sv / sw;
        let t = smooth((d_min - 0.35 * REACH) / (0.65 * REACH));
        lerp(near, far, t)
    }
}

/// The course with this id, built on first use.
pub fn course(id: CourseId) -> &'static Course {
    static C: [OnceLock<Course>; 4] = [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let slot = match id { CourseId::Proving => 0, CourseId::Emberfall => 1, CourseId::Glassway => 2, CourseId::Moatwalk => 3 };
    C[slot].get_or_init(|| super::courses::build(id))
}

/// Every course: the practice run first, then the cup in running order.
pub fn courses() -> &'static [CourseId] { &[CourseId::Proving, CourseId::Emberfall, CourseId::Glassway, CourseId::Moatwalk] }

/// The cup: the big courses, in running order.
pub fn cup() -> &'static [CourseId] { &[CourseId::Emberfall, CourseId::Glassway, CourseId::Moatwalk] }
