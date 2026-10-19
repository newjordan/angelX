//! What the skills, the perils and the Boss Key look like, drawn by hand in
//! the arena's own inks: gas that swells and swirls in greens and greys,
//! flame that flickers, mines that blink toward arming and burst, a
//! palisade that goes up stake by stake and comes down in splinters, the
//! thrown shield turning in the air, a fire jet's hiss and roar, a vent's
//! bubbling, a seam of ember, a floor that cracks and falls, and the
//! key and the chained door it opens.
//!
//! Everything reads the run's own state (`Run::works`), so a friend's
//! mirror and a browser guest's picture show what the host's does. The
//! frame is pure: the same run paints the same pixels.

use super::super::ink::{Img, bayer, hash};
use super::{TILE, UNIT, at, doors, stand_at, stature};
use crate::rules::keyring::DOOR_REACH;
use crate::rules::perils::{self, Peril};
use crate::rules::skills::{self, Barricade, BoomKind, Mine};
use crate::rules::{Enemy, EnemyKind, Run, ledge::FOOT};
use std::f32::consts::{PI, TAU};

fn feet(run: &Run) -> f32 {
    if run.side_on() { FOOT } else { 0.0 }
}

/// A point on the ground, in native pixels.
fn ground(run: &Run, x: f32, y: f32) -> (i32, i32) {
    at(x, y + feet(run))
}

fn rows(rows: &[&str]) -> Img {
    Img::from_rows(rows)
}

/// A column of flame `h` pixels tall standing on `(x, base)`, flickering:
/// blood at its foot, amber through the middle, gold at the tip.
fn tongue(cv: &mut Img, x: i32, base: i32, h: i32, tick: u32, seed: i32) {
    for k in 0..h {
        let f = k as f32 / h.max(1) as f32;
        let sway = ((tick as f32 * 0.35 + seed as f32 * 1.7 + k as f32 * 0.4).sin() * f * 1.4)
            .round() as i32;
        let ink = if f < 0.22 {
            'o'
        } else if f < 0.5 {
            '@'
        } else if f < 0.82 {
            '6'
        } else if tick.wrapping_add(seed as u32).is_multiple_of(2) {
            'c'
        } else {
            'T'
        };
        cv.put(x + sway, base - k, ink);
        // The body of the flame is two wide near its foot.
        if f < 0.6 {
            cv.put(x + sway + 1, base - k, if f < 0.3 { 'P' } else { 'o' });
        }
    }
}

// ─── gas ─────────────────────────────────────────────────────────────────────

/// A cloud: cached rolling lobes inside the original radius, a crisp rim
/// marking bad air and cool low interiors that keep figures readable.
pub(super) fn gas_stamp(run: &Run, cloud: &skills::Cloud, modern: bool) -> (std::sync::Arc<Img>, i32, i32) {
    let (cx, cy) = at(cloud.x, cloud.y);
    // The RGB kit's existing core is 1.8 radii across. The ink kit's
    // floor projection keeps its existing 0.72/0.8 squash.
    let r = cloud.radius() * UNIT * if modern { 0.9 } else { 1.0 };
    let squash = if modern { 1.0 } else if run.side_on() { 0.8 } else { 0.72 };
    let (rx, ry) = (r as i32, (r * squash) as i32);
    let kind = match cloud.tint { 1 => super::fx::Stamp::Sulphur, 2 => super::fx::Stamp::GasWall, 3 => super::fx::Stamp::SulphurWall, _ => super::fx::Stamp::Gas };
    let im = super::fx::stamp(kind, rx, ry, ((run.tick / 4) % 6) as u8);
    (im, cx - rx, cy - ry)

}

fn gas(cv: &mut Img, run: &Run, cloud: &skills::Cloud) {
    let (im,x,y)=gas_stamp(run,cloud,false);
    cv.stamp(&im,x,y);
}

// ─── fire ────────────────────────────────────────────────────────────────────

/// Burning ground: a bed of embers, and tongues of flame that rise and
/// gutter as it burns down.
fn blaze(cv: &mut Img, run: &Run, b: &skills::Blaze) {
    let tick = run.tick as u32;
    let (cx, cy) = ground(run, b.x, b.y);
    let left = b.life.saturating_sub(b.age);
    let vigour = (left as f32 / 40.0).min(1.0);
    let rx = (b.radius * UNIT * (0.6 + 0.4 * vigour)) as i32;
    let ry = if run.side_on() { rx / 4 } else { rx / 2 };
    let bed = super::fx::stamp(super::fx::Stamp::Embers, rx, ry, 0);
    cv.stamp(&bed, cx - rx, cy - ry);
    let mut x = -rx + 1;
    while x < rx {
        let n = hash(cx + x, 5, tick / 2) % 100;
        let fall = 1.0 - (x as f32 / rx.max(1) as f32).powi(2);
        let dy = ((hash(cx + x, 9, 3) % 5) as i32 - 2) * ry / 3;
        let h = ((5.0 + 19.0 * n as f32 / 100.0) * fall * vigour).round() as i32;
        if h >= 2 {
            tongue(cv, cx + x, cy + dy, h, tick, x);
        }
        x += 2;
    }
    // Sparks climb out of it.
    for k in 0..4 {
        let t = (tick as i32 + k * 9) % 30;
        let sx = cx + (hash(k, tick as i32 / 30, 4) % (2 * rx.max(1) as u32 + 1)) as i32 - rx;
        cv.put(sx, cy - 6 - t / 2, if t < 15 { '6' } else { '@' });
    }
}

