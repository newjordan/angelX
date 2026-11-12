//! Latch Duel's effects, all pure functions of an age in ticks on the realm's inks: the closing ring
//! of a tell, the spark and rings of a counter, Clasp coming apart, the dust of a folded guard, a
//! fist of white for a struck body, the zoom of the last blow, and the forge's embers. Nothing is
//! kept between frames; the pen's budget caps what any one effect may paint.
use super::super::super::ink::{BLACK, Img, bayer, hash};
use super::super::juice;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use crate::motion::{self, ease};
use crate::rules::juice::Weight;
use std::f32::consts::{FRAC_PI_2, PI};

/// Where a counter lands: the forward fist in Clasp's flinch.
pub(super) const CONTACT: (f32, f32) = (234.0, 130.0);
/// Where Clasp's body is centred, and the floor both bodies stand on.
pub(super) const CLASP: (f32, f32) = (267.0, 106.0);
pub(super) const FLOOR: f32 = 152.0;
/// The centre of the ring that closes on a lane, and the lane's two heights.
pub(super) const LANE_X: i32 = 192;
pub(super) const LANE_W: i32 = 25;

/// A sprite in the white of a blow landing: every lit pixel in a pale ink by its own light, so the
/// frame has the figure's form and not a blob, and its black outline stays.
pub(super) fn flash_frame(src: &Img) -> Img {
    let mut out = src.clone();
    for y in 0..src.h {
        for x in 0..src.w {
            if let Some([r, g, b]) = src.get(x, y).filter(|&c| c != BLACK) {
                let light = (u32::from(r) * 3 + u32::from(g) * 6 + u32::from(b)) / 10;
                out.put(x, y, if light > 150 { 'H' } else if light > 80 { 'i' } else { 'h' });
            }
        }
    }
    out
}

