//! The bazaar drawn: a night market under the Sozzled Griffin's east wall,
//! flagstones and awnings, six stalls of odd vendors, a well that grants
//! nothing, a complaints box and an oracle in a fishbowl. Its wares are
//! the run's stock (`bazaar::stock`), drawn on the figures' layer, since
//! the still scenery is shared by every run that has this hall.

use super::super::ink::{Img, hash, text_width};
use super::super::kit::{self, RockKind, Tiles};
use super::super::light::Light;
use super::{TILE, at, dusk, fire, stand};
use crate::rules::bazaar::{
    BACK_WALL, COMPLAINTS, FISHBOWL, STALL, STALLS, Stall, WARES, WELL,
};
use crate::rules::curios::Icon;
use crate::rules::{Run, Tile};

const T: i32 = TILE;

/// The market at rest.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let (pw, ph) = (room.cols as i32 * T, room.rows as i32 * T);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    flags(&mut cv, run);
    let tiles = Tiles::get();
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            if room.tile(col, row) == Tile::Wall {
                cv.stamp(tiles.rock(RockKind::Slate, hash(col, row, 29)), col * T, row * T);
            }
        }
    }
    back_wall(&mut cv, &mut lights);
    for (i, s) in STALL.iter().enumerate() {
        stall(&mut cv, s, i as i32, &mut lights);
    }
    well(&mut cv, &mut lights);
    complaints(&mut cv);
    fishbowl(&mut cv, &mut lights);
    doors(&mut cv, &mut lights);
    for (col, row) in [(1, 6), (30, 6), (1, 15), (30, 15), (13, 12), (18, 12)] {
        let lamp = kit::lantern(true);
        let (x, y) = (col * T + (T - lamp.w) / 2, row * T - 6);
        cv.stamp(&lamp, x, y);
        lights.push(fire(x + 3, y + 3, 62.0, 0.4));
    }
    dusk(&mut cv, &lights, ('j', 'o'));
    cv
}

/// Flagstones, each its own grey, a few of them cracked, with plank
/// walkways under the stalls.
fn flags(cv: &mut Img, run: &Run) {
    let room = run.room();
    for y in 0..cv.h {
        for x in 0..cv.w {
            if room.tile(x / T, y / T) == Tile::Wall {
                continue;
            }
            let (col, row) = (x / 16, y / 16);
            let (lx, ly) = (x % 16, y % 16);
            let stone = hash(col, row, 11) % 5;
            let edge = lx == 0 || ly == 0;
            let ink = if edge {
                'X'
            } else if lx == 1 || ly == 1 {
                match stone {
                    0 | 1 => 'G',
                    _ => 'j',
                }
            } else if (lx + ly + stone as i32) % 11 == 0 {
                'g'
            } else {
                match stone {
                    0 => 'j',
                    1 => 'g',
                    2 => 'G',
                    3 => 'j',
                    _ => 'g',
                }
            };
            cv.put(x, y, ink);
        }
    }
}

fn back_wall(cv: &mut Img, lights: &mut Vec<Light>) {
    let (c, r, w, h) = BACK_WALL;
    let (x0, y0, x1, y1) = (c * T, r * T, (c + w) * T, (r + h) * T);
    for y in y0..y1 {
        for x in x0..x1 {
            let course = (y - y0) / 6;
            let off = if course % 2 == 0 { 0 } else { 8 };
            let mortar = (y - y0) % 6 == 0 || (x - x0 + off) % 16 == 0;
            cv.put(x, y, if mortar { 'X' } else if hash((x - x0 + off) / 16, course, 3) % 3 == 0 { 'S' } else { 'x' });
        }
    }
    cv.line(x0, y1 - 1, x1 - 1, y1 - 1, 'k');
    // Bunting along the wall, one pennant to a tile.
    let cols = ['R', 'O', 'z', 'E', 'T', 'R'];
    for k in 0..(x1 - x0) / 8 {
        let px = x0 + k * 8;
        let sag = ((k as f32 * 0.35).sin().abs() * 2.0) as i32;
        cv.put(px, y0 + 1 + sag, 'g');
        cv.put(px + 1, y0 + 1 + sag, 'g');
        let flag = cols[(hash(k, 1, 19) % cols.len() as u32) as usize];
        cv.rect(px + 1, y0 + 2 + sag, 5, 3, flag);
        cv.put(px + 2, y0 + 5 + sag, flag);
        cv.put(px + 3, y0 + 5 + sag, flag);
    }
    for k in (6..30).step_by(8) {
        lights.push(fire(x0 + k * T / 2 + 8, y0 + 8, 44.0, 0.18));
    }
}

