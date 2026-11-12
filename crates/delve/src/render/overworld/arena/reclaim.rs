//! King Brannoc's reclaiming, drawn: the three lost halls below (the
//! Delvers' Gallery, the Hall of Lamps, the Anvil Gallery) from cold and
//! held to lit and the King's, their keepers, the talk with the King and
//! with a keeper, and the three lamps in the King's Hall that burn one for
//! each hall won.
//!
//! The still art (pillars, thrones, stones, lamps, benches, rubble) is the
//! cached scenery and keyed by the hall's state (`print`); the keepers and
//! the dig's bars are drawn each frame.

use super::super::chrome::{self, Tone};
use super::super::ink::{Img, hash, ink, text_width};
use super::super::kit;
use super::{TILE, fire, home, near_prompt, stand};
use crate::render::overworld::light::Light;
use crate::rules::barony::{self, Court};
use crate::rules::folk::{Figure, Folk, Pose};
use crate::rules::reclaim::{
    HALLS, Who, hall_of,
    play::DIG_HOLD,
    rooms::{ANVILS, CHAIR, DIGS, FORGE_MOUTH, HEAPS, KEEPER_AT, LAMPS, RENT_PLATE, SLABS, SONS, pillars, rite_ages},
};
use crate::rules::{RoomKind, Run, TILE_UNITS};
use std::hash::{Hash, Hasher};

fn px(tile: f32) -> i32 {
    (tile * TILE as f32) as i32
}

/// A scatter offset in `0..span`, from a hash.
fn scatter(a: i32, b: i32, seed: u32, span: i32) -> i32 {
    (hash(a, b, seed) % span.max(1) as u32) as i32
}

/// A hall as the party finds it now.
struct Look {
    i: usize,
    /// The King's by the realm's word, or played out and waiting on it.
    won: bool,
    how: String,
    stage: u8,
    lit: Vec<u8>,
    dug: u8,
}

fn look(run: &Run) -> Option<Look> {
    if run.at_home_now() || run.room().kind != RoomKind::DwarfHall {
        return None;
    }
    let (i, def) = hall_of(run.dungeon.pack)?;
    let halls = &run.home.barony.halls;
    let held = halls.is_held(def.id);
    let play = run.hall_play();
    let stage = if held { 2 } else { play.map_or(0, |p| p.stage) };
    let how = halls
        .how(def.id)
        .map(str::to_string)
        .or_else(|| play.map(|p| p.how.clone()))
        .unwrap_or_default();
    Some(Look {
        i,
        won: stage == 2,
        how,
        stage,
        lit: play.map(|p| p.lit.clone()).unwrap_or_default(),
        dug: play.map_or(0, |p| p.dug),
    })
}

/// What the hall's still art depends on, for the cached drawing.
pub(super) fn print(run: &Run) -> u64 {
    let Some(l) = look(run) else { return 0 };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (l.i, l.won, &l.how, l.stage, &l.lit, l.dug, run.keeper_at().is_some()).hash(&mut h);
    h.finish()
}

// ── Sprites ──────────────────────────────────────────────────────────────

/// Chief Nubbin Tallowtooth: a goblin in a dwarf's cracked helm with a
/// tallow candle burning on it, fat in a leather vest.
const NUBBIN: [&str; 19] = [
    ".......@........",
    "......@6@.......",
    ".......T........",
    ".......T........",
    ".....GGGGGG.....",
    "....GhhGGhhG....",
    "...jjjjjjjjjj...",
    ".lMmmEmmmmEmmMl.",
    "lMMmm4mmmm4mmMMl",
    ".lMmmmmRRmmmmMl.",
    "..lmmcmmmmcmml..",
    "...EmmccccmmE...",
    "....pppRRppp....",
    "...ppBBBBBBpp...",
    "..ppBBOBBOBBpp..",
    "..pBBBBBBBBBBp..",
    "...PPbbbbbbPP...",
    "...bPP....PPb...",
    "...nn......nn...",
];

/// Thane Madoc Greyhelm: a dwarf in a grey helm and a white beard to his
/// belt, drawn in the cold ink of the dead.
const MADOC: [&str; 20] = [
    ".....GGGGGG.....",
    "....GhhhhhhG....",
    "...GhhiiiihhG...",
    "...jjjjjjjjjj...",
    "...uUU3UU3UUu...",
    "...uUUUVVUUUu...",
    "..uUUVVVVVVUUu..",
    "..uUVViVViVVUu..",
    ".uUUVVVVVVVVUUu.",
    ".uUVVViVVViVVUu.",
    ".uUVVVVVVVVVVUu.",
    ".uUUVVViiVVVUUu.",
    "..uUUVVVVVVUUu..",
    "..uuUUVVVVUUuu..",
    "..xuUUUVVUUUux..",
    "..xxuUUUUUUuxx..",
    "...xxuUUUUuxx...",
    "...xx.uUUu.xx...",
    "....x..xx..x....",
    ".....x.....x....",
];

