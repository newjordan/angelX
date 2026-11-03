//! The cavern's dark overhead and what moves in it: the back wall with its
//! stalactites, the glowworms that are the deep people's stars, each
//! people's own weather (spores drifting, a waterfall and its glints,
//! smoke and the slow fire, mist on the lake), and the falling star the
//! party arrives as: a streak down out of the dark, a burst where it lands.

use super::super::super::ink::{Img, hash, vnoise};
use super::super::super::light::Light;
use super::super::TILE;
use super::{Look, px};
use crate::rules::{
    Run,
    deep::{Culture, Village, village as v},
};

/// The back wall: the rock face the village stands against, layered and
/// lit from below, and the stalactites hanging from the roof of the dark.
pub(super) fn back_wall(cv: &mut Img, vl: &Village, lk: &Look, lights: &mut Vec<Light>) {
    let w = cv.w;
    let face = v::BACK as i32 * TILE;
    for x in 0..w {
        // The face's foot wanders, and the dark above it goes up forever.
        let foot = face + 2 + (vnoise(x as f32 / 13.0, 1.0, 9201) * 6.0) as i32;
        let crest = 40 + (vnoise(x as f32 / 29.0, 2.0, 9202) * 30.0) as i32;
        for y in 0..foot {
            if y < crest {
                // The dark itself: black paper, a faint grain far up.
                if hash(x, y, 9203) % 61 == 0 {
                    cv.put(x, y, 'K');
                }
                continue;
            }
            let strata = vnoise(x as f32 / 30.0, y as f32 / 3.5, 9204);
            let grain = vnoise(x as f32 / 4.0, y as f32 / 4.0, 9205);
            let down = (y - crest) as f32 / (foot - crest).max(1) as f32;
            let t = 0.15 + down * 0.45 + (strata - 0.5) * 0.5 + (grain - 0.5) * 0.25;
            let i = ((t * lk.rock.len() as f32) as i32).clamp(0, lk.rock.len() as i32 - 1) as usize;
            cv.put(x, y, lk.rock[i]);
            // Cracks in the face.
            if (strata * 40.0).fract() < 0.05 {
                cv.put(x, y, lk.rock[0]);
            }
        }
    }
    // Stalactites, long and short, from the roof of the dark.
    for k in 0..34 {
        let h = hash(k, 1, 9206);
        let x = (h % w as u32) as i32;
        let len = 12 + (h >> 9) as i32 % 48;
        let wide = 2 + (h >> 17) as i32 % 4;
        for d in 0..len {
            let half = (wide as f32 * (1.0 - d as f32 / len as f32)).ceil() as i32;
            for o in -half..=half {
                let i = if o < 0 { 2 } else if o == 0 { 3 } else { 1 };
                cv.put(x + o, d, lk.rock[i]);
            }
        }
        cv.put(x, len, lk.rock[4]);
    }
    match vl.culture {
        Culture::Capfolk => giant_caps(cv, lk),
        Culture::Glimmerkin => crystal_seams(cv, lights),
        Culture::Emberfolk => lava_falls(cv, lights),
        Culture::Lampwick => lake(cv),
        Culture::Roostfolk => super::roost::back(cv, vl, lights),
        Culture::Rootkin => super::roots::back(cv, vl, lights),
        Culture::Chimefolk => super::bells::back(cv, vl, lights),
        Culture::Saltkin => super::salt::back(cv, vl, lights),
    }
}

