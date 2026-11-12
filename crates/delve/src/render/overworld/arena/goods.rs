//! The forges' goods, drawn: the ore cart on the mine-head track and the
//! heap in the yard, a forge-hall's bins with what waits in them, the
//! fire a forge shows by how its last shift went, and the boards that say
//! what a forge makes and what it is waiting on.

use super::super::ink::{Img, hash, text_width};
use super::super::light::Light;
use super::{TILE, at, fire, home};
use crate::realm::{Spoil, Spoils};
use crate::rules::barony::{self, Barony, Forge};
use crate::rules::goods::{
    self, ARMOURY, BELLS_HUNG, BREW_SACKS, Burn, Good, Goods, IN_BIN, OUT_BIN, TYPE_PER_NOTICE,
};
use crate::rules::world::{ADIT, SHAFT};
use crate::rules::{HZ, Run};

// ── The mine-head: the cart on the track, the heap in the yard ───────────

/// An ore cart seen from the end it runs toward: its timber box, iron
/// bands, the wheels either side on the rails.
fn mine_cart(loaded: bool) -> Img {
    let mut im = Img::from_rows(&[
        "..bbbbbbbbbbbbbbbbb..",
        ".bPPPPPPPPPPPPPPPPPb.",
        ".bPrrrrrrrrrrrrrrrPb.",
        ".bPrRRRRRRRRRRRRRrPb.",
        ".bPrRRRRRRRRRRRRRrPb.",
        "UbPrrrrrrrrrrrrrrrPbU",
        "UvbJJJJJJJJJJJJJJJbvU",
        ".UbbbbbbbbbbbbbbbbbU.",
        "..jj.............jj..",
    ]);
    if loaded {
        for k in 0..9 {
            let x = 3 + k * 2;
            let y = 1 + ((k * 5) % 3);
            im.rect(
                x,
                y,
                2,
                2,
                ['J', 'R', 'h', 'j', 'R', 'J', 'o', 'j', 'R'][k as usize],
            );
        }
    }
    im
}

/// The track's two rails and its ends, in canvas pixels: the rails run from
/// the adit's mouth down the road to the yard (`world::mine_head`).
fn track() -> (i32, i32, i32, i32) {
    let (_, r, _, h) = ADIT;
    let (sc, _, sw, _) = SHAFT;
    let (mx0, mx1) = (sc * TILE - 2, (sc + sw) * TILE + 2);
    (mx0 + 6, mx1 - 7, (r + h) * TILE, 13 * TILE)
}

/// The ore carts on the mine-head's track. While the mine digs, a loaded
/// cart rolls down the rails to the yard, tips, and climbs back empty; a
/// second runs half a round behind once the heap passes a dozen. With the
/// stockpile full the cart stands loaded in the yard and does not move;
/// before any rent day it waits empty at the mouth.
pub(super) fn ore_carts(cv: &mut Img, run: &Run, tick: u32) {
    if !run.home.barony.holds_workings() {
        return;
    }
    let g = &run.home.goods;
    let (xa, xb, top, bottom) = track();
    let (x, y_top, y_bot) = ((xa + xb) / 2 - 10, top + 4, bottom - 20);
    let stamp = |cv: &mut Img, loaded: bool, y: i32| cv.stamp(&mine_cart(loaded), x, y);
    if g.mine_digging() {
        let period = 22 * HZ;
        let carts = if g.have(Good::Ore) >= 12 { 2 } else { 1 };
        for k in 0..carts {
            let p = ((tick + k * period / 2) % period) as f32 / period as f32;
            let (loaded, y) = if p < 0.45 {
                (true, y_top + ((y_bot - y_top) as f32 * p / 0.45) as i32)
            } else if p < 0.55 {
                (p < 0.5, y_bot)
            } else if p < 0.6 {
                (false, y_bot)
            } else {
                (
                    false,
                    y_bot - ((y_bot - y_top) as f32 * (p - 0.6) / 0.4) as i32,
                )
            };
            stamp(cv, loaded, y);
        }
    } else if g.mine_full() {
        stamp(cv, true, y_bot);
    } else {
        stamp(cv, false, y_top);
    }
}

