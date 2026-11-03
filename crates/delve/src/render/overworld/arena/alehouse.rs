//! The Sozzled Griffin drawn: a great plank-floored hall in the dark, warm
//! only where the fire and the lanterns are. A hearth with a suspicious
//! stew, a long bar with its shelves of bottles, five tables with benches
//! and the company sitting at them, a stage, kegs, a dartboard, a notice
//! board, an arm-wrestling table, two patrons asleep and a cat that
//! saunters round. Metal Slug dark: black paper, timber and stone, and
//! amber light.

use super::super::ink::{BLACK, Img, Rgb, hash, text_width};
use super::super::kit;
use super::super::light::Light;
use super::{TILE, at, dusk, fire, knights, stand};
use crate::rules::alehouse::{
    self as a, BACK_WALL, BAR, CRATES, FENCE_AT, FRONT_DOOR, HEARTH, KEEPER_AT, KEGS, LUTE_AT, PATRONS,
    Spot, STAGE, TABLES, Toy, WRESTLE_TABLE, WRESTLERS_AT,
};
use crate::rules::bazaar;
use crate::rules::{RoomKind, Run, Tile};

const T: i32 = TILE;

/// The hall at rest: floor, walls, fixtures and the light of its fires.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let (pw, ph) = (room.cols as i32 * T, room.rows as i32 * T);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    planks(&mut cv, run);
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            // Beyond the annex's south wall is black exterior, not masonry
            // filling the rest of the room's rectangular camera bounds.
            let inside = col <= a::BATHROOM.0 || row < a::BATHROOM.1 + a::BATHROOM.3;
            if room.tile(col, row) == Tile::Wall && inside {
                stone_wall(&mut cv, col, row);
            }
        }
    }
    back_wall(&mut cv);
    hearth(&mut cv, &mut lights);
    shelves(&mut cv);
    sign(&mut cv, &mut lights);
    bar(&mut cv);
    kegs(&mut cv);
    stage(&mut cv, &mut lights);
    for (i, &table) in TABLES.iter().enumerate() {
        self::table(&mut cv, table, i as i32, &mut lights);
    }
    arm_table(&mut cv);
    barrels(&mut cv);
    chandeliers(&mut cv, &mut lights);
    crates(&mut cv, &mut lights);
    dartboard(&mut cv);
    notice(&mut cv);
    rugs(&mut cv);
    front_door(&mut cv, &mut lights);
    bazaar_door(&mut cv, &mut lights);
    trimmings(&mut cv);
    // Authored fixture pixels need the room's lighting, not procedural
    // material relief: relief would turn the thin pegs/rails into floor grain.
    let mut fixtures = Img::new(pw, ph);
    home_lamp(&mut fixtures, &mut lights);
    arms_rack(&mut fixtures);
    if room.cols >= a::HALL.0 {
        bathroom(&mut cv, &mut fixtures, &mut lights);
    }
    // Lanterns on the walls, and the light they throw.
    for (col, row) in [(1, 3), (1, 13), (34, 4), (34, 12), (9, 18), (26, 18), (11, 7), (28, 7)] {
        let lamp = kit::lantern(true);
        let (x, y) = (col * T + (T - lamp.w) / 2, row * T - 6);
        cv.stamp(&lamp, x, y);
        lights.push(fire(x + 3, y + 3, 76.0, 0.5));
    }
    dusk(&mut cv, &lights, ('j', 'o'));
    // The metal is already shaded warm gold; neutral illumination preserves
    // its three authored steps instead of warming every midtone to cream.
    let fixture_lights: Vec<_> = lights.iter().map(|l| Light { fire: false, ..*l }).collect();
    let solid = vec![true; (pw * ph) as usize];
    let mask: Vec<_> = (0..pw * ph).map(|i| fixtures.get(i % pw, i / pw).is_some()).collect();
    super::light_rows(&mut fixtures, &fixture_lights, ('j', 'o'), 1.2, &solid, 0..ph);
    for (i, keep) in mask.into_iter().enumerate() {
        if !keep { fixtures.clear(i as i32 % pw, i as i32 / pw); }
    }
    cv.stamp(&fixtures, 0, 0);
    cv
}

/// Six pegs on the south wall, not a freestanding obstacle through the
/// table aisle. Black paper, stone greys and brass, with no luminous rim.
fn arms_rack(cv: &mut Img) {
    let left = (a::arm_stand(0).0 * T as f32) as i32 - 11;
    let right = (a::arm_stand(a::home_arms().len() - 1).0 * T as f32) as i32 + 11;
    // The backplate meets the south stonework (row 19), below the rug.
    cv.rect(left, 282, right - left + 1, 36, 'k');
    cv.frame(left, 282, right - left + 1, 36, 'g');
    let mid = (left + right) / 2;
    cv.rect(mid - 16, 272, 32, 10, 'k');
    cv.text(mid - text_width("ARMS") / 2, 274, "ARMS", 't');
    for (i, card) in a::home_arms().iter().enumerate() {
        let x = (a::arm_stand(i).0 * T as f32) as i32;
        cv.line(x - 8, 290, x + 8, 290, 'g');
        cv.put(x, 286, 't');
        let art = super::cards::card_art(card);
        let scale = (14.0 / art.w as f32).min(22.0 / art.h as f32).min(1.0);
        let icon = super::resized(&art, scale);
        for yy in 0..icon.h {
            for xx in 0..icon.w {
                if let Some(c) = icon.get(xx, yy).filter(|c| *c != BLACK) {
                    cv.set(x - icon.w / 2 + xx, 292 + yy, paper_ink(c));
                }
            }
        }
    }
}

/// Preserve warm amber; everything else is a quiet tonal grey.
fn paper_ink([r, g, b]: Rgb) -> Rgb {
    let v = ((r as u16 * 3 + g as u16 * 6 + b as u16) / 10) as u8;
    // Tiny wooden shafts need daylight against the black mounting board.
    if v < 96 { return [112, 112, 112]; }
    if r > g.saturating_add(18) && g > b.saturating_add(12) { return [r, g, b]; }
    [v, v, v]
}

/// A low gold Aladdin lamp: open loop handle, lid, belly and long upturned
/// spout. Authored in the same ink-row pipeline, rather than the wish UI icon.
fn lamp_art() -> Img {
    Img::from_rows(&[
        "..........................................",
        "...................O......................",
        "..................OTO.....................",
        "...............BOOOTOOOB..................",
        "..............BttTTTTTttB.................",
        "...BOOOOB.......BOOOOOB..............OTB..",
        "..BOt..tOB....BOOOOOOOOOOB...........OTTB.",
        ".BOt....tOB.BOttTTTTTTTttOB.......BOttB...",
        ".BO......tOBOttTTTTTTTTtttOB....BOttOB....",
        ".BO......tOBottTTTTTTTtttttOBOOOttOB......",
        "..BO....tOBBOttttttttttttttOttttOB........",
        "...BOOOOOB..BOOOOOOOOOOOOOOOOOB...........",
        ".............BROOOOOOOOOOOORB.............",
        "...............BRRRRRRRRRB................",
        "..................BOOOB...................",
        "...............BOOOttOOOB.................",
        "...............BBBBBBBBB..................",
        "..........................................",
    ])
}

/// The lamp's table stays clear of the champions along the bar at y=7.2.
fn home_lamp(cv: &mut Img, _lights: &mut Vec<Light>) {
    let (x, y) = ((a::LAMP_AT.0 * T as f32) as i32, (a::LAMP_AT.1 * T as f32) as i32);
    cv.ellipse(x as f32, (y + 6) as f32, 15.0, 3.5, 'X');
    // Quiet dark inset behind the open handle and spout, isolated from the
    // shelf grain; a broad tabletop supports the low, long vessel.
    cv.rect(x - 24, y - 26, 48, 22, 's');
    cv.frame(x - 24, y - 26, 48, 22, 'I');
    cv.rect(x - 14, y - 3, 28, 12, 'P');
    cv.rect(x - 23, y - 5, 46, 5, 'p');
    cv.line(x - 9, y, x - 9, y + 8, 'o');
    cv.line(x + 8, y, x + 8, y + 8, 'o');
    let lamp = lamp_art();
    cv.stamp(&lamp, x - lamp.w / 2, y - 5 - lamp.h);
    // The genie stays visible on every company page, without a bloom or
    // an availability-dependent cyan halo.
    cv.line(x, y - 24, x - 3, y - 29, 'o');
    cv.line(x - 3, y - 29, x + 2, y - 33, 't');
    cv.line(x + 2, y - 33, x - 1, y - 36, 'o');
}