/// Dwellings cut into the back wall above the village, its far windows
/// lit one by one as hope grows, ledges and ladders between them: the
/// cavern is deep, and lived in all the way up.
pub(super) fn far_dwellings(cv: &mut Img, vl: &Village, lk: &Look, lights: &mut Vec<Light>) {
    // The roost, the cathedral and the salt terraces keep their own up
    // there: nests, arches and salt-lamps.
    if matches!(vl.culture, Culture::Roostfolk | Culture::Chimefolk | Culture::Saltkin) {
        return;
    }
    let face = v::BACK as i32 * TILE;
    let rootkin = vl.culture == Culture::Rootkin;
    for k in 0..16 {
        let h = hash(k, 8, 9280);
        let x = 20 + (h % 536) as i32;
        let y = face - 12 - ((h >> 10) % 30) as i32;
        let lit = (k as u32 % 8) < u32::from(vl.hope);
        // A ledge under it, and now and then a ladder down.
        cv.line(x - 7, y + 6, x + 8, y + 6, lk.rock[4]);
        cv.line(x - 7, y + 7, x + 8, y + 7, lk.rock[0]);
        if (h >> 20) % 3 == 0 {
            for ly in (y + 7..face).step_by(3) {
                cv.put(x + 5, ly, 'P');
                cv.put(x + 8, ly, 'P');
                cv.line(x + 5, ly, x + 8, ly, 'b');
            }
        }
        if rootkin {
            // A round knothole of a window in the roots.
            cv.ellipse(x as f32, y as f32, 4.0, 4.0, lk.rock[0]);
            cv.ellipse(x as f32, y as f32, 2.6, 2.6, if lit { '4' } else { 'K' });
            if lit {
                cv.put(x, y, '5');
                lights.push(Light { x: x as f32, y: y as f32, r: 26.0, s: 0.3, fire: true });
            }
            continue;
        }
        // An arched window in the rock.
        cv.rect(x - 3, y - 4, 7, 9, lk.rock[0]);
        cv.rect(x - 2, y - 3, 5, 8, if lit { '5' } else { 'K' });
        cv.put(x - 2, y - 3, lk.rock[0]);
        cv.put(x + 2, y - 3, lk.rock[0]);
        if lit {
            cv.rect(x - 1, y - 1, 3, 4, '6');
            lights.push(Light { x: x as f32, y: y as f32, r: 30.0, s: 0.32, fire: lk.warm });
        }
    }
}

/// Far up the Capfolk's wall, the caps of the old giants, silhouettes in
/// the dark, their gills lit faintly blue.
fn giant_caps(cv: &mut Img, lk: &Look) {
    for (k, &(x, top, r)) in [(70, 50, 34.0f32), (205, 40, 26.0), (395, 46, 30.0), (520, 38, 22.0)].iter().enumerate() {
        let foot = v::BACK as i32 * TILE;
        cv.rect(x - 4, top + (r * 0.5) as i32, 9, foot - top, lk.rock[1]);
        cv.line(x - 4, top + (r * 0.5) as i32, x - 4, foot, lk.rock[0]);
        for y in top..top + (r * 0.6) as i32 {
            for xx in x - r as i32..=x + r as i32 {
                let (dx, dy) = ((xx - x) as f32 / r, (y - top - (r * 0.6) as i32) as f32 / (r * 0.6));
                if dx * dx + dy * dy < 1.0 {
                    let n = vnoise(xx as f32 / 5.0, y as f32 / 5.0, 9210 + k as u32);
                    cv.put(xx, y, if n > 0.62 { 'B' } else if n > 0.3 { 'b' } else { 'n' });
                }
            }
        }
        let rim = top + (r * 0.6) as i32;
        for xx in x - r as i32 + 3..x + r as i32 - 2 {
            if xx % 2 == 0 {
                cv.put(xx, rim, 'q');
                cv.put(xx, rim + 1, 's');
            }
        }
    }
}

/// The Glimmerkin's seams: crystal clusters in the face, and the falls.
fn crystal_seams(cv: &mut Img, lights: &mut Vec<Light>) {
    for k in 0..12 {
        let h = hash(k, 2, 9220);
        let (x, y) = (12 + (h % 552) as i32, 60 + ((h >> 10) % 44) as i32);
        crystal(cv, x, y, 5 + (h >> 20) as i32 % 9, k as u32);
        lights.push(Light { x: x as f32, y: y as f32 - 4.0, r: 26.0, s: 0.18, fire: false });
    }
    // The falls: a sheet of water down the face into a pool.
    let x0 = 452;
    for y in 0..v::BACK as i32 * TILE + 6 {
        for x in x0..x0 + 18 {
            let n = hash(x, y / 3, 9221) % 4;
            cv.put(x, y, ['S', 'u', 'U', 'v'][n as usize]);
        }
    }
    cv.ellipse((x0 + 9) as f32, (v::BACK as i32 * TILE + 8) as f32, 16.0, 5.0, 'S');
    cv.ellipse((x0 + 9) as f32, (v::BACK as i32 * TILE + 8) as f32, 12.0, 3.0, 'u');
}

