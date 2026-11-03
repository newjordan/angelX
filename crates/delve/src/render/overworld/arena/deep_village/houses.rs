//! The deep people's architecture: each people's houses on the same
//! footprints (`village::HOUSES`), drawn in their own way. The Capfolk live
//! under giant caps, their gills glowing; the Glimmerkin in hewn stone with
//! crystal breaking through the roofs; the Emberfolk in basalt under iron,
//! a chimney each; the Lampwick in stilt-houses over the water. The great
//! hall at the top of the plaza is a people's own grander house, the heart
//! in the plaza their own wonder, and the wishing place stands by the well.
//!
//! A house's windows light in its turn as hope grows (`lit_at`); the
//! heart wakes from the first light and blazes at the last.

use super::super::super::ink::{Img, hash};
use super::super::super::light::Light;
use super::super::super::material as mat;
use super::super::fire;
use super::{Look, all as keep_all, px_rect, sky};
use crate::rules::deep::{Culture, Errand, Town, Village, village as v};

/// The hope at which house `i` lights its windows.
fn lit_at(i: usize) -> u8 {
    [2, 3, 4, 5, 3, 6, 7, 8][i % 8]
}

/// Where a house's chimney (or its people's like of one) stands, in
/// pixels: the smoke rises from it.
pub(super) fn chimney(rect: (i32, i32, i32, i32)) -> (i32, i32) {
    let (x, y, w, _) = px_rect(rect);
    (x + w - 16, y - 22)
}

pub(super) fn all(cv: &mut Img, vl: &Village, town: Option<&Town>, lk: &Look, lights: &mut Vec<Light>, tick: u32) {
    let gathered = town.is_some_and(|t| t.deeds.contains(&Errand::Gather));
    for (i, &rect) in v::HOUSES.iter().enumerate() {
        let lit = vl.hope >= lit_at(i);
        house(cv, vl.culture, rect, i as u32, lit, lk, lights);
        if gathered {
            over_the_door(cv, vl.culture, rect, lk, lights);
        }
    }
    hall(cv, vl, town, lk, lights);
    heart(cv, vl, lk, lights, tick);
    wishing_place(cv, vl, lk, lights, tick);
}

/// What the party gathered below, set over every door as the village
/// promised: a glowcap, a sky-crystal, a lump of star-iron, a pearl wick.
fn over_the_door(cv: &mut Img, culture: Culture, rect: (i32, i32, i32, i32), lk: &Look, lights: &mut Vec<Light>) {
    let (x, y, w, h) = px_rect(rect);
    let (cx, cy) = (x + w / 2, y + h - 22);
    match culture {
        Culture::Capfolk => {
            cv.line(cx, cy, cx, cy - 3, 'T');
            cv.ellipse(cx as f32, (cy - 4) as f32, 4.0, 2.5, '2');
            cv.put(cx - 1, cy - 5, 'w');
        }
        Culture::Glimmerkin => sky::crystal(cv, cx, cy, 8, 5),
        Culture::Emberfolk => {
            cv.ellipse(cx as f32, (cy - 2) as f32, 3.0, 2.5, '@');
            cv.put(cx, cy - 3, '6');
        }
        Culture::Lampwick => {
            cv.ellipse(cx as f32, (cy - 2) as f32, 2.5, 2.5, 'W');
            cv.put(cx, cy - 3, 'w');
        }
        Culture::Roostfolk => super::roost::over_the_door(cv, cx, cy),
        Culture::Rootkin => super::roots::over_the_door(cv, cx, cy),
        Culture::Chimefolk => super::bells::over_the_door(cv, cx, cy),
        Culture::Saltkin => super::salt::over_the_door(cv, cx, cy),
    }
    lights.push(Light { x: cx as f32, y: (cy - 3) as f32, r: 26.0, s: 0.3, fire: lk.warm });
}