/// A little washroom through an open timber jamb. Only its masonry and
/// floor take material relief; the mirror, crockery and linen keep their
/// authored pixel clusters on the same fixture layer as the lamp and rack.
fn bathroom(cv: &mut Img, fixtures: &mut Img, lights: &mut Vec<Light>) {
    let (c, r, w, h) = a::BATHROOM;
    let (left, top, right, bottom) = ((c + 1) * T, (r + 1) * T, (c + w - 1) * T, (r + h - 1) * T);
    // Warm, worn flagstones: broad quiet faces, a bevel and dark grout.
    for y in top..bottom {
        for x in left..right {
            let (tx, ty) = ((x - left) / 12, (y - top) / 12);
            let (dx, dy) = ((x - left) % 12, (y - top) % 12);
            let ch = if dx == 0 || dy == 0 {
                'g'
            } else if dy == 1 {
                'h'
            } else if dx == 11 || dy == 11 {
                'G'
            } else if (tx + ty) % 2 == 0 {
                'H'
            } else {
                'h'
            };
            cv.put(x, y, ch);
        }
    }
    // Timber lining behind the washstand, with a narrow oak shelf rail.
    cv.rect(left, top, right - left, T, 'I');
    for x in (left..right).step_by(8) {
        cv.line(x, top, x, top + T - 1, 'b');
        cv.line(x + 1, top, x + 1, top + T - 1, 'P');
    }
    cv.rect(left, top + T - 2, right - left, 3, 'B');
    cv.line(left, top + T - 2, right - 1, top + T - 2, 'o');

    // A clear two-tile passage, not a painted shut door or a travel plate.
    let (dc, dr, dw, dh) = a::BATHROOM_DOOR;
    let (dx, dy, door_w, door_h) = (dc * T, dr * T, dw * T, dh * T);
    cv.rect(dx, dy, door_w, door_h, 'G');
    cv.line(dx + door_w - 2, dy, dx + door_w - 2, dy + door_h - 1, 'h');
    for y in (dy + 3..dy + door_h).step_by(6) {
        cv.line(dx, y, dx + door_w - 1, y, 'B');
    }
    for y in [dy - 4, dy + door_h] {
        cv.rect(dx - 2, y, door_w + 4, 4, 'b');
        cv.line(dx - 2, y, dx + door_w + 1, y, 'o');
    }
    // The door leaf is folded back against the hall side of the wall.
    cv.rect(dx - 5, dy - 22, 5, 20, 'I');
    cv.line(dx - 5, dy - 22, dx - 5, dy - 3, 'o');
    cv.rect(dx - 3, dy - 17, 3, 2, 'g');
    cv.rect(dx - 3, dy - 7, 3, 2, 'g');

    // Washstand: two feet, a dark apron, an oak top. All fit its solid tile.
    let (bc, br, bw, bh) = a::BATHROOM_BASIN;
    let (bx, by, ww, hh) = (bc * T, br * T, bw * T, bh * T);
    fixtures.rect(bx + 2, by + 4, ww - 4, hh - 4, 'b');
    fixtures.rect(bx + 3, by + 7, 3, hh - 7, 'P');
    fixtures.rect(bx + ww - 6, by + 7, 3, hh - 7, 'P');
    fixtures.rect(bx, by + 2, ww, 6, 'B');
    fixtures.line(bx, by + 2, bx + ww - 1, by + 2, 'o');
    fixtures.line(bx + 1, by + 3, bx + ww - 2, by + 3, 'r');
    fixtures.line(bx + 4, by + 9, bx + ww - 5, by + 9, 'r');
    // Pale ceramic basin: a shaded bowl and a quiet blue-grey water ellipse.
    fixtures.ellipse((bx + 21) as f32, (by + 5) as f32, 9.0, 4.0, 'U');
    fixtures.ellipse((bx + 21) as f32, (by + 3) as f32, 10.0, 3.0, 'V');
    fixtures.ellipse((bx + 21) as f32, (by + 3) as f32, 7.0, 2.0, 'S');
    fixtures.line(bx + 17, by + 2, bx + 24, by + 2, 'U');
    fixtures.line(bx + 16, by + 5, bx + 25, by + 5, 'W');
    // A pale glazed jug: the spout and open handle read against the oak.
    fixtures.stamp(
        &Img::from_rows(&[
            "..SVVVV.......",
            ".SVWWV........",
            "..SVWV........",
            "..SVWV........",
            ".SVWWWVSS.....",
            "SVWWWWVS.S....",
            "SVWWWWVS.S....",
            "SVWWWUVSS.....",
            ".SVWUUV.......",
            ".SUUUUV.......",
            "..SSSS........",
        ]),
        bx,
        by - 7,
    );

    // The wall mirror: a thick oak frame, bevelled metal, cold glass and
    // two restrained diagonal catches. No portrait or knight is baked in.
    let (mx, my) = (
        (a::MIRROR_AT.0 * T as f32) as i32,
        (a::MIRROR_AT.1 * T as f32) as i32,
    );
    fixtures.rect(mx - 13, my - 14, 27, 28, 'n');
    fixtures.rect(mx - 12, my - 13, 25, 26, 'B');
    fixtures.line(mx - 11, my - 12, mx + 11, my - 12, 'o');
    fixtures.line(mx - 11, my - 12, mx - 11, my + 10, 'r');
    fixtures.rect(mx - 9, my - 10, 19, 20, 'U');
    fixtures.rect(mx - 8, my - 9, 17, 18, 'S');
    fixtures.rect(mx - 7, my - 8, 15, 16, 'u');
    fixtures.line(mx - 5, my + 1, mx + 2, my - 6, 'V');
    fixtures.line(mx - 4, my + 3, mx + 4, my - 5, 'U');
    fixtures.line(mx + 1, my + 6, mx + 5, my + 2, 'V');
    fixtures.line(mx - 8, my + 9, mx + 9, my + 9, 'V');
    fixtures.rect(mx - 3, my - 16, 7, 2, 'P');

    // Linen draped over a short wooden rail to the right of the mirror.
    let (tx, ty) = (right - 24, top + 3);
    fixtures.rect(tx - 3, ty + 2, 21, 3, 'B');
    fixtures.line(tx - 3, ty + 2, tx + 17, ty + 2, 'o');
    fixtures.rect(tx, ty + 1, 12, 20, 'V');
    fixtures.rect(tx + 1, ty + 1, 3, 19, 'W');
    fixtures.rect(tx + 8, ty + 3, 3, 17, 'U');
    fixtures.line(tx + 1, ty + 16, tx + 10, ty + 16, 'r');
    fixtures.line(tx + 2, ty + 18, tx + 10, ty + 18, 'P');
    for x in (tx + 1..tx + 12).step_by(3) {
        fixtures.put(x, ty + 21, 'V');
    }
    // A stave bucket, its floor shadow, iron hoops and arched handle.
    let (kc, kr, _, _) = a::BATHROOM_BUCKET;
    let (kx, ky) = (kc * T, kr * T);
    fixtures.ellipse((kx + 8) as f32, (ky + 13) as f32, 8.0, 2.0, 'n');
    fixtures.stamp(
        &Img::from_rows(&[
            ".....jjjjj......",
            "....j.....j.....",
            "...j.......j....",
            "..j.........j...",
            "..jBBBBBBBBBj...",
            "..jroPPPrPPBj...",
            "..jBBBBBBBBBj...",
            "...roPPPrPPB....",
            "...rBPPBrBPB....",
            "...rBPPBrBPB....",
            "...jjjjjjjjj....",
            "....rPPPrPB.....",
            "....BBBBBBB.....",
        ]),
        kx,
        ky,
    );
    let lamp = kit::lantern(true);
    let (lx, ly) = (left + 6, top + 3);
    fixtures.stamp(&lamp, lx, ly);
    lights.push(fire(lx + 3, ly + 3, 110.0, 0.65));
}

/// The tavern's outer wall: dark mortared stone, courses staggered, here
/// and there a timber post.
fn stone_wall(cv: &mut Img, col: i32, row: i32) {
    let (x0, y0) = (col * T, row * T);
    for y in 0..T {
        let course = (row * T + y) / 8;
        let off = if course % 2 == 0 { 0 } else { 7 };
        for x in 0..T {
            let gx = col * T + x + off;
            let mortar = (row * T + y) % 8 == 0 || gx % 14 == 0;
            let ink = if mortar {
                'X'
            } else {
                match hash(gx / 14, course, 41) % 5 {
                    0 => 'g',
                    1 | 2 => 'j',
                    3 => 'G',
                    _ => 'j',
                }
            };
            cv.put(x0 + x, y0 + y, ink);
        }
    }
    if (col + row) % 6 == 0 {
        cv.rect(x0 + 6, y0, 4, T, 'B');
        cv.line(x0 + 6, y0, x0 + 6, y0 + T - 1, 'r');
    }
}

/// Oak boards laid east to west, long ones, each its own tone, nailed at
/// the ends, the joints jittered row to row.
fn planks(cv: &mut Img, run: &Run) {
    let room = run.room();
    const LEN: i32 = 76;
    for y in 0..cv.h {
        let row = y / 6;
        let dy = y % 6;
        let shift = (hash(row, 3, 71) % 48) as i32;
        let start = |k: i32| k * LEN + (hash(k, row, 5) % 44) as i32 - 22;
        for x in 0..cv.w {
            if room.tile(x / T, y / T) == Tile::Wall {
                continue;
            }
            let xs = x + shift;
            let mut board = xs.div_euclid(LEN);
            if xs < start(board) {
                board -= 1;
            } else if xs >= start(board + 1) {
                board += 1;
            }
            let along = xs - start(board);
            let (lit, face, deep) = match hash(board, row, 19) % 6 {
                0 => ('o', 'r', 'P'),
                1 | 2 => ('r', 'P', 'B'),
                3 => ('P', 'B', 'I'),
                _ => ('r', 'B', 'B'),
            };
            let grain = dy == 3 && hash((xs) / 9, row, 13).is_multiple_of(4);
            let nail = (along == 3 || along == LEN - 5) && dy == 2;
            let ink = if dy == 0 || along == 0 {
                'b'
            } else if nail {
                'g'
            } else if dy == 1 {
                lit
            } else if dy == 5 || grain {
                deep
            } else {
                face
            };
            cv.put(x, y, ink);
        }
    }
}

/// The back wall: upright boards under a heavy beam, a skirting below.
fn back_wall(cv: &mut Img) {
    let (c, r, w, h) = BACK_WALL;
    let (x0, y0, x1, y1) = (c * T, r * T, (c + w) * T, (r + h) * T);
    for x in x0..x1 {
        let plank = (x - x0) / 8;
        let (lit, face) = match hash(plank, 5, 31) % 4 {
            0 => ('B', 'I'),
            1 => ('I', 'b'),
            _ => ('I', 'I'),
        };
        for y in y0..y1 {
            let ink = if (x - x0) % 8 == 0 {
                'n'
            } else if (x - x0) % 8 == 1 {
                lit
            } else if y > y1 - 4 {
                'b'
            } else {
                face
            };
            cv.put(x, y, ink);
        }
    }
    cv.rect(x0, y0, x1 - x0, 3, 'P');
    cv.line(x0, y0, x1 - 1, y0, 'o');
    cv.line(x0, y0 + 3, x1 - 1, y0 + 3, 'n');
    cv.line(x0, y1 - 1, x1 - 1, y1 - 1, 'n');
    for px in (x0 + 24..x1).step_by(96) {
        cv.rect(px, y0, 4, y1 - y0, 'P');
        cv.line(px, y0, px, y1 - 1, 'r');
        cv.line(px + 3, y0, px + 3, y1 - 1, 'n');
    }
}

