//! The town's rooms drawn: the market's stalls and their keepers, the
//! bathhouse's tubs and Brother Suds, the smithy's hearth, anvil and Old
//! Dint. Furniture is cached scenery; keepers' faces, soakers, the hammer
//! and the helmet test are drawn per frame from the town's state.
use super::super::chrome;
use super::super::ink::{Img, hash, text_width};
use super::super::{kit, light::Light};
use super::{TILE, at, fire, home, nooks, props};
use super::super::material::{self as mat, ramp};
use crate::rules::{
    RoomKind, Run,
    town::{self as t, ANVIL, AT_ANVIL, AT_RACK, BARREL, BATH_GOLD, BENCH, COPPER, CROSS, HEARTH, MEND_GOLD, RACK, STALLS, TEMPER_GOLD, TUBS},
};

type Rect = (i32, i32, i32, i32);

fn px((c, r, w, h): Rect) -> (i32, i32, i32, i32) {
    (c * TILE, r * TILE, w * TILE, h * TILE)
}

/// The awning's two stripes for each stall: ore grey, wax cream, bone red.
const STRIPES: [(char, char); 3] = [('G', 'h'), ('o', 'T'), ('R', 'T')];

/// A stall's counter and awning, its goods laid out.
fn stall(cv: &mut Img, i: usize) {
    let (x, y, w, h) = px(STALLS[i].counter);
    let front = if i == 2 { y } else { y + h - 12 };
    let top = if i == 2 { y + h - 9 } else { y - 2 };
    props::stall(cv, (x, y, w, h), STRIPES[i], front, top, 211 + i as u32);
    let gy = front + 3;
    match i {
        0 => {
            for k in 0..6 {
                let gx = x + 6 + k * 9;
                mat::lobe(cv, (gx as f32, (gy + 1) as f32), (3.5, 2.5), ramp::STONE, 212 + k as u32, 1.2, 0.2, &nooks::all);
                cv.put(gx - 1, gy, 'z');
            }
        }
        1 => {
            for k in 0..8 {
                let gx = x + 6 + k * 7;
                let tall = 4 + (k % 3) as i32 * 2;
                cv.rect(gx, gy + 6 - tall, 3, tall, 'T');
                cv.line(gx, gy + 6 - tall, gx, gy + 5, 't');
                cv.put(gx + 1, gy + 5 - tall, 'b');
            }
        }
        _ => {
            // A soup pot on the counter, steaming.
            let (px_, py_) = (x + 15, gy);
            mat::lobe(cv, (px_ as f32, (py_ + 1) as f32), (8.0, 5.0), nooks::IRON, 213, 2.0, 0.0, &nooks::all);
            cv.ellipse(px_ as f32, (py_ - 3) as f32, 7.0, 2.0, 'J');
            cv.ellipse(px_ as f32, (py_ - 3) as f32, 6.0, 1.0, 'P');
            for k in 0..3 {
                cv.put(px_ - 4 + k * 4, py_ - 7 - k % 2, 'G');
            }
            for k in 0..4 {
                let bx = x + 34 + k * 7;
                cv.line(bx, gy + 4, bx + 5, gy + 1, 'V');
                cv.rect(bx - 1, gy + 4, 2, 2, 'V');
                cv.rect(bx + 5, gy, 2, 2, 'V');
            }
        }
    }
}

/// The stone market cross on its steps.
fn cross(cv: &mut Img) {
    let (x, y, w, h) = px(CROSS);
    mat::block(cv, (x - 2, y + h - 8, w + 4, 8), ramp::STONE, false, 221);
    mat::block(cv, (x + 3, y + h - 13, w - 6, 5), ramp::STONE, false, 222);
    mat::block(cv, (x + w / 2 - 2, y - 10, 5, h - 3), ramp::STONE, false, 223);
    mat::block(cv, (x + w / 2 - 8, y - 5, 17, 5), ramp::STONE, false, 224);
}

/// A bathing tub.
fn tub(cv: &mut Img, x: i32, y: i32) {
    props::tub(cv, x, y, (x * 3) as u32);
}

/// The copper over its fire, for the hot water.
fn copper(cv: &mut Img, tick: u32) {
    props::copper(cv, px(COPPER), tick);
}