// ─── mines ───────────────────────────────────────────────────────────────────

fn mine_body() -> Img {
    rows(&[
        "....jJJj....",
        "..jJhhhhJj..",
        ".jJhGGGGhJj.",
        "jGhGJJJJGhGj",
        "jGGJhhhhJGGj",
        ".gjGGGGGGjg.",
        "..gggggggg..",
    ])
}

fn mine(cv: &mut Img, run: &Run, m: &Mine) {
    let (x, y) = ground(run, m.x, m.y);
    let body = mine_body().recolor(&[('h','U'),('J','V'),('G','u'),('g','x'),('j','S')]);
    cv.stamp(&body, x - body.w / 2, y - body.h + 1);
    let tick = run.tick;
    // Its light: quick amber while arming, a slow red pulse once armed, a
    // gold flicker when a neighbour's blast has lit it.
    let ink = if m.fuse > 0 {
        if tick.is_multiple_of(2) { '6' } else { '5' }
    } else if !m.armed() {
        if (tick / 4).is_multiple_of(2) {
            '4'
        } else {
            'a'
        }
    } else if (tick / 24).is_multiple_of(2) {
        '7'
    } else {
        '8'
    };
    for (dx, dy) in [(0, -4), (-1, -4), (0, -3), (-1, -3)] {
        cv.put(x + dx, y + dy, ink);
    }
    // Studs round the shoulder.
    for dx in [-4, 3] {
        cv.put(x + dx, y - 3, 'i');
    }
}

// ─── barricades ──────────────────────────────────────────────────────────────

/// A palisade: stakes along the line, a crossbeam lashed over them. A worn
/// one has stakes snapped to stumps; a falling one tips and sinks.
fn barricade(cv: &mut Img, run: &Run, w: &Barricade) {
    let (a, b) = (at(w.a.0, w.a.1), at(w.b.0, w.b.1));
    let fell = w
        .fall
        .map_or(0.0, |left| 1.0 - left as f32 / skills::WALL_CRUMBLE as f32);
    let wear = (1.0 - w.share()).clamp(0.0, 1.0);
    if run.side_on() {
        // A wall of planks standing on the ground, as tall as it is.
        let planks = 4;
        let (top, base) = (a.1.min(b.1), a.1.max(b.1));
        let height = base - top;
        for k in 0..planks {
            let x = a.0 - 6 + k * 4;
            let broken = (hash(k, 3, 8) % 100) as f32 / 100.0 < wear * 1.1;
            let h = if broken { height * 2 / 3 } else { height };
            let h = (h as f32 * (1.0 - fell)) as i32;
            let tilt = (fell * (k as f32 - 1.5) * 3.0) as i32;
            for dy in 0..h {
                let yy = base - dy;
                let tip = dy >= h - 3;
                for dx in 0..3 {
                    let ink = if tip && dx != 1 {
                        continue;
                    } else if dx == 0 {
                        'O'
                    } else if dx == 1 {
                        if dy % 5 == 0 { 'p' } else { 'o' }
                    } else {
                        'P'
                    };
                    cv.put(x + dx + tilt * dy / height.max(1), yy, ink);
                }
            }
        }
        if fell == 0.0 {
            for beam in [height / 4, height * 3 / 4] {
                for x in a.0 - 7..a.0 + 9 {
                    cv.put(x, base - beam, 'P');
                    cv.put(x, base - beam + 1, 'B');
                }
            }
        }
        return;
    }
    let (dx, dy) = ((b.0 - a.0) as f32, (b.1 - a.1) as f32);
    let length = dx.hypot(dy).max(1.0);
    let stakes = (length / 4.0).round() as i32 + 1;
    // A lashed beam low along the line, then the stakes on it, nearest last.
    if fell == 0.0 {
        for i in 0..=length as i32 {
            let t = i as f32 / length;
            let (x, y) = (a.0 as f32 + dx * t, a.1 as f32 + dy * t);
            cv.put(x.round() as i32, y.round() as i32 - 2, 'B');
            cv.put(x.round() as i32, y.round() as i32 - 1, 'p');
            cv.put(x.round() as i32, y.round() as i32, 'b');
        }
    }
    for k in 0..stakes {
        let t = k as f32 / (stakes - 1).max(1) as f32;
        let (sx, sy) = (a.0 as f32 + dx * t, a.1 as f32 + dy * t);
        let broken = (hash(k, 3, 8) % 100) as f32 / 100.0 < wear * 1.1;
        let mut h = if broken {
            4.0
        } else {
            9.0 + ((k * 7) % 3) as f32
        };
        h *= 1.0 - fell;
        let shove = fell * ((hash(k, 5, 2) % 7) as f32 - 3.0);
        let (x, y) = (
            sx.round() as i32 + shove as i32,
            sy.round() as i32 + (fell * 3.0) as i32,
        );
        for k2 in 0..h as i32 {
            let tip = k2 >= h as i32 - 2;
            let top = k2 == h as i32 - 1;
            cv.put(x - 2, y - k2, if tip { 'P' } else { 's' });
            cv.put(
                x - 1,
                y - k2,
                if top {
                    'T'
                } else if tip {
                    'O'
                } else {
                    'O'
                },
            );
            cv.put(
                x,
                y - k2,
                if top {
                    'T'
                } else if k2 % 4 == 3 {
                    'o'
                } else {
                    'O'
                },
            );
            cv.put(x + 1, y - k2, if top { 'O' } else { 'o' });
            cv.put(x + 2, y - k2, if tip { 'B' } else { 'P' });
        }
        if broken && fell == 0.0 {
            // A splinter beside the stump.
            cv.put(x + 3, y - 1, 'O');
            cv.put(x + 4, y, 'o');
        }
    }
}

