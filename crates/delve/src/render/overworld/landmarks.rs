//! What stands on each countryside lot of the kingdom map: one landmark per
//! place, recognisable at map scale, matching what is inside its room and
//! the picture on its plaque. Built from the shared kit and the plain inks;
//! each fits a lot (64 by 48 pixels) and stands on its bottom row, with its
//! door left of centre over the knight's stand.
use super::ink::{Img, hash};
use super::kit::{self, Heraldry, House, Roof, Top, Trade, Wall};
use super::light::at_dusk;
use crate::rules::RoomKind;

const W: i32 = 64;
const H: i32 = 48;

/// The landmark for a countryside place, or `None` to keep the plain house.
/// Drawn at dusk: a building on the map is scenery, so it wears no lamp,
/// heraldry or bright paint that could read as something happening.
pub fn landmark(kind: RoomKind) -> Option<Img> {
    let im = match kind {
        RoomKind::Lighthouse => lighthouse(),
        RoomKind::Sluice => sluice(),
        RoomKind::Aviary => pigeon_loft(),
        RoomKind::Kitewright => kite_shop(),
        RoomKind::LongbowGreen => archery_butts(),
        RoomKind::Scarecrow => scarecrow(),
        RoomKind::Bathhouse => bathhouse(),
        RoomKind::Fletchery => fletchery(),
        RoomKind::GooseGreen => goose_pen(),
        RoomKind::Fairground => duck_shoot(),
        RoomKind::Mummers => stage(),
        RoomKind::Laundry => wash_vats(),
        RoomKind::HedgeMaze => hedge_maze(),
        RoomKind::Bothy => bothy(),
        RoomKind::Timberyard => timberyard(),
        RoomKind::Orchard => orchard(),
        RoomKind::Ruins => old_watch(),
        RoomKind::Crag => crag(),
        RoomKind::Belfry => belfry(),
        RoomKind::Aqueduct => aqueduct(),
        RoomKind::Priory => priory(),
        RoomKind::Cliffs => cliffs(),
        RoomKind::Saltworks => salt_pans(),
        RoomKind::Strand => strand(),
        RoomKind::Millpond => watermill(),
        RoomKind::SunkenAbbey => sunken_abbey(),
        RoomKind::Riverbank => quay(),
        RoomKind::Causeway => causeway(),
        RoomKind::TollBridge => toll_bridge(),
        RoomKind::Pasture => flower_fields(),
        RoomKind::Downs => cheese_downs(),
        RoomKind::SignalHill => loud_flags(),
        RoomKind::Caravan => caravan(),
        RoomKind::Wheelwright => wheelwright(),
        RoomKind::Windmill => windmill(),
        RoomKind::Hippodrome => hippodrome(),
        RoomKind::Eyrie => eyrie(),
        _ => return None,
    };
    Some(at_dusk(&im))
}

/// Whether a countryside place's landmark has a chimney for smoke to rise
/// from (the rest are yards, fields, stalls and ruins).
pub fn chimney(kind: RoomKind) -> bool {
    matches!(
        kind,
        RoomKind::Bothy
            | RoomKind::Millpond
            | RoomKind::Wheelwright
            | RoomKind::Lighthouse
            | RoomKind::Bathhouse
            | RoomKind::Ranch
            | RoomKind::LandOffice
    )
}

/// An anvil on its block, for a forge's yard.
pub fn anvil() -> Img {
    Img::from_rows(&[
        "jjjjjjjjjjj.",
        ".kGGGGGGGGGj",
        "...kGGGGGk..",
        "....kjjjk...",
        "...BBBBBBB..",
        "...BrrrrrB..",
        "...BBBBBBB..",
    ])
}

fn sheet() -> Img {
    Img::new(W, H)
}

fn house(w: i32, h: i32, wall: Wall, roof: Roof, chimney: bool) -> Img {
    kit::house(&House {
        w,
        h,
        roof_h: h * 2 / 5,
        wall,
        roof,
        door_glow: false,
        windows: if w >= 30 { 2 } else { 1 },
        lit: false,
        chimney,
    })
}

/// Still water: its corners worn round, the bank's shadow along its top,
/// short ripples on it and a broken pale line where it laps the near bank.
fn water(im: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    for yy in y..y + h {
        for xx in x..x + w {
            let (dl, dr, dt, db) = (xx - x, x + w - 1 - xx, yy - y, y + h - 1 - yy);
            let worn = |a: i32, b: i32| a + b < 2;
            if worn(dl, dt) || worn(dr, dt) || worn(dl, db) || worn(dr, db) {
                continue;
            }
            let ripple = (yy - y) % 3 == 1 && hash(xx / 3, yy, 7) % 4 == 0 && dt > 0;
            let ch = if dt == 0 {
                'S'
            } else if db == 0 {
                if hash(xx, yy, 9) % 3 == 0 { 'q' } else { 'u' }
            } else if ripple {
                'Q'
            } else {
                'q'
            };
            im.put(xx, yy, ch);
        }
    }
}

/// Clear an ellipse back to nothing (the hole in a wheel).
fn hollow(im: &mut Img, cx: f32, cy: f32, rx: f32, ry: f32) {
    for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
        for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
            let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
            if dx * dx + dy * dy <= 1.0 {
                im.clear(x, y);
            }
        }
    }
}