fn house(cv: &mut Img, culture: Culture, rect: (i32, i32, i32, i32), seed: u32, lit: bool, lk: &Look, lights: &mut Vec<Light>) {
    let (x, y, w, h) = px_rect(rect);
    // The nests hang or stand on poles, and the spires stand in the water:
    // their shadows are their own.
    if !matches!(culture, Culture::Roostfolk | Culture::Chimefolk | Culture::Rootkin | Culture::Saltkin) {
        mat::shadow(cv, ((x + w / 2 + 6) as f32, (y + h + 2) as f32), (w as f32 * 0.5, 4.0), &keep_all);
    }
    match culture {
        Culture::Capfolk => cap_house(cv, (x, y, w, h), seed, lit, lights),
        Culture::Glimmerkin => crystal_house(cv, (x, y, w, h), seed, lit, lights),
        Culture::Emberfolk => forge_house(cv, (x, y, w, h), seed, lit, lights),
        Culture::Lampwick => stilt_house(cv, (x, y, w, h), seed, lit, lights),
        Culture::Roostfolk => super::roost::house(cv, (x, y, w, h), seed, lit, lights),
        Culture::Rootkin => super::roots::house(cv, (x, y, w, h), seed, lit, lights),
        Culture::Chimefolk => super::bells::house(cv, (x, y, w, h), seed, lit, lights),
        Culture::Saltkin => super::salt::house(cv, (x, y, w, h), seed, lit, lights),
    }
    let _ = lk;
}

/// A window `ww` by `wh` at `(x, y)`: lamplit, or dark glass.
fn window(cv: &mut Img, (x, y, ww, wh): (i32, i32, i32, i32), frame: char, lit: bool, glow: (char, char), lights: &mut Vec<Light>, warm: bool) {
    cv.rect(x - 1, y - 1, ww + 2, wh + 2, frame);
    cv.rect(x, y, ww, wh, if lit { glow.0 } else { 'K' });
    if lit {
        cv.rect(x + 1, y + 1, (ww - 2).max(1), (wh - 2).max(1), glow.1);
        lights.push(Light { x: (x + ww / 2) as f32, y: (y + wh / 2) as f32, r: 44.0, s: 0.36, fire: warm });
    } else {
        cv.put(x + 1, y + 1, 'x');
    }
}