// ─── blasts ──────────────────────────────────────────────────────────────────

fn boom(cv: &mut Img, run: &Run, b: &skills::Boom) {
    let p = 1.0 - b.left as f32 / b.of.max(1) as f32;
    let ground_level = matches!(b.kind, BoomKind::Mine | BoomKind::Dust | BoomKind::Burst);
    let (cx, cy) = if ground_level {
        ground(run, b.x, b.y)
    } else {
        at(b.x, b.y)
    };
    let tick = run.tick as i32;
    match b.kind {
        BoomKind::Mine => {
            // A ring of fire, a fireball inside it, smoke and sparks outward.
            let reach = b.reach * UNIT;
            let r = reach * (0.35 + 0.65 * p.sqrt());
            let n = (r * 5.0) as i32;
            for i in 0..n {
                let a = i as f32 / n as f32 * TAU;
                let (x, y) = (cx + (a.cos() * r) as i32, cy + (a.sin() * r * 0.7) as i32);
                cv.put(
                    x,
                    y,
                    if p < 0.4 {
                        'c'
                    } else if p < 0.7 {
                        '@'
                    } else {
                        'o'
                    },
                );
                cv.put(x, y - 1, if p < 0.6 { 'o' } else { 'S' });
            }
            // The fireball: solid and bright at first, burning down to embers.
            let core = (reach * 0.55 * (1.0 - p * 0.7)) as i32;
            let fire = super::fx::stamp(super::fx::Stamp::Fireball, core, core, (p * 5.0) as u8);
            cv.stamp(&fire, cx - core, cy - core - core / 3);
            for k in 0..10 {
                let a = k as f32 * 0.63 + hash(k, 2, 6) as f32 * 0.01;
                let d = r * (0.7 + 0.5 * p);
                cv.put(
                    cx + (a.cos() * d) as i32,
                    cy + (a.sin() * d * 0.7) as i32 - (p * 8.0) as i32,
                    if p < 0.6 { '4' } else { 'J' },
                );
            }
            // Smoke lifts off the crater.
            for k in 0..6 {
                let h = (p * 16.0) as i32 + k * 2;
                cv.put(cx - 5 + k * 2, cy - h, if p < 0.5 { 'G' } else { 'J' });
            }
        }
        BoomKind::Flash => {
            // Gas lighting all through: a swelling sheet of gold and green.
            let reach = b.reach * UNIT;
            let r = reach * (0.5 + 0.5 * p);
            let (rx, ry) = (r as i32, (r * 0.75) as i32);
            let flash = super::fx::stamp(super::fx::Stamp::Flash, rx, ry, (p * 5.0) as u8);
            cv.stamp(&flash, cx - rx, cy - ry);

        }
        BoomKind::Dust => {
            // A cloud of grit that rises and thins.
            let reach = b.reach * UNIT;
            for k in 0..22 {
                let a = k as f32 * 0.7 + hash(k, 1, 4) as f32 * 0.01;
                let d = reach * (0.2 + 0.8 * p) * (0.5 + (hash(k, 7, 5) % 50) as f32 / 100.0);
                let (x, y) = (
                    cx + (a.cos() * d) as i32,
                    cy + (a.sin() * d * 0.5) as i32 - (p * 10.0) as i32,
                );
                if bayer(x, y) < 1.0 - p * 0.9 {
                    cv.put(
                        x,
                        y,
                        if k % 3 == 0 {
                            'h'
                        } else if k % 3 == 1 {
                            'J'
                        } else {
                            'P'
                        },
                    );
                }
            }
        }
        BoomKind::Burst => {
            // A flash of rays from the ground.
            let reach = b.reach * UNIT;
            for k in 0..12 {
                let a = k as f32 * TAU / 12.0;
                let len = reach * (0.3 + 0.7 * p) * if k % 2 == 0 { 1.0 } else { 0.6 };
                for s in 0..(len as i32) {
                    cv.put(
                        cx + (a.cos() * s as f32) as i32,
                        cy + (a.sin() * s as f32 * 0.6) as i32,
                        if s == 0 && p < 0.2 { '9' } else if s as f32 > len * 0.7 { 'c' } else if p < 0.5 { '@' } else { 'o' },
                    );
                }
            }
            let _ = tick;
        }
        BoomKind::Clang => {
            // A four-pointed starburst, gold and silver.
            let len = 3 + (4.0 * (1.0 - (p - 0.4).abs())) as i32;
            for s in 0..=len {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    cv.put(cx + dx * s, cy + dy * s, if s == 0 { '9' } else if s < 2 { 'c' } else { 'U' });
                }
                if s < len / 2 + 1 {
                    for (dx, dy) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
                        cv.put(cx + dx * s, cy + dy * s, '4');
                    }
                }
            }
        }
    }
}

