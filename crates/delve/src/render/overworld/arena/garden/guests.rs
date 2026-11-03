//! The garden's guests as the garden sees them (`nooks::gnome::WAVES`):
//! the leeches come as fat striped caterpillars that inch along, the
//! biters as earthworms stood up on their tails and wriggling, the
//! spiderlings as shiny beetles clicking their mandibles, and the
//! Silkmother as the moth queen, a crown on her fuzz. Only here: the
//! bestiary keeps its own pictures everywhere else.
//!
//! Each is drawn a pose at a time and kept (`foe_material::pose`), facing
//! the cellar door while it makes for it; what each leaves when it falls
//! is drawn with the kit (`deaths`), in its own colours.

use super::super::super::ink::{Img, bayer, hash};
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ease, ramps};
use super::super::{UNIT, at, foe_material};
use super::{Party, g};
use crate::rules::{Enemy, EnemyKind, Run, TILE_UNITS};
use std::f32::consts::{PI, TAU};

/// The kinds the garden draws as its own.
pub(super) fn is_guest(kind: EnemyKind) -> bool {
    matches!(kind, EnemyKind::Leech | EnemyKind::Biter | EnemyKind::Spiderling | EnemyKind::Silkmother)
}

/// The mark a garden pose is kept under, apart from the bestiary's own
/// poses of the same kind.
const KEPT: u32 = 0x6a7d;

fn kept(kind: EnemyKind, state: [u32; 3], paint: impl FnOnce() -> Img) -> Img {
    foe_material::pose(128 + kind as u8, [state[0], state[1], state[2], KEPT], paint)
}

/// Guest `e` as it stands this tick.
pub(super) fn figure(run: &Run, garden: Option<&g::Garden>, e: &Enemy, tick: u32) -> Img {
    // Each guest keeps its own time, so a wave does not march in step.
    let t = tick.wrapping_add(e.id.wrapping_mul(7));
    let im = match e.kind {
        EnemyKind::Leech => {
            let pose = e.stage.min(2);
            let frame = if pose == 0 { (t / 3) % 8 } else { 0 };
            kept(e.kind, [frame, u32::from(pose), 0], || caterpillar(frame, pose))
        }
        EnemyKind::Biter => {
            let pose = e.stage.min(2);
            let frame = (t / 3) % 8;
            kept(e.kind, [frame, u32::from(pose), 0], || earthworm(frame, pose))
        }
        EnemyKind::Spiderling => {
            let legs = (t / 3) % 2;
            // Click-click, and a pause.
            let jaws = u32::from(matches!((t / 2) % 10, 0 | 2));
            let shell = e.id % SHELLS.len() as u32;
            kept(e.kind, [legs, jaws, shell], || beetle(legs, jaws, shell))
        }
        // The queen faces the room: she has no side to turn.
        _ => return kept(e.kind, [(t / 4) % 4, 0, 0], || moth((t / 4) % 4)),
    };
    if facing(run, garden, e) < 0.0 { im.flip_h() } else { im }
}

/// The way a guest faces: the way it lunges, the cellar door while it
/// makes for it, or else the nearest knight.
fn facing(run: &Run, garden: Option<&g::Garden>, e: &Enemy) -> f32 {
    if e.stage >= 1 && e.dir.0 != 0.0 {
        return e.dir.0;
    }
    let lured = garden.is_some_and(|gd| gd.party == Party::Holding && gd.guests.contains(&e.id));
    let way = if lured {
        g::DOOR.0 * TILE_UNITS - e.x
    } else {
        run.players
            .values()
            .filter(|h| h.hp > 0)
            .map(|h| h.x - e.x)
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))
            .unwrap_or(1.0)
    };
    // Square under it: keep to the way it is walking.
    if way.abs() < 0.6 { if e.dir.0 < 0.0 { -1.0 } else { 1.0 } } else { way }
}

/// Fill the ellipse round `c` with radii `r`, each pixel's ink chosen by
/// where it lies across it (`nx`, `ny`, each -1 to 1).
fn disc(cv: &mut Img, (cx, cy): (f32, f32), (rx, ry): (f32, f32), ink: impl Fn(f32, f32) -> char) {
    for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
        for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
            let nx = (x as f32 + 0.5 - cx) / rx;
            let ny = (y as f32 + 0.5 - cy) / ry;
            if nx * nx + ny * ny <= 1.0 {
                cv.put(x, y, ink(nx, ny));
            }
        }
    }
}

