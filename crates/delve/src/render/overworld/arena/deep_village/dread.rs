//! The dreads of the Deep, drawn (`rules::deep::dread`): each people's
//! fear in its own shape and inks, breathing as it waits, winding up its
//! signature where everyone can see, striking, then standing spent.
//!
//! The Glow-Drinker is a great pale moth-bat, its belly swollen with the
//! light it drank; the Seam-Swallower a slug of the mines with crystal
//! through its hide and a swallowed cart inside it; the Coal-Glutton a fat
//! imp of stolen coals split along seams of fire, slit eyes under a heavy
//! brow and a grin of coal teeth with the furnace behind them, the first
//! spark for a crown; the Mist-Mother a drowned shape in a hood with a
//! lantern that gives no warmth, whose beam is that lantern's light thrown
//! down onto the floor like a searchlight; the Snare-Queen a spider the
//! size of a nest, hung from the dark on her own thread; the Blight a
//! beetle of rot with fungus for a back; the Drowned Toll a giant with a
//! cracked bell for a head and its clapper for a hammer, whose beam is its
//! hum, rung out down the floor in rings of bronze; the Brine-Wyrm a
//! salt-crusted lamprey rearing out of its coils.
//!
//! Under each, while it winds up, the floor shows what is coming in its
//! people's way: the room's light streaming into the Glow-Drinker, crystal
//! breaking up along the Seam-Swallower's lane, cinders falling where the
//! Coal-Glutton will land, mist gathering at the Mist-Mother's feet, a web
//! spun where the Snare-Queen will drop, rot creeping out round the Blight,
//! the floor ringing round the Toll, salt cracking along the Wyrm's lane.
//! The checked charge-up's own marks (`signatures`) are drawn as well:
//! these are its people's colour on top, never in place of the warning.
//! A dread glows at its rim as it winds up (`marks::rim_heat`), so the
//! pose it winds up in shows through the warning.

use super::super::super::ink::{Img, hash, vnoise};
use super::super::super::material as mat;
use super::super::pfx::{Pen, Ramp, ramps};
use super::super::{UNIT, at};
use super::sky;
use crate::rules::{
    Boss, Enemy, Run,
    bosses::Windup,
    deep::{Culture, dread},
    signatures as fight,
};
use std::f32::consts::{PI, TAU};

/// How a dread stands this tick.
#[derive(Clone, Copy, Debug)]
struct Pose {
    /// How far through its wind-up, `0..=1` (0 when not winding).
    wind: f32,
    /// Mid-blow: dashing, or in the air.
    rush: bool,
    /// Spent after a blow.
    spent: bool,
    tick: u32,
    /// The charge-up it is at.
    kind: Option<Windup>,
    /// A beam it has loosed is burning.
    burn: bool,
}

fn pose(enemy: &Enemy, boss: &Boss, tick: u32) -> Pose {
    let up = fight::winding(enemy, boss);
    let wind = match (enemy.stage, up) {
        (fight::WIND, Some(up)) => (enemy.timer as f32 / up.wind.max(1) as f32).min(1.0),
        _ => 0.0,
    };
    let burn = enemy.stage == fight::DAZED && up.is_some_and(|u| u.kind == Windup::Beam && enemy.timer <= u32::from(fight::beam_burn(u)));
    Pose {
        wind,
        rush: matches!(enemy.stage, fight::DASH | fight::JUMP),
        spent: enemy.stage == fight::DAZED && !burn,
        tick,
        kind: up.map(|u| u.kind),
        burn,
    }
}

/// The dread `enemy` is, drawn, if it is one.
pub(in super::super) fn figure(run: &Run, enemy: &Enemy, boss: &Boss, tick: u32) -> Option<Img> {
    if !dread::is(boss) {
        return None;
    }
    let culture = dread::culture_of(&boss.id)?;
    let p = pose(enemy, boss, tick);
    let im = match culture {
        Culture::Capfolk => glow_drinker(p),
        Culture::Glimmerkin => seam_swallower(p),
        Culture::Emberfolk => coal_glutton(p),
        Culture::Lampwick => mist_mother(p),
        Culture::Roostfolk => snare_queen(p),
        Culture::Rootkin => blight(p),
        Culture::Chimefolk => drowned_toll(p),
        Culture::Saltkin => brine_wyrm(p),
    };
    // The long ones face left as drawn, the Mist-Mother holds her light to
    // the right: each turns to face the way it means to go.
    let turned = match culture {
        Culture::Glimmerkin | Culture::Rootkin | Culture::Saltkin => enemy.dir.0 > 0.2,
        Culture::Lampwick => enemy.dir.0 < -0.2,
        _ => false,
    };
    let mut im = if turned { im.flip_h() } else { im };
    // A dark rim round it, so it reads against any floor.
    im.outline_outside('k');
    // In the air on a leap: lifted off its shadow.
    if enemy.stage == fight::JUMP {
        let t = enemy.timer as f32 / fight::FLIGHT as f32;
        let rise = ((t * PI).sin() * 30.0).max(0.0) as i32;
        let mut airborne = Img::new(im.w, im.h + rise);
        airborne.stamp(&im, 0, 0);
        return Some(airborne);
    }
    // Shaking at the last of its wind-up.
    if p.wind > 0.7 {
        let mut shaking = Img::new(im.w + 2, im.h);
        shaking.stamp(&im, (tick % 2) as i32 * 2, 0);
        return Some(shaking);
    }
    let _ = run;
    Some(im)
}

/// `culture`'s dread at rest as `.boss` art rows, at most 32 by 32: the
/// figure the trophy room hangs (`print_dread_art`).
#[cfg(any(test, feature = "testing"))]
pub(in super::super) fn rows_for(culture: Culture) -> Vec<String> {
    let p = Pose { wind: 0.0, rush: false, spent: false, tick: 0, kind: None, burn: false };
    let mut im = match culture {
        Culture::Capfolk => glow_drinker(p),
        Culture::Glimmerkin => seam_swallower(p),
        Culture::Emberfolk => coal_glutton(p),
        Culture::Lampwick => mist_mother(p),
        Culture::Roostfolk => snare_queen(p),
        Culture::Rootkin => blight(p),
        Culture::Chimefolk => drowned_toll(p),
        Culture::Saltkin => brine_wyrm(p),
    };
    im.outline_outside('k');
    let inks: Vec<char> = "kKZXgjGJhiHfDFEelNmLAMyCYnbIBpPrRoOtTsxSuUvVWqQz0123wa4@56$c987".chars().collect();
    let k = (im.w.max(im.h) as f32 / 32.0).max(1.0);
    let (w, h) = ((im.w as f32 / k) as i32, (im.h as f32 / k) as i32);
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| {
                    let c = im.get((x as f32 * k + k / 2.0) as i32, (y as f32 * k + k / 2.0) as i32);
                    c.and_then(|c| inks.iter().copied().find(|&ch| super::super::super::ink::ink(ch) == Some(c))).unwrap_or('.')
                })
                .collect()
        })
        .collect()
}

/// `n` points round an ellipse, for scalloped edges and rings.
fn round_pts(cx: f32, cy: f32, rx: f32, ry: f32, n: usize) -> impl Iterator<Item = (i32, i32)> {
    (0..n).map(move |k| {
        let a = k as f32 / n as f32 * TAU;
        ((cx + a.cos() * rx) as i32, (cy + a.sin() * ry) as i32)
    })
}

/// A shaded ball: `ramp` dark to light, lit from the upper left.
fn ball(im: &mut Img, (cx, cy): (f32, f32), (rx, ry): (f32, f32), ramp: &[char], seed: u32) {
    mat::lobe(im, (cx, cy), (rx, ry), ramp, seed, 2.5, 0.04, &|_, _| true);
}

// ─── the Capfolk's dread ─────────────────────────────────────────────────────