/// Olwen Anvilhand: a copper braid, a soot-dark beard braided with rings, a
/// leather apron.
const OLWEN: [&str; 17] = [
    "....pPPPPPp.....",
    "...pPPPPPPPp....",
    "...PPOOOOOPP....",
    "...POKOOOKOP....",
    "...POOOooOOP....",
    "...pRRRRRRRp....",
    "..pRRrRRrRRRp...",
    "..pRRtRRRtRRp...",
    ".OJBBBBBBBBBJO..",
    ".OJBBpBBBpBBJO..",
    "..JBBBBBBBBBJ...",
    "..JBBppppppBJ...",
    "...bBBBBBBBBb...",
    "...bBBBBBBBBb...",
    "....jj....jj....",
    "....nn....nn....",
    "................",
];

fn figure(folk: Folk, pose: Pose, t: u32, loaded: bool, east: bool) -> Img {
    let f = Figure {
        folk,
        x: 0.0,
        y: 0.0,
        east,
        pose,
        loaded,
        t,
    };
    super::barony::figure(&f, t)
}

/// A goblin tenant: a miner's build in the Tallow-Hands' green.
fn goblin(t: u32) -> Img {
    figure(Folk::Miner, Pose::Walk, t, false, true).recolor(&[('O', 'M'), ('K', 'F'), ('I', 'l'), ('r', 'E')])
}

/// The dead: every other pixel falls away down the lower rows.
fn ghostly(im: &Img, fade_from: i32, tick: u32) -> Img {
    let mut out = im.clone();
    for y in fade_from..im.h {
        for x in 0..im.w {
            let keep = ((x + y + (tick / 12) as i32) % 2 == 0) && (y - fade_from) < (im.h - fade_from) * 3 / 4;
            if !keep {
                out.clear(x, y);
            }
        }
    }
    out
}

// ── Scenery: the halls ───────────────────────────────────────────────────

/// A dwarf-hewn pillar standing on tile `(c, r)`, a bowl on its head that
/// burns when the hall is the King's.
fn pillar(cv: &mut Img, lights: &mut Vec<Light>, c: i32, r: i32, lit: bool, tick: u32) {
    let (x, y) = (c * TILE, (r + 1) * TILE);
    cv.rect(x + 1, y - 4, 14, 4, 'j');
    cv.rect(x + 2, y - 5, 12, 1, 'G');
    cv.rect(x + 3, y - 21, 10, 16, 'G');
    cv.rect(x + 3, y - 21, 2, 16, 'J');
    cv.rect(x + 11, y - 21, 2, 16, 'j');
    cv.line(x + 7, y - 20, x + 7, y - 6, 'g');
    cv.rect(x + 3, y - 13, 10, 2, 'X');
    for k in 0..4 {
        cv.put(x + 4 + k * 3, y - 13, 'h');
        cv.put(x + 5 + k * 3, y - 12, 'J');
    }
    cv.rect(x + 1, y - 24, 14, 3, 'j');
    cv.rect(x + 2, y - 25, 12, 1, 'G');
    cv.rect(x + 4, y - 28, 8, 3, 'g');
    cv.rect(x + 3, y - 29, 10, 1, 'j');
    if lit {
        flame(cv, x + 8, y - 30, tick, c * 7 + r);
        lights.push(fire(x + 8, y - 32, 58.0, 0.46));
    }
}

/// A flame of the realm's fire inks with its tip flickering.
fn flame(cv: &mut Img, x: i32, y: i32, tick: u32, seed: i32) {
    let f = (hash(tick as i32 / 3, seed, 91) % 3) as i32;
    cv.rect(x - 2, y - 2, 4, 3, '@');
    cv.put(x - 1 - f % 2, y - 4, '5');
    cv.put(x - 1, y - 3, '5');
    cv.put(x, y - 3, '6');
    cv.put(x - f % 2, y - 5, '6');
}

/// The tankard of Caer Dwfn, on a banner or a throne's back.
fn tankard(cv: &mut Img, x: i32, y: i32, body: char, foam: char) {
    cv.rect(x, y + 2, 6, 7, body);
    cv.rect(x + 6, y + 3, 2, 4, body);
    cv.put(x + 7, y + 4, '.');
    cv.rect(x, y, 6, 2, foam);
    cv.put(x + 1, y - 1, foam);
    cv.put(x + 4, y - 1, foam);
}