/// The hearth: a stone chimney-breast, a mantel with its clutter, the
/// black firebox with logs laid in it. The flames are the figures'.
fn hearth(cv: &mut Img, lights: &mut Vec<Light>) {
    let (c, r, w, h) = HEARTH;
    let (x, y, hw, hh) = (c * T, r * T, w * T, h * T);
    for yy in 0..hh {
        let course = yy / 8;
        for xx in 0..hw {
            let off = if course % 2 == 0 { 0 } else { 6 };
            let block = (xx + off) / 12;
            let mortar = yy % 8 == 0 || (xx + off) % 12 == 0;
            let ink = if mortar {
                'g'
            } else {
                match hash(block, course, 7) % 4 {
                    0 => 'j',
                    1 => 'G',
                    2 => 'J',
                    _ => 'G',
                }
            };
            cv.put(x + xx, y + yy, ink);
        }
    }
    cv.line(x, y, x + hw - 1, y, 'h');
    // The firebox.
    let (fx, fy, fw, fh) = (x + 14, y + 12, hw - 28, hh - 12);
    cv.rect(fx, fy, fw, fh, 'k');
    cv.rect(fx + 2, fy + 2, fw - 4, fh - 2, 'n');
    cv.rect(fx + 6, fy + 8, fw - 12, fh - 8, 'k');
    cv.frame(fx - 1, fy - 1, fw + 2, fh + 1, 'X');
    // Logs.
    cv.rect(fx + 10, fy + fh - 5, fw - 20, 4, 'B');
    cv.line(fx + 10, fy + fh - 5, fx + fw - 11, fy + fh - 5, 'r');
    cv.line(fx + 10, fy + fh - 2, fx + fw - 11, fy + fh - 2, 'n');
    cv.rect(fx + 18, fy + fh - 9, fw - 36, 4, 'p');
    cv.line(fx + 18, fy + fh - 9, fx + fw - 19, fy + fh - 9, 'R');
    // The mantel, and the clutter on it: a skull, two mugs, a candle.
    cv.rect(x - 3, y + 6, hw + 6, 5, 'P');
    cv.line(x - 3, y + 6, x + hw + 2, y + 6, 'O');
    cv.line(x - 3, y + 10, x + hw + 2, y + 10, 'n');
    cv.rect(x + 8, y + 1, 5, 5, 'H');
    cv.put(x + 9, y + 3, 'k');
    cv.put(x + 11, y + 3, 'k');
    cv.put(x + 10, y + 5, 'g');
    for mx in [x + 24, x + 31] {
        cv.rect(mx, y + 2, 4, 4, 'o');
        cv.put(mx + 4, y + 3, 'o');
    }
    cv.rect(x + hw - 14, y + 1, 2, 5, 'T');
    cv.put(x + hw - 14, y, 't');
    lights.push(fire(x + hw - 13, y, 22.0, 0.2));
    // The great fire's glow on the room.
    lights.push(fire(x + hw / 2, y + hh - 6, 118.0, 0.72));
    lights.push(fire(x + hw / 2, y + hh + 14, 70.0, 0.4));
}

/// Two boards of bottles behind the bar, in every colour.
fn shelves(cv: &mut Img) {
    let (c, _, w, _) = BAR;
    let (x0, x1) = (c * T + 4, (c + w) * T - 4);
    for (k, y) in [27, 41].into_iter().enumerate() {
        cv.line(x0, y, x1, y, 'P');
        cv.line(x0, y + 1, x1, y + 1, 'b');
        let mut x = x0 + 2;
        while x < x1 - 3 {
            let pick = hash(x, k as i32, 57) % 7;
            let glass = ['C', 'z', 'R', 'Y', 'q', 'e', 'o'][pick as usize];
            let tall = 5 + (hash(x, k as i32, 9) % 4) as i32;
            cv.rect(x, y - tall, 2, tall, glass);
            cv.put(x, y - tall - 1, 'g');
            cv.put(x, y - tall + 1, 'H');
            x += 4 + (hash(x, k as i32, 3) % 3) as i32;
        }
    }
}

/// The sign over the passage between hearth and bar: a griffin.
fn sign(cv: &mut Img, lights: &mut Vec<Light>) {
    let (x, y, w, h) = (10 * T + 2, T + 4, 3 * T - 4, 28);
    cv.line(x + 4, y - 4, x + 4, y, 'g');
    cv.line(x + w - 5, y - 4, x + w - 5, y, 'g');
    cv.rect(x, y, w, h, 'P');
    cv.frame(x, y, w, h, 'B');
    cv.frame(x + 1, y + 1, w - 2, h - 2, 'r');
    // The griffin: an eagle's head and a hooked beak, a wing swept back.
    let griffin = Img::from_rows(&[
        "...tt........",
        "..tOOt..t....",
        ".tOOOOtttT...",
        ".tOkOOOOtT...",
        ".ttOOOOOT....",
        "..ttOOOTT....",
        "...ooOOOOt...",
        "...oOOtOOOt..",
        "....oo.tt.ot.",
    ]);
    cv.stamp(&griffin, x + (w - griffin.w) / 2, y + 3);
    let name = "GRIFFIN";
    cv.text(x + (w - text_width(name)) / 2 + 1, y + h - 9, name, 'T');
    lights.push(fire(x + w / 2, y + h + 6, 30.0, 0.16));
}

/// The bar: a long counter of oak, panelled in front, taps and mugs on it.
fn bar(cv: &mut Img) {
    let (c, r, w, h) = BAR;
    let (x, y, bw, bh) = (c * T, r * T, w * T, h * T);
    cv.rect(x, y + 3, bw, bh - 3, 'P');
    cv.rect(x, y, bw, 5, 'r');
    cv.line(x, y, x + bw - 1, y, 'T');
    cv.line(x, y + 1, x + bw - 1, y + 1, 'O');
    cv.line(x, y + 4, x + bw - 1, y + 4, 'n');
    for px in (x + 4..x + bw - 2).step_by(16) {
        cv.rect(px, y + 7, 12, bh - 10, 'p');
        cv.frame(px, y + 7, 12, bh - 10, 'r');
        cv.put(px + 1, y + 8, 'R');
    }
    cv.line(x, y + bh - 1, x + bw - 1, y + bh - 1, 'n');
    cv.rect(x, y + bh - 3, bw, 2, 'b');
    // Brass taps along the counter's top, a row of mugs.
    for k in 0..3 {
        let tx = x + bw / 2 - 40 + k * 40;
        cv.rect(tx, y - 4, 2, 5, 'o');
        cv.put(tx - 1, y - 4, 't');
        cv.put(tx + 2, y - 4, 't');
        cv.put(tx, y - 5, 'T');
    }
    for k in 0..6 {
        let mx = x + 12 + k * 42;
        cv.rect(mx, y - 2, 4, 5, 'V');
        cv.line(mx, y - 3, mx + 3, y - 3, 'T');
        cv.put(mx + 4, y, 'V');
    }
}

/// Kegs stacked in the north-east corner, one tapped.
fn kegs(cv: &mut Img) {
    let (c, r, w, h) = KEGS;
    let (x, y, kw, kh) = (c * T, r * T, w * T, h * T);
    cv.rect(x, y, kw, kh, 'b');
    let barrels = kit::barrels();
    for (bx, by) in [(x + 1, y + kh - 15), (x + 18, y + kh - 15), (x + 36, y + kh - 15), (x + 8, y + kh - 28), (x + 28, y + kh - 28), (x + 16, y + kh - 41)] {
        cv.stamp(&barrels, bx.min(x + kw - 17), by.max(y + 1));
    }
    // The tapped keg's tap, and the puddle under it.
    cv.rect(x + 20, y + kh - 6, 4, 2, 'o');
    cv.rect(x + 22, y + kh - 4, 2, 4, 'o');
    cv.ellipse((x + 23) as f32, (y + kh + 3) as f32, 4.0, 1.5, 'X');
}

/// Sir... the stage: a dais of boards, footlights along its lip, a stool.
fn stage(cv: &mut Img, lights: &mut Vec<Light>) {
    let (c, r, w, h) = STAGE;
    let (x, y, sw, sh) = (c * T, r * T, w * T, h * T);
    cv.rect(x, y + 2, sw, sh - 2, 'O');
    for px in (x..x + sw).step_by(6) {
        cv.line(px, y + 2, px, y + sh - 6, 'o');
    }
    cv.rect(x, y + sh - 6, sw, 6, 'r');
    cv.line(x, y + sh - 6, x + sw - 1, y + sh - 6, 't');
    cv.line(x, y + sh - 1, x + sw - 1, y + sh - 1, 'n');
    cv.line(x, y + 2, x + sw - 1, y + 2, 't');
    // Curtains at the east wall, a garland across the front.
    cv.rect(x + sw - 6, y + 2, 6, sh - 8, 'p');
    for py in (y + 2..y + sh - 8).step_by(4) {
        cv.line(x + sw - 6, py, x + sw - 1, py, '7');
    }
    for k in 0..6 {
        let fx = x + 6 + k * (sw - 14) / 5;
        cv.put(fx, y + sh - 7, '6');
        cv.put(fx, y + sh - 8, '5');
        lights.push(fire(fx, y + sh - 8, 20.0, 0.2));
    }
    // A stool and a hat for coins.
    cv.rect(x + 14, y + sh - 15, 6, 5, 'r');
    cv.frame(x + 14, y + sh - 15, 6, 5, 'b');
    cv.rect(x + 56, y + sh - 12, 7, 3, 'I');
    cv.line(x + 55, y + sh - 10, x + 63, y + sh - 10, 'n');
    cv.put(x + 58, y + sh - 13, '5');
    cv.put(x + 60, y + sh - 13, '6');
}