/// As `disc`, turned by `angle` (radians, clockwise on the screen).
fn blob(cv: &mut Img, (cx, cy): (f32, f32), (rx, ry): (f32, f32), angle: f32, ink: impl Fn(f32, f32) -> char) {
    let (s, c) = angle.sin_cos();
    let reach = rx.max(ry);
    for y in (cy - reach).floor() as i32..=(cy + reach).ceil() as i32 {
        for x in (cx - reach).floor() as i32..=(cx + reach).ceil() as i32 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let (u, v) = (dx * c + dy * s, -dx * s + dy * c);
            let (nx, ny) = (u / rx, v / ry);
            if nx * nx + ny * ny <= 1.0 {
                cv.put(x, y, ink(nx, ny));
            }
        }
    }
}

/// `im` from row `top` down: the rows above it are only air.
fn from_row(im: Img, top: i32) -> Img {
    Img::from_view(&im, 0, top, im.w, im.h - top)
}

/// A fat striped caterpillar side on, facing right: `frame` (0 to 7)
/// rolls a hump from its tail to its head as it inches along; `pose` 1
/// rears it up with its mouth open, 2 stretches it out in its lunge.
fn caterpillar(frame: u32, pose: u8) -> Img {
    const W: i32 = 26;
    const H: i32 = 21;
    let mut cv = Img::new(W, H);
    let ground = (H - 1) as f32;
    let r = 2.9f32;
    let phase = frame as f32 / 8.0 * TAU;
    let gap = if pose == 2 { 3.0 } else { 2.55 };
    let mut seg = [(0.0f32, 0.0f32); 7];
    for (i, s) in seg.iter_mut().enumerate() {
        let lift = match pose {
            1 => [0.0, 0.0, 0.4, 1.6, 3.6, 6.0, 8.6][i],
            2 => [0.0, 0.0, 0.0, 0.0, 0.3, 0.8, 1.5][i],
            _ => (phase - i as f32 * 0.85).sin().max(0.0).powf(1.5) * 2.4,
        };
        // Reared, the raised end leans back over the rest.
        let lean = if pose == 1 { [0.0, 0.0, 0.0, -0.3, -0.8, -1.4, -2.0][i] } else { 0.0 };
        // Humped, the segments either side draw in under it.
        *s = (2.8 + i as f32 * gap + lean, ground - 1.0 - r - lift);
    }
    // The dark rim all round first, so it shows only at the edge.
    for (i, &(x, y)) in seg.iter().enumerate() {
        let (rx, ry) = if i == 6 { (3.3, 3.2) } else { (r + 0.3, r) };
        cv.ellipse(x, y, rx + 1.0, ry + 1.0, 'F');
    }
    for (i, &(x, y)) in seg[..6].iter().enumerate() {
        let spot = i % 2 == 0;
        disc(&mut cv, (x, y), (r + 0.3, r), |nx, ny| {
            if nx < -0.42 && nx > -0.78 && ny < 0.75 {
                'k'
            } else if ny < -0.5 && nx < 0.35 {
                'Y'
            } else if ny > 0.55 {
                'A'
            } else if spot && (nx - 0.15).abs() < 0.22 && (ny + 0.1).abs() < 0.24 {
                'T'
            } else {
                'y'
            }
        });
        // Its little legs, where it is down on the ground.
        if y > ground - 1.0 - r - 0.6 {
            cv.put(x.round() as i32 - 1, ground as i32, 'm');
            cv.put(x.round() as i32 + 1, ground as i32, 'm');
        }
    }
    // The horn on its tail.
    let (tx, ty) = seg[0];
    cv.put((tx - 1.0).round() as i32, (ty - r).round() as i32, 'O');
    cv.put((tx - 2.0).round() as i32, (ty - r - 1.0).round() as i32, 't');
    // The head: a round dark face, one big eye, a grin.
    let (hx, hy) = seg[6];
    disc(&mut cv, (hx, hy), (3.3, 3.2), |nx, ny| if ny < -0.45 && nx < 0.2 { 'L' } else if ny > 0.5 { 'E' } else { 'N' });
    let (ex, ey) = (hx.round() as i32, (hy - 1.0).round() as i32);
    cv.rect(ex, ey - 1, 2, 2, 'H');
    cv.put(ex + 1, ey, 'k');
    let (mx, my) = (ex + 2, (hy + 1.0).round() as i32);
    if pose == 0 {
        cv.put(mx, my, 'k');
        cv.put(mx - 1, my + 1, 'k');
    } else {
        cv.rect(mx - 1, my - 1, 2, 2, '7');
        cv.put(mx, my + 1, '7');
    }
    cv.line(ex - 1, ey - 2, ex - 2, ey - 4, 'K');
    cv.put(ex - 2, ey - 5, 'T');
    cv.line(ex + 1, ey - 2, ex + 2, ey - 4, 'K');
    cv.put(ex + 3, ey - 5, 'T');
    from_row(cv, if pose == 1 { 0 } else { 7 })
}

