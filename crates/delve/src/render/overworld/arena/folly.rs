//! The follies as a seat sees them: the Belfry Shoot, the Keg Gantry, Book
//! Hop and the Powder Room, each with its standings board and its title card
//! (`rules::folly`).

use super::*;

/// The Belfry Shoot's board and title card (the bats are the room's own).
pub(super) fn bats_layer(over: &mut Img, run: &Run) {
    let Some(shoot) = run.bats.as_ref().filter(|b| b.room == run.at) else {
        return;
    };
    // The Belfry's own strip, title and moments (`belfry_shoot`) stand in for this board.
    if super::belfry_shoot::walled(run) {
        return;
    }
    let head = if shoot.done { "BELFRY  DONE".to_string() } else { format!("BELFRY  {}S", shoot.left()) };
    let rows: Vec<(u32, u32)> = shoot.scores.iter().map(|(&id, &n)| (id, n)).collect();
    folly_board(over, run, &head, &rows);
    if shoot.titled() {
        folly_title(over, "THE BELFRY SHOOT", "SHOOT BATS.");
    }
}

/// The Keg Gantry: the ogre on the top girder, the kegs rolling and
/// falling, the board (how high each knight has climbed) and the title.
pub(super) fn gantry_layer(over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    use crate::rules::gantry::TOP_ROW;
    let Some(g) = run.gantry.as_ref().filter(|g| g.room == run.at) else {
        return;
    };
    if !g.done {
        // The ogre, swaying at the top-left with a keg on his shoulder.
        let (x, y) = at(1.6 * TILE_UNITS, (TOP_ROW as f32 + 0.1) * TILE_UNITS);
        let (x, y) = (x - ox, y - oy);
        let sway = [0, 1, 0, -1][(run.tick / 8 % 4) as usize];
        let ogre = [
            "....mmmm....",
            "...mmmmmm...",
            "...mkmmkm...",
            "...mmmmmm...",
            "....m77m....",
            ".mmmmmmmmmm.",
            "mmmMMMMMMmmm",
            "m.mMMMMMMm.m",
            "m.mMMMMMMm.m",
            "...MMMMMM...",
            "...mm..mm...",
            "...mm..mm...",
        ];
        // Drawn twice the size: an ogre should loom.
        for (ry, row) in ogre.iter().enumerate() {
            for (rx, c) in row.chars().enumerate() {
                if c != '.' {
                    over.rect(x - 8 + sway + rx as i32 * 2, y - 24 + ry as i32 * 2, 2, 2, c);
                }
            }
        }
        for k in &g.kegs {
            let (kx, ky) = k.pos();
            let (px, py) = at(kx * TILE_UNITS / TILE_UNITS, ky);
            let (px, py) = (px - ox, py - oy);
            let roll = (kx * 4.0) as i32 % 2 == 0;
            over.rect(px - 4, py - 4, 9, 8, 'R');
            over.rect(px - 4, py - 2, 9, 1, 'h');
            over.rect(px - 4, py + 1, 9, 1, 'h');
            over.put(px - 1 + i32::from(roll), py - 1, 'o');
        }
    }
    let head = if g.done { "GANTRY  DONE".to_string() } else { format!("GANTRY  {}S", g.left()) };
    let rows: Vec<(u32, u32)> = g.height.iter().map(|(&id, &h)| (id, u32::from(h))).collect();
    folly_board(over, run, &head, &rows);
    if g.titled() {
        folly_title(over, "THE KEG GANTRY", "CLIMB. JUMP KEGS.");
    }
}