/// A table's slab: oak with its candle and mugs. `slab_img` for the
/// scenery, and its top rows again over the company seated behind it.
fn slab_img(tw: i32, th: i32, n: i32) -> Img {
    let mut cv = Img::new(tw, th);
    cv.rect(0, 0, tw, th, 'P');
    cv.frame(0, 0, tw, th, 'r');
    cv.line(1, 1, tw - 2, 1, 'O');
    cv.line(1, th - 2, tw - 2, th - 2, 'B');
    for k in 0..4 {
        let mx = 6 + k * (tw - 16) / 3;
        let my = 5 + (hash(k, n, 41) % 6) as i32;
        cv.rect(mx, my, 4, 4, 'o');
        cv.line(mx, my - 1, mx + 3, my - 1, 'T');
        cv.put(mx + 4, my + 1, 'o');
    }
    let cx = tw / 2;
    cv.rect(cx - 2, th / 2 + 1, 5, 2, 'g');
    cv.rect(cx, th / 2 - 3, 2, 4, 'T');
    cv.put(cx, th / 2 - 4, '6');
    cv
}

/// Where a table's slab lies (pixels): its top-left and size.
pub(super) fn slab_rect((c, r, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    (c * T + 2, r * T + 4, w * T - 4, h * T - 8)
}

/// A table: a slab of oak with its candle and mugs, a bench each side.
pub(super) fn table(cv: &mut Img, rect: (i32, i32, i32, i32), n: i32, lights: &mut Vec<Light>) {
    let (x, y, tw, th) = slab_rect(rect);
    // The benches first, so the slab sits over their edges.
    for by in [y - 8, y + th + 1] {
        cv.rect(x + 3, by, tw - 6, 6, 'r');
        cv.line(x + 3, by, x + tw - 4, by, 'O');
        cv.line(x + 3, by + 5, x + tw - 4, by + 5, 'n');
    }
    cv.stamp(&slab_img(tw, th, n), x, y);
    lights.push(fire(x + tw / 2, y + th / 2 - 4, 40.0, 0.34));
}

/// The arm-wrestling table in the south-east, two mugs sloshed on it.
fn arm_table(cv: &mut Img) {
    let (c, r, w, h) = WRESTLE_TABLE;
    let (x, y, tw, th) = (c * T, r * T + 4, w * T, h * T - 6);
    cv.rect(x, y, tw, th, 'P');
    cv.frame(x, y, tw, th, 'r');
    cv.line(x + 1, y + 1, x + tw - 2, y + 1, 'O');
    cv.rect(x + 4, y + 5, 4, 4, 'o');
    cv.rect(x + tw - 8, y + 5, 4, 4, 'o');
    cv.ellipse((x + tw / 2) as f32, (y + 12) as f32, 5.0, 1.5, 'B');
    // A sign: the stakes.
    cv.text(x + 1, y + th + 6, "10 GOLD", 'o');
}

/// Barrels stacked along the south wall.
fn barrels(cv: &mut Img) {
    let stack = kit::barrels();
    for &(c, r, w, h) in &a::BARRELS {
        let (x, y) = (c * T, r * T);
        cv.rect(x, y + 2, w * T, h * T - 2, 'b');
        cv.stamp(&stack, x, y + h * T - 15);
        cv.stamp(&stack, x + 15, y + h * T - 15);
        cv.stamp(&stack, x + 8, y + 2);
    }
}

/// Iron rings hung over the aisles, each with its candles, throwing a
/// warm pool of light on the planks below.
fn chandeliers(cv: &mut Img, lights: &mut Vec<Light>) {
    for (n, (tx, ty)) in [(9.0f32, 12.6f32), (19.0, 12.2), (27.5, 11.8), (6.0, 7.5)].into_iter().enumerate() {
        let (cx, cy) = ((tx * T as f32) as i32, (ty * T as f32) as i32);
        // The iron ring, seen from above at a slant, and a candle at each point.
        for step in 0..48 {
            let a = step as f32 / 48.0 * std::f32::consts::TAU;
            cv.put(cx + (a.cos() * 10.0) as i32, cy + (a.sin() * 4.0) as i32, if step % 2 == 0 { 'j' } else { 'g' });
        }
        for k in 0..6 {
            let a = k as f32 / 6.0 * std::f32::consts::TAU + n as f32;
            let (px, py) = (cx + (a.cos() * 10.0) as i32, cy + (a.sin() * 4.0) as i32);
            cv.put(px, py, 'T');
            cv.put(px, py - 1, '6');
            cv.put(px, py - 2, 't');
        }
        lights.push(fire(cx, cy - 2, 64.0, 0.42));
    }
}

/// The fence's crates, and what is on them: lamp, lock, a sign.
fn crates(cv: &mut Img, lights: &mut Vec<Light>) {
    let (c, r, w, h) = CRATES;
    let (x, y, cw, ch) = (c * T, r * T, w * T, h * T);
    cv.rect(x, y + 2, cw, ch - 2, 'o');
    cv.frame(x, y + 2, cw, ch - 2, 'n');
    for px in [x + cw / 3, x + 2 * cw / 3] {
        cv.line(px, y + 2, px, y + ch - 1, 'n');
    }
    cv.line(x, y + ch / 2 + 2, x + cw - 1, y + ch / 2 + 2, 'n');
    cv.line(x + 2, y + 3, x + cw - 3, y + ch - 3, 'r');
    cv.rect(x + 3, y - 4, 10, 6, 'O');
    cv.frame(x + 3, y - 4, 10, 6, 'n');
    let lamp = Img::from_rows(&[".g.", "gTg", "gtg", ".g."]);
    cv.stamp(&lamp, x + cw - 6, y - 2);
    lights.push(fire(x + cw - 4, y, 40.0, 0.28));
    cv.text(x + 1, y - 14, "NO QUESTIONS", 'j');
}

/// The dartboard on the west wall, and the line to throw from.
fn dartboard(cv: &mut Img) {
    let (cx, cy) = (8.0, 9.6 * T as f32 + 2.0);
    for (r, ink) in [(8.0, 'n'), (7.0, 'H'), (5.6, 'k'), (4.4, 'R'), (3.0, 'H'), (1.8, 'F'), (0.9, 'R')] {
        cv.ellipse(cx, cy, r, r, ink);
    }
    for (dx, dy, ink) in [(-3, -1, 'g'), (2, 3, 'g'), (4, -4, 'j'), (-9, 2, 'j')] {
        cv.put(cx as i32 + dx, cy as i32 + dy, ink);
        cv.put(cx as i32 + dx + 1, cy as i32 + dy, 'o');
    }
    // The throw line.
    for y in (9 * T - 6..11 * T + 6).step_by(2) {
        cv.put(3 * T, y, 'G');
        cv.put(3 * T + 1, y, 'j');
    }
}

/// The notice board: planks on posts, papers pinned all over.
fn notice(cv: &mut Img) {
    let (c, r, w, h) = (1, 5, 2, 2);
    let (x, y, bw, _) = (c * T, r * T, w * T, h * T);
    cv.rect(x + 3, y + 22, 2, 10, 'b');
    cv.rect(x + bw - 5, y + 22, 2, 10, 'b');
    cv.rect(x + 1, y + 2, bw - 2, 20, 'P');
    cv.frame(x + 1, y + 2, bw - 2, 20, 'B');
    for (k, (nx, ny, nw, nh)) in [(4, 5, 8, 10), (15, 4, 7, 9), (24, 6, 6, 11)].into_iter().enumerate() {
        cv.rect(x + nx, y + ny, nw, nh, if k == 1 { 'i' } else { 'T' });
        cv.put(x + nx + nw / 2, y + ny, 'R');
        for line in 0..(nh - 3) / 2 {
            cv.line(x + nx + 1, y + ny + 3 + line * 2, x + nx + nw - 2, y + ny + 3 + line * 2, 'g');
        }
    }
}

/// The rugs: a long faded one before the front door, a hide before the fire.
fn rugs(cv: &mut Img) {
    let (x, y, w, h) = (14 * T, 15 * T + 4, 8 * T, 2 * T + 4);
    cv.rect(x, y, w, h, 'p');
    cv.frame(x, y, w, h, 'o');
    cv.frame(x + 3, y + 3, w - 6, h - 6, 'R');
    for k in 0..(w / 8) {
        let dx = x + 6 + k * 8;
        cv.rect(dx, y + h / 2 - 2, 4, 4, 'r');
        cv.put(dx + 1, y + h / 2 - 1, 'O');
    }
    for px in (x..x + w).step_by(3) {
        cv.put(px, y - 1, 'o');
        cv.put(px, y + h, 'o');
    }
    let (hx, hy) = (7 * T, 5 * T + 4);
    cv.ellipse(hx as f32, hy as f32, 34.0, 8.0, 'p');
    cv.ellipse(hx as f32, hy as f32, 31.0, 6.5, 'P');
    for k in 0..16 {
        let (dx, dy) = ((hash(k, 1, 91) % 52) as i32 - 26, (hash(k, 2, 91) % 9) as i32 - 4);
        cv.put(hx + dx, hy + dy, 'r');
    }
}

/// The front door in the south wall: its frame, the night beyond, doors
/// folded back, a lamp over the lintel.
fn front_door(cv: &mut Img, lights: &mut Vec<Light>) {
    let (c, r, w, _) = FRONT_DOOR;
    let (x, y) = (c * T - 4, (r + 1) * T - 2);
    let (dw, dh) = (w * T + 8, T + 2);
    cv.rect(x, y, dw, dh, 'k');
    // Night, a few stars, the road.
    cv.rect(x + 3, y + 2, dw - 6, dh - 2, 'x');
    for (sx, sy) in [(6, 5), (15, 3), (24, 7), (33, 4)] {
        cv.put(x + sx, y + sy, 'w');
    }
    cv.rect(x + 3, y + dh - 5, dw - 6, 3, 's');
    // The frame and the doors.
    cv.frame(x, y, dw, dh, 'P');
    cv.frame(x + 1, y + 1, dw - 2, dh - 1, 'B');
    cv.rect(x, y - 3, dw, 4, 'P');
    cv.line(x, y - 3, x + dw - 1, y - 3, 'O');
    cv.rect(x - 1, y, 3, dh, 'r');
    cv.rect(x + dw - 2, y, 3, dh, 'r');
    // The lamp over it.
    let lamp = kit::lantern(true);
    cv.stamp(&lamp, x + dw / 2 - 3, y - 14);
    lights.push(fire(x + dw / 2, y - 10, 54.0, 0.4));
    // A threshold of warm light spilled in.
    lights.push(fire(x + dw / 2, y - 6, 36.0, 0.2));
}

/// The east door to the bazaar: an arch hung with a bead curtain, and
/// lanterns either side.
fn bazaar_door(cv: &mut Img, lights: &mut Vec<Light>) {
    let (x, y, dw, dh) = (34 * T + 6, 14 * T - 2, 12, 3 * T + 4);
    cv.rect(x, y, dw, dh, 'k');
    cv.frame(x - 1, y - 1, dw + 2, dh + 1, 'P');
    for py in (y + 2..y + dh).step_by(3) {
        for px in (x + 2..x + dw - 1).step_by(3) {
            cv.put(px, py, if (px + py) % 2 == 0 { 't' } else { 'o' });
        }
    }
    for px in (x + 2..x + dw - 1).step_by(3) {
        cv.line(px, y + 2, px, y + dh - 3, 'r');
    }
    cv.rect(x - 2, y - 7, dw + 4, 6, 'P');
    cv.frame(x - 2, y - 7, dw + 4, 6, 'B');
    cv.text(x - 2, y - 15, "BAZAAR", 'O');
    lights.push(fire(x + dw / 2, y + dh / 2, 52.0, 0.4));
}

/// Little things: a spilled mug, a hat on a peg, a broom, a stack of logs.
fn trimmings(cv: &mut Img) {
    // Logs beside the hearth.
    for (k, (lx, ly)) in [(163, 58), (168, 58), (165, 53)].into_iter().enumerate() {
        cv.rect(lx, ly, 8, 4, if k == 2 { 'B' } else { 'r' });
        cv.line(lx, ly, lx + 7, ly, 'O');
        cv.line(lx, ly + 3, lx + 7, ly + 3, 'n');
    }
    // A hat on a peg, by the front door; a broom leaning in the corner.
    cv.rect(31 * T + 4, 17 * T + 6, 8, 2, 'I');
    cv.rect(31 * T + 6, 17 * T + 3, 4, 3, 'I');
    cv.line(30 * T, 17 * T - 4, 30 * T + 6, 18 * T + 2, 'o');
    cv.rect(30 * T + 4, 18 * T - 2, 6, 5, 't');
    // A spilled mug and its puddle, by the middle table.
    cv.ellipse(21.0 * T as f32 + 4.0, 12.0 * T as f32 + 9.0, 6.0, 2.0, 'X');
    cv.rect(21 * T + 12, 12 * T + 5, 4, 3, 'o');
    // Bones for the dog nobody has seen.
    cv.line(12 * T, 18 * T + 4, 12 * T + 5, 18 * T + 6, 'H');
}

// ---- the figures ---------------------------------------------------------

pub(super) const KEEPER: [&str; 18] = [
    "....OOOO....",
    "...OOOOOO...",
    "..OOKOOKOO..",
    "..OOOOOOOO..",
    "..OrrrrrrO..",
    "...OrrrrO...",
    "..BBBBBBBB..",
    ".BBHHHHHHBB.",
    ".BHHHHHHHHB.",
    "OOHHHHHHHHOO",
    "OOHHHHHHHHOO",
    ".OHHHHHHHHO.",
    "..HHHHHHHH..",
    "..HHHHHHHH..",
    "..BB....BB..",
    "..BB....BB..",
    "..nn....nn..",
    "..nn....nn..",
];

const LUTE: [&str; 18] = [
    "...5....5...",
    "..5w5..5w5..",
    "...7777777..",
    "...77OOO77..",
    "...OKOOKO...",
    "....OrrO....",
    "..77777777..",
    ".7777777777.",
    ".O777777777O",
    "OOoo777777..",
    "OOooo7777...",
    "..ooo777....",
    "...77777....",
    "...77.77....",
    "...77.77....",
    "...77.77....",
    "..nnn.nnn...",
    "............",
];

const FENCE: [&str; 18] = [
    "....XXXX....",
    "...XXXXXX...",
    "..XXXkkXXX..",
    "..XXkOOkXX..",
    "..XXkOOkXX..",
    "..XXXkkXXX..",
    "..XXXXXXXX..",
    ".XXXXXXXXXX.",
    ".XXXXXXXXXXO",
    ".XXXjXXXXXXO",
    ".XXXjXXXXXX.",
    ".XXXXXXXXXX.",
    "..XXXXXXXX..",
    "..XXXXXXXX..",
    "..XXXXXXXX..",
    "..XXXXXXXX..",
    "..nn....nn..",
    "..nn....nn..",
];

const SNORER: [&str; 10] = [
    "...OOO......",
    "..OOOOO.....",
    "..OkOOkO....",
    "..OOOOOO....",
    "BBBBBBBBB...",
    "BBBBBBBBBBB.",
    "BBBBBBBBBBBB",
    "IIBBBBBBBBBB",
    "II..II..IIII",
    "nn..nn..nnnn",
];

const WRESTLER: [&str; 16] = [
    "...OOOO.....",
    "..OOOOOO....",
    "..OKOOKO....",
    "..OOrrOO....",
    "...OOOO.....",
    "..RRRRRR....",
    ".RRRRRRRRO..",
    ".RRRRRRRRRO.",
    "ORRRRRRRRRRO",
    "OORRRRRR.OOO",
    "..RRRRRR....",
    "..RRRRRR....",
    "..II..II....",
    "..II..II....",
    "..II..II....",
    "..nn..nn....",
];

const CAT: [&str; 7] = [
    "....X.X.....",
    "...XXXXX..XX",
    "..XXkXkXXXXX",
    "XXXXXXXXXXX.",
    "XXXXXXXXXXX.",
    ".X.X..X.X...",
    ".X.X..X.X...",
];

const CAT_B: [&str; 7] = [
    "....X.X.....",
    "...XXXXX..XX",
    "..XXkXkXXXXX",
    "XXXXXXXXXXX.",
    "XXXXXXXXXXX.",
    "X.X.....X.X.",
    "X.X.....X.X.",
];

/// One flame: a wide base tapering to a tip, three tongues of colour,
/// swaying a little.
pub(super) fn flame(cv: &mut Img, x: i32, y: i32, tick: u32, k: i32, tall: i32) {
    let flick = (hash(k, (tick / 3) as i32, 17) % 5) as i32;
    let h = tall - 2 + flick;
    let sway = ((tick / 5 + k as u32 * 3) % 4) as i32 - 1;
    for yy in 0..h {
        let t = yy as f32 / h as f32;
        let half = ((1.0 - t) * 6.0 * (1.0 - t * 0.3)).round() as i32;
        let lean = (t * t * sway as f32 * 2.0) as i32;
        for xx in -half..=half {
            let inner = xx.abs() * 3 < half * 2;
            let ink = if t < 0.25 {
                if inner { 'O' } else { 'R' }
            } else if t < 0.6 {
                if xx.abs() * 2 < half { 't' } else if inner { 'O' } else { 'R' }
            } else if inner {
                'T'
            } else {
                't'
            };
            cv.put(x + xx + lean, y - yy, ink);
        }
    }
}

/// The company, the keeper, the fence, the toys' folk and the fire, each
/// where the hall puts them.
/// The inn's front at room scale (pixels), its door's foot on the courtyard
/// door's lower edge (`alehouse::COURTYARD_DOOR`).
const INN_W: i32 = 96;
const INN_H: i32 = 80;

/// The Sozzled Griffin from the Delve's courtyard, where the green house
/// stood by the well: a great two-storey inn of oak, its upper floor
/// jettied out over the lower on a heavy beam, a steep shingled gable with
/// its chimney smoking, every window lit, and over the door a giant
/// dragon's head, jaws wide, mounted on the gable like the house's trophy.
/// A tankard sign hangs from an iron arm at the west corner; a lantern
/// burns either side of the door.
pub(super) fn courtyard_inn(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let (dc, dr, dw, dh) = crate::rules::alehouse::COURTYARD_DOOR;
    let (x0, y0) = (dc * TILE + dw * TILE / 2 - INN_W / 2, (dr + dh) * TILE - INN_H);
    cv.stamp(&inn_front(tick), x0, y0);
    // The firelight in its windows and door.
    for (x, y) in [(19, 66), (77, 66), (12, 43), (84, 43), (48, 70)] {
        lights.push(fire(x0 + x, y0 + y, 22.0, 0.2));
    }
    // A lantern on an iron bracket either side of the door.
    for x in [x0 + 34, x0 + 61] {
        let y = y0 + 61;
        cv.line(x - 1, y - 2, x + 1, y - 2, 'K');
        cv.rect(x - 1, y - 1, 3, 4, 'X');
        cv.rect(x, y, 1, 2, if tick / 7 % 5 == 0 { 'o' } else { 't' });
        lights.push(fire(x, y + 1, 30.0, 0.3));
    }
    courtyard_sign(cv, (x0 + 5, y0 + 56), tick);
}

/// The inn's front, drawn whole: gable, upper floor, beam, ground floor,
/// footing, then the dragon's head over all.
fn inn_front(tick: u32) -> Img {
    let (w, h) = (INN_W, INN_H);
    let mut im = Img::new(w, h);
    // The gable: its wall of upright oak boards, then the roof's two slopes
    // over it, thick with shingles, and a bargeboard of light oak.
    for y in 0..36 {
        let half = 4 + y * 44 / 35;
        let (l, r) = (w / 2 - half, w / 2 + half);
        for x in l.max(0)..r.min(w) {
            let ch = if x % 5 == 0 { 'B' } else if (x / 5 + y / 9) % 2 == 0 { 'r' } else { 'R' };
            im.put(x, y, ch);
        }
        for k in 0..7 {
            let course = (y + k) / 3;
            let ink = if (y + k) % 3 == 2 { 'n' } else if course % 2 == 0 { 'I' } else { 'b' };
            im.put(l + k, y, ink);
            im.put(r - 1 - k, y, ink);
        }
        im.put(l + 7, y, 'o');
        im.put(r - 8, y, 'o');
    }
    // The chimney, stone, on the east slope; its smoke curling up.
    im.rect(71, 6, 8, 16, 'G');
    for y in (6..22).step_by(3) {
        im.line(71, y, 78, y, 'g');
    }
    im.rect(70, 5, 10, 2, 'j');
    for k in 0..3u32 {
        let rise = ((tick / 6 + k * 5) % 15) as i32;
        let (sx, sy) = (74 + (rise / 4) % 3, 3 - rise / 3);
        if sy >= 0 {
            im.put(sx, sy, if k == 0 { 'h' } else { 'J' });
            im.put(sx + 1, sy, 'J');
        }
    }
    // The eave line under the roof.
    im.line(0, 35, w - 1, 35, 'n');
    // The upper floor, jettied out: oak planks laid along, posts at the corners.
    for y in 36..54 {
        for x in 2..w - 2 {
            let ch = if (y - 36) % 4 == 3 { 'B' } else if ((y - 36) / 4 + x / 23) % 2 == 0 { 'R' } else { 'r' };
            im.put(x, y, ch);
        }
    }
    for x in [2, w - 6] {
        im.rect(x, 36, 4, 18, 'I');
        im.line(x, 36, x, 53, 'b');
    }
    for x in [6, 78] {
        window(&mut im, x, 38, 12, 11, tick);
    }
    // The beam the upper floor stands out on, its joists' ends showing.
    im.rect(1, 54, w - 2, 3, 'b');
    for x in (4..w - 4).step_by(8) {
        im.rect(x, 54, 3, 2, 'o');
    }
    // The ground floor, set back under the jetty.
    for y in 57..77 {
        for x in 5..w - 5 {
            let ch = if (y - 57) % 4 == 3 { 'B' } else if ((y - 57) / 4 + x / 19) % 2 == 0 { 'r' } else { 'P' };
            im.put(x, y, ch);
        }
    }
    for x in [5, w - 9] {
        im.rect(x, 57, 4, 20, 'I');
    }
    for x in [12, 70] {
        window(&mut im, x, 61, 14, 10, tick + 3);
    }
    // The door: a lintel, two oak leaves banded in iron, a ring.
    im.rect(38, 57, 20, 3, 'b');
    im.rect(40, 60, 16, 20, 'b');
    im.rect(41, 61, 14, 19, 'I');
    for x in [44, 47, 51] {
        im.line(x, 61, x, 79, 'b');
    }
    for y in [64, 72] {
        im.line(41, y, 54, y, 'X');
    }
    im.put(46, 68, 'O');
    im.put(49, 68, 'O');
    im.line(48, 61, 48, 79, 'n');
    // The footing of dressed stone.
    for x in 3..w - 3 {
        for y in 77..h {
            im.put(x, y, if (x + (y - 77) * 3) % 7 == 0 { 'g' } else if y == 77 { 'J' } else { 'G' });
        }
    }
    im.rect(40, 77, 16, 3, 'b');
    // The dragon's head over the door, its jaw above the lintel.
    im.stamp(&dragon_head(tick), 16, 16);
    im
}

/// A lit window: an oak frame, four panes of firelight, a cross of mullions;
/// one pane dims now and then as someone passes inside.
fn window(im: &mut Img, x: i32, y: i32, w: i32, h: i32, tick: u32) {
    im.rect(x, y, w, h, 'b');
    im.rect(x + 1, y + 1, w - 2, h - 2, 't');
    im.rect(x + 1, y + 1, w - 2, 2, 'T');
    let shadow = (tick / 40 % 4) as i32;
    let (px, py) = (x + 1 + (shadow % 2) * (w / 2), y + 1 + (shadow / 2) * (h / 2));
    im.rect(px, py, w / 2 - 1, h / 2 - 1, 'o');
    im.line(x + w / 2, y, x + w / 2, y + h - 1, 'b');
    im.line(x, y + h / 2, x + w - 1, y + h / 2, 'b');
    im.line(x - 1, y + h, x + w, y + h, 'B');
}

/// The giant dragon's head, mounted in profile and facing west over the
/// courtyard like a ship's figurehead: horns of old bone swept back, a crest
/// of spines, a heavy brow over an amber eye slit black, the long snout
/// smoking at the nostril, jaws wide on a dark throat with a forked tongue
/// and fangs above and below, tan plates under the jaw and an iron collar
/// where the neck meets the wall.
fn dragon_head(tick: u32) -> Img {
    let (w, h) = (60, 42);
    let mut im = Img::new(w, h);
    let mut scaly = vec![false; (w * h) as usize];
    // Green scales lit from above, a darker scale edge every third row.
    let scale = |x: i32, y: i32| {
        if y % 3 == 0 && (x + y / 3 * 2) % 4 == 0 {
            'F'
        } else if y < 12 {
            'm'
        } else if y < 17 {
            'l'
        } else if y < 27 {
            'E'
        } else {
            'F'
        }
    };
    // The far horn, behind the head.
    horn(&mut im, &[(40, 13), (46, 9), (52, 6), (58, 5)], 4, ['O', 'o', 'r']);
    let parts: [&[(i32, i32)]; 4] = [
        // The neck, back to the wall.
        &[(36, 13), (50, 9), (57, 12), (59, 18), (59, 36), (52, 41), (40, 38), (34, 30)],
        // The skull.
        &[(22, 11), (30, 7), (40, 7), (46, 12), (44, 22), (28, 23), (22, 18)],
        // The upper jaw, long to the nose.
        &[(1, 18), (5, 14), (14, 12), (24, 10), (32, 13), (34, 21), (12, 23), (2, 22)],
        // The lower jaw, dropped open.
        &[(6, 32), (20, 29), (34, 25), (40, 30), (36, 35), (16, 37), (7, 36)],
    ];
    for part in parts {
        poly(&mut im, part, |x, y| {
            if let Some(cell) = scaly.get_mut((y * w + x) as usize) {
                *cell = true;
            }
            Some(scale(x, y))
        });
    }
    // The open mouth: the dark throat, the tongue forked at its tip.
    poly(&mut im, &[(6, 24), (33, 21), (36, 26), (9, 32)], |x, _| Some(if x > 28 { 'k' } else { 'n' }));
    im.line(32, 25, 14, 29, 'p');
    im.line(31, 26, 15, 30, 'p');
    im.line(14, 29, 10, 27, 'R');
    im.line(14, 30, 10, 32, 'R');
    // Fangs: long ones down from the upper jaw, short ones up from the lower.
    for (x, long) in [(8, 4), (14, 3), (20, 3), (26, 2)] {
        im.line(x, 22, x, 22 + long, 'H');
        im.line(x + 1, 22, x + 1, 22 + long - 1, 'i');
    }
    for (x, top) in [(11, 29), (17, 28), (23, 27)] {
        im.line(x, top, x, top + 3, 'H');
        im.put(x + 1, top + 2, 'i');
    }
    // Tan plates under the jaw and down the throat.
    poly(&mut im, &[(12, 36), (36, 34), (44, 37), (52, 41), (40, 41), (14, 38)], |x, y| {
        Some(if (x + y) % 4 == 0 { 'r' } else if y < 38 { 'O' } else { 'o' })
    });
    // A dark edge round the green, so the head stands off the oak.
    let edge: Vec<(i32, i32)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            scaly[(y * w + x) as usize]
                && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    !(0..w).contains(&nx) || !(0..h).contains(&ny) || im.get(nx, ny).is_none()
                })
        })
        .collect();
    for (x, y) in edge {
        im.put(x, y, 'f');
    }
    // Spines along the skull and neck, and the frill at the jaw's hinge.
    for (bx, by) in [(24, 10), (30, 8), (46, 10), (51, 11)] {
        im.line(bx, by, bx + 1, by - 4, 'l');
        im.line(bx + 1, by, bx + 2, by - 3, 'E');
        im.put(bx + 1, by - 5, 'M');
        im.put(bx - 1, by, 'f');
    }
    for (x0, y0, x1, y1) in [(36, 24, 44, 21), (38, 28, 47, 27), (37, 32, 45, 35)] {
        im.line(x0, y0, x1, y1, 'l');
        im.line(x0, y0 + 1, x1, y1 + 1, 'f');
        im.put(x1, y1, 'M');
    }
    // A horn on the nose.
    im.line(5, 14, 4, 10, 'T');
    im.line(6, 14, 5, 11, 'o');
    // The brow, heavy over the eye; the eye amber and slit.
    im.line(22, 12, 33, 9, 'm');
    im.line(22, 13, 33, 10, 'l');
    im.line(24, 14, 32, 12, 'D');
    im.ellipse(28.0, 15.5, 3.6, 2.2, 'f');
    im.ellipse(28.0, 15.5, 2.8, 1.6, 't');
    im.line(26, 15, 30, 15, 'T');
    im.line(28, 14, 28, 17, 'k');
    im.put(27, 14, 'H');
    // The nostril, and its smoke drifting up off the snout.
    im.put(5, 16, 'k');
    im.put(6, 16, 'k');
    im.put(6, 15, 'D');
    let puff = (tick / 8 % 3) as i32;
    for k in 0..3 {
        let (sx, sy) = (3 - k - puff / 2, 13 - k * 2 - puff);
        if (0..w).contains(&sx) && (0..h).contains(&sy) {
            im.put(sx, sy, if k == 0 { 'h' } else { 'J' });
        }
    }
    // The near horn, over the skull.
    horn(&mut im, &[(36, 12), (42, 7), (48, 3), (54, 1), (59, 0)], 5, ['T', 't', 'o']);
    // The iron collar the neck is mounted in.
    im.rect(54, 13, 5, 26, 'X');
    im.line(54, 13, 54, 38, 'g');
    im.line(58, 13, 58, 38, 'K');
    for y in [16, 22, 28, 34] {
        im.put(56, y, 'J');
    }
    im
}