/// The ore heap in the barony's yard: a lump for every piece in the
/// stockpile, heaped from the middle out, so it grows with the digging and
/// shrinks as the Ore-Forge smelts it.
pub(super) fn ore_heap(cv: &mut Img, run: &Run) {
    if !run.home.barony.holds_workings() {
        return;
    }
    let n = run.home.goods.have(Good::Ore) as i32;
    let (sx, sy) = at(
        barony::STOCKPILE_AT.0 * 2.0 + 7.0,
        barony::STOCKPILE_AT.1 * 2.0 + 1.0,
    );
    // A base of timber sleepers the heap sits on.
    cv.rect(sx - 2, sy + 1, 36, 3, 'b');
    cv.line(sx - 2, sy + 1, sx + 33, sy + 1, 'P');
    pile(cv, sx, sy, 32, n, 24, &LUMP, 71);
}

const LUMP: [&str; 4] = [".hJR.", "hJRjJ", "JRjJR", ".jJj."];

/// `n` of a sprite heaped on the foot `(x, y)` of a `w` wide bay: a row on
/// the bottom, each row above one fewer and centred, a little out of true;
/// never more than fit in `room`.
fn pile(cv: &mut Img, x: i32, y: i32, w: i32, n: i32, room: i32, rows: &[&str], seed: u32) {
    let item = Img::from_rows(rows);
    let step = (item.w, (item.h - 1).max(1));
    let mut left = n.min(room);
    // The base is as wide as a heap of this many needs, to the bay's width.
    let widest = (w / step.0).max(1);
    let mut cols = 1;
    while cols * (cols + 1) / 2 < left && cols < widest {
        cols += 1;
    }
    let mut row = 0;
    while left > 0 && row < 8 {
        let take = left.min(cols);
        let inset = (w - take * step.0) / 2;
        for c in 0..take {
            let jx = (hash(c, row, seed) % 3) as i32 - 1;
            cv.stamp(
                &item,
                x + inset + c * step.0 + jx,
                y - item.h - row * step.1,
            );
        }
        left -= take;
        row += 1;
        cols = (cols - 1).max(1);
    }
}

// ── A forge-hall's bins ──────────────────────────────────────────────────

fn sprite(good: Good) -> &'static [&'static str] {
    match good {
        Good::Ore => &LUMP,
        Good::Ingot => &[".VVVVV.", "vVVvvvU", "vvvvvUU", ".UUUUU."],
        Good::Spore => &[".yCy.", "yCYCy", "CYCYC", ".CyC."],
        Good::Bell => &["..o..", ".oOo.", "oOtOo", "oOOOo", "ooooo", "t.R.t"],
        Good::Blade => &["Vv", "vv", "vv", "vv", "vv", "vv", "UU", "oo", "RR"],
        Good::Type => &["JJJJJ", "JhhJG", "JJhhG", "JhJJG", "GGGGG"],
        Good::Brew => &["..o..", ".rRr.", "rRCRr", "rRRRr", "rRRRr", ".rrr."],
    }
}

fn spoil_sprite(spoil: Spoil) -> &'static [&'static str] {
    match spoil {
        Spoil::Wax => &["c9", "c9", "cc", "$$"],
        Spoil::Ember => &[".7.", "7@7", ".8."],
        Spoil::Bone => &["H.i", ".iH", "iH."],
        _ => &["J"],
    }
}

/// What a bin compartment holds: goods from the stockpile, or a spoil
/// from the realm's purse.
#[derive(Clone, Copy)]
enum Stuff {
    Good(Good),
    Spoil(Spoil),
}