/// A banner of the tankard's red, hung from the wall at tile column `c`.
fn banner(cv: &mut Img, c: i32) {
    let x = c * TILE;
    cv.rect(x - 1, 1, 12, 2, 'P');
    cv.rect(x, 3, 10, 18, 'p');
    cv.rect(x + 1, 3, 2, 18, 'R');
    cv.rect(x + 7, 3, 2, 18, '8');
    for k in 0..5 {
        cv.put(x + k * 2 + 1, 21 + (k % 2), 'p');
    }
    tankard(cv, x + 1, 8, 't', 'T');
}

fn run_has_chief(l: &Look) -> bool {
    l.stage == 0 || (l.won && l.how == "bargain")
}

fn gallery(cv: &mut Img, lights: &mut Vec<Light>, l: &Look, tick: u32) {
    // The King's grandsire's chair: a hewn throne with the tankard on its
    // back, barrels piled either side.
    let (cx, cy, cw, _) = CHAIR;
    let x = cx * TILE;
    let y = cy * TILE;
    let barrels = kit::barrels();
    cv.stamp(&barrels, x, y + 16);
    cv.stamp(&barrels.flip_h(), x + cw * TILE - barrels.w, y + 16);
    cv.rect(x + 17, y + 2, 30, 26, 'G');
    cv.rect(x + 17, y + 2, 3, 26, 'J');
    cv.rect(x + 44, y + 2, 3, 26, 'j');
    cv.rect(x + 20, y + 6, 24, 18, 'j');
    for k in 0..24 {
        cv.put(x + 20 + k, y + 5, 'X');
    }
    cv.rect(x + 19, y, 26, 2, 'J');
    tankard(cv, x + 29, y + 10, 'O', 'T');
    cv.rect(x + 14, y + 24, 36, 4, 'h');
    cv.rect(x + 14, y + 28, 36, 3, 'g');
    cv.rect(x + 14, y + 20, 4, 8, 'G');
    cv.rect(x + 46, y + 20, 4, 8, 'G');
    // The hoard in the corners, or the dwarves' ore.
    for (k, &(hc, hr, hw, _)) in HEAPS.iter().enumerate() {
        let (hx, hy) = (hc * TILE, hr * TILE);
        if l.won {
            let sacks = kit::sacks(9);
            cv.stamp(&sacks, hx + (hw * TILE - sacks.w) / 2, hy + 14);
            cv.rect(hx + 4, hy + 4, 10, 10, 'B');
            cv.line(hx + 4, hy + 8, hx + 13, hy + 8, 'O');
            cv.line(hx + 28, hy + 10, hx + 40, hy + 4, 'P');
            cv.line(hx + 38, hy + 2, hx + 43, hy + 6, 'h');
        } else {
            cv.ellipse((hx + hw * TILE / 2) as f32, (hy + 22) as f32, 21.0, 10.0, 'o');
            for n in 0..44 {
                let (dx, dy) = (scatter(n, k as i32, 41, 38) - 19, scatter(n, k as i32, 42, 14) - 6);
                cv.put(hx + hw * TILE / 2 + dx, hy + 19 + dy, ['t', 'T', '5', 'O'][(n % 4) as usize]);
            }
            cv.rect(hx + 28, hy + 12, 5, 4, 'z');
            cv.rect(hx + 6, hy + 14, 7, 3, 'h');
        }
    }
    if run_has_chief(l) {
        // The candle on the chief's helm lights his chair.
        lights.push(fire(x + 32, y + 30, 70.0, 0.42));
    }
    if !l.won {
        // The Tallow-Hands' candles, stuck in their own wax about the chair.
        for (n, &(cx2, cy2)) in [(8.0, 5.6), (15.5, 6.2), (7.0, 9.2), (17.0, 9.4), (10.5, 3.8), (13.8, 3.9)].iter().enumerate() {
            let (qx, qy) = (px(cx2), px(cy2));
            cv.rect(qx - 2, qy, 4, 2, 'T');
            cv.rect(qx - 1, qy - 4, 2, 4, 'c');
            let steady = !hash(tick as i32 / 4, n as i32, 83).is_multiple_of(5);
            cv.put(qx - 1, qy - 5, if steady { '5' } else { '6' });
            lights.push(fire(qx, qy - 4, 30.0, 0.26));
        }
    } else {
        banner(cv, 6);
        banner(cv, 16);
    }
}