/// A stall: planks under it, the counter with its cloth hung in front, the
/// awning over the vendor, and the posts that hold it all up.
fn stall(cv: &mut Img, s: &Stall, n: i32, lights: &mut Vec<Light>) {
    let (c, r, w, h) = s.counter;
    let (x, y, sw, sh) = (c * T, r * T, w * T, h * T);
    // The planks the vendor stands on.
    cv.rect(x - 4, y - 14, sw + 8, 14, 'I');
    for px in (x - 4..x + sw + 4).step_by(8) {
        cv.line(px, y - 14, px, y - 1, 'b');
    }
    // The counter.
    cv.rect(x, y, sw, sh, 'P');
    cv.rect(x, y, sw, 8, 'r');
    cv.line(x, y, x + sw - 1, y, 'T');
    cv.line(x, y + 1, x + sw - 1, y + 1, 'O');
    cv.line(x, y + 7, x + sw - 1, y + 7, 'n');
    // The cloth hung over the front, in the awning's two colours.
    for px in x + 2..x + sw - 2 {
        let stripe = ((px - x) / 6) % 2 == 0;
        let ink = if stripe { s.awning.0 } else { s.awning.1 };
        cv.rect(px, y + 8, 1, sh - 8, ink);
        if (px - x) % 6 == 5 || (px - x) % 6 == 0 {
            cv.put(px, y + sh - 1, 'n');
        }
    }
    cv.line(x + 2, y + sh - 1, x + sw - 3, y + sh - 1, 'n');
    // Posts and the awning over the vendor.
    let top = y - 14 - 22 + (n % 2) * 2;
    for px in [x - 2, x + sw - 1] {
        cv.rect(px, top, 3, y - top, 'B');
        cv.line(px, top, px, y - 1, 'r');
    }
    for px in x - 4..x + sw + 4 {
        let stripe = ((px - x + 4) / 6) % 2 == 0;
        let ink = if stripe { s.awning.0 } else { s.awning.1 };
        let hem = if (px - x) % 6 == 1 || (px - x) % 6 == 2 { 9 } else { 7 };
        for yy in 0..hem {
            cv.put(px, top + yy, ink);
        }
        cv.put(px, top + hem, 'n');
    }
    cv.line(x - 4, top, x + sw + 3, top, 'T');
    lights.push(fire(x + sw / 2, y - 12, 54.0, 0.34));
}

fn well(cv: &mut Img, lights: &mut Vec<Light>) {
    let (c, r, w, h) = WELL;
    let (x, y, ww, wh) = (c * T, r * T, w * T, h * T);
    cv.rect(x, y + 4, ww, wh - 4, 'G');
    cv.frame(x, y + 4, ww, wh - 4, 'g');
    for k in 0..(ww / 8) {
        cv.line(x + k * 8, y + 5, x + k * 8, y + wh - 3, 'j');
    }
    cv.line(x + 1, y + 5, x + ww - 2, y + 5, 'h');
    cv.ellipse((x + ww / 2) as f32, (y + 17) as f32, ww as f32 / 2.0 - 5.0, 9.0, 'k');
    cv.ellipse((x + ww / 2) as f32, (y + 17) as f32, ww as f32 / 2.0 - 8.0, 7.0, 'q');
    cv.ellipse((x + ww / 2 - 6) as f32, (y + 15) as f32, 5.0, 2.0, 'z');
    // A beam and a bucket on a rope, and the coins at the bottom.
    cv.rect(x + 4, y - 14, 3, 20, 'B');
    cv.rect(x + ww - 7, y - 14, 3, 20, 'B');
    cv.rect(x + 4, y - 15, ww - 8, 3, 'r');
    cv.line(x + ww / 2, y - 12, x + ww / 2, y + 2, 'g');
    cv.rect(x + ww / 2 - 3, y + 2, 7, 5, 'o');
    cv.frame(x + ww / 2 - 3, y + 2, 7, 5, 'n');
    for (cx, cy) in [(-8, 20), (6, 22), (1, 18), (10, 17)] {
        cv.put(x + ww / 2 + cx, y + cy, 't');
    }
    // The sign.
    cv.text(x + 4, y + wh + 3, "NO WISHES", 'o');
    lights.push(fire(x + ww / 2, y + 10, 46.0, 0.22));
}