// ─── the thrown shield ───────────────────────────────────────────────────────

/// The knight's own sun-star shield, face on, as a disc: a gold rim, the
/// house colour on the field, and the star in gold.
fn shield_disc() -> Img {
    rows(&[
        "...44444...",
        "..4777774..",
        ".477757774.",
        "47775557774",
        "47755555774",
        "47557575574",
        "47755555774",
        "47775557774",
        ".477757774.",
        "..4777774..",
        "...44444...",
    ])
}

/// The disc turned about its upright axis, `turn` radians on: narrower as
/// it goes edge-on, the rim showing alone at the edge.
fn shield_turned(colours: &[(char, char)], turn: f32) -> Img {
    let disc = shield_disc().recolor(colours);
    let k = turn.cos().abs();
    let w = ((disc.w as f32 * k).round() as i32).max(2);
    let mut out = Img::new(disc.w, disc.h);
    for y in 0..disc.h {
        for x in 0..w {
            let sx = ((x as f32 + 0.5) / w as f32 * disc.w as f32) as i32;
            let c = disc.get(sx.min(disc.w - 1), y);
            if let Some(c) = c {
                let ox = (disc.w - w) / 2 + x;
                // Edge-on it is only the rim.
                if w <= 3 {
                    out.put(ox, y, '4');
                } else {
                    out.set(ox, y, c);
                }
            }
        }
    }
    out
}

fn flight(cv: &mut Img, run: &Run, f: &skills::Flight) {
    let (x, y) = at(f.x, f.y);
    let hero = run.players.get(&f.owner);
    let colours: &[(char, char)] = hero.map_or(&[], |h| {
        if h.colours().is_empty() {
            super::knights::seat_colours(f.owner)
        } else {
            h.colours()
        }
    });
    // A comet's tail of the rim's gold behind it.
    let speed = f.vx.hypot(f.vy).max(0.01);
    for k in 1..6 {
        let back = k as f32 * 2.2;
        cv.put(
            x - (f.vx / speed * back) as i32,
            y - (f.vy / speed * back) as i32,
            if k < 3 { '4' } else { 'a' },
        );
    }
    let turn = f.age as f32 * 0.9;
    let im = shield_turned(colours, turn);
    cv.stamp(&im, x - im.w / 2, y - im.h / 2);
    // A glint where the rim catches the light.
    if (f.age / 2).is_multiple_of(3) {
        cv.put(x + (turn.sin() * 4.0) as i32, y - 4, '6');
    }
}

// ─── the rooms' perils ───────────────────────────────────────────────────────

fn grate(lit: f32, tick: u32) -> Img {
    // The slit smoulders even at rest, so the plate is known for what it is.
    let glow = if lit > 0.66 {
        if tick.is_multiple_of(2) { '6' } else { '@' }
    } else if lit > 0.33 {
        if tick.is_multiple_of(3) { '@' } else { '7' }
    } else if lit > 0.0 {
        '7'
    } else {
        '8'
    };
    let mut im = rows(&[
        "..jJhhhhhhJj..",
        ".jGgXXXXXXgGj.",
        "jGXkkkkkkkkXGj",
        "jXkkkkkkkkkkXj",
        "jGXkkkkkkkkXGj",
        ".jGgXXXXXXgGj.",
        "..jjGGGGGGjj..",
    ]);
    for x in 3..11 {
        im.put(x, 3, glow);
        if lit > 0.33 {
            im.put(x, 2, glow);
            im.put(x, 4, if lit > 0.66 { '@' } else { '8' });
        }
    }
    im
}