/// A post from `top` down to `base`.
fn post(im: &mut Img, x: i32, top: i32, base: i32) {
    im.line(x, top, x, base, 'r');
    im.line(x + 1, top, x + 1, base, 'B');
}

fn steam(im: &mut Img, x: i32, y: i32, seed: u32) {
    im.stamp(&kit::smoke(seed).recolor(&[('i', 'h'), ('h', 'J')]), x, y);
}

/// A round tree, with apples if `fruit`.
fn tree(im: &mut Img, cx: i32, base: i32, fruit: bool) {
    im.rect(cx - 1, base - 6, 3, 6, 'I');
    im.ellipse(cx as f32, (base - 11) as f32, 7.0, 6.0, 'E');
    im.ellipse(cx as f32 - 2.0, (base - 13) as f32, 3.5, 3.0, 'l');
    if fruit {
        for (dx, dy) in [(-4, -10), (2, -14), (4, -8), (-1, -7), (-3, -15)] {
            im.put(cx + dx, base + dy, 'R');
            im.put(cx + dx + 1, base + dy, 'p');
        }
    }
}

/// A short fence of pales along `y`.
fn fence(im: &mut Img, x: i32, y: i32, w: i32) {
    im.line(x, y, x + w - 1, y, 'r');
    im.line(x, y + 3, x + w - 1, y + 3, 'B');
    for px in (x..x + w).step_by(4) {
        im.line(px, y - 1, px, y + 5, 'P');
    }
}

fn lighthouse() -> Img {
    let mut im = sheet();
    // The keeper's hut at its foot.
    im.stamp(&house(26, 22, Wall::Plaster, Roof::Slate, true), 6, 26);
    // A tapering tower, banded, with a gallery and a warm lamp.
    for y in 8..46 {
        let half = 5 + (y - 8) / 9;
        let band = (y / 6) % 2 == 0;
        for x in 44 - half..=44 + half {
            let edge = x == 44 - half || x == 44 + half;
            im.put(x, y, if edge { 'x' } else if band { 'h' } else { 'u' });
        }
    }
    im.rect(37, 6, 15, 2, 'k');
    im.rect(39, 1, 11, 5, 'K');
    im.rect(41, 2, 7, 3, 't');
    im.rect(43, 2, 3, 3, 'T');
    im.line(39, 0, 49, 0, 'S');
    im.rect(42, 40, 5, 6, 'b');
    im.rect(0, 46, W, 2, 'S');
    im
}

fn sluice() -> Img {
    let mut im = sheet();
    water(&mut im, 0, 36, W, 12);
    // Two stone piers and the gate between them.
    for x in [8, 46] {
        im.rect(x, 10, 10, 34, 'j');
        im.rect(x + 1, 10, 8, 2, 'G');
        im.frame(x, 10, 10, 34, 'x');
    }
    im.rect(18, 18, 28, 22, 'B');
    for y in (19..40).step_by(4) {
        im.line(18, y, 45, y, 'r');
    }
    im.rect(18, 8, 28, 3, 'G');
    im.frame(18, 8, 28, 3, 'x');
    // The valve wheel on the beam, and the race through the gate.
    im.ellipse(32.0, 5.0, 5.0, 5.0, 'o');
    hollow(&mut im, 32.0, 5.0, 3.5, 3.5);
    for (dx, dy) in [(0, -4), (0, 4), (-4, 0), (4, 0)] {
        im.line(32, 5, 32 + dx, 5 + dy, 'R');
    }
    im.put(32, 5, 'O');
    for x in 20..44 {
        if hash(x, 0, 9) % 2 == 0 {
            im.put(x, 41, 'U');
        }
        im.put(x, 42, 'Q');
    }
    im
}

fn pigeon_loft() -> Img {
    let mut im = sheet();
    // A tall dovecote on a post, a cone roof and rows of holes.
    post(&mut im, 30, 28, 46);
    im.rect(16, 12, 32, 18, 'P');
    im.frame(16, 12, 32, 18, 'b');
    for y in [15, 21] {
        for x in (19..46).step_by(5) {
            im.rect(x, y, 3, 3, 'k');
            im.line(x - 1, y + 3, x + 3, y + 3, 'o');
        }
    }
    for y in 0..12 {
        let half = 3 + y * 17 / 12;
        im.line(32 - half, y, 32 + half, y, if y % 3 == 0 { 'B' } else { 'R' });
    }
    // Pigeons on the ledges and the ridge.
    let pigeon = kit::raven_perched().recolor(&[('k', 'G'), ('K', 'j'), ('g', 'h'), ('X', 'J')]);
    for (x, y) in [(20, 24), (36, 18), (29, -1), (42, 24)] {
        im.stamp(&pigeon, x, y);
    }
    im
}

/// A diamond kite with its tail.
fn kite(im: &mut Img, cx: i32, cy: i32, ink: char, trim: char) {
    for dy in -5i32..=5 {
        let half = 4 - dy.abs() * 4 / 5;
        im.line(cx - half, cy + dy, cx + half, cy + dy, ink);
    }
    im.line(cx, cy - 5, cx, cy + 5, trim);
    im.line(cx - 4, cy, cx + 4, cy, trim);
    for k in 0..6 {
        im.put(cx + k % 2, cy + 6 + k, 'h');
    }
}

