//! The world above the Delve, drawn: the rooms of the realm the party walks
//! with the Delve's keys, in the overworld's own hand. Meadow tufts and
//! pebbled roads on black paper (the overworld's ground marks), trees and
//! the mountain's rock for their edges, its buildings at room scale, dusk
//! and lantern light. An entrance (a stair down, a mine's shaft) wears a
//! board when a knight comes near, and a ring that fills while one stands
//! on it.

use super::super::ground;
use super::super::ink::{Img, hash};
use super::super::kit::{self, RockKind, Tiles};
use super::{TILE, at, dusk, fire, home};
use crate::drive::together_shooter::world::{
    self, ADIT, GATE_ARCH, GATE_STAIR, PADDOCK, PAVILIONS, POST, SHAFT, SPOIL, STABLE_BLOCK,
    STANDS, TILT, TROUGH, WELL,
};
use crate::drive::together_shooter::{RoomKind, Run, Tile};
use crate::stage::world_viz::overworld::light::Light;

/// A rectangle of tiles: column, row, width, height.
type Rect = (i32, i32, i32, i32);

fn inside((c, r, w, h): Rect, col: i32, row: i32) -> bool {
    col >= c && col < c + w && row >= r && row < r + h
}

/// The roads through a room of the world, by its doors and entrances.
fn roads(kind: RoomKind) -> &'static [Rect] {
    match kind {
        // North-south from the realm's road to the mine-head; east-west
        // between the stables and the lists; a spur up to the gate's stair.
        RoomKind::Gate => &[(11, 0, 2, 14), (0, 6, 24, 2), (5, 5, 2, 1)],
        RoomKind::Stables => &[(2, 4, 20, 2), (21, 6, 3, 2), (19, 4, 2, 4)],
        RoomKind::Lists => &[(0, 6, 3, 2), (1, 8, 2, 2)],
        RoomKind::MineHead => &[(11, 4, 2, 10)],
        _ => &[],
    }
}

/// A road mark at pixel `(x, y)`: the overworld's pebbled bed, its edges
/// ragged where the road meets the meadow.
fn road_mark(kind: RoomKind, x: i32, y: i32) -> Option<char> {
    let (tx, ty) = (x.div_euclid(TILE), y.div_euclid(TILE));
    let (lx, ly) = (x.rem_euclid(TILE), y.rem_euclid(TILE));
    let on = |dx: i32, dy: i32| roads(kind).iter().any(|&r| inside(r, tx + dx, ty + dy));
    let rag = |a: i32, b: i32, s: u32| (hash(a, b, s) % 3) as i32 + 1;
    let mut edge = false;
    for (gone, dist, reach) in [
        (!on(-1, 0), lx, rag(y, tx, 11)),
        (!on(1, 0), TILE - 1 - lx, rag(y, tx, 12)),
        (!on(0, -1), ly, rag(x, ty, 13)),
        (!on(0, 1), TILE - 1 - ly, rag(x, ty, 14)),
    ] {
        if gone {
            if dist < reach {
                return ground::meadow(x, y);
            }
            if dist == reach {
                edge = true;
            }
        }
    }
    if edge {
        return (!hash(x, y, 16).is_multiple_of(3)).then_some('I');
    }
    match hash(x, y, 15) % 64 {
        0..=5 => Some('P'),
        6..=7 => Some('r'),
        8 => Some('o'),
        _ => None,
    }
}

/// The room's ground: roads where they run, meadow everywhere else.
fn ground(cv: &mut Img, run: &Run) {
    let room = run.room();
    let kind = room.kind;
    for y in 0..cv.h {
        for x in 0..cv.w {
            let (tx, ty) = (x / TILE, y / TILE);
            if !matches!(room.tile(tx, ty), Tile::Floor | Tile::Door | Tile::Stairs) {
                continue;
            }
            let on_road = roads(kind).iter().any(|&r| inside(r, tx, ty));
            let mark = if kind == RoomKind::KingsHall {
                // Indoors: dressed flagstones.
                super::flags(x, y)
            } else if on_road {
                road_mark(kind, x, y)
            } else if kind == RoomKind::MineHead {
                // Trodden ground before the mine: pebbles more than grass.
                if hash(x / 5, y / 4, 61).is_multiple_of(4) {
                    ground::sand(x, y)
                } else {
                    ground::meadow(x, y)
                }
            } else {
                ground::meadow(x, y)
            };
            if let Some(ch) = mark {
                cv.put(x, y, ch);
            }
        }
    }
}