fn peril_floor(cv: &mut Img, run: &Run, p: &Peril) {
    let tick = run.tick;
    let t32 = tick as u32;
    let (px, py) = p.at();
    let (cx, cy) = at(px, py);
    let warn = p.warning(tick);
    let strike = p.striking(tick);
    match *p {
        Peril::Jet { .. } => {
            let lit = warn.unwrap_or(if strike.is_some() { 1.0 } else { 0.0 });
            let im = grate(lit, t32 / 2);
            cv.stamp(&im, cx - im.w / 2, cy - im.h / 2);
            if lit > 0.0 {
                // The floor round it takes the glow.
                for k in 0..16 {
                    let ang = k as f32 * 0.4;
                    let d = 8.0 + 3.0 * lit;
                    if hash(k, t32 as i32 / 3, 4) % 3 != 0 {
                        cv.put(
                            cx + (ang.cos() * d) as i32,
                            cy + (ang.sin() * d * 0.5) as i32,
                            if lit > 0.66 { '7' } else { '8' },
                        );
                    }
                }
            }
        }
        Peril::Seep { .. } => {
            // A crack in a damp, green-stained patch of floor.
            let wet = warn.unwrap_or(0.0);
            for k in 0..22 {
                let a = k as f32 * 0.7;
                let d = 3.0 + (hash(k, 4, 9) % 8) as f32;
                cv.put(
                    cx + (a.cos() * d) as i32,
                    cy + (a.sin() * d * 0.6) as i32,
                    if k % 3 == 0 {
                        'E'
                    } else if k % 2 == 0 {
                        'F'
                    } else {
                        'D'
                    },
                );
            }
            for x in -4..=4 {
                cv.put(cx + x, cy, 'k');
                cv.put(cx + x / 2, cy + 1, 'X');
                cv.put(
                    cx + x,
                    cy - 1,
                    if wet > 0.0 && x % 2 == 0 { 'l' } else { 'D' },
                );
            }
            if strike.is_some() {
                for x in -3..=3 {
                    cv.put(cx + x, cy, 'y');
                }
            }
        }
        Peril::Crack { across, .. } => {
            let half = (perils::CRACK_LONG * UNIT) as i32;
            let lit = warn.unwrap_or(if strike.is_some() { 1.0 } else { 0.0 });
            let pulse = if warn.is_some() && (tick / 6).is_multiple_of(2) {
                1.0
            } else {
                0.5
            };
            for i in -half..=half {
                let wob = ((i as f32 * 0.5).sin() * 1.6) as i32 + (hash(i, 3, 12) % 2) as i32;
                let (x, y) = if across {
                    (cx + i, cy + wob)
                } else {
                    (cx + wob, cy + i)
                };
                let (sx, sy) = if across { (x, y + 1) } else { (x + 1, y) };
                cv.put(x, y, 'k');
                cv.put(sx, sy, 'k');
                // Ember seen through the seam: dull at rest, pulsing as it warns.
                let glow = (lit * pulse * 100.0) as u32 + 18;
                if hash(i, 7, t32 / 4) % 100 < glow {
                    let ink = if lit > 0.7 {
                        '@'
                    } else if lit > 0.3 {
                        '7'
                    } else {
                        '8'
                    };
                    cv.put(x, y, ink);
                    cv.put(sx, sy, if lit > 0.3 { '7' } else { '8' });
                }
            }
        }
        Peril::Crumble { .. } => {
            let half = (perils::CRUMBLE_HALF * UNIT) as i32;
            let beat = p.beat(tick);
            let (x0, y0) = (cx - half, cy - half);
            if warn.is_none()
                && !(perils::CRUMBLE_FALL..perils::CRUMBLE_RUBBLE).contains(&beat)
                && beat < perils::CRUMBLE_RUBBLE
            {
                // Loose paving: a few dark seams, the stones a shade apart.
                for i in (0..2 * half).step_by(3) {
                    cv.put(x0 + i, y0, 'X');
                    cv.put(x0 + i + 1, y0 + 2 * half - 1, 'X');
                    cv.put(x0, y0 + i + 1, 'X');
                    cv.put(x0 + 2 * half - 1, y0 + i, 'X');
                }
                cv.line(cx, y0, cx, y0 + 3, 'X');
                cv.line(x0, cy, x0 + 4, cy + 1, 'X');
            }
            if let Some(w) = warn {
                // Hairlines that spread, and grit shaken loose.
                let reach = (w * 2.0 * half as f32) as i32 + 4;
                for k in 0..6 {
                    let a = k as f32 * 1.05 + 0.4;
                    for s in 0..reach.min(half) {
                        let wob = ((s as f32 * 0.8 + k as f32).sin() * 1.5) as i32;
                        let (hx, hy) = (
                            cx + (a.cos() * s as f32) as i32 + wob,
                            cy + (a.sin() * s as f32) as i32,
                        );
                        cv.put(hx, hy, 'k');
                        cv.put(hx + 1, hy, 'k');
                    }
                }
                for k in 0..4 {
                    let gx = x0 + (hash(k, t32 as i32 / 4, 3) % (2 * half as u32)) as i32;
                    let gy = y0
                        + (hash(k, t32 as i32 / 4, 4) % (2 * half as u32)) as i32
                        + (t32 as i32 % 4);
                    cv.put(gx, gy, 'J');
                }
                if w > 0.75 && (tick / 2).is_multiple_of(2) {
                    // The shudder before it goes.
                    for x in -half..half {
                        cv.put(cx + x, y0 - 1, 'X');
                    }
                }
            } else if beat >= perils::CRUMBLE_FALL && beat < perils::CRUMBLE_RUBBLE {
                // The hole: black, with a ragged lip, chunks still falling.
                let g = (beat - perils::CRUMBLE_FALL) as i32;
                for y in -half..half {
                    for x in -half..half {
                        let edge = x == -half || x == half - 1 || y == -half || y == half - 1;
                        if edge {
                            if hash(x, y, 14) % 3 != 0 {
                                cv.put(cx + x, cy + y, 'X');
                            }
                        } else {
                            cv.put(cx + x, cy + y, 'k');
                        }
                    }
                }
                for k in 0..5 {
                    let gx = cx - half + 3 + (hash(k, 5, 6) % (2 * half as u32 - 6)) as i32;
                    let gy = cy - half + (g * 2 + k * 5) % (2 * half);
                    cv.put(gx, gy, 'h');
                    cv.put(gx + 1, gy, 'G');
                }
            } else if beat >= perils::CRUMBLE_RUBBLE {
                // Rubble where it fell.
                for k in 0..16 {
                    let rx = cx - half + 2 + (hash(k, 6, 8) % (2 * half as u32 - 4)) as i32;
                    let ry = cy - half + 2 + (hash(k, 7, 8) % (2 * half as u32 - 4)) as i32;
                    cv.put(rx, ry, if k % 2 == 0 { 'G' } else { 'J' });
                    cv.put(rx + 1, ry, 'j');
                }
            }
        }
    }
}