fn kite_shop() -> Img {
    let mut im = sheet();
    im.stamp(&house(34, 30, Wall::Timber, Roof::Tile, false), 4, 18);
    kite(&mut im, 46, 7, 'R', 'T');
    kite(&mut im, 57, 15, 'Q', 'V');
    kite(&mut im, 41, 22, 't', 'B');
    // Strings back to the shop's eave.
    im.line(30, 22, 46, 13, 'J');
    im.line(34, 24, 57, 21, 'J');
    im.line(36, 26, 41, 28, 'J');
    im
}

/// A straw butt and its painted face.
fn butt(im: &mut Img, cx: i32, cy: i32) {
    im.line(cx - 6, cy + 10, cx - 2, cy, 'B');
    im.line(cx + 6, cy + 10, cx + 2, cy, 'B');
    im.ellipse(cx as f32, cy as f32, 8.0, 8.0, 'O');
    im.ellipse(cx as f32, cy as f32, 6.0, 6.0, 'T');
    im.ellipse(cx as f32, cy as f32, 4.0, 4.0, 'R');
    im.ellipse(cx as f32, cy as f32, 2.0, 2.0, 't');
}

fn archery_butts() -> Img {
    let mut im = sheet();
    butt(&mut im, 14, 30);
    butt(&mut im, 46, 28);
    // Arrows in the straw, and a shooting line.
    for (x, y) in [(12, 28), (16, 32), (44, 27)] {
        im.line(x, y, x + 5, y - 3, 'h');
        im.put(x + 5, y - 3, 'G');
        im.put(x + 6, y - 4, 'T');
    }
    im.line(2, 45, 61, 45, 'P');
    im
}

fn scarecrow() -> Img {
    let mut im = sheet();
    // Rows of crop: each plant a tuft lit on its top, set a little off its
    // neighbours, a gap here and there.
    for (row, y) in (28..47).step_by(4).enumerate() {
        for x in (row as i32 % 2..W).step_by(3) {
            let h = hash(x, y, 3);
            if h % 7 == 0 {
                continue;
            }
            let bx = x + (h >> 4) as i32 % 2;
            im.put(bx, y, if (h >> 6) % 3 == 0 { 'L' } else { 'm' });
            im.put(bx, y + 1, 'N');
            im.put(bx + 1, y + 1, 'l');
        }
    }
    // The scarecrow: cross, straw head, battered hat, ragged coat.
    post(&mut im, 31, 10, 46);
    im.line(18, 18, 45, 18, 'r');
    im.rect(25, 16, 14, 14, 'P');
    im.rect(23, 17, 3, 6, 'P');
    im.rect(38, 17, 3, 6, 'P');
    for x in [24, 27, 33, 37] {
        im.line(x, 30, x - 1, 33, 'O');
    }
    im.line(17, 18, 15, 21, 'O');
    im.line(46, 18, 48, 21, 'O');
    im.ellipse(32.0, 11.0, 4.0, 4.0, 'O');
    im.put(30, 10, 'k');
    im.put(33, 10, 'k');
    im.rect(26, 4, 12, 2, 'b');
    im.rect(28, 1, 8, 4, 'I');
    // A crow on the arm.
    im.stamp(&kit::raven_perched(), 40, 12);
    im
}

fn bathhouse() -> Img {
    let mut im = sheet();
    im.stamp(&house(46, 34, Wall::Plaster, Roof::Tile, false), 2, 14);
    // A shallow dome over the hot room, and its steam.
    im.ellipse(38.0, 16.0, 9.0, 6.0, 'G');
    im.ellipse(37.0, 14.0, 6.0, 3.5, 'h');
    im.rect(29, 16, 19, 2, 'j');
    steam(&mut im, 30, -2, 3);
    steam(&mut im, 42, 0, 9);
    // A tub outside the door.
    im.rect(50, 38, 12, 8, 'B');
    im.line(50, 41, 61, 41, 'r');
    im.rect(51, 37, 10, 2, 'U');
    steam(&mut im, 50, 22, 5);
    im
}

/// A bundle of arrows standing in a barrel.
fn arrows(im: &mut Img, x: i32, base: i32) {
    im.rect(x, base - 8, 9, 8, 'B');
    im.line(x, base - 5, x + 8, base - 5, 'r');
    for k in 0..5 {
        let ax = x + 1 + k * 2;
        im.line(ax, base - 18, ax, base - 8, 'O');
        im.put(ax, base - 19, 'h');
        im.put(ax, base - 18, 'T');
    }
}

fn fletchery() -> Img {
    let mut im = sheet();
    im.stamp(&kit::workshop(Trade::Weaver, false), 2, 3);
    arrows(&mut im, 38, 46);
    arrows(&mut im, 50, 46);
    // A long bow hung on the wall.
    for y in 0..14 {
        let bend = (7 - (y - 7i32).abs()) / 3;
        im.put(36 - bend, 18 + y, 'R');
    }
    im.line(36, 18, 36, 31, 'h');
    im
}