/// The room's edges: trees in the meadows, the mountain's rock behind the
/// mine-head; a door leaves a gap for its road.
fn edges(cv: &mut Img, run: &Run) {
    let room = run.room();
    let tiles = Tiles::get();
    let mountain = room.kind == RoomKind::MineHead;
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            if room.tile(col, row) != Tile::Wall {
                continue;
            }
            let (x, y) = (col * TILE, row * TILE);
            let v = hash(col, row, 71);
            // The mountain rises behind the mine-head; trees close the
            // rest, thinning into the mountain's rock up its flanks.
            let rock = mountain && (row < 1 || (row < 8 && v % 3 != 0));
            if room.kind == RoomKind::KingsHall {
                // A hall's walls: dressed stone, braziers in the corners.
                let corner = (col == 0 || col == room.cols as i32 - 1)
                    && (row == 0 || row == room.rows as i32 - 1);
                if !corner {
                    cv.stamp(tiles.rock(RockKind::Slate, v), x, y);
                }
            } else if rock {
                cv.stamp(tiles.rock(RockKind::Ore, v), x, y);
            } else {
                cv.stamp(tiles.tree(v), x, y);
            }
        }
    }
}

/// Paint a room of the world at rest.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let (pw, ph) = (room.cols as i32 * TILE, room.rows as i32 * TILE);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    let tick = run.tick as u32;
    ground(&mut cv, run);
    edges(&mut cv, run);
    match room.kind {
        RoomKind::Gate => gate(&mut cv, &mut lights, tick),
        RoomKind::Stables => stables(&mut cv, &mut lights, tick),
        RoomKind::Lists => lists(&mut cv, &mut lights, tick),
        RoomKind::MineHead => mine_head(&mut cv, run, &mut lights, tick),
        RoomKind::KingsHall => super::barony::kings_hall(&mut cv, run, &mut lights, tick),
        _ => {}
    }
    dusk(&mut cv, &lights, ('F', 'I'));
    cv
}

/// A lantern on its post, lit; its light.
fn lantern(cv: &mut Img, lights: &mut Vec<Light>, x: i32, y: i32) {
    let im = kit::lantern(true);
    cv.stamp(&im, x - im.w / 2, y - im.h + 1);
    lights.push(fire(x, y - im.h + 3, 44.0, 0.34));
}

/// A torch on a timber post.
fn torch(cv: &mut Img, lights: &mut Vec<Light>, x: i32, y: i32, tick: u32, seed: u32) {
    let im = kit::torch(tick / 4, seed);
    cv.stamp(&im, x - im.w / 2, y - im.h + 1);
    lights.push(fire(x, y - im.h + 2, 50.0, 0.4));
}

/// The Delve's gate, at room scale: a slate rise, a dressed-stone arch, the
/// stair going down into the dark, a torch on either side. The overworld's
/// gate (`wishes::delve_sprite`) drawn twice the size, with its detail.
fn gate_arch(tick: u32) -> Img {
    let (w, h) = (GATE_ARCH.2 * TILE, GATE_ARCH.3 * TILE);
    let mut im = kit::blob(
        w,
        h,
        &[
            (20.0, 40.0, 20.0, 22.0),
            (60.0, 40.0, 20.0, 22.0),
            (40.0, 24.0, 30.0, 22.0),
        ],
        &['x', 'S', 'u', 'U', 'v'],
        0.5,
        29,
        2.0,
    );
    im.outline_inside('s');
    let (cx, cy) = (40.0f32, 40.0f32);
    let (ar, mr) = (17.0f32, 11.5f32);
    for y in 18..h {
        for x in 20..60 {
            let (dx, dy) = ((x as f32 + 0.5 - cx) / ar, (y as f32 + 0.5 - cy) / ar);
            let arch = y as f32 >= cy || dx * dx + dy * dy <= 1.0;
            let (mx, my) = ((x as f32 + 0.5 - cx) / mr, (y as f32 + 0.5 - cy) / mr);
            let mouth =
                (x as f32 + 0.5 - cx).abs() <= mr && (y as f32 >= cy || mx * mx + my * my <= 1.0);
            if mouth {
                // Treads lit near the top, then only the dark going down.
                let tread = y >= 44 && (y - 44) % 5 == 0;
                let lip = y >= 44 && (y - 44) % 5 == 1;
                im.put(
                    x,
                    y,
                    if tread && y < 58 {
                        'j'
                    } else if lip && y < 58 {
                        'g'
                    } else {
                        'k'
                    },
                );
            } else if arch {
                // Dressed blocks: courses three high, joints staggered.
                let course = y / 4;
                let joint = y % 4 == 0 || (x + course * 5) % 9 == 0;
                im.put(x, y, if joint { 'J' } else { 'h' });
            }
        }
    }
    // The keystone and the voussoirs' bright edge.
    for k in 0..40 {
        let a = std::f32::consts::PI * (k as f32 / 39.0);
        let (x, y) = (cx - a.cos() * (ar - 0.5), cy - a.sin() * (ar - 0.5));
        im.put(x as i32, y as i32, 'i');
    }
    im.rect(37, 20, 6, 4, 'H');
    im.rect(38, 21, 4, 2, 'i');
    let _ = tick;
    im
}