fn lamps(cv: &mut Img, lights: &mut Vec<Light>, l: &Look, seed: u64, tick: u32) {
    // A faded runner down the nave.
    let (x0, x1) = (11 * TILE, 13 * TILE);
    for y in 3 * TILE..12 * TILE {
        for x in x0..x1 {
            let edge = x < x0 + 3 || x >= x1 - 3;
            let wear = hash(x, y, 77) % 5 == 0;
            let ink_ = if edge {
                'O'
            } else if wear {
                'b'
            } else if (y / 6 + x / 6) % 2 == 0 {
                'p'
            } else {
                'B'
            };
            cv.put(x, y, ink_);
        }
    }
    // The four name-stones: a son and his winters each, cut so the dead can
    // be read by.
    let ages = rite_ages(seed);
    for (k, &(c, r, w, h)) in SLABS.iter().enumerate() {
        let (x, y) = (c * TILE, r * TILE);
        cv.rect(x + 2, y + 2, w * TILE - 4, h * TILE - 2, 'u');
        cv.rect(x + 2, y + 2, w * TILE - 4, 2, 'U');
        cv.rect(x + 2, y + 2, 2, h * TILE - 2, 'U');
        cv.rect(x + w * TILE - 4, y + 2, 2, h * TILE - 2, 'S');
        cv.rect(x + 5, y + 6, w * TILE - 10, h * TILE - 8, 's');
        lights.push(Light { x: (x + w * TILE / 2) as f32, y: (y + 26) as f32, r: 46.0, s: 0.3, fire: false });
        let (name, winters) = SONS[ages[k]];
        let name_x = x + (w * TILE - text_width(name)) / 2;
        cv.text(name_x, y + 8, name, '2');
        let age = format!("AGED {winters}");
        cv.text(x + (w * TILE - text_width(&age)) / 2, y + 19, &age, '1');
    }
    // The lamps: a bowl on three legs, cold, or kindled.
    for (k, &(lx, ly)) in LAMPS.iter().enumerate() {
        let (x, y) = (px(lx), px(ly));
        let lit = l.won || l.lit.contains(&(k as u8));
        cv.line(x - 5, y + 4, x - 2, y - 4, 'g');
        cv.line(x + 5, y + 4, x + 2, y - 4, 'g');
        cv.line(x, y + 5, x, y - 3, 'j');
        cv.rect(x - 5, y - 7, 10, 3, 'G');
        cv.rect(x - 6, y - 8, 12, 1, 'J');
        cv.rect(x - 4, y - 6, 8, 1, if lit { '8' } else { 'k' });
        if lit {
            flame(cv, x, y - 9, tick, k as i32 + 31);
            lights.push(fire(x, y - 12, 78.0, 0.62));
        }
    }
    // Wax on the floor, bones at the edges.
    for n in 0..16 {
        let (bx, by) = (2 * TILE + scatter(n, 5, 61, 20 * TILE), 9 * TILE + scatter(n, 6, 61, 3 * TILE));
        cv.rect(bx, by, 3, 1, 'i');
        cv.put(bx + 3, by - (n % 2), 'h');
    }
    if !l.won {
        // The thane's cold light on the stones.
        lights.push(Light { x: px(11.5) as f32, y: px(5.5) as f32, r: 130.0, s: 0.5, fire: false });
    } else {
        lights.push(Light { x: px(11.5) as f32, y: px(6.5) as f32, r: 90.0, s: 0.28, fire: false });
    }
}

/// A broken stone with its light on the upper left.
fn boulder(cv: &mut Img, x: i32, y: i32, rx: f32, ry: f32) {
    cv.ellipse(x as f32, y as f32 + 1.0, rx, ry, 'g');
    cv.ellipse(x as f32, y as f32, rx, ry - 0.5, 'j');
    cv.ellipse(x as f32 - 1.0, y as f32 - 1.0, rx * 0.6, ry * 0.5, 'G');
    cv.put(x - 2, y - 2, 'J');
}