/// A crystal cluster at `(x, y)` (its foot), `size` tall: three shards,
/// the middle one tallest, each a prism with a lit face, a dark face, a
/// bright ridge and a pale tip.
pub(super) fn crystal(cv: &mut Img, x: i32, y: i32, size: i32, seed: u32) {
    for (j, lean) in [-1i32, 0, 1].into_iter().enumerate() {
        let tall = (size * if j == 1 { 10 } else { 6 + (hash(seed as i32, j as i32, 9222) % 3) as i32 } / 10).max(3);
        let half = (tall / 5).clamp(1, 4);
        let bx = x + lean * (half + 2);
        for d in 0..tall {
            let tip = tall - d <= half + 1;
            let hw = if tip { tall - d - 1 } else { half };
            let shift = lean * d / 4;
            for o in -hw..=hw {
                let ink = if o == 0 { 'z' } else if o < 0 { 'Q' } else { 'q' };
                cv.put(bx + o + shift, y - d, ink);
            }
            if !tip && d % 5 == 2 {
                cv.put(bx - hw + shift, y - d, 'v');
            }
        }
        cv.put(bx + lean * tall / 4, y - tall, 'V');
    }
}

/// The Emberfolk's wall: slow falls of fire down the face, into a channel.
fn lava_falls(cv: &mut Img, lights: &mut Vec<Light>) {
    for &x0 in &[96, 410] {
        for y in 18..v::BACK as i32 * TILE + 4 {
            let wide = 3 + (vnoise(y as f32 / 6.0, x0 as f32, 9230) * 4.0) as i32;
            for x in x0 - wide..=x0 + wide {
                let hot = (x - x0).abs() < wide / 2;
                cv.put(x, y, if hot { '@' } else { '7' });
            }
        }
        for k in 0..4 {
            lights.push(super::super::fire(x0, 24 + k * 12, 46.0, 0.35));
        }
    }
}

/// The Lampwick's lake: black water along the foot of the wall, far lamps
/// on it.
fn lake(cv: &mut Img) {
    let foot = v::BACK as i32 * TILE;
    for y in foot - 14..foot + 6 {
        for x in 0..cv.w {
            if (y + hash(x / 9, y, 9240) as i32 % 2) % 3 == 0 {
                cv.put(x, y, if hash(x, y, 9241) % 7 == 0 { 'Q' } else { 's' });
            } else {
                cv.put(x, y, 'x');
            }
        }
    }
}

/// The glowworms: the deep people's stars, threads hanging from the roof,
/// each with a bead of light that breathes. They brighten with the
/// village, and the falling star wakes them as it passes.
pub(super) fn glowworms(cv: &mut Img, run: &Run, vl: &Village, lk: &Look) {
    let tick = run.tick as u32;
    let since = run.tick.saturating_sub(vl.arrived);
    let woken = (since as f32 / v::FALL as f32).min(1.0);
    let bright = vl.light().max(woken * 0.5);
    let count = 40 + (bright * 50.0) as i32;
    for k in 0..count {
        let h = hash(k, 5, 9250);
        let x = (h % cv.w as u32) as i32;
        let y = 2 + ((h >> 11) % 62) as i32;
        let thread = (h >> 20) as i32 % 6;
        cv.line(x, y - thread, x, y - 1, 'X');
        let breath = (tick / 5 + (h >> 7)) % 24;
        let ink = match (breath < 4, bright > 0.5) {
            (true, true) => lk.glow[3],
            (true, false) | (false, true) => lk.glow[2],
            (false, false) => lk.glow[1],
        };
        cv.put(x, y, ink);
        if breath < 2 && bright > 0.3 {
            cv.put(x - 1, y, lk.glow[1]);
            cv.put(x + 1, y, lk.glow[1]);
        }
    }
}

