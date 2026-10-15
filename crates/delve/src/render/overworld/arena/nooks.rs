//! The nooks below ground, drawn: Grubb's kitchen, the Quiet Room, the
//! Great Bellows, the Bone Dominoes, Mother Mort's still and the Bell
//! Loft. Furniture is cached scenery; the cook, the books, the heat, the
//! falling bones, the jars and the bells move per frame.
use super::super::ink::{Img, hash};
use super::super::material::{self as mat, ramp};
use super::super::{kit, light::Light};
use super::{TILE, at, fire, home};
#[allow(unused_imports)]
use super::props;
use crate::rules::{
    RoomKind, Run,
    nooks::{self as n, BELLS, CAULDRON, Cook, GAPS, HANDLES, HEAT_FULL, HEAT_IMPS, JARS, LECTERNS, PATH, POT, SCOOPS, SEQS, Show},
};

type Rect = (i32, i32, i32, i32);

fn px((c, r, w, h): Rect) -> (i32, i32, i32, i32) {
    (c * TILE, r * TILE, w * TILE, h * TILE)
}

/// A plank shelf of crocks and jars against the wall.
fn crocks(cv: &mut Img, (c, r, w, h): Rect, seed: u32) {
    let (x, y, w, h) = px((c, r, w, h));
    for (k, sy) in [y + h / 2 - 1, y + h - 3].into_iter().enumerate() {
        cv.rect(x, sy, w, 2, 'r');
        cv.line(x, sy, x + w - 1, sy, 'o');
        let mut cx = x + 2;
        while cx < x + w - 6 {
            let v = hash(cx, k as i32, seed);
            let (jw, jh) = (4 + (v % 3) as i32, 5 + (v / 3 % 4) as i32);
            let ink = ['u', 'B', 'G', 'S', 'P'][(v / 7 % 5) as usize];
            cv.rect(cx, sy - jh, jw, jh, ink);
            cv.put(cx + 1, sy - jh, 'h');
            cx += jw + 2;
        }
    }
    cv.line(x + 1, y, x + 1, y + h - 1, 'B');
    cv.line(x + w - 2, y, x + w - 2, y + h - 1, 'B');
}

/// Dark iron, for pots, anvils and tongs.
pub(super) const IRON: &[char] = &['K', 'X', 'g', 'j', 'G', 'J'];
/// Tanned leather, for the bellows.
pub(super) const LEATHER: &[char] = &['n', 'b', 'I', 'B', 'p', 'P', 'r'];
/// Goblin hide.
pub(super) const HIDE: &[char] = &['D', 'F', 'E', 'l', 'm', 'A', 'M'];
/// Linen: the apron, the toque.
pub(super) const LINEN: &[char] = &['G', 'J', 'h', 'i', 'H'];
/// Fresh-cut coal.
pub(super) const COAL: &[char] = &['K', 'K', 'X', 'g', 'j'];
/// A witch's cloak: dark, cool.
pub(super) const CLOAK: &[char] = &['K', 's', 'x', 'S', 'u'];

pub(super) fn all(_: i32, _: i32) -> bool {
    true
}

/// A fire of split logs, flames dancing on the tick.
pub(super) fn logs(cv: &mut Img, cx: i32, y: i32, tick: u32) {
    for (dx, dy) in [(-7, 0), (2, 1), (-3, -1)] {
        cv.rect(cx + dx, y + dy, 9, 3, 'B');
        cv.line(cx + dx, y + dy, cx + dx + 8, y + dy, 'r');
        cv.put(cx + dx + 8, y + dy + 1, 'o');
    }
    for k in 0..7 {
        let fx = cx - 7 + k * 2;
        let tall = 3 + (hash(k, (tick / 3) as i32, 301) % 5) as i32;
        cv.line(fx, y - 1, fx, y - tall, if k % 2 == 0 { '@' } else { 'a' });
        if tall > 5 {
            cv.put(fx, y - tall, '5');
        }
    }
}

/// An iron pot (or cauldron) on its fire; `stew` the surface's inks, and
/// what bobs in it.
pub(super) fn cauldron(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), stew: (char, char, char), tick: u32) {
    let (cx, cy) = (x + w / 2, y + h / 2);
    mat::shadow(cv, (cx as f32 + 5.0, (cy + 15) as f32), (17.0, 3.5), &all);
    logs(cv, cx, cy + 13, tick);
    // The belly, lit from the north-west, and three stubby feet.
    mat::lobe(cv, (cx as f32, (cy + 2) as f32), (16.0, 11.0), IRON, 41, 3.0, 0.0, &all);
    for fx in [cx - 10, cx + 9] {
        cv.rect(fx, cy + 10, 3, 4, 'X');
    }
    // The rim and the stew.
    cv.ellipse(cx as f32, (cy - 7) as f32, 16.0, 5.0, 'J');
    cv.ellipse(cx as f32, (cy - 6) as f32, 15.0, 4.0, 'g');
    cv.ellipse(cx as f32, (cy - 6) as f32, 13.0, 3.0, stew.0);
    for k in 0..6 {
        let bx = cx - 10 + (hash(k, (tick / 6) as i32, 302) % 20) as i32;
        let by = cy - 7 + (k % 3) as i32;
        cv.put(bx, by, if k % 2 == 0 { stew.1 } else { stew.2 });
        cv.put(bx + 1, by, stew.1);
    }
    // Two lugs and the ladle's hook.
    cv.rect(cx - 18, cy - 6, 3, 3, 'X');
    cv.rect(cx + 16, cy - 6, 3, 3, 'X');
    // Steam, tonal.
    for k in 0..3 {
        let sy = cy - 13 - ((tick / 4 + k * 5) % 12) as i32;
        cv.put(cx - 6 + k as i32 * 6, sy, 'G');
        cv.put(cx - 5 + k as i32 * 6, sy - 2, 'j');
    }
}