/// `src` seen `k` times closer about `centre` (nearest pixels): the zoom a last blow punches in.
pub(super) fn punch(src: &Img, centre: (i32, i32), k: f32) -> Img {
    if k <= 1.001 {
        return src.clone();
    }
    let mut out = Img::black(src.w, src.h);
    for y in 0..src.h {
        for x in 0..src.w {
            let sx = centre.0 + ((x - centre.0) as f32 / k).round() as i32;
            let sy = centre.1 + ((y - centre.1) as f32 / k).round() as i32;
            if let Some(c) = src.get(sx, sy) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// The zoom of the winning blow `age` ticks after it, as the realm's own hold (`hold`, ticks it still
/// has to run) lets it carry on: in hard, out slowly.
pub(super) fn zoom(age: u32, hold: u32) -> f32 {
    let t = (age + hold) as f32;
    1.0 + 0.16 * ease::out((t / 5.0).min(1.0)) * (1.0 - ease::in_quad(((t - 5.0) / 26.0).clamp(0.0, 1.0)))
}

/// The forge breathing: embers drift up off the hearth and the lanterns, and the hearth's light
/// swells and gutters.
pub(super) fn embers(world: &mut Img, tick: u64, slot: bool) {
    let sources: &[(i32, i32, i32)] = if slot { &[(352, 112, 10), (14, 66, 4), (376, 52, 4)] } else { &[(56, 56, 3), (322, 56, 3)] };
    let mut n = 0;
    for &(sx, sy, spread) in sources {
        for _ in 0..(if sx > 300 && sy > 100 { 12 } else { 5 }) {
            n += 1;
            let h = hash(n, 11, 0xe3);
            let period = 50 + (h % 44) as i32;
            let t = (tick as i32 + (h >> 8) as i32 % period).rem_euclid(period);
            let k = t as f32 / period as f32;
            let x = sx + ((h >> 16) % (spread as u32 * 2 + 1)) as i32 - spread + ((t as f32 * 0.17 + n as f32).sin() * 3.0) as i32;
            let y = sy - (k * if sy > 100 { 62.0 } else { 26.0 }) as i32;
            world.put(x, y, ramps::EMBER.at(k));
            if k < 0.35 {
                world.put(x, y + 1, ramps::EMBER.at(k + 0.3));
            }
        }
    }
    if slot {
        // The hearth's light swells and gutters, and the three lanterns flicker out of step.
        let swell = 0.5 + 0.5 * (tick as f32 * 0.21).sin() * (tick as f32 * 0.047).cos();
        let mut pen = Pen::new(world, 700);
        pen.glow((352.0, 104.0), 28.0 + swell * 4.0, 0.8, ramps::FIRE, 0.30 + 0.12 * swell);
        for (n, (x, y)) in [(11.0, 52.0), (203.0, 52.0), (376.0, 42.0)].into_iter().enumerate() {
            let flick = 0.5 + 0.5 * (tick as f32 * (0.37 + n as f32 * 0.11) + n as f32 * 2.0).sin();
            pen.glow((x, y), 11.0 + flick * 3.0, 1.0, ramps::FIRE, 0.26 + 0.22 * flick);
        }
    }
}

/// Sparks off the anvil at the left of the bay, a fistful every few seconds.
pub(super) fn anvil(world: &mut Img, tick: u64) {
    let k = (tick % 96) as f32;
    if k >= 14.0 {
        return;
    }
    let seed = (tick / 96) as u32 * 0x9e37 + 5;
    let mut pen = Pen::new(world, 160);
    let s = Spray { gravity: 0.12, angle: -FRAC_PI_2, spread: 1.7, ..Spray::round(seed, 9, (26.0, 142.0), (1.4, 3.4), (8.0, 14.0)) };
    s.each(k, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 1, tick));
}

/// The spark that carries a counter from the fist to the pip it lights: it arcs up, then dives, with
/// a tail of three. `age` runs from the blow; it flies from tick 3 to `hud::FLY`.
pub(super) fn carry(out: &mut Img, from: (f32, f32), to: (f32, f32), age: u32, fly: u32) {
    const START: u32 = 3;
    if age < START || age >= fly {
        return;
    }
    let t = (age - START) as f32 / (fly - START) as f32;
    for (k, ink) in ['H', 'T', 'o', 'R'].into_iter().enumerate() {
        let (x, y) = motion::arc(from, to, 40.0, ease::both((t - k as f32 * 0.07).max(0.0)));
        let s = if k == 0 { 3 } else { 2 };
        out.rect(x as i32 - s / 2, y as i32 - s / 2, s, s, ink);
    }
}

/// The lane an attack will land on and the ring that closes on it: the lane fills as the arm comes,
/// the ring tightens, and in the window both burn gold and the ring beats.
pub(super) fn tell(world: &mut Img, lane: i32, ticks: u32, windup: u32, window: u32, tick: u64) {
    let open = ticks <= window;
    let prog = 1.0 - ticks as f32 / windup.max(1) as f32;
    world.rect(LANE_X, lane, LANE_W, 2, 'S');
    world.rect(LANE_X, lane, (LANE_W as f32 * prog) as i32, 2, if open { 't' } else { 'o' });
    let centre = ((LANE_X + LANE_W / 2) as f32, lane as f32 + 1.0);
    let mut pen = Pen::new(world, 700);
    if open {
        // The closing beat: the ring is a small hot disc that beats, and the lane flares.
        let beat = if (tick / 2) % 2 == 0 { 5.0 } else { 3.5 };
        pen.ring(centre, beat, 1.6, 1.0, ramps::GOLD, 0.0);
        pen.glow(centre, 9.0, 1.0, ramps::HOLY, 0.7);
        world.rect(LANE_X, lane, LANE_W, 2, if (tick / 2) % 2 == 0 { 'H' } else { 'T' });
    } else {
        let r = 4.0 + 18.0 * (1.0 - prog).powf(1.2);
        pen.ring(centre, r, 1.5, 1.0, ramps::EMBER, 0.15 + 0.5 * (1.0 - prog));
        // Four ticks on the ring, so its roundness reads at a glance.
        for (dx, dy) in [(r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)] {
            pen.light((centre.0 + dx) as i32, (centre.1 + dy) as i32, 'o');
        }
    }
}

/// A pair of chevrons that point the guard to the height it must hold: blinking red, at the shield.
pub(super) fn guard_hint(world: &mut Img, up: bool, tick: u64) {
    if (tick / 4) % 2 == 0 {
        return;
    }
    let x = 176;
    for k in 0..2 {
        let y = if up { 94 - k * 6 } else { 152 + k * 6 };
        for i in 0..5i32 {
            let (dx, dy) = (i - 2, if up { -(2 - (i - 2).abs()) } else { 2 - (i - 2).abs() });
            world.put(x + dx, y + dy, 'R');
            world.put(x + dx, y + dy + 1, 'n');
        }
    }
}

/// A counter landing at the fist: the kit's burst and star, rings that grow with the chain, rays for
/// a perfect one, copper chips off the plates and dust off both pairs of feet.
pub(super) fn counter(world: &mut Img, weight: Weight, chain: u8, perfect: bool, age: u32, seed: u32, tick: u64) {
    juice::impact_burst(world, CONTACT, age, seed, weight, tick);
    let a = age as f32;
    let power = 1.25 + f32::from(chain - 1) * 0.5 + if weight >= Weight::Crushing { 0.3 } else { 0.0 };
    let mut pen = Pen::new(world, 900);
    pen.shock(CONTACT, 4.0 + a * 3.4 * power, 2.6, 0.9, if chain >= 2 { ramps::GOLD } else { ramps::EMBER }, a / (11.0 + f32::from(chain) * 3.0));
    if chain >= 2 && age >= 3 {
        pen.shock(CONTACT, 2.0 + (a - 3.0) * 2.8 * power, 2.0, 0.9, ramps::BONE, (a - 3.0) / 12.0);
    }
    if perfect && age < 9 {
        pfx::rays(&mut pen, CONTACT, 12, 34.0 + f32::from(chain) * 8.0, 1.0, seed as f32 * 0.7, ramps::GOLD, a / 9.0);
    }
    if age < 4 {
        // The blades meeting: a hard dithered flash in the middle of the burst.
        pen.glow(CONTACT, 14.0 + f32::from(chain) * 4.0, 1.0, ramps::HOLY, 0.9 * (1.0 - a / 4.0));
    }
    if age < 30 {
        let chips = Spray {
            gravity: 0.17,
            drag: 0.05,
            angle: -0.35,
            spread: 2.2,
            ..Spray::round(seed ^ 0x61, 6 + u32::from(chain) * 3, CONTACT, (1.8, 4.4), (14.0, 28.0))
        };
        chips.each(a, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::EARTH, 2, tick));
    }
    dust(&mut pen, (150.0, FLOOR), PI, age, seed ^ 0x13, tick, 3 + u32::from(chain));
    dust(&mut pen, (282.0, FLOOR), 0.0, age, seed ^ 0x29, tick, 3 + u32::from(chain));
}

/// A puff of floor dust thrown along `angle` (0 right, PI left).
pub(super) fn dust(pen: &mut Pen, at: (f32, f32), angle: f32, age: u32, seed: u32, tick: u64, n: u32) {
    let puff = Spray { angle, spread: 0.7, gravity: -0.012, drag: 0.15, squash: 0.35, ..Spray::round(seed, n, at, (1.0, 2.3), (9.0, 17.0)) };
    puff.each(age as f32, |m| pfx::mote(pen, &m, Shape::Dot, ramps::SMOKE, 2, tick));
}

/// Clasp's last blow: rings out of its body, rays, plates flung out and down, a rain of sparks, two
/// warm eyes lifting away and smoke curling up off what is left.
pub(super) fn fell(world: &mut Img, age: u32, seed: u32, tick: u64) {
    let a = age as f32;
    let mut pen = Pen::new(world, 1500);
    pen.shock(CLASP, 5.0 + a * 6.0, 2.8, 0.9, ramps::EMBER, a / 30.0);
    if age >= 4 {
        pen.shock(CLASP, 3.0 + (a - 4.0) * 4.6, 2.0, 0.9, ramps::BONE, (a - 4.0) / 22.0);
    }
    if age < 12 {
        pfx::rays(&mut pen, CLASP, 18, 72.0, 1.0, seed as f32, ramps::GOLD, a / 12.0);
    }
    let plates = Spray {
        gravity: 0.14,
        drag: 0.04,
        angle: -FRAC_PI_2,
        spread: 3.4,
        ..Spray::round(seed, 36, CLASP, (2.0, 6.2), (20.0, 42.0))
    };
    plates.each(a, |m| {
        let ramp = if m.i % 3 == 0 { ramps::WATER } else { ramps::EARTH };
        pfx::mote(&mut pen, &m, Shape::Shard, ramp, 3, tick);
    });
    let sparks = Spray { gravity: 0.07, ..Spray::round(seed ^ 0xc3, 26, CLASP, (2.4, 6.2), (10.0, 24.0)) };
    sparks.each(a, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 2, tick));
    let eyes = Spray { gravity: -0.035, drag: 0.05, angle: -FRAC_PI_2, spread: 0.7, ..Spray::round(seed ^ 0x77, 2, (CLASP.0, CLASP.1 - 14.0), (0.9, 1.3), (30.0, 40.0)) };
    eyes.each(a, |m| pfx::mote(&mut pen, &m, Shape::Star, ramps::GOLD, 1, tick));
    if age >= 6 {
        let smoke = Spray { gravity: -0.05, drag: 0.07, angle: -FRAC_PI_2, spread: 1.4, ..Spray::round(seed ^ 0x55, 7, (CLASP.0, FLOOR - 14.0), (0.4, 1.1), (22.0, 40.0)) };
        smoke.each(a - 6.0, |m| pfx::mote(&mut pen, &m, Shape::Dot, ramps::SMOKE, 3, tick));
    }
    dust(&mut pen, (CLASP.0 - 20.0, FLOOR), PI, age, seed ^ 0x31, tick, 6);
    dust(&mut pen, (CLASP.0 + 20.0, FLOOR), 0.0, age, seed ^ 0x37, tick, 6);
}