/// The towel bench, towels folded on it.
fn bench(cv: &mut Img) {
    let (x, y, w, h) = px(BENCH);
    mat::shadow(cv, ((x + w / 2 + 4) as f32, (y + h + 2) as f32), (w as f32 * 0.5, 2.0), &nooks::all);
    mat::boards(cv, (x, y + 4, w, 5), false, 231);
    for lx in [x + 2, x + w - 4] {
        cv.rect(lx, y + 9, 2, h - 9, 'b');
    }
    for k in 0..5 {
        let tx = x + 4 + k * 15;
        mat::lobe(cv, ((tx + 5) as f32, (y + 2) as f32), (5.0, 2.5), if k % 2 == 0 { nooks::LINEN } else { ramp::SLATE }, 232 + k as u32, 2.0, 0.0, &nooks::all);
    }
}

/// The smith's hearth, its coals always hot.
fn hearth(cv: &mut Img, tick: u32) {
    props::hearth(cv, px(HEARTH), 2 + (tick / 6 % 2), 241);
    // The bellows' nozzle in its west side, and a pair of tongs.
    let (x, y, _, _) = px(HEARTH);
    cv.rect(x - 10, y + 24, 10, 6, 'B');
    cv.rect(x - 3, y + 26, 4, 2, 'o');
}

/// The anvil on its stump.
fn anvil(cv: &mut Img) {
    let (x, y, w, h) = px(ANVIL);
    nooks::anvil(cv, x + w / 2, y + h + 2);
}

/// The armour rack.
fn rack(cv: &mut Img) {
    props::rack(cv, px(RACK));
}

/// The quench trough by the door.
fn barrel(cv: &mut Img) {
    let (x, y, w, h) = px(BARREL);
    nooks::trough(cv, (x, y + 8, w, h - 8), 0);
}