fn gate(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let (c, r, _, _) = GATE_ARCH;
    let arch = gate_arch(tick);
    cv.stamp(&arch, c * TILE, r * TILE);
    // Torches either side of the mouth, burning: the Delve is open.
    let (sc, sr, sw, _) = GATE_STAIR;
    for (k, x) in [(0u32, sc * TILE - 6), (1, (sc + sw) * TILE + 5)] {
        torch(cv, lights, x, sr * TILE + 14, tick, 31 + k);
    }
    // The well, its roof and bucket.
    let (wc, wr, _, _) = WELL;
    cv.stamp(&well(), wc * TILE, wr * TILE);
    signpost(cv, 13 * TILE + 4, 6 * TILE - 2);
    // The notice post by the road: the realm's notices, Wren's hand.
    let (pc, pr, _, _) = POST;
    let (px, py) = (pc * TILE + 8, pr * TILE + 15);
    cv.rect(px - 1, py - 14, 2, 15, 'B');
    cv.rect(px - 7, py - 15, 14, 8, 'b');
    cv.rect(px - 6, py - 14, 12, 6, 'P');
    for (dx, dy, ink) in [
        (-5, -13, 'T'),
        (-1, -12, 't'),
        (2, -13, 'T'),
        (-4, -10, 't'),
    ] {
        cv.rect(px + dx, py + dy, 3, 2, ink);
    }
    // The road home to the realm, out through the palisade's south gate.
    for x in [10 * TILE + 12, 13 * TILE + 3] {
        cv.rect(x, 13 * TILE, 2, TILE, 'B');
        cv.put(x, 13 * TILE, 'o');
    }
    lantern(cv, lights, 9 * TILE + 2, 5 * TILE + 8);
    lantern(cv, lights, 14 * TILE + 2, 9 * TILE + 2);
    lantern(cv, lights, 9 * TILE + 8, 11 * TILE);
}

/// The courtyard's well at room scale: a ring of dressed stone round dark
/// water, a little shingled roof on two posts, its bucket on the rope.
fn well() -> Img {
    let mut im = Img::new(32, 32);
    // The ring, seen from above and a little in front.
    for y in 14..30 {
        for x in 2..30 {
            let (dx, dy) = ((x as f32 - 15.5) / 13.5, (y as f32 - 22.0) / 7.5);
            let d = dx * dx + dy * dy;
            if d <= 1.0 {
                let water = d < 0.45 && y < 25;
                let joint = (x + (y / 3) * 3) % 6 == 0 || y % 3 == 0;
                im.put(
                    x,
                    y,
                    if water {
                        if (x + y) % 7 == 0 { 'Q' } else { 'q' }
                    } else if joint {
                        'G'
                    } else if y < 22 {
                        'h'
                    } else {
                        'J'
                    },
                );
            }
        }
    }
    // Two posts and the roof.
    im.rect(5, 6, 2, 18, 'B');
    im.rect(25, 6, 2, 18, 'B');
    for y in 0..7 {
        let inset = 6 - y;
        for x in 2 + inset..30 - inset {
            im.put(x, y, if (x + y) % 3 == 0 { 'x' } else { 'S' });
        }
    }
    im.line(2, 7, 29, 7, 'b');
    // The windlass and the bucket on its rope.
    im.line(7, 9, 24, 9, 'r');
    im.line(16, 10, 16, 15, 'o');
    im.rect(14, 15, 5, 4, 'B');
    im.line(14, 15, 18, 15, 'o');
    im.outline_inside('k');
    im
}