/// A cottage under a giant cap: a cream stem swelling to its foot, a round
/// door and round windows, and over it all the cap's dome, spotted, its
/// rim curling under onto gills that glow when the house is awake.
fn cap_house(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32, lit: bool, lights: &mut Vec<Light>) {
    let cx = x + w / 2;
    let (top_half, foot_half) = (w * 26 / 100, w * 36 / 100);
    let (stem_top, stem_foot) = (y - 6, y + h - 1);
    const STEM: [char; 6] = ['P', 'o', 'O', 't', 'T', 'T'];
    for yy in stem_top..stem_foot {
        let f = (yy - stem_top) as f32 / (stem_foot - stem_top) as f32;
        // The stem swells toward its foot.
        let half = top_half + ((foot_half - top_half) as f32 * f * f) as i32;
        for xx in cx - half..=cx + half {
            let u = (xx - cx) as f32 / half as f32;
            // Lit from the upper left: a broad pale face, a shaded right.
            let light = (1.0 - (u + 0.35).abs() * 1.1).max(0.0);
            let mut i = (light * 5.0) as usize;
            if hash(xx, yy / 2, 9301 + seed) % 11 == 0 {
                i = i.saturating_sub(1);
            }
            cv.put(xx, yy, STEM[i.min(5)]);
        }
        cv.put(cx - half, yy, 'r');
        cv.put(cx + half, yy, 'P');
    }
    // The round door on its step, and a round window or two.
    cv.rect(cx - 8, stem_foot - 1, 17, 2, 'g');
    cv.ellipse(cx as f32, (stem_foot - 11) as f32, 6.5, 7.0, 'B');
    cv.rect(cx - 6, stem_foot - 11, 13, 10, 'B');
    cv.ellipse(cx as f32, (stem_foot - 11) as f32, 5.0, 5.5, 'p');
    cv.rect(cx - 5, stem_foot - 11, 11, 9, 'p');
    cv.line(cx, stem_foot - 16, cx, stem_foot - 3, 'B');
    cv.put(cx + 3, stem_foot - 7, 'O');
    for wx in [cx - top_half + 4, cx + top_half - 10] {
        let wy = y + 6;
        cv.ellipse((wx + 3) as f32, (wy + 3) as f32, 4.5, 4.5, 'B');
        cv.ellipse((wx + 3) as f32, (wy + 3) as f32, 3.0, 3.0, if lit { '5' } else { 'K' });
        if lit {
            cv.put(wx + 2, wy + 2, '6');
            cv.put(wx + 3, wy + 2, '6');
            lights.push(Light { x: (wx + 3) as f32, y: (wy + 3) as f32, r: 46.0, s: 0.4, fire: true });
        }
    }
    // A little cap growing off the stem's side.
    let (sx, sy) = (cx + foot_half - 2, stem_foot - 8);
    cv.line(sx, sy, sx + 4, sy - 5, 'T');
    cv.ellipse((sx + 5) as f32, (sy - 6) as f32, 4.0, 2.5, 'R');
    cv.put(sx + 4, sy - 7, 'T');
    // The cap: a dome, rust, cream or blue.
    let ramps: [&[char]; 3] = [
        &['n', 'B', 'p', 'R', 'R', 'o', 'O'],
        &['b', 'I', 'P', 'r', 'o', 'O', 't', 'T'],
        &['s', 'x', 'q', 'Q', 'z', 'v', 'V'],
    ];
    let pick = (seed as usize * 7 + 1) % 3;
    let ramp = ramps[pick];
    let spot = if pick == 1 { ('R', 'r') } else { ('W', 'V') };
    let (rx, ry) = ((w / 2 + 12) as f32, (w as f32 * 0.42).max(22.0));
    let rim = y - 4;
    mat::lobe(cv, (cx as f32, rim as f32), (rx, ry), ramp, 9302 + seed, 3.0, 0.05, &|_, yy| yy <= rim + 1);
    for k in 0..(w / 6) {
        let hk = hash(k, seed as i32, 9303);
        let (ox, oy) = ((hk % 1000) as f32 / 1000.0 * 2.0 - 1.0, (hk >> 10) % 1000);
        let oy = oy as f32 / 1000.0;
        let (sx, sy) = (cx + (ox * rx * 0.8) as i32, rim - 3 - (oy * ry * 0.85 * (1.0 - ox * ox).sqrt()) as i32);
        let r = 1.5 + ((hk >> 20) % 3) as f32;
        cv.ellipse(sx as f32, sy as f32, r, r * 0.75, spot.1);
        cv.ellipse(sx as f32 - 0.5, sy as f32 - 0.5, (r - 0.8).max(0.6), (r * 0.75 - 0.8).max(0.6), spot.0);
    }
    // The rim curls under onto the gills.
    for xx in cx - rx as i32 + 2..cx + rx as i32 - 1 {
        let u = (xx - cx) as f32 / rx;
        let droop = ((1.0 - u * u) * 3.0) as i32;
        cv.put(xx, rim + 1, ramp[1]);
        cv.put(xx, rim + 2, ramp[0]);
        for g in 0..droop {
            let gill = if (xx + g).rem_euclid(2) == 0 { if lit { '2' } else { 'b' } } else { 'n' };
            cv.put(xx, rim + 3 + g, gill);
        }
    }
    if lit {
        lights.push(Light { x: cx as f32, y: (rim + 4) as f32, r: 58.0, s: 0.26, fire: false });
    }
}