/// An earthworm stood up on its tail, facing right: `frame` (0 to 7)
/// sways it; `pose` 1 has it coiled down to spring with its eyes lit, 2
/// flung out in its leap with its mouth wide.
fn earthworm(frame: u32, pose: u8) -> Img {
    const W: i32 = 19;
    const H: i32 = 18;
    const N: usize = 20;
    let mut cv = Img::new(W, H);
    let ground = (H - 1) as f32;
    let phase = frame as f32 / 8.0 * TAU;
    let cx = 9.0f32;
    let mut pts = [(0.0f32, 0.0f32, 0.0f32); N + 1];
    for (j, p) in pts.iter_mut().enumerate() {
        let s = j as f32 / N as f32;
        let tail = (cx - 5.0 * (0.3 - s) / 0.3, ground - 1.6);
        let (x, y) = match pose {
            1 => {
                let shiver = if frame.is_multiple_of(2) { 0.5 } else { -0.5 };
                if s < 0.3 {
                    tail
                } else {
                    let u = (s - 0.3) / 0.7;
                    (cx + shiver + 2.4 * (u * PI * 2.0).sin() * (1.0 - u * 0.3), ground - 1.6 - u * 6.5)
                }
            }
            2 => (cx - 7.0 + s * 14.0, ground - 2.0 - (s * PI).sin() * 3.5 - s * 3.0),
            _ => {
                if s < 0.3 {
                    tail
                } else {
                    let u = (s - 0.3) / 0.7;
                    (cx + 2.4 * (u * PI * 1.25 - phase).sin() * u, ground - 1.6 - u * 11.0)
                }
            }
        };
        let rad = if s < 0.12 { 1.0 + s / 0.12 * 0.6 } else if s > 0.9 { 2.1 } else { 1.65 };
        *p = (x, y, rad);
    }
    for &(x, y, rad) in &pts {
        cv.ellipse(x, y, rad + 1.0, rad + 1.0, 'b');
    }
    for (j, &(x, y, rad)) in pts.iter().enumerate() {
        let s = j as f32 / N as f32;
        let saddle = (0.6..0.72).contains(&s);
        let ring = j % 3 == 0 && s < 0.9;
        disc(&mut cv, (x, y), (rad, rad), |nx, ny| {
            if saddle {
                if ny < -0.2 || nx < -0.3 { 'O' } else { 'o' }
            } else if nx < -0.35 && ny < 0.3 {
                'o'
            } else if nx > 0.5 || ny > 0.6 {
                'p'
            } else if ring {
                'r'
            } else {
                'R'
            }
        });
    }
    // The face: two eyes and a grin, or wide open in the leap.
    let (hx, hy, _) = pts[N];
    let (ex, ey) = (hx.round() as i32, hy.round() as i32 - 1);
    let (eye, pupil) = if pose == 1 { ('@', '7') } else { ('H', 'k') };
    cv.put(ex - 1, ey, eye);
    cv.put(ex + 1, ey, eye);
    cv.put(ex, ey - 1, 'o');
    cv.put(ex - 1, ey + 1, pupil);
    cv.put(ex + 1, ey + 1, pupil);
    if pose == 2 {
        cv.rect(ex, ey + 2, 2, 2, '7');
    } else {
        cv.put(ex - 1, ey + 2, 'p');
        cv.put(ex, ey + 3, 'p');
        cv.put(ex + 1, ey + 2, 'p');
    }
    from_row(cv, if pose == 0 { 0 } else { 4 })
}

