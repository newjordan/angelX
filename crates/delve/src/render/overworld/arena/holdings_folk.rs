//! The village drawn: the named folk of a holding walking about its steward's
//! hall and its cross-section at the Court, at the trade the ledger gives
//! them, and what the village does when the realm's last rent day made
//! something of it — a feast laid at the table, a tantrum among the
//! splinters, a bier carried the length of the hall, a workshop lit by a
//! fey mood and the masterwork that comes out of it.
//!
//! Everything here is read from the holding (`rules::strolls` says where
//! each soul is): a figure is somebody in the ledger, a name chip is their
//! name, a grey smock is a hungry or glum belly, a gold cap-dot a content
//! one, a light is an activity under way. Nothing is drawn that the rules
//! did not do.

use super::super::chrome;
use super::super::ink::{Img, Rgb, bayer, hash, ink, text_width};
use super::fx::{self, Hue};
use super::{NATIVE_W, TILE};
use crate::rules::fey;
use crate::rules::holdings::{self, Holding};
use crate::rules::occasions::{Broke, Occasion};
use crate::rules::strolls::{self, Pose, Stroller};
use crate::rules::villagers::{Trade, Villager, COMING_OF_AGE, ELDER};
use crate::rules::{HZ, Run, TILE_UNITS};

// ── Figures ──────────────────────────────────────────────────────────────

/// A tunic's four inks: light, middle, shade and the outline that edges
/// it (a deeper hue of the same, never black).
type Ramp = [char; 4];

fn ramp(f: &Villager) -> Ramp {
    if f.age >= ELDER {
        return ['i', 'h', 'G', 'j'];
    }
    match f.trade {
        Trade::Brewer => ['t', 'O', 'R', 'p'],
        Trade::Smith => ['v', 'U', 'u', 's'],
        Trade::Acolyte => [']', '[', ')', '('],
        Trade::Trader => ['z', 'Q', 'q', 's'],
        Trade::Guard => ['M', 'm', 'l', 'F'],
        Trade::Labourer => ['o', 'r', 'p', 'b'],
        Trade::Child => ['%', ';', '?', '!'],
    }
}

/// Grey smocks for a belly gone hungry or a mood gone glum, as the Court's
/// cross-section has always shown them.
fn wan(f: &Villager) -> bool {
    f.hunger > 0 || f.mood <= -2
}

/// A soul's look from their name: a fixed number, so the same soul is the
/// same face in every view.
fn name_hash(name: &str, who: usize) -> u32 {
    hash(name.bytes().map(i32::from).sum(), name.len() as i32 + who as i32 * 3, 0x61_17)
}

fn pal(f: &Villager, who: usize, mourning: bool, rage: bool) -> Vec<(char, Rgb)> {
    let h = name_hash(&f.name, who) as usize;
    let skins = [('c', '$'), ('T', 't'), ('9', 'c'), ('t', 'o')];
    let hairs = [('P', 'b'), ('B', 'n'), ('o', 'r'), ('k', 'Z'), ('r', 'p')];
    let (skin, shade) = skins[h % skins.len()];
    let (hair, dark) = if f.age >= ELDER { ('i', 'h') } else { hairs[(h >> 3) % hairs.len()] };
    let mut r = ramp(f);
    if mourning {
        r = ['h', 'G', 'j', 'g'];
    } else if wan(f) {
        r = ['i', 'h', 'G', 'j'];
    }
    let edge = if rage { '7' } else { r[3] };
    let pick = |c: char| ink(c).unwrap_or([0, 0, 0]);
    vec![
        ('g', pick(dark)),
        ('h', pick(hair)),
        ('s', pick(skin)),
        ('t', pick(shade)),
        ('k', pick('k')),
        ('o', pick(edge)),
        ('a', pick(r[0])),
        ('b', pick(r[1])),
        ('d', pick(r[2])),
        ('l', pick(match f.trade {
            Trade::Smith | Trade::Guard => 'g',
            Trade::Acolyte => '(',
            _ => 'B',
        })),
        ('f', pick('Z')),
        ('x', pick('J')),
        ('y', pick('r')),
        ('w', pick('5')),
    ]
}

const HEAD: [&str; 5] = ["...gggg...", "..ghhhhg..", "..gsssst..", "..gskskt..", "...gsst..."];
const TORSO: [&str; 7] = [
    "..oaaaao..",
    ".oaabbbdo.",
    ".sabbbbds.",
    ".sabbbbds.",
    ".oabbbbdo.",
    "..obbbddo.",
    "..oddddo..",
];
const LEGS: [[&str; 3]; 3] = [
    ["...ll.ll..", "...ll.ll..", "..ff..ff.."],
    ["...ll.ll..", "...ll..ll.", "..ff...ff."],
    ["...ll.ll..", ".ll...ll..", ".ff...ff.."],
];

fn grid(rows: &[&str]) -> Vec<Vec<char>> {
    rows.iter().map(|r| r.chars().collect()).collect()
}