impl Stuff {
    fn word(self) -> &'static str {
        match self {
            Stuff::Good(g) => g.word(),
            Stuff::Spoil(s) => s.word(),
        }
    }

    fn rows(self) -> &'static [&'static str] {
        match self {
            Stuff::Good(g) => sprite(g),
            Stuff::Spoil(s) => spoil_sprite(s),
        }
    }

    fn count(self, g: &Goods, purse: &Spoils) -> u32 {
        match self {
            Stuff::Good(good) => g.have(good),
            Stuff::Spoil(s) => purse.get(s),
        }
    }
}

/// A forge's inputs, in bin order, and the one thing it makes.
fn stuffs(f: &Forge) -> Option<(Vec<Stuff>, Stuff)> {
    let r = goods::recipe(f.id)?;
    let mut inputs: Vec<Stuff> = r.from.iter().map(|&(g, _)| Stuff::Good(g)).collect();
    inputs.extend(r.fuel.iter().map(|&(s, _)| Stuff::Spoil(s)));
    Some((inputs, Stuff::Good(r.makes.0)))
}

/// A timber bin `w` by `h` at `(x, y)`: back boards, then a low front lip
/// with the grain showing. `worn` leaves it split and bare.
fn bin(cv: &mut Img, x: i32, y: i32, w: i32, h: i32, worn: bool) {
    cv.rect(x, y, w, h, 'n');
    cv.rect(x + 1, y + 1, w - 2, h - 2, 'b');
    for yy in (y + 5..y + h - 4).step_by(5) {
        cv.line(x + 1, yy, x + w - 2, yy, 'n');
    }
    cv.frame(x, y, w, h, if worn { 'I' } else { 'P' });
    cv.rect(x + 1, y + h - 5, w - 2, 4, if worn { 'I' } else { 'r' });
    cv.line(
        x + 1,
        y + h - 5,
        x + w - 2,
        y + h - 5,
        if worn { 'B' } else { 'O' },
    );
    cv.line(x + 1, y + h - 2, x + w - 2, y + h - 2, 'b');
    if worn {
        // Split boards: a gap or two in the back.
        for k in 0..3 {
            let gx = x + 4 + (hash(k, y, 41) % (w as u32 - 8)) as i32;
            cv.line(gx, y + 2, gx, y + h - 7, 'n');
        }
    }
}

/// A forge-hall's bins: its inputs on the west side, what it makes on the
/// east, a piece for every piece waiting, the stockpile it is starved of
/// marked in red.
pub(super) fn bins(cv: &mut Img, run: &Run, lights: &mut Vec<Light>) {
    let b = &run.home.barony;
    let Some(f) = barony::forge_of(run.dungeon.pack) else {
        return;
    };
    let Some((inputs, out)) = stuffs(f) else {
        return;
    };
    let g = &run.home.goods;
    let built = f.work.and_then(|w| b.progress(w)) == Some(1000);
    let burn = g.burn(b, f.id);
    let (ix, iy, iw, ih) = rect(IN_BIN);
    let (ox, oy, ow, oh) = rect(OUT_BIN);
    bin(cv, ix, iy, iw, ih, !built);
    bin(cv, ox, oy, ow, oh, !built);
    // A bin in ruin holds nothing.
    if !built {
        return;
    }
    let short = match &burn {
        Burn::Starved(of) => Some(of.as_str()),
        _ => None,
    };
    let bay = iw / inputs.len() as i32;
    for (k, s) in inputs.iter().enumerate() {
        let x = ix + k as i32 * bay;
        let n = s.count(g, &run.treasury) as i32;
        pile(
            cv,
            x + 3,
            iy + ih - 5,
            bay - 5,
            n,
            30,
            s.rows(),
            31 + k as u32,
        );
        if short == Some(s.word()) {
            // Starved of it: the bay stands empty, a red cross on its back
            // boards and its lip lined in red.
            let (cx, cy) = (x + bay / 2, iy + ih - 14);
            for d in -2..=2 {
                cv.put(cx + d, cy + d, '7');
                cv.put(cx + d, cy - d, '7');
            }
            cv.line(x + 2, iy + ih - 5, x + bay - 3, iy + ih - 5, '7');
        }
        if k > 0 {
            cv.line(x, iy + 2, x, iy + ih - 6, 'n');
        }
    }
    let n = out.count(g, &run.treasury) as i32;
    if matches!(out, Stuff::Good(Good::Bell)) {
        // Bells hang from a beam over the rack.
        cv.rect(ox + 3, oy + 4, ow - 6, 2, 'P');
        for k in 0..n.min(BELLS_HUNG as i32 + 2) {
            let bell = Img::from_rows(sprite(Good::Bell));
            cv.stamp(&bell, ox + 5 + k * 7, oy + 6);
        }
    } else if matches!(out, Stuff::Good(Good::Blade)) {
        // Blades stand upright in the rack.
        for k in 0..n.min(ARMOURY as i32) {
            let blade = Img::from_rows(sprite(Good::Blade));
            cv.stamp(&blade, ox + 6 + k * 6, oy + oh - 5 - blade.h + 3);
        }
    } else {
        pile(cv, ox + 4, oy + oh - 5, ow - 8, n, 30, out.rows(), 33);
    }
    // A forge that is working lights its bins a little: warm, soft.
    if matches!(burn, Burn::Working) {
        lights.push(fire(ix + iw / 2, iy + ih, 30.0, 0.22));
        lights.push(fire(ox + ow / 2, oy + oh, 30.0, 0.22));
    }
}