/// A signpost at the crossroads: an arm to each way, its name on it.
fn signpost(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x, y - 40, 2, 41, 'r');
    cv.line(x, y - 40, x, y, 'o');
    // North on top, pointing up; west and east below, pointing their ways.
    for (i, (name, way)) in [("MINES", 0), ("STABLES", -1), ("LISTS", 1)]
        .into_iter()
        .enumerate()
    {
        let w = super::super::ink::text_width(name) + 6;
        let by = y - 40 + i as i32 * 10;
        let bx = match way {
            -1 => x - w + 1,
            1 => x + 1,
            _ => x + 1 - w / 2,
        };
        cv.rect(bx, by, w, 8, 'B');
        cv.line(bx, by, bx + w - 1, by, 'r');
        match way {
            -1 => cv.line(bx - 1, by + 3, bx - 1, by + 4, 'B'),
            1 => cv.line(bx + w, by + 3, bx + w, by + 4, 'B'),
            _ => cv.line(bx + w / 2 - 1, by - 1, bx + w / 2, by - 1, 'r'),
        }
        cv.text(bx + 3, by + 1, name, 'T');
    }
}

/// The stables, at rest: the stable block with its three stalls, the
/// trough, the paddock fence (the horses are figures).
fn stables(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let (c, r, w, h) = STABLE_BLOCK;
    let (x0, y0, bw, bh) = (c * TILE, r * TILE, w * TILE, h * TILE);
    // A long thatched roof over timber walls, three stall doors open.
    for y in y0..y0 + 22 {
        for x in x0..x0 + bw {
            // Thatch in combed bands, the ridge lit, the eaves in shadow.
            let ramp = ['o', 'r', 'P', 'p', 'B'];
            let k = ((y - y0) * 5 / 22) as usize;
            let comb = (x + (y - y0) / 3) % 5 == 0;
            let straw = hash(x / 2, y, 41).is_multiple_of(7);
            cv.put(
                x,
                y,
                if straw {
                    'O'
                } else if comb {
                    'b'
                } else {
                    ramp[k.min(4)]
                },
            );
        }
    }
    cv.line(x0, y0, x0 + bw - 1, y0, 't');
    cv.line(x0, y0 + 22, x0 + bw - 1, y0 + 22, 'b');
    for y in y0 + 23..y0 + bh {
        for x in x0..x0 + bw {
            let board = (x - x0) % 6 == 0;
            cv.put(x, y, if board { 'I' } else { 'B' });
        }
    }
    for &(dx, dw) in &stall_doors() {
        cv.rect(dx, y0 + 25, dw, bh - 25, 'k');
        cv.rect(dx - 1, y0 + 24, dw + 2, 1, 'o');
        cv.line(dx - 1, y0 + 24, dx - 1, y0 + bh - 1, 'r');
        cv.line(dx + dw, y0 + 24, dx + dw, y0 + bh - 1, 'r');
    }
    // The trough: a timber box, water in it.
    let (tc, tr, tw, _) = TROUGH;
    let (tx, ty) = (tc * TILE + 2, tr * TILE + 4);
    cv.rect(tx, ty, tw * TILE - 4, 9, 'B');
    cv.rect(tx + 2, ty + 2, tw * TILE - 8, 4, 'q');
    cv.line(tx + 3, ty + 3, tx + 9, ty + 3, 'Q');
    // The paddock fence: two rails on posts.
    let (fc, fr, fw, _) = PADDOCK;
    let (fx, fy) = (fc * TILE, fr * TILE + 8);
    for x in fx..fx + fw * TILE {
        cv.put(x, fy - 4, 'o');
        cv.put(x, fy, 'r');
    }
    for x in (fx..fx + fw * TILE).step_by(16) {
        cv.rect(x, fy - 7, 2, 10, 'B');
    }
    // Hay bales, a saddle on its stand.
    for (bx, by) in [(18 * TILE + 4, 6 * TILE + 2), (19 * TILE + 6, 6 * TILE + 8)] {
        cv.rect(bx, by, 12, 8, 'o');
        cv.line(bx, by + 3, bx + 11, by + 3, 'r');
        cv.line(bx, by, bx + 11, by, 'T');
    }
    lantern(cv, lights, 12 * TILE, 5 * TILE - 2);
    let _ = tick;
}

