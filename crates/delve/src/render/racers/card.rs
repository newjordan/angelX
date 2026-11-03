//! The loop card's shots: the cockpit's `/loop` card shows the race's own
//! hell track and knight. Each is a short film rendered once on the CPU
//! with the race's raster and played back by the window: the knight on the
//! grid with the flag waving, the flag dropping and the knight away, the
//! knight on a crag calling his shot at a far keep, and the racer from
//! above at every heading for the corner widget's little circuit.

use super::{
    BLUE, Course, H_HALF, Hold, IRON, LAVA, LAVA_HOT, Part, RACERS, Raster, STEEL, cuboid, dungeon_span, gate,
    racer, rod, spike, standing_knight, tri,
};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::{Mesh, Tri, mat};
use crate::render::world3d::raster::View3;
use std::f32::consts::{PI, TAU};

/// The knight the card shows: Excalibur, in Excalibur Blue.
pub const KNIGHT: usize = 0;

/// Frames of a film, all one size, straight RGBA.
#[derive(Clone, Debug, Default)]
pub struct Film {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub frames: Vec<Vec<u8>>,
}

impl Film {
    /// The frames as one picture, `cols` to a row, left to right then down.
    pub fn sheet(&self, cols: usize) -> (u32, u32, Vec<u8>) {
        let cols = cols.clamp(1, self.frames.len().max(1));
        let rows = self.frames.len().div_ceil(cols).max(1);
        let (fw, fh) = (self.width as usize, self.height as usize);
        let (w, h) = (fw * cols, fh * rows);
        let mut out = vec![0u8; w * h * 4];
        for (i, frame) in self.frames.iter().enumerate() {
            let (ox, oy) = (i % cols * fw, i / cols * fh);
            for y in 0..fh {
                let from = &frame[y * fw * 4..(y + 1) * fw * 4];
                out[((oy + y) * w + ox) * 4..][..fw * 4].copy_from_slice(from);
            }
        }
        (w as u32, h as u32, out)
    }
}

/// The films are drawn this many times larger and shrunk, so their edges
/// come out smooth.
const SUPER: usize = 2;

/// Seconds the idle film loops over, and its frames.
pub const IDLE_SECONDS: f32 = 2.0;
pub const IDLE_FRAMES: usize = 24;
/// The start: the flag drops and the knight goes.
pub const LAUNCH_SECONDS: f32 = 1.1;
pub const LAUNCH_FRAMES: usize = 22;
/// The goal shot loops its banner and cape over this long.
pub const GOAL_SECONDS: f32 = 1.6;
pub const GOAL_FRAMES: usize = 12;

// ---------------------------------------------------------------- the grid

/// Where the knight waits: his engines just short of the line.
const GRID_X: f32 = -3.2;
/// The racers are built big for the cinematic; on the grid as on the circuit.
const GRID_SCALE: f32 = 0.72;

/// The straight by the start: the dungeon both ways, cut away on the near
/// side like a stage set (its lava kept, its wall, pillars and vault gone)
/// so the camera can stand back far enough for the profile.
fn grid_world() -> Mesh {
    let mut m = dungeon_span(-48.0, 72.0, 0.0);
    cut_away(&mut m);
    m
}

/// The start gate and the starter's flag, on the same cut-away set.
fn grid_props(t: f32, drop: f32) -> Mesh {
    let mut m = Mesh::new();
    gate(&mut m, Course::Hell);
    flag(&mut m, t, drop);
    cut_away(&mut m);
    m
}

/// Off with whatever stands on the near side of the track.
fn cut_away(m: &mut Mesh) {
    m.tris.retain(|t| {
        let c = (t.v[0] + t.v[1] + t.v[2]) / 3.0;
        !(c.y < -H_HALF - 0.2 && c.z > -0.25) && !(c.y < -2.0 && c.z > 4.5)
    });
}

/// The dungeon behind the knight, sunk back: dimmer and greyer, so the
/// racer and the line carry the frame.
pub fn backdrop(c: [u8; 3]) -> [u8; 3] {
    let c = c.map(f32::from);
    let grey = 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
    std::array::from_fn(|i| ((c[i] + (grey - c[i]) * 0.45) * 0.62) as u8)
}