fn set(g: &mut [Vec<char>], x: usize, y: usize, c: char) {
    if let Some(row) = g.get_mut(y)
        && let Some(cell) = row.get_mut(x)
    {
        *cell = c;
    }
}

/// The tool a trade works with, raised or lowered: the smith's hammer, the
/// brewer's ladle, the acolyte's censer, the trader's scale, a labourer's
/// mallet.
fn tool(g: &mut [Vec<char>], trade: Trade, up: bool, laden: bool) {
    let (hx, hy) = if up { (9, 1) } else { (9, 4) };
    set(g, 8, if up { 3 } else { 4 }, 's');
    match trade {
        Trade::Smith | Trade::Labourer => {
            set(g, hx, hy + 1, 'y');
            set(g, hx, hy, 'x');
            set(g, hx - 1, hy, 'x');
        }
        Trade::Brewer => {
            set(g, hx, hy, 'y');
            set(g, hx, hy - 1, 'w');
        }
        Trade::Acolyte => {
            set(g, hx, hy, 'w');
            set(g, hx, hy + 1, 'y');
        }
        Trade::Trader => {
            set(g, hx, hy, 'y');
            set(g, hx - 1, hy - 1, 'x');
            set(g, hx, hy - 1, 'x');
        }
        _ => {}
    }
    if laden {
        // A beam across the shoulder.
        for x in 0..10 {
            set(g, x, 5, if x % 4 == 0 { 'y' } else { 'w' });
        }
    }
}

/// One soul, drawn at `s`'s pose at `tick`. Children are smaller; the old
/// are grey. The sprite's feet are its bottom row.
fn figure(f: &Villager, s: &Stroller, tick: u64, mourning: bool) -> Img {
    let child = f.trade == Trade::Child || f.age < COMING_OF_AGE;
    let step = if s.pose == Pose::Walk || s.pose == Pose::Play || s.pose == Pose::Mourn || s.pose == Pose::Bear {
        1 + ((tick / 6 + s.who as u64) % 2) as usize
    } else {
        0
    };
    let rage = s.pose == Pose::Rage;
    let pal = pal(f, s.who, mourning, rage);
    let mut g = grid(&HEAD);
    g.extend(grid(&TORSO));
    g.extend(grid(&LEGS[step]));
    let beat = (tick / 5 + s.who as u64).is_multiple_of(2);
    match s.pose {
        Pose::Work => tool(&mut g, f.trade, beat, s.laden),
        Pose::Walk if s.laden => tool(&mut g, f.trade, false, true),
        Pose::Rage => {
            // Both fists up, mouth open.
            for x in [0, 9] {
                set(&mut g, x, 2, 's');
                set(&mut g, x, 3, 'o');
                set(&mut g, x, 4, 'a');
            }
            set(&mut g, 3, 4, 'o');
            set(&mut g, 5, 4, 'o');
            set(&mut g, 4, 5, '7');
        }
        Pose::Toast => {
            set(&mut g, 8, 3, 's');
            set(&mut g, 9, 2, 's');
            set(&mut g, 9, 1, 'y');
            set(&mut g, 9, 0, 'w');
        }
        Pose::Bear => {
            for x in [0, 9] {
                set(&mut g, x, 8, 's');
            }
        }
        Pose::Mourn => {
            // Head bowed: a row of hair over the brow.
            set(&mut g, 3, 3, 'h');
            set(&mut g, 4, 3, 'h');
            set(&mut g, 5, 3, 'h');
            set(&mut g, 6, 3, 'h');
        }
        Pose::Vigil => {
            set(&mut g, 4, 8, 's');
            set(&mut g, 5, 8, 's');
            set(&mut g, 1, 8, 'd');
            set(&mut g, 8, 8, 'd');
        }
        Pose::Seated => {
            // Down behind the bench: no legs, a bench edge.
            g.truncate(12);
            g.push("yyyyyyyyyy".chars().collect());
        }
        _ => {}
    }
    if s.pose == Pose::Seated && f.age >= ELDER {
        // A blanket over the knees.
        for x in 2..8 {
            set(&mut g, x, 11, 'd');
        }
    }
    // A content soul wears a gold dot in the cap.
    if !wan(f) && f.mood >= 3 && !mourning && !rage {
        set(&mut g, 4, 1, 'w');
    }
    let rows: Vec<String> = g.into_iter().map(|r| r.into_iter().collect()).collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut im = Img::from_rows_pal(&rows, &pal);
    if child {
        im = shrunk(&im);
    }
    if s.east { im } else { im.flip_h() }
}

/// A child: the same soul, three fifths the height, by dropping every
/// other row and column of the adult's middle.
fn shrunk(im: &Img) -> Img {
    let (w, h) = (im.w * 3 / 5 + 1, im.h * 3 / 5 + 1);
    let mut o = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = im.get(x * 5 / 3, y * 5 / 3) {
                o.set(x, y, c);
            }
        }
    }
    o
}