fn complaints(cv: &mut Img) {
    let (c, r, w, h) = COMPLAINTS;
    let (x, y, bw, bh) = (c * T, r * T, w * T, h * T);
    cv.rect(x + 2, y + 4, bw - 4, bh - 6, 'p');
    cv.frame(x + 2, y + 4, bw - 4, bh - 6, 'R');
    cv.rect(x + 4, y + 9, bw - 8, 2, 'k');
    cv.rect(x + 4, y + 2, bw - 8, 3, 'r');
    cv.text(x + 1, y - 7, "COMPLAINTS", 'o');
    // Papers sticking out of it.
    cv.rect(x + 5, y + 6, 4, 3, 'T');
    cv.put(x + 6, y + 5, 'i');
}

fn fishbowl(cv: &mut Img, lights: &mut Vec<Light>) {
    let (c, r, w, h) = FISHBOWL;
    let (x, y, bw, bh) = (c * T, r * T, w * T, h * T);
    cv.rect(x + 8, y + 18, bw - 16, bh - 18, 'G');
    cv.rect(x + 6, y + bh - 4, bw - 12, 4, 'g');
    cv.ellipse((x + bw / 2) as f32, (y + 12) as f32, 12.0, 11.0, 'S');
    cv.ellipse((x + bw / 2) as f32, (y + 12) as f32, 10.0, 9.0, 'q');
    cv.ellipse((x + bw / 2 - 3) as f32, (y + 8) as f32, 3.0, 2.0, 'z');
    cv.put(x + bw / 2 + 4, y + 15, 'R');
    cv.rect(x + bw / 2 - 1, y + 10, 5, 3, 'R');
    cv.put(x + bw / 2 + 5, y + 11, 'R');
    cv.put(x + bw / 2 - 2, y + 11, 'k');
    cv.text(x - 6, y - 9, "ORACLE", 'o');
    lights.push(fire(x + bw / 2, y + 12, 34.0, 0.18));
}

/// West to the tavern, south to the kingdom.
fn doors(cv: &mut Img, lights: &mut Vec<Light>) {
    // West: an arch in the wall, warm light beyond it.
    let (x, y, dw, dh) = (2, 8 * T - 2, 14, 3 * T + 4);
    cv.rect(x, y, dw, dh, 'k');
    cv.rect(x + 2, y + 2, dw - 2, dh - 4, 'b');
    cv.frame(x, y - 1, dw + 1, dh + 1, 'P');
    cv.rect(x, y - 6, dw + 1, 5, 'P');
    cv.text(x - 1, y - 14, "TAVERN", 'O');
    lights.push(fire(x + dw, y + dh / 2, 56.0, 0.46));
    // South: the gate, night beyond.
    let (gx, gy, gw, gh) = (15 * T - 4, 17 * T - 4, 2 * T + 8, T + 4);
    cv.rect(gx, gy, gw, gh, 'k');
    cv.rect(gx + 3, gy + 2, gw - 6, gh - 2, 'x');
    for (sx, sy) in [(6, 5), (17, 4), (28, 6)] {
        cv.put(gx + sx, gy + sy, 'w');
    }
    cv.frame(gx, gy, gw, gh, 'P');
    cv.rect(gx, gy - 4, gw, 4, 'P');
    cv.text(gx + 4, gy - 12, "KINGDOM", 'O');
    let lamp = kit::lantern(true);
    cv.stamp(&lamp, gx - 5, gy - 6);
    lights.push(fire(gx - 2, gy - 3, 50.0, 0.4));
}

// ---- the vendors ---------------------------------------------------------

const GNOME: [&str; 17] = [
    "....7.......",
    "...777......",
    "...7777.....",
    "..777777....",
    "..OOOOOO....",
    "..OKOOKO....",
    "..HHHHHH....",
    ".HHHHHHHH...",
    ".HHHHHHHH...",
    "..EEEEEE.Ob.",
    ".EEEEEEEEOb.",
    ".EEEEEEEE.b.",
    "..EE..EE....",
    "..EE..EE....",
    "..II..II....",
    "..II..II....",
    ".nnn..nnn...",
];