/// Book Hop's flood, under the knights: ink in the lanes, the banks and the
/// landing dry, the books drifting, and ripples where someone sank.
pub(super) fn hop_floor(cv: &mut Img, run: &Run) {
    use crate::rules::hop;
    let Some(h) = run.hop.as_ref().filter(|h| h.room == run.at) else {
        return;
    };
    // The drowned archive (`book_hop`) lays its own hall, books and strip.
    if super::book_hop::walled(run) {
        return;
    }
    let (cols, rows) = (run.room().cols as i32, run.room().rows as i32);
    let width = cols as f32 * TILE_UNITS;
    let t = h.t.saturating_sub(crate::rules::folly::TITLE);
    for r in 1..rows - 1 {
        let y = r * TILE;
        let Some(l) = hop::lane(rows, r) else {
            // A dry bank or the landing: flagstones, each lit along its top
            // and shaded at its foot, their joints staggered row to row.
            let shift = if r % 2 == 0 { 0 } else { TILE / 2 };
            for yy in y..y + TILE {
                for x in TILE..(cols - 1) * TILE {
                    let (dx, dy) = ((x - TILE + shift).rem_euclid(TILE), yy - y);
                    let ink = if dx == 0 || dy == TILE - 1 {
                        's'
                    } else if dy == 0 {
                        'S'
                    } else if dy >= TILE - 3 {
                        'x'
                    } else if hash((x - TILE + shift) / TILE, r, 61) % 3 == 0 {
                        'S'
                    } else {
                        'x'
                    };
                    cv.put(x, yy, ink);
                }
            }
            continue;
        };
        // The water: one river from bank to bank (lane to lane the same
        // water, no stripes), its shallows a step paler against a bank;
        // ripples in short clumps that drift with each lane's flow.
        let flow = (t as i32 * if l % 2 == 0 { 1 } else { -1 }) / 3;
        let bank_above = hop::lane(rows, r - 1).is_none();
        let bank_below = hop::lane(rows, r + 1).is_none();
        for yy in y..y + TILE {
            let dy = yy - y;
            // The stone bank's own steps (no signal ink: water is scenery).
            let shore = (bank_above && dy < 2) || (bank_below && dy >= TILE - 2);
            let shallow = (bank_above && dy < 4) || (bank_below && dy >= TILE - 4);
            let base = if shore {
                rgb(0x2f4a52)
            } else if shallow {
                rgb(0x263b40)
            } else {
                rgb(0x12516a)
            };
            for x in TILE..(cols - 1) * TILE {
                let wx = (x - TILE + flow).rem_euclid((cols - 2) * TILE);
                let glint = (4..TILE - 4).contains(&dy)
                    && hash(wx / 5, (dy + l as i32) / 2, 211 + l as u32) % 9 == 0
                    && (wx % 5) < 3;
                cv.set(x, yy, if glint { rgb(0x527597) } else { base });
            }
        }
        let (lefts, len) = hop::books(l, t, width);
        let spines = ['R', 'B', 'e', 'U', 'P'];
        // A book running off one end comes back in at the other.
        let span = lefts.len() as i32 * hop::SPACING as i32 * TILE;
        for (k, &bx) in lefts.iter().enumerate() {
            let x0 = (bx / TILE_UNITS * TILE as f32) as i32;
            let w = (len / TILE_UNITS * TILE as f32) as i32;
            for piece in [x0, x0 - span] {
                let (a, b) = (piece.max(TILE), (piece + w).min((cols - 1) * TILE));
                if a >= b {
                    continue;
                }
                // A book's spine, its ends rounded where they are its own
                // (not where it runs off the lane), lit along its top in its
                // own colour's lighter step and shaded along its foot, with
                // gilt bands.
                let ink = spines[(k + l) % spines.len()];
                let c = super::super::ink::ink(ink).unwrap_or(BLACK);
                let (lit, deep) = (super::super::material::lighter(c), super::super::material::darker(c));
                let (h0, h1) = (y + 2, y + TILE - 2);
                for yy in h0..h1 {
                    let row = yy - h0;
                    let last = h1 - h0 - 1;
                    let end = row == 0 || row == last;
                    let (from, to) = (
                        if end && a == piece { a + 1 } else { a },
                        if end && b == piece + w { b - 1 } else { b },
                    );
                    let tone = if row == 0 {
                        lit
                    } else if row == last || row == last - 1 {
                        deep
                    } else {
                        c
                    };
                    for x in from..to {
                        cv.set(x, yy, tone);
                    }
                }
                for sx in (a + 5..b - 2).step_by(9) {
                    cv.line(sx, y + 4, sx, y + TILE - 5, 'O');
                }
            }
        }
    }
    for &(x, y, left) in &h.splashes {
        let (px, py) = at(x, y);
        let rad = (hop_ripple(left) as f32).max(2.0);
        cv.ellipse(px as f32, py as f32, rad, rad / 2.0, 'U');
    }
}

fn hop_ripple(left: u32) -> u32 {
    3 + (crate::rules::HZ / 2 - left.min(crate::rules::HZ / 2)) / 2
}