/// The starter's chequered flag on its pole on the far side of the line,
/// waving at `t`; `drop` (0 to 1) brings it down in one stroke.
fn flag(m: &mut Mesh, t: f32, drop: f32) {
    let foot = v3(2.0, 6.2, 0.0);
    let top = foot + v3(0.0, 0.0, 3.9);
    rod(m, foot, top, 0.07, IRON);
    spike(m, top, top + v3(0.0, 0.0, 0.35), 0.11, STEEL);
    // The cloth hangs off the pole toward the knight; dropping, the pole's
    // head swings it down toward the track like a starter's arm.
    let (n, rows) = (8usize, 5usize);
    let (len, tall) = (2.6, 1.6);
    let fall = drop.clamp(0.0, 1.0);
    let swing = fall * fall * (3.0 - 2.0 * fall) * 1.25;
    let point = |i: usize, j: usize| {
        let (u, w) = (i as f32 / n as f32, j as f32 / rows as f32);
        let wave = (u * 5.2 - t * TAU * 1.5).sin() * 0.32 * u + (u * 2.1 - t * TAU * 0.5).sin() * 0.12 * u;
        let along = -len * u;
        let down = -tall * w - 0.12 * u * u;
        // Swing the cloth's plane about the pole head as the flag falls.
        let (s, c) = swing.sin_cos();
        let (x, z) = (along * c - down * s, along * s + down * c);
        top + v3(x * 0.92, wave, z - 0.1)
    };
    for i in 0..n {
        for j in 0..rows {
            let k = if (i + j) % 2 == 0 { STEEL } else { IRON };
            m.push_quad(point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1), k);
        }
    }
}

/// A racer as the circuit builds it: the knight's own, scaled to the grid,
/// with its nose at `x`.
fn grid_racer(t: f32, x: f32, burn: f32) -> Mesh {
    let mut m = Mesh::new();
    racer(&mut m, KNIGHT, t, V3::ZERO);
    if burn > 0.0 {
        afterburn(&mut m, KNIGHT, burn, t);
    }
    for tr in &mut m.tris {
        for p in &mut tr.v {
            *p = v3(x + p.x * GRID_SCALE, p.y * GRID_SCALE, p.z * GRID_SCALE);
        }
    }
    m
}

/// The launch: each engine's plume thrown long and hot behind it.
fn afterburn(m: &mut Mesh, i: usize, power: f32, t: f32) {
    let s = &RACERS[i];
    let gy = s.gap + s.r;
    for (y, z) in [(-gy, 1.2), (gy, 1.23)] {
        let back = v3(-s.len * 0.5 - 0.15, y, z);
        let flick = 1.0 + 0.18 * (t * 41.0 + y).sin();
        spike(m, back, back - v3(s.r * (3.0 + 4.5 * power) * flick, 0.0, 0.0), s.r * 0.7, mat::FIRE);
        spike(m, back, back - v3(s.r * (1.8 + 3.0 * power) * flick, 0.0, 0.0), s.r * 0.42, LAVA_HOT);
        spike(m, back, back - v3(s.r * (1.0 + 2.0 * power), 0.0, 0.0), s.r * 0.2, BLUE);
    }
}

/// The grid camera: back across the track past the cut-away lava, the
/// knight in three-quarter profile heading right for the line, drifting a
/// little on the loop.
fn grid_view(u: f32, shake: f32) -> View3 {
    let a = u * TAU;
    let look = v3(-2.3 + 0.35 * a.sin(), 1.0, 1.55);
    let pos = v3(5.5 + 0.8 * a.sin(), -21.5 + 0.3 * a.cos(), 4.4 + 0.15 * (a + 0.8).sin());
    let d = look - pos;
    let jolt = shake * (u * 97.0).sin() * 0.01;
    View3 { pos, heading_rad: d.y.atan2(d.x) + jolt, pitch: (d.z / (d.x * d.x + d.y * d.y).sqrt()).atan() + jolt * 0.6, fov_rad: 0.82 }
}