/// A big white goose, side on.
fn goose(im: &mut Img, x: i32, y: i32) {
    let g = Img::from_rows(&[
        "......VV....",
        ".....VVVVtt.",
        ".....VVV....",
        "....VVV.....",
        "VVVVVVVV....",
        ".VVVVVVVv...",
        "..vVVVVv....",
        "...t..t.....",
        "...tt.tt....",
    ]);
    im.stamp(&g, x, y);
}

fn goose_pen() -> Img {
    let mut im = sheet();
    // A picnic cloth in checks and the pen's fence.
    for y in 30..40 {
        for x in 6..26 {
            im.put(x, y, if (x / 3 + y / 3) % 2 == 0 { 'R' } else { 'T' });
        }
    }
    im.rect(10, 32, 4, 3, 'o');
    fence(&mut im, 0, 22, W);
    fence(&mut im, 0, 42, W);
    for y in [22, 42] {
        im.line(0, y, 0, y + 4, 'P');
        im.line(63, y, 63, y + 4, 'P');
    }
    goose(&mut im, 34, 28);
    goose(&mut im, 48, 31);
    im
}

/// A striped awning `w` wide over posts.
fn awning(im: &mut Img, x: i32, y: i32, w: i32, a: char, b: char) {
    for row in 0..6 {
        for col in x..x + w {
            im.put(col, y + row, if (col / 4) % 2 == 0 { a } else { b });
        }
    }
    for col in (x..x + w).step_by(4) {
        im.put(col + 1, y + 6, if (col / 4) % 2 == 0 { a } else { b });
    }
}

fn duck_shoot() -> Img {
    let mut im = sheet();
    awning(&mut im, 2, 8, 60, 'T', 'R');
    post(&mut im, 3, 14, 46);
    post(&mut im, 59, 14, 46);
    im.rect(5, 16, 54, 20, 'b');
    // A row of tin ducks on the rail, and the counter.
    let duck = Img::from_rows(&[".tt..", "tktOO", "ttt..", "tttt.", ".tt.."]);
    for x in (9..56).step_by(8) {
        im.stamp(&duck, x, 22);
    }
    im.line(5, 28, 58, 28, 'r');
    im.rect(5, 36, 54, 10, 'P');
    im.line(5, 36, 58, 36, 'o');
    im.stamp(&kit::barrels(), 46, 38);
    im
}

fn stage() -> Img {
    let mut im = sheet();
    // A raised stage, curtains drawn back, bunting along the top.
    im.rect(2, 34, 60, 12, 'P');
    im.line(2, 34, 61, 34, 'o');
    for x in (4..60).step_by(6) {
        im.line(x, 35, x, 45, 'B');
    }
    post(&mut im, 3, 4, 34);
    post(&mut im, 59, 4, 34);
    im.rect(3, 3, 59, 3, 'B');
    for y in 6..34 {
        let w = 12 - (y - 6) / 4;
        im.line(5, y, 5 + w, y, if y % 4 == 0 { 'p' } else { 'R' });
        im.line(58 - w, y, 58, y, if y % 4 == 0 { 'p' } else { 'R' });
    }
    for x in (6..58).step_by(6) {
        im.put(x, 7, 'T');
        im.put(x + 1, 8, 'T');
        im.put(x + 2, 7, 'Q');
    }
    // A mummer's mask hung centre stage.
    im.ellipse(32.0, 20.0, 5.0, 6.0, 'T');
    im.put(30, 19, 'k');
    im.put(34, 19, 'k');
    im.line(30, 23, 34, 23, 'k');
    im
}

/// An oversized wash vat full of suds.
fn vat(im: &mut Img, x: i32, y: i32) {
    im.rect(x, y, 18, 12, 'B');
    im.line(x, y + 4, x + 17, y + 4, 'G');
    im.line(x, y + 9, x + 17, y + 9, 'G');
    im.rect(x + 1, y - 2, 16, 3, 'Q');
    for k in 0..5 {
        im.ellipse((x + 2 + k * 3) as f32, (y - 3) as f32, 1.6, 1.4, 'v');
    }
}

fn wash_vats() -> Img {
    let mut im = sheet();
    vat(&mut im, 4, 34);
    vat(&mut im, 26, 34);
    // A washing line with the king's smalls.
    post(&mut im, 46, 6, 46);
    post(&mut im, 61, 6, 46);
    im.line(2, 10, 61, 10, 'J');
    for (x, w, ink) in [(4, 6, 'h'), (13, 5, 'R'), (21, 8, 'T'), (33, 5, 'u'), (51, 6, 'h')] {
        im.rect(x, 11, w, 7, ink);
    }
    // The royal sock.
    im.rect(40, 11, 3, 6, 'R');
    im.rect(40, 16, 5, 2, 'R');
    post(&mut im, 1, 6, 30);
    im
}