/// Where the three stall doors stand along the stable block (x, width).
pub(super) fn stall_doors() -> [(i32, i32); 3] {
    [(5 * TILE, 22), (11 * TILE + 5, 22), (17 * TILE + 10, 22)]
}

/// The lists, at rest: the stands and their crowd's benches, the tilt's
/// striped rail, the pavilions.
fn lists(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    // Raked sand inside the lists' fence.
    for y in 4 * TILE..12 * TILE {
        for x in 3 * TILE..21 * TILE {
            if hash(x, y, 102).is_multiple_of(11) {
                cv.put(x, y, 'I');
            }
        }
    }
    let (c, r, w, h) = STANDS;
    let (x0, y0, sw, sh) = (c * TILE, r * TILE, w * TILE, h * TILE);
    // A canopy in the realm's red and gold over tiered benches.
    for x in x0..x0 + sw {
        for y in y0..y0 + 6 {
            cv.put(x, y, if (x / 6) % 2 == 0 { 'T' } else { 'R' });
        }
        cv.put(x, y0 + 6, 'p');
        for y in y0 + 7..y0 + sh {
            let tier = (y - y0 - 7) % 7 == 0;
            cv.put(x, y, if tier { 'r' } else { 'b' });
        }
    }
    // The tilt: a striped rail on posts down the middle of the lists.
    let (tc, tr, tw, _) = TILT;
    let (tx, ty) = (tc * TILE, tr * TILE + 6);
    for x in tx..tx + tw * TILE {
        cv.put(x, ty, if (x / 6) % 2 == 0 { 'T' } else { 'R' });
        cv.put(x, ty + 1, if (x / 6) % 2 == 0 { 'o' } else { 'p' });
        cv.put(x, ty + 2, 'B');
    }
    for x in (tx..tx + tw * TILE).step_by(12) {
        cv.rect(x, ty - 2, 2, 8, 'b');
    }
    // The fence round the lists.
    for x in 3 * TILE..21 * TILE {
        cv.put(x, 12 * TILE + 4, 'r');
    }
    for x in (3 * TILE..21 * TILE).step_by(8) {
        cv.rect(x, 12 * TILE + 2, 2, 5, 'B');
    }
    // The pavilions: the realm's (red, west) and the rival's (blue, east).
    for (i, (pc, pr, pw, ph)) in PAVILIONS.into_iter().enumerate() {
        let tent = kit::tent(if i == 0 {
            kit::Heraldry::Red
        } else {
            kit::Heraldry::Blue
        });
        cv.stamp(
            &tent,
            pc * TILE + (pw * TILE - tent.w) / 2,
            (pr + ph) * TILE - tent.h,
        );
    }
    lantern(cv, lights, 2 * TILE + 4, 6 * TILE - 2);
    lantern(cv, lights, 22 * TILE - 2, 9 * TILE);
    let _ = tick;
}