fn anvils(cv: &mut Img, lights: &mut Vec<Light>, l: &Look, tick: u32) {
    // The great forge-mouth in the north wall: dark and webbed, or burning.
    let (mc, _, mw, _) = FORGE_MOUTH;
    let x = mc * TILE;
    cv.rect(x - 2, 4, mw * TILE + 4, 28, 'G');
    cv.rect(x - 2, 4, mw * TILE + 4, 3, 'J');
    cv.rect(x + 8, 10, mw * TILE - 16, 22, 'X');
    for yy in 12..32 {
        for xx in x + 12..x + mw * TILE - 12 {
            let ink_ = if l.won {
                match hash(xx, yy + (tick / 3) as i32, 99) % 5 {
                    0 => '6',
                    1 | 2 => '@',
                    3 => '7',
                    _ => '4',
                }
            } else {
                'k'
            };
            cv.put(xx, yy, ink_);
        }
    }
    cv.rect(x + 8, 8, mw * TILE - 16, 3, 'j');
    if l.won {
        lights.push(fire(x + mw * TILE / 2, 26, 120.0, 0.7));
    } else {
        for k in 0..6 {
            cv.line(x + 14 + k * 12, 12, x + 20 + k * 11, 30, 'g');
        }
    }
    // Two benches with an anvil on a stump, their tools hung above.
    for &(c, r, w, _) in &ANVILS {
        let (bx, by) = (c * TILE, r * TILE);
        cv.rect(bx + 2, by + 6, w * TILE - 4, 8, 'B');
        cv.rect(bx + 2, by + 6, w * TILE - 4, 2, 'P');
        cv.rect(bx + 14, by + 2, 18, 4, 'h');
        cv.rect(bx + 12, by + 1, 6, 3, 'J');
        cv.rect(bx + 20, by + 6, 6, 8, 'j');
        for k in 0..4 {
            cv.line(bx + 4 + k * 3, 3, bx + 4 + k * 3, 9, if k % 2 == 0 { 'h' } else { 'G' });
        }
    }
    // The three heaps of fallen roof, or where they were.
    for (s, &(c, r, w, h)) in DIGS.iter().enumerate() {
        let (hx, hy) = (c * TILE, r * TILE);
        let (cx, cy) = ((hx + w * TILE / 2) as f32, (hy + h * TILE / 2 + 4) as f32);
        if l.dug & 1 << s == 0 && !l.won {
            // A mound of broken roof, the lowest stones drawn last.
            let mut stones: Vec<(i32, i32, f32)> = (0..20)
                .map(|n| {
                    let (rx, ry) = ((w * TILE / 2 - 8) as f32, (h * TILE / 2 - 5) as f32);
                    let (a, d) = (scatter(n, s as i32, 51, 628) as f32 / 100.0, scatter(n, s as i32, 52, 100) as f32 / 100.0);
                    let (px_, py_) = (cx + a.cos() * rx * d.sqrt(), cy + a.sin() * ry * d.sqrt());
                    (px_ as i32, py_ as i32, 3.5 + scatter(n, s as i32, 53, 40) as f32 / 10.0)
                })
                .collect();
            stones.sort_by_key(|&(_, y, _)| y);
            cv.line(cx as i32 - 16, cy as i32 - 5, cx as i32 + 13, cy as i32 - 13, 'B');
            cv.line(cx as i32 - 16, cy as i32 - 4, cx as i32 + 13, cy as i32 - 12, 'r');
            for (bx, by, r) in stones {
                boulder(cv, bx, by, r + 1.0, r);
            }
            // Embers in the rubble: the Keep's imps nest here.
            lights.push(fire(cx as i32, cy as i32 - 2, 44.0, 0.3));
        } else {
            for n in 0..9 {
                let dx = scatter(n, s as i32, 53, w * TILE - 6) - (w * TILE - 6) / 2;
                let dy = scatter(n, s as i32, 54, h * TILE - 8) - (h * TILE - 8) / 2;
                cv.rect(cx as i32 + dx, cy as i32 + dy, 2, 1, 'g');
            }
        }
    }
    // The beam that pinned her, across the smith, until she is freed.
    if !l.won {
        let (bx, by) = (px(KEEPER_AT[2].0), px(KEEPER_AT[2].1));
        cv.rect(bx - 24, by - 8, 48, 5, 'B');
        cv.rect(bx - 24, by - 8, 48, 1, 'P');
        cv.rect(bx - 28, by - 3, 18, 6, 'g');
        cv.rect(bx + 10, by - 2, 18, 5, 'j');
    }
}

/// The hall's still art: pillars, then what each hall keeps in it.
pub(super) fn scenery(cv: &mut Img, run: &Run, lights: &mut Vec<Light>) {
    let Some(l) = look(run) else { return };
    let tick = run.tick as u32;
    for &(c, r) in pillars(l.i) {
        pillar(cv, lights, c, r, l.won, tick);
    }
    match l.i {
        0 => gallery(cv, lights, &l, tick),
        1 => lamps(cv, lights, &l, run.seed, tick),
        _ => anvils(cv, lights, &l, tick),
    }
}

// ── The King's Hall: a lamp for each hall won ────────────────────────────