// ── The hall ─────────────────────────────────────────────────────────────

fn px((x, y): (f32, f32)) -> (i32, i32) {
    ((x * TILE as f32).round() as i32, (y * TILE as f32).round() as i32)
}

/// How near (tiles) a knight must be to a soul for their name to show, and
/// how many names show at once.
const NAME_REACH: f32 = 3.8;
const NAMES_AT_ONCE: usize = 1;

fn distance(run: &Run, s: &Stroller) -> f32 {
    run.players
        .values()
        .filter(|h| h.hp > 0)
        .map(|h| (h.x / TILE_UNITS - s.x).hypot(h.y / TILE_UNITS - s.y))
        .fold(f32::MAX, f32::min)
}

/// A soft pool of coloured light on dark ground: dithered rings of `inks`,
/// outermost first, laid only over what is dark already, so a pool never
/// washes out a figure or a board.
fn pool(cv: &mut Img, cx: i32, cy: i32, rx: i32, ry: i32, inks: &[char]) {
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let d = (dx * dx) as f32 / (rx * rx) as f32 + (dy * dy) as f32 / (ry * ry) as f32;
            if d >= 1.0 {
                continue;
            }
            let k = (1.0 - d) * inks.len() as f32;
            let level = (k.floor() as usize).min(inks.len() - 1);
            let rise = k - k.floor();
            let (x, y) = (cx + dx, cy + dy);
            let ink_at = if level + 1 < inks.len() && bayer(x, y) < rise { level + 1 } else { level };
            if k < 0.5 && bayer(x, y) > k * 2.0 {
                continue;
            }
            if cv.get(x, y).is_none_or(|c| u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]) < 150) {
                cv.put(x, y, inks[ink_at]);
            }
        }
    }
}

const WARM_POOL: [char; 5] = ['n', 'Z', 'b', 'I', 'B'];
const COLD_POOL: [char; 4] = ['s', 'x', 'S', 'u'];
const RED_POOL: [char; 4] = ['n', 'Z', 'b', '8'];

/// The village in the hall, and what it is in the middle of: called with
/// the rest of the hall's ground (`holdings::hall`). The crew's unnamed
/// hands are drawn there; these are the ledger's.
pub(super) fn hall_folk(cv: &mut Img, run: &Run, held: &Holding) {
    if run.holding_raid.is_some() {
        return;
    }
    let tick = run.tick;
    let strollers = strolls::strollers(held, tick);
    props_under(cv, run, held, tick);
    let mourning = matches!(held.occasion, Some(Occasion::Mourning { .. }));
    let mut order: Vec<&Stroller> = strollers.iter().collect();
    order.sort_by(|a, b| a.y.total_cmp(&b.y));
    for s in &order {
        let Some(f) = held.folk.get(s.who) else { continue };
        let grieving = mourning && matches!(s.pose, Pose::Bear | Pose::Mourn);
        let im = figure(f, s, tick, grieving);
        let (x, y) = px((s.x, s.y));
        cv.stamp(&im, x - im.w / 2, y - im.h);
    }
    props_over(cv, run, held, tick);
}

/// What is drawn over the furniture, with the boards: a tantrum's bang and
/// shards, a seized bench's sparks, and the names — the nearest couple to a
/// knight, and whoever the hall is about. A name in a hungry or glum soul's
/// red is the same state the smock shows. Chips keep clear of one another.
fn marks_and_names(cv: &mut Img, run: &Run, held: &Holding) {
    let tick = run.tick;
    let strollers = strolls::strollers(held, tick);
    let mut order: Vec<&Stroller> = strollers.iter().collect();
    order.sort_by(|a, b| a.y.total_cmp(&b.y));
    for s in &order {
        let Some(f) = held.folk.get(s.who) else { continue };
        let (x, y) = px((s.x, s.y));
        let top = y - if f.trade == Trade::Child { 10 } else { 15 };
        if s.pose == Pose::Rage {
            rage_marks(cv, x, top, tick);
        }
        if s.pose == Pose::Work && held.seized.as_ref().is_some_and(|z| z.who == f.name) {
            sparks(cv, x + if s.east { 8 } else { -8 }, top + 4, tick);
        }
    }
    let mut near: Vec<(&Stroller, f32)> = order.iter().map(|s| (*s, distance(run, s))).filter(|&(_, d)| d < NAME_REACH).collect();
    near.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut shown: Vec<(i32, i32, i32, i32)> = Vec::new();
    for s in &order {
        let Some(f) = held.folk.get(s.who) else { continue };
        let subject = match &held.occasion {
            Some(Occasion::Tantrum { who, .. }) => *who == f.name,
            _ => false,
        } || held.seized.as_ref().is_some_and(|z| z.who == f.name)
            || s.pose == Pose::Vigil;
        if !subject && !near.iter().take(NAMES_AT_ONCE).any(|(n, _)| n.who == s.who) {
            continue;
        }
        let (x, y) = px((s.x, s.y));
        let tag = f.name.to_uppercase();
        let w = text_width(&tag);
        let mut top = y - if f.trade == Trade::Child { 20 } else { 26 };
        let cx = (x - w / 2).clamp(2, NATIVE_W - w - 2);
        while shown.iter().any(|&(sx, sy, sw, sh)| cx - 1 < sx + sw && cx + w + 1 > sx && top - 1 < sy + sh && top + 8 > sy) && top > 12 {
            top -= 10;
        }
        shown.push((cx - 1, top - 1, w + 2, 9));
        chrome::chip(cv, (cx - 1, top - 1, w + 2, 9));
        cv.text(cx, top, &tag, if wan(f) { '7' } else { 'i' });
    }
}