/// The Glow-Drinker: a moth-bat as wide as a cottage, its wings spotted
/// with eyes of stolen glow, its belly a lamp of what it drank, a curled
/// tongue under its head that uncurls to drink the room's light.
fn glow_drinker(p: Pose) -> Img {
    let (w, h) = (64, 50);
    let mut im = Img::new(w, h);
    let cx = 32.0;
    let drinking = p.kind == Some(Windup::Inhale) && p.wind > 0.0;
    let flap = if p.spent { 0.0 } else { (p.tick as f32 * if p.wind > 0.0 { 0.5 } else { 0.22 }).sin() };
    let spread = 1.0 + p.wind * 0.15;
    // The wings: membranes from the shoulders to their tips, scalloped
    // between the finger-bones, an eye of glow on each.
    for side in [-1.0f32, 1.0] {
        let sh = (cx + side * 4.0, 17.0);
        let tips = [(28.0 * spread, -13.0 + flap * 5.0), (31.0 * spread, 2.0 + flap * 3.0), (25.0 * spread, 17.0 + flap * 2.0), (14.0, 22.0)];
        // Fill: each fan between neighbouring bones.
        for pair in tips.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            for s in 0..=24 {
                let f = s as f32 / 24.0;
                let tip = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
                // The trailing edge scoops in between the bones.
                let scoop = (f * PI).sin() * 0.22;
                let end = (sh.0 + side * tip.0 * (1.0 - scoop), sh.1 + tip.1 * (1.0 - scoop));
                let n = ((end.0 - sh.0).abs().max((end.1 - sh.1).abs())) as i32;
                for k in 0..=n {
                    let g = k as f32 / n.max(1) as f32;
                    let (x, y) = (sh.0 + (end.0 - sh.0) * g, sh.1 + (end.1 - sh.1) * g);
                    let mark = vnoise(x / 3.0, y / 3.0, 9601 + side as u32);
                    let ink = if g > 0.93 { 'K' } else if mark > 0.7 { 'j' } else if g < 0.3 { 'G' } else { 'g' };
                    im.put(x as i32, y as i32, ink);
                }
            }
        }
        // The finger-bones.
        for &(tx, ty) in &tips {
            im.line(sh.0 as i32, sh.1 as i32, (sh.0 + side * tx) as i32, (sh.1 + ty) as i32, 'J');
        }
        // An eye of glow on each wing, brighter as it drinks.
        let (ex, ey) = (sh.0 + side * 15.0 * spread, sh.1 + 1.0 + flap * 2.0);
        im.ellipse(ex, ey, 4.5, 3.5, 'K');
        im.ellipse(ex, ey, 3.2, 2.4, if drinking { '2' } else { 'q' });
        im.ellipse(ex, ey, 1.6, 1.2, if drinking && p.wind > 0.5 { 'w' } else { 'z' });
    }
    // The body: furred thorax, the belly of drunk light under it.
    ball(&mut im, (cx, 18.0), (6.0, 7.0), &['g', 'j', 'G', 'J', 'h', 'i'], 9602);
    let glow = if drinking { 0.5 + p.wind * 0.5 } else if p.spent { 0.15 } else { 0.35 + (p.tick as f32 * 0.08).sin() * 0.1 };
    let r = 8.0 + glow * 2.5;
    im.ellipse(cx, 30.0, r + 1.0, r + 2.0, 'K');
    im.ellipse(cx, 30.0, r, r + 1.0, 'q');
    im.ellipse(cx, 30.0, r * 0.8, r * 0.85, if glow > 0.3 { '1' } else { 'Q' });
    im.ellipse(cx - 1.0, 29.0, r * 0.55, r * 0.6, if glow > 0.45 { '2' } else { 'z' });
    im.ellipse(cx - 2.0, 27.0, r * 0.25, r * 0.3, if glow > 0.6 { 'w' } else { '3' });
    for k in 0..5 {
        let y = 24 + k * 3;
        im.line((cx - r * 0.7) as i32, y, (cx + r * 0.7) as i32, y, if k % 2 == 0 { 'K' } else { 'q' });
    }
    im.ellipse(cx, 30.0, r * 0.55, r * 0.6, if glow > 0.45 { '2' } else { 'z' });
    im.put(cx as i32 - 2, 27, if glow > 0.5 { 'w' } else { '3' });
    // Its head: great black eyes, feathered feelers, the curled tongue.
    ball(&mut im, (cx, 9.0), (5.0, 4.5), &['g', 'j', 'G', 'J', 'h'], 9603);
    for side in [-1, 1] {
        let ex = cx as i32 + side * 3;
        im.ellipse(ex as f32, 9.0, 2.6, 2.8, 'k');
        im.put(ex - side, 8, if drinking { '3' } else { '1' });
        // Feelers, feathered, curling up and out.
        for k in 0..9 {
            let f = k as f32 / 8.0;
            let (fx, fy) = (cx + side as f32 * (2.0 + f * 9.0), 5.0 - f * 6.0 + f * f * 4.0);
            im.put(fx as i32, fy as i32, 'h');
            if k % 2 == 0 {
                im.put(fx as i32, fy as i32 - 1, 'J');
            }
        }
    }
    if drinking {
        // The tongue uncurls and drinks.
        let reach = 6.0 + p.wind * 14.0;
        im.line(cx as i32, 13, cx as i32, 13 + reach as i32, 'i');
        im.put(cx as i32, 13 + reach as i32, '3');
    } else {
        for k in 0..10 {
            let a = k as f32 * 0.7;
            let rr = 3.0 - k as f32 * 0.25;
            im.put((cx + a.cos() * rr) as i32, (15.0 + a.sin() * rr) as i32, 'h');
        }
    }
    // Thin legs, dangling.
    for side in [-1, 1] {
        im.line(cx as i32 + side * 3, 38, cx as i32 + side * 6, 47, 'G');
        im.line(cx as i32 + side * 6, 47, cx as i32 + side * 8, 49, 'j');
    }
    im
}

// ─── the Glimmerkin's dread ──────────────────────────────────────────────────

/// The Seam-Swallower: a slug of the deep seams, glossy and blue-grey,
/// crystal broken out through its back, a swallowed cart dark inside it,
/// a round mouth of teeth that opens as it means to charge.
fn seam_swallower(p: Pose) -> Img {
    let (w, h) = (68, 46);
    let mut im = Img::new(w, h);
    let rear = p.wind;
    let (cx, cy) = (36.0, 30.0 - rear * 4.0);
    let (rx, ry) = (27.0 - rear * 3.0, 14.0 + rear * 3.0);
    // The body: a long glossy blob, a sheen along its back.
    for y in (cy - ry) as i32..=(cy + ry) as i32 {
        for x in (cx - rx) as i32..=(cx + rx) as i32 {
            let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
            // Flat underneath where it lies on the floor.
            let d = dx * dx + if dy > 0.0 { dy * dy * dy.abs() } else { dy * dy };
            if d >= 1.0 || y > 44 {
                continue;
            }
            let z = (1.0 - d).sqrt();
            let lit = (z * 0.7 - dy * 0.5 - dx * 0.2).clamp(0.0, 1.0);
            let ramp = ['x', 'S', 'S', 'u', 'U', 'v', 'V'];
            let mut i = (lit * 6.0) as i32;
            if vnoise(x as f32 / 4.0, y as f32 / 3.0, 9611) > 0.7 {
                i -= 1;
            }
            if d > 0.9 {
                i = 0;
            }
            im.put(x, y, ramp[i.clamp(0, 6) as usize]);
        }
    }
    // The cart it swallowed, dark through its hide: a tub tipped on its
    // side, wheels up, ore spilled round it.
    let (kx, ky) = (cx as i32 + 4, cy as i32 + 2);
    for y in ky - 4..ky + 4 {
        let lean = (ky + 4 - y) / 3;
        for x in kx - 7 + lean..kx + 7 - lean {
            if (x + y) % 2 == 0 {
                im.put(x, y, 'x');
            }
        }
    }
    for (wx, wy) in [(kx - 5, ky - 5), (kx + 4, ky - 5)] {
        im.ellipse(wx as f32, wy as f32, 1.5, 1.5, 's');
    }
    for k in 0..5 {
        im.put(kx - 9 + k * 4, ky + 3 + k % 2, 'G');
    }
    // Crystal through its back.
    for (k, (dx, tall)) in [(-14i32, 9), (-6, 14), (3, 11), (11, 15), (18, 8)].into_iter().enumerate() {
        let bx = cx as i32 + dx;
        let foot = (cy - ry * (1.0 - (dx as f32 / rx).powi(2)).max(0.0).sqrt()) as i32 + 3;
        sky::crystal(&mut im, bx, foot, tall + (rear * 3.0) as i32, k as u32 + 30);
        if p.wind > 0.4 {
            im.put(bx, foot - tall - 1, 'w');
        }
    }
    // Its mouth, at the front: a ring of teeth round the dark.
    let open = 3.0 + rear * 4.0 + if p.rush { 3.0 } else { 0.0 };
    let (mx, my) = ((cx - rx + 5.0) as i32, cy as i32 + 2);
    im.ellipse(mx as f32, my as f32, open + 2.0, open + 3.0, 'S');
    im.ellipse(mx as f32, my as f32, open, open + 1.0, 'k');
    for (tx, ty) in round_pts(mx as f32, my as f32, open + 0.5, open + 1.5, 10) {
        im.put(tx, ty, 'H');
    }
    // Eye-stalks, waving.
    for (k, dx) in [8, 13].into_iter().enumerate() {
        let sway = ((p.tick as f32 * 0.12 + k as f32).sin() * 2.0) as i32;
        let base = (mx + dx, (cy - ry + 4.0) as i32 + k as i32);
        im.line(base.0, base.1, base.0 - 2 + sway, base.1 - 8, 'v');
        im.put(base.0 - 2 + sway, base.1 - 9, if p.wind > 0.0 { 'w' } else { '3' });
    }
    // Its slime under it.
    for x in (cx - rx) as i32..(cx + rx) as i32 {
        if hash(x, 1, 9612) % 3 == 0 {
            im.put(x, 45, 'S');
        }
    }
    im
}

// ─── the Emberfolk's dread ───────────────────────────────────────────────────