fn hedge_maze() -> Img {
    let mut im = sheet();
    im.rect(2, 14, 60, 32, 'F');
    // Lanes cut through the hedge.
    for (x, y, w, h) in [
        (6, 18, 52, 3),
        (6, 18, 3, 24),
        (14, 26, 3, 16),
        (14, 26, 30, 3),
        (22, 34, 30, 3),
        (55, 18, 3, 20),
        (40, 26, 3, 9),
        (6, 40, 14, 3),
        (28, 40, 3, 6),
    ] {
        im.rect(x, y, w, h, 'b');
    }
    for y in 14..46 {
        for x in 2..62 {
            if im.get(x, y) == super::ink::ink('F') && hash(x, y, 4) % 5 == 0 {
                im.put(x, y, 'E');
            }
        }
    }
    im.line(2, 14, 61, 14, 'l');
    im.frame(2, 14, 60, 32, 'D');
    // A mushroom by the gate.
    im.ellipse(52.0, 9.0, 6.0, 4.0, 'R');
    for (x, y) in [(49, 8), (53, 7), (55, 10)] {
        im.put(x, y, 'T');
    }
    im.rect(51, 12, 3, 4, 'T');
    im
}

fn bothy() -> Img {
    let mut im = sheet();
    im.stamp(&house(36, 28, Wall::Dark, Roof::Thatch, true), 2, 20);
    // The cook's cauldron on its fire.
    im.line(42, 30, 50, 30, 'G');
    im.line(42, 30, 42, 46, 'B');
    im.line(58, 30, 50, 30, 'G');
    im.line(58, 30, 58, 46, 'B');
    im.ellipse(50.0, 39.0, 7.0, 5.0, 'k');
    im.line(44, 35, 56, 35, 'g');
    im.rect(45, 34, 11, 1, 'm');
    for x in [45, 49, 53] {
        im.put(x, 45, 'o');
        im.put(x + 1, 44, 'R');
    }
    steam(&mut im, 44, 16, 6);
    im
}

/// A log lying end on: rings and bark.
fn log_end(im: &mut Img, cx: i32, cy: i32) {
    im.ellipse(cx as f32, cy as f32, 4.5, 4.5, 'I');
    im.ellipse(cx as f32, cy as f32, 3.5, 3.5, 'O');
    im.ellipse(cx as f32, cy as f32, 1.6, 1.6, 'o');
    im.put(cx, cy, 'R');
}

fn timberyard() -> Img {
    let mut im = sheet();
    im.stamp(&house(30, 30, Wall::Timber, Roof::Slate, false), 0, 18);
    // A stacked log pile, ends out.
    for (row, n) in [(0, 3), (1, 2), (2, 1)] {
        for k in 0..n {
            log_end(&mut im, 38 + row * 4 + k * 9, 42 - row * 8);
        }
    }
    // A saw-horse with a log on it.
    im.line(54, 38, 58, 46, 'B');
    im.line(58, 38, 54, 46, 'B');
    im.rect(48, 36, 15, 3, 'P');
    im
}

fn orchard() -> Img {
    let mut im = sheet();
    for (cx, base) in [(10, 30), (32, 28), (54, 30), (21, 46), (43, 46)] {
        tree(&mut im, cx, base, true);
    }
    // A basket of windfalls.
    im.rect(28, 41, 8, 5, 'P');
    im.line(28, 41, 35, 41, 'o');
    for x in [29, 31, 33] {
        im.put(x, 40, 'R');
    }
    im
}

fn old_watch() -> Img {
    let mut im = sheet();
    im.stamp(&kit::ruins(), 0, 2);
    // A tattered flag still flying off the broken top.
    post(&mut im, 52, 2, 46);
    im.rect(54, 3, 9, 6, 'h');
    im.clear(62, 5);
    im.clear(60, 8);
    im.line(54, 5, 61, 5, 'G');
    im
}

fn crag() -> Img {
    let mut im = sheet();
    let rock = kit::blob(
        W,
        H,
        &[(22.0, 32.0, 20.0, 16.0), (40.0, 26.0, 16.0, 22.0), (52.0, 38.0, 12.0, 10.0)],
        &['k', 'x', 'g', 'j', 'G'],
        0.4,
        11,
        1.5,
    );
    im.stamp(&rock, 0, 0);
    // Snow on the ridge.
    for x in 0..W {
        let top = (0..H).find(|&y| im.get(x, y).is_some());
        if let Some(top) = top {
            im.line(x, top, x, top + 1 + (hash(x, 0, 5) % 2) as i32, 'h');
        }
    }
    // The brass weather rooster on its rod, and a beacon flame.
    let peak = (0..H).find(|&y| im.get(40, y).is_some()).unwrap_or(10);
    im.line(40, peak - 9, 40, peak, 'h');
    im.rect(38, peak - 12, 5, 3, 'o');
    im.put(43, peak - 12, 'O');
    im.put(37, peak - 13, 'O');
    im.stamp(&kit::brazier(0, 3), 14, 18);
    im
}

fn belfry() -> Img {
    let mut im = sheet();
    im.stamp(&kit::tower(26, 46, 12, Top::Cone(Roof::Slate), Wall::Stone), 19, 2);
    // The open bell chamber, its bell showing.
    im.rect(24, 16, 16, 12, 'k');
    im.ellipse(32.0, 23.0, 5.0, 5.0, 'o');
    im.rect(27, 23, 11, 4, 'o');
    im.line(26, 27, 38, 27, 'O');
    im.put(32, 28, 'r');
    im.line(29, 19, 33, 19, 'O');
    im
}