/// What lies on the floor and the table before the figures stand over it.
fn props_under(cv: &mut Img, run: &Run, held: &Holding, tick: u64) {
    match &held.occasion {
        Some(Occasion::Feast) => feast_table(cv, tick),
        Some(Occasion::Tantrum { who, broke, .. }) => {
            match *broke {
                Broke::Course(bay) => {
                    let (x, y) = px(strolls::work_spot(usize::from(bay)));
                    pool(cv, x, y + 2, 26, 14, &RED_POOL);
                    splinters(cv, x, y);
                }
                Broke::Sack => spilt_grain(cv),
                Broke::Nothing => {
                    if let Some(s) = strolls::strollers(held, tick).iter().find(|s| held.folk.get(s.who).is_some_and(|f| f.name == *who)) {
                        let (x, y) = px((s.x, s.y));
                        pool(cv, x, y, 22, 12, &RED_POOL);
                    }
                }
            }
        }
        Some(Occasion::Mourning { .. }) => bier(cv, tick),
        None => {}
    }
    // A seized workshop's bench is lit by the fey, a finished work stands
    // lit on its bench.
    if let Some(z) = &held.seized
        && let Some(bay) = fey::workshop(held, z.trade)
    {
        fey_light(cv, bay, tick);
    }
    for m in &held.works {
        if let Some(bay) = fey::workshop(held, m.trade) {
            let carried = m.curio().is_some_and(|c| carrier(run, c.id).is_some());
            plinth(cv, bay, m.trade, tick, carried);
        }
    }
}

/// The knight carrying the artifact `id` this run, if any.
fn carrier<'r>(run: &'r Run, id: &str) -> Option<(u32, &'r crate::rules::Hero)> {
    run.players.iter().find(|(_, h)| h.pockets.has(id)).map(|(&n, h)| (n, h))
}

fn props_over(cv: &mut Img, run: &Run, held: &Holding, tick: u64) {
    let _ = run;
    match &held.occasion {
        Some(Occasion::Feast) => {
            bunting(cv, tick);
            for &(sx, sy) in &[(10.4f32, 5.4f32), (13.6, 5.4)] {
                let (x, y) = px((sx, sy));
                candle(cv, x, y, tick);
            }
        }
        Some(Occasion::Mourning { .. }) => {
            let (at, _) = strolls::bier(tick);
            let (x, y) = px(at);
            fx::glow(cv, x, y - 8, Hue::Warm);
        }
        _ => {}
    }
}

// ── Props ────────────────────────────────────────────────────────────────

fn feast_table(cv: &mut Img, tick: u64) {
    // A long trestle where the steward's table is, laid with a cloth, loaves
    // and cups; warm light pooled over it.
    let (x0, y0) = px((9.6, 4.9));
    let (w, h) = (6 * TILE - 4, TILE + 8);
    pool(cv, x0 + w / 2, y0 + h / 2, w / 2 + 18, h + 6, &WARM_POOL);
    cv.rect(x0, y0 + h, w, 3, 'b');
    cv.rect(x0 + 2, y0 + 2, w - 4, h, 'p');
    cv.rect(x0, y0, w, h, 'o');
    cv.rect(x0, y0, w, 1, 'O');
    cv.rect(x0, y0 + h - 2, w, 2, 'r');
    // A runner in the realm's blue and gold.
    cv.rect(x0 + 2, y0 + h / 2 - 2, w - 4, 4, 'q');
    for k in 0..w / 6 {
        cv.put(x0 + 4 + k * 6, y0 + h / 2 - 1, '4');
    }
    // Loaves, a roast, cups.
    for (i, &(dx, ch)) in [(10, 'R'), (24, 'O'), (40, 'r'), (58, 'R'), (74, 'O')].iter().enumerate() {
        let (lx, ly) = (x0 + dx, y0 + 3 + (i as i32 % 2) * 11);
        cv.rect(lx, ly, 6, 3, ch);
        cv.put(lx + 1, ly, 't');
    }
    for dx in [16, 32, 50, 66, 82] {
        let (cx, cy) = (x0 + dx, y0 + h - 7);
        cv.rect(cx, cy, 2, 3, 'Q');
        cv.put(cx, cy, 'z');
        if (tick / 9 + dx as u64).is_multiple_of(5) {
            cv.put(cx, cy - 1, 'T');
        }
    }
}