pub(super) fn scenery(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    match run.room().kind {
        RoomKind::Market => {
            for i in 0..STALLS.len() {
                stall(cv, i);
            }
            cross(cv);
            // Bunting from wall to wall.
            for k in 0..1 {
                let y = 5 * TILE + 8;
                let (x0, x1) = (TILE, 23 * TILE);
                for x in x0..x1 {
                    let sag = ((x - x0) * (x1 - x)) / 4000;
                    cv.put(x, y + sag - 40 * k, 'P');
                    if (x - x0) % 14 == 7 {
                        let ink = ['R', 'T', 'u', 'O'][(((x - x0) / 14) as usize + k as usize) % 4];
                        for d in 0..5 {
                            cv.line(x - 2 + d / 2, y + sag - 40 * k + 1 + d, x + 2 - d / 2, y + sag - 40 * k + 1 + d, ink);
                        }
                    }
                }
            }
            // Crates and a barrel by the walls.
            mat::barrel(cv, 9 * TILE + 4, 12 * TILE + 6);
            mat::barrel(cv, 10 * TILE + 2, 12 * TILE + 4);
            mat::crate_box(cv, 12 * TILE, 12 * TILE + 6, 251);
            mat::crate_box(cv, 13 * TILE + 2, 12 * TILE + 6, 252);
            for k in 0..3 {
                mat::hay(cv, 9 * TILE + 4 + k * 10, 2 * TILE + 8, 4.0, 253 + k as u32);
            }
        }
        RoomKind::Bathhouse => {
            // A tiled dais under the tubs: glazed tiles in close tones, each
            // lit along its top and west edge and worn at its corners, set in
            // dark grout.
            for yy in 2 * TILE..5 * TILE + 8 {
                for xx in 2 * TILE..21 * TILE {
                    let (cx, cy, lx, ly) = (xx / 8, yy / 8, xx % 8, yy % 8);
                    let corner = (lx == 1 || lx == 7) && (ly == 1 || ly == 7);
                    if lx == 0 || ly == 0 || corner {
                        continue;
                    }
                    let deep = hash(cx, cy, 231) % 3 == 0;
                    let ch = if ly == 1 || lx == 1 {
                        if deep { 's' } else { 'S' }
                    } else if ly == 7 || lx == 7 {
                        'k'
                    } else if deep {
                        'x'
                    } else {
                        's'
                    };
                    cv.put(xx, yy, ch);
                }
            }
            for xx in 2 * TILE..21 * TILE {
                cv.put(xx, 5 * TILE + 8, 'S');
            }
            for &(x, y) in &TUBS {
                let (x, y) = at(x * 2.0, y * 2.0);
                tub(cv, x, y);
            }
            copper(cv, tick);
            bench(cv);
            // Washing lines of towels across the south.
            cv.line(9 * TILE, 10 * TILE, 19 * TILE, 10 * TILE + 3, 'o');
            for k in 0..6 {
                // A towel over the line, falling in soft folds lit from the
                // west, a stripe woven near its hem.
                let tx = 9 * TILE + 6 + k * 26;
                let ramp: &[char] = if k % 2 == 0 { &['J', 'h', 'i', 'H'] } else { &['S', 'u', 'U', 'v'] };
                for dx in 0..9 {
                    let fold = ((dx as f32 * 1.3 + k as f32).sin() * 0.22) + 0.78 - dx as f32 * 0.03;
                    let hem = 11 - (dx == 0 || dx == 8) as i32;
                    for dy in 0..hem {
                        let ink = if dy == 8 { 'T' } else { mat::pick(ramp, if dy == hem - 1 { fold - 0.25 } else { fold }) };
                        cv.put(tx + dx, 10 * TILE + 1 + k / 2 + dy, ink);
                    }
                }
            }
            // The price, painted on a board by the door.
            let sign = format!("SOAK {BATH_GOLD}G");
            let w = text_width(&sign) + 6;
            chrome::inset_panel(cv, (2 * TILE, 5 * TILE - 2, w, 11), chrome::Tone::TIMBER);
            cv.text(2 * TILE + 3, 5 * TILE, &sign, 'T');
        }
        RoomKind::Smithy => {
            hearth(cv, tick);
            anvil(cv);
            rack(cv);
            barrel(cv);
            cv.stamp(&kit::blade_rack(4), 3 * TILE, 9 * TILE);
            nooks::coal(cv, 17 * TILE, 11 * TILE + 8);
            // A stack of bar iron, waiting.
            for k in 0..4 {
                let by = 10 * TILE + 4 + k * 3;
                cv.rect(7 * TILE - k % 2, by, 30, 2, if k % 2 == 0 { 'G' } else { 'j' });
                cv.line(7 * TILE - k % 2, by, 7 * TILE + 29 - k % 2, by, 'J');
            }
            // Soot and scale on the floor round the anvil.
            for k in 0..40 {
                let sx = 9 * TILE + (hash(k, 1, 221) % (6 * TILE as u32)) as i32;
                let sy = 5 * TILE + (hash(k, 2, 221) % (3 * TILE as u32)) as i32;
                cv.put(sx, sy, if k % 3 == 0 { 'g' } else { 'X' });
            }
        }
        _ => {}
    }
}

pub(super) fn lights(lights: &mut Vec<Light>, run: &Run) {
    match run.room().kind {
        RoomKind::Smithy => {
            let (x, y, w, _) = px(HEARTH);
            lights.push(fire(x + w / 2, y + 32, 96.0, 0.5));
        }
        RoomKind::Bathhouse => {
            let (x, y, w, h) = px(COPPER);
            lights.push(fire(x + w / 2, y + h - 6, 60.0, 0.36));
            lights.push(fire(12 * TILE, 2 * TILE, 110.0, 0.3));
        }
        RoomKind::Market => {
            lights.push(fire(12 * TILE, 4 * TILE, 120.0, 0.3));
            lights.push(fire(12 * TILE, 10 * TILE, 120.0, 0.3));
        }
        _ => {}
    }
}