/// A Glimmerkin house: hewn blue stone, a timber door bound in iron, and
/// crystal breaking out through the roof.
fn crystal_house(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32, lit: bool, lights: &mut Vec<Light>) {
    let stone: &[char] = &['s', 'x', 'S', 'u', 'U', 'v'];
    mat::ashlar(cv, (x + 3, y - 10, w - 6, h + 8), stone, 9311 + seed);
    // The roof slab, its edge lit.
    cv.rect(x, y - 16, w, 6, 'S');
    cv.line(x, y - 16, x + w - 1, y - 16, 'v');
    cv.line(x, y - 11, x + w - 1, y - 11, 'x');
    for k in 0..(w / 16).max(2) {
        let hk = hash(k, seed as i32, 9312);
        let (gx, tall) = (x + 10 + k * 16 + (hk % 5) as i32, 14 + (hk >> 6) as i32 % 14);
        sky::crystal(cv, gx, y - 15, tall, hk);
        if lit {
            cv.put(gx, y - 15 - tall, 'w');
            cv.put(gx, y - 14 - tall, '3');
            lights.push(Light { x: gx as f32, y: (y - 18) as f32, r: 30.0, s: 0.3, fire: false });
        }
    }
    // The door, bound in iron.
    let dx = x + w / 2 - 6;
    cv.rect(dx, y + h - 20, 12, 18, 'P');
    for by in [y + h - 16, y + h - 8] {
        cv.line(dx, by, dx + 11, by, 'G');
    }
    cv.frame(dx - 1, y + h - 21, 14, 20, 'x');
    for wx in [x + 8, x + w - 18] {
        window(cv, (wx, y + h - 26, 10, 8), 'x', lit, ('3', 'w'), lights, false);
    }
    // A cart of crystal by the door.
    if seed % 2 == 0 {
        let (cx, cy) = (x + w - 10, y + h + 6);
        cv.rect(cx - 8, cy - 6, 16, 6, 'I');
        cv.frame(cx - 8, cy - 6, 16, 6, 'G');
        cv.put(cx - 5, cy + 1, 'G');
        cv.put(cx + 4, cy + 1, 'G');
        sky::crystal(cv, cx, cy - 6, 5, seed);
    }
}

/// An Emberfolk house: basalt courses under iron plates, a chimney, a
/// window red with the forge inside, an anvil at the door.
fn forge_house(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32, lit: bool, lights: &mut Vec<Light>) {
    let basalt: &[char] = &['k', 'K', 'X', 'g', 'j', 'G'];
    mat::ashlar(cv, (x + 2, y - 6, w - 4, h + 4), basalt, 9321 + seed);
    mat::roof_hip(cv, (x - 2, y - 22, w + 4, 18), &['k', 'K', 'X', 'g', 'j', 'G', 'J'], 9322 + seed, 6);
    let (chx, chy) = (x + w - 16, y - 22);
    cv.rect(chx - 4, chy - 10, 9, 16, 'X');
    cv.frame(chx - 4, chy - 10, 9, 16, 'K');
    cv.line(chx - 5, chy - 10, chx + 5, chy - 10, 'j');
    // The forge's mouth, glowing or banked.
    let (fx, fy) = (x + w / 2 - 8, y + h - 18);
    cv.rect(fx - 1, fy - 1, 18, 14, 'K');
    cv.ellipse((fx + 8) as f32, (fy + 2) as f32, 8.0, 5.0, 'K');
    cv.rect(fx, fy, 16, 12, if lit { '@' } else { '8' });
    cv.rect(fx + 3, fy + 4, 10, 8, if lit { '5' } else { 'Z' });
    if lit {
        cv.rect(fx + 6, fy + 7, 4, 4, '6');
        lights.push(fire(fx + 8, fy + 6, 70.0, 0.55));
    }
    window(cv, (x + 6, y + 6, 8, 7), 'K', lit, ('@', '5'), lights, true);
    // The anvil.
    let (ax, ay) = (x + 10 + (seed % 2) as i32 * (w - 30), y + h + 8);
    cv.rect(ax - 6, ay - 7, 13, 3, 'j');
    cv.line(ax - 8, ay - 7, ax + 8, ay - 7, 'G');
    cv.rect(ax - 2, ay - 4, 5, 5, 'X');
    cv.rect(ax - 4, ay, 9, 2, 'g');
}