/// Where the King's Hall hangs its lamps of Caer Dwfn (tile columns), and
/// the plate before them where the halls are read.
pub const HALL_LAMPS: [f32; 3] = [15.0, 16.5, 18.0];
pub const LAMP_PLATE: (i32, i32, i32, i32) = (15, 4, 2, 1);

pub(super) fn kings_lamps(cv: &mut Img, run: &Run, lights: &mut Vec<Light>) {
    let tick = run.tick as u32;
    for (k, &c) in HALL_LAMPS.iter().enumerate() {
        let x = px(c);
        let lit = run.home.barony.halls.is_held(HALLS[k].id);
        cv.rect(x - 1, 2, 3, 5, 'j');
        cv.rect(x - 5, 6, 11, 3, 'G');
        cv.rect(x - 6, 5, 13, 1, 'J');
        cv.rect(x - 4, 8, 9, 2, 'g');
        cv.rect(x - 3, 6, 7, 1, if lit { '8' } else { 'k' });
        if lit {
            flame(cv, x + 1, 5, tick, k as i32 + 5);
            lights.push(fire(x + 1, 6, 74.0, 0.55));
        }
    }
}

// ── Figures ──────────────────────────────────────────────────────────────

/// The keepers and the held halls' dwarves.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let Some(l) = look(run) else { return };
    let tick = run.tick as u32;
    let at = |t: (f32, f32)| (px(t.0), px(t.1));
    let fought = l.how == "fight";
    match l.i {
        0 => {
            if run.keeper_at().is_some() {
                let (x, y) = at(KEEPER_AT[0]);
                if l.won && fought {
                    let steward = figure(Folk::Guard, Pose::Idle, tick, false, true);
                    stand(cv, &steward, x, y);
                } else {
                    let bob = i32::from((tick / 20).is_multiple_of(2));
                    stand(cv, &Img::from_rows(&NUBBIN), x, y - 4 + bob / 2);
                }
            }
            if l.won {
                // Delvers back at the ore, or the Tallow-Hands digging for rent.
                for (k, &(tx, ty)) in [(4.0f32, 5.5f32), (19.5, 5.5), (7.5, 8.0)].iter().enumerate() {
                    let (x, y) = at((tx, ty));
                    let t = tick + k as u32 * 11;
                    let mut worker = if !fought && k != 1 {
                        goblin(t)
                    } else {
                        figure(Folk::Miner, Pose::Walk, t, false, k % 2 == 0)
                    };
                    if (t / 14).is_multiple_of(2) {
                        worker = worker.clone();
                    }
                    stand(cv, &worker, x, y);
                }
            }
        }
        1 => {
            if run.keeper_at().is_some() {
                let sway = ((tick as f32 * 0.07).sin() * 1.5) as i32;
                let rest = l.stage == 2;
                let mut im = Img::from_rows(&MADOC);
                if rest {
                    // Eyes closed, at rest.
                    im.put(6, 4, 'U');
                    im.put(9, 4, 'U');
                }
                let (x, y) = at(KEEPER_AT[1]);
                stand(cv, &ghostly(&im, 12, tick), x, y + sway);
            }
        }
        _ => {
            let (x, y) = at(KEEPER_AT[2]);
            if l.won {
                // Pell at the bench, hammering; Olwen on her feet.
                let pell = figure(Folk::Smith, Pose::Work, tick, false, true);
                stand(cv, &pell, px(ANVILS[0].0 as f32 + 1.5) + 8, px(ANVILS[0].1 as f32 + 1.5) + 6);
                stand(cv, &Img::from_rows(&OLWEN), x, y + 18);
            } else {
                // Only her head and shoulders show above the beam.
                let buried = Img::from_rows(&OLWEN[..9]);
                stand(cv, &buried, x, y - 6);
            }
        }
    }
    if look(run).is_some() && run.keeper_at().is_some() && run.reclaim.talks.is_empty() {
        let (x, y) = at(KEEPER_AT[l.i]);
        near_prompt(cv, run, (x, y - 4), "TALK");
    }
}

// ── Boards ───────────────────────────────────────────────────────────────

fn place(cv: &mut Img, board: &Img, (x, y): (i32, i32)) {
    cv.stamp(board, (x - board.w / 2).clamp(2, cv.w - board.w - 2), y.clamp(2, cv.h - board.h - 2));
}