fn bunting(cv: &mut Img, tick: u64) {
    // Pennants along a swag across the hall's middle, realm colours.
    let (y0, x0, x1) = (2 * TILE + 6, 5 * TILE, 19 * TILE);
    for x in x0..x1 {
        let sag = (((x - x0) as f32 / (x1 - x0) as f32 * std::f32::consts::PI).sin() * 8.0) as i32;
        cv.put(x, y0 + sag, 'r');
        if (x - x0) % 12 == 5 {
            let c = ['q', 'O', 'R', 'z', '5'][((x - x0) / 12) as usize % 5];
            let wave = ((tick / 8 + ((x - x0) / 12) as u64) % 2) as i32;
            cv.rect(x - 2, y0 + sag + 1, 5, 3, c);
            cv.put(x, y0 + sag + 4 + wave, c);
        }
    }
}

fn candle(cv: &mut Img, x: i32, y: i32, tick: u64) {
    cv.rect(x, y - 3, 2, 4, 'O');
    let flick = (tick / 4 + x as u64) % 3;
    cv.put(x, y - 4 - (flick == 0) as i32, '5');
    cv.put(x + 1, y - 4, '6');
    fx::glow(cv, x, y - 4, Hue::Warm);
}

fn splinters(cv: &mut Img, x: i32, y: i32) {
    for (dx, dy, w, c) in [(-8, 2, 5, 'R'), (3, 5, 4, 'r'), (-2, 8, 6, 'o'), (9, 3, 3, 'R'), (-13, 6, 3, 'p'), (6, 10, 5, 'r')] {
        cv.rect(x + dx, y + dy, w, 1, c);
        cv.put(x + dx + w / 2, y + dy + 1, 'b');
    }
}

fn spilt_grain(cv: &mut Img) {
    let (x, y) = px((12.0, 8.6));
    for k in 0..26 {
        let (dx, dy) = ((k * 7 % 19) - 9, (k * 5 % 9) - 3);
        cv.put(x + dx, y + dy, if k % 3 == 0 { 't' } else { 'O' });
    }
    cv.rect(x - 7, y - 2, 4, 3, 'p');
}

fn rage_marks(cv: &mut Img, x: i32, top: i32, tick: u64) {
    // A red bang over the head, and a few shards flying off.
    let up = ((tick / 4) % 2) as i32;
    cv.rect(x, top - 9 - up, 2, 5, '7');
    cv.rect(x, top - 3 - up, 2, 2, '7');
    for k in 0..5u64 {
        let a = (tick / 3 + k * 7) % 12;
        let (dx, dy) = ((k as i32 - 2) * (a as i32 + 2) / 2, 12 - a as i32 - (a as i32 * a as i32) / 8);
        cv.put(x + 1 + dx, top + 8 + dy / 2, if k % 2 == 0 { 'R' } else { 'o' });
    }
}

fn sparks(cv: &mut Img, x: i32, y: i32, tick: u64) {
    if tick.is_multiple_of(6) || (tick / 3).is_multiple_of(4) {
        return;
    }
    for k in 0..4i32 {
        let d = ((tick / 2) as i32 + k * 3) % 6;
        cv.put(x + k - 2 + d / 2, y - d, if k % 2 == 0 { '5' } else { '4' });
    }
}

/// The bier: a plank on two poles, a violet pall edged in gold over the
/// dead, a sprig of rose along it, a candle at each end, lit.
fn bier(cv: &mut Img, tick: u64) {
    let (at, east) = strolls::bier(tick);
    let (x, y) = px(at);
    let k = if east { 1 } else { -1 };
    pool(cv, x, y - 8, 34, 16, &WARM_POOL);
    cv.rect(x - 22, y - 9, 44, 2, 'r');
    cv.rect(x - 20, y - 15, 40, 8, ')');
    cv.rect(x - 20, y - 15, 40, 1, '[');
    cv.rect(x - 20, y - 8, 40, 1, '(');
    for dx in (-18..19).step_by(4) {
        cv.put(x + dx, y - 14, '4');
        cv.put(x + dx, y - 9, '4');
    }
    for dx in (-8..10).step_by(5) {
        cv.put(x + dx * k, y - 12, '%');
        cv.put(x + dx * k + 1, y - 12, ';');
    }
    // The poles' ends, where the bearers hold.
    cv.rect(x - 26, y - 8, 5, 1, 'B');
    cv.rect(x + 21, y - 8, 5, 1, 'B');
    // A candle at each end, set on the pall.
    for cx in [x - 18, x + 17] {
        cv.rect(cx, y - 19, 2, 4, 'T');
        let flick = (tick / 4 + cx as u64) % 3;
        cv.put(cx, y - 20 - (flick == 0) as i32, '5');
        cv.put(cx + 1, y - 20, '6');
        fx::glow(cv, cx, y - 20, Hue::Warm);
    }
}

