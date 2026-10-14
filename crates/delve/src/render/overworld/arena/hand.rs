//! The demon hand. A few seconds after a guardian falls and its loot has
//! dropped, the ground under the body cracks and glows; a great hand climbs
//! out of the crack, claws spread; it closes on the body and drags it down
//! into the depths while the ground shakes, and the crack shuts over the
//! place. The loot, lying round the body, stays.
//!
//! Everything is drawn from the claim's clock (`spectacle::Beat`), in ink
//! on black paper: a glowing, dithered patch of ground, a jagged crack, a
//! hole, then the hand as a thick-lined arm, palm and four jointed fingers
//! and a thumb that curl by one number, shaded from the glow below and
//! edged in black. The body is the guardian's own sprite, greyed and laid
//! on its side.

use super::super::ink::{Img, bayer, hash, vnoise};
use super::{UNIT, at, sprites, stature};
use crate::rules::spectacle::{Beat, Corpse};
use crate::rules::{EnemyKind, Run};

/// A line of discs: a thick stroke tapering from radius `r0` to `r1`.
fn thick(im: &mut Img, (x0, y0): (f32, f32), (x1, y1): (f32, f32), r0: f32, r1: f32, ch: char) {
    let steps = ((x1 - x0).hypot(y1 - y0) * 1.5).ceil().max(1.0) as i32;
    for i in 0..=steps {
        let k = i as f32 / steps as f32;
        let r = r0 + (r1 - r0) * k;
        im.ellipse(x0 + (x1 - x0) * k, y0 + (y1 - y0) * k, r, r, ch);
    }
}

/// The body's lightness, on the structure greys a notch darker than a
/// statue's: a thing that has burned out.
fn charred(im: &Img) -> Img {
    const RAMP: [char; 7] = ['k', 'X', 'g', 'j', 'G', 'J', 'h'];
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(x, y, RAMP[(l as usize * RAMP.len() / 256).min(RAMP.len() - 1)]);
            }
        }
    }
    out
}

/// The body, charred and laid on its side.
fn body(run: &Run, c: &Corpse) -> Img {
    let im = match c.boss.and_then(|i| stature::boss_figure(run, i)) {
        Some(im) => im,
        None if c.kind == EnemyKind::Slime => stature::monster(
            c.kind,
            sprites::enemy(c.kind, 0),
            c.radius / sprites::SLIME_REACH,
        ),
        None => stature::monster(c.kind, sprites::enemy(c.kind, 0), 1.0),
    };
    let mut lying = charred(&im);
    if lying.h > lying.w {
        lying = sprites::quarter_turn(&lying);
    }
    lying.outline_outside('k');
    lying
}

/// Where the hand rises, how far up, and how tightly it is closed, for a
/// beat: `(rise 0..1, curl 0..1, sink 0..1)`.
fn motion(beat: Beat) -> (f32, f32, f32) {
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    match beat {
        Beat::Lying | Beat::Cracking(_) | Beat::Gone => (0.0, 0.0, 0.0),
        Beat::Rising(t) => (ease(t), 0.0, 0.0),
        Beat::Grabbing(t) => (1.0, ease(t), 0.0),
        Beat::Pulling(t) => (1.0 - 0.25 * t, 1.0, ease(t)),
        Beat::Closing(_) => (0.0, 1.0, 1.0),
    }
}

/// How glowing the ground is (0..1) and how far open the hole is (0..1).
fn glow_and_hole(beat: Beat) -> (f32, f32) {
    match beat {
        Beat::Lying | Beat::Gone => (0.0, 0.0),
        Beat::Cracking(t) => (t, ((t - 0.55) / 0.45).clamp(0.0, 1.0)),
        Beat::Rising(_) | Beat::Grabbing(_) | Beat::Pulling(_) => (1.0, 1.0),
        Beat::Closing(t) => (1.0 - t, 1.0 - t),
    }
}