/// The knight on the grid, the flag waving, the camera drifting: a loop of
/// `IDLE_FRAMES` over `IDLE_SECONDS` at `width` by `height` pixels.
pub fn idle(width: u32, height: u32) -> Film {
    let world = grid_world();
    let mut raster = frame_raster(width, height);
    let mut film = Film { width, height, fps: IDLE_FRAMES as f32 / IDLE_SECONDS, frames: Vec::with_capacity(IDLE_FRAMES) };
    for f in 0..IDLE_FRAMES {
        let u = f as f32 / IDLE_FRAMES as f32;
        let t = u * IDLE_SECONDS;
        let knight = grid_racer(t, GRID_X, 0.0);
        let props = grid_props(u, 0.0);
        raster.draw_parts(&[(&world, Part::Backdrop), (&props, Part::Track), (&knight, Part::Racer(KNIGHT))], &grid_view(u, 0.0));
        film.frames.push(finish(&raster, width, height, Edges::Window));
    }
    film
}

/// The start: the flag comes down, the plumes catch and the knight is gone
/// through the gate. `LAUNCH_FRAMES` over `LAUNCH_SECONDS`.
pub fn launch(width: u32, height: u32) -> Film {
    let world = grid_world();
    let mut raster = frame_raster(width, height);
    let mut film = Film { width, height, fps: LAUNCH_FRAMES as f32 / LAUNCH_SECONDS, frames: Vec::with_capacity(LAUNCH_FRAMES) };
    for f in 0..LAUNCH_FRAMES {
        let t = f as f32 / LAUNCH_FRAMES as f32 * LAUNCH_SECONDS;
        let drop = (t / 0.32).min(1.0);
        let go = (t - 0.28).max(0.0);
        let x = GRID_X + 0.5 * 64.0 * go * go;
        let burn = (go / 0.25).min(1.0);
        let knight = grid_racer(t, x, burn);
        let props = grid_props(t / IDLE_SECONDS, drop);
        let shake = if go > 0.0 { (1.0 - go / 0.5).max(0.0) } else { 0.0 };
        raster.draw_parts(&[(&world, Part::Backdrop), (&props, Part::Track), (&knight, Part::Racer(KNIGHT))], &grid_view(0.0, shake * 3.0));
        film.frames.push(finish(&raster, width, height, Edges::Window));
    }
    film
}

// ---------------------------------------------------------------- the goal

/// Where the far keep stands.
const KEEP: V3 = V3 { x: 78.0, y: 7.5, z: 0.0 };

/// The open country under the night, as the goal shot lights it: a cold
/// moon low ahead, the ground and stone flat-shaded in steps, glowing things
/// left unlit, and a warm haze that takes the distance.
pub fn vista_shade(t: &Tri, eye: V3) -> [u8; 3] {
    let (base, glow) = Course::Hell.paint(t.mat, false);
    let cen = (t.v[0] + t.v[1] + t.v[2]) / 3.0;
    let n = if t.normal.dot(eye - cen) < 0.0 { -t.normal } else { t.normal };
    let moon = v3(0.55, -0.35, 0.62).normalize();
    let c = if glow {
        match t.mat {
            LAVA => [150.0, 26.0, 12.0],
            LAVA_HOT => [214.0, 110.0, 44.0],
            _ => base,
        }
    } else {
        let lit = n.dot(moon).max(0.0);
        let k = 0.42 + 0.95 * (lit * 4.0).round() / 4.0;
        let base = match t.mat {
            mat::ROCK => [30.0, 29.0, 34.0],
            mat::STONE => [74.0, 74.0, 84.0],
            _ => base,
        };
        [base[0] * k * 0.96, base[1] * k, base[2] * k * 1.08]
    };
    let dist = (cen - eye).length();
    let haze = [24.0, 12.0, 14.0];
    let f = 1.0 - (-dist * if glow { 0.004 } else { 0.0105 }).exp();
    std::array::from_fn(|i| (c[i] + (haze[i] - c[i]) * f).clamp(0.0, 255.0) as u8)
}