/// A horn along `path`, `thick` at its root and tapering to a point: its
/// top edge lit, its body, its underside in `inks`' order, ridged.
fn horn(im: &mut Img, path: &[(i32, i32)], thick: i32, inks: [char; 3]) {
    let steps: i32 = path.windows(2).map(|s| (s[1].0 - s[0].0).abs().max((s[1].1 - s[0].1).abs())).sum();
    let mut done = 0;
    for seg in path.windows(2) {
        let ((x0, y0), (x1, y1)) = (seg[0], seg[1]);
        let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=n {
            let (x, y) = (x0 + (x1 - x0) * i / n, y0 + (y1 - y0) * i / n);
            let t = (thick - (done + i) * thick / steps.max(1)).max(1);
            for d in 0..t {
                let ink = if d == 0 { inks[0] } else if d == t - 1 { inks[2] } else { inks[1] };
                im.put(x, y + d, if (done + i) % 5 == 4 && d > 0 { inks[2] } else { ink });
            }
        }
        done += n;
    }
}

/// Fill polygon `pts` (pixel corners), asking `ink` for each pixel inside.
fn poly(im: &mut Img, pts: &[(i32, i32)], mut ink: impl FnMut(i32, i32) -> Option<char>) {
    let lo = pts.iter().fold((i32::MAX, i32::MAX), |a, &(x, y)| (a.0.min(x), a.1.min(y)));
    let hi = pts.iter().fold((i32::MIN, i32::MIN), |a, &(x, y)| (a.0.max(x), a.1.max(y)));
    for y in lo.1.max(0)..=hi.1.min(im.h - 1) {
        for x in lo.0.max(0)..=hi.0.min(im.w - 1) {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut inside = false;
            let mut j = pts.len() - 1;
            for i in 0..pts.len() {
                let ((xi, yi), (xj, yj)) = ((pts[i].0 as f32, pts[i].1 as f32), (pts[j].0 as f32, pts[j].1 as f32));
                if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
                    inside = !inside;
                }
                j = i;
            }
            if inside && let Some(ch) = ink(x, y) {
                im.put(x, y, ch);
            }
        }
    }
}