/// The Coal-Glutton: a fat imp of stolen coals, a lump of black coal split
/// into facets along seams glowing with what it ate, stubby arms, little
/// horns, and an iron crown with the forge's first spark set in it. Its
/// face is the fire inside it showing through: two slit eyes under a heavy
/// brow, and a grin of jagged coal teeth with the furnace behind them. It
/// crouches and gapes before it leaps (BELLYFLOP); its cheeks puff and its
/// mouth purses to a little O as it draws the room in (HUUUFF); spent, it
/// sags, its tongue out.
fn coal_glutton(p: Pose) -> Img {
    let (w, h) = (60, 56);
    let mut im = Img::new(w, h);
    let leaping = p.kind == Some(Windup::Leap) && (p.wind > 0.0 || p.rush);
    let drawing = p.kind == Some(Windup::Inhale) && p.wind > 0.0;
    let squat = if leaping { p.wind * 0.2 } else { 0.0 };
    let puff = if drawing { p.wind } else { 0.0 };
    let (cx, cy) = (30.0, 36.0 + squat * 10.0);
    let (rx, ry) = (19.0 * (1.0 + squat) + puff * 2.0, 17.0 * (1.0 - squat) + puff);
    let breath = (p.tick as f32 * 0.1).sin();
    let heat = if p.wind > 0.0 { 0.55 + p.wind * 0.45 } else if p.spent { 0.25 } else { 0.42 + breath * 0.08 };
    // Stubby legs, three toes each.
    for side in [-1.0f32, 1.0] {
        let fx = cx + side * 9.0;
        im.rect((fx - 3.0) as i32, (cy + ry - 4.0) as i32, 7, 6, 'K');
        im.line((fx - 4.0) as i32, (cy + ry + 1.0) as i32, (fx + 4.0) as i32, (cy + ry + 1.0) as i32, 'k');
        for t in [-3, 0, 3] {
            im.put(fx as i32 + t, (cy + ry + 1.0) as i32, 'X');
        }
    }
    // The body: coal in facets, split along seams that glow, hottest deep
    // in its middle.
    let seeds: Vec<(f32, f32)> = (0..15)
        .map(|k| {
            let hk = hash(k, 5, 9625);
            let a = (hk % 628) as f32 / 100.0;
            let r = ((hk >> 10) % 100) as f32 / 100.0;
            (cx + a.cos() * rx * r.sqrt() * 0.95, cy + a.sin() * ry * r.sqrt() * 0.95)
        })
        .collect();
    let coal = ['k', 'K', 'X', 'g', 'j', 'G'];
    for y in (cy - ry) as i32..=(cy + ry) as i32 {
        for x in (cx - rx) as i32..=(cx + rx) as i32 {
            let (px_, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (dx, dy) = ((px_ - cx) / rx, (py - cy) / ry);
            let d = dx * dx + dy * dy;
            if d >= 1.0 {
                continue;
            }
            let (mut d1, mut d2, mut id) = (f32::MAX, f32::MAX, 0usize);
            for (k, &(sx, sy)) in seeds.iter().enumerate() {
                let dd = (sx - px_).hypot((sy - py) * 1.2);
                if dd < d1 {
                    (d2, d1, id) = (d1, dd, k);
                } else if dd < d2 {
                    d2 = dd;
                }
            }
            let z = (1.0 - d).sqrt();
            let lit = (-dx * 0.45 - dy * 0.6 + z * 0.65).clamp(0.0, 1.0);
            if d2 - d1 < 1.3 && d < 0.92 {
                // A seam: the fire in it, hotter nearer the middle.
                let core = (1.0 - d) * heat * 1.6;
                im.put(x, y, if core > 0.95 { '6' } else if core > 0.5 { '@' } else if d < 0.8 || core > 0.2 { '7' } else { '8' });
                continue;
            }
            // A facet: each leans its own way to the light, its edge by the
            // seam catching it.
            let lean = (hash(id as i32, 6, 9626) % 100) as f32 / 100.0 - 0.5;
            let mut v = lit * 0.85 + lean * 0.35;
            if d2 - d1 < 2.2 && lean > 0.0 {
                v += 0.18;
            }
            let i = (v * coal.len() as f32).clamp(0.0, (coal.len() - 1) as f32) as usize;
            im.put(x, y, coal[if d > 0.86 { i.min(1) } else { i }]);
        }
    }
    // Arms, stubby: up when it means to leap, on its belly as it draws in.
    for side in [-1.0f32, 1.0] {
        let (sx, sy) = (cx + side * rx * 0.9, cy - 1.0);
        let (hx, hy) = if leaping {
            (sx + side * 6.0, sy - 9.0)
        } else if drawing {
            (sx - side * 3.0, sy + 6.0)
        } else {
            (sx + side * 5.0, sy + 3.0 + breath)
        };
        im.line(sx as i32, sy as i32, hx as i32, hy as i32, 'K');
        im.line(sx as i32, sy as i32 + 1, hx as i32, hy as i32 + 1, 'X');
        im.ellipse(hx, hy, 2.6, 2.4, 'K');
        im.put(hx as i32 - 1, hy as i32 - 1, 'g');
        for t in [-1.0f32, 0.0, 1.0] {
            im.put((hx + side * 2.0 + t * 0.5) as i32, (hy + t * 1.5) as i32, 'X');
        }
    }
    face(&mut im, p, (cx, cy, rx, ry), heat, leaping, drawing);
    // Horns, curling up, and the crown with the first spark.
    let top = (cy - ry) as i32;
    for side in [-1i32, 1] {
        let base = (cx as i32 + side * 9, top + 3);
        for k in 0..6 {
            let f = k as f32 / 5.0;
            let (x, y) = (base.0 + side * (f * 4.0) as i32, base.1 - (f * 8.0) as i32 + (f * f * 2.0) as i32);
            im.put(x, y, if k < 3 { 'g' } else { 'j' });
            if k < 3 {
                im.put(x + side, y, 'X');
            }
        }
    }
    im.rect(cx as i32 - 8, top - 3, 17, 4, 'X');
    im.line(cx as i32 - 8, top - 3, cx as i32 + 8, top - 3, 'G');
    for k in 0..5 {
        let x = cx as i32 - 8 + k * 4;
        im.line(x, top - 3, x, top - 6 - i32::from(k == 2), 'G');
        im.put(x, top - 7 - i32::from(k == 2), 'J');
    }
    im.line(cx as i32 - 8, top, cx as i32 + 8, top, 'g');
    let spark = if p.wind > 0.5 { 'w' } else { '9' };
    im.ellipse(cx, (top - 2) as f32, 2.2, 2.0, '6');
    im.put(cx as i32, top - 2, spark);
    im.put(cx as i32, top - 5, '6');
    im
}

/// The Coal-Glutton's face, on its body `(cx, cy, rx, ry)`: the brow, the
/// eyes and the grin, burning at `heat`.
fn face(im: &mut Img, p: Pose, (cx, cy, rx, ry): (f32, f32, f32, f32), heat: f32, leaping: bool, drawing: bool) {
    let ey = (cy - ry * 0.36) as i32;
    let my = cy + ry * 0.14;
    let glance = [0, 1, 0, -1][(p.tick / 40 % 4) as usize];
    // Cheeks, puffing out as it draws in.
    if drawing {
        for side in [-1.0f32, 1.0] {
            let c = (cx + side * rx * 0.55, my - 1.0);
            im.ellipse(c.0, c.1, 4.0 + p.wind * 2.0, 3.0 + p.wind * 1.5, 'g');
            im.ellipse(c.0 - side, c.1 - 1.0, 2.5 + p.wind, 1.6 + p.wind * 0.6, 'j');
            im.put((c.0 - side * 2.0) as i32, c.1 as i32 - 2, 'G');
        }
    }
    // Its eyes: slits of fire in dark sockets, a heavy brow over them
    // coming down to the middle; squeezed shut as it draws in, wide as it
    // means to leap, the lids down when it is spent.
    for side in [-1i32, 1] {
        let ex = cx as i32 + side * 7;
        im.ellipse(ex as f32, ey as f32, 4.6, 3.2, 'k');
        if drawing && p.wind > 0.3 {
            for o in -3..=3 {
                im.put(ex + o, ey - (2 - o.abs() / 2), if o.abs() < 2 { '6' } else { '@' });
            }
        } else {
            let tall = if leaping { 2.2 } else { 1.7 };
            im.ellipse(ex as f32, ey as f32, 3.4, tall, '@');
            im.ellipse(ex as f32 - side as f32 * 0.3, ey as f32 - 0.3, 2.4, tall - 0.6, '6');
            im.put(ex - side, ey - 1, if p.wind > 0.5 { 'w' } else { '9' });
            // The slit, looking about.
            let px_ = ex + glance + side * i32::from(leaping);
            im.line(px_, ey - 1, px_, ey + 1, 'k');
            if p.spent {
                im.rect(ex - 4, ey - 3, 9, 3, 'K');
                im.line(ex - 4, ey, ex + 4, ey, 'X');
            }
        }
        // The brow: thick and low, slanting down to the middle.
        for o in -5..=5 {
            let y = ey - 4 + (o * side + 5) / 4;
            im.put(ex + o, y, 'k');
            im.put(ex + o, y - 1, 'K');
            im.put(ex + o, y - 2, if o % 2 == 0 { 'g' } else { 'X' });
        }
    }
    // A nose of two holes, a breath of smoke off them.
    for side in [-1i32, 1] {
        im.put(cx as i32 + side * 2, ey + 3, 'k');
        let puff = ((p.tick + side.unsigned_abs() * 7) % 24) as i32;
        if !drawing && puff < 12 {
            im.put(cx as i32 + side * (2 + puff / 6), ey + 2 - puff / 3, if puff < 6 { 'G' } else { 'j' });
        }
    }
    // The grin: a wide crescent, its ends curling up, the furnace inside;
    // jagged teeth of coal across its top and a few up from below. As it
    // draws in it purses to a little round O.
    let half = if drawing { 13.0 - p.wind * 9.0 } else { 13.0 + if leaping { 2.0 } else { 0.0 } };
    let open = if drawing {
        3.0 + p.wind * 2.0
    } else if leaping {
        4.0 + p.wind * 5.0 + if p.rush { 3.0 } else { 0.0 }
    } else if p.spent {
        3.0
    } else {
        4.0 + breath_of(p.tick) * 0.6
    };
    let hn = half.ceil() as i32;
    for o in -hn..=hn {
        let u = o as f32 / half;
        if u.abs() > 1.0 {
            continue;
        }
        let curl = if drawing { 0.0 } else { u * u * 4.0 };
        let gap = open * (1.0 - u * u).max(0.0).powf(if drawing { 0.5 } else { 0.7 });
        let top = (my - curl - if drawing { gap / 2.0 } else { 0.0 }).round() as i32;
        let bottom = top + gap.round() as i32;
        let x = cx as i32 + o;
        im.put(x, top - 1, 'k');
        for y in top..=bottom.max(top) {
            let depth = (y - top) as f32 / gap.max(1.0);
            let ink = if heat > 0.5 && depth > 0.35 && depth < 0.85 && u.abs() < 0.6 { '6' } else if depth > 0.2 { '@' } else { '7' };
            im.put(x, y, ink);
        }
        im.put(x, bottom + 1, 'k');
        if !drawing {
            // Teeth: a jag down every third pixel, a few up from the lip.
            if o.rem_euclid(3) == 0 && gap > 1.5 {
                im.put(x, top, 'X');
                im.put(x, top + 1, 'K');
                im.put(x + 1, top, 'K');
                if gap > 4.0 {
                    im.put(x, top + 2, 'k');
                }
            }
            if o.rem_euclid(5) == 2 && gap > 3.0 && u.abs() < 0.8 {
                im.put(x, bottom, 'K');
                im.put(x, bottom - 1, 'k');
            }
        }
    }
    // Spent, its tongue lolls out over its lip; hungry, molten drool.
    if p.spent {
        let ty = (my + open + 1.0) as i32;
        im.ellipse(cx + 3.0, ty as f32 + 1.0, 2.4, 2.0, '@');
        im.put(cx as i32 + 3, ty + 1, '6');
        im.put(cx as i32 + 2, ty, '7');
    } else if !drawing {
        let t = ((p.tick + 5) % 26) as i32;
        let (dx, dy) = (cx as i32 + (half * 0.7) as i32, (my - 1.5 + open * 0.6) as i32);
        if t < 18 {
            im.put(dx, dy + t / 4, '@');
            if t > 6 {
                im.put(dx, dy + t / 4 - 1, '7');
            }
        }
    }
}

fn breath_of(tick: u32) -> f32 {
    (tick as f32 * 0.1).sin()
}

// ─── the Lampwick's dread ────────────────────────────────────────────────────

/// The Mist-Mother: a drowned shape in a long wet hood come to a point,
/// two pale eyes in its dark, her robe going to mist at its hem, long
/// thin arms, and a crook taller than she is with a lantern that burns
/// cold: raised and flaring as she means to sweep the floor with its light.
fn mist_mother(p: Pose) -> Img {
    let (w, h) = (50, 48);
    let mut im = Img::new(w, h);
    let cx = 20;
    let sway = (p.tick as f32 * 0.06).sin() * 1.5;
    let robe = ['k', 's', 'x', 'S', 'u', 'U', 'v'];
    // The robe: narrow at the shoulders, falling wide, folds down it, its
    // hem going to mist.
    for y in 14..h {
        let f = (y - 14) as f32 / (h - 14) as f32;
        let half = (8.0 + f * 9.0) as i32;
        let drift = (sway * f) as i32;
        for o in -half..=half {
            let u = o as f32 / half as f32;
            let lit = (1.0 - (u + 0.45).abs()).clamp(0.0, 1.0);
            let fold = ((o as f32 / 2.6 + f * 1.5).sin() * 0.5 + 0.5) * 0.9;
            let mut i = (lit * 4.6 + fold) as i32;
            if f > 0.55 && super::super::super::ink::bayer(o + 40, y) < (f - 0.55) * 2.0 {
                continue;
            }
            if f > 0.7 {
                i = i.min(3);
            }
            im.put(cx + o + drift, y, robe[i.clamp(0, 6) as usize]);
        }
    }
    // Wet hair hanging out of the hood over her shoulders.
    for k in [-6, -4, 4, 6] {
        im.line(cx + k, 12, cx + k + k.signum(), 24 + k.abs(), 'S');
    }
    // The hood, peaked, its edge lit.
    for y in 0..16 {
        let f = y as f32 / 15.0;
        let half = (1.0 + f.powf(0.7) * 8.0) as i32;
        let tip = if y < 3 { 1 } else { 0 };
        for o in -half..=half {
            let u = o as f32 / half.max(1) as f32;
            let i = ((1.0 - (u + 0.45).abs()) * 5.0) as i32 + 1;
            im.put(cx + o + tip, y, robe[i.clamp(0, 6) as usize]);
        }
    }
    // The dark inside it, and her eyes.
    im.ellipse(cx as f32 + 0.5, 11.0, 4.0, 4.5, 'k');
    let eye = if p.wind > 0.0 { 'w' } else { 'W' };
    im.put(cx - 1, 10, eye);
    im.put(cx + 2, 10, eye);
    // Her arms: one hanging, long-fingered; one up on the crook.
    im.line(cx - 7, 16, cx - 12, 30, 'U');
    im.line(cx - 12, 30, cx - 14, 34, 'v');
    im.line(cx - 12, 30, cx - 11, 35, 'v');
    let (hx, hy) = lamp_hand(&p);
    im.line(cx + 7, 16, hx, hy, 'U');
    im.line(hx - 1, hy, hx + 1, hy - 1, 'v');
    // The crook and its lantern hung from it.
    let (tx, ty) = (hx + 2, (hy - 18).max(2));
    im.line(hx + 1, (hy + 18).min(h - 1), tx, ty, 'b');
    im.line(hx + 2, (hy + 18).min(h - 1), tx + 1, ty, 'n');
    im.line(tx, ty, tx + 6, ty - 2, 'b');
    im.line(tx + 6, ty - 2, tx + 8, ty + 1, 'b');
    let lit = p.wind > 0.0 || p.rush || p.burn;
    let (lx, ly) = lamp_at(&p);
    // Its cap and ring, its glass in a frame of four bars, and the cold
    // flame in it: a bead at rest, a tall white tongue when it burns.
    im.rect(lx - 2, ly - 6, 5, 1, 'g');
    im.rect(lx - 3, ly - 5, 7, 2, 'K');
    im.line(lx - 3, ly - 5, lx + 3, ly - 5, 'j');
    im.rect(lx - 3, ly - 3, 7, 7, 'k');
    im.rect(lx - 2, ly - 2, 5, 5, if lit { '2' } else { 'x' });
    if lit {
        let tall = if p.burn { 4 } else { 2 + (p.wind * 2.0) as i32 };
        im.rect(lx - 1, ly + 2 - tall, 3, tall, '3');
        im.line(lx, ly + 2 - tall, lx, ly + 1, 'w');
        if p.burn || p.wind > 0.6 {
            im.put(lx, ly + 1 - tall, 'w');
        }
    } else {
        let bob = (p.tick / 9 % 2) as i32;
        im.put(lx, ly + bob, '1');
        im.put(lx, ly + bob - 1, 'q');
    }
    for x in [lx - 3, lx + 3] {
        im.line(x, ly - 3, x, ly + 3, 'g');
    }
    im.line(lx - 3, ly + 4, lx + 3, ly + 4, 'K');
    im.put(lx, ly + 5, 'g');
    im
}

/// The Mist-Mother's hand on her crook, raised as she means to sweep and
/// held up while the light burns.
fn lamp_hand(p: &Pose) -> (i32, i32) {
    let raise = (p.wind * 9.0 + if p.rush { 3.0 } else { 0.0 } + if p.burn { 9.0 } else { 0.0 }).min(9.0) as i32;
    (20 + 12, 26 - raise)
}

/// The middle of the Mist-Mother's lantern glass, on her picture.
fn lamp_at(p: &Pose) -> (i32, i32) {
    let (hx, hy) = lamp_hand(p);
    let (tx, ty) = (hx + 2, (hy - 18).max(2));
    (tx + 8, ty + 8)
}

// ─── the Mist-Mother's light ─────────────────────────────────────────────────

/// Cold lantern-light: white at the flame, through the pale blues.
const LAMPLIGHT: Ramp = Ramp(['w', '3', 'W', 'V', 'z', 'Q']);

/// The Mist-Mother's lantern light down the beam she sweeps with it, if
/// `shot` is her beam. While it warns, the glass fills with light, the
/// mist is drawn in to it and a ghost of the light goes out along the way
/// it will fall. When it burns she throws it from the lantern like a
/// searchlight: a cone of cold light down onto the floor, and the line it
/// lights there is the line that burns, mist rising off it. True when it
/// has drawn the burning beam whole (the warning's marks are still the
/// beam's own).
pub(in super::super) fn lantern_beam(cv: &mut Img, run: &Run, shot: &crate::rules::Projectile, beam: crate::rules::patterns::Beam) -> bool {
    if !shot.hostile {
        return false;
    }
    let Some((e, boss, culture)) = run.enemies.iter().filter(|e| e.hp > 0).find_map(|e| {
        let b = e.boss.and_then(|i| run.bosses.get(usize::from(i)))?;
        let c = dread::culture_of(&b.id)?;
        (matches!(c, Culture::Lampwick | Culture::Chimefolk) && (e.x - shot.x).hypot(e.y - shot.y) < 0.8).then_some((e, b, c))
    }) else {
        return false;
    };
    if culture == Culture::Chimefolk {
        return hum(cv, run, shot, beam);
    }
    let tick = run.tick as u32;
    let p = pose(e, boss, tick);
    let (fx, fy) = at(e.x, e.y);
    let (lx, ly) = lamp_at(&p);
    let lx = if e.dir.0 < -0.2 { 49 - lx } else { lx };
    let lamp = ((fx - 25 + lx) as f32 + 0.5, (fy - 48 + 1 + ly) as f32);
    let age = shot.traits.path.age;
    let (ox, oy) = at(shot.x, shot.y);
    let o = (ox as f32, oy as f32);
    let reach = beam.reach * UNIT;
    let end = (o.0 + beam.angle.cos() * reach, o.1 + beam.angle.sin() * reach);
    let mut pen = Pen::new(cv, 2600);
    if !beam.burning(age) {
        let charge = f32::from(age.min(u16::from(beam.warn))) / f32::from(beam.warn.max(1));
        pen.keep = true;
        cone(&mut pen, lamp, o, end, 0.08 + 0.16 * charge, tick);
        pen.keep = false;
        pen.glow(lamp, 5.0 + charge * 7.0, 1.0, LAMPLIGHT, 0.45 + 0.5 * charge);
        for k in 0..if pen.calm { 6 } else { 12 } {
            let h = hash(k, 13, 9690);
            let life = 14 + h % 10;
            let ph = ((tick + h % life) % life) as f32 / life as f32;
            let a = (h % 628) as f32 / 100.0;
            let r = (26.0 + (h >> 9) as f32 % 14.0) * (1.0 - ph);
            pen.streak((lamp.0 + a.cos() * r, lamp.1 + a.sin() * r * 0.8), (-a.cos(), -a.sin() * 0.8), 2.0 + charge * 3.0, LAMPLIGHT.cooler(1), ph);
        }
        return false;
    }
    pen.keep = true;
    cone(&mut pen, lamp, o, end, 0.42, tick);
    pen.keep = false;
    stripe(&mut pen, o, end, tick);
    pen.glow(lamp, 13.0, 1.0, LAMPLIGHT, 0.95);
    pen.twinkle(lamp, 4.0, ((tick % 8) as f32 + 2.0) / 12.0, LAMPLIGHT);
    true
}

/// The Drowned Toll's hum, when its beam burns: the note it rings going
/// out down the line in bowed rings of bronze, one after another, over a
/// line of it humming on the floor, and the floor ringing round it. True:
/// the burning beam is drawn whole (the warning's marks are the beam's).
fn hum(cv: &mut Img, run: &Run, shot: &crate::rules::Projectile, beam: crate::rules::patterns::Beam) -> bool {
    if !beam.burning(shot.traits.path.age) {
        return false;
    }
    let tick = run.tick as u32;
    let (ox, oy) = at(shot.x, shot.y);
    let o = (ox as f32, oy as f32);
    let reach = beam.reach * UNIT;
    let (ux, uy) = (beam.angle.cos(), beam.angle.sin());
    let (nx, ny) = (-uy, ux);
    let mut pen = Pen::new(cv, 2000);
    for s in 0..reach as i32 {
        let (x, y) = (o.0 + ux * s as f32, o.1 + uy * s as f32);
        let lit = (s + tick as i32 * 2) % 9 < 3;
        for (k, ink) in [(-1i32, 'O'), (0, if lit { 'T' } else { 't' }), (1, 'O')] {
            pen.put((x + nx * k as f32).round() as i32, (y + ny * k as f32).round() as i32, ink);
        }
    }
    // The rings of its note, running out along it, bowed the way they go.
    let gap = 11.0;
    let run_on = (tick as f32 * 2.2) % gap;
    let mut d = 6.0 + run_on;
    while d < reach - 2.0 {
        let fade = d / reach;
        let w = 3.0 + 2.0 * (1.0 - fade);
        for j in -(w as i32)..=(w as i32) {
            let f = j as f32 / w;
            let bow = (1.0 - f * f) * 2.5;
            let (x, y) = (o.0 + ux * (d + bow) + nx * j as f32, o.1 + uy * (d + bow) + ny * j as f32);
            pen.light(x.round() as i32, y.round() as i32, ramps::GOLD.at(0.15 + fade * 0.6 + f.abs() * 0.2));
        }
        d += gap;
    }
    let ph = (tick % 10) as f32 / 10.0;
    pen.ring((o.0, o.1 + 2.0), 6.0 + ph * 14.0, 1.0, 0.45, ramps::GOLD, ph);
    true
}

/// A cone of cold light from the lantern `l` down onto the floor line
/// `a` to `b`: faint at the glass, fuller where it lands, banded by the
/// mist drifting through it, and only on bare floor (`Pen::keep`).
fn cone(pen: &mut Pen, l: (f32, f32), a: (f32, f32), b: (f32, f32), power: f32, tick: u32) {
    let power = if pen.calm { power * 0.6 } else { power };
    let x0 = l.0.min(a.0).min(b.0).floor() as i32;
    let x1 = l.0.max(a.0).max(b.0).ceil() as i32;
    let y0 = l.1.min(a.1).min(b.1).floor() as i32;
    let y1 = l.1.max(a.1).max(b.1).ceil() as i32;
    let det = (a.1 - b.1) * (l.0 - b.0) + (b.0 - a.0) * (l.1 - b.1);
    if det.abs() < 1.0 {
        return;
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px_, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let wl = ((a.1 - b.1) * (px_ - b.0) + (b.0 - a.0) * (py - b.1)) / det;
            let wa = ((b.1 - l.1) * (px_ - b.0) + (l.0 - b.0) * (py - b.1)) / det;
            let wb = 1.0 - wl - wa;
            if wl < 0.0 || wa < 0.0 || wb < 0.0 {
                continue;
            }
            let down = 1.0 - wl;
            let along = wb / (wa + wb).max(1e-3);
            let band = 0.5 + 0.5 * (along * 18.0 - tick as f32 * 0.2 + down * 3.0).sin();
            let i = power * (0.3 + 0.7 * down) * (1.0 - 0.55 * along) * (0.75 + 0.35 * band);
            if super::super::super::ink::bayer(x, y) < i {
                pen.light(x, y, LAMPLIGHT.dithered((1.0 - i * 1.6).clamp(0.2, 1.0), x, y));
            }
        }
    }
}