/// The set-piece's progress, the dig's hold bars, and the rent plate of a
/// hall the King holds.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let Some(l) = look(run) else { return };
    if l.stage == 1 {
        let board = match l.i {
            1 => Some(home::board(&[
                ("THE RITE OF THE LAMPS".into(), '9'),
                (format!("{} OF 4 BURN", l.lit.len()), '5'),
                ("ELDEST SON FIRST: E AT A LAMP".into(), 'h'),
            ])),
            2 => Some(home::board(&[
                ("THE ANVIL GALLERY".into(), '9'),
                (format!("{} OF 3 HEAPS DUG", l.dug.count_ones()), '5'),
                ("STAND ON A HEAP, HOLD E".into(), 'h'),
            ])),
            _ => None,
        };
        if let Some(board) = board {
            place(cv, &board, (5 * TILE + 6, 10 * TILE + 8));
        }
    }
    if l.i == 2 && l.stage == 1 {
        for hero in run.players.values().filter(|h| h.hp > 0 && h.buying > 0) {
            let (x, y) = super::at(hero.x, hero.y);
            let fill = (hero.buying.min(DIG_HOLD) * 28 / DIG_HOLD) as i32;
            chrome::meter(cv, (x - 15, y - 30, 30, 5), fill, ink('5').unwrap_or_default());
        }
    }
    if l.won && run.hall_held() {
        let (c, r, w, h) = RENT_PLATE;
        let on = run.players.values().any(|p| {
            p.hp > 0 && {
                let (tx, ty) = (p.x / TILE_UNITS, p.y / TILE_UNITS);
                tx >= c as f32 && tx < (c + w) as f32 && ty >= r as f32 && ty < (r + h) as f32
            }
        });
        if on {
            let def = &HALLS[l.i];
            let mut rent = vec![(def.name.to_uppercase(), '9'), ("THE KING'S, AND LIT".into(), '5')];
            let mine = {
                let mut s = crate::realm::Spoils::default();
                for &(sp, n) in def.rent {
                    s.add(sp, n);
                }
                if l.how == "bargain" {
                    for &(sp, n) in def.rent_bargain {
                        s.add(sp, n);
                    }
                }
                s
            };
            rent.push((format!("RENT A FLOOR: {}", mine.label().to_uppercase()), 'h'));
            rent.push((format!("{} DWARVES HOME FROM IT", def.crew), 'h'));
            place(cv, &home::board(&rent), (cv.w / 2, 6 * TILE));
        }
    }
}

/// The plate where the rent is read, once the hall is the King's.
pub(super) fn plates(cv: &mut Img, run: &Run) {
    let Some(l) = look(run) else { return };
    if l.won && run.hall_held() {
        let lit = run.players.values().any(|p| {
            p.hp > 0 && {
                let (c, r, w, h) = RENT_PLATE;
                let (tx, ty) = (p.x / TILE_UNITS, p.y / TILE_UNITS);
                tx >= c as f32 && tx < (c + w) as f32 && ty >= r as f32 && ty < (r + h) as f32
            }
        });
        home::plate_marked(cv, RENT_PLATE, lit, &Img::from_rows(&["OOOOO", "O.O.O", "OOOOO", "O...O", "OOOOO"]));
    }
}

/// The King's Hall: the lamps' plate, and what the halls pay.
pub(super) fn hall_boards(cv: &mut Img, run: &Run) {
    let b = &run.home.barony;
    if b.court != Court::Sworn || run.room().kind != RoomKind::KingsHall {
        return;
    }
    let on = barony::on_plate(run, LAMP_PLATE).is_some();
    if !on {
        return;
    }
    let mut lines = vec![("THE LAMPS OF CAER DWFN".to_string(), '9')];
    for def in HALLS.iter() {
        let state = if b.halls.is_held(def.id) {
            "LIT".to_string()
        } else if b.halls.asked_hall().is_some_and(|(_, h)| h.id == def.id) {
            "ASKED OF YOU".to_string()
        } else {
            "COLD".to_string()
        };
        lines.push((format!("{}: {state}", def.name.to_uppercase()), if b.halls.is_held(def.id) { '5' } else { 'h' }));
    }
    let rent = b.halls.rent();
    lines.push((
        if rent.is_empty() { "NO RENT YET".to_string() } else { format!("RENT A FLOOR: {}", rent.label().to_uppercase()) },
        'O',
    ));
    let board = home::board(&lines);
    let (c, r, w, h) = LAMP_PLATE;
    let x = c * TILE + w * TILE / 2 - board.w / 2;
    cv.stamp(&board, x.clamp(2, cv.w - board.w - 2), ((r + h) * TILE + 4).min(cv.h - board.h - 2));
}

pub(super) fn hall_plate(cv: &mut Img, run: &Run) {
    let b = &run.home.barony;
    if b.court != Court::Sworn || run.room().kind != RoomKind::KingsHall {
        return;
    }
    let lit = barony::on_plate(run, LAMP_PLATE).is_some();
    home::plate_marked(cv, LAMP_PLATE, lit, &Img::from_rows(&["..O..", ".OOO.", "OOOOO", ".OOO.", "..O.."]));
}