/// The goal's sky: near black overhead, a low ember haze along the horizon.
fn goal_sky(k: f32) -> [u8; 3] {
    let near = (-((k - 0.47) / 0.16).powi(2)).exp();
    let top = [5.0, 5.0, 8.0];
    let haze = [38.0, 18.0, 18.0];
    std::array::from_fn(|i| (top[i] + (haze[i] - top[i]) * near) as u8)
}

/// The country between the crag and the keep: rolling dark ground, a lava
/// river winding out to the keep, and the keep with its banner up.
fn vista(t: f32) -> Mesh {
    let mut m = Mesh::new();
    let ground = |x: f32, y: f32| 0.35 * (x * 0.11 + y * 0.07).sin() + 0.25 * (y * 0.19 - x * 0.05).cos() - 0.6;
    let step = 4.0;
    let mut x = -16.0;
    while x < 230.0 {
        let mut y = -80.0;
        while y < 80.0 {
            let p = |dx: f32, dy: f32| v3(x + dx, y + dy, ground(x + dx, y + dy));
            m.push_quad(p(0.0, 0.0), p(step, 0.0), p(step, step), p(0.0, step), mat::ROCK);
            y += step;
        }
        x += step;
    }
    // The river: it leaves the crag's foot and winds out to the keep.
    let path = |x: f32| KEEP.y * (x / KEEP.x) + 7.0 * (x * 0.045).sin() * (1.0 - x / KEEP.x).max(0.0);
    let mut x = 8.0;
    while x < KEEP.x - 6.0 {
        let nx = x + 2.0;
        let w = 0.22 + 0.42 * (1.0 - x / KEEP.x);
        let hot = ((x * 0.37).sin() + (x * 0.11 + t * TAU).cos() * 0.4) > 0.75;
        let k = if hot { LAVA_HOT } else { LAVA };
        let (y0, y1) = (path(x), path(nx));
        let (z0, z1) = (ground(x, y0) + 0.06, ground(nx, y1) + 0.06);
        m.push_quad(v3(x, y0 - w, z0), v3(nx, y1 - w, z1), v3(nx, y1 + w, z1), v3(x, y0 + w, z0), k);
        x = nx;
    }
    // Far hills on the horizon behind the keep.
    for i in 0..14 {
        let y = -120.0 + i as f32 * 19.0;
        let h = 9.0 + 7.0 * ((i as f32 * 1.7).sin().abs());
        let x = 210.0 + 12.0 * (i as f32 * 2.3).cos();
        tri(&mut m, v3(x, y - 16.0, -0.6), v3(x, y + 16.0, -0.6), v3(x + 4.0, y + 2.0, h), mat::ROCK);
    }
    keep(&mut m, t);
    m
}

/// The objective: a black keep on the horizon, a lit window or two, and a
/// beacon burning on its tower: the far mark the knight calls.
fn keep(m: &mut Mesh, t: f32) {
    let k = KEEP;
    let at = |x: f32, y: f32, z: f32| k + v3(x, y, z) * 0.85;
    cuboid(m, at(-5.0, -8.0, -1.0), at(5.0, 8.0, 7.0), mat::STONE); // the curtain wall
    for i in 0..8 {
        let y = -7.6 + i as f32 * 2.1;
        cuboid(m, at(-5.2, y, 7.0), at(-4.0, y + 1.0, 8.0), mat::STONE);
    }
    cuboid(m, at(-3.0, -3.0, -1.0), at(3.0, 3.0, 14.0), mat::STONE); // the keep
    for (dx, dy) in [(-3.0, -3.0), (-3.0, 3.0), (3.0, -3.0), (3.0, 3.0)] {
        cuboid(m, at(dx - 0.7, dy - 0.7, 14.0), at(dx + 0.7, dy + 0.7, 15.4), mat::STONE);
    }
    cuboid(m, at(-1.3, -1.3, 14.0), at(1.3, 1.3, 19.0), mat::STONE); // the tower
    for (z, y) in [(4.5, -1.6), (9.5, 1.2), (16.2, 0.0)] {
        cuboid(m, at(-3.1, y - 0.35, z), at(-2.9, y + 0.35, z + 1.0), mat::WINDOW);
    }
    // The beacon in its iron basket on the tower top, burning.
    cuboid(m, at(-1.6, -1.6, 19.0), at(1.6, 1.6, 19.8), IRON);
    for (i, (dx, dy, tall)) in [(0.0, 0.0, 4.6), (-0.7, 0.6, 3.0), (0.6, -0.5, 3.4), (0.4, 0.7, 2.4)].into_iter().enumerate() {
        let flick = 1.0 + 0.22 * (t * TAU * 2.0 + i as f32 * 1.9).sin();
        let lean = 0.5 * (t * TAU + i as f32).sin();
        spike(m, at(dx, dy, 19.8), at(dx + lean, dy, 19.8 + tall * flick), 0.55 + 0.1 * tall, mat::FIRE);
    }
    spike(m, at(0.0, 0.0, 19.8), at(0.15, 0.0, 22.0), 0.7, LAVA_HOT);
}