/// A beetle's shell: its dark, its body, its lit side and its shine.
const SHELLS: [[char; 4]; 4] = [['s', 'q', 'z', 'W'], ['F', 'l', 'y', 'Y'], ['b', 'R', 'o', 'T'], ['K', 'g', 'J', 'H']];

/// A shiny beetle side on, facing right: a domed shell with a gleam
/// along it, its legs in two sets of three (`legs` which set is down),
/// and mandibles that click (`jaws` 1 open).
fn beetle(legs: u32, jaws: u32, shell: u32) -> Img {
    const W: i32 = 19;
    const H: i32 = 12;
    let mut cv = Img::new(W, H);
    let [dark, body, lit, shine] = SHELLS[shell as usize % SHELLS.len()];
    let ground = H - 1;
    // Six legs, three a side; the near three show, stepping in turn.
    for (k, lx) in [4, 8, 11].into_iter().enumerate() {
        let step = if (k as u32 + legs).is_multiple_of(2) { 1 } else { -1 };
        cv.line(lx, 8, lx + step, ground - 1, 'K');
        cv.put(lx + step * 2, ground, 'K');
    }
    // The shell: a dome, flat beneath, its gleam up its shoulder.
    cv.ellipse(7.5, 7.5, 6.8, 5.6, 'k');
    disc(&mut cv, (7.5, 7.5), (5.8, 4.8), |nx, ny| {
        if ny > 0.25 {
            'K'
        } else if (-0.75..-0.2).contains(&nx) && (-0.85..-0.45).contains(&ny) {
            shine
        } else if ny < -0.1 && nx < 0.3 {
            lit
        } else if nx > 0.6 || ny > 0.0 {
            dark
        } else {
            body
        }
    });
    // The seam where its wing-cases meet.
    cv.line(9, 3, 8, 8, dark);
    cv.put(6, 4, shine);
    // The plate behind its head, and the head.
    disc(&mut cv, (13.0, 6.8), (2.2, 2.4), |_, ny| if ny < -0.4 { lit } else { dark });
    disc(&mut cv, (15.2, 7.6), (1.9, 1.7), |_, ny| if ny < -0.5 { 'g' } else { 'K' });
    cv.put(15, 7, 'V');
    // Feelers, and the mandibles, open and shut.
    cv.line(14, 5, 16, 2, 'K');
    cv.put(17, 2, 'g');
    if jaws == 1 {
        cv.line(16, 7, 18, 5, 'B');
        cv.line(16, 9, 18, 10, 'B');
        cv.put(18, 5, 'o');
        cv.put(18, 10, 'o');
    } else {
        cv.line(16, 7, 18, 8, 'B');
        cv.line(16, 9, 18, 8, 'B');
        cv.put(18, 8, 'o');
    }
    cv
}