/// The mine-head: the mountain's adit with its timbers and the shaft going
/// down, the fallen gate-hall of Caer Dwfn to the west, spoil heaps and a
/// cart to the east.
fn mine_head(cv: &mut Img, run: &Run, lights: &mut Vec<Light>, tick: u32) {
    // The adit: a dark mouth in the rock, timbered, rails going in.
    let (c, r, w, h) = ADIT;
    let (x0, y0, ah) = (c * TILE, r * TILE, h * TILE);
    let tiles = Tiles::get();
    for row in 0..h {
        for col in 0..w {
            cv.stamp(
                tiles.rock(RockKind::Ore, hash(c + col, r + row, 73)),
                x0 + col * TILE,
                y0 + row * TILE,
            );
        }
    }
    let (sc, _, sw, _) = SHAFT;
    let (mx0, mx1) = (sc * TILE - 2, (sc + sw) * TILE + 2);
    for y in y0 + 10..y0 + ah {
        for x in mx0..mx1 {
            let tread = y > y0 + ah - 14 && (y - y0) % 4 == 0;
            cv.put(x, y, if tread { 'g' } else { 'k' });
        }
    }
    // The timber set: two posts, a lintel, a cap piece.
    cv.rect(mx0 - 4, y0 + 8, 4, ah - 8, 'B');
    cv.rect(mx1, y0 + 8, 4, ah - 8, 'B');
    cv.rect(mx0 - 6, y0 + 5, mx1 - mx0 + 12, 4, 'r');
    cv.line(mx0 - 6, y0 + 5, mx1 + 5, y0 + 5, 'o');
    cv.line(mx0 - 4, y0 + 8, mx0 - 4, y0 + ah - 1, 'p');
    // Rails out of the mouth, down the road.
    for x in [mx0 + 6, mx1 - 7] {
        cv.line(x, y0 + ah, x, 13 * TILE, 'h');
    }
    for y in ((y0 + ah)..13 * TILE).step_by(5) {
        cv.line(mx0 + 4, y, mx1 - 5, y, 'b');
    }
    torch(cv, lights, mx0 - 10, y0 + ah + 6, tick, 5);
    torch(cv, lights, mx1 + 9, y0 + ah + 6, tick, 6);
    // Spoil heaps and a cart east of the adit.
    let (pc, pr, _, ph) = SPOIL;
    let heaps = kit::blob(
        64,
        32,
        &[
            (14.0, 22.0, 12.0, 9.0),
            (34.0, 19.0, 14.0, 12.0),
            (52.0, 23.0, 11.0, 8.0),
        ],
        &['b', 'I', 'r', 'P', 'R'],
        0.5,
        83,
        2.0,
    );
    cv.stamp(&heaps, pc * TILE, (pr + ph) * TILE - heaps.h);
    let cart = kit::handcart();
    cv.stamp(&cart, 15 * TILE, 5 * TILE);
    // The gate-hall of Caer Dwfn: a ruin, until the realm pays to raise it.
    super::barony::gate_hall(cv, run, lights, tick);
}

/// Over everything in a room of the world: the board of the entrance a
/// knight is near.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    for e in world::ENTRANCES.iter().filter(|e| e.room == kind) {
        let (ex, ey) = world::entrance_centre(e);
        let reach = if world::open(e, &run.home) { 4.2 } else { 1.8 };
        let near = run
            .players
            .values()
            .any(|h| h.hp > 0 && (h.x - ex).hypot(h.y - ey) < reach);
        if !near {
            continue;
        }
        let board = if world::open(e, &run.home) {
            home::board(&[
                (e.label.to_string(), '9'),
                ("STAND ON THE STAIR".into(), 'O'),
            ])
        } else {
            home::board(&[
                ("THE GATE-HALL OF CAER DWFN".to_string(), '9'),
                ("FALLEN IN: ITS DOOR IS RUBBLE".to_string(), 'h'),
            ])
        };
        let (x, y) = at(ex, ey);
        // Beside the entrance, never over the knights who arrive below it.
        let bx = if x + 22 + board.w <= cv.w - 2 {
            x + 22
        } else {
            x - 22 - board.w
        };
        cv.stamp(
            &board,
            bx.clamp(2, cv.w - board.w - 2),
            (y - board.h / 2).clamp(2, cv.h - board.h - 2),
        );
    }
}

/// The ring filling round the entrance a knight stands on.
pub(super) fn entrance_ring(cv: &mut Img, run: &Run) {
    if run.descending == 0 {
        return;
    }
    let Some((e, _)) = world::entrance_under(run) else {
        return;
    };
    let (ex, ey) = world::entrance_centre(e);
    let (cx, cy) = at(ex, ey);
    let k = run.descending as f32 / crate::drive::together_shooter::home::DESCEND_HOLD as f32;
    let n = 48;
    for i in 0..(n as f32 * k.min(1.0)) as i32 {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
        for r in [16.0f32, 17.0] {
            cv.put(
                cx + (a.cos() * r) as i32,
                cy + (a.sin() * r) as i32,
                if i % 2 == 0 { '6' } else { '5' },
            );
        }
    }
}