const GOAT: [&str; 18] = [
    "..kkkkkk....",
    "..kkkkkk....",
    ".kkkkkkkk...",
    "..iHHHHi.h..",
    ".hiHKHHKih..",
    "..iHHHHHi...",
    "...iHrrHi...",
    "...iHHHHi...",
    "....HHHH....",
    "..BBBBBBBB..",
    ".BBTTTTTTBB.",
    ".BBTTTTTTBB.",
    "HHBTTTTTTBHH",
    "..BTTTTTTB..",
    "..HH....HH..",
    "..HH....HH..",
    "..hh....hh..",
    "..kk....kk..",
];

const BARREL: [&str; 19] = [
    "......q.....",
    ".....qqq....",
    "....qqqqq...",
    "...qqqqqqq..",
    "..qQqqqqqQq.",
    ".qqqqqqqqqqq",
    "...HHHHHH...",
    "..BBBBBBBB..",
    "..BkBBBBkB..",
    "..BBBkkBBB..",
    "..HHBBBBHH..",
    "..HHHHHHHH..",
    "..BBBBBBBB..",
    "..rrBBBBrr..",
    "..BBBBBBBB..",
    "..rrrrrrrr..",
    "..BBBBBBBB..",
    "..nnnnnnnn..",
    "............",
];

const PILGRIM: [&str; 18] = [
    "...PPPPP....",
    "..PPPPPPP...",
    ".PPPOOOPPP..",
    ".PPOKOKOPP..",
    ".PPOOOOOPP..",
    ".PPPrrrPPP..",
    "..PPPPPPP.o.",
    ".PPPPPPPPPO.",
    ".PPPPPPPPPO.",
    "PPPTTPPPPPo.",
    "PPTTTPPPPPo.",
    "PPPTTPPPPPo.",
    ".PPPPPPPPP..",
    ".PPPPPPPPP..",
    ".PPPPPPPPP..",
    ".PPPPPPPPP..",
    "..nn...nn...",
    "..nn...nn...",
];

const CHILDREN: [&str; 20] = [
    "....OOOO....",
    "...OOOOOO...",
    "...OKOOKO...",
    "...OOrrOO...",
    "....OOOO....",
    "..ooooooo...",
    ".ooOOKOOKoo.",
    ".ooOOOOOOoo.",
    ".ooOOrrOOoo.",
    ".oooooooooo.",
    ".ooooTToooo.",
    ".ooooTToooo.",
    ".oooooooooo.",
    ".oooooooooo.",
    ".oooooooooo.",
    ".oooooooooo.",
    ".oooooooooo.",
    "..ooo..ooo..",
    "..II....II..",
    "..nn....nn..",
];

const MOLE: [&str; 15] = [
    "............",
    "...XXXXXX...",
    "..XXXXXXXX..",
    ".XXkTXXTkXX.",
    ".XXXXXXXXXX.",
    ".XXXXRRXXXX.",
    "..XXXXXXXX..",
    "..zzzzzzzz..",
    ".zzzzTTzzzz.",
    "XXzzzzzzzzXX",
    "XXzzzzzzzzXX",
    ".XXzzzzzzXX.",
    "..XX....XX..",
    "..XX....XX..",
    ".XXX....XXX.",
];

const VENDORS: [&[&str]; STALLS] = [&GNOME, &GOAT, &BARREL, &PILGRIM, &CHILDREN, &MOLE];

// ---- the wares' little pictures ------------------------------------------