/// The lit line on the floor, where her light burns: a white core in a
/// pale rim, shimmering, mist rising off it.
fn stripe(pen: &mut Pen, a: (f32, f32), b: (f32, f32), tick: u32) {
    let len = (b.0 - a.0).hypot(b.1 - a.1).max(1.0);
    let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    let (nx, ny) = (-uy, ux);
    for s in 0..len as i32 {
        let (x, y) = (a.0 + ux * s as f32, a.1 + uy * s as f32);
        let flick = (s + tick as i32) % 7 == 0;
        for (k, ink) in [(-2i32, 'V'), (-1, '3'), (0, if flick { '3' } else { 'w' }), (1, '3'), (2, 'V')] {
            if k.abs() == 2 && (s + k) % 2 == 0 {
                continue;
            }
            pen.put((x + nx * k as f32).round() as i32, (y + ny * k as f32).round() as i32, ink);
        }
    }
    // Mist lifting off the lit floor.
    for k in 0..if pen.calm { 8 } else { 16 } {
        let h = hash(k, 17, 9691);
        let life = 16 + h % 10;
        let ph = ((tick + h % life) % life) as f32 / life as f32;
        let s = (h >> 8) as f32 % len;
        let (x, y) = (a.0 + ux * s, a.1 + uy * s - ph * 9.0);
        pen.light(x.round() as i32 + ((ph * 6.0).sin() * 1.5) as i32, y.round() as i32, LAMPLIGHT.at(0.35 + ph * 0.6));
    }
}