/// The hand, in local pixels, `s` the size of its palm. Returns the arm and
/// palm, and the fingers (which close over the body, so they draw after it).
/// `a` is where the hole's middle is in the picture.
fn hand(s: f32, reach: f32, rise: f32, curl: f32, tick: u32) -> (Img, Img, (i32, i32)) {
    let (w, h) = ((s * 2.7) as i32, (s * 2.7) as i32);
    let a = (w / 2, h - 2);
    let mut back = Img::new(w, h);
    let mut front = Img::new(w, h);
    let py = a.1 as f32 + 1.2 * s - rise * (1.2 * s + reach);
    let px = a.0 as f32 + (tick as f32 * 0.13).sin() * 0.6;
    // The forearm, wider as it goes down into the dark.
    for y in (py as i32)..h {
        let k = (y as f32 - py) / s;
        let half = 0.27 * s + 0.12 * s * k.min(2.0);
        back.rect((px - half) as i32, y, (2.0 * half) as i32, 1, 'B');
    }
    // The palm, in front of the body with the fingers.
    front.ellipse(px, py, 0.56 * s, 0.42 * s, 'B');
    front.ellipse(px, py + 0.14 * s, 0.5 * s, 0.38 * s, 'B');
    // Fingers: four, jointed three times, curling toward the middle.
    let lengths = [0.36 * s, 0.30 * s, 0.24 * s];
    let bends = [0.55, 1.35, 2.1];
    for i in 0..4 {
        let side = if i < 2 { -1.0 } else { 1.0 };
        let spread = (i as f32 - 1.5) * 0.34;
        let size = [0.82, 1.0, 1.0, 0.8][i];
        let mut at = (
            px + (i as f32 - 1.5) * 0.32 * s,
            py - 0.3 * s + (i as f32 - 1.5).abs() * 0.07 * s,
        );
        let mut tip_dir = (0.0, -1.0);
        for j in 0..3 {
            let angle = spread + side * curl * bends[j];
            let (dx, dy) = (angle.sin(), -angle.cos());
            let len = lengths[j] * size;
            let to = (at.0 + dx * len, at.1 + dy * len);
            let (r0, r1) = (0.15 * s - j as f32 * 0.014 * s, 0.125 * s - j as f32 * 0.02 * s);
            thick(&mut front, at, to, r0, r1, 'B');
            at = to;
            tip_dir = (dx, dy);
        }
        // The claw: a dark hook on the tip.
        let claw = (at.0 + tip_dir.0 * 0.2 * s, at.1 + tip_dir.1 * 0.2 * s);
        thick(&mut front, at, claw, 0.07 * s, 0.02 * s, 'J');
        front.put(claw.0.round() as i32, claw.1.round() as i32, 'h');
    }
    // The thumb, off the palm's side.
    let tb = (px - 0.5 * s, py + 0.1 * s);
    let (t1, t2) = (
        (tb.0 - 0.28 * s * (1.0 - curl * 0.8), tb.1 - 0.2 * s - curl * 0.1 * s),
        (
            tb.0 - 0.34 * s * (1.0 - curl * 1.4),
            tb.1 - 0.44 * s + curl * 0.3 * s,
        ),
    );
    thick(&mut front, tb, t1, 0.16 * s, 0.13 * s, 'B');
    thick(&mut front, t1, t2, 0.13 * s, 0.1 * s, 'B');
    thick(&mut front, t2, (t2.0 - 0.06 * s, t2.1 - 0.16 * s), 0.07 * s, 0.02 * s, 'J');
    // The whole hand's silhouette shades as one skin.
    let skin = |im: &Img| {
        let src = im.clone();
        let mut out = Img::new(im.w, im.h);
        for y in 0..im.h {
            for x in 0..im.w {
                if !src.solid(x, y) || src.get(x, y) == super::super::ink::ink('J') {
                    if let Some(c) = src.get(x, y) {
                        out.set(x, y, c);
                    }
                    continue;
                }
                let n = vnoise(x as f32 * 0.32, y as f32 * 0.32, 5);
                let near = (a.1 - y) as f32 / s;
                let ink = if !src.solid(x, y + 1) {
                    '7'
                } else if !src.solid(x, y - 1) {
                    '8'
                } else if !src.solid(x - 1, y) || !src.solid(x + 1, y) {
                    'B'
                } else if near < 0.55 && bayer(x, y) < (0.55 - near) * 1.6 {
                    '@'
                } else if n > 0.7 {
                    '7'
                } else if n < 0.3 {
                    '8'
                } else {
                    'p'
                };
                out.put(x, y, ink);
            }
        }
        out.outline_outside('k');
        out
    };
    let mut back = skin(&back);
    let front = skin(&front);
    // Veins of the heat it came from, in the arm.
    for v in 0..3 {
        let vx = px + (v as f32 - 1.0) * 0.17 * s;
        for y in (py + 0.3 * s) as i32..h {
            let wob = ((y as f32 * 0.35) + v as f32 * 2.0).sin() * 1.4;
            let on = (hash(v, y / 3, 7) + tick / 5) % 4 != 0;
            if on && back.solid((vx + wob) as i32, y) {
                back.put((vx + wob) as i32, y, '7');
            }
        }
    }
    (back, front, a)
}