/// A trestle table of boards with a bench either side, laid for supper.
pub(super) fn table(cv: &mut Img, (c, r, w, h): Rect, seed: u32) {
    let (x, y, w, h) = px((c, r, w, h));
    mat::shadow(cv, ((x + w / 2 + 4) as f32, (y + h + 5) as f32), (w as f32 * 0.5, 3.0), &all);
    // Benches, then the top over them.
    mat::boards(cv, (x + 2, y - 4, w - 4, 3), false, seed ^ 1);
    mat::boards(cv, (x + 2, y + h + 1, w - 4, 3), false, seed ^ 2);
    mat::boards(cv, (x, y, w, h), false, seed);
    for lx in [x + 3, x + w - 5] {
        cv.rect(lx, y + h, 2, 4, 'b');
    }
    // Trenchers, mugs, a loaf and a candle stub.
    for k in 0..(w / 16) {
        let tx = x + 6 + k * 16;
        cv.ellipse(tx as f32, (y + 7) as f32, 4.0, 2.0, 'G');
        cv.ellipse(tx as f32, (y + 7) as f32, 3.0, 1.0, 'J');
        cv.rect(tx + 6, y + 3, 3, 5, 'B');
        cv.put(tx + 6, y + 3, 'h');
    }
    mat::lobe(cv, ((x + w - 12) as f32, (y + 6) as f32), (5.0, 3.0), ramp::THATCH, seed, 1.5, 0.0, &all);
    cv.rect(x + w / 2, y + 2, 2, 4, 'h');
    cv.put(x + w / 2, y + 1, 'a');
}

/// Bookshelves: spines in timber and stone inks.
fn shelves(cv: &mut Img, (c, r, w, h): Rect) {
    let (x, y, w, h) = px((c, r, w, h));
    cv.rect(x, y, w, h, 'b');
    for xx in x + 1..x + w - 1 {
        let v = hash(xx / 2, r, 311);
        if xx % 2 == 0 && v % 5 != 0 {
            let ink = ['R', 'S', 'P', 'u', 'B', 'I'][(v / 5 % 6) as usize];
            let top = y + 3 + (v % 3) as i32;
            cv.line(xx, top, xx, y + h - 2, ink);
        }
    }
    cv.line(x, y + h - 1, x + w - 1, y + h - 1, 'r');
}

/// A lectern and its open book: unread, read (a quill laid in it), or
/// gone (a mimic took it, torn pages left). A mimic breathes now and then.
fn lectern(cv: &mut Img, (c, r): (i32, i32), state: u8, bob: bool) {
    let (x, y) = (c * TILE, r * TILE);
    mat::shadow(cv, ((x + 11) as f32, (y + 16) as f32), (6.0, 1.5), &all);
    // The stand: a post on a cross foot, a sloped desk.
    cv.rect(x + 7, y + 7, 3, 9, 'B');
    cv.line(x + 7, y + 7, x + 7, y + 15, 'r');
    cv.rect(x + 3, y + 14, 11, 2, 'b');
    mat::boards(cv, (x, y + 2, 17, 5), false, (c * 7 + r) as u32);
    let lift = bob as i32;
    match state {
        2 => {
            for (tx, ty) in [(x + 3, y), (x + 12, y + 1), (x + 8, y - 1)] {
                cv.rect(tx, ty, 3, 2, 'T');
                cv.put(tx + 1, ty + 1, 'o');
            }
        }
        _ => {
            // Open pages either side of the spine, a ribbon hanging.
            let top = y - 3 - lift;
            cv.rect(x + 1, top, 7, 6, 't');
            cv.rect(x + 9, top, 7, 6, 'T');
            cv.line(x + 1, top + 5, x + 7, top + 5, 'o');
            cv.line(x + 9, top + 5, x + 15, top + 5, 'o');
            cv.line(x + 8, top - 1, x + 8, top + 5, 'b');
            for k in 0..3 {
                cv.line(x + 2, top + 1 + k * 2, x + 6, top + 1 + k * 2, 'G');
                cv.line(x + 10, top + 1 + k * 2, x + 14 - k, top + 1 + k * 2, 'G');
            }
            cv.line(x + 8, top + 6, x + 9, top + 9, '7');
            if state == 1 {
                // Read: a quill laid across it.
                cv.line(x + 4, top + 6, x + 13, top - 2, 'h');
                cv.put(x + 13, top - 3, 'i');
            }
        }
    }
}

/// A great leather bellows, nozzle toward the forge: two hinged boards,
/// a pleated leather bag between, handles at the wide end. Plumped, or
/// squashed flat just after a pump. `east` mirrors it.
fn bellows_bag(cv: &mut Img, (c, r, w, h): Rect, squashed: bool, east: bool) {
    let (x0, y, w, h) = px((c, r, w, h));
    let mid = y + h / 2;
    let open = 0.0;
    let span = w - 12;
    let mut im = Img::new(w + 8, h + 16);
    let (ox, om) = (6, h / 2 + 8);
    let lip = if squashed { 5 } else { 13 };
    let _ = open;
    // The leather between the boards, lit from above, pleated in folds.
    for k in 0..span {
        let g = lip * (span - k) / span + 1;
        for dy in -g + 1..g {
            let t = 0.75 - (dy + g) as f32 / (2 * g) as f32 * 0.6;
            let fold = (k % 6) as f32 / 6.0;
            let i = t + if fold < 0.2 { -0.3 } else if fold > 0.8 { 0.15 } else { 0.0 };
            im.put(ox + k, om + dy, mat::pick(LEATHER, i));
        }
    }
    // The boards, tapering to the nozzle: lit above, dark below.
    for k in 0..span {
        let g = lip * (span - k) / span + 1;
        im.put(ox + k, om - g, 'r');
        im.put(ox + k, om - g - 1, 'o');
        im.put(ox + k, om + g, 'b');
        im.put(ox + k, om + g + 1, 'B');
    }
    // The brass nozzle and the handles.
    im.rect(ox + span, om - 2, 9, 4, 'o');
    im.line(ox + span, om - 2, ox + span + 8, om - 2, 'T');
    im.rect(ox - 6, om - lip - 3, 7, 3, 'B');
    im.rect(ox - 6, om + lip + 1, 7, 3, 'B');
    im.line(ox - 6, om - lip - 3, ox, om - lip - 3, 'r');
    let im = if east { im.flip_h() } else { im };
    mat::shadow(cv, ((x0 + w / 2 + 4) as f32, (mid + 12) as f32), (w as f32 * 0.45, 3.0), &all);
    cv.stamp(&im, if east { x0 - 4 } else { x0 - 6 }, mid - om);
}