/// Each people's own weather, moving in the light.
pub(super) fn ambient(cv: &mut Img, run: &Run, vl: &Village, lk: &Look) {
    let tick = run.tick as u32;
    let lit = vl.light();
    match vl.culture {
        // Spores drift up off the caps and the heart, catching the light.
        Culture::Capfolk => {
            for k in 0..(18 + (lit * 30.0) as u32) {
                let h = hash(k as i32, 9, 9260);
                let life = 160 + h % 120;
                let t = (tick + h % life) % life;
                let x0 = 40 + (h >> 8) % 500;
                let x = x0 as i32 + ((t as f32 * 0.07 + k as f32).sin() * 6.0) as i32;
                let y = 348 - (t as i32 * 300 / life as i32);
                cv.put(x, y, if t % 30 < 12 { lk.glow[2] } else { lk.glow[1] });
            }
        }
        // The falls run, and the crystals glint as the light moves.
        Culture::Glimmerkin => {
            let x0 = 452;
            for k in 0..26 {
                let h = hash(k, 3, 9261);
                let x = x0 + (h % 18) as i32;
                let y = ((tick * 3 + h % 120) % 122) as i32;
                cv.line(x, y, x, y + 3, if k % 3 == 0 { 'W' } else { 'V' });
            }
            for k in 0..10 {
                let h = hash(k, 4, 9262);
                if (tick / 3 + h) % 40 < 2 {
                    let (x, y) = (12 + (h % 552) as i32, 50 + ((h >> 10) % 220) as i32);
                    cv.put(x, y, 'w');
                    cv.put(x - 1, y, '3');
                    cv.put(x + 1, y, '3');
                    cv.put(x, y - 1, '3');
                    cv.put(x, y + 1, '3');
                }
            }
        }
        // Smoke from every chimney, embers rising off the falls.
        Culture::Emberfolk => {
            for &rect in v::HOUSES.iter().chain(std::iter::once(&v::HALL)) {
                let (cx, cy) = super::houses::chimney(rect);
                let cy = cy - 10;
                for k in 0..6u32 {
                    let rise = ((tick + k * 17 + cx as u32) % 70) as i32;
                    let drift = ((rise as f32 * 0.08 + k as f32).sin() * 4.0) as i32 + rise / 9;
                    let ink = if rise < 24 { 'G' } else if rise < 48 { 'j' } else { 'g' };
                    cv.put(cx + drift, cy - rise, ink);
                    cv.put(cx + drift + 1, cy - rise, ink);
                }
            }
            for k in 0..20 {
                let h = hash(k, 6, 9263);
                let t = (tick + h % 90) % 90;
                let x = if k % 2 == 0 { 96 } else { 410 } + ((h >> 6) % 14) as i32 - 7;
                cv.put(x + (t as i32 / 9), 118 - t as i32, if t < 30 { '6' } else { '@' });
            }
        }
        // Mist drifts over the lake and the far lamps sway on it.
        Culture::Lampwick => {
            for k in 0..60 {
                let h = hash(k, 7, 9264);
                let x = ((h % 600) as i32 + (tick as i32 / 3)) % 600 - 12;
                let y = 86 + ((h >> 10) % 26) as i32;
                if (h >> 20) % 3 != 0 {
                    cv.put(x, y, 'u');
                }
            }
            for k in 0..5 {
                let x = 50 + k * 115;
                let bob = ((tick as f32 * 0.05 + k as f32).sin() * 1.5) as i32;
                cv.put(x, 98 + bob, '6');
                cv.put(x, 99 + bob, '5');
                cv.put(x, 104 - bob, '4');
            }
        }
        Culture::Roostfolk => super::roost::ambient(cv, run, vl, lk),
        Culture::Rootkin => super::roots::ambient(cv, run, vl, lk),
        Culture::Chimefolk => super::bells::ambient(cv, run, vl, lk),
        Culture::Saltkin => super::salt::ambient(cv, run, vl, lk),
    }
}

