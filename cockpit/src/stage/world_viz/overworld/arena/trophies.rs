//! The Trophy Hall, drawn: a long hall south of the Undercroft, the
//! realm's red runner from its door to the Grail's dais, plinths of pale
//! stone along its walls. Each guardian the realm has felled stands on its
//! plinth as a statuette (its own art at half size, cast in bronze, gilded
//! after ten), the dragon's in the middle; the Grail stands on its dais once
//! found. Sir Kay keeps it, by the door.

use super::super::ink::{Img, Rgb, hash};
use super::super::kit::{self, RockKind, Tiles};
use super::{TILE, at, dusk, fire, flags, home, stand};
use crate::drive::together_shooter::trophies::{GILDED, KAY_AT, PLINTHS, Plinth, plinth_at};
use crate::drive::together_shooter::{Run, Tile};
use crate::stage::world_viz::overworld::light::Light;

/// The hall at rest.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let tick = run.tick as u32;
    let tiles = Tiles::get();
    let (pw, ph) = (room.cols as i32 * TILE, room.rows as i32 * TILE);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    for y in 0..ph {
        for x in 0..pw {
            if !matches!(room.tile(x / TILE, y / TILE), Tile::Floor | Tile::Door) {
                continue;
            }
            if let Some(ch) = flags(x, y) {
                cv.put(x, y, ch);
            }
        }
    }
    runner(&mut cv);
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            if room.tile(col, row) != Tile::Wall {
                continue;
            }
            let (x, y) = (col * TILE, row * TILE);
            let corner = (col == 0 || col == room.cols as i32 - 1)
                && (row == 0 || row == room.rows as i32 - 1);
            if corner {
                let im = kit::brazier(tick / 4, (col * 3 + row) as u32);
                cv.stamp(&im, x + (TILE - im.w) / 2, y + TILE - im.h);
                lights.push(fire(x + TILE / 2, y + 4, 56.0, 0.32));
            } else {
                cv.stamp(tiles.rock(RockKind::Slate, hash(col, row, 17)), x, y);
            }
        }
    }
    super::tavern::rubble(&mut cv, run);
    for plinth in &PLINTHS {
        let felled = run.home.trophies.get(plinth.id).copied().unwrap_or(0);
        pedestal(&mut cv, plinth, felled);
        let (col, row, w, _) = plinth.at;
        let (cx, top) = (col * TILE + w * TILE / 2, row * TILE + 3);
        if felled == 0 {
            sheet(&mut cv, cx, top);
            continue;
        }
        let piece = piece(run, plinth.id, felled >= GILDED);
        cv.stamp(&piece, cx - piece.w / 2, top - piece.h + 3);
        if plinth.id == "grail" {
            lights.push(fire(cx, top - 6, 60.0, 0.4));
        }
    }
    dusk(&mut cv, &lights, ('x', 'I'));
    cv
}

/// The realm's red runner, from the door to the Grail's dais.
fn runner(cv: &mut Img) {
    let (x0, x1) = (11 * TILE + 2, 13 * TILE - 2);
    let (y0, y1) = (0, 10 * TILE - 2);
    for y in y0..y1 {
        for x in x0..x1 {
            let edge = x == x0 || x == x1 - 1;
            let inner = x == x0 + 2 || x == x1 - 3;
            let ink = if edge {
                'p'
            } else if inner {
                'O'
            } else {
                'B'
            };
            cv.put(x, y, ink);
        }
    }
}

/// A plinth of pale stone, a brass plate on its face; a bigger one for the
/// dragon, a stepped dais for the Grail.
fn pedestal(cv: &mut Img, plinth: &Plinth, felled: u32) {
    let (col, row, w, h) = plinth.at;
    let (x0, y0, pw, ph) = (col * TILE + 1, row * TILE + 2, w * TILE - 2, h * TILE - 3);
    cv.rect(x0, y0, pw, ph, 'j');
    cv.rect(x0, y0, pw, 3, 'V');
    cv.line(x0, y0, x0 + pw - 1, y0, 'i');
    cv.line(x0, y0 + 3, x0 + pw - 1, y0 + 3, 'J');
    cv.line(x0, y0 + ph - 1, x0 + pw - 1, y0 + ph - 1, 'g');
    cv.line(x0, y0, x0, y0 + ph - 1, 'G');
    cv.line(x0 + pw - 1, y0, x0 + pw - 1, y0 + ph - 1, 'G');
    // The plate: brass, brighter once something stands above it.
    let plate = if felled > 0 { '5' } else { 'a' };
    let (mx, my) = (x0 + pw / 2, y0 + ph / 2 + 1);
    cv.rect(mx - 4, my - 1, 8, 3, plate);
    if plinth.id == "grail" {
        // Steps, and two candles either side.
        cv.line(x0 - 3, y0 + ph, x0 + pw + 2, y0 + ph, 'J');
        cv.line(x0 - 3, y0 + ph + 1, x0 + pw + 2, y0 + ph + 1, 'g');
    }
}

/// A dust sheet over an empty plinth: what has not been felled yet.
fn sheet(cv: &mut Img, cx: i32, top: i32) {
    let im = Img::from_rows(&[
        "...JJJJ...",
        "..JhhhhJ..",
        ".JhhJhhhJ.",
        ".JhhhhJhJ.",
        "JhhJhhhhhJ",
        "JJJJJJJJJJ",
    ]);
    cv.stamp(&im, cx - im.w / 2, top - im.h + 3);
}