/// The fey bench: a cold teal light pooled on the floor before the
/// workshop, with motes rising off it in violet and teal.
fn fey_light(cv: &mut Img, bay: usize, tick: u64) {
    let (x, y) = px(strolls::work_spot(bay));
    pool(cv, x, y + 6, 30, 22, &COLD_POOL);
    let top = y - 6;
    fx::glow(cv, x, top, Hue::Cold);
    for k in 0..8i64 {
        let age = (tick as i64 / 3 + k * 4) % 32;
        let dx = ((k * 37 % 23) - 11) as i32 + ((age as f32 / 4.0).sin() * 2.0) as i32;
        let dy = -(age as i32) + 4;
        let c = if age < 8 { '3' } else if age < 20 { '2' } else { '[' };
        cv.put(x + dx, y + dy, c);
        if k % 2 == 0 {
            cv.put(x + dx + 1, y + dy, c);
            cv.put(x + dx, y + dy + 1, ']');
        }
    }
}

/// A masterwork on its pedestal in front of the maker's bench: the craft's
/// own thing, gold-lit.
fn plinth(cv: &mut Img, bay: usize, trade: Trade, tick: u64, carried: bool) {
    let (bx, by) = px(strolls::work_spot(bay));
    let (x, y) = (bx + 17, by + 2);
    if !carried {
        pool(cv, x, y - 10, 22, 22, &WARM_POOL);
    }
    // A stone plinth with a gilded edge.
    cv.rect(x - 5, y - 7, 11, 7, 'J');
    cv.rect(x - 5, y - 7, 11, 1, 'i');
    cv.rect(x - 6, y, 13, 2, 'G');
    cv.rect(x - 4, y - 6, 1, 6, 'h');
    cv.rect(x - 5, y - 4, 11, 1, '4');
    if carried {
        // A knight has it: the pedestal stands bare.
        return;
    }
    let art: &[&str] = match trade {
        Trade::Smith => &["..v..", ".vUv.", ".vUv.", "vvUvv", "..r..", "..r.."],
        Trade::Brewer => &[".RRR.", "RROOR", "ROORR", "RROOR", ".RRR."],
        Trade::Acolyte => &["..5..", ".444.", "48884", ".888.", "..4.."],
        _ => &["z...z", "xz.zx", ".xzx.", "..y..", "..y.."],
    };
    let im = Img::from_rows(art);
    cv.stamp(&im, x - im.w / 2, y - 8 - im.h);
    fx::glow(cv, x, y - 12, Hue::Warm);
    fx::glow(cv, x, y - 14, Hue::Warm);
    let t = tick / 9;
    if t % 3 != 2 {
        let (sx, sy) = ((t % 7) as i32 - 3, (t % 5) as i32);
        cv.put(x + sx, y - 17 - sy, '6');
        cv.put(x + sx - 1, y - 17 - sy, '5');
    }
}

// ── Boards ───────────────────────────────────────────────────────────────

/// The hall's own notice: what the village is in the middle of, in a
/// plain chip at the top, and a line for the soul beside a knight.
pub(super) fn boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(held) = holdings::held_floor(&run.home.holdings, run.dungeon.pack) else { return };
    if run.holding_raid.is_some() {
        return;
    }
    marks_and_names(cv, run, held);
    let mut top = 18;
    if let Some(line) = banner(held) {
        let w = text_width(&line) + 8;
        chrome::board(cv, ((NATIVE_W - w) / 2, top, w, 13), 'B');
        cv.text((NATIVE_W - w) / 2 + 4, top + 3, &line, 'O');
        top += 15;
    }
    let id = focus.unwrap_or(1);
    // What the hall last said to this knight: a soul's word, the artifact
    // taken, who the village was mourning as they came in.
    if let Some((when, who, line)) = run.found.as_ref()
        && run.tick.saturating_sub(*when) < u64::from(HZ * 6)
        && (*who == 0 || *who == id)
        && !crate::rules::take_line(line)
    {
        let lines = wrap(line, 46);
        let w = lines.iter().map(|l| text_width(l)).max().unwrap_or(0) + 8;
        let h = lines.len() as i32 * 9 + 5;
        chrome::board(cv, ((NATIVE_W - w) / 2, top, w, h), 'B');
        for (n, l) in lines.iter().enumerate() {
            cv.text((NATIVE_W - w) / 2 + 4, top + 3 + n as i32 * 9, l, 'i');
        }
    }
    let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0) else { return };
    let tile = ((hero.x / TILE_UNITS).floor() as i32, (hero.y / TILE_UNITS).floor() as i32);
    let inside = |(c, r, w, h): (i32, i32, i32, i32)| tile.0 >= c && tile.0 < c + w && tile.1 >= r && tile.1 < r + h;
    if inside(holdings::STEWARD_PLATE) || (0..held.rooms.len().min(holdings::SLOTS)).any(|b| inside(holdings::bay(b).1)) {
        return;
    }
    let Some(i) = strolls::near(held, run.tick, hero.x, hero.y) else { return };
    let f = &held.folk[i];
    let mut lines = vec![(format!("{} · {}", f.name.to_uppercase(), f.trade.word().to_uppercase()), '9')];
    if let Some(m) = held.works.iter().find(|m| m.maker == f.name && m.trade == f.trade)
        && let Some(c) = m.curio()
    {
        lines.push((c.name.to_uppercase(), 'O'));
        lines.push((format!("+ {}", c.boon), 'y'));
        lines.push((format!("- {}", c.bane), '7'));
        lines.push(match carrier(run, c.id) {
            Some((n, _)) if n == id => ("YOU CARRY IT THIS RUN".to_string(), 'h'),
            Some((_, h)) => (format!("{} CARRIES IT THIS RUN", h.name.to_uppercase()), 'h'),
            None => ("E: TAKE IT (ONE KNIGHT AT A TIME)".to_string(), 'O'),
        });
    } else {
        lines.push(("E: SPEAK".to_string(), 'h'));
        if f.hunger > 0 {
            lines.push(("HUNGRY".to_string(), '7'));
        } else if f.mood <= -2 {
            lines.push(("GLUM".to_string(), '7'));
        }
    }
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 8;
    let bh = lines.len() as i32 * 9 + 5;
    let (x, y) = (((NATIVE_W - w) / 2).clamp(24, NATIVE_W - w - 96), super::NATIVE_H - bh - 1);
    chrome::board(cv, (x, y, w, bh), 'B');
    for (n, (text, ink)) in lines.iter().enumerate() {
        cv.text(x + (w - text_width(text)) / 2, y + 3 + n as i32 * 9, text, *ink);
    }
}