/// An anvil on its stump: the horn west, the heel east, a hammer on it.
pub(super) fn anvil(cv: &mut Img, x: i32, foot: i32) {
    mat::shadow(cv, ((x + 4) as f32, foot as f32), (13.0, 3.0), &all);
    mat::lobe(cv, (x as f32, (foot - 4) as f32), (8.0, 5.0), ramp::TIMBER, 71, 2.0, 0.0, &all);
    let top = foot - 16;
    cv.rect(x - 4, top + 5, 9, 5, 'X');
    cv.rect(x - 9, top, 19, 5, 'g');
    cv.line(x - 9, top, x + 9, top, 'J');
    cv.line(x - 9, top + 4, x + 9, top + 4, 'K');
    for k in 0..6 {
        cv.line(x - 10 - k, top + 1 + k / 3, x - 10 - k, top + 3, 'g');
    }
    cv.put(x - 16, top + 2, 'J');
    // A hammer laid on the face.
    cv.line(x - 2, top - 2, x + 9, top - 4, 'r');
    cv.rect(x - 5, top - 4, 4, 3, 'X');
}

/// A quench trough: a boarded box of dark water, tongs across it.
pub(super) fn trough(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), tick: u32) {
    mat::shadow(cv, ((x + w / 2 + 4) as f32, (y + h + 2) as f32), (w as f32 * 0.5, 3.0), &all);
    mat::boards(cv, (x, y + 8, w, h - 8), false, 81);
    cv.rect(x, y, w, 9, 'r');
    cv.line(x, y, x + w - 1, y, 'o');
    cv.rect(x + 2, y + 2, w - 4, 6, 'q');
    for k in 0..6 {
        let rx = x + 4 + ((hash(k, (tick / 10) as i32, 83) % (w as u32 - 10)) as i32);
        cv.line(rx, y + 3 + k % 3, rx + 3, y + 3 + k % 3, 'Q');
    }
    cv.line(x + 6, y - 2, x + w - 8, y + 3, 'J');
    cv.line(x + 7, y - 1, x + w - 6, y + 2, 'g');
}

/// A heap of coal with a shovel standing in it.
pub(super) fn coal(cv: &mut Img, x: i32, foot: i32) {
    mat::shadow(cv, ((x + 6) as f32, foot as f32), (22.0, 4.0), &all);
    mat::lobe(cv, (x as f32, (foot - 6) as f32), (20.0, 8.0), COAL, 91, 2.0, 0.25, &all);
    for k in 0..14 {
        let lx = x - 16 + (hash(k, 3, 351) % 32) as i32;
        let ly = foot - 11 + (hash(k, 4, 351) % 9) as i32;
        cv.put(lx, ly, 'J');
    }
    cv.line(x + 8, foot - 10, x + 14, foot - 30, 'r');
    cv.rect(x + 13, foot - 33, 4, 3, 'B');
    cv.rect(x + 5, foot - 11, 7, 4, 'j');
}

/// A skull, lit, with its sockets.
fn skull(cv: &mut Img, x: i32, y: i32) {
    mat::lobe(cv, (x as f32, (y - 1) as f32), (3.5, 3.0), super::props::BONE, (x * 7 + y) as u32, 2.0, 0.0, &all);
    cv.put(x - 1, y - 1, 'k');
    cv.put(x + 1, y - 1, 'k');
    cv.line(x - 1, y + 2, x + 1, y + 2, 'G');
}

/// One bone domino of the line, stood on end with its pips, or fallen the
/// way the line runs.
fn domino(cv: &mut Img, i: usize, fallen: bool) {
    let (c, r) = PATH[i];
    let (x, y) = (c * TILE + TILE / 2, r * TILE + TILE / 2);
    let next = PATH.get(i + 1).copied().unwrap_or((c + 1, r));
    let (dx, dy) = (next.0 - c, next.1 - r);
    if fallen {
        super::props::slab_down(cv, x, y + 3, (dx, dy));
    } else {
        super::props::slab(cv, x, y + 6, ((i % 3) as u8 + 1, ((i * 2 + 1) % 3) as u8 + 1));
    }
}