fn aqueduct() -> Img {
    let mut im = sheet();
    im.rect(0, 10, W, 36, 'j');
    im.rect(0, 6, W, 4, 'G');
    // The water channel along the top, dry at one end.
    im.rect(0, 4, 44, 3, 'Q');
    im.line(0, 4, 43, 4, 'U');
    for x in [6, 27, 48] {
        super::kit::arch(&mut im, x, 22, 12, 24, false);
        im.rect(x + 2, 26, 8, 20, 'k');
    }
    for y in (12..46).step_by(5) {
        for x in 0..W {
            if (x + y) % 7 == 0 {
                im.put(x, y, 'x');
            }
        }
    }
    im.outline_inside('x');
    im
}

fn priory() -> Img {
    let mut im = sheet();
    // A cloister wall with its arches, and the chapel behind.
    im.rect(0, 32, W, 14, 'j');
    for x in (2..W).step_by(9) {
        im.rect(x, 36, 6, 10, 'k');
    }
    im.line(0, 32, 63, 32, 'G');
    im.stamp(&kit::chapel(false), 16, 2);
    im
}

fn cliffs() -> Img {
    let mut im = sheet();
    let rock = kit::blob(
        W,
        30,
        &[(16.0, 20.0, 18.0, 14.0), (44.0, 18.0, 22.0, 14.0)],
        &['k', 'x', 'g', 'j'],
        0.5,
        21,
        1.5,
    );
    im.stamp(&rock, 0, 18);
    // The sea below the drop on the right, and a railed overlook on top.
    water(&mut im, 50, 40, 14, 8);
    let top = (0..H).find(|&y| im.get(30, y).is_some()).unwrap_or(20);
    im.line(8, top - 4, 50, top - 4, 'r');
    for x in (8..51).step_by(6) {
        im.line(x, top - 4, x, top, 'B');
    }
    // A telescope on its tripod.
    im.line(20, top - 12, 26, top - 15, 'o');
    im.line(22, top - 13, 20, top, 'B');
    im.line(22, top - 13, 25, top, 'B');
    im
}

fn salt_pans() -> Img {
    let mut im = sheet();
    // A grid of shallow pans and their salt heaps.
    for (x, y) in [(0, 26), (16, 26), (0, 37), (16, 37), (32, 37)] {
        im.rect(x, y, 15, 10, 'S');
        im.frame(x, y, 15, 10, 'j');
        for k in 0..4 {
            im.put(x + 2 + k * 3, y + 3 + k % 2 * 3, 'U');
            im.put(x + 3 + k * 3, y + 3 + k % 2 * 3, 'u');
        }
    }
    for (x, y) in [(36, 30), (44, 32)] {
        for row in 0..7 {
            im.line(x - row, y + row, x + row, y + row, if row % 3 == 0 { 'U' } else { 'v' });
        }
    }
    im.stamp(&house(22, 22, Wall::Timber, Roof::Slate, false), 40, 4);
    im
}

fn strand() -> Img {
    let mut im = sheet();
    // Two striped bathing huts on the sand and an upturned boat.
    for (x, a, b) in [(4, 'T', 'u'), (24, 'T', 'R')] {
        for y in 14..40 {
            for col in x..x + 14 {
                im.put(col, y, if (col - x) / 3 % 2 == 0 { a } else { b });
            }
        }
        for y in 0..7 {
            im.line(x + 6 - y, 8 + y, x + 7 + y, 8 + y, 'B');
        }
        im.rect(x + 5, 30, 4, 10, 'b');
    }
    im = super::kit::shaded_rim(&im);
    im.ellipse(52.0, 42.0, 10.0, 4.0, 'B');
    im.line(43, 42, 61, 42, 'r');
    // The chamber pot for the putting game.
    im.rect(44, 30, 6, 5, 'J');
    im.line(43, 30, 50, 30, 'h');
    im
}

fn watermill() -> Img {
    let mut im = sheet();
    im.stamp(&house(36, 34, Wall::Stone, Roof::Tile, true), 0, 14);
    water(&mut im, 34, 38, 30, 10);
    // The wheel turning in the race.
    im.ellipse(48.0, 31.0, 12.0, 12.0, 'B');
    hollow(&mut im, 48.0, 31.0, 10.0, 10.0);
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::TAU / 8.0;
        im.line(48, 31, 48 + (a.cos() * 11.0) as i32, 31 + (a.sin() * 11.0) as i32, 'r');
    }
    im.ellipse(48.0, 31.0, 2.0, 2.0, 'k');
    im
}

fn sunken_abbey() -> Img {
    let mut im = sheet();
    im.stamp(&kit::ruins(), 8, 0);
    // The flood has the lower courses.
    water(&mut im, 0, 36, W, 12);
    for x in 12..54 {
        if hash(x, 1, 2) % 2 == 0 {
            im.put(x, 36, 'U');
        }
    }
    im
}

fn quay() -> Img {
    let mut im = sheet();
    water(&mut im, 0, 30, W, 18);
    im.stamp(&kit::docks(), 0, 22);
    // A boat at the quay and the buckets waiting.
    im.ellipse(46.0, 40.0, 12.0, 4.0, 'B');
    im.line(35, 38, 57, 38, 'r');
    post(&mut im, 46, 20, 38);
    im.rect(48, 22, 8, 10, 'h');
    for x in [6, 14, 22] {
        im.rect(x, 18, 6, 6, 'G');
        im.line(x, 18, x + 5, 18, 'h');
        im.line(x + 1, 16, x + 4, 16, 'J');
    }
    im
}