// ─── the Roostfolk's dread ───────────────────────────────────────────────────

/// The Snare-Queen: a spider the size of a nest, her abdomen marked with
/// a bat's skull in red, a cluster of red eyes, eight long jointed legs,
/// and her thread going up from her into the dark, taut as she gathers
/// herself to drop.
fn snare_queen(p: Pose) -> Img {
    let (w, h) = (68, 50);
    let mut im = Img::new(w, h);
    let cx = 34.0;
    let gather = p.wind;
    // Her thread, up off the top of the picture.
    im.line(cx as i32, 0, cx as i32, 12, if gather > 0.0 { 'H' } else { 'i' });
    // The legs: four a side, fanned, up to the knee then down to the
    // floor, drawn in as she gathers herself.
    let knees = [(13.0, 12.0), (19.0, 16.0), (20.0, 23.0), (15.0, 29.0)];
    let feet = [(22.0, 33.0), (31.0, 42.0), (30.0, 47.0), (22.0, 48.0)];
    for side in [-1.0f32, 1.0] {
        for k in 0..4 {
            let pull = 1.0 - gather * 0.3;
            let step = ((p.tick as f32 * 0.25 + k as f32 * 1.3 + side).sin() * 1.5) * (1.0 - gather);
            let root = (cx + side * 4.0, 31.0 + k as f32 * 1.5);
            let knee = (cx + side * knees[k].0 * pull, knees[k].1 - gather * 5.0 + step);
            let foot = (cx + side * feet[k].0 * pull, feet[k].1);
            for (a, b) in [(root, knee), (knee, foot)] {
                im.line(a.0 as i32, a.1 as i32, b.0 as i32, b.1 as i32, 'K');
                im.line(a.0 as i32 + side as i32, a.1 as i32 - 1, b.0 as i32 + side as i32, b.1 as i32 - 1, if side < 0.0 { 'g' } else { 'X' });
            }
            im.put(knee.0 as i32, knee.1 as i32 - 1, 'j');
            im.put(knee.0 as i32, knee.1 as i32 - 2, 'K');
            im.put(foot.0 as i32, foot.1 as i32, 'k');
        }
    }
    // Her abdomen, big and round, the bat's skull on it.
    ball(&mut im, (cx, 21.0), (13.0, 12.0), &['k', 'K', 'X', 'g', 'j', 'G'], 9641);
    let (sx, sy) = (cx as i32, 19);
    im.ellipse(sx as f32, sy as f32, 4.5, 3.5, '8');
    im.ellipse(sx as f32, sy as f32, 3.5, 2.5, '7');
    for side in [-1, 1] {
        im.line(sx + side * 3, sy - 2, sx + side * 7, sy - 6, '7');
        im.line(sx + side * 3, sy + 1, sx + side * 7, sy + 4, '8');
        im.put(sx + side * 2, sy, 'k');
    }
    im.rect(sx - 1, sy + 3, 3, 3, '7');
    // Her head and its cluster of red eyes.
    ball(&mut im, (cx, 34.0), (7.0, 5.5), &['k', 'K', 'X', 'g', 'j'], 9642);
    for (k, (ex, ey)) in [(-3, 32), (3, 32), (-1, 31), (1, 31), (-4, 34), (4, 34)].into_iter().enumerate() {
        let lit = p.wind > 0.0 || k < 2 || (p.tick / 20 + k as u32) % 5 != 0;
        im.put(cx as i32 + ex, ey, if lit { '7' } else { '8' });
    }
    if p.wind > 0.5 {
        im.put(cx as i32 - 3, 31, 'c');
        im.put(cx as i32 + 3, 31, 'c');
    }
    // Fangs.
    im.line(cx as i32 - 2, 38, cx as i32 - 3, 42, 'H');
    im.line(cx as i32 + 2, 38, cx as i32 + 3, 42, 'H');
    im
}