/// What the village is in the middle of, for the chip at the top.
pub(crate) fn banner(held: &Holding) -> Option<String> {
    Some(match (&held.occasion, &held.seized) {
        (Some(Occasion::Feast), _) => "FEAST: THE GRANARY WAS FULL".to_string(),
        (Some(Occasion::Tantrum { who, cause, .. }), _) => format!("{} IS {}: A TANTRUM", who.to_uppercase(), cause.word().to_uppercase()),
        (Some(Occasion::Mourning { who, dying, .. }), _) => format!("MOURNING {}, DEAD {}", who.to_uppercase(), dying.word().to_uppercase()),
        (None, Some(z)) => format!("{} HAS THE WORKSHOP: {}D LEFT", z.who.to_uppercase(), z.days),
        (None, None) => return None,
    })
}

// ── The Court's cross-section ───────────────────────────────────────────

/// The holding's folk as small figures walking their bays in the Court's
/// cross-section, in place of the anonymous roster when anyone lives
/// there. `x`, `y` as the cross-section's own.
pub(super) fn cross_section(cv: &mut Img, x: i32, y: i32, held: &Holding, tick: u32) {
    let floor_y = y + 13;
    let bay_x = |i: usize| x + 16 + i as i32 * 30;
    let mourning = matches!(held.occasion, Some(Occasion::Mourning { .. }));
    for (n, f) in held.folk.iter().enumerate() {
        let child = f.trade == Trade::Child || f.age < COMING_OF_AGE;
        let home = match f.trade {
            t if fey::is_craft(t) => fey::workshop(held, t),
            Trade::Labourer => held.rising(),
            _ => None,
        };
        let h = (name_hash(&f.name, n) >> 5) as i32;
        let t = i64::from(tick) / 4;
        let (px_, working) = match home {
            Some(bay) => {
                let span: i32 = 10;
                let k = ((t + i64::from(h)) % i64::from(span * 4)) as i32;
                let sway = if k < span { k } else if k < span * 2 { span } else if k < span * 3 { span * 3 - k } else { 0 };
                (bay_x(bay) + 6 + sway.min(span), k >= span && k < span * 2)
            }
            None if f.age >= ELDER => (x + 3 + (n as i32 % 3) * 3, false),
            None => {
                // Along the whole run of bays and back, a slow patrol.
                let len = holdings::SLOTS as i32 * 30;
                let k = ((t / 2 + i64::from(h) * 9) % i64::from(len * 2)) as i32;
                (x + 14 + if k < len { k } else { len * 2 - k }, false)
            }
        };
        let east = ((t + i64::from(h)) / 40) % 2 == 0;
        let pose = if working { Pose::Work } else if child { Pose::Play } else { Pose::Walk };
        let s = Stroller { who: n, x: 0.0, y: 0.0, east, pose, laden: false };
        let mut im = figure(f, &s, u64::from(tick), mourning);
        im = squeezed(&im);
        cv.stamp(&im, px_, floor_y - im.h);
    }
    // The village's occasion over the bays, as small lit marks.
    match &held.occasion {
        Some(Occasion::Feast) => {
            for k in 0..3 {
                let (cx, cy) = (x + 20 + k * 90, y + 2);
                cv.put(cx, cy, if (tick / 8 + k as u32) % 2 == 0 { '5' } else { '6' });
                cv.put(cx + 1, cy, '4');
                fx::glow(cv, cx, cy, Hue::Warm);
            }
        }
        Some(Occasion::Tantrum { broke: Broke::Course(bay), .. }) => {
            let bx = bay_x(usize::from(*bay));
            for (dx, c) in [(2, '7'), (5, 'R'), (9, '7')] {
                cv.put(bx + dx, y + 3 + dx % 3, c);
            }
        }
        _ => {}
    }
    if let Some(z) = &held.seized
        && let Some(bay) = fey::workshop(held, z.trade)
    {
        fx::glow(cv, bay_x(bay) + 12, y + 4, Hue::Cold);
        cv.put(bay_x(bay) + 12, y + 3 - ((tick / 6) % 3) as i32, '3');
    }
    for m in &held.works {
        if let Some(bay) = fey::workshop(held, m.trade) {
            cv.put(bay_x(bay) + 22, y + 3, '6');
            cv.put(bay_x(bay) + 22, y + 2, '5');
            fx::glow(cv, bay_x(bay) + 22, y + 3, Hue::Warm);
        }
    }
}