/// The tavern's sign: a tankard on a board, hung from an iron arm that
/// reaches west from the inn's corner post at `(x, y)`, swinging a pixel in
/// the wind.
fn courtyard_sign(cv: &mut Img, (x, y): (i32, i32), tick: u32) {
    let x = x - 22;
    // The arm out from the post, and its brace.
    cv.line(x - 1, y, x + 22, y, 'K');
    cv.line(x + 17, y + 1, x + 21, y + 5, 'K');
    let sway = [0, 0, 1, 0, 0, -1][(tick / 9 % 6) as usize];
    // Two short chains, then the board.
    cv.line(x + 1, y + 1, x + 1 + sway, y + 3, 'J');
    cv.line(x + 14, y + 1, x + 14 + sway, y + 3, 'J');
    super::super::chrome::inset_panel(
        cv,
        (x - 2 + sway, y + 4, 20, 14),
        super::super::chrome::Tone::TIMBER.rimmed(super::super::chrome::rim_of('O')),
    );
    let tankard = Img::from_rows(&[
        ".666...",
        "66666..",
        "OOOOO..",
        "OOOOOOO",
        "OOOOO.O",
        "OOOOOOO",
        "OOOOO..",
        ".OOO...",
    ]);
    cv.stamp(&tankard, x + 5 + sway, y + 7);
}