/// A labelled glass jar: its spoil inside (a bone, a wax candle, an ore
/// lump), a cork in it, its name on a paper label. Feet at `(x, y)`.
fn jar(cv: &mut Img, x: i32, y: i32, j: usize, word: Option<&str>) {
    mat::shadow(cv, ((x + 4) as f32, y as f32), (11.0, 2.5), &all);
    let (top, bottom) = (y - 26, y - 1);
    // The glass: a dark inside, a pale rim, a lit edge on the north-west.
    for yy in top + 3..=bottom {
        let half = if yy < top + 6 { 8 } else { 11 };
        cv.line(x - half + 1, yy, x + half - 1, yy, 's');
        cv.put(x - half, yy, 'U');
        cv.put(x + half, yy, 'u');
    }
    cv.line(x - 10, bottom, x + 10, bottom, 'u');
    cv.line(x - 8, top + 3, x + 8, top + 3, 'v');
    cv.line(x - 9, top + 7, x - 9, bottom - 3, 'V');
    // The cork.
    cv.rect(x - 5, top, 11, 4, 'o');
    cv.line(x - 5, top, x + 5, top, 't');
    // What is inside.
    match j {
        0 => {
            cv.line(x - 6, top + 15, x + 5, top + 9, 'V');
            cv.line(x - 6, top + 16, x + 5, top + 10, 'v');
            for (bx, by) in [(x - 7, top + 15), (x + 5, top + 8)] {
                cv.rect(bx, by, 3, 3, 'V');
            }
        }
        1 => {
            cv.rect(x - 2, top + 7, 5, 11, 'T');
            cv.line(x - 2, top + 7, x - 2, top + 17, 't');
            cv.put(x, top + 6, 'b');
            cv.put(x, top + 5, '5');
            cv.put(x + 1, top + 8, 't');
        }
        _ => {
            mat::lobe(cv, (x as f32, (top + 13) as f32), (6.0, 5.0), ramp::STONE, 101, 1.5, 0.2, &all);
            cv.put(x - 2, top + 11, 'z');
            cv.put(x + 2, top + 14, 'Q');
        }
    }
    // The label.
    if let Some(word) = word {
        let lw = super::super::ink::text_width(word) + 4;
        super::super::chrome::shaped(cv, (x - lw / 2, bottom - 9, lw, 9), &[1], super::super::chrome::Tone::PAINTED, false);
        cv.text(x - lw / 2 + 2, bottom - 8, word, 'b');
    }
}

/// A spoil in the air over a knight: the bone, the candle or the ore lump,
/// on a dark disc so it reads against anything.
fn token(cv: &mut Img, x: i32, y: i32, j: usize) {
    cv.ellipse(x as f32, y as f32, 6.0, 6.0, 'K');
    match j {
        0 => {
            cv.line(x - 3, y + 2, x + 3, y - 2, 'V');
            cv.rect(x - 4, y + 2, 2, 2, 'V');
            cv.rect(x + 3, y - 3, 2, 2, 'V');
        }
        1 => {
            cv.rect(x - 1, y - 2, 3, 6, 'T');
            cv.put(x, y - 3, 'b');
            cv.put(x, y - 4, '5');
        }
        _ => mat::lobe(cv, (x as f32, y as f32), (4.0, 3.0), ramp::STONE, 102, 1.2, 0.2, &all),
    }
}

/// Mother Mort: hooded, hunched, a hooked nose and a wart, knotted hands
/// on a long spoon in the brew.
fn mort(cv: &mut Img, x: i32, y: i32, tick: u32) {
    mat::shadow(cv, ((x + 4) as f32, y as f32), (9.0, 2.0), &all);
    // The cloak, hunched forward.
    mat::lobe(cv, (x as f32, (y - 9) as f32), (9.0, 10.0), CLOAK, 111, 2.5, 0.1, &all);
    // The hood and the face in it.
    mat::lobe(cv, ((x + 1) as f32, (y - 22) as f32), (7.0, 6.0), CLOAK, 112, 2.0, 0.0, &all);
    cv.rect(x - 1, y - 24, 6, 6, 'o');
    cv.rect(x, y - 23, 4, 4, 't');
    cv.put(x + 1, y - 22, 'k');
    cv.put(x + 3, y - 22, 'k');
    cv.line(x + 4, y - 21, x + 6, y - 19, 'o');
    cv.put(x + 6, y - 18, 'R');
    cv.put(x - 1, y - 20, 'P');
    cv.line(x + 1, y - 19, x + 3, y - 19, 'b');
    // Grey hair straggling out.
    cv.line(x - 3, y - 21, x - 4, y - 15, 'h');
    // Hands on the spoon, stirring.
    let a = (tick / 5) as f32 * 0.7;
    let (sx, sy) = (x - 4 + (a.cos() * 3.0) as i32, y + 8);
    cv.line(x + 6, y - 14, sx, sy, 'r');
    cv.rect(x + 4, y - 15, 3, 3, 't');
    cv.rect(x + 1, y - 11, 3, 3, 't');
    cv.put(x + 6, y - 15, 'o');
}

/// Bell `b` on the beam, its rope down to where a ringer stands.
fn bell(cv: &mut Img, b: usize, lit: bool, tick: u32) {
    let (c, r) = BELLS[b];
    let (x, top) = (c * TILE + TILE / 2, r * TILE - 6);
    let swing = if lit { [-2, 0, 2, 0][(tick / 3 % 4) as usize] } else { 0 };
    super::props::bell(cv, x, top, lit, swing, 10);
}