/// A stall's keeper behind the counter, red in the face as the haggling
/// climbs; shuttered up when offended.
fn keeper(cv: &mut Img, run: &Run, i: usize) {
    let (x, y, w, h) = px(STALLS[i].counter);
    if run.town.shut[i] > 0 {
        // Shutters down, a sulk behind them.
        let front = if i == 2 { y } else { y + h - 12 };
        // Upright planks of timber, a batten across them, the corners
        // worn off, its foot in shadow.
        let (sx, sy, sw, sh) = (x + 3, front - 14, w - 6, 26);
        let kept = super::super::chrome::corners(cv, (sx, sy, sw, sh), &[2, 1]);
        mat::boards(cv, (sx, sy, sw, sh), true, 214 + i as u32);
        mat::boards(cv, (sx, sy + 4, sw, 3), false, 215 + i as u32);
        super::super::chrome::restore(cv, &kept);
        cv.text(x + w / 2 - 9, front - 6, "OUT", 'T');
        return;
    }
    let haggle = run.town.haggles.values().find(|&&(s, _)| usize::from(s) == i).map(|&(_, n)| n);
    let temper = haggle.map_or(0, |n| n * 3 / t::haggle_limit(i).max(1));
    let face = ['t', 'R', '7'][temper.min(2) as usize];
    let (kx, ky) = (x + w / 2, if i == 2 { y + h - 2 } else { y + h - 13 });
    // Body behind the counter, then the head over it.
    let (coat, hat) = [('U', 'R'), ('P', 'K'), ('h', 'c')][i];
    cv.rect(kx - 6, ky - 9, 13, 9, coat);
    let _ = face;
    props::face(cv, kx, ky - 13, temper);
    match i {
        0 => cv.rect(kx - 5, ky - 19, 11, 3, hat),
        1 => {
            cv.rect(kx - 4, ky - 23, 9, 6, hat);
            cv.line(kx - 6, ky - 18, kx + 6, ky - 18, hat);
        }
        _ => {
            cv.rect(kx - 5, ky - 22, 11, 5, hat);
            cv.ellipse(kx as f32, (ky - 23) as f32, 6.0, 3.0, hat);
        }
    }
    if temper >= 2 {
        // Steam from the ears.
        for s in [-1, 1] {
            cv.line(kx + s * 6, ky - 15, kx + s * 9, ky - 19, 'h');
        }
    }
}

/// Brother Suds: a monk in a wet habit with a very long brush.
fn suds(cv: &mut Img, x: i32, y: i32, scrubbing: bool, tick: u64) {
    let frame = if scrubbing { (tick / 4 % 2) as i32 } else { 0 };
    cv.rect(x - 5, y - 14, 11, 14, 'u');
    cv.rect(x - 4, y - 12, 9, 12, 'S');
    cv.line(x - 4, y - 7, x + 4, y - 7, 'o');
    cv.rect(x - 3, y - 20, 7, 6, 't');
    cv.put(x - 1, y - 18, 'k');
    cv.put(x + 2, y - 18, 'k');
    cv.line(x - 3, y - 21, x + 3, y - 21, 'r');
    // The brush, reaching into the nearest tub.
    let (bx, by) = (x - 12 - frame * 2, y - 22 + frame * 2);
    cv.line(x - 4, y - 10, bx, by, 'o');
    cv.rect(bx - 4, by - 2, 6, 3, 'T');
    for k in 0..3 {
        cv.put(bx - 3 + k * 2, by + 1, 'h');
    }
}

/// A knight up to the chin in a tub: the helm, its plume in the seat's
/// colour, suds and the duck.
fn soaker(cv: &mut Img, x: i32, y: i32, id: u32, tick: u64) {
    let plume = match id {
        1 => '7',
        2 => '1',
        3 => 'A',
        _ => '4',
    };
    // The helm, still on (it is a very brave bath), the plume up.
    cv.ellipse(x as f32, (y - 6) as f32, 7.0, 7.0, 'J');
    cv.rect(x - 7, y - 6, 15, 6, 'J');
    cv.line(x - 6, y - 12, x + 4, y - 12, 'h');
    cv.rect(x - 5, y - 6, 11, 2, 'k');
    cv.rect(x - 1, y - 18, 3, 6, plume);
    cv.rect(x + 1, y - 19, 3, 3, plume);
    // Suds round the neck.
    for k in 0..9 {
        let a = k as f32 * std::f32::consts::TAU / 9.0 + (tick / 6) as f32 * 0.3;
        cv.ellipse(x as f32 + a.cos() * 11.0, y as f32 + a.sin() * 3.5, 2.5, 2.0, 'v');
    }
    // The duck, bobbing.
    let bob = (tick / 10 % 2) as i32;
    let (dx, dy) = (x + 11, y - 1 + bob);
    cv.rect(dx - 3, dy - 3, 7, 4, '5');
    cv.rect(dx + 1, dy - 7, 4, 4, '5');
    cv.rect(dx + 5, dy - 6, 2, 2, '4');
    cv.put(dx + 3, dy - 6, 'k');
    // Steam.
    for k in 0..2 {
        let sy = y - 14 - ((tick / 5 + k * 7) % 10) as i32;
        cv.put(x - 6 + k as i32 * 12, sy, 'G');
        cv.put(x - 5 + k as i32 * 12, sy - 2, 'j');
    }
}