/// A Lampwick stilt-house: boards on posts over a puddle of the lake, a
/// steep shingled roof, a round window and a lamp on a pole.
fn stilt_house(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32, lit: bool, lights: &mut Vec<Light>) {
    // The water under it.
    for yy in y + h - 10..y + h + 6 {
        for xx in x..x + w {
            let (dx, dy) = ((xx - x - w / 2) as f32 / (w / 2 + 2) as f32, (yy - y - h + 2) as f32 / 8.0);
            if dx * dx + dy * dy < 1.0 {
                cv.put(xx, yy, if hash(xx / 5, yy, 9331) % 6 == 0 { 'Q' } else { 's' });
            }
        }
    }
    for px_ in [x + 6, x + w / 2, x + w - 7] {
        cv.rect(px_, y + h - 14, 3, 18, 'b');
        cv.put(px_, y + h + 3, 'u');
    }
    mat::boards(cv, (x + 3, y - 8, w - 6, h - 4), true, 9332 + seed);
    // The roof: steep, shingled, its ridge lit.
    let peak = y - 34;
    for yy in peak..y - 6 {
        let half = (w / 2 + 4) * (yy - peak) / (y - 6 - peak).max(1);
        for xx in x + w / 2 - half..=x + w / 2 + half {
            let row = (yy - peak) / 3;
            let shade = if xx < x + w / 2 { 2 } else { 1 };
            let ink = if (yy - peak) % 3 == 2 { 'n' } else { ['b', 'B', 'p', 'R'][(shade + (hash(xx / 4, row, 9333) % 2) as usize).min(3)] };
            cv.put(xx, yy, ink);
        }
    }
    cv.line(x + w / 2, peak, x + w / 2, peak + 2, 'o');
    window(cv, (x + w / 2 - 4, y + 2, 8, 8), 'n', lit, ('5', '6'), lights, true);
    cv.rect(x + w / 2 - 5, y + h - 22, 10, 14, 'n');
    // The lamp on its pole by the door.
    let (lx, ly) = (x + w - 4, y + h - 4);
    cv.line(lx, ly, lx, ly - 30, 'B');
    cv.line(lx, ly - 30, lx - 5, ly - 30, 'B');
    cv.stamp(&super::super::super::kit::lantern(lit), lx - 9, ly - 30);
    if lit {
        lights.push(fire(lx - 6, ly - 26, 50.0, 0.4));
    }
}

/// The great hall: the people's grandest house, its door at the top of
/// the plaza, the town's name over it, and a banner for every deed the
/// party has done for it.
fn hall(cv: &mut Img, vl: &Village, town: Option<&Town>, lk: &Look, lights: &mut Vec<Light>) {
    let lit = vl.hope >= 1;
    house(cv, vl.culture, v::HALL, 7, lit, lk, lights);
    let (x, y, w, _) = px_rect(v::HALL);
    // The deeds' banners, hung along the hall's front.
    let deeds: Vec<Errand> = town.map_or_else(Vec::new, |t| t.deeds.clone());
    for (k, deed) in deeds.iter().take(4).enumerate() {
        let bx = x + 6 + k as i32 * (w - 12) / 4;
        banner(cv, bx, y + 4, *deed, lk);
    }
    // The town's name on a board over the door.
    let name = vl.culture.town(crate::rules::deep::chapter(vl.floor));
    let tw = super::super::super::ink::text_width(name);
    let (sx, sy) = (x + w / 2 - tw / 2 - 3, y + 2);
    cv.rect(sx, sy, tw + 6, 9, 'n');
    cv.frame(sx, sy, tw + 6, 9, 'B');
    cv.text(sx + 3, sy + 1, name, if lit { 'O' } else { 'P' });
}

/// A deed's banner: a cloth hung from a pole, its errand's emblem on it.
fn banner(cv: &mut Img, x: i32, y: i32, deed: Errand, lk: &Look) {
    cv.line(x - 1, y, x + 9, y, 'B');
    let cloth = match deed {
        Errand::Beacons => 'R',
        Errand::Gather => 'A',
        Errand::Rescue => 'Q',
        Errand::Slay(_) => 'p',
    };
    cv.rect(x, y + 1, 9, 14, cloth);
    cv.put(x, y + 15, cloth);
    cv.put(x + 8, y + 15, cloth);
    cv.put(x + 4, y + 16, cloth);
    let mark = lk.glow[3];
    match deed {
        Errand::Beacons => {
            cv.rect(x + 3, y + 6, 3, 5, 'T');
            cv.put(x + 4, y + 4, mark);
            cv.put(x + 4, y + 5, '6');
        }
        Errand::Gather => {
            cv.ellipse((x + 4) as f32, (y + 8) as f32, 2.5, 2.5, mark);
        }
        Errand::Rescue => {
            cv.rect(x + 3, y + 5, 3, 3, 'T');
            cv.rect(x + 2, y + 8, 5, 4, 'T');
        }
        Errand::Slay(_) => {
            cv.line(x + 2, y + 11, x + 6, y + 3, 'V');
            cv.line(x + 3, y + 9, x + 5, y + 9, 'O');
        }
    }
}