pub(super) fn scenery(cv: &mut Img, run: &Run, lights: &mut Vec<Light>) {
    let tick = run.tick as u32;
    let room = run.room();
    match room.kind {
        RoomKind::Kitchen => {
            cauldron(cv, px(POT), ('P', 'o', 'R'), tick);
            lights.push(fire(12 * TILE, 5 * TILE, 70.0, 0.45));
            crocks(cv, (2, 1, 5, 2), 321);
            cv.stamp(&kit::sacks(8), 18 * TILE + 6, TILE + 10);
            cv.stamp(&kit::barrels(), 19 * TILE + 8, TILE + 2);
            table(cv, (4, 9, 5, 1), 401);
            table(cv, (15, 9, 5, 1), 402);
            // Hams on hooks over the tables.
            for hx in [6 * TILE, 17 * TILE] {
                cv.line(hx, 7 * TILE, hx, 7 * TILE + 5, 'j');
                cv.ellipse(hx as f32, (7 * TILE + 9) as f32, 3.0, 4.0, 'R');
                cv.put(hx, 7 * TILE + 13, 'T');
            }
        }
        RoomKind::Reading => {
            shelves(cv, (2, 1, 7, 1));
            shelves(cv, (15, 1, 7, 1));
            // A long rug down the middle.
            let (x, y) = (10 * TILE + 4, 2 * TILE);
            cv.rect(x, y, 4 * TILE - 8, 11 * TILE, 'b');
            cv.frame(x + 2, y + 2, 4 * TILE - 12, 11 * TILE - 4, 'p');
            for k in 0..5 {
                let (dx, dy) = (x + 4 * TILE / 2 - 4, y + 18 + k * 34);
                for d in 0..6 {
                    cv.line(dx - d, dy - 6 + d, dx + d, dy - 6 + d, 'B');
                    cv.line(dx - d, dy + 6 - d, dx + d, dy + 6 - d, 'B');
                }
            }
            // A sign on the wall: hush.
            super::super::chrome::inset_panel(cv, (11 * TILE, TILE + 2, 2 * TILE, 11), super::super::chrome::Tone::TIMBER);
            cv.text(11 * TILE + 4, TILE + 4, "SHHH", 'T');
            for &(c, r) in &[(3, 6), (20, 6)] {
                let im = kit::candles(3, tick);
                cv.stamp(&im, c * TILE, r * TILE);
                lights.push(fire(c * TILE + 4, r * TILE, 40.0, 0.3));
            }
        }
        RoomKind::Bellows => {
            let (x, y, w, h) = px(n::BELLOWS_FORGE);
            mat::block(cv, (x, y + 8, w, h - 8), ramp::STONE, false, 131);
            // The hood rising to the flue.
            for k in 0..10 {
                let inset = 8 + k;
                for xx in x + inset..x + w - inset {
                    cv.put(xx, y + 9 - k, mat::pick(ramp::STONE, 0.35 + ((xx - x) as f32 / w as f32) * 0.3));
                }
            }
            cv.rect(x + w / 2 - 6, y, 12, 2, 'X');
            cv.rect(x + 14, y + 20, w - 28, 14, 'k');
            cv.line(x + 14, y + 20, x + w - 15, y + 20, 'J');
            lights.push(fire(x + w / 2, y + 30, 80.0, 0.4));
            // The forge's company: an anvil, a quench trough, a coal heap.
            trough(cv, (8 * TILE, 10 * TILE - 2, 4 * TILE, 16), tick);
            anvil(cv, 14 * TILE, 11 * TILE + 4);
            coal(cv, 19 * TILE, 11 * TILE + 6);
        }
        RoomKind::Ossuary => {
            // The chalk line the bones stand on.
            for i in 0..PATH.len() - 1 {
                let (a, b) = (PATH[i], PATH[i + 1]);
                let (x0, y0) = (a.0 * TILE + TILE / 2, a.1 * TILE + TILE / 2 + 7);
                let (x1, y1) = (b.0 * TILE + TILE / 2, b.1 * TILE + TILE / 2 + 7);
                for k in 0..TILE {
                    if k % 4 < 2 {
                        cv.put(x0 + (x1 - x0) * k / TILE, y0 + (y1 - y0) * k / TILE, 'J');
                    }
                }
            }
            // The bone coffer: a block of old bone, skulls along its lid.
            let (x, y, w, h) = px(n::OSSUARY_COFFER);
            mat::block(cv, (x + 1, y + 4, w - 2, h - 4), super::props::BONE, false, 191);
            // Iron bands and a lock: a coffer, not a wall.
            for bx in [x + 6, x + w - 8] {
                cv.rect(bx, y + 4, 2, h - 4, 'X');
            }
            cv.rect(x + w / 2 - 2, y + 14, 4, 4, 'O');
            cv.put(x + w / 2 - 1, y + 16, 'k');
            for k in 0..4 {
                skull(cv, x + 6 + k * 7, y + 4);
            }
            for &(c, r) in &[(7, 6), (16, 6)] {
                mat::shadow(cv, ((c * TILE + 12) as f32, ((r + 2) * TILE) as f32), (8.0, 2.0), &all);
                for k in 0..4 {
                    skull(cv, c * TILE + 5 + (k % 2) * 6, r * TILE + 6 + k * 7);
                }
            }
            // The gaps, chalked: a knight's outline on the flags.
            for &g in &GAPS {
                let (c, r) = PATH[g];
                let (gx, gy) = (c * TILE + 2, r * TILE + 1);
                for k in (0..12).step_by(3) {
                    for (px_, py_) in [(gx + k, gy), (gx + k, gy + 13), (gx, gy + k), (gx + 12, gy + k)] {
                        cv.put(px_, py_, 'h');
                    }
                }
                cv.rect(gx + 4, gy + 3, 5, 4, 'J');
                cv.line(gx + 5, gy + 5, gx + 7, gy + 5, 'k');
                cv.rect(gx + 5, gy + 7, 3, 4, 'J');
            }
            for &(c, r) in &[(2, 6), (21, 6)] {
                lights.push(fire(c * TILE, r * TILE, 40.0, 0.25));
            }
        }
        RoomKind::Still => {
            cauldron(cv, px(CAULDRON), ('C', 'y', 'Y'), tick);
            lights.push(fire(12 * TILE, 5 * TILE, 70.0, 0.4));
            crocks(cv, (2, 1, 5, 2), 331);
            crocks(cv, (17, 1, 5, 2), 332);
            // Herbs drying on a line.
            cv.line(4 * TILE, 4 * TILE, 20 * TILE, 4 * TILE + 2, 'o');
            for k in 0..9 {
                let hx = 4 * TILE + 6 + k * 28;
                cv.line(hx, 4 * TILE + 1, hx - 1, 4 * TILE + 7, if k % 2 == 0 { 'A' } else { 'l' });
                cv.line(hx + 1, 4 * TILE + 1, hx + 2, 4 * TILE + 6, 'M');
            }
        }
        RoomKind::Loft => {
            // The beam the bells hang from, on two posts.
            for px_ in [3 * TILE, 21 * TILE - 4] {
                mat::boards(cv, (px_, 2 * TILE - 8, 4, 4 * TILE), true, 201);
            }
            mat::boards(cv, (3 * TILE, 2 * TILE - 10, 18 * TILE, 5), false, 202);
            mat::shadow(cv, ((12 * TILE) as f32, (2 * TILE) as f32), (9.0 * TILE as f32, 1.5), &all);
            for (c, r, w, h) in [(4, 9, 5, 1), (15, 9, 5, 1)] {
                let (x, y, w, h) = px((c, r, w, h));
                mat::shadow(cv, ((x + w / 2 + 4) as f32, (y + h + 3) as f32), (w as f32 * 0.5, 2.0), &all);
                mat::boards(cv, (x, y - 6, w, 5), false, (c * 13) as u32);
                mat::boards(cv, (x, y + 1, w, h - 1), false, (c * 17) as u32);
                for lx in [x + 2, x + w - 4] {
                    cv.rect(lx, y + h, 2, 3, 'b');
                }
            }
            for &(c, r) in &[(2, 6), (21, 6)] {
                let im = kit::candles(4, tick);
                cv.stamp(&im, c * TILE - 4, r * TILE);
                lights.push(fire(c * TILE, r * TILE, 44.0, 0.3));
            }
        }
        _ => {}
    }
}