/// The moth queen, wings out, facing the room: `flap` (0 to 3) beats her
/// wings up, level, down and level again. A pale green moth with long
/// tails on her hind wings, eyespots, a fuzzy body and a little crown.
fn moth(flap: u32) -> Img {
    const W: i32 = 29;
    const H: i32 = 25;
    let mut cv = Img::new(W, H);
    let f = [0.0f32, 1.0, 2.0, 1.0][flap as usize % 4];
    let up = 2.0 - f;
    // Her body rises a touch as her wings come down.
    let lift = f * 0.5;
    let cx = 14.0f32;
    let wing = |nx: f32, ny: f32| {
        let spot = ((nx - 0.2) * (nx - 0.2) + (ny * 1.2) * (ny * 1.2)).sqrt();
        if spot < 0.12 {
            'z'
        } else if spot < 0.2 {
            'k'
        } else if spot < 0.3 {
            't'
        } else if ny < -0.62 {
            'P'
        } else if nx * nx + ny * ny > 0.72 {
            'y'
        } else if ny < -0.2 {
            'Y'
        } else {
            'C'
        }
    };
    // One side, then the other as its mirror.
    let mut side = Img::new(W, H);
    let tip = (cx + 7.0, 21.5 - lift + up * 0.2);
    side.line((cx + 4.0) as i32, (15.0 - lift) as i32, tip.0 as i32, tip.1 as i32, 'E');
    side.line((cx + 5.0) as i32, (15.0 - lift) as i32, tip.0 as i32 + 1, tip.1 as i32, 'C');
    side.put(tip.0 as i32 + 1, tip.1 as i32 + 1, 'Y');
    let hind = (cx + 5.0, 14.5 - lift - up * 0.4);
    blob(&mut side, hind, (5.0, 3.6), 0.5 - f * 0.12, |_, _| 'E');
    blob(&mut side, hind, (4.2, 2.8), 0.5 - f * 0.12, |nx, ny| if nx * nx + ny * ny > 0.6 { 'y' } else { 'C' });
    let fore = (cx + 6.5 - up * 0.6, 10.0 - lift - up * 1.5);
    let angle = -0.55 + f * 0.36;
    blob(&mut side, fore, (8.0, 4.4 - up * 0.3), angle, |_, _| 'E');
    blob(&mut side, fore, (7.0, 3.6 - up * 0.3), angle, wing);
    for y in 0..H {
        for x in 0..W {
            if let Some(c) = side.get(x, y) {
                cv.set(x, y, c);
                cv.set((2.0 * cx) as i32 - x, y, c);
            }
        }
    }
    // The fuzzy body down her middle.
    let b = |y: f32| y - lift;
    cv.ellipse(cx + 0.5, b(16.5), 2.2, 4.2, 'r');
    disc(&mut cv, (cx + 0.5, b(16.5)), (1.6, 3.6), |_, ny| if ((ny + 1.0) * 3.0) as i32 % 2 == 0 { 't' } else { 'o' });
    cv.ellipse(cx + 0.5, b(10.5), 3.0, 3.6, 'r');
    disc(&mut cv, (cx + 0.5, b(10.5)), (2.5, 3.1), |nx, ny| if ny < -0.3 && nx < 0.2 { 'H' } else if nx > 0.5 { 'O' } else { 'T' });
    disc(&mut cv, (cx + 0.5, b(6.5)), (2.2, 1.9), |_, ny| if ny < -0.3 { 'H' } else { 'T' });
    // Big dark eyes, and her feathered feelers.
    let (hx, hy) = (cx as i32, b(6.5) as i32);
    cv.put(hx - 1, hy, 'k');
    cv.put(hx + 2, hy, 'k');
    cv.put(hx - 1, hy - 1, 'h');
    cv.put(hx + 2, hy - 1, 'h');
    for (dir, foot) in [(-1, hx), (1, hx + 1)] {
        for k in 0..4 {
            let (x, y) = (foot + dir * (1 + k), hy - 2 - k);
            cv.put(x, y, 'O');
            cv.put(x + dir, y + 1, 'o');
        }
    }
    // The crown.
    let top = hy - 3;
    for x in hx - 1..=hx + 2 {
        cv.put(x, top, 'O');
    }
    for x in [hx - 1, hx + 2] {
        cv.put(x, top - 1, 't');
    }
    cv.put(hx, top - 1, 'T');
    cv.put(hx + 1, top - 1, 'T');
    cv.put(hx, top - 2, 'z');
    cv.put(hx + 1, top - 2, 'z');
    cv
}

/// A caterpillar's goo, a worm's own browns, a moth's pale greens.
const GOO: Ramp = Ramp(['Y', 'y', 'C', 'A', 'l', 'E']);
const WORM: Ramp = Ramp(['O', 'o', 'R', 'r', 'p', 'b']);
const WING: Ramp = Ramp(['w', 'Y', 'C', 'y', 'l', 'E']);