fn rect((c, r, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    (c * TILE, r * TILE, w * TILE, h * TILE)
}

// ── The fire a forge shows ───────────────────────────────────────────────

/// The ink at one pixel of a forge's mouth, by how it went: full fire
/// while it works, a low bed of embers while it waits (kindled, banked,
/// rack full), the black of a cold hearth when it is starved.
pub(super) fn mouth(burn: &Burn, x: i32, y: i32, depth: i32, tick: u32) -> char {
    match burn {
        Burn::Working => match hash(x, y + (tick / 3) as i32, 99) % 5 {
            0 => '6',
            1 | 2 => '@',
            _ => '7',
        },
        Burn::Kindled | Burn::Banked | Burn::Full => {
            // Only the bottom of the mouth glows, and it breathes.
            if depth < 5 && hash(x, y + (tick / 12) as i32, 99) % 3 == 0 {
                if hash(x, y, 98).is_multiple_of(2) {
                    '7'
                } else {
                    '8'
                }
            } else {
                'k'
            }
        }
        Burn::Starved(_) | Burn::Cold => {
            if depth < 2 && hash(x, y, 97) % 4 == 0 {
                'g'
            } else {
                'k'
            }
        }
    }
}

/// How bright the light of a forge's fire is, if it has one, by radius and
/// strength.
pub(super) fn glow(burn: &Burn) -> Option<(f32, f32)> {
    match burn {
        Burn::Working => Some((90.0, 0.6)),
        Burn::Kindled | Burn::Banked | Burn::Full => Some((34.0, 0.26)),
        Burn::Starved(_) | Burn::Cold => None,
    }
}

// ── Boards ───────────────────────────────────────────────────────────────

/// How a burning forge stands, in a board's words, with its ink.
fn state(burn: &Burn) -> (String, char) {
    match burn {
        Burn::Working => ("WORKING".into(), '5'),
        Burn::Starved(of) => (format!("STARVED, NO {}", of.to_uppercase()), '7'),
        Burn::Full => ("RACK FULL".into(), 'h'),
        Burn::Banked => ("BANKED".into(), 'h'),
        Burn::Kindled => ("LIT, NO SHIFT YET".into(), 'h'),
        Burn::Cold => ("COLD".into(), 'h'),
    }
}

/// The war table's line for a forge that burns.
pub(super) fn war_state(b: &Barony, g: &Goods, f: &Forge) -> (String, char) {
    let (words, ink) = state(&g.burn(b, f.id));
    (format!("{}: {words}", f.name.to_uppercase()), ink)
}

/// The war table's lines for the stockpiles and for where the racks' goods
/// go, for the forges that burn.
pub(super) fn war_stock(run: &Run) -> Vec<(String, char)> {
    let (b, g, h) = (&run.home.barony, &run.home.goods, &run.home.holdings);
    let mut lines = vec![(
        format!(
            "ORE {}/{}   INGOTS {}/{}",
            g.have(Good::Ore),
            Good::Ore.cap(),
            g.have(Good::Ingot),
            Good::Ingot.cap()
        ),
        'O',
    )];
    if b.is_lit("bell-forge") {
        lines.push((
            format!(
                "BELLS HUNG {}/{}: EACH CALLS {} TO THE WALL",
                h.bells,
                BELLS_HUNG,
                goods::BELL_CALL
            ),
            'h',
        ));
    }
    if b.is_lit("ember-forge") {
        lines.push((
            format!("BLADES {}/{}: EACH ARMS A MAN", h.blades, ARMOURY),
            'h',
        ));
    }
    if b.is_lit("type-foundry") {
        lines.push((
            format!(
                "TYPE {}/{}: {TYPE_PER_NOTICE} PRINT A NOTICE",
                g.have(Good::Type),
                Good::Type.cap()
            ),
            'h',
        ));
    }
    if b.is_lit("spore-kiln") {
        lines.push((
            format!(
                "GRANARY {}/{}: A BREW IS {BREW_SACKS} SACKS",
                h.stores,
                crate::rules::holdings::STORES_CAP
            ),
            'h',
        ));
    }
    lines
}

/// The plate board of a burning forge: how it stands, its recipe, what
/// waits in its bin and on its rack, and the hold that banks it.
pub(super) fn forge_board(run: &Run, f: &Forge, buying: u32) -> Img {
    let b = &run.home.barony;
    let g = &run.home.goods;
    let Some(r) = goods::recipe(f.id) else {
        return home::board(&[
            (format!("{} BURNS", f.name.to_uppercase()), '5'),
            (f.makes.to_uppercase(), 'h'),
        ]);
    };
    let (inputs, out) = stuffs(f).expect("a recipe");
    let held = |purse: bool| -> String {
        let parts: Vec<String> = inputs
            .iter()
            .filter(|s| matches!(s, Stuff::Spoil(_)) == purse)
            .map(|s| format!("{} {}", s.count(g, &run.treasury), s.word().to_uppercase()))
            .collect();
        if parts.is_empty() {
            String::new()
        } else {
            format!(
                "{} {}  ",
                if purse { "PURSE" } else { "BIN" },
                parts.join(" ")
            )
        }
    };
    let rack = match out {
        Stuff::Good(out) => format!(
            "RACK {} {}/{}",
            out.word().to_uppercase(),
            g.have(out),
            out.cap()
        ),
        Stuff::Spoil(_) => String::new(),
    };
    let lines = [
        war_state(b, g, f),
        (r.line().to_uppercase(), 'h'),
        (
            format!("{}{}{rack}", held(false), held(true))
                .trim()
                .to_string(),
            'O',
        ),
        (String::new(), 'h'),
    ];
    let mut im = home::board(&lines);
    let bar_y = 3 + 3 * 9;
    let label = if g.is_banked(f.id) {
        "HOLD E TO STOKE"
    } else {
        "HOLD E TO BANK"
    };
    let bar_w = 40;
    let start = (im.w - (text_width(label) + 4 + bar_w)) / 2;
    let after = im.text(start, bar_y, label, 'O');
    let hold = crate::rules::home::BUY_HOLD;
    let fill = (buying.min(hold) * (bar_w as u32 - 2) / hold) as i32;
    super::super::chrome::meter(
        &mut im,
        (after + 4, bar_y + 1, bar_w, 5),
        fill,
        super::super::ink::ink('5').unwrap_or_default(),
    );
    im
}