/// The heart in the plaza: the people's own wonder, waking with the first
/// light and blazing at the last.
fn heart(cv: &mut Img, vl: &Village, lk: &Look, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::HEART);
    let (cx, foot) = (x + w / 2, y + h);
    let light = vl.light();
    let awake = vl.hope >= 1;
    mat::shadow(cv, ((cx + 8) as f32, (foot + 2) as f32), (34.0, 7.0), &keep_all);
    match vl.culture {
        Culture::Capfolk => {
            // The mother cap: a great blue toadstool, the village's lamp.
            const STEM: [char; 6] = ['P', 'o', 'O', 't', 'T', 'T'];
            for yy in foot - 46..foot {
                let f = (yy - (foot - 46)) as f32 / 46.0;
                let half = 6 + (f * f * 7.0) as i32;
                for xx in cx - half..=cx + half {
                    let u = (xx - cx) as f32 / half as f32;
                    let i = ((1.0 - (u + 0.35).abs() * 1.1).max(0.0) * 5.0) as usize;
                    cv.put(xx, yy, STEM[i.min(5)]);
                }
            }
            // A ring of little caps round its foot.
            for k in 0..7 {
                let a = std::f32::consts::PI * (0.1 + k as f32 * 0.8 / 6.0);
                let (mx, my) = (cx + (a.cos() * 24.0) as i32, foot - 2 + (a.sin() * 5.0) as i32);
                cv.line(mx, my, mx, my - 4, 'T');
                cv.ellipse(mx as f32, (my - 5) as f32, 3.5, 2.0, if awake { 'z' } else { 'q' });
            }
            let ramp: &[char] = if awake { &['s', 'q', 'Q', 'z', 'v', 'V', 'W'] } else { &['k', 's', 'x', 'q', 'Q', 'z'] };
            let rim = foot - 44;
            mat::lobe(cv, (cx as f32, rim as f32), (54.0, 27.0), ramp, 9341, 3.0, 0.05, &|_, yy| yy <= rim + 1);
            for xx in cx - 54..cx + 55 {
                let u = (xx - cx) as f32 / 56.0;
                let droop = ((1.0 - u * u) * 5.0) as i32;
                cv.put(xx, rim + 1, ramp[1]);
                for g in 0..droop {
                    let ink = if xx.rem_euclid(2) == 0 { if light > 0.5 { '3' } else if awake { '2' } else { 's' } } else { 'k' };
                    cv.put(xx, rim + 2 + g, ink);
                }
            }
            for k in 0..12 {
                let hk = hash(k, 1, 9342);
                let ox = (hk % 1000) as f32 / 500.0 - 1.0;
                let up = ((hk >> 10) % 1000) as f32 / 1000.0;
                let (sx, sy) = (cx + (ox * 44.0) as i32, rim - 4 - (up * 21.0 * (1.0 - ox * ox).sqrt()) as i32);
                let r = 2.0 + (hk >> 20) as f32 % 3.0;
                cv.ellipse(sx as f32, sy as f32, r, r * 0.7, if awake { 'w' } else { 'v' });
            }
            if awake {
                lights.push(Light { x: cx as f32, y: (rim + 6) as f32, r: 90.0 + light * 70.0, s: 0.32 + light * 0.3, fire: false });
            }
        }
        Culture::Glimmerkin => {
            // The great crystal: a spire of facets on a stone foot.
            mat::ashlar(cv, (cx - 26, foot - 12, 52, 12), &['s', 'x', 'S', 'u', 'U'], 9343);
            cv.line(cx - 26, foot - 12, cx + 25, foot - 12, 'v');
            for (lean, tall, seed) in [(-20i32, 30i32, 3u32), (19, 26, 4), (-11, 44, 5), (12, 40, 6)] {
                sky::crystal(cv, cx + lean, foot - 12, tall, seed);
            }
            for yy in foot - 80..foot - 12 {
                let t = (yy - (foot - 80)) as f32 / 68.0;
                let half = (2.0 + t.sqrt() * 15.0) as i32;
                for xx in cx - half..=cx + half {
                    let f = (xx - cx) as f32 / half.max(1) as f32;
                    let facet = ((f + 1.0) * 2.5) as i32;
                    let ink = match (facet, awake) {
                        (0, _) => 'q',
                        (1, false) | (3, false) | (4, false) => 'Q',
                        (2, false) => 'z',
                        (1, true) => 'z',
                        (2, true) => if light > 0.5 || t < 0.3 { 'w' } else { '3' },
                        (3, true) => '3',
                        _ => 'Q',
                    };
                    cv.put(xx, yy, ink);
                }
            }
            if awake {
                lights.push(Light { x: cx as f32, y: (foot - 50) as f32, r: 100.0 + light * 60.0, s: 0.32 + light * 0.35, fire: false });
            }
        }
        Culture::Emberfolk => {
            // The forge-heart: a round furnace under a tall stack, its
            // molten core breathing.
            mat::ashlar(cv, (cx - 10, foot - 76, 20, 40), &['k', 'K', 'X', 'g', 'j'], 9346);
            cv.rect(cx - 12, foot - 80, 24, 5, 'g');
            cv.line(cx - 12, foot - 80, cx + 11, foot - 80, 'J');
            mat::ashlar(cv, (cx - 30, foot - 40, 60, 40), &['k', 'K', 'X', 'g', 'j'], 9344);
            cv.ellipse(cx as f32, (foot - 40) as f32, 30.0, 8.0, 'g');
            cv.ellipse(cx as f32, (foot - 40) as f32, 24.0, 6.0, 'K');
            let breath = (tick / 6) % 4;
            cv.ellipse(cx as f32, (foot - 40) as f32, 21.0, 5.0, if awake { '@' } else { '8' });
            cv.ellipse(cx as f32, (foot - 40) as f32, 12.0 + breath as f32, 3.0, if awake { '5' } else { '7' });
            if light > 0.5 {
                cv.ellipse(cx as f32, (foot - 40) as f32, 6.0, 1.5, '6');
            }
            // Its mouth, and the great anvil before it.
            cv.ellipse(cx as f32, (foot - 18) as f32, 11.0, 9.0, 'K');
            cv.rect(cx - 11, foot - 18, 23, 14, 'K');
            cv.ellipse(cx as f32, (foot - 17) as f32, 8.0, 7.0, if awake { '@' } else { '8' });
            cv.rect(cx - 8, foot - 17, 17, 11, if awake { '@' } else { '8' });
            cv.rect(cx - 5, foot - 13, 11, 7, if awake { '5' } else { 'Z' });
            if awake {
                lights.push(fire(cx, foot - 34, 110.0 + light * 50.0, 0.45 + light * 0.3));
                lights.push(fire(cx, foot - 12, 60.0, 0.4));
            }
        }
        Culture::Lampwick => {
            // The great lamp: a tall post in its pool, a lantern up top.
            for yy in foot - 14..foot + 4 {
                for xx in cx - 36..cx + 36 {
                    let (dx, dy) = ((xx - cx) as f32 / 36.0, (yy - foot + 5) as f32 / 9.0);
                    if dx * dx + dy * dy < 1.0 {
                        cv.put(xx, yy, if hash(xx / 4, yy, 9345) % 5 == 0 { 'Q' } else { 'x' });
                    }
                }
            }
            mat::ashlar(cv, (cx - 9, foot - 16, 18, 10), &['s', 'x', 'S', 'u', 'U'], 9347);
            cv.rect(cx - 3, foot - 54, 7, 40, 'u');
            cv.line(cx - 3, foot - 54, cx - 3, foot - 14, 'v');
            cv.line(cx + 3, foot - 54, cx + 3, foot - 14, 'S');
            for ry in [foot - 28, foot - 42] {
                cv.line(cx - 4, ry, cx + 4, ry, 'x');
            }
            let (lx, ly) = (cx, foot - 70);
            cv.line(lx - 14, ly - 4, lx + 14, ly - 4, 'K');
            cv.line(lx - 8, ly - 7, lx + 8, ly - 7, 'K');
            cv.put(lx, ly - 9, 'K');
            cv.rect(lx - 12, ly - 3, 24, 20, 'k');
            cv.frame(lx - 12, ly - 3, 24, 20, 'B');
            cv.rect(lx - 9, ly, 18, 14, if awake { '5' } else { 'g' });
            cv.line(lx, ly - 3, lx, ly + 16, 'k');
            if awake {
                cv.rect(lx - 6, ly + 3, 4, 8, '6');
                cv.rect(lx + 3, ly + 3, 4, 8, '6');
                lights.push(fire(lx, ly + 8, 110.0 + light * 50.0, 0.42 + light * 0.3));
            }
        }
        Culture::Roostfolk => super::roost::heart(cv, vl, lights, tick),
        Culture::Rootkin => super::roots::heart(cv, vl, lights, tick),
        Culture::Chimefolk => super::bells::heart(cv, vl, lights, tick),
        Culture::Saltkin => super::salt::heart(cv, vl, lights, tick),
    }
    let _ = lk;
}