// ── The talk ─────────────────────────────────────────────────────────────

fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut row = String::new();
    for word in s.split_whitespace() {
        if !row.is_empty() && row.len() + 1 + word.len() > n {
            out.push(std::mem::take(&mut row));
        }
        if !row.is_empty() {
            row.push(' ');
        }
        row.push_str(word);
    }
    if !row.is_empty() {
        out.push(row);
    }
    out
}

/// A speaker's face: the head of their figure, in a well.
fn portrait(who: Who, run: &Run) -> Img {
    super::resized(&face(who, run), 2.0)
}

fn face(who: Who, run: &Run) -> Img {
    let tick = run.tick as u32;
    match who {
        Who::King => Img::from_rows(&super::barony::KING[..12]),
        Who::Keeper => match hall_of(run.dungeon.pack).map(|(i, _)| i).unwrap_or(0) {
            0 if run.hall_play().is_some_and(|p| p.stage == 2) && run.home.barony.halls.how("gallery") == Some("fight") => {
                figure(Folk::Guard, Pose::Idle, tick, false, true)
            }
            0 => Img::from_rows(&NUBBIN[..13]),
            1 => Img::from_rows(&MADOC[..12]),
            _ => Img::from_rows(&OLWEN[..12]),
        },
    }
}

/// A knight's talk, on their own screen: the speaker's face, their name and
/// words, and the answers on 1, 2 and 3.
pub(super) fn screen(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let id = focus.unwrap_or(1);
    let Some(view) = run.reclaim_view(id) else { return };
    let face_w = portrait(view.who, run).w;
    let lines = wrap(&view.words, ((cv.w - 24 - face_w - 24) / 6).max(24) as usize);
    let h = (lines.len() as i32 * 9 + view.answers.len() as i32 * 10 + 32).max(66);
    let y = cv.h - h - 6;
    chrome::panel(cv, (6, y, cv.w - 12, h), Tone::FRAMED);
    chrome::ring(cv, (6, y, cv.w - 12, h), 3);
    let face = portrait(view.who, run);
    chrome::inset_panel(cv, (12, y + 8, face.w + 6, face.h + 6), Tone::WELL);
    cv.stamp(&face, 15, y + 11);
    let tx = 24 + face.w;
    cv.text(tx, y + 7, &view.name, '9');
    cv.text(tx, y + 16, &view.tag, 'J');
    for (i, line) in lines.iter().enumerate() {
        cv.text(tx, y + 27 + i as i32 * 9, line, 'i');
    }
    let ay = y + 30 + lines.len() as i32 * 9;
    for (i, answer) in view.answers.iter().enumerate() {
        let yy = ay + i as i32 * 10;
        let keyed = answer.as_bytes().first().is_some_and(u8::is_ascii_digit);
        let (key, rest) = answer.split_at(if keyed { 2 } else { 2.min(answer.len()) });
        let after = cv.text(tx, yy, key, 'O');
        cv.text(after + 2, yy, rest.trim_start(), '5');
    }
}

/// What the King's plate says of the lost halls: the one he has sent the
/// party to, or that he has one to ask.
pub(super) fn mission_lines(run: &Run) -> Vec<(String, char)> {
    let halls = &run.home.barony.halls;
    if let Some((_, hall)) = halls.asked_hall() {
        vec![
            (format!("LOST HALL: {}", hall.name.to_uppercase()), '5'),
            (format!("OFF THE FLOORS OF {}", hall.pack.name().to_uppercase()), 'h'),
        ]
    } else if halls.offer(&run.home.barony).is_some() {
        vec![("HE HAS A LOST HALL TO ASK: E BESIDE HIM".into(), 'O')]
    } else {
        Vec::new()
    }
}

/// The King's prompt: E beside him to talk.
pub(super) fn king_prompt(cv: &mut Img, run: &Run) {
    let b = &run.home.barony;
    if b.court < Court::Met || !run.reclaim.talks.is_empty() {
        return;
    }
    let Some((room, at)) = barony::king_at(b) else { return };
    if room == run.room().kind && run.at_home_now() {
        near_prompt(cv, run, (px(at.0), px(at.1) - 12), "TALK");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sprite_row_is_as_wide_as_the_first() {
        for (name, rows) in [("nubbin", &NUBBIN[..]), ("madoc", &MADOC[..]), ("olwen", &OLWEN[..])] {
            let w = rows[0].len();
            for (y, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), w, "{name} row {y}");
            }
        }
    }
}