/// A claim's ground: glow, crack, hole and embers. `(cx, cy)` is the hole's
/// middle in native pixels.
fn ground(cv: &mut Img, (cx, cy): (i32, i32), s: f32, beat: Beat, tick: u32, seed: i32) {
    let (glow, hole) = glow_and_hole(beat);
    if glow <= 0.0 {
        return;
    }
    // The glowing ground, dithered out to its edge.
    let (rx, ry) = (s * 1.25 * (0.4 + 0.6 * glow), s * 0.5 * (0.4 + 0.6 * glow));
    let pulse = 0.85 + 0.15 * (tick as f32 * 0.22).sin();
    for y in (cy as f32 - ry) as i32..=(cy as f32 + ry) as i32 {
        for x in (cx as f32 - rx) as i32..=(cx as f32 + rx) as i32 {
            let d = (((x - cx) as f32 / rx).powi(2) + ((y - cy) as f32 / ry).powi(2)).sqrt();
            if d > 1.0 {
                continue;
            }
            let k = (1.0 - d) * glow * pulse;
            if bayer(x, y) < k * 1.3 {
                cv.put(x, y, if k > 0.6 { '@' } else if k > 0.32 { '7' } else { '8' });
            }
        }
    }
    // The crack: seven jagged arms from the middle.
    let grow = match beat {
        Beat::Cracking(t) => t,
        _ => 1.0,
    };
    let cooling = matches!(beat, Beat::Closing(_));
    for k in 0..7 {
        let ang = k as f32 * std::f32::consts::TAU / 7.0 + 0.35 + (hash(k, seed, 3) % 50) as f32 / 100.0;
        let len = s * (0.6 + (hash(k, seed, 9) % 55) as f32 / 100.0) * grow;
        let (mut px, mut py) = (cx as f32, cy as f32);
        let segments = 7;
        for seg in 0..segments {
            let a = ang + ((hash(k * 17 + seg, seed, 11) % 100) as f32 / 100.0 - 0.5) * 1.1;
            let step = len / segments as f32;
            let (nx, ny) = (px + a.cos() * step, py + a.sin() * step * 0.55);
            if let Beat::Closing(t) = beat
                && t > 0.55
                && (hash(k, seg, 5) % 100) as f32 / 100.0 < (t - 0.55) / 0.45
            {
                break;
            }
            cv.line(px as i32, py as i32, nx as i32, ny as i32, 'k');
            cv.line(px as i32, py as i32 + 1, nx as i32, ny as i32 + 1, 'k');
            let lava = if cooling {
                'p'
            } else if seg < 3 {
                '@'
            } else {
                '7'
            };
            cv.line(px as i32, py as i32, nx as i32, ny as i32, lava);
            (px, py) = (nx, ny);
        }
    }
    // The hole.
    if hole > 0.0 {
        let (hx, hy) = (s * 0.62 * hole, s * 0.24 * hole);
        cv.ellipse(cx as f32, cy as f32, hx + 1.5, hy + 1.5, '7');
        cv.ellipse(cx as f32, cy as f32, hx, hy, 'k');
        cv.ellipse(cx as f32, cy as f32 + hy * 0.25, hx * 0.8, hy * 0.6, '8');
        cv.ellipse(cx as f32, cy as f32 + hy * 0.35, hx * 0.55, hy * 0.35, 'k');
    }
    // Embers climbing off it.
    if glow > 0.3 {
        for n in 0..14 {
            let h = hash(n, seed, 23);
            let life = 36 + h % 20;
            let t = (tick + h % 40) % life;
            let x = cx + (h % (s as u32 * 2 + 1)) as i32 - s as i32;
            let y = cy - (t as f32 * 0.9) as i32 + 2;
            let fade = t as f32 / life as f32;
            if fade < 0.8 {
                cv.put(x, y, if fade < 0.35 { '5' } else { '@' });
            } else {
                cv.put(x, y, '7');
            }
        }
    }
}