fn icon_rows(icon: Icon) -> &'static [&'static str] {
    match icon {
        Icon::Boot => &[
            ".kkk.....",
            ".kMk.....",
            ".kMk.....",
            ".kMMkkk..",
            ".kMMMMMk.",
            "kMMMMMMMk",
            "kLMMMMMMk",
            "kDDDDDDDk",
            ".kkkkkkk.",
        ],
        Icon::Hat => &[
            "....k....",
            "...kMk...",
            "..kMMMk..",
            "..kMLMk..",
            ".kMMMMMk.",
            "kMMMMMMMk",
            "kDDDDDDDk",
            ".kkkkkkk.",
            ".........",
        ],
        Icon::Spoon => &[
            "......kk.",
            ".....kLMk",
            "....kMMMk",
            "....kMMk.",
            "...kMk...",
            "..kMk....",
            ".kMk.....",
            "kDk......",
            ".k.......",
        ],
        Icon::Jar => &[
            "..kkkkk..",
            "..kDDDk..",
            ".kLMMMMk.",
            "kLMMMMMMk",
            "kMMLMMMMk",
            "kMMMMMMMk",
            "kMMMMMDDk",
            ".kDDDDDk.",
            "..kkkkk..",
        ],
        Icon::Bell => &[
            "....k....",
            "...kMk...",
            "..kLMMk..",
            ".kLMMMMk.",
            ".kMMMMMk.",
            "kMMMMMMMk",
            "kDDDDDDDk",
            ".kkkkkkk.",
            "...kDk...",
        ],
        Icon::Ring => &[
            "...kLk...",
            "..kLLLk..",
            "...kMk...",
            "..kMMMk..",
            ".kM...Mk.",
            "kM.....Mk",
            "kM.....Mk",
            ".kD...Dk.",
            "..kkkkk..",
        ],
        Icon::Scroll => &[
            "kkkkkkkk.",
            "kLLLLLLMk",
            ".kMMMMMk.",
            ".kMkkkMk.",
            ".kMMMMMk.",
            ".kMkkMMk.",
            ".kMMMMMk.",
            "kDDDDDDDk",
            ".kkkkkkk.",
        ],
        Icon::Sack => &[
            "...kkk...",
            "..kMMMk..",
            "...kDk...",
            "..kMMMk..",
            ".kLMMMMk.",
            "kLMMMMMMk",
            "kMMMMMMMk",
            "kMMMMMDDk",
            ".kkkkkkk.",
        ],
    }
}

/// The two inks a curio is drawn in, by its name: main, dark, light.
fn inks(id: &str) -> (char, char, char) {
    const PAIRS: [(char, char, char); 7] = [
        ('Q', 'q', 'z'),
        ('R', 'p', 'O'),
        ('E', 'F', 'C'),
        ('o', 'B', 'T'),
        ('U', 'S', 'V'),
        ('L', 'N', 'Y'),
        ('P', 'I', 'r'),
    ];
    let h = id.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    PAIRS[(h % PAIRS.len() as u32) as usize]
}

/// A curio's little picture.
pub(super) fn icon_of(curio: &crate::rules::curios::Curio) -> Img {
    let (m, d, l) = inks(curio.id);
    Img::from_rows(icon_rows(curio.icon)).recolor(&[('M', m), ('D', d), ('L', l)])
}

/// The vendors behind their counters, each ware on its cloth with its
/// price beneath, and the market's small life.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let stock = run.bazaar_stock();
    for (i, s) in STALL.iter().enumerate() {
        let (vx, vy) = at(s.vendor.0 * 2.0, s.vendor.1 * 2.0);
        let bob = ((tick / 30 + i as u32 * 7) % 4 == 0) as i32;
        let sprite = Img::from_rows(VENDORS[i]);
        stand(cv, &sprite, vx, vy - bob);
        let (_, r, _, _) = s.counter;
        for ware in 0..WARES {
            let slot = i * WARES + ware;
            let Some(offer) = stock.get(slot) else { continue };
            let curio = offer.curio();
            let icon = icon_of(curio);
            let (sx, _) = at(s.stands[ware].0 * 2.0, 0.0);
            let ix = sx - icon.w / 2;
            // Set on the counter's top, with a price on the cloth below.
            cv.stamp(&icon, ix, r * T - 2 - icon.h + 8);
            let price = offer.price.to_string();
            let px = sx - text_width(&price) / 2;
            cv.rect(px - 1, r * T + 12, text_width(&price) + 2, 9, 'n');
            cv.text(px, r * T + 13, &price, 'T');
        }
    }
    // The well's water glints; a coin, now and then, lands in it.
    let (wc, wr, ww, _) = WELL;
    let glint = (tick / 14) % 5;
    cv.put(wc * T + ww * T / 2 - 8 + glint as i32 * 3, wr * T + 16, 'V');
    // The fish, circling.
    let (fc, fr, fw, _) = FISHBOWL;
    let swim = ((tick / 10) % 8) as i32;
    cv.put(fc * T + fw * T / 2 - 6 + swim, fr * T + 12, 'R');
}