fn causeway() -> Img {
    let mut im = sheet();
    water(&mut im, 0, 34, W, 14);
    // A low stone causeway on arches.
    im.rect(0, 26, W, 10, 'j');
    im.line(0, 26, 63, 26, 'G');
    for x in (4..W).step_by(14) {
        im.ellipse((x + 4) as f32, 38.0, 5.0, 4.0, 'k');
    }
    // The troll's toll hut and his sign.
    im.stamp(&house(22, 22, Wall::Dark, Roof::Thatch, false), 38, 6);
    post(&mut im, 14, 10, 26);
    im.rect(8, 10, 14, 7, 'P');
    im.frame(8, 10, 14, 7, 'b');
    im.line(10, 13, 19, 13, 'k');
    im
}

fn toll_bridge() -> Img {
    let mut im = sheet();
    water(&mut im, 0, 36, W, 12);
    // A timber bridge on piles.
    im.rect(0, 30, W, 4, 'R');
    im.line(0, 29, 63, 29, 'o');
    for x in (2..W).step_by(10) {
        im.rect(x, 34, 3, 12, 'B');
    }
    im.line(0, 24, 63, 24, 'P');
    // The toll booth and its striped barrier.
    im.stamp(&house(20, 24, Wall::Plaster, Roof::Tile, false), 2, 5);
    for x in 24..50 {
        im.put(x, 20, if (x / 3) % 2 == 0 { 'h' } else { 'R' });
        im.put(x, 21, if (x / 3) % 2 == 0 { 'h' } else { 'R' });
    }
    post(&mut im, 23, 18, 29);
    im
}

fn flower_fields() -> Img {
    let mut im = sheet();
    // Rows of flowers, each plant a small tuft with its bloom on top, set
    // a little off its neighbours, a gap here and there.
    for (row, y) in (18..46).step_by(5).enumerate() {
        for x in (3 + row as i32 % 2..61).step_by(3) {
            let h = hash(x, y, 8);
            if h % 6 == 0 {
                continue;
            }
            let bx = x + (h >> 4) as i32 % 2;
            im.put(bx, y + 2, 'F');
            im.put(bx + 1, y + 2, 'E');
            im.put(bx, y + 1, 'l');
            im.put(bx, y, ['T', 'R', 'Q', 't', 'h'][(h >> 6) as usize % 5]);
        }
    }
    fence(&mut im, 0, 12, W);
    im
}

fn cheese_downs() -> Img {
    let mut im = sheet();
    // A green rise, a stack of cheeses and a goat with a basket.
    im.ellipse(32.0, 50.0, 34.0, 16.0, 'm');
    for (x, y, w) in [(6, 34, 16), (8, 28, 12), (10, 23, 8)] {
        im.rect(x, y, w, 5, 't');
        im.line(x, y, x + w - 1, y, 'T');
        im.line(x, y + 4, x + w - 1, y + 4, 'o');
    }
    let goat = Img::from_rows(&[
        "h.h.........",
        ".hh.........",
        ".hhJhhhhhh..",
        "..hhhhhhhhh.",
        "...hhhhhhh..",
        "...J.J..J.J.",
        "...J.J..J.J.",
    ]);
    im.stamp(&goat, 38, 30);
    im.rect(44, 28, 5, 4, 'P');
    // The cheese-chuck tee's flag.
    im.line(56, 24, 56, 38, 'h');
    im.rect(57, 24, 5, 4, 'R');
    im
}

fn loud_flags() -> Img {
    let mut im = sheet();
    im.ellipse(32.0, 50.0, 32.0, 14.0, 'm');
    im.ellipse(32.0, 50.0, 28.0, 11.0, 'A');
    // Three tall poles, three big flags.
    for (x, top, a, b) in [(12, 6, 'R', 'T'), (32, 2, 'Q', 'V'), (52, 8, 't', 'B')] {
        post(&mut im, x, top, 44);
        for row in 0..9 {
            for col in 0..12 {
                im.put(x + 2 + col, top + 1 + row + col / 6, if (col / 4 + row / 3) % 2 == 0 { a } else { b });
            }
        }
    }
    im
}

fn caravan() -> Img {
    let mut im = sheet();
    im.stamp(&kit::tent(Heraldry::Blue), 0, 18);
    im.stamp(&kit::tent(Heraldry::Red), 20, 14);
    im.stamp(&kit::wagon(false, 0, None), 38, 30);
    im
}

/// A cart wheel leaning on a wall.
fn wheel(im: &mut Img, cx: i32, cy: i32, r: f32) {
    im.ellipse(cx as f32, cy as f32, r, r, 'B');
    hollow(im, cx as f32, cy as f32, r - 1.5, r - 1.5);
    for k in 0..6 {
        let a = k as f32 * std::f32::consts::TAU / 6.0;
        im.line(cx, cy, cx + (a.cos() * r) as i32, cy + (a.sin() * r) as i32, 'o');
    }
    im.ellipse(cx as f32, cy as f32, 1.5, 1.5, 'k');
}