/// Clasp's own plates, cut from its body in tiles, flung out and down and landing on the floor: the
/// pieces of `src` (stood at `origin`) that a hash of the seed picks, `age` ticks after the blow.
pub(super) fn plates(world: &mut Img, src: &Img, origin: (i32, i32), age: u32, seed: u32) {
    const TILE: i32 = 10;
    if age >= 58 {
        return;
    }
    let t = age as f32;
    for ty in 0..src.h / TILE {
        for tx in 0..src.w / TILE {
            let h = hash(tx, ty, seed);
            if h % 3 != 0 {
                continue;
            }
            let mut tile = Img::new(TILE, TILE);
            let mut lit = 0;
            for y in 0..TILE {
                for x in 0..TILE {
                    if let Some(c) = src.get(tx * TILE + x, ty * TILE + y) {
                        tile.set(x, y, c);
                        lit += 1;
                    }
                }
            }
            if lit < 20 {
                continue;
            }
            // Out along the way from the middle of the body, and up.
            let (cx, cy) = ((tx * TILE + TILE / 2 - 50) as f32, (ty * TILE + TILE / 2 - 56) as f32);
            let len = cx.hypot(cy).max(1.0);
            let speed = 1.6 + (h >> 8) as f32 % 3.0 * 0.9;
            let (vx, vy) = (cx / len * speed, cy / len * speed - 2.2 - ((h >> 4) % 3) as f32 * 0.5);
            let rest = FLOOR - 6.0 + ((h >> 12) % 7) as f32 - origin.1 as f32 - (ty * TILE) as f32;
            let g = 0.2;
            let drag = 1.0 - crate::motion::math::exp(-t * 0.12);
            let y = (vy * t + 0.5 * g * t * t).min(rest);
            let x = vx * drag / 0.12;
            // Come to rest on the floor and stay a while, then go in a dither.
            let landed = vy * t + 0.5 * g * t * t >= rest;
            if age > 40 && bayer(tx * TILE, ty * TILE) < (t - 40.0) / 18.0 {
                continue;
            }
            let _ = landed;
            world.stamp(&tile, origin.0 + tx * TILE + x as i32, origin.1 + ty * TILE + y as i32);
        }
    }
}