/// Grubb the goblin cook. Stirring, he shows the room his hunched green
/// back, apron strings tied in a bow and his long ears out either side of a
/// tall toque. Turning, his ears prick and an eye comes round. Looking, he
/// faces the room: yellow eyes, a long nose, a gappy grin, a stained apron
/// and the ladle up.
fn grubb(cv: &mut Img, x: i32, y: i32, state: Cook, tick: u32) {
    mat::shadow(cv, ((x + 5) as f32, y as f32), (11.0, 2.5), &all);
    let looking = state == Cook::Looking;
    // Short legs and big feet.
    cv.rect(x - 5, y - 6, 4, 5, 'b');
    cv.rect(x + 2, y - 6, 4, 5, 'b');
    cv.rect(x - 7, y - 2, 6, 2, 'I');
    cv.rect(x + 2, y - 2, 6, 2, 'I');
    // The body: a hunched lobe, the hump high on his back.
    mat::lobe(cv, (x as f32, (y - 13) as f32), (9.0, 9.0), HIDE, 121, 2.5, 0.0, &all);
    if !looking {
        mat::lobe(cv, ((x - 1) as f32, (y - 18) as f32), (7.0, 5.0), HIDE, 122, 2.5, 0.0, &all);
        // His back: a spine shadow and the apron's bow.
        cv.line(x, y - 21, x, y - 8, 'F');
        // The nape: a dark fold where the head sinks into the hump.
        cv.line(x - 4, y - 21, x + 4, y - 21, 'D');
        cv.line(x - 3, y - 20, x + 3, y - 20, 'F');
        cv.line(x - 6, y - 9, x + 6, y - 9, 'h');
        for (dx, dy) in [(-3, -1), (-2, 0), (2, 0), (3, -1), (-3, 1), (3, 1)] {
            cv.put(x + dx, y - 9 + dy, 'i');
        }
        cv.line(x - 1, y - 8, x - 2, y - 4, 'h');
        cv.line(x + 1, y - 8, x + 2, y - 4, 'h');
    } else {
        // The apron, stained.
        cv.rect(x - 6, y - 17, 13, 12, 'h');
        cv.line(x - 6, y - 17, x + 6, y - 17, 'i');
        cv.rect(x - 4, y - 21, 9, 4, 'h');
        for (dx, dy, ink) in [(-3, -12, 'P'), (2, -9, 'r'), (0, -14, 'P')] {
            cv.rect(x + dx, y + dy, 2, 2, ink);
        }
    }
    // The head: green, sunk between the shoulders.
    let hy = y - 24;
    mat::lobe(cv, (x as f32, hy as f32), (6.0, 5.0), HIDE, 123, 2.0, 0.0, &all);
    // Ears: long and pointed, pricked up when he turns.
    let prick = if state == Cook::Stirring { 0 } else { 3 + (tick / 2 % 2) as i32 };
    for s in [-1, 1] {
        for k in 0..8 {
            let ex = x + s * (5 + k);
            let ey = hy - 1 - (k * prick) / 6 - k / 3;
            cv.line(ex, ey, ex, ey + 2 - k / 4, if k < 6 { 'l' } else { 'A' });
        }
        cv.put(x + s * 7, hy, 'm');
    }
    match state {
        Cook::Looking => {
            cv.rect(x - 4, hy - 2, 3, 3, 'Y');
            cv.rect(x + 2, hy - 2, 3, 3, 'Y');
            cv.put(x - 3, hy - 1, 'k');
            cv.put(x + 3, hy - 1, 'k');
            cv.line(x - 5, hy - 4, x - 1, hy - 3, 'D');
            cv.line(x + 5, hy - 4, x + 1, hy - 3, 'D');
            cv.line(x, hy, x + 1, hy + 4, 'm');
            cv.line(x - 3, hy + 3, x + 3, hy + 3, 'k');
            for tx in [x - 2, x + 2] {
                cv.put(tx, hy + 3, 'i');
            }
            // Hands and the ladle, raised.
            cv.rect(x - 11, y - 15, 3, 3, 'm');
            cv.rect(x + 9, y - 21, 3, 3, 'm');
            cv.line(x + 10, y - 22, x + 13, y - 36, 'J');
            mat::lobe(cv, ((x + 14) as f32, (y - 38) as f32), (3.5, 2.5), IRON, 124, 1.5, 0.0, &all);
            cv.put(x + 14, y - 34, 'P');
        }
        Cook::Turning => {
            // One eye round, wide.
            cv.rect(x + 3, hy - 1, 3, 3, 'Y');
            cv.put(x + 4, hy, 'k');
            cv.line(x + 5, hy + 1, x + 7, hy + 3, 'm');
        }
        Cook::Stirring => {}
    }
    // The toque.
    cv.rect(x - 5, hy - 10, 11, 6, 'i');
    cv.line(x - 5, hy - 4, x + 5, hy - 4, 'J');
    mat::lobe(cv, (x as f32, (hy - 12) as f32), (7.0, 4.0), LINEN, 125, 2.0, 0.1, &all);
    if !looking {
        // His stirring arm and the ladle going round in the pot.
        let a = (tick / 4) as f32 * 0.8;
        cv.rect(x + 7, y - 16, 3, 3, 'm');
        cv.line(x + 8, y - 14, x + 4 + (a.cos() * 6.0) as i32, y + 6 + (a.sin() * 2.0) as i32, 'J');
    }
    if state == Cook::Turning {
        cv.rect(x + 12, hy - 18, 7, 12, 'K');
        cv.rect(x + 14, hy - 16, 3, 6, '6');
        cv.rect(x + 14, hy - 9, 3, 2, '6');
    }
}