/// Each guest that falls in the garden: the caterpillar bursts in green
/// goo and a leaf or two, the worm in a spray of earth with its two ends
/// wriggling off, the beetle's shell in bright shards with a plink, the
/// queen in a cloud of wing dust that glitters as it settles.
pub(super) fn deaths(pen: &mut Pen, run: &Run) {
    use crate::rules::feel::DEATH_TICKS;
    for d in &run.feel.deaths {
        let Some(kind) = d.kind.filter(|&k| is_guest(k)) else { continue };
        let age = f32::from(DEATH_TICKS.saturating_sub(d.ttl));
        let at = (d.x * UNIT, d.y * UNIT - 3.0);
        let s = hash((d.x * 8.0) as i32, (d.y * 8.0) as i32, KEPT);
        let calm = pen.calm;
        let few = |n: u32| if calm { n / 2 } else { n };
        match kind {
            EnemyKind::Leech => {
                if age < 6.0 {
                    pen.glow(at, 7.0, 0.9, ramps::POISON, 1.0 - age / 6.0);
                }
                pen.ring((at.0, at.1 + 2.0), 2.0 + ease::out(age / 12.0) * 10.0, 1.2, 0.6, GOO, age / 13.0);
                let goo = Spray { seed: s, n: few(12), at, angle: -PI / 2.0, spread: 3.6, speed: (1.0, 2.8), life: (9.0, 14.0), drag: 0.08, gravity: 0.2, squash: 0.8, stagger: 0.0 };
                goo.each(age, |m| pfx::mote(pen, &m, Shape::Droplet, GOO, 2, run.tick));
                let leaves = Spray { seed: s ^ 0x1eaf, n: 2, at, angle: -PI / 2.0, spread: 2.0, speed: (0.8, 1.2), life: (13.0, 14.0), drag: 0.15, gravity: 0.03, squash: 1.0, stagger: 0.0 };
                leaves.each(age, |m| pfx::mote(pen, &m, Shape::Petal, ramps::NATURE, 2, run.tick));
            }
            EnemyKind::Biter => {
                let dirt = Spray { seed: s, n: few(14), at: (at.0, at.1 + 3.0), angle: -PI / 2.0, spread: 2.8, speed: (1.2, 3.0), life: (8.0, 13.0), drag: 0.1, gravity: 0.25, squash: 0.9, stagger: 0.0 };
                dirt.each(age, |m| pfx::mote(pen, &m, Shape::Dot, ramps::EARTH, 1, run.tick));
                // Its two ends, off two ways.
                let ends = Spray { seed: s ^ 0x2e, n: 2, at: (at.0, at.1 + 3.0), angle: 0.0, spread: PI * 2.0, speed: (0.7, 0.9), life: (13.0, 14.0), drag: 0.0, gravity: 0.0, squash: 0.3, stagger: 0.0 };
                ends.each(age, |m| pfx::mote(pen, &m, Shape::Worm, WORM, 2, run.tick));
                if age < 8.0 {
                    pen.ring((at.0, at.1 + 3.0), 2.0 + age * 1.2, 1.0, 0.5, ramps::EARTH, age / 8.0);
                }
            }
            EnemyKind::Spiderling => {
                let [_, body, lit, shine] = SHELLS[(s % SHELLS.len() as u32) as usize];
                let shell = Ramp([shine, lit, lit, body, body, 'K']);
                let shards = Spray { seed: s, n: few(10), at, angle: -PI / 2.0, spread: 3.2, speed: (1.4, 3.2), life: (9.0, 14.0), drag: 0.1, gravity: 0.22, squash: 0.85, stagger: 0.0 };
                shards.each(age, |m| pfx::mote(pen, &m, Shape::Shard, shell, 2, run.tick));
                pfx::rays(pen, at, 6, 7.0, 0.8, (s % 7) as f32, ramps::HOLY, age / 8.0);
                pen.twinkle((at.0 + 3.0, at.1 - 6.0), 2.5, age / 12.0, ramps::HOLY);
            }
            _ => {
                if age < 7.0 {
                    pen.glow(at, 12.0, 1.0, WING, 1.0 - age / 7.0);
                }
                pen.shock((at.0, at.1 + 4.0), 4.0 + ease::out(age / 13.0) * 18.0, 1.6, 0.55, WING, ease::inward(age / 13.0));
                let dust = Spray { seed: s, n: few(10), at, angle: -PI / 2.0, spread: TAU, speed: (0.6, 1.8), life: (11.0, 14.0), drag: 0.12, gravity: -0.03, squash: 0.8, stagger: 0.0 };
                dust.each(age, |m| pfx::mote(pen, &m, Shape::Smoke, ramps::BONE, 1, run.tick));
                let glitter = Spray { seed: s ^ 0x91, n: few(16), at, angle: -PI / 2.0, spread: TAU, speed: (1.2, 3.4), life: (10.0, 14.0), drag: 0.1, gravity: 0.05, squash: 0.9, stagger: 0.0 };
                glitter.each(age, |m| pfx::mote(pen, &m, Shape::Star, if m.i % 2 == 0 { WING } else { ramps::HOLY }, 1, run.tick));
            }
        }
    }
}