/// Company silhouettes are immutable builtin kit art. Do not reconstruct
/// and recolour every visible body on every frame.
fn patron_bodies() -> &'static [Img] {
    static BODIES: std::sync::OnceLock<Vec<Img>> = std::sync::OnceLock::new();
    BODIES.get_or_init(|| PATRONS.iter().map(|p| {
        let colours = crate::rules::knights::knight(p.kit).map_or(&[][..], |k| k.colours);
        match crate::rules::champions::Champion::of_kit(p.kit) {
            Some(c) => super::champions::standing(c),
            None => knights::of_kit(Some(p.kit), colours),
        }
    }).collect())
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    // The hearth's flames and the stew over them.
    let (c, r, w, h) = HEARTH;
    let (hx, hy) = (c * T + w * T / 2, (r + h) * T - 4);
    // An ember bed under the flames.
    for k in 0..30 {
        let ex = hx - 28 + (k * 2) % 57;
        let ey = hy - 1 - (hash(k, 3, 29) % 3) as i32;
        cv.put(ex, ey, if hash(k, (tick / 9) as i32, 5) % 3 == 0 { 'T' } else { 'R' });
    }
    for (k, dx) in [-19, -6, 7, 20].into_iter().enumerate() {
        flame(cv, hx + dx, hy - 1 - (k as i32 % 2), tick, k as i32, 17 + (k as i32 % 3) * 4);
    }
    let pot_x = hx;
    cv.line(pot_x, r * T + 12, pot_x, hy - 22, 'g');
    cv.ellipse(pot_x as f32, (hy - 17) as f32, 8.0, 6.0, 'k');
    cv.line(pot_x - 8, hy - 21, pot_x + 8, hy - 21, 'j');
    cv.line(pot_x - 6, hy - 20, pot_x + 6, hy - 20, '6');
    cv.line(pot_x - 5, hy - 19, pot_x + 5, hy - 19, 'L');
    for k in 0..3 {
        let age = ((tick + k * 17) % 50) as i32;
        let sx = pot_x - 3 + k as i32 * 3 + (age as f32 * 0.3).sin() as i32 * 2;
        cv.put(sx, hy - 24 - age / 3, 'i');
    }
    // The keeper behind the bar, polishing a mug.
    let (kx, ky) = at(KEEPER_AT.0 * 2.0, KEEPER_AT.1 * 2.0);
    stand(cv, &Img::from_rows(&KEEPER), kx, ky);
    let swing = ((tick / 6) % 4) as i32;
    cv.rect(kx - 8 + swing, ky - 6, 4, 4, 'V');
    // The lute player on the stage, notes rising.
    let (lx, ly) = at(LUTE_AT.0 * 2.0, LUTE_AT.1 * 2.0);
    stand(cv, &Img::from_rows(&LUTE), lx, ly);
    for k in 0..2u32 {
        let age = ((tick + k * 41) % 80) as i32;
        let note = Img::from_rows(&["..h", "..h", "hhh", "hh."]);
        cv.stamp(&note, lx + 8 + (k as i32) * 5 + (age as f32 * 0.2).sin() as i32 * 2, ly - 22 - age / 3);
    }
    // The fence, hooded, at the crates.
    let (fx, fy) = at(FENCE_AT.0 * 2.0, FENCE_AT.1 * 2.0);
    stand(cv, &Img::from_rows(&FENCE), fx, fy);
    // The sleepers, and their z's.
    for (i, &(toy, pos)) in a::TOYS.iter().enumerate() {
        if let Toy::Snorer(n) = toy {
            let (sx, sy) = at((pos.0 - if n == 0 { 1.6 } else { -1.2 }) * 2.0, pos.1 * 2.0 + 3.0);
            let body = Img::from_rows(&SNORER);
            stand(cv, &if n == 1 { body.flip_h() } else { body }, sx, sy);
            let phase = ((tick / 12 + i as u32 * 5) % 6) as i32;
            for z in 0..3.min(phase / 2 + 1) {
                cv.text(sx + 6 + z * 4, sy - 14 - z * 5 - phase / 3, "Z", 'h');
            }
        }
    }
    // The two wrestlers, arms locked across the table.
    let wob = ((tick / 8) % 2) as i32;
    let (w0x, w0y) = at(WRESTLERS_AT[0].0 * 2.0, WRESTLERS_AT[0].1 * 2.0);
    let (w1x, w1y) = at(WRESTLERS_AT[1].0 * 2.0, WRESTLERS_AT[1].1 * 2.0);
    stand(cv, &Img::from_rows(&WRESTLER).recolor(&[('R', 'Q')]), w0x, w0y + wob);
    stand(cv, &Img::from_rows(&WRESTLER).recolor(&[('R', 'E')]).flip_h(), w1x, w1y - wob);
    patrons(cv, run, RoomKind::Alehouse, &TABLES);
    // The cat.
    let (cx, cy) = a::cat_at(run.tick);
    let (nx, ny) = at(cx * 2.0, cy * 2.0);
    let (ox, _) = a::cat_at(run.tick + 5);
    let step = (run.tick / 8) % 2 == 0;
    let cat = Img::from_rows(if step { &CAT } else { &CAT_B });
    stand(cv, &if ox < cx { cat } else { cat.flip_h() }, nx, ny);
    // The fire's sparks.
    for k in 0..4u32 {
        let age = ((tick * 2 + k * 29) % 70) as i32;
        let sx = hx - 14 + (hash(k as i32, (tick / 70) as i32, 3) % 28) as i32;
        cv.put(sx, hy - 16 - age / 2, if age < 30 { 'T' } else { 'o' });
    }
}