// ─── the Rootkin's dread ─────────────────────────────────────────────────────

/// The Blight: a beetle of rot as long as a cart, its shell of rotten bark
/// grown over with fungus, pale rot spots on it, sickly eyes, mandibles,
/// and black sap leaking from its joints. It rears before it slams.
fn blight(p: Pose) -> Img {
    let (w, h) = (66, 50);
    let mut im = Img::new(w, h);
    let rear = p.wind;
    let (cx, cy) = (34.0, 30.0 - rear * 6.0);
    // Six legs.
    for side in [-1.0f32, 1.0] {
        for k in 0..3 {
            let x0 = cx - 10.0 + k as f32 * 9.0;
            let step = ((p.tick as f32 * 0.2 + k as f32 * 2.0 + side).sin() * 2.0) * (1.0 - rear);
            let knee = (x0 + side * 0.0 + step, cy + 8.0 + side * 2.0);
            let foot = (x0 - 4.0 + step * 1.5, 48.0 - if side > 0.0 { 0.0 } else { 1.0 });
            im.line(x0 as i32, (cy + 4.0) as i32, knee.0 as i32, knee.1 as i32, 'n');
            im.line(knee.0 as i32, knee.1 as i32, foot.0 as i32, foot.1 as i32, 'b');
        }
    }
    // The abdomen behind, segmented, and the shell over it.
    for k in 0..4 {
        let x = cx + 10.0 + k as f32 * 5.0;
        ball(&mut im, (x, cy + 4.0 + k as f32), (6.0 - k as f32, 6.0 - k as f32 * 0.8), &['n', 'b', 'I', 'B', 'p'], 9651 + k);
    }
    ball(&mut im, (cx + 2.0, cy), (19.0, 12.0 + rear * 2.0), &['n', 'b', 'I', 'B', 'p', 'P'], 9652);
    // The seam down the shell, and fungus and rot over it.
    im.line(cx as i32 - 14, cy as i32 - 1, cx as i32 + 18, cy as i32 - 3, 'n');
    for k in 0..22 {
        let hk = hash(k, 3, 9653);
        let (fx, fy) = (cx - 16.0 + (hk % 34) as f32, cy - 10.0 + ((hk >> 8) % 14) as f32);
        let (dx, dy) = ((fx - cx - 2.0) / 19.0, (fy - cy) / 12.0);
        if dx * dx + dy * dy > 0.85 {
            continue;
        }
        match k % 4 {
            0 => {
                // A shelf of fungus.
                im.ellipse(fx, fy, 3.0, 1.5, 'E');
                im.line(fx as i32 - 2, fy as i32 - 1, fx as i32 + 2, fy as i32 - 1, 'A');
            }
            1 => im.put(fx as i32, fy as i32, 'h'),
            2 => {
                im.put(fx as i32, fy as i32, 'D');
                im.put(fx as i32 + 1, fy as i32, 'l');
            }
            _ => im.put(fx as i32, fy as i32, 'J'),
        }
    }
    // Its head, eyes and mandibles, opening as it rears.
    let (hx, hy) = (cx - 18.0, cy + 3.0 - rear * 2.0);
    ball(&mut im, (hx, hy), (7.0, 6.0), &['n', 'b', 'I', 'B'], 9654);
    im.put(hx as i32 - 2, hy as i32 - 2, 'Y');
    im.put(hx as i32 - 3, hy as i32 - 2, 'y');
    im.put(hx as i32 + 2, hy as i32 - 3, 'Y');
    let gape = 2.0 + rear * 5.0;
    for side in [-1.0f32, 1.0] {
        let base = (hx - 5.0, hy + side * 2.0);
        let tip = (hx - 12.0, hy + side * gape);
        im.line(base.0 as i32, base.1 as i32, tip.0 as i32, tip.1 as i32, 'O');
        im.line(tip.0 as i32, tip.1 as i32, tip.0 as i32 + 2, (tip.1 - side * 2.0) as i32, 'T');
    }
    // Black sap from its joints, dripping.
    for k in 0..4 {
        let t = ((p.tick + k * 17) % 34) as i32;
        let x = cx as i32 - 12 + k as i32 * 8;
        im.put(x, cy as i32 + 10 + t / 6, 'f');
        im.put(x, cy as i32 + 9 + t / 6, 'D');
    }
    im
}

// ─── the Chimefolk's dread ───────────────────────────────────────────────────