/// What stands on a plinth: the guardian's statuette, the dragon's, or the
/// Grail itself.
fn piece(run: &Run, id: &str, gilded: bool) -> Img {
    match id {
        "grail" => Img::from_rows(&[
            ".4444444.",
            "4@66666@4",
            "4@66666@4",
            ".4@666@4.",
            "..4@6@4..",
            "...4@4...",
            "....4....",
            "...4@4...",
            "..44444..",
        ]),
        "dragon" => statuette(&kit::dragon(false, 0), gilded),
        _ => match run.bosses.iter().find(|b| b.id == id) {
            Some(boss) => statuette(
                &Img::from_rows(&boss.art.iter().map(String::as_str).collect::<Vec<_>>()),
                gilded,
            ),
            None => Img::new(1, 1),
        },
    }
}

/// A guardian's art at half size, cast in bronze (or gold) by how light
/// each part of it is.
pub(super) fn statuette(art: &Img, gilded: bool) -> Img {
    let inks = if gilded {
        ['8', 'a', '4', '@', '6']
    } else {
        ['n', 'p', 'R', 'o', 'T']
    };
    let (w, h) = ((art.w + 1) / 2, (art.h + 1) / 2);
    let mut out = Img::new(w, h);
    let light = |c: Rgb| 0.3 * c[0] as f32 + 0.59 * c[1] as f32 + 0.11 * c[2] as f32;
    for y in 0..h {
        for x in 0..w {
            let brightest = [(0, 0), (1, 0), (0, 1), (1, 1)]
                .into_iter()
                .filter_map(|(dx, dy)| art.get(x * 2 + dx, y * 2 + dy))
                .map(light)
                .fold(None, |best: Option<f32>, l| {
                    Some(best.map_or(l, |b| b.max(l)))
                });
            if let Some(l) = brightest {
                let band = ((l / 256.0) * inks.len() as f32) as usize;
                out.put(x, y, inks[band.min(inks.len() - 1)]);
            }
        }
    }
    out
}

/// Sir Kay by the door, and the Grail's candles.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let (kx, ky) = (KAY_AT.0 * 2.0, KAY_AT.1 * 2.0);
    let (x, y) = at(kx, ky);
    stand(cv, &Img::from_rows(&KAY), x, y);
    if let Some(grail) = PLINTHS.iter().find(|p| p.id == "grail") {
        let (col, row, w, _) = grail.at;
        for (k, cx) in [(0u32, col * TILE - 6), (1, (col + w) * TILE + 4)] {
            let lit = !hash((tick / 5) as i32, k as i32, 91).is_multiple_of(3);
            let y0 = row * TILE + 2;
            cv.rect(cx, y0, 2, 8, 'c');
            cv.put(cx, y0 - 1, if lit { '6' } else { '5' });
            cv.put(cx + 1, y0 - 2, if lit { '5' } else { '@' });
        }
    }
}

/// A plinth's plaque, to a knight standing at its front.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let Some((hero, plinth)) = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .find_map(|h| Some((h, plinth_at(h.x, h.y)?)))
    else {
        return;
    };
    let felled = run.home.trophies.get(plinth.id).copied().unwrap_or(0);
    let found = plinth.id == "grail";
    let count = match (felled, found) {
        (0, true) => "STILL BELOW".to_string(),
        (0, false) => "STILL OUT THERE".to_string(),
        (1, true) => "FOUND ONCE".to_string(),
        (1, false) => "FELLED ONCE".to_string(),
        (n, true) => format!("FOUND {n} TIMES"),
        (n, false) => format!("FELLED {n} TIMES"),
    };
    let mut lines = vec![
        (plinth.name.to_uppercase(), '9'),
        (count, if felled > 0 { 'H' } else { 'h' }),
    ];
    if felled >= GILDED && !found {
        lines.push(("GILDED".to_string(), '5'));
    } else if felled > 0 && !found {
        lines.push((format!("GILDED AT {GILDED}"), 'h'));
    }
    // Under the knight, so the piece it is about stays in sight; over them
    // at the far end of the hall.
    let board = home::board(&lines);
    let (x, y) = at(hero.x, hero.y);
    let bx = (x - board.w / 2).clamp(2, cv.w - board.w - 2);
    let by = if y + 10 + board.h < cv.h - 2 {
        y + 10
    } else {
        (y - 40 - board.h).max(2)
    };
    cv.stamp(&board, bx, by);
}

/// Sir Kay, the seneschal: grey, stern, his keys at his belt.
pub(super) const KAY: [&str; 17] = [
    "....hiiih......",
    "...hiiiiih.....",
    "..hhOOOOOhh....",
    "...KKOOOKK.....",
    "...OKOOOKO.....",
    "...OhhhhhO.....",
    "....OhhhO......",
    "...pRRRRRp.....",
    "..pRRR5RRRp....",
    ".OpRR555RRpO...",
    ".OpRRR5RRRpO...",
    "..pRRR5RRRp....",
    "..bBB4B4BBb....",
    "...RRR4RRR.....",
    "...nnn.nnn.....",
    "...nnn.nnn.....",
    "..Knnn.nnnK....",
];