/// What rises over the figures: a jet's column, a seam's flames, a vent's
/// bubbling.
fn peril_air(cv: &mut Img, run: &Run, p: &Peril) {
    let t32 = run.tick as u32;
    let (px, py) = p.at();
    let (cx, cy) = at(px, py);
    match *p {
        Peril::Jet { .. } => {
            if let Some(w) = p.warning(run.tick) {
                // Hissing: sparks leap off the grate, more and higher.
                let n = 2 + (w * 8.0) as i32;
                for k in 0..n {
                    let up = (hash(k, t32 as i32 / 2, 5) % (4 + (w * 10.0) as u32)) as i32;
                    let sx = cx - 3 + (hash(k, t32 as i32 / 2, 6) % 7) as i32;
                    cv.put(sx, cy - 2 - up, if up > 5 { '6' } else { '4' });
                }
            }
            if let Some(s) = p.striking(run.tick) {
                // The roar: a column that climbs, holds, and gutters.
                let swell = (s * PI).sin().sqrt();
                let height = (40.0 * swell) as i32;
                for dx in -6..=6i32 {
                    let fall = 1.0 - (dx as f32 / 7.0).powi(2);
                    let h = (height as f32
                        * fall
                        * (0.7 + 0.3 * (hash(dx, 1, t32) % 100) as f32 / 100.0))
                        as i32;
                    if h > 1 {
                        tongue(cv, cx + dx, cy, h, t32, dx);
                    }
                }
            }
        }
        Peril::Seep { .. } => {
            if let Some(w) = p.warning(run.tick) {
                // Bubbles break at the crack; thin green wisps climb.
                for k in 0..(2 + (w * 4.0) as i32) {
                    let phase = (t32 as i32 * 2 + k * 11) % 24;
                    let sx = cx - 3
                        + (hash(k, 2, 7) % 7) as i32
                        + ((phase as f32 * 0.4).sin() * 2.0) as i32;
                    cv.put(sx, cy - phase / 2, if phase < 12 { 'y' } else { 'A' });
                    if phase % 8 == 0 {
                        cv.put(sx + 1, cy - phase / 2 - 1, 'i');
                    }
                }
            }
        }
        Peril::Crack { across, .. } => {
            if let Some(s) = p.striking(run.tick) {
                let half = (perils::CRACK_LONG * UNIT) as i32;
                let swell = (s * PI).sin().sqrt();
                let mut i = -half;
                while i <= half {
                    let h =
                        ((6.0 + 14.0 * (hash(i, 2, t32 / 2) % 100) as f32 / 100.0) * swell) as i32;
                    let (x, y) = if across { (cx + i, cy) } else { (cx, cy + i) };
                    if h > 1 {
                        tongue(cv, x, y, h, t32, i);
                    }
                    i += 2;
                }
            } else if p.warning(run.tick).is_some() {
                // Embers lifting off the seam.
                for k in 0..3 {
                    let up = (t32 as i32 + k * 7) % 14;
                    let along = (hash(k, t32 as i32 / 14, 8) % 18) as i32 - 9;
                    let (x, y) = if across {
                        (cx + along, cy)
                    } else {
                        (cx, cy + along)
                    };
                    cv.put(x, y - up, if up < 7 { '@' } else { '7' });
                }
            }
        }
        Peril::Crumble { .. } => {}
    }
}

// ─── poisoned and burning ────────────────────────────────────────────────────