fn wheelwright() -> Img {
    let mut im = sheet();
    im.stamp(&house(34, 32, Wall::Timber, Roof::Tile, true), 0, 16);
    wheel(&mut im, 44, 38, 8.0);
    wheel(&mut im, 56, 40, 6.0);
    // And one square wheel, the house speciality.
    im.frame(36, 18, 12, 12, 'B');
    im.line(36, 18, 47, 29, 'o');
    im.line(47, 18, 36, 29, 'o');
    im.put(41, 23, 'k');
    im
}

fn windmill() -> Img {
    let mut im = sheet();
    let mill = kit::windmill(0);
    im.stamp(&mill, (W - mill.w) / 2, H - mill.h);
    // Sacks of flour at its door.
    im.stamp(&kit::sacks(2), 40, 36);
    im
}

fn hippodrome() -> Img {
    let mut im = sheet();
    // A rusty stand along the back.
    im.rect(4, 2, 56, 10, 'B');
    for y in (3..12).step_by(3) {
        im.line(5, y, 58, y, 'p');
    }
    for x in (6..58).step_by(7) {
        im.put(x, 4, 'h');
        im.put(x + 3, 7, 'J');
    }
    // The oval track, its grass and the spina down the middle: packed dirt
    // with a wheel rut round it and clods thrown up, a kerb lit on the
    // north-west and shaded to the south-east, the infield's grass in clumps.
    for y in 0..H {
        for x in 0..W {
            let (dx, dy) = (x as f32 + 0.5 - 32.0, y as f32 + 0.5 - 31.0);
            let ro = ((dx / 31.0).powi(2) + (dy / 16.0).powi(2)).sqrt();
            let ri = ((dx / 23.0).powi(2) + (dy / 9.0).powi(2)).sqrt();
            if ro >= 1.0 {
                continue;
            }
            let clump = super::ink::vnoise(x as f32 / 3.0, y as f32 / 2.0, 914);
            let ink = if ri < 1.0 {
                if ri > 0.9 {
                    'F'
                } else if clump > 0.74 {
                    'l'
                } else {
                    'E'
                }
            } else {
                let f = (ri - 1.0) / (ri - ro).max(1e-3);
                let lit = dx * 0.5 + dy < 0.0;
                if f > 0.86 {
                    if lit { 'r' } else { 'B' }
                } else if f < 0.1 || (0.44..0.56).contains(&f) {
                    'I'
                } else if clump > 0.7 {
                    'r'
                } else {
                    'P'
                }
            };
            im.put(x, y, ink);
        }
    }
    im.rect(14, 30, 36, 3, 'j');
    im.line(14, 30, 49, 30, 'G');
    for x in [14, 49] {
        im.rect(x - 1, 25, 3, 6, 'G');
        im.put(x, 24, 'h');
    }
    // A chariot rounding the far turn.
    im.rect(50, 40, 6, 4, 'R');
    im.ellipse(52.0, 45.0, 2.0, 2.0, 'B');
    im.line(56, 41, 61, 41, 'o');
    im
}

/// The Griffin Eyrie: a crag with a great nest of branches on its top and a
/// griffin perched on the rim, wings half open; a griffin master's hut at
/// its foot with a lit doorway.
fn eyrie() -> Img {
    let mut im = sheet();
    let rock = kit::blob(
        W,
        H,
        &[(30.0, 34.0, 18.0, 14.0), (36.0, 24.0, 12.0, 14.0), (16.0, 40.0, 14.0, 8.0)],
        &['k', 'x', 'g', 'j', 'G'],
        0.4,
        23,
        1.5,
    );
    im.stamp(&rock, 0, 0);
    // The nest: woven branches in a bowl on the summit.
    let top = (0..H).find(|&y| im.get(36, y).is_some()).unwrap_or(12);
    im.ellipse(36.0, top as f32, 11.0, 3.0, 'b');
    for k in 0..12 {
        let x = 26 + k * 2;
        im.line(x, top - 1 + (k % 2), x + 3, top + 1 - (k % 2), if k % 3 == 0 { 'P' } else { 'I' });
    }
    // The griffin on the rim: tawny body, white head, a gold beak, its
    // wings half open.
    let (gx, gy) = (34, top - 6);
    im.rect(gx, gy, 6, 4, 'o');
    im.rect(gx + 1, gy + 4, 1, 2, 'O');
    im.rect(gx + 4, gy + 4, 1, 2, 'O');
    im.rect(gx + 5, gy - 3, 3, 3, 'i');
    im.put(gx + 8, gy - 2, '5');
    im.put(gx + 6, gy - 2, 'k');
    im.line(gx - 6, gy - 5, gx, gy, 'R');
    im.line(gx - 6, gy - 4, gx, gy + 1, 'r');
    im.line(gx - 5, gy - 5, gx - 1, gy - 1, 'O');
    im.line(gx - 1, gy + 2, gx - 4, gy + 3, 'o');
    // The griffin master's hut at the foot, its door lit.
    let hut = house(18, 14, Wall::Stone, Roof::Thatch, false);
    im.stamp(&hut, 6, H - hut.h);
    im
}