/// The Drowned Toll: a giant in waterlogged robes hung with chain, a
/// cracked bronze bell for its head with the dark under its lip and two
/// pale eyes in it, and the bell's own clapper for a hammer: lifted over
/// its head as it means to strike itself.
fn drowned_toll(p: Pose) -> Img {
    let (w, h) = (56, 50);
    let mut im = Img::new(w, h);
    let cx = 26.0;
    let lift = p.wind;
    // The robe: heavy and dripping, hung with chain.
    for y in 20..48 {
        let f = (y - 20) as f32 / 28.0;
        let half = (9.0 + f * 9.0) as i32;
        for o in -half..=half {
            let u = o as f32 / half as f32;
            let lit = (1.0 - (u + 0.4).abs()).clamp(0.0, 1.0);
            let fold = ((o as f32 / 2.5).sin() * 0.5 + 0.5) * 0.5;
            let i = (lit * 4.0 + fold) as i32;
            let ink = ['k', 's', 'x', 'S', 'u'][i.clamp(0, 4) as usize];
            if y > 45 && hash(o, y, 9661) % 2 == 0 {
                continue;
            }
            im.put(cx as i32 + o, y, ink);
        }
    }
    for k in 0..3 {
        let y0 = 26 + k * 7;
        for o in -10..=10 {
            let yy = y0 + (o * o) / 30;
            im.put(cx as i32 + o, yy, if o % 2 == 0 { 'G' } else { 'J' });
        }
    }
    // Drips off the hem.
    for k in 0..4 {
        let t = ((p.tick + k * 11) % 22) as i32;
        im.put(cx as i32 - 12 + k as i32 * 8, 47 + t / 8, 'z');
    }
    // The bell for a head: cracked, verdigris, lit on the left.
    let (bx, by) = (cx as i32, 2);
    let tall = 16;
    for d in 0..tall {
        let f = d as f32 / tall as f32;
        let half = (10.0 * (0.45 + 0.25 * f + 0.45 * f.powi(4))) as i32;
        for o in -half..=half {
            let u = o as f32 / half.max(1) as f32;
            let i = ((1.0 - (u + 0.35).abs()) * 6.4) as i32;
            let rust = vnoise((bx + o) as f32 / 2.5, d as f32 / 2.5, 9662) > 0.66;
            let ink = if rust { ['e', 'C', 'y'][((d + o).unsigned_abs() % 3) as usize] } else { ['n', 'b', 'I', 'R', 'o', 'O', 't'][i.clamp(0, 6) as usize] };
            im.put(bx + o, by + d, ink);
        }
    }
    // The crack down it.
    for (a, b) in [((bx + 2, by + 1), (bx - 1, by + 6)), ((bx - 1, by + 6), (bx + 3, by + 10)), ((bx + 3, by + 10), (bx + 1, by + 15))] {
        im.line(a.0, a.1, b.0, b.1, 'k');
    }
    // The dark under its lip, and the eyes in it.
    im.rect(bx - 9, by + tall, 19, 3, 'k');
    let eye = if lift > 0.0 { '3' } else { '2' };
    im.put(bx - 4, by + tall + 1, eye);
    im.put(bx + 4, by + tall + 1, eye);
    // The hammer: the bell's clapper on a haft, up over its head to strike.
    let a = PI * (0.9 - lift * 0.75);
    let (sx, sy) = (cx + 14.0, 24.0);
    let len = 20.0;
    let (ex, ey) = (sx + a.cos() * len * 0.6, (sy - a.sin() * len).max(3.0));
    im.line(cx as i32 + 8, 22, sx as i32, sy as i32, 'u');
    im.line(sx as i32, sy as i32, ex as i32, ey as i32, 'b');
    im.line(sx as i32 + 1, sy as i32, ex as i32 + 1, ey as i32, 'P');
    im.ellipse(ex, ey, 4.5, 4.0, 'G');
    im.ellipse(ex - 1.0, ey - 1.0, 2.5, 2.0, 'J');
    im.put(ex as i32 - 2, ey as i32 - 2, 'h');
    im
}

// ─── the Saltkin's dread ─────────────────────────────────────────────────────

/// The Brine-Wyrm: a lamprey of the brine as thick as a barrel, rearing
/// out of its own coils, its hide ringed and crusted with salt in cubes,
/// a round mouth of white teeth, brine dripping from it; it draws its head
/// down and back before it charges.
fn brine_wyrm(p: Pose) -> Img {
    let (w, h) = (68, 54);
    let mut im = Img::new(w, h);
    let coil = p.wind;
    // The coils on the floor behind it.
    for k in 0..3 {
        let (ccx, ccy) = (40.0 + k as f32 * 6.0, 44.0 - k as f32 * 2.0);
        let (rx, ry) = (14.0 - k as f32 * 2.0 - coil * 2.0, 6.0 - k as f32);
        for (x, y) in round_pts(ccx, ccy, rx, ry, 60) {
            for t in -2..=2 {
                let ink = if t < 0 { 'O' } else if t == 0 { 'o' } else { 'r' };
                im.put(x, y + t, ink);
            }
        }
    }
    // The neck rising out of the coils toward its head.
    let (hx, hy) = (20.0 + coil * 6.0, 14.0 + coil * 10.0);
    let segs = 14;
    for k in 0..=segs {
        let f = k as f32 / segs as f32;
        let (x, y) = (34.0 + (hx - 34.0) * f + (f * PI).sin() * 6.0, 40.0 + (hy - 40.0) * f);
        let r = 7.5 - f * 1.5;
        im.ellipse(x, y, r, r, 'r');
        im.ellipse(x - 1.0, y - 1.0, r - 1.2, r - 1.2, 'o');
        im.ellipse(x - 2.0, y - 2.0, r * 0.45, r * 0.45, 'O');
        // A ring between segments, and salt crusted white on its back.
        if k % 2 == 0 {
            im.line((x - r) as i32, y as i32, (x + r) as i32, y as i32, 'P');
        }
        for c in 0..6 {
            let a = -0.4 + c as f32 * 0.35;
            let (sx, sy) = (x + a.cos() * (r - 1.0), y - a.sin().abs() * (r - 1.0) - 1.0);
            if hash(k, c, 9681) % 3 != 0 {
                im.put(sx as i32, sy as i32, if c % 2 == 0 { 'H' } else { 'i' });
            }
        }
        if k % 3 == 1 {
            super::salt::cube(&mut im, (x + r * 0.5) as i32, (y - r * 0.4) as i32, 5 + (k % 2) as i32, false);
        }
    }
    // Its head: a round lamprey mouth of white teeth.
    let open = 4.0 + coil * 2.0 + if p.rush { 2.0 } else { 0.0 };
    im.ellipse(hx, hy, open + 5.0, open + 4.0, 'r');
    im.ellipse(hx - 1.0, hy - 1.0, open + 3.5, open + 2.5, 'o');
    im.ellipse(hx, hy, open + 1.0, open, 'k');
    im.ellipse(hx, hy, open * 0.5, open * 0.45, '8');
    for (tx, ty) in round_pts(hx, hy, open + 0.5, open - 0.2, 12) {
        im.put(tx, ty, 'H');
    }
    for (tx, ty) in round_pts(hx, hy, open * 0.7, open * 0.6, 8) {
        im.put(tx, ty, 'i');
    }
    // Brine dripping from it.
    for k in 0..3 {
        let t = ((p.tick + k * 9) % 24) as i32;
        im.put(hx as i32 - 3 + k as i32 * 3, (hy + open + 1.0) as i32 + t / 4, if t < 12 { 'z' } else { 'Q' });
    }
    im
}

// ─── the floor's warnings, in each people's way ──────────────────────────────

/// Under each dread winding up: what is coming, drawn in its people's way
/// on the floor (under the figures). And after a blow, what it leaves.
pub(in super::super) fn under(cv: &mut Img, run: &Run) {
    for e in run.enemies.iter().filter(|e| e.hp > 0) {
        let Some(boss) = e.boss.and_then(|i| run.bosses.get(usize::from(i))).filter(|b| dread::is(b)) else {
            continue;
        };
        let Some(culture) = dread::culture_of(&boss.id) else { continue };
        let Some(up) = fight::winding(e, boss) else { continue };
        let t = if e.stage == fight::WIND { (e.timer as f32 / up.wind.max(1) as f32).min(1.0) } else { 1.0 };
        let striking = e.stage != fight::WIND;
        // How far into standing spent it is: what its blow left fades.
        let after = if e.stage == fight::DAZED { (e.timer as f32 / fight::RECOVER as f32).min(1.0) } else { 0.0 };
        let (x, y) = at(e.x, e.y);
        let tick = run.tick as u32;
        let mut pen = Pen::new(cv, 2400);
        match up.kind {
            Windup::Inhale => inhale_marks(&mut pen, culture, (x, y), t, striking, tick),
            Windup::Charge => {
                if let Some(r) = e.signature.rails.first() {
                    lane_marks(&mut pen, culture, at(r.a.0, r.a.1), at(r.b.0, r.b.1), t, striking, tick);
                }
            }
            Windup::Leap => {
                let m = at(e.signature.mark.0, e.signature.mark.1);
                landing_marks(&mut pen, culture, m, t, e.stage, tick);
            }
            Windup::Slam => slam_marks(&mut pen, culture, (x, y), t, striking, after, tick),
            Windup::Beam => beam_marks(&mut pen, culture, (x, y), e.dir, up.arc, t, striking, tick),
        }
    }
}

/// The light of the room drawn in: motes streaming in to the dread from
/// all round, faster and thicker to the last; then breathed back out.
fn inhale_marks(pen: &mut Pen, culture: Culture, (x, y): (i32, i32), t: f32, out: bool, tick: u32) {
    let ramp = match culture {
        Culture::Emberfolk => ramps::FIRE,
        Culture::Saltkin => ramps::ROSE,
        _ => ramps::GENIE,
    };
    let n = 36;
    for k in 0..n {
        let h = hash(k, 7, 9670);
        let a = (h % 628) as f32 / 100.0;
        let life = 20 + h % 12;
        let phase = ((tick + h % life) % life) as f32 / life as f32;
        let reach = 30.0 + (h >> 10) as f32 % 60.0;
        let d = if out { reach * phase } else { reach * (1.0 - phase) * (1.0 - t * 0.3) };
        let p = (x as f32 + a.cos() * d, y as f32 - 14.0 + a.sin() * d * 0.6);
        let dir = if out { (a.cos(), a.sin() * 0.6) } else { (-a.cos(), -a.sin() * 0.6) };
        pen.streak(p, dir, 3.0 + t * 4.0, ramp, phase);
    }
    if !out {
        pen.ring((x as f32, y as f32 + 2.0), 48.0 * (1.0 - t) + 10.0, 1.5, 0.45, ramp, 0.4);
    }
}