/// Draw `src` onto `cv` at `(x, y)`, showing only rows above `max_y`: what
/// has gone below the lip of the hole is gone.
fn stamp_above(cv: &mut Img, src: &Img, x: i32, y: i32, max_y: i32) {
    for sy in 0..src.h {
        if y + sy > max_y {
            break;
        }
        for sx in 0..src.w {
            if let Some(c) = src.get(sx, sy) {
                cv.set(x + sx, y + sy, c);
            }
        }
    }
}

/// What each claim has to draw this tick.
struct Claim {
    lying: Img,
    body: (i32, i32),
    hole: (i32, i32),
    s: f32,
    beat: Beat,
    seed: i32,
}

fn claims(run: &Run) -> Vec<Claim> {
    let mut out = Vec::new();
    for (n, c) in run.spectacle.corpses.iter().enumerate() {
        let beat = c.beat(run.tick);
        if c.room != run.at || matches!(beat, Beat::Gone) {
            continue;
        }
        let lying = body(run, c);
        let (bx, by) = at(c.x, c.y);
        let hole = (bx, by - lying.h / 4);
        // A hand as big as the room allows: it must stand inside the view.
        let room_above = (hole.1 - 4) as f32 - lying.h as f32 * 0.7;
        let s = (c.radius * UNIT * 2.4)
            .clamp(26.0, 58.0)
            .min(room_above / 1.5)
            .max(20.0);
        out.push(Claim {
            body: (bx - lying.w / 2, by - lying.h),
            hole,
            s,
            beat,
            seed: n as i32 * 7 + c.born as i32 % 11,
            lying,
        });
    }
    out
}

/// The cracking ground, glow and hole: under the loot lying on it.
pub(super) fn ground_layer(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for c in claims(run) {
        ground(cv, c.hole, c.s, c.beat, tick, c.seed);
    }
}

/// The body and the hand over it, over the loot but under the knights.
pub(super) fn over(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for c in claims(run) {
        let (rise, curl, sink) = motion(c.beat);
        if matches!(c.beat, Beat::Closing(_)) {
            // Gone below: only the shutting ground remains.
            continue;
        }
        let drop = (sink * (c.lying.h as f32 + c.s * 0.9)) as i32;
        let lip = c.hole.1 + 1;
        if rise > 0.0 || sink > 0.0 {
            // The palm sits at the body's top edge when fully up.
            let reach = c.lying.h as f32 * 0.7 + c.s * 0.2;
            let (back, front, a) = hand(c.s, reach, rise, curl, tick);
            let (ox, oy) = (c.hole.0 - a.0, c.hole.1 - a.1 + drop);
            stamp_above(cv, &back, ox, oy, lip);
            stamp_above(cv, &c.lying, c.body.0, c.body.1 + drop, lip);
            stamp_above(cv, &front, ox, oy, lip);
        } else {
            cv.stamp(&c.lying, c.body.0, c.body.1);
        }
    }
}