/// The party, falling as a star: a bright head with a long tail streaking
/// down out of the dark to the plaza, then a burst where it lands, rings
/// going out, and a glow that lingers.
pub(super) fn star(cv: &mut Img, run: &Run, vl: &Village) {
    let since = run.tick.saturating_sub(vl.arrived);
    let (lx, ly) = px(v::LANDING);
    let (sx, sy) = (lx - 176, ly - 116);
    // First a glint in the dark, growing, before it falls.
    const GLINT: u64 = 12;
    if since < GLINT {
        let r = 1 + since as i32 / 3;
        cv.put(sx, sy, 'w');
        for d in 1..=r {
            for (ox, oy) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
                cv.put(sx + ox, sy + oy, if d == r { '3' } else { 'w' });
            }
        }
        return;
    }
    if since < v::FALL {
        let t = (since - GLINT) as f32 / (v::FALL - GLINT) as f32;
        // Easing in: the star gathers speed as it falls.
        let e = t * t;
        let (hx, hy) = (sx as f32 + (lx - sx) as f32 * e, sy as f32 + (ly - sy) as f32 * e);
        let (dx, dy) = ((lx - sx) as f32, (ly - sy) as f32);
        let len = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = (dx / len, dy / len);
        // The tail: thick at the head, thinning and cooling behind it.
        let tail = 60.0 + 80.0 * t;
        for k in 0..(tail as i32) {
            let f = k as f32 / tail;
            let (x, y) = (hx - ux * k as f32, hy - uy * k as f32);
            let ink = if f < 0.15 { 'w' } else if f < 0.35 { '3' } else if f < 0.6 { '2' } else { '1' };
            let thick = ((1.0 - f) * 4.5) as i32;
            for o in -thick - 1..=thick + 1 {
                let (px_, py_) = ((x - uy * o as f32) as i32, (y + ux * o as f32) as i32);
                if o.abs() > thick {
                    // A faint glow round the tail.
                    if hash(k, o, 9273) % 3 == 0 {
                        cv.put(px_, py_, '1');
                    }
                } else if hash(k, o, (since as u32) ^ 0x51) % 3 != 0 || o == 0 {
                    cv.put(px_, py_, ink);
                }
            }
            // Sparks shed off the tail.
            if k % 9 == 0 && f > 0.2 {
                let h = hash(k, since as i32, 9270);
                cv.put(x as i32 + (h % 9) as i32 - 4, y as i32 + ((h >> 4) % 9) as i32 - 4, if h % 2 == 0 { '9' } else { '6' });
            }
        }
        // The head: a white core in a halo, with a cross of light.
        let (x, y) = (hx as i32, hy as i32);
        for k in 0..32 {
            let a = k as f32 / 32.0 * std::f32::consts::TAU;
            cv.put(x + (a.cos() * 9.0) as i32, y + (a.sin() * 9.0) as i32, if k % 2 == 0 { '2' } else { '1' });
        }
        cv.ellipse(hx, hy, 6.0, 6.0, '3');
        cv.ellipse(hx, hy, 4.0, 4.0, 'w');
        for d in 4..10 {
            for (ox, oy) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
                cv.put(x + ox, y + oy, if d < 7 { 'w' } else { '3' });
            }
        }
        return;
    }
    let after = since - v::FALL;
    // The burst: rings going out across the plaza, light flung off in
    // rays, a glow that settles on the ground.
    if after < 36 {
        let f = after as f32;
        for (ring, ink) in [(f * 3.2, '9'), (f * 2.3, '6'), (f * 1.5, '3')] {
            let r = ring + 2.0;
            let steps = (r * 6.0) as i32 + 8;
            for k in 0..steps {
                let a = k as f32 / steps as f32 * std::f32::consts::TAU;
                if hash(k, after as i32, 9271) % 4 == 0 {
                    continue;
                }
                cv.put(lx + (a.cos() * r) as i32, ly + (a.sin() * r * 0.55) as i32, ink);
            }
        }
        for k in 0..14 {
            let a = k as f32 / 14.0 * std::f32::consts::TAU + 0.2;
            let (r0, r1) = (f * 1.8, f * 3.6 + 6.0);
            if after < 18 {
                cv.line(
                    lx + (a.cos() * r0) as i32,
                    ly + (a.sin() * r0 * 0.55) as i32 - 4,
                    lx + (a.cos() * r1) as i32,
                    ly + (a.sin() * r1 * 0.55) as i32 - 4,
                    if k % 2 == 0 { 'w' } else { '6' },
                );
            }
        }
        if after < 10 {
            cv.ellipse(lx as f32, (ly - 6) as f32, 14.0 - after as f32, 14.0 - after as f32, 'w');
        }
    }
    // The glow where it came down, fading while the village gathers.
    if after < v::GATHER {
        let fade = 1.0 - after as f32 / v::GATHER as f32;
        let r = 10.0 + 18.0 * fade;
        for k in 0..60 {
            let h = hash(k, (run.tick / 4) as i32, 9272);
            let a = (h % 628) as f32 / 100.0;
            let d = ((h >> 10) % 100) as f32 / 100.0 * r;
            if (h >> 20) % 100 < (fade * 100.0) as u32 {
                cv.put(lx + (a.cos() * d) as i32, ly + (a.sin() * d * 0.5) as i32, if d < r * 0.4 { '9' } else { '6' });
            }
        }
    }
}