/// A figure at the cross-section's scale: the hall's drawn two thirds as tall.
fn squeezed(im: &Img) -> Img {
    let (w, h) = (im.w * 7 / 10, im.h * 7 / 10);
    let mut o = Img::new(w.max(1), h.max(1));
    for y in 0..o.h {
        for x in 0..o.w {
            if let Some(c) = im.get(x * 10 / 7, y * 10 / 7) {
                o.set(x, y, c);
            }
        }
    }
    o
}

/// Words broken into lines of at most `n` characters.
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

// ── The countryside's keepers ──────────────────────────────────────────

/// The trade a plot's keeper works at, for their figure's tunic and tool.
fn keeper_trade(plot: &str) -> Trade {
    match plot {
        "bakery" | "bothy" | "inn" | "harbour" => Trade::Brewer,
        "beacon" | "post" | "kitepost" => Trade::Guard,
        "ferry" | "punthouse" | "caravan" | "racecoach" | "quivercoach" | "ropeway" | "lift" | "aerie" => Trade::Trader,
        _ => Trade::Labourer,
    }
}

/// A built plot's keeper, by name, at their work beside its ring (`at`, the
/// ring's pixel) and out for a breath and back: the figure grey when the
/// realm's granary is bare or its unrest high, as the board's "LEAN TIMES"
/// and "WORRIED" say, and wearing the gold cap-dot when neither is so. Their
/// name stands over them while a knight is near (`KEPT BY` is on the board).
pub(super) fn keeper(cv: &mut Img, run: &Run, plot: &crate::rules::countryside::Plot, at: (i32, i32)) {
    let h = &run.home.holdings;
    let lean = h.stores == 0 || h.unrest >= holdings::RESTLESS;
    let name = run
        .home
        .countryside
        .keepers
        .get(plot.id)
        .cloned()
        .unwrap_or_else(|| crate::rules::villagers::keeper_name(plot.id));
    let f = Villager {
        name,
        trade: keeper_trade(plot.id),
        mood: if lean { -2 } else { 3 },
        hunger: 0,
        skill: 0,
        practiced: 0,
        age: 30,
        seasons: 0,
    };
    let seed = name_hash(plot.id, 0) as u64;
    let day = 24 * HZ as u64;
    let p = ((run.tick + seed % day) % day) as f32 / day as f32;
    let reach = 16;
    let (dx, pose, east) = if p < 0.55 {
        (0, Pose::Work, true)
    } else if p < 0.65 {
        (((p - 0.55) / 0.10 * reach as f32) as i32, Pose::Walk, true)
    } else if p < 0.80 {
        (reach, Pose::Idle, false)
    } else if p < 0.90 {
        (((0.90 - p) / 0.10 * reach as f32) as i32, Pose::Walk, false)
    } else {
        (0, Pose::Work, true)
    };
    let s = Stroller { who: seed as usize % 7, x: 0.0, y: 0.0, east, pose, laden: false };
    let im = figure(&f, &s, run.tick, false);
    let (x, y) = (at.0 + dx, at.1 + 4);
    cv.stamp(&im, x - im.w / 2, y - im.h);
    let near = run.players.values().any(|k| k.hp > 0 && (k.x / TILE_UNITS * TILE as f32 - x as f32).hypot(k.y / TILE_UNITS * TILE as f32 - y as f32) < 4.0 * TILE as f32);
    if near {
        let tag = f.name.to_uppercase();
        let w = text_width(&tag);
        let (cx, cy) = (x - w / 2 + 6, y - im.h - 20);
        chrome::chip(cv, (cx - 1, cy - 1, w + 2, 9));
        cv.text(cx, cy, &tag, if lean { '7' } else { 'i' });
    }
}