/// The queen sheds the dust off her wings as she flies: a few pale motes
/// drifting down under her, always.
pub(super) fn wing_dust(pen: &mut Pen, run: &Run) {
    let tick = run.tick as u32;
    for e in run.enemies.iter().filter(|e| e.kind == EnemyKind::Silkmother && e.hp > 0 && e.boss.is_none()) {
        let (x, y) = at(e.x, e.y);
        // A pale light about her.
        pen.glow((x as f32, (y - 11) as f32), 15.0, 0.8, WING.cooler(1), 0.22 + 0.06 * (tick as f32 * 0.11).sin());
        for k in 0..5u32 {
            let life = 34;
            let t = tick + k * 7 + e.id * 5;
            let a = (t % life) as f32;
            let s = hash(e.id as i32, (t / life) as i32, k);
            let p = (x as f32 + ((s % 25) as f32 - 12.0), (y - 16) as f32 + (s >> 8) as f32 % 6.0 + a * 0.45);
            if bayer(p.0 as i32, p.1 as i32) < a / life as f32 {
                continue;
            }
            pen.light(p.0 as i32, p.1 as i32, if k % 2 == 0 { 'Y' } else { 'C' });
        }
    }
}

/// The back end of guest `kind` going in at the hatch whose middle is
/// `(hx, hy)`, `age` ticks after it got there: only what is still above
/// the hatch's lip is drawn.
pub(super) fn going_in(cv: &mut Img, kind: EnemyKind, (hx, hy): (i32, i32), age: f32) {
    let lip = hy + 1;
    let sink = age * 0.7;
    let mut put = |x: i32, y: i32, ch: char| {
        if y <= lip {
            cv.put(x, y, ch);
        }
    };
    let wig = (age * 1.3).sin() * 1.5;
    match kind {
        EnemyKind::Leech => {
            for (k, (dx, dy)) in [(3.0, -1.0), (6.0, -2.5)].into_iter().enumerate() {
                let (cx, cy) = (hx as f32 + dx + wig * 0.5, hy as f32 + dy + sink);
                for y in -3..=3 {
                    for x in -3..=3 {
                        if (x * x + y * y) as f32 <= 7.0 {
                            let ch = if x == -1 { 'k' } else if y < -1 { 'Y' } else { 'y' };
                            put(cx as i32 + x, cy as i32 + y, ch);
                        }
                    }
                }
                if k == 1 {
                    put(cx as i32 + 2, cy as i32 - 3, 'O');
                    put(cx as i32 + 3, cy as i32 - 4, 't');
                }
            }
        }
        EnemyKind::Biter => {
            for j in 0..9 {
                let s = j as f32 / 8.0;
                let (x, y) = (hx as f32 + 1.0 + (s * 3.0 + age * 0.9).sin() * 2.0 * s, hy as f32 - s * 8.0 + sink);
                for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    put(x as i32 + ox, y as i32 + oy, if j % 3 == 0 { 'r' } else { 'R' });
                }
            }
        }
        EnemyKind::Spiderling => {
            let (cx, cy) = (hx as f32, hy as f32 - 1.0 + sink * 0.8);
            for y in -3..=1 {
                for x in -4..=4 {
                    if (x * x) as f32 / 16.0 + (y * y) as f32 / 9.0 <= 1.0 {
                        put(cx as i32 + x, cy as i32 + y, if y < -1 { 'z' } else { 'q' });
                    }
                }
            }
            // Its legs kicking.
            let kick = if (age as i32 / 2) % 2 == 0 { 1 } else { -1 };
            for lx in [-3, 0, 3] {
                put(cx as i32 + lx, cy as i32 - 4, 'K');
                put(cx as i32 + lx + kick, cy as i32 - 5, 'K');
            }
        }
        _ => {
            for dir in [-1, 1] {
                for k in 0..5 {
                    let (x, y) = (hx + dir * (2 + k), hy - 1 - k / 2 + sink as i32 - (wig.abs() as i32) * (k / 3));
                    put(x, y, if k > 3 { 'y' } else { 'C' });
                }
            }
        }
    }
}