/// The crag the knight stands on, and the knight on it calling his shot,
/// his cape lifting on the wind.
fn crag_and_knight(t: f32) -> (Mesh, Mesh) {
    let mut crag = Mesh::new();
    // A faceted outcrop: a flat top the knight stands on, its faces falling
    // away unevenly to the plain.
    let top = [v3(-2.2, -1.9, 1.0), v3(1.4, -1.6, 1.0), v3(2.0, 0.6, 1.0), v3(0.6, 2.0, 1.0), v3(-2.6, 1.4, 1.0)];
    let foot = [v3(-4.6, -4.2, -0.7), v3(3.6, -3.4, -0.7), v3(5.4, 1.2, -0.7), v3(1.2, 4.4, -0.7), v3(-5.2, 2.8, -0.7)];
    let mid = [v3(-3.2, -2.9, 0.2), v3(2.5, -2.4, 0.35), v3(3.4, 0.9, 0.1), v3(0.9, 3.0, 0.3), v3(-3.8, 2.0, 0.15)];
    for i in 0..5 {
        let j = (i + 1) % 5;
        crag.push_quad(top[i], top[j], mid[j], mid[i], mat::ROCK);
        crag.push_quad(mid[i], mid[j], foot[j], foot[i], mat::ROCK);
    }
    tri(&mut crag, top[0], top[1], top[2], mat::ROCK);
    tri(&mut crag, top[0], top[2], top[3], mat::ROCK);
    tri(&mut crag, top[0], top[3], top[4], mat::ROCK);
    let mut knight = Mesh::new();
    let r = &RACERS[KNIGHT];
    let u = 1.9;
    standing_knight(&mut knight, v3(0.0, 0.0, 1.0), u, r, Hold::Point(0.3));
    // A long cape off his shoulders, lifting and settling.
    let neck = v3(-0.22 * u, 0.0, 1.0 + (0.48 + 0.42) * u);
    let (n, rows) = (5usize, 4usize);
    let point = |i: usize, j: usize| {
        let (a, w) = (i as f32 / n as f32 - 0.5, j as f32 / rows as f32);
        let lift = (w * 3.0 - t * TAU).sin() * 0.12 * w + 0.28 * w * w;
        neck + v3(-lift * u - 0.05 * u, a * (0.3 + 0.1 * w) * u, -w * 0.74 * u)
    };
    for i in 0..n {
        for j in 0..rows {
            knight.push_quad(point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1), r.cloth);
        }
    }
    (crag, knight)
}

/// The goal camera: low behind the knight's sword arm, so he stands up
/// against the sky with the keep on the horizon past his blade.
fn goal_view() -> View3 {
    let pos = v3(-10.2, -6.2, 1.8);
    View3 { pos, heading_rad: 0.3, pitch: 0.045, fov_rad: 0.9 }
}