/// Book Hop's board and title, over everything.
pub(super) fn hop_layer(over: &mut Img, run: &Run) {
    let Some(h) = run.hop.as_ref().filter(|h| h.room == run.at) else {
        return;
    };
    if super::book_hop::walled(run) {
        return;
    }
    let head = if h.done { "BOOK HOP  DONE".to_string() } else { format!("BOOK HOP  {}S", h.left()) };
    let rows: Vec<(u32, u32)> = h.crossings.iter().map(|(&id, &n)| (id, u32::from(n))).collect();
    folly_board(over, run, &head, &rows);
    if h.titled() {
        folly_title(over, "BOOK HOP", "HOP. DON'T SINK.");
    }
}

/// The Powder Room: lit kegs, the fire of a blast, the board and the title.
/// Crate tiles are already drawn as room blocks, beneath the actors.
pub(super) fn powder_layer(over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    let Some(p) = run.powder.as_ref().filter(|p| p.room == run.at) else {
        return;
    };
    // The store's own strip, kegs and moments (`powder_room`) stand in for this board.
    if super::powder_room::walled(run) {
        return;
    }
    let spot = |tx: i32, ty: i32| {
        let (x, y) = at(tx as f32 * TILE_UNITS, ty as f32 * TILE_UNITS);
        (x - ox, y - oy)
    };
    for k in &p.kegs {
        let (x, y) = spot(k.at.0, k.at.1);
        over.rect(x + 4, y + 5, 8, 9, 'K');
        over.rect(x + 4, y + 7, 8, 1, 'G');
        over.rect(x + 4, y + 11, 8, 1, 'G');
        // The fuse burns down, sparking.
        let spark = if run.tick / 3 % 2 == 0 { '6' } else { '7' };
        over.put(x + 9, y + 3, spark);
        over.put(x + 8, y + 4, 'h');
    }
    for &((tx, ty), left) in &p.fire {
        let (x, y) = spot(tx, ty);
        let ink = if left > 6 { '6' } else { '7' };
        over.rect(x + 2, y + 2, TILE - 4, TILE - 4, ink);
        over.rect(x + 5, y + 5, TILE - 10, TILE - 10, '9');
    }
    let head = if p.done { "POWDER  DONE".to_string() } else { format!("POWDER  {}S", p.left()) };
    let rows: Vec<(u32, u32)> = p.broken.iter().map(|(&id, &n)| (id, u32::from(n))).collect();
    folly_board(over, run, &head, &rows);
    if p.titled() {
        folly_title(over, "THE POWDER ROOM", "BOMB THE CRATES.");
    }
}

/// A folly's standings, top right: a headline, then each knight's colour,
/// name and score.
pub(super) fn folly_board(over: &mut Img, run: &Run, head: &str, rows: &[(u32, u32)]) {
    let (w, row) = (118, 11);
    let x0 = NATIVE_W - w - 5;
    chrome::panel(over, (x0 - 4, 1, w + 8, row * (rows.len() as i32 + 1) + 5), chrome::Tone::SLATE);
    over.text(x0, 4, head, 'O');
    for (k, &(id, n)) in rows.iter().enumerate() {
        let y = 4 + (k as i32 + 1) * row;
        let ink = knights::seat_mark(id);
        over.rect(x0, y + 1, 5, 5, ink);
        let name: String = run
            .players
            .get(&id)
            .map_or_else(|| format!("SEAT {id}"), |h| h.name.to_uppercase())
            .chars()
            .take(9)
            .collect();
        over.text(x0 + 8, y, &name, 'O');
        let count = format!("{n}");
        over.text(x0 + w - super::super::ink::text_width(&count), y, &count, 'i');
    }
}

/// A folly's title card: its name, and how to play in a few words.
pub(super) fn folly_title(over: &mut Img, a: &str, b: &str) {
    let tw = super::super::ink::text_width(b).max(super::super::ink::text_width(a));
    // High in the view: the middle is where an unused wish waits.
    let (bx, by) = ((NATIVE_W - tw) / 2 - 8, 44);
    chrome::panel(over, (bx, by, tw + 16, 34), chrome::Tone::FRAMED);
    over.text((NATIVE_W - super::super::ink::text_width(a)) / 2, by + 7, a, 'O');
    over.text((NATIVE_W - super::super::ink::text_width(b)) / 2, by + 20, b, 'O');
}