/// A guard folding under the arm: copper sparks and dust at the shield, a short red ring.
pub(super) fn miss(world: &mut Img, lane: i32, age: u32, seed: u32, tick: u64) {
    let at = (176.0, lane as f32 + 4.0);
    let a = age as f32;
    let mut pen = Pen::new(world, 400);
    pen.shock(at, 3.0 + a * 2.6, 2.0, 0.9, ramps::BLOOD, a / 10.0);
    let sparks = Spray { gravity: 0.1, angle: PI, spread: 2.2, ..Spray::round(seed, 9, at, (1.6, 3.8), (7.0, 14.0)) };
    sparks.each(a, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::EMBER, 1, tick));
    dust(&mut pen, (166.0, FLOOR), PI, age, seed ^ 0x8d, tick, 5);
}

/// A chip of a pip or a bar giving way: a few shards in `ramp` falling from `at`.
pub(super) fn chips(out: &mut Img, at: (f32, f32), age: u32, seed: u32, tick: u64, n: u32) {
    let mut pen = Pen::new(out, 120);
    let s = Spray { gravity: 0.2, drag: 0.04, angle: -FRAC_PI_2, spread: 2.6, ..Spray::round(seed, n, at, (1.0, 2.8), (10.0, 18.0)) };
    s.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::EMBER, 1, tick));
}

/// A card's burst when a rival lands a counter: sparks round the portrait and a ring.
pub(super) fn cheer(out: &mut Img, at: (f32, f32), age: u32, seed: u32, tick: u64, big: bool) {
    let mut pen = Pen::new(out, 160);
    let sparks = Spray { gravity: 0.06, ..Spray::round(seed, if big { 12 } else { 7 }, at, (1.0, 2.6), (7.0, 14.0)) };
    sparks.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 1, tick));
    pen.ring(at, 3.0 + age as f32 * 1.6, 1.4, 1.0, ramps::GOLD, age as f32 / 9.0);
}