/// The knight on his crag pointing his sword at the far keep: a loop of
/// `GOAL_FRAMES` over `GOAL_SECONDS` (the banner and the cape move).
pub fn goal(width: u32, height: u32) -> Film {
    let mut raster = frame_raster(width, height);
    raster.sky = Some(goal_sky);
    let mut film = Film { width, height, fps: GOAL_FRAMES as f32 / GOAL_SECONDS, frames: Vec::with_capacity(GOAL_FRAMES) };
    for f in 0..GOAL_FRAMES {
        let u = f as f32 / GOAL_FRAMES as f32;
        let land = vista(u);
        let (crag, knight) = crag_and_knight(u);
        raster.draw_parts(&[(&land, Part::Vista), (&crag, Part::Vista), (&knight, Part::Racer(KNIGHT))], &goal_view());
        film.frames.push(finish(&raster, width, height, Edges::Window));
    }
    film
}

// ---------------------------------------------------------------- the widget

/// The knight's racer in side profile on a clear ground, for the loop
/// ticker embedded under the cockpit header: the same chariot and twin
/// engines as the grid shot, close enough to read at two rows tall.
/// `burn` (0 to 1) throws the engines' plumes behind it.
pub fn ticker_racer(width: u32, height: u32, burn: f32) -> Film {
    let mut raster = frame_raster(width, height);
    raster.sky = Some(|_| [0, 0, 0]);
    let knight = grid_racer(0.4, GRID_X, burn.clamp(0.0, 1.0));
    // The grid camera moved in and across, so the racer fills the strip.
    let look = v3(-4.2, 0.6, 1.35);
    let pos = v3(2.5, -13.0, 3.2);
    let d = look - pos;
    let view = View3 {
        pos,
        heading_rad: d.y.atan2(d.x),
        pitch: (d.z / (d.x * d.x + d.y * d.y).sqrt()).atan(),
        fov_rad: 0.82,
    };
    raster.draw_parts(&[(&knight, Part::Racer(KNIGHT))], &view);
    Film {
        width,
        height,
        fps: 0.0,
        frames: vec![finish(&raster, width, height, Edges::Cutout)],
    }
}

/// The knight's racer seen from above at `headings` turns (the first along
/// +x, then counter-clockwise on screen), each `side` pixels square on a
/// clear ground: the corner widget drives it round its little circuit.
pub fn racer_turns(side: u32, headings: usize) -> Film {
    let mut raster = frame_raster(side, side);
    raster.sky = Some(|_| [0, 0, 0]);
    let mut film = Film { width: side, height: side, fps: 0.0, frames: Vec::with_capacity(headings) };
    for h in 0..headings {
        let a = h as f32 / headings as f32 * TAU;
        let mut m = Mesh::new();
        racer(&mut m, KNIGHT, 0.4, V3::ZERO);
        let (s, c) = a.sin_cos();
        m.tris.retain(|t| !super::pool(t));
        for tr in &mut m.tris {
            for p in &mut tr.v {
                let (x, y) = (p.x + 2.0, p.y);
                *p = v3(x * c - y * s, x * s + y * c, p.z);
            }
            tr.normal = v3(tr.normal.x * c - tr.normal.y * s, tr.normal.x * s + tr.normal.y * c, tr.normal.z);
        }
        // From above and a little south, so the knight stands up off the deck.
        let view = View3 { pos: v3(0.0, -15.0, 21.0), heading_rad: PI / 2.0, pitch: -0.93, fov_rad: 0.6 };
        raster.draw_parts(&[(&m, Part::Racer(KNIGHT))], &view);
        film.frames.push(finish(&raster, side, side, Edges::Cutout));
    }
    film
}

// ---------------------------------------------------------------- finishing

/// How a frame meets the glass round it.
#[derive(Clone, Copy, PartialEq)]
enum Edges {
    /// A window into the scene: soft rounded corners, and the sides fading
    /// into the glass.
    Window,
    /// Only what was drawn: the ground is clear.
    Cutout,
}

fn frame_raster(width: u32, height: u32) -> Raster {
    Raster::new(Course::Hell, width as usize * SUPER, height as usize * SUPER)
}