fn banes(cv: &mut Img, run: &Run) {
    let t32 = run.tick as u32;
    for bane in &run.works.banes {
        let Some(enemy) = run.enemies.iter().find(|e| e.id == bane.foe && e.hp > 0) else {
            continue;
        };
        let (x, y, h) = foe_box(run, enemy);
        match bane.kind {
            skills::Bane::Poison => {
                // Green bubbles rising off it, now and then a grey one.
                for k in 0..3 {
                    let phase = (t32 as i32 + k * 8 + enemy.id as i32 * 3) % 22;
                    let bx = x - 3 + k * 3 + ((phase as f32 * 0.5).sin() * 1.5) as i32;
                    let by = y - h / 2 - phase / 2;
                    cv.put(bx, by, if phase < 11 { 'y' } else { 'C' });
                    if phase < 4 {
                        cv.put(bx + 1, by, 'A');
                    }
                }
            }
            skills::Bane::Burn => {
                // Small flames licking up its sides.
                for k in 0..3 {
                    let fx = x - h / 4 + k * (h / 4).max(2);
                    tongue(
                        cv,
                        fx,
                        y - h / 3,
                        4 + (hash(k, t32 as i32 / 2, 2) % 4) as i32,
                        t32,
                        k,
                    );
                }
            }
        }
    }
}

/// Where a foe stands and how tall it is, in native pixels: its feet and
/// height.
fn foe_box(run: &Run, e: &Enemy) -> (i32, i32, i32) {
    let (x, y) = if run.side_on() {
        at(e.x, e.y + if e.boss.is_some() { e.radius() } else { FOOT })
    } else {
        at(e.x, e.y)
    };
    let library = e.boss.and_then(|i| run.bosses.get(i as usize))
        .and_then(|b| crate::rules::bosses::Art::for_id(&b.id))
        .and_then(|art| super::boss_art::library_figure(run,e,art,super::art_v2::enabled() && super::art_v2::supported(run)));
    let h = match library.map(|im| (*im).clone()).or_else(|| e.boss.and_then(|i| stature::boss_figure(run, i))) {
        Some(im) => im.h,
        None => ((e.radius() * 2.0 * UNIT) as i32).max(stature::FOE_MIN),
    };
    let _ = EnemyKind::Bat;
    (x, y, h)
}

// ─── the Boss Key and its door ───────────────────────────────────────────────

pub(super) fn key_sprite() -> Img {
    rows(&[
        ".444.........",
        "4@66@4.......",
        "4@..@4444444.",
        "4@66@4.4.4.44",
        ".444..a.a.a..",
    ])
}

fn key_lying(cv: &mut Img, run: &Run, x: f32, y: f32) {
    let (px, py) = at(x, y + feet(run));
    let bob = ((run.tick as f32 * 0.14).sin() * 1.5).round() as i32;
    let im = key_sprite();
    cv.stamp(&im, px - im.w / 2, py - 8 + bob);
    // A slow ring of glints round it.
    for k in 0..4 {
        let a = run.tick as f32 * 0.05 + k as f32 * PI / 2.0;
        cv.put(
            px + (a.cos() * 9.0) as i32,
            py - 5 + (a.sin() * 5.0) as i32,
            if k % 2 == 0 { '6' } else { '4' },
        );
    }
}

fn key_carried(cv: &mut Img, run: &Run) {
    let Some(id) = run.key_bearer() else {
        return;
    };
    let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0) else {
        return;
    };
    let (x, y) = at(hero.x, hero.y + feet(run));
    let im = key_sprite();
    let bob = ((run.tick as f32 * 0.12).sin()).round() as i32;
    cv.stamp(&im, x - im.w / 2, y - 26 + bob);
}

/// The boss door in this room's wall, shut and chained.
fn locked_door(cv: &mut Img, run: &Run) {
    let Some(side) = run.shut_door_side() else {
        return;
    };
    let (col, row) = doors(run.room())[side];
    let wide = side % 2 == 0;
    let (x0, y0) = (col * TILE, row * TILE);
    let (w, h) = if wide {
        (2 * TILE, TILE)
    } else {
        (TILE, 2 * TILE)
    };
    // Iron-banded planks across the doorway.
    for y in 0..h {
        for x in 0..w {
            let plank = if wide { x % 5 == 0 } else { y % 5 == 0 };
            let band = if wide {
                y == 3 || y == h - 4
            } else {
                x == 3 || x == w - 4
            };
            let ink = if band {
                'J'
            } else if plank {
                'X'
            } else if hash(x, y, 3) % 9 == 0 {
                'g'
            } else {
                'K'
            };
            cv.put(x0 + x, y0 + y, ink);
        }
    }
    for (rx, ry) in [(3, 3), (w - 4, 3), (3, h - 4), (w - 4, h - 4)] {
        cv.put(x0 + rx, y0 + ry, 'i');
    }
    // Chains from corner to corner.
    for i in 0..w.min(h) * 2 {
        let t = i as f32 / (w.min(h) * 2) as f32;
        let (cx, cy) = ((t * (w - 1) as f32) as i32, (t * (h - 1) as f32) as i32);
        for (px, py) in [(cx, cy), (w - 1 - cx, cy)] {
            if i % 3 != 2 {
                cv.put(x0 + px, y0 + py, if i % 3 == 0 { 'h' } else { 'G' });
            }
        }
    }
    // The padlock, gold: the shackle lifts when the key is brought near.
    let near = run
        .key_bearer()
        .and_then(|id| run.players.get(&id))
        .is_some_and(|hero| {
            let ((dx, dy), _) = crate::rules::hazards::doorway(run.room(), side);
            (hero.x - dx).hypot(hero.y - dy) <= DOOR_REACH
        });
    let (mx, my) = (x0 + w / 2, y0 + h / 2);
    let lift = if near {
        3 + ((run.tick / 3) % 2) as i32
    } else {
        0
    };
    for (dx, dy) in [(-2, -6), (-3, -5), (-3, -4), (2, -6), (3, -5), (3, -4)] {
        cv.put(mx + dx, my + dy - lift, 'h');
    }
    for dx in -2..=2 {
        cv.put(mx + dx, my - 7 - lift, 'h');
    }
    let lock = rows(&[
        "o444444o", "4@6666@4", "4@6kk6@4", "4@66k6@4", "o@6666@o", ".oooooo.",
    ]);
    cv.stamp(&lock, mx - 4, my - 3);
    if near && (run.tick / 4).is_multiple_of(2) {
        cv.put(mx + 6, my - 9, '6');
        cv.put(mx - 7, my - 6, '6');
    }
}