/// The choirmaster, his baton on the bell he sounds.
fn choirmaster(cv: &mut Img, x: i32, y: i32, pointing: Option<usize>, tick: u32) {
    let aim = pointing.map(|b| (BELLS[b].0 * TILE + TILE / 2, BELLS[b].1 * TILE));
    super::props::ghost(cv, x, y, aim, tick);
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let Some(nook) = run.nook.as_ref().filter(|nk| nk.room == run.at) else {
        return;
    };
    let tick = run.tick as u32;
    match run.room().kind {
        RoomKind::Kitchen => {
            let (x, y) = at(n::KITCHEN_STOVE.0 * 2.0, n::KITCHEN_STOVE.1 * 2.0);
            let state = if nook.pot == 0 { Cook::Stirring } else { n::cook(nook) };
            grubb(cv, x, y + 2, state, tick);
            // The stew's level, as coins glinting in the pot.
            let (px_, py, pw, _) = px(POT);
            for k in 0..nook.pot.min(SCOOPS) as i32 / 3 {
                cv.put(px_ + 8 + k * 5, py + 10, '6');
            }
            if nook.pot > 0 && state != Cook::Looking {
                // Beside the pot, clear of it and of the knights.
                super::near_prompt(cv, run, (px_ + pw / 2 - 38, py + 46), "SCOOP");
            }
        }
        RoomKind::Reading => {
            for (i, &l) in LECTERNS.iter().enumerate() {
                let state = if nook.opened & 1 << i == 0 {
                    0
                } else if nook.mimics & 1 << i != 0 {
                    2
                } else {
                    1
                };
                // A mimic breathes now and then: the only tell.
                let bob = state == 0 && nook.mimics & 1 << i != 0 && (tick + i as u32 * 17) % (2 * 30) < 4;
                lectern(cv, l, state, bob);
                if state == 0 {
                    let (sx, sy) = at(n::lectern_stand(i).0 * 2.0, n::lectern_stand(i).1 * 2.0);
                    super::near_prompt(cv, run, (sx, sy + 36), "READ");
                }
            }
        }
        RoomKind::Bellows => {
            bellows_bag(cv, (4, 2, 3, 2), nook.last == 1, false);
            bellows_bag(cv, (17, 2, 3, 2), nook.last == 2, true);
            // The hearth's coals, as hot as the heat.
            let (x, y, w, _) = px(n::BELLOWS_FORGE);
            let hot = nook.heat * 3 / HEAT_FULL;
            for k in 0..10 {
                let cx = x + 16 + (hash(k, 9, 341) % (w as u32 - 34)) as i32;
                let ink = match hot {
                    0 => 'g',
                    1 => 'a',
                    2 => '@',
                    _ => '6',
                };
                cv.rect(cx, y + 28 + (k % 3) as i32 * 2, 3, 2, ink);
            }
            // The heat gauge on the hood: a mark at the imps, full at the top.
            let (gx, gy) = (x + w - 8, y + 10);
            cv.rect(gx, gy, 4, 24, 'K');
            let fill = (nook.heat * 22 / HEAT_FULL) as i32;
            cv.rect(gx + 1, gy + 23 - fill, 2, fill, if nook.heat >= HEAT_IMPS { '@' } else { 'a' });
            cv.put(gx - 1, gy + 23 - (HEAT_IMPS * 22 / HEAT_FULL) as i32, 'h');
            for (s, &(hx, hy)) in HANDLES.iter().enumerate() {
                let (hx, hy) = at(hx * 2.0, hy * 2.0);
                cv.line(hx, hy - 4, hx, hy - 18, 'r');
                cv.rect(hx - 4, hy - 20, 9, 3, 'B');
                let next = nook.last != s as u8 + 1;
                if !nook.done && next {
                    super::near_prompt(cv, run, (hx, hy + 36), "PUMP");
                }
            }
        }
        RoomKind::Ossuary => {
            for i in 0..PATH.len() {
                let fallen = i < usize::from(nook.fallen);
                if n::is_gap(i) {
                    if fallen {
                        // A knight fell here: a dented helm on the flags.
                        let (c, r) = PATH[i];
                        cv.rect(c * TILE + 4, r * TILE + 6, 8, 5, 'J');
                        cv.line(c * TILE + 5, r * TILE + 8, c * TILE + 10, r * TILE + 8, 'k');
                    }
                    continue;
                }
                domino(cv, i, fallen);
            }
            if nook.done {
                let (x, y, w, _) = px(n::OSSUARY_COFFER);
                cv.rect(x + 4, y - 2, w - 8, 6, 'U');
                for k in 0..5 {
                    cv.put(x + 7 + k * 4, y + 6, '6');
                }
            } else if !nook.running && nook.stalled == 0 {
                let (sx, sy) = at(n::domino_at(0).0 * 2.0, n::domino_at(0).1 * 2.0);
                super::near_prompt(cv, run, (sx, sy + 36), "TOPPLE");
            }
        }
        RoomKind::Still => {
            let (x, y) = at(n::STILL_MORT.0 * 2.0, n::STILL_MORT.1 * 2.0);
            mort(cv, x, y, tick);
            for (j, &((jx, jy), _, word)) in JARS.iter().enumerate() {
                let (x, y) = at(jx * 2.0, jy * 2.0);
                jar(cv, x, y - 20, j, Some(word));
                super::near_prompt(cv, run, (x, y + 36), word);
            }
            // A knight's first spoil floats over their head until the second.
            for (&id, &j) in &nook.jars {
                if let Some(hero) = run.players.get(&id) {
                    let (x, y) = at(hero.x, hero.y);
                    token(cv, x + 13, y - 16, usize::from(j - 1));
                }
            }
        }
        RoomKind::Loft => {
            let shown = n::show(nook);
            let ringing = match shown {
                Show::Ringing(b) => b.map(usize::from),
                Show::YourTurn => None,
            };
            let lit = nook.lit.map(|(b, _)| usize::from(b));
            for b in 0..BELLS.len() {
                bell(cv, b, Some(b) == ringing || Some(b) == lit, tick);
            }
            choirmaster(cv, 12 * TILE, 7 * TILE + 4, ringing, tick);
            if !nook.done && shown == Show::YourTurn {
                for b in 0..BELLS.len() {
                    let (x, y) = at(n::loft_under(b).0 * 2.0, n::loft_under(b).1 * 2.0);
                    super::near_prompt(cv, run, (x, y + 36), "RING");
                }
            }
        }
        _ => {}
    }
}