/// The supersampled raster shrunk to `width` by `height`, straight RGBA,
/// its edges as `edges` says.
fn finish(raster: &Raster, width: u32, height: u32, edges: Edges) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![0u8; w * h * 4];
    let n = (SUPER * SUPER) as f32;
    let radius = (h as f32 * 0.09).clamp(4.0, 14.0);
    for y in 0..h {
        for x in 0..w {
            let (mut sum, mut cover) = ([0f32; 3], 0f32);
            for sy in 0..SUPER {
                for sx in 0..SUPER {
                    let (px, py) = (x * SUPER + sx, y * SUPER + sy);
                    let p = raster.img.get_pixel(px as u32, py as u32).0;
                    let hit = raster.covered(px, py);
                    if edges == Edges::Cutout && !hit {
                        continue;
                    }
                    sum.iter_mut().zip(p).for_each(|(s, c)| *s += f32::from(c));
                    cover += 1.0;
                }
            }
            let o = (y * w + x) * 4;
            if cover == 0.0 {
                continue;
            }
            for c in 0..3 {
                out[o + c] = (sum[c] / cover).round() as u8;
            }
            let alpha = match edges {
                Edges::Cutout => cover / n,
                Edges::Window => window_alpha(x as f32 + 0.5, y as f32 + 0.5, w as f32, h as f32, radius),
            };
            out[o + 3] = (alpha * 255.0).round() as u8;
        }
    }
    out
}

/// A window's coverage at a pixel: rounded corners, and the left and right
/// eighths fading out into the glass.
fn window_alpha(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let q = [(x - w / 2.0).abs() - (w / 2.0 - r), (y - h / 2.0).abs() - (h / 2.0 - r)];
    let d = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt() + q[0].max(q[1]).min(0.0) - r;
    let corner = (0.5 - d).clamp(0.0, 1.0);
    let fade = w * 0.08;
    let side = (x.min(w - x) / fade).clamp(0.0, 1.0);
    let side = side * side * (3.0 - 2.0 * side);
    corner * (0.25 + 0.75 * side)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_lays_frames_out_in_rows() {
        let film = Film { width: 2, height: 1, fps: 1.0, frames: (0..5u8).map(|i| vec![i; 8]).collect() };
        let (w, h, rgba) = film.sheet(2);
        assert_eq!((w, h), (4, 3));
        // frame 3 is the second of the second row
        assert_eq!(rgba[(4 + 2) * 4], 3);
        // the empty slot after the last frame stays clear
        assert_eq!(rgba[(8 + 2) * 4 + 3], 0);
    }

    #[test]
    fn the_films_are_whole_and_their_windows_fade_at_the_sides() {
        let film = idle(96, 24);
        assert_eq!(film.frames.len(), IDLE_FRAMES);
        assert!(film.frames.iter().all(|f| f.len() == 96 * 24 * 4));
        let alpha = |x: usize, y: usize| film.frames[0][(y * 96 + x) * 4 + 3];
        assert!(alpha(48, 12) == 255, "the middle is solid");
        assert!(alpha(1, 12) < 128, "the sides fade into the glass");
        // something was drawn: the frame is not all one colour
        let first = &film.frames[0][(12 * 96) * 4..(12 * 96 + 96) * 4];
        assert!(first.chunks(4).any(|p| p[..3] != first[..3]));
    }

    #[test]
    fn the_racer_turns_are_cut_out_of_a_clear_ground() {
        let film = racer_turns(32, 4);
        assert_eq!(film.frames.len(), 4);
        for frame in &film.frames {
            assert_eq!(frame[3], 0, "a corner is clear");
            assert!(frame.chunks(4).any(|p| p[3] == 255), "the racer is solid");
        }
    }

    #[test]
    fn the_flag_drops_and_the_knight_leaves_the_grid() {
        let film = launch(48, 16);
        assert_eq!(film.frames.len(), LAUNCH_FRAMES);
        assert_ne!(film.frames[0], film.frames[LAUNCH_FRAMES - 1]);
    }
}