// ─── layers ──────────────────────────────────────────────────────────────────

/// Under the figures: what lies on the ground.
pub(super) fn under_with(cv: &mut Img, run: &Run, modern: bool) {
    for p in &run.works.perils {
        peril_floor(cv, run, p);
    }
    locked_door(cv, run);
    for b in &run.works.blazes {
        blaze(cv, run, b);
    }
    for m in &run.works.mines {
        mine(cv, run, m);
    }
    for w in &run.works.barricades {
        barricade(cv, run, w);
    }
    // RGB bodies are drawn by combat::under before the actors. Only the
    // crisp danger rims belong in its protected cue overlay.
    if !modern { for c in &run.works.clouds { gas(cv, run, c); } }
    // Keep every wall segment's danger core readable through its neighbours.
    for c in run.works.clouds.iter().filter(|c| c.wall()) {
        let (x,y)=at(c.x,c.y);
        let r=c.radius()*UNIT*if modern {0.9}else{1.0};
        let squash=if modern {1.0}else if run.side_on(){0.8}else{0.72};
        let (rx,ry)=(r as i32,(r*squash) as i32);
        let kind=if c.tint==3 {super::fx::Stamp::SulphurRim}else{super::fx::Stamp::GasRim};
        let rim=super::fx::stamp(kind,rx,ry,0);
        cv.stamp(&rim,x-rx,y-ry);
    }
    if let Some((x, y)) = run.key_on_floor() {
        key_lying(cv, run, x, y);
    }
}

/// Over the figures: flame columns, blasts, shields in the air, ailments,
/// the key a knight carries.
pub(super) fn over(cv: &mut Img, run: &Run) {
    for p in &run.works.perils {
        peril_air(cv, run, p);
    }
    // A sparse rising fringe makes gas read as a cloud above the floor,
    // without painting an opaque fog over the knights' bodies.
    for cloud in &run.works.clouds {
        let (x, y) = at(cloud.x, cloud.y);
        let r = cloud.radius() * UNIT;
        for i in 0..if cloud.wall() { 1 } else { 3 } {
            let t = (run.tick as f32 * 0.13 + i as f32 * 2.1).sin();
            let wx = x + (t * r * 0.6) as i32;
            let wy = y - (r * 0.7) as i32 - ((run.tick + i * 7) % 13) as i32;
            let ink = if cloud.tint == 1 { 'Y' } else { 'y' };
            cv.line(wx - 2, wy + 1, wx + 1, wy, ink);
            cv.put(wx + 2, wy - 1, 'A');
        }
    }
    banes(cv, run);
    for f in &run.works.flights {
        flight(cv, run, f);
    }
    for b in &run.works.booms {
        boom(cv, run, b);
    }
    key_carried(cv, run);
}

/// A knight who crouches: the sprite is shorter by a few rows, squatting.
pub(super) fn crouched(im: &Img) -> Img {
    let mut out = Img::new(im.w, im.h);
    let mut ty = im.h - 1;
    for sy in (0..im.h).rev() {
        // Drop every fourth row from the top half: head closer to shoulders.
        if sy % 4 == 1 && sy < im.h - 4 {
            continue;
        }
        for x in 0..im.w {
            if let Some(c) = im.get(x, sy) {
                out.set(x, ty, c);
            }
        }
        ty -= 1;
    }
    out
}

/// A body that stands on `(x, y)` (arena units): placed like the figures.
#[allow(dead_code)]
pub(super) fn place(cv: &mut Img, im: &Img, x: f32, y: f32) {
    stand_at(cv, im, x, y, 0);
}