/// The patrons who drink in a room of `kind` on the benches behind its
/// `tables`, the table's edge over their knees; a seat taken shows its
/// knight's colour (the tavern's company, the muster tent's recruits).
pub(super) fn patrons(cv: &mut Img, run: &Run, kind: RoomKind, tables: &[(i32, i32, i32, i32)]) {
    let tick = run.tick as u32;
    let still = super::scenery_cached(run);
    for (i, p) in PATRONS
        .iter()
        .enumerate()
        .filter(|(i, _)| a::patron_visible(run.alehouse.company_page, *i) && a::patron_room(*i) == kind)
    {
        let (px, py) = at(p.at.0 * 2.0, (p.at.1 + 0.2) * 2.0);
        let taken = run
            .players
            .iter()
            .find(|(_, h)| h.knight.as_deref() == Some(p.kit) && h.hp > 0)
            .map(|(&id, _)| id);
        match taken {
            None => {
                let body = &patron_bodies()[i];
                let bob = ((tick / 40 + i as u32 * 3) % 5 == 0) as i32;
                stand(cv, body, px, py - bob);
                // A mug raised, now and then.
                let sip = (tick / 25 + i as u32 * 7) % 9 == 0;
                cv.rect(px + 7, py - 8 - if sip { 4 } else { 0 }, 3, 4, 'o');
            }
            // The seat's colour on the bench where they sat.
            Some(id) => {
                let pip = knights::seat_mark(id);
                cv.rect(px - 3, py - 4, 6, 3, pip);
            }
        }
    }
    for &rect in tables {
        let (x, y, tw, _) = slab_rect(rect);
        let edge = Img::from_view(&still, x, y, tw, 7);
        cv.stamp(&edge, x, y);
    }
}

/// What a line of speech needs: wrapped to a width in letters.
pub(super) fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(last) if last.len() + 1 + word.len() <= width => {
                last.push(' ');
                last.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// The authoritative page-aware target, shared by both board pipelines.
fn near_spot(run: &Run, id: u32) -> Option<Spot> {
    let hero = run.players.get(&id)?;
    if hero.hp <= 0 || hero.absent { return None; }
    match run.room().kind {
        RoomKind::Bazaar => bazaar::spot_near(hero.x, hero.y),
        kind => a::spot_near_in(kind, hero.x, hero.y, run.tick, run.alehouse.company_page),
    }
}

fn board_lines(run: &Run, id: u32) -> Vec<(String, char)> {
    if let Some(said) = run.alehouse.said.get(&id).filter(|s| run.tick.saturating_sub(s.at) < a::SAID_TICKS) {
        // An acknowledgement never grows into a paragraph over the hall.
        return wrap(&said.text, 34).into_iter().take(3).map(|l| (l, 't')).collect();
    }
    near_spot(run, id).map_or_else(Vec::new, |spot| prompt(run, id, spot))
}

/// A watcher gets one deterministic active reader, never a pile of six
/// overlapping world-space boards. A seat sees only its own interaction.
fn reader(run: &Run, focus: Option<u32>) -> Option<u32> {
    let eligible = |id: u32| run.players.get(&id).is_some_and(|h| h.hp > 0 && !h.absent);
    match focus {
        Some(id) => eligible(id).then_some(id),
        None => run.players.keys().copied().find(|&id| eligible(id) && !board_lines(run, id).is_empty()),
    }
}

/// Black paper with a quiet grey rule and warm amber lettering. No glow.
fn prompt_board(lines: &[(String, char)]) -> Img {
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 12;
    let h = lines.len() as i32 * 9 + 6;
    let mut im = Img::new(w, h);
    // True black paper, not the palette's brown ink.
    for y in 0..h { for x in 0..w { im.set(x, y, BLACK); } }
    im.line(1, 0, w - 2, 0, 'g');
    for (i, (text, _)) in lines.iter().enumerate() {
        im.text((w - text_width(text)) / 2, 4 + i as i32 * 9, text, if i == 0 { 't' } else { 'G' });
    }
    im
}

/// Top dock avoids the skills tray and never follows a world-space head
/// outside the camera crop. One rectangle per seat means no prompt overlap.
fn dock(cv: &Img, board: &Img) -> (i32, i32) {
    ((cv.w - board.w) / 2, 6)
}

/// Full-room overview/export path. Seat prompts are painted after cropping.
pub(super) fn boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    if focus.is_none() { paint_board(cv, run, None); }
}

pub(super) fn screen_boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    match focus {
        None => boards(cv, run, None),
        Some(id) => paint_board(cv, run, Some(id)),
    }
}

fn paint_board(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(id) = reader(run, focus) else { return; };
    let lines = board_lines(run, id);
    if lines.is_empty() { return; }
    let board = prompt_board(&lines);
    let (x, y) = dock(cv, &board);
    cv.stamp(&board, x, y);
}

/// What a spot says to the knight beside it.
fn prompt(run: &Run, id: u32, spot: Spot) -> Vec<(String, char)> {
    let buying = run.players.get(&id).map_or(0, |h| h.buying);
    let hold = || (format!("HOLD E {}", a::bar(buying)), 'O');
    match spot {
        Spot::Arm(slot) => {
            let Some(card) = a::home_arms().get(slot) else { return Vec::new() };
            let equipped = run.players.get(&id).is_some_and(|h| h.arm.as_deref() == Some(card.id.as_str()));
            vec![
                (card.name.to_uppercase(), 't'),
                ("FREE WEAPONS WALL".into(), 'G'),
                (if equipped { "EQUIPPED - SWAP AT PEG".into() }
                    else { format!("HOLD E TO EQUIP {}", a::bar(buying)) }, 't'),
            ]
        }
        Spot::Lamp => {
            let claimed = run.alehouse.lamp_claimed.contains(&id);
            let hero = &run.players[&id];
            vec![
                ("GENIE LAMP".into(), '9'),
                (if claimed { "THREE WISHES CLAIMED THIS RUN" } else { "THREE FREE WISHES, ONCE A RUN" }.into(), 'h'),
                (format!("{} WISHES HELD", hero.wishes + hero.great_wishes), '5'),
                (if hero.offer.is_some() { "CHOOSE FROM YOUR WISH SCROLL" } else if !claimed || run.can_wish(id) { "HOLD E: RUB LAMP, THEN WISH" } else { "NO WISHES HELD" }.into(), 'O'),
            ]
        }
        Spot::Patron(i) => {
            let p = &PATRONS[i];
            let mut lines = vec![(p.label.to_uppercase(), '9')];
            lines.extend(wrap(&p.pitch.to_uppercase(), 32).into_iter().map(|l| (l, 'h')));
            let mine = run.players.get(&id).and_then(crate::rules::alehouse::patron_of) == Some(i);
            if mine {
                lines.push(("YOU ARE THIS KNIGHT".into(), '5'));
            } else if run.patron_taken(i, id).is_some() {
                lines.push(("ADVENTURING WITH A FRIEND".into(), '7'));
            } else {
                lines.push(("E: BE THIS KNIGHT".into(), 'O'));
            }
            lines
        }
        Spot::Pour(i) => {
            let pour = &a::POURS[i];
            vec![
                (pour.name.to_uppercase(), '9'),
                (pour.does.to_string(), 'h'),
                (format!("{} GOLD", pour.price), '5'),
                hold(),
            ]
        }
        Spot::Fence => {
            let offer = run.fence_offer();
            let c = offer.curio();
            let mut lines = vec![(c.name.to_uppercase(), '9'), ("A BARGAIN. DO NOT ASK.".into(), 'h')];
            lines.push((format!("+ {}", c.boon), '5'));
            lines.push((format!("- {}", c.bane), '7'));
            lines.push((format!("{} GOLD", offer.price), '5'));
            lines.push(hold());
            lines
        }
        Spot::Ware(slot) => ware_lines(run, id, slot),
        Spot::Toy(toy) => vec![(Spot::Toy(toy).verb().to_string(), 'O')],
    }
}

/// A ware's board: its name, the vendor's line, its boon and bane, price.
pub(super) fn ware_lines(run: &Run, id: u32, slot: u8) -> Vec<(String, char)> {
    let buying = run.players.get(&id).map_or(0, |h| h.buying);
    let Some(offer) = run.bazaar_stock().get(usize::from(slot)).copied() else {
        return Vec::new();
    };
    let c = offer.curio();
    let price = run.ware_price(id, slot).unwrap_or(offer.price);
    let mut lines = vec![(c.name.to_uppercase(), '9')];
    if let Some(stall) = bazaar::STALL.get(usize::from(slot) / bazaar::WARES) {
        for l in wrap(stall.who, 40) {
            lines.push((l, 'i'));
        }
    }
    lines.extend(wrap(&c.says.to_uppercase(), 36).into_iter().map(|l| (l, 'h')));
    for l in wrap(&format!("+ {}", c.boon), 36) {
        lines.push((l, '5'));
    }
    for l in wrap(&format!("- {}", c.bane), 36) {
        lines.push((l, '7'));
    }
    let owned = run.players.get(&id).is_some_and(|h| h.pockets.has(c.id));
    if owned {
        lines.push(("IN YOUR POCKETS".into(), '5'));
    } else {
        lines.push((format!("{price} GOLD"), '5'));
        lines.push((format!("HOLD E {}", a::bar(buying)), 'O'));
    }
    lines
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__alehouse__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/render/arena__alehouse__lamp_readability.rs"]
mod lamp_readability;

#[cfg(test)]
#[path = "../../../../tests/render/arena__alehouse__rack_readability.rs"]
mod rack_readability;

#[cfg(test)]
#[path = "../../../../tests/render/arena__alehouse__bathroom.rs"]
mod bathroom_tests;