pub(super) fn boards(cv: &mut Img, run: &Run) {
    let Some(nook) = run.nook.as_ref().filter(|nk| nk.room == run.at) else {
        return;
    };
    let mut lines: Vec<(String, char)> = Vec::new();
    match run.room().kind {
        RoomKind::Kitchen => {
            lines.push(("GRUBB'S KITCHEN".into(), '9'));
            lines.push(("E: SCOOP STEW".into(), 'O'));
            lines.push(match (nook.pot, n::cook(nook)) {
                (0, _) => ("POT EMPTY".into(), 'h'),
                (_, Cook::Looking) => ("FREEZE!".into(), '6'),
                (_, Cook::Turning) => ("HE'S TURNING!".into(), '6'),
                (left, _) => (format!("{left} SCOOPS LEFT"), 'h'),
            });
        }
        RoomKind::Reading => {
            lines.push(("THE QUIET ROOM".into(), '9'));
            lines.push(("E: READ".into(), 'O'));
            let real = !nook.mimics & ((1 << LECTERNS.len()) - 1);
            lines.push((format!("{} OF {} READ", (nook.opened & real).count_ones(), real.count_ones()), 'h'));
        }
        RoomKind::Bellows => {
            lines.push(("THE GREAT BELLOWS".into(), '9'));
            lines.push(("PUMP IN TURN".into(), 'O'));
            lines.push(if nook.done { ("ARMS TEMPERED".into(), 'h') } else { (format!("HEAT {}%", nook.heat * 100 / HEAT_FULL), 'h') });
        }
        RoomKind::Ossuary => {
            lines.push(("THE BONE DOMINOES".into(), '9'));
            lines.push(("E: TOPPLE".into(), 'O'));
            lines.push(if nook.done { ("THE COFFER RANG".into(), 'h') } else { ("FILL THE GAPS".into(), 'h') });
        }
        RoomKind::Still => {
            lines.push(("MOTHER MORT'S STILL".into(), '9'));
            lines.push(("PICK TWO JARS".into(), 'O'));
            lines.push(("BONE WAX ORE".into(), 'h'));
        }
        RoomKind::Loft => {
            lines.push(("THE BELL LOFT".into(), '9'));
            lines.push(("RING THE TUNE".into(), 'O'));
            lines.push(if nook.done {
                ("THE HYMN IS SUNG".into(), 'h')
            } else if n::show(nook) == Show::YourTurn {
                (format!("YOUR TURN {}/{}", nook.pos, SEQS[usize::from(nook.round)].len()), '6')
            } else {
                (format!("LISTEN  TUNE {}/3", nook.round + 1), 'h')
            });
        }
        _ => return,
    }
    let board = home::board(&lines);
    // The bellows' west bag stands where the board would: it goes low.
    let y = if run.room().kind == RoomKind::Bellows { cv.h - board.h - 6 } else { 13 };
    cv.stamp(&board, 4, y);
}