/// A charge's lane in its people's way: crystal shards (or salt, or
/// splinters) breaking up out of the floor along it one after another as
/// the wind-up goes on.
fn lane_marks(pen: &mut Pen, culture: Culture, a: (i32, i32), b: (i32, i32), t: f32, dashing: bool, tick: u32) {
    let len = ((b.0 - a.0) as f32).hypot((b.1 - a.1) as f32).max(1.0);
    let n = (len / 9.0) as i32;
    let (ux, uy) = ((b.0 - a.0) as f32 / len, (b.1 - a.1) as f32 / len);
    for k in 1..=n {
        let f = k as f32 / n as f32;
        if f > t * 1.15 {
            break;
        }
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let (px_, py) = (a.0 as f32 + ux * f * len - uy * side * 9.0, a.1 as f32 + uy * f * len + ux * side * 9.0 + 6.0);
        let grow = ((t * 1.15 - f) * 6.0).min(1.0);
        let tall = (3.0 + grow * 6.0) as i32;
        match culture {
            Culture::Saltkin => {
                for d in 0..tall {
                    let half = (tall - d) / 3;
                    for o in -half..=half {
                        pen.put(px_ as i32 + o, py as i32 - d, if o < 0 { 'W' } else { 'h' });
                    }
                }
            }
            Culture::Lampwick => {
                // Mist rising off the lane.
                for d in 0..tall {
                    if hash(k, d, tick / 3) % 2 == 0 {
                        pen.light(px_ as i32 + (d % 3) - 1, py as i32 - d * 2, 'u');
                    }
                }
            }
            _ => {
                for d in 0..tall {
                    let half = ((tall - d) / 3).max(0);
                    for o in -half..=half {
                        pen.put(px_ as i32 + o, py as i32 - d, if o < 0 { 'z' } else if o == 0 { 'V' } else { 'Q' });
                    }
                }
                if grow >= 1.0 && (tick / 3 + k as u32) % 6 == 0 {
                    pen.twinkle((px_, py - tall as f32), 2.0, 0.5, ramps::FROST);
                }
            }
        }
        // Cracks running between the shards.
        pen.put(px_ as i32 + (uy * side * 4.0) as i32, py as i32 - (ux * side * 4.0) as i32 + 1, 'k');
    }
    if dashing {
        // What it leaves behind: a trail of grit where it went.
        for k in 0..n {
            let f = k as f32 / n.max(1) as f32;
            let (px_, py) = (a.0 as f32 + ux * f * len, a.1 as f32 + uy * f * len + 6.0);
            pen.put(px_ as i32, py as i32, if culture == Culture::Saltkin { 'H' } else { 'u' });
        }
    }
}

/// Where a leap will land, in its people's way: cinders falling on a ring
/// of heat for the Coal-Glutton, a web spun on the floor and the thread
/// down to it for the Snare-Queen, spores for the others.
fn landing_marks(pen: &mut Pen, culture: Culture, (x, y): (i32, i32), t: f32, stage: u8, tick: u32) {
    let r = (fight::LANDING * UNIT) as f32;
    match culture {
        Culture::Roostfolk => {
            // The web: spokes out from where she will land, and its rings
            // spun on them one by one.
            let spokes = 10;
            for k in 0..spokes {
                let a = k as f32 / spokes as f32 * TAU + 0.2;
                let reach = r * (0.3 + t.min(1.0) * 0.9);
                pen.line((x as f32, y as f32), (x as f32 + a.cos() * reach, y as f32 + a.sin() * reach * 0.5), 'i');
            }
            let rings = (t * 5.0) as i32;
            for ring in 1..=rings {
                let rr = r * ring as f32 / 5.0 * 1.1;
                for k in 0..spokes {
                    let (a0, a1) = (k as f32 / spokes as f32 * TAU + 0.2, (k + 1) as f32 / spokes as f32 * TAU + 0.2);
                    pen.line((x as f32 + a0.cos() * rr, y as f32 + a0.sin() * rr * 0.5), (x as f32 + a1.cos() * rr, y as f32 + a1.sin() * rr * 0.5), 'h');
                }
            }
            if stage == fight::WIND {
                // Her thread, down out of the dark to the web's middle.
                for yy in (y - 220).max(0)..y {
                    if (yy + tick as i32) % 5 != 0 {
                        pen.light(x, yy, 'i');
                    }
                }
            }
        }
        Culture::Emberfolk => {
            pen.ring((x as f32, y as f32), r * (0.6 + t * 0.5), 2.0, 0.5, ramps::FIRE, 0.5 - t * 0.4);
            pen.glow((x as f32, y as f32), r * t, 0.5, ramps::EMBER, t * 0.6);
            for k in 0..16 {
                let h = hash(k, 9, 9671);
                let life = 18 + h % 10;
                let ph = ((tick + h % life) % life) as f32 / life as f32;
                let (cx, cy) = (x as f32 + ((h % 40) as f32 - 20.0), y as f32 - 60.0 + ph * 60.0 + ((h >> 8) % 8) as f32);
                pen.streak((cx, cy), (0.2, 1.0), 4.0, ramps::EMBER, ph);
            }
        }
        _ => {
            pen.ring((x as f32, y as f32), r * (1.2 - t * 0.4), 1.5, 0.5, ramps::POISON, 0.3);
            for k in 0..12 {
                let h = hash(k, 10, 9672);
                let ph = ((tick + h % 24) % 24) as f32 / 24.0;
                pen.twinkle((x as f32 + (h % 30) as f32 - 15.0, y as f32 - ph * 20.0), 1.5, ph, ramps::GENIE);
            }
        }
    }
}

/// A slam in its people's way: rot creeping out over the floor round the
/// Blight, the floor ringing round the Toll like the bell it is, cracks
/// for the others.
fn slam_marks(pen: &mut Pen, culture: Culture, (x, y): (i32, i32), t: f32, struck: bool, after: f32, tick: u32) {
    match culture {
        Culture::Rootkin => {
            // Tendrils of rot, branching out over the floor.
            for k in 0..9 {
                let a = k as f32 / 9.0 * TAU + 0.3;
                let reach = 16.0 + t * 46.0;
                let mut p = (x as f32, y as f32);
                let mut ang = a;
                let steps = reach as i32;
                for s in 0..steps {
                    ang += (vnoise(s as f32 / 6.0, k as f32, 9673) - 0.5) * 0.35;
                    p = (p.0 + ang.cos(), p.1 + ang.sin() * 0.5);
                    pen.put(p.0 as i32, p.1 as i32, if s % 7 == 3 { 'l' } else { 'f' });
                    if s % 9 == 5 {
                        pen.put(p.0 as i32, p.1 as i32 - 1, 'D');
                    }
                }
                if t > 0.5 {
                    pen.light(p.0 as i32, p.1 as i32 - 1, 'Y');
                }
            }
            if struck {
                pen.glow((x as f32, y as f32), 40.0, 0.5, ramps::POISON, 0.5 * (1.0 - after));
            }
        }
        Culture::Chimefolk => {
            // The floor rings: arcs going out, quicker to the last.
            let n = 3;
            for k in 0..n {
                let ph = ((tick as f32 / (8.0 - t * 4.0)) + k as f32 / n as f32).fract();
                let rr = 10.0 + ph * (30.0 + t * 30.0);
                pen.ring((x as f32, y as f32), rr, 1.0, 0.45, ramps::GENIE, ph * 0.8);
            }
            if struck && after < 1.0 {
                pen.shock((x as f32, y as f32), 30.0 + after * 50.0, 3.0, 0.45, ramps::GOLD, 0.2 + after * 0.8);
            }
        }
        _ => {
            for k in 0..8 {
                let a = k as f32 / 8.0 * TAU;
                let reach = 10.0 + t * 30.0;
                pen.line((x as f32, y as f32), (x as f32 + a.cos() * reach, y as f32 + a.sin() * reach * 0.5), 'k');
            }
        }
    }
}

/// A beam in its people's way: mist gathering at the Mist-Mother's feet
/// and a cold glow down the way her light will go; for the Toll a hum.
fn beam_marks(pen: &mut Pen, culture: Culture, (x, y): (i32, i32), dir: (f32, f32), arc: f32, t: f32, sweeping: bool, tick: u32) {
    let a0 = dir.1.atan2(dir.0);
    let half = arc.to_radians() / 2.0;
    let ramp = if culture == Culture::Lampwick { ramps::FROST } else { ramps::GENIE };
    // The swept arc, shown faintly on the floor.
    let reach = 40.0 + t * 60.0;
    for k in 0..=12 {
        let a = a0 - half + k as f32 / 12.0 * half * 2.0;
        let p = (x as f32 + a.cos() * reach, y as f32 + a.sin() * reach * 0.6);
        pen.light(p.0 as i32, p.1 as i32, ramp.step(3));
    }
    // Mist gathering round her.
    for k in 0..30 {
        let h = hash(k, 11, 9674);
        let ph = ((tick + h % 40) % 40) as f32 / 40.0;
        let a = (h % 628) as f32 / 100.0 + ph;
        let d = 22.0 * (1.0 - t * 0.5) + (h >> 10) as f32 % 10.0;
        pen.under((x as f32 + a.cos() * d) as i32, (y as f32 + a.sin() * d * 0.4) as i32, if k % 3 == 0 { 'u' } else { 'S' });
    }
    if !sweeping {
        pen.glow((x as f32, y as f32 - 20.0), 10.0 + t * 12.0, 1.0, ramp, t);
    }
}