/// Old Dint the smith: broad, bald, bearded, leather-aproned, hammer up
/// or down.
fn smith(cv: &mut Img, x: i32, y: i32, swing: bool) {
    cv.rect(x - 7, y - 16, 15, 16, 'B');
    cv.rect(x - 5, y - 14, 11, 14, 'P');
    cv.rect(x - 9, y - 15, 3, 8, 't');
    cv.rect(x + 7, y - 15, 3, 8, 't');
    cv.rect(x - 4, y - 24, 9, 8, 't');
    cv.rect(x - 4, y - 19, 9, 5, 'G');
    cv.put(x - 2, y - 21, 'k');
    cv.put(x + 2, y - 21, 'k');
    cv.line(x - 4, y - 24, x + 4, y - 24, 'T');
    // The hammer: raised over the shoulder, or down on the anvil.
    if swing {
        cv.line(x - 8, y - 12, x - 18, y - 9, 'r');
        cv.rect(x - 22, y - 12, 5, 6, 'X');
    } else {
        cv.line(x + 8, y - 14, x + 11, y - 26, 'r');
        cv.rect(x + 8, y - 31, 7, 5, 'X');
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    if !t::PLACES.contains(&kind) {
        return;
    }
    match kind {
        RoomKind::Market => {
            for i in 0..STALLS.len() {
                keeper(cv, run, i);
                let (x, y) = at(STALLS[i].at.0 * 2.0, STALLS[i].at.1 * 2.0);
                if run.town.shut[i] == 0 && !run.town.haggles.values().any(|&(s, _)| usize::from(s) == i) {
                    super::near_prompt(cv, run, (x, y), "HAGGLE");
                }
            }
            // The offer over the counter, live.
            for (&_id, &(s, n)) in &run.town.haggles {
                let i = usize::from(s);
                let (x, y) = at(STALLS[i].at.0 * 2.0, STALLS[i].at.1 * 2.0);
                let gold = t::offer(i, n);
                let label = format!("{}G", gold);
                let w = text_width(&label) + 6;
                let top = gold == STALLS[i].top;
                // Beside the seller, clear of the keeper's face.
                let (lx, ly) = (x + 12, y - 22);
                chrome::board(cv, (lx, ly, w, 11), if top { '5' } else { 'B' });
                cv.text(lx + 3, ly + 2, &label, if top { '5' } else { 'O' });
            }
        }
        RoomKind::Bathhouse => {
            let mut busy = None;
            for (&id, hero) in &run.players {
                if hero.soak == 0 {
                    continue;
                }
                if let Some(tub) = t::tub_of(hero) {
                    let (x, y) = at(TUBS[tub].0 * 2.0, TUBS[tub].1 * 2.0);
                    soaker(cv, x, y, id, run.tick);
                    busy.get_or_insert(x);
                }
            }
            // Brother Suds goes to whoever is soaking, else waits by the copper.
            let (sx, sy) = busy.map_or((20 * TILE, 8 * TILE), |x| (x + 22, 5 * TILE + 2));
            suds(cv, sx, sy, busy.is_some(), run.tick);
            for (i, &(x, y)) in TUBS.iter().enumerate() {
                if run.players.values().any(|h| h.soak > 0 && t::tub_of(h) == Some(i)) {
                    continue;
                }
                // The price under the knight's feet, clear of the tub.
                let (x, y) = at(x * 2.0, (y + 1.6) * 2.0);
                super::near_prompt(cv, run, (x, y + 39), &format!("SOAK {BATH_GOLD}G"));
            }
            // Fresh from the tub: a twinkle or two.
            for hero in run.players.values().filter(|h| h.scrubbed && h.soak == 0) {
                let (x, y) = at(hero.x, hero.y);
                for (n, (dx, dy)) in [(-10, -22), (9, -17), (-4, -30)].into_iter().enumerate() {
                    if (run.tick / 6 + n as u64) % 3 == 0 {
                        continue;
                    }
                    let (sx, sy) = (x + dx, y + dy);
                    cv.put(sx, sy, 'c');
                    for (ox, oy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        cv.put(sx + ox, sy + oy, 'U');
                    }
                }
            }
        }
        _ => {
            let (ax, ay) = at(AT_ANVIL.0 * 2.0, AT_ANVIL.1 * 2.0);
            let hammering = run.town.hammer.is_some_and(|(_, n)| n > 0);
            let swing = hammering && (run.tick / 4).is_multiple_of(2);
            let (_, y, w, _) = px(ANVIL);
            smith(cv, px(ANVIL).0 + w + 14, y + 12, swing);
            if hammering && swing {
                let spark = kit::sparks((run.tick % 6) as u32);
                cv.stamp(&spark, px(ANVIL).0 + w / 2 - spark.w / 2, y - 6);
            }
            let _ = ax;
            // Prices under the knight's feet, clear of the anvil and rack.
            super::near_prompt(cv, run, (ax, ay + 36), &format!("TEMPER {TEMPER_GOLD}G"));
            let (rx, ry) = at(AT_RACK.0 * 2.0, AT_RACK.1 * 2.0);
            super::near_prompt(cv, run, (rx, ry + 36), &format!("MEND {MEND_GOLD}G"));
            // The helmet test: stars round the knight's head.
            if let Some((id, n)) = run.town.ding
                && let Some(hero) = run.players.get(&id)
            {
                let (x, y) = at(hero.x, hero.y);
                for k in 0..3 {
                    let a = (n as f32 / 4.0) + k as f32 * 2.1;
                    let (sx, sy) = (x + (a.cos() * 9.0) as i32, y - 26 + (a.sin() * 3.0) as i32);
                    cv.put(sx, sy, '5');
                    cv.put(sx - 1, sy, '4');
                    cv.put(sx + 1, sy, '4');
                    cv.put(sx, sy - 1, '4');
                    cv.put(sx, sy + 1, '4');
                }
            }
        }
    }
}

/// The room's board (its name, what to do, the treasury) shows while a
/// knight stands by a counter, a tub, the anvil or the rack.
pub(super) fn board_visible(run: &Run) -> bool {
    let spots: Vec<(f32, f32)> = match run.room().kind {
        RoomKind::Market => STALLS.iter().map(|s| s.at).collect(),
        RoomKind::Bathhouse => TUBS.iter().map(|&(x, y)| (x, y + 1.6)).collect(),
        RoomKind::Smithy => vec![AT_ANVIL, AT_RACK],
        _ => return false,
    };
    spots.into_iter().any(|p| super::knight_near(run, p, 3.0))
}

pub(super) fn boards(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    if !board_visible(run) {
        return;
    }
    let mut lines: Vec<(String, char)> = Vec::new();
    match kind {
        RoomKind::Market => {
            lines.push(("HAGGLER'S SQUARE".into(), '9'));
            if run.town.haggles.is_empty() {
                lines.push(("HOLD E: HAGGLE".into(), 'O'));
            } else {
                lines.push(("LET GO: SELL".into(), 'O'));
            }
            let stock: Vec<String> = STALLS
                .iter()
                .map(|s| format!("{} {}", s.word, run.treasury.get(s.buys)))
                .collect();
            lines.push((stock.join("  "), 'h'));
        }
        RoomKind::Bathhouse => {
            lines.push(("THE ROYAL SOAK".into(), '9'));
            lines.push(("SKIP ONE PRIVY".into(), 'O'));
        }
        _ => {
            lines.push(("THE DENTED ANVIL".into(), '9'));
            lines.push(("TEMPER OR MEND".into(), 'O'));
        }
    }
    lines.push((format!("TREASURY {}G", run.treasury.get(crate::realm::Spoil::Gold)), 'h'));
    // The market's board sits low, clear of Magda's and the chandler's
    // counters; indoors it hangs by the door.
    let board = home::board(&lines);
    let y = if kind == RoomKind::Market { cv.h - board.h - 6 } else { 13 };
    cv.stamp(&board, 4, y);
}