/// The wishing place by the well: a mushroom-roofed well, a wishing
/// stone, an anvil on its stump, a lamp in a pool.
fn wishing_place(cv: &mut Img, vl: &Village, lk: &Look, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::WELL);
    let (cx, foot) = (x + w / 2, y + h);
    let glow = vl.hope >= 2;
    match vl.culture {
        Culture::Roostfolk => return super::roost::wishing_place(cv, vl, lights, tick),
        Culture::Rootkin => return super::roots::wishing_place(cv, vl, lights, tick),
        Culture::Chimefolk => return super::bells::wishing_place(cv, vl, lights, tick),
        Culture::Saltkin => return super::salt::wishing_place(cv, vl, lights, tick),
        _ => {}
    }
    mat::shadow(cv, ((cx + 4) as f32, (foot + 1) as f32), (16.0, 4.0), &keep_all);
    match vl.culture {
        Culture::Capfolk => {
            mat::ashlar(cv, (x + 1, foot - 14, w - 2, 14), &['b', 'I', 'P', 'r', 'o'], 9351);
            cv.ellipse(cx as f32, (foot - 14) as f32, 14.0, 4.0, 'n');
            cv.ellipse(cx as f32, (foot - 14) as f32, 11.0, 2.5, if glow { '2' } else { 's' });
            for sx in [x + 2, x + w - 3] {
                cv.line(sx, foot - 14, sx, foot - 32, 'B');
            }
            mat::lobe(cv, (cx as f32, (foot - 32) as f32), (21.0, 11.0), &['n', 'B', 'p', 'R', 'o', 'O'], 9352, 2.0, 0.05, &|_, yy| yy <= foot - 30);
        }
        Culture::Glimmerkin => {
            mat::ashlar(cv, (x + 2, foot - 10, w - 4, 10), &['s', 'x', 'S', 'u', 'U'], 9353);
            sky::crystal(cv, cx, foot - 10, 20, 77);
            if glow {
                cv.put(cx, foot - 22, 'w');
            }
        }
        Culture::Emberfolk => {
            cv.rect(x + 6, foot - 12, w - 12, 12, 'B');
            cv.frame(x + 6, foot - 12, w - 12, 12, 'b');
            cv.rect(x + 1, foot - 18, w - 2, 5, 'j');
            cv.line(x - 2, foot - 18, x + w + 1, foot - 18, 'G');
            cv.line(x + 2, foot - 19, x + w - 3, foot - 19, 'J');
            if glow {
                cv.line(x + 4, foot - 20, x + w - 5, foot - 20, '@');
            }
        }
        Culture::Lampwick => {
            cv.ellipse(cx as f32, (foot - 4) as f32, 16.0, 6.0, 'x');
            cv.ellipse(cx as f32, (foot - 4) as f32, 12.0, 4.0, 's');
            cv.line(cx, foot - 4, cx, foot - 26, 'B');
            cv.stamp(&super::super::super::kit::lantern(glow), cx - 3, foot - 34);
        }
        Culture::Roostfolk | Culture::Rootkin | Culture::Chimefolk | Culture::Saltkin => {}
    }
    if glow {
        lights.push(Light { x: cx as f32, y: (foot - 16) as f32, r: 46.0, s: 0.35, fire: lk.warm });
    }
}
