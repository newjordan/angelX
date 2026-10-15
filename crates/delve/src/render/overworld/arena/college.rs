//! Original pixel scenery and portrait for the College of the Labyrinth.
use super::super::ink::{Img, hash, text_width};
use super::{TILE, at, stand};
use crate::rules::{RoomKind, Run, Tile, college as c, folk};

/// Bartosz: an original storybook wizard, with a crooked hat and a kind face.
pub fn wizard(tick: u32) -> Img {
    let mut im = Img::from_rows(&[
        "...........v.............",
        "..........vVu............",
        ".........vVVVu...........",
        ".........vVVVVuu.........",
        "........vVuVVVuuu........",
        ".......vVuVVVVVuuu.......",
        "......vVVVVVVVVuuuu......",
        ".....uuuuuuuuuuuuuuuu....",
        ".....vVVVVVVVVVVVVVv.....",
        "......vOOOOOOOOOOv.......",
        "......vOHHOOHHOOOv..uVu..",
        "......vOHkOOHkOOOv..VVV..",
        ".......OOOooOOO....uVu...",
        ".......HiOOOOiH.....T....",
        "......vHiHOOHiHv....T....",
        ".....vVuHiHHiHuuu...O....",
        "....vVVuuHiHiuuVu..OTO...",
        "...vVVVuuuHuuuVVuu.OOO...",
        "...vVVVuVuuVVVVVuu..T....",
        "...vVVVuVVuVVVVVuu..T....",
        "....OOOvVVuVVVuuuu..T....",
        ".....OOvVVuVVuuuuu..T....",
        "......vVVVuVuuuuuu..T....",
        "......vVVVuVVVuuuu..T....",
        ".....vVVVVuVVVVuuu..T....",
        ".....vVVVVuVVVVuuu..T....",
        "....vVVVVVuVVVVuuuu.T....",
        "....vvvvvvvvvvvvvvv.T....",
        "......nnn....nnn....T....",
        ".....nnnn....nnnn........",
    ]);
    for yy in 0..im.h {
        for xx in 0..im.w {
            if let Some(rgb) = im.get(xx, yy) {
                for (a, b) in [('v', 'q'), ('V', 'Q'), ('u', 'z')] {
                    if Some(rgb) == super::super::ink::ink(a) {
                        im.put(xx, yy, b);
                        break;
                    }
                }
            }
        }
    }
    if tick % 100 < 4 {
        im.rect(8, 11, 2, 1, 'O');
        im.rect(12, 11, 2, 1, 'O');
    }
    // A pocket folio, silver beard and a quietly breathing staff light.
    im.rect(3, 18, 7, 8, 'b');
    im.frame(3, 18, 7, 8, 'O');
    im.line(5, 20, 8, 20, 'T');
    im.line(5, 23, 8, 23, 't');
    im.put(10, 8, 'T');
    im.put(8, 13, 'i');
    im.put(11, 16, 'V');
    im.put(10, 18, 'i');
    let mut out = Img::new(32, 38);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(rgb) = im.get(x * im.w / out.w, y * im.h / out.h) {
                out.set(x, y, rgb);
            }
        }
    }
    let pulse = (tick / 8 % 4).min(4 - tick / 8 % 4) as i32;
    out.ellipse(27.0, 14.0, (3 + pulse) as f32, (4 + pulse) as f32, 'q');
    out.ellipse(27.0, 14.0, 2.0, 3.0, 'z');
    out.put(27, 13, 'V');
    out
}
pub fn student(tick: u32) -> Img {
    let mut im = Img::from_rows(&[
        "......P.......",
        ".....PrP......",
        "....PrrrP.....",
        "...PPPPPPP....",
        "...bOOOOOb....",
        "....OkOkO.....",
        "....OOOOO.....",
        "...SqQQQqS....",
        "..SqQQQQQSS...",
        "..SOQQQQOTTT..",
        "...OQQQQOTHT..",
        "...SqQQQQTTT..",
        "...SqQQQqS....",
        "...SSSSSSS....",
        "....nn.nn.....",
        "...nnn.nnn....",
    ]);
    if (tick / 6) % 2 == 1 {
        im.rect(3, 15, 2, 1, 'n');
        im.rect(8, 15, 2, 1, 'n');
    }
    im
}
fn sign(cv: &mut Img, x: i32, y: i32, text: &str) {
    let w = text_width(text) + 10;
    super::super::chrome::panel(cv, (x - w / 2, y, w, 13), super::super::chrome::Tone::SLATE);
    cv.text(x - w / 2 + 5, y + 3, text, 'i');
}
fn candle(cv: &mut Img, x: i32, y: i32, t: u32) {
    cv.ellipse(x as f32, y as f32 - 7.0, 6.0 + (t / 7 % 2) as f32, 5.0, 'P');
    cv.ellipse(x as f32, y as f32 + 2.0, 7.0, 2.0, 'n');
    cv.rect(x - 1, y - 7, 3, 8, 'T');
    cv.put(x, y - 8, 'O');
    cv.put(x + (t / 5 % 2) as i32, y - 10, 'T');
}
fn shelf(cv: &mut Img, x: i32, y: i32, w: i32, h: i32, seed: u32) {
    cv.rect(x, y, w, h, 'n');
    cv.frame(x, y, w, h, 'O');
    for row in (y + 4..y + h - 2).step_by(11) {
        cv.rect(x + 2, row + 7, w - 4, 2, 'r');
        for col in (x + 4..x + w - 5).step_by(5) {
            let ink = ['u', 'V', 'e', 'r', 'O', 'i'][(hash(col, row, seed) % 6) as usize];
            let ht = 5 + (hash(col, row, seed + 1) % 3) as i32;
            cv.rect(col, row + 7 - ht, 3, ht, ink);
            cv.put(col + 1, row + 8 - ht, 'T');
        }
    }
    cv.line(x + 1, y + 1, x + 1, y + h - 1, 't');
}
fn table(cv: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    cv.rect(x + 3, y + h, w - 6, 4, 'n');
    cv.rect(x, y, w, h, 'r');
    cv.frame(x, y, w, h, 'P');
    cv.line(x + 2, y + 2, x + w - 3, y + 2, 'O');
    for yy in (y + 5..y + h).step_by(5) {
        cv.line(x + 2, yy, x + w - 3, yy, 'B');
    }
}
fn book(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x, y, 18, 11, 'O');
    cv.rect(x + 1, y - 1, 7, 10, 'T');
    cv.rect(x + 9, y - 1, 7, 10, 'i');
    cv.line(x + 8, y, x + 8, y + 10, 'P');
    for yy in [2, 5, 7] {
        cv.line(x + 2, y + yy, x + 6, y + yy, 'o');
        cv.line(x + 10, y + yy, x + 14, y + yy, 'h');
    }
}
fn map(cv: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    cv.rect(x, y, w, h, 't');
    cv.frame(x, y, w, h, 'O');
    for i in 0..5 {
        let sx = x + 5 + i * (w - 10) / 5;
        let sy = y + 5 + ((i * 7) % (h - 12));
        cv.rect(sx, sy, 7, 5, 'P');
        if i < 4 {
            cv.line(sx + 3, sy + 2, sx + 3 + (w - 10) / 5, sy + 2, 'r');
        }
    }
    cv.text(x + 4, y + h - 10, "MAP", 'b');
}
fn jar(cv: &mut Img, x: i32, y: i32, ink: char, t: u32) {
    cv.rect(x - 5, y - 15, 10, 3, 'O');
    cv.rect(x - 6, y - 12, 12, 14, 'u');
    cv.frame(x - 6, y - 12, 12, 14, 'V');
    cv.rect(x - 4, y - 5, 8, 5, ink);
    cv.line(x - 4, y - 11, x - 4, y - 6, 'H');
    cv.put(x, y - 7 - (t / 10 % 3) as i32, ink);
}
fn stair(cv: &mut Img, x: i32, y: i32, up: bool) {
    cv.rect(x - 5, y - 22, 42, 40, 's');
    for i in 0..7 {
        let yy = y + 14 - i * 5;
        let xx = x + if up { i * 2 } else { 12 - i * 2 };
        cv.rect(xx, yy, 25, 4, 'u');
        cv.line(xx, yy, xx + 24, yy, 'i');
        cv.line(xx, yy + 4, xx + 24, yy + 4, 'x');
    }
    cv.line(x - 5, y - 22, x - 5, y + 18, 'O');
    cv.line(x + 37, y - 22, x + 37, y + 18, 'O');
}
fn cauldron(cv: &mut Img, x: i32, y: i32, t: u32) {
    cv.ellipse(x as f32, y as f32 + 7.0, 23.0, 7.0, 'n');
    cv.ellipse(x as f32, y as f32, 21.0, 14.0, 'x');
    cv.ellipse(x as f32 - 3.0, y as f32 - 3.0, 17.0, 10.0, 'u');
    cv.ellipse(x as f32, y as f32 - 8.0, 22.0, 9.0, 'h');
    cv.ellipse(x as f32, y as f32 - 9.0, 18.0, 6.0, 'e');
    for i in 0..5 {
        let bx = x - 12 + i * 6;
        let by = y - 11 - ((t / 4 + i as u32 * 3) % 17) as i32;
        cv.ellipse(bx as f32, by as f32, 2.0, 2.0, 'C');
        cv.put(bx, by - 1, 'Y');
    }
    for i in 0..3 {
        let age = (t / 3 + i * 11) % 31;
        let drift = ((age / 5 + i) % 3) as i32 - 1;
        let sy = y - 20 - age as i32;
        cv.ellipse((x + drift * 4) as f32, sy as f32, 4.0, 2.0, 'e');
        cv.line(x + drift * 4 - 2, sy, x + drift * 4 + 2, sy, 'C');
    }
    cv.rect(x - 14, y + 11, 4, 6, 's');
    cv.rect(x + 10, y + 11, 4, 6, 's');
    for i in [-9, 0, 9] {
        cv.rect(x + i, y + 14, 4, 3, 'r');
        cv.put(x + i + 1, y + 12 - (t / 3 % 3) as i32, 't');
    }
}
fn tower_window(cv: &mut Img, x: i32, y: i32, t: u32) {
    cv.ellipse(x as f32, y as f32 + 5.0, 16.0, 9.0, 'S');
    cv.rect(x - 14, y, 28, 34, 'k');
    cv.frame(x - 14, y, 28, 34, 'u');
    cv.ellipse(x as f32 + 6.0, y as f32 + 7.0, 3.0, 4.0, 'U');
    for i in 0..8 {
        let sx = x - 10 + (hash(i, 3, 81) % 20) as i32;
        let sy = y + 3 + (hash(i, 9, 80) % 26) as i32;
        cv.put(
            sx,
            sy,
            if (t / 9 + i as u32) % 4 == 0 {
                'V'
            } else {
                'Q'
            },
        );
    }
    cv.line(x, y, x, y + 33, 'S');
    cv.line(x - 13, y + 20, x + 13, y + 20, 'S');
    cv.rect(x - 17, y + 34, 34, 3, 'U');
}

fn rug(cv: &mut Img, x: i32, y: i32, w: i32, h: i32, ink: char) {
    cv.rect(x, y, w, h, ink);
    cv.frame(x, y, w, h, 'P');
    cv.frame(x + 3, y + 3, w - 6, h - 6, 'O');
    for xx in (x + 5..x + w - 4).step_by(8) {
        cv.line(xx, y - 2, xx, y, 'o');
        cv.line(xx, y + h, xx, y + h + 2, 'o');
    }
    for yy in (y + 9..y + h - 5).step_by(12) {
        cv.line(x + 7, yy, x + 10, yy + 3, 'S');
        cv.line(x + w - 11, yy + 3, x + w - 8, yy, 'S');
    }
}
fn banner(cv: &mut Img, x: i32, y: i32, ink: char) {
    cv.rect(x - 2, y, 22, 2, 'O');
    cv.rect(x, y + 2, 18, 27, ink);
    cv.frame(x + 2, y + 4, 14, 22, 'P');
    cv.line(x + 9, y + 8, x + 5, y + 14, 't');
    cv.line(x + 5, y + 14, x + 9, y + 20, 't');
    cv.line(x + 9, y + 20, x + 13, y + 14, 't');
    cv.line(x + 13, y + 14, x + 9, y + 8, 't');
}
fn hearth(cv: &mut Img, x: i32, y: i32, t: u32) {
    cv.rect(x, y, 58, 27, 'g');
    cv.frame(x, y, 58, 27, 'j');
    cv.rect(x + 7, y + 5, 44, 20, 'n');
    cv.rect(x - 3, y - 2, 64, 4, 'P');
    cv.rect(x - 3, y + 26, 64, 4, 'j');
    for i in 0..5 {
        let xx = x + 13 + i * 7;
        let ht = 6 + ((t / 4 + i as u32 * 3) % 7) as i32;
        cv.rect(xx, y + 24 - ht, 5, ht, 'R');
        cv.line(xx + 2, y + 22, xx + 2, y + 23 - ht, 'O');
        cv.put(xx + 2, y + 24 - ht, 't');
    }
    cv.line(x + 10, y + 24, x + 47, y + 24, 'b');
}
fn reading_lamp(cv: &mut Img, x: i32, y: i32, t: u32) {
    cv.ellipse(x as f32, y as f32, 20.0, 7.0, 'P');
    cv.rect(x - 3, y - 1, 7, 2, 'O');
    cv.line(x, y - 12, x, y - 1, 'O');
    cv.rect(x - 6, y - 16, 13, 5, 'e');
    cv.line(x - 7, y - 11, x + 7, y - 11, 'o');
    cv.put(x + (t / 9 % 2) as i32, y - 10, 'T');
}
fn material(kind: RoomKind, wall: bool, x: i32, y: i32) -> char {
    use RoomKind::*;
    if wall {
        let (joint, face, edge) = match kind {
            CollegeHall => ('b', 'P', 'r'),
            CollegeClass => ('b', 'I', 'P'),
            CollegeLibrary => ('n', 'I', 'P'),
            CollegeLab => ('s', 'e', 'S'),
            TowerEntry => ('X', 'g', 'j'),
            TowerStairs => ('s', 'S', 'u'),
            TowerStars => ('s', 'q', 'Q'),
            WizardStudy => ('b', 'P', 'o'),
            _ => ('s', 'x', 'u'),
        };
        // Each brick lit along its top and west edge, its corners worn
        // round into the joints.
        let (lx, ly) = ((x + (y / 8 % 2) * 8) % 16, y % 8);
        return if ly == 0 || lx == 0 {
            joint
        } else if (lx == 1 || lx == 15) && (ly == 1 || ly == 7) {
            face
        } else if ly == 1 || lx == 1 {
            edge
        } else {
            face
        };
    }
    match kind {
        CollegeGrounds => {
            if (8 * TILE..16 * TILE).contains(&x) || y > 10 * TILE {
                if x % 16 == 0 || y % 16 == 0 { 'P' } else { 'g' }
            } else {
                ['D', 'F', 'F', 'E', 'e'][(hash(x, y, 93) % 5) as usize]
            }
        }
        CollegeHall | CollegeClass | CollegeLibrary | WizardStudy => {
            let (floor, grain) = match kind {
                CollegeHall => ('I', 'B'),
                CollegeLibrary => ('b', 'I'),
                WizardStudy => ('I', 'P'),
                _ => ('I', 'b'),
            };
            if y % 8 == 0 || (x + y / 8 * 17) % 65 == 0 {
                'b'
            } else if y % 8 == 4 && hash(x / 9, y / 8, 43) % 4 == 0 {
                grain
            } else {
                floor
            }
        }
        CollegeLab => {
            if y % 16 == 0 || (x + y / 16 % 2 * 16) % 32 == 0 {
                's'
            } else if hash(x / 32, y / 16, 41) % 4 == 0 {
                'e'
            } else {
                'x'
            }
        }
        _ => {
            // Concentric joints read as the curved flags of a round tower.
            let dx = x - 192;
            let dy = (y - 110) * 3 / 2;
            let radius = ((dx * dx + dy * dy) as f32).sqrt() as i32;
            let (floor, seam, edge) = match kind {
                TowerEntry => ('g', 'X', 'j'),
                TowerStairs => ('S', 's', 'u'),
                _ => ('x', 's', 'S'),
            };
            if radius % 23 == 0 || x % 48 == 0 {
                seam
            } else if radius % 23 == 1 {
                edge
            } else {
                floor
            }
        }
    }
}

pub fn scenery(run: &Run) -> Img {
    let mut cv = Img::black(run.room().cols as i32 * TILE, run.room().rows as i32 * TILE);
    let kind = run.room().kind;
    let t = run.tick as u32;
    let state = run.college_progress();
    // Each room keeps its own tonal material; furniture leaves clear aisles.
    for y in 0..cv.h {
        for x in 0..cv.w {
            let tile = run.room().tile(x / TILE, y / TILE);
            let ink = material(kind, tile == Tile::Wall, x, y);
            cv.put(x, y, ink);
        }
    }
    cv.line(TILE, TILE, cv.w - TILE - 1, TILE, 'u');
    // Low cutaway walls preserve the routes and the room's playable floor.
    for e in c::DOORS.iter().filter(|e| e.room == kind) {
        let (x, y, w, _) = e.at;
        let (px, py) = (x * TILE, y * TILE);
        cv.rect(px, py, w * TILE, TILE, 's');
        cv.frame(px, py, w * TILE, TILE, 'O');
        for i in 0..4 {
            cv.line(
                px + 3,
                py + 3 + i * 3,
                px + w * TILE - 4,
                py + 3 + i * 3,
                'u',
            );
        }
    }
    match kind {
        RoomKind::CollegeGrounds => {
            // School and tower facades sit above the playable door tiles.
            let school = building(t);
            cv.stamp(&school, 88, 1 * TILE - 4);
            let tower = tower(t);
            cv.stamp(&tower, 17 * TILE, 25);
            let (cx, cy) = (184, 110);
            cv.ellipse(cx as f32, cy as f32 + 6.0, 32.0, 19.0, 's');
            cv.ellipse(cx as f32, cy as f32, 30.0, 17.0, 'i');
            cv.ellipse(cx as f32, cy as f32, 24.0, 12.0, 'u');
            for i in 0..5 {
                cv.ellipse(
                    cx as f32,
                    cy as f32,
                    (6 + i * 4) as f32,
                    (3 + i * 2) as f32,
                    if (t / 5 + i as u32) % 3 == 0 {
                        'V'
                    } else {
                        'S'
                    },
                );
            }
            cv.rect(cx - 3, cy - 21, 7, 23, 'h');
            cv.ellipse(cx as f32, cy as f32 - 22.0, 13.0, 5.0, 'i');
            cv.line(cx, cy - 36, cx, cy - 23, 'V');
            cv.put(cx - 1, cy - 37, 'H');
            for (x, y) in [(55, 146), (305, 158), (65, 90), (320, 99)] {
                cv.ellipse(x as f32, y as f32, 15.0, 7.0, 'D');
                for i in 0..6 {
                    cv.put(x - 10 + i * 4, y - (i % 2) * 3, 'Y');
                }
            }
            for (x, y) in [(48, 185), (287, 188)] {
                table(&mut cv, x, y, 44, 6);
            }
        }
        RoomKind::CollegeHall => {
            rug(&mut cv, 10 * TILE, 6 * TILE, 4 * TILE, 6 * TILE, 'e');
            hearth(&mut cv, 162, 18, t);
            banner(&mut cv, 100, 24, 'q');
            banner(&mut cv, 266, 24, 'e');
            for x in [4 * TILE, 15 * TILE] {
                table(&mut cv, x, 8 * TILE, 5 * TILE, 2 * TILE);
                book(&mut cv, x + 28, 8 * TILE + 8);
                candle(&mut cv, x + 9, 8 * TILE + 5, t);
                table(&mut cv, x + 4, 8 * TILE - 10, 5 * TILE - 8, 5);
                table(&mut cv, x + 4, 10 * TILE + 5, 5 * TILE - 8, 5);
                for dx in [14, 61] {
                    cv.ellipse((x + dx) as f32, 151.0, 5.0, 2.0, 'h');
                    cv.rect(x + dx + 8, 146, 3, 5, 'O');
                }
            }
            for x in [7 * TILE, 16 * TILE] {
                cv.rect(x, 5 * TILE - 16, 16, 33, 'u');
                cv.line(x, 5 * TILE - 16, x, 5 * TILE + 16, 'i');
                cv.rect(x - 2, 5 * TILE + 13, 20, 5, 'h');
            }
        }
        RoomKind::CollegeClass => {
            let (x, y) = (6 * TILE, 2 * TILE);
            super::super::chrome::inset_panel(&mut cv, (x, y, 12 * TILE, 2 * TILE), super::super::chrome::Tone::CHALK);
            cv.text(x + 12, y + 7, "q -> test -> claim", 'C');
            cv.text(x + 12, y + 19, "x + y = ?", 'Y');
            for &(x, y, w, h) in c::blocks(kind).iter().skip(1) {
                table(
                    &mut cv,
                    x as i32 * TILE,
                    y as i32 * TILE,
                    w as i32 * TILE,
                    h as i32 * TILE,
                );
                book(&mut cv, x as i32 * TILE + 8, y as i32 * TILE + 3);
            }
            for x in [58, 325] {
                candle(&mut cv, x, 69, t);
            }
            banner(&mut cv, 35, 33, 'e');
            banner(&mut cv, 331, 33, 'q');
            shelf(&mut cv, 31, 125, 23, 24, 18);
            cv.rect(335, 126, 14, 18, 'P');
            cv.frame(335, 126, 14, 18, 'O');
            cv.line(338, 131, 345, 131, 't');
        }
        RoomKind::CollegeLibrary => {
            rug(&mut cv, 104, 86, 174, 57, 'e');
            rug(&mut cv, 47, 148, 86, 42, 'q');
            rug(&mut cv, 252, 148, 84, 42, 'q');
            for &(x, y, w, h) in c::blocks(kind) {
                shelf(
                    &mut cv,
                    x as i32 * TILE,
                    y as i32 * TILE,
                    w as i32 * TILE,
                    h as i32 * TILE + 8,
                    17,
                );
            }
            for x in [139, 245] {
                cv.line(x, 30, x - 8, 87, 'O');
                cv.line(x + 10, 30, x + 2, 87, 't');
                for i in 0..7 {
                    let y = 34 + i * 8;
                    cv.line(x - i, y, x + 10 - i, y, 'T');
                }
            }
            for (i, x) in [130, 190, 250].into_iter().enumerate() {
                let phase = (t / 6 + i as u32 * 4) % 16;
                let y = 94 + phase.min(16 - phase) as i32;
                cv.ellipse(x as f32 + 9.0, 127.0, 12.0, 3.0, 's');
                book(&mut cv, x, y);
            }
            for x in [96, 256] {
                table(&mut cv, x, 144, 32, 16);
                book(&mut cv, x + 5, 147);
                reading_lamp(&mut cv, x + 24, 143, t);
                table(&mut cv, x + 8, 167, 18, 5);
            }
        }
        RoomKind::CollegeLab => {
            // Muted pools of green, kept inside the work area.
            for x in [88, 184, 296] {
                cv.ellipse(x as f32, 122.0, 35.0, 20.0, 'e');
                cv.ellipse(x as f32, 119.0, 27.0, 14.0, 'S');
            }
            for x in [3 * TILE, 16 * TILE] {
                shelf(&mut cv, x, 2 * TILE, 5 * TILE, 2 * TILE, 12);
                for j in 0..4 {
                    jar(
                        &mut cv,
                        x + 12 + j * 18,
                        2 * TILE + 24,
                        ['e', 'u', 'r', 'C'][j as usize],
                        t,
                    );
                }
            }
            for x in [88, 184, 296] {
                cauldron(&mut cv, x, 112, t + x as u32);
            }
            table(&mut cv, 9 * TILE, 9 * TILE, 6 * TILE, 18);
            book(&mut cv, 169, 149);
            jar(&mut cv, 205, 151, 'e', t);
            for (i, x) in [47, 69, 315, 337].into_iter().enumerate() {
                jar(&mut cv, x, 173, ['C', 'e', 'm', 'u'][i], t);
            }
            for x in [40, 343] {
                cv.line(x, 78, x + 12, 78, 'P');
                for dx in [2, 6, 10] {
                    cv.line(x + dx, 79, x + dx - 2, 90, 'e');
                    cv.put(x + dx - 3, 86, 'm');
                }
            }
        }
        RoomKind::TowerEntry => {
            rug(&mut cv, 159, 91, 63, 91, 'I');
            banner(&mut cv, 72, 23, 'q');
            tower_window(&mut cv, 266, 28, t);
            for x in [10 * TILE, 15 * TILE] {
                table(
                    &mut cv,
                    x,
                    if x == 10 * TILE { 3 * TILE } else { 9 * TILE },
                    4 * TILE,
                    16,
                );
            }
            let slide = if state.bookshelf { 36 } else { 0 };
            shelf(&mut cv, 3 * TILE - slide, 4 * TILE, 4 * TILE, 22, 4);
            if state.bookshelf {
                cv.rect(3 * TILE, 4 * TILE, 4 * TILE, 1, 'u');
            }
            map(&mut cv, 10 * TILE + 6, 3 * TILE + 3, 48, 24);
            tower_window(&mut cv, 148, 95, t);
            book(&mut cv, 15 * TILE + 10, 9 * TILE + 3);
            candle(&mut cv, 286, 151, t);
            table(&mut cv, 49, 155, 64, 8);
            candle(&mut cv, 76, 155, t);
            stair(&mut cv, 19 * TILE, 3 * TILE, true);
        }
        RoomKind::TowerStairs => {
            tower_window(&mut cv, 151, 25, t);
            banner(&mut cv, 256, 38, 'e');
            // The spiral's central well, guarded by a stone balustrade.
            cv.ellipse(184.0, 106.0, 28.0, 31.0, 's');
            cv.ellipse(184.0, 106.0, 22.0, 25.0, 'k');
            for i in 0..13 {
                let a = i as f32 * 0.45;
                let x = 184.0 + a.cos() * 24.0;
                let y = 106.0 + a.sin() * 27.0;
                cv.ellipse(x, y, 5.0, 3.0, 'h');
            }
            shelf(&mut cv, 3 * TILE, 3 * TILE, 4 * TILE, 2 * TILE, 23);
            stair(&mut cv, 19 * TILE, 3 * TILE, true);
            stair(&mut cv, 4 * TILE, 12 * TILE, false);
            cv.rect(5 * TILE, 6 * TILE, 22, 12, 'u');
            cv.frame(5 * TILE, 6 * TILE, 22, 12, 'h');
            tower_window(&mut cv, 266, 116, t);
            table(&mut cv, 16 * TILE, 9 * TILE, 4 * TILE, 16);
            candle(&mut cv, 290, 146, t);
            book(&mut cv, 266, 148);
            for (x, y) in [(136, 80), (231, 133)] {
                candle(&mut cv, x, y, t);
            }
        }
        RoomKind::TowerStars => {
            cv.ellipse(184.0, 125.0, 92.0, 48.0, 's');
            cv.ellipse(184.0, 125.0, 86.0, 43.0, 'q');
            for x in [66, 184, 266] {
                tower_window(&mut cv, x, 18, t);
            }
            table(&mut cv, 10 * TILE, 3 * TILE, 4 * TILE, 2 * TILE);
            map(&mut cv, 10 * TILE + 7, 3 * TILE + 4, 50, 24);
            shelf(&mut cv, 3 * TILE, 3 * TILE, 3 * TILE, 28, 94);
            candle(&mut cv, 104, 86, t);
            candle(&mut cv, 242, 154, t);
            for i in 0..7 {
                let x = 112 + i * 15;
                let y = 111 + ((i * i * 7) % 27);
                cv.put(x, y, 'V');
                if (t / 8 + i as u32) % 4 == 0 {
                    cv.line(x - 2, y, x + 2, y, 'i');
                    cv.line(x, y - 2, x, y + 2, 'i');
                }
                if i < 6 {
                    cv.line(
                        x + 2,
                        y,
                        112 + (i + 1) * 15,
                        111 + (((i + 1) * (i + 1) * 7) % 27),
                        'u',
                    );
                }
            }
            // Telescope on three brass legs.
            cv.line(287, 131, 272, 154, 'O');
            cv.line(287, 131, 303, 154, 'O');
            cv.line(287, 131, 287, 156, 't');
            cv.line(267, 109, 304, 85, 'u');
            cv.line(267, 110, 304, 86, 'V');
            cv.line(267, 111, 304, 87, 'i');
            cv.ellipse(305.0, 86.0, 5.0, 8.0, 'O');
            stair(&mut cv, 19 * TILE, 3 * TILE, true);
            stair(&mut cv, 4 * TILE, 12 * TILE, false);
        }
        RoomKind::WizardStudy => {
            rug(&mut cv, 132, 74, 115, 110, 'q');
            tower_window(&mut cv, 145, 26, t);
            tower_window(&mut cv, 238, 26, t);
            hearth(&mut cv, 263, 91, t);
            shelf(&mut cv, 3 * TILE, 2 * TILE, 5 * TILE, 2 * TILE, 81);
            shelf(&mut cv, 16 * TILE, 2 * TILE, 5 * TILE, 2 * TILE, 42);
            table(&mut cv, 10 * TILE, 3 * TILE, 4 * TILE, 2 * TILE);
            map(&mut cv, 10 * TILE + 5, 3 * TILE + 3, 45, 23);
            candle(&mut cv, 215, 56, t);
            for (i, x) in [264, 286, 308].into_iter().enumerate() {
                jar(&mut cv, x, 63, ['e', 'u', 'i'][i], t);
            }
            // Three pinned creature sketches, recognisable at room scale.
            for i in 0..3 {
                let x = 50 + i * 23;
                cv.rect(x, 86, 19, 22, 'T');
                cv.frame(x, 86, 19, 22, 'O');
                match i {
                    0 => {
                        cv.line(x + 3, 95, x + 9, 100, 's');
                        cv.line(x + 9, 100, x + 16, 95, 's');
                        cv.rect(x + 8, 96, 3, 6, 's');
                    }
                    1 => {
                        cv.rect(x + 6, 90, 7, 6, 'h');
                        cv.line(x + 9, 95, x + 9, 104, 's');
                        cv.line(x + 4, 99, x + 14, 99, 's');
                    }
                    _ => {
                        cv.ellipse(x as f32 + 9.0, 99.0, 6.0, 4.0, 'e');
                        cv.put(x + 7, 97, 's');
                        cv.put(x + 11, 97, 's');
                    }
                }
            }
            table(&mut cv, 5 * TILE, 8 * TILE, 2 * TILE, 16);
            book(&mut cv, 5 * TILE + 7, 8 * TILE - 2);
            cv.ellipse(302.0, 153.0, 28.0, 16.0, 'S');
            cv.ellipse(302.0, 153.0, 23.0, 12.0, 'u');
            cv.frame(279, 141, 46, 24, 'P');
            book(&mut cv, 218, 139);
            candle(&mut cv, 146, 168, t);
        }
        _ => {}
    }
    for x in [24, cv.w - 25] {
        for y in [37, cv.h - 32] {
            candle(&mut cv, x, y, t + x as u32);
        }
    }
    sign(&mut cv, 192, 3, c::label(kind));
    cv
}

pub fn building(tick: u32) -> Img {
    let mut im = Img::new(112, 55);
    im.rect(7, 23, 98, 32, 'u');
    im.frame(7, 23, 98, 32, 's');
    for y in 23..55 {
        for x in 8..105 {
            if y % 7 == 0 || (x + (y / 7 % 2) * 7) % 14 == 0 {
                im.put(x, y, 'S');
            }
        }
    }
    for y in 0..25 {
        let inset = (24 - y) / 2;
        im.rect(
            inset,
            y,
            112 - inset * 2,
            1,
            if y % 4 == 0 { 'u' } else { 'x' },
        );
    }
    im.line(1, 24, 110, 24, 'i');
    for x in [20, 82] {
        im.rect(x, 30, 10, 14, 's');
        im.line(x + 4, 30, x + 4, 43, 'O');
        im.line(x, 36, x + 9, 36, 'O');
        im.put(x + 2, 33, if tick / 10 % 2 == 0 { 'V' } else { 'u' });
    }
    im.rect(48, 35, 17, 20, 's');
    im.frame(48, 35, 17, 20, 'O');
    im.rect(51, 38, 11, 17, 'k');
    book(&mut im, 48, 10);
    im
}
pub fn tower(tick: u32) -> Img {
    let mut im = Img::new(62, 99);
    im.rect(12, 31, 38, 65, 'u');
    im.rect(12, 31, 6, 65, 'S');
    im.rect(45, 31, 5, 65, 'x');
    for y in 31..96 {
        for x in 12..50 {
            if y % 9 == 0 || (x + (y / 9 % 2) * 8) % 16 == 0 {
                im.put(x, y, 's');
            }
        }
    }
    for y in 0..32 {
        let half = 3 + y * 29 / 32;
        im.rect(
            31 - half,
            y,
            2 * half,
            1,
            if y % 4 == 0 { 'u' } else { 'v' },
        );
    }
    im.line(2, 31, 59, 31, 'i');
    for y in [40, 58, 76] {
        im.rect(27, y, 9, 12, 's');
        im.frame(27, y, 9, 12, 'O');
        im.put(
            30,
            y + 3,
            if (tick / 10 + y as u32) % 3 == 0 {
                'V'
            } else {
                'u'
            },
        );
    }
    im.rect(25, 88, 14, 11, 's');
    im.frame(25, 88, 14, 11, 'O');
    im.put(31, 3, 'T');
    im
}
fn compact(im: &Img) -> Img {
    let mut out = Img::new(im.w * 2 / 3, im.h * 2 / 3);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(rgb) = im.get(x * 3 / 2, y * 3 / 2) {
                out.set(x, y, rgb);
            }
        }
    }
    out
}
pub fn gate_building(cv: &mut Img, t: u32) {
    let im = compact(&building(t));
    cv.stamp(&im, 298, 80 - im.h);
    let im = compact(&tower(t));
    cv.stamp(&im, 328, 80 - im.h);
}
pub fn figures(cv: &mut Img, run: &Run) {
    if c::is_room(run.room().kind) {
        for f in folk::figures(&run.home, run.room().kind, run.dungeon.pack, run.tick) {
            let im = match f.folk {
                folk::Folk::Wizard => wizard(run.tick as u32),
                folk::Folk::Student => {
                    let im = student(f.t);
                    if f.east { im } else { im.flip_h() }
                }
                _ => continue,
            };
            let (x, y) = at(f.x, f.y);
            stand(cv, &im, x, y);
        }
        let state = run.college_progress();
        for (i, &(kind, spot, _, _)) in c::CLUES.iter().enumerate() {
            if kind == run.room().kind && state.clues & (1 << i) != 0 {
                let (x, y) = at(spot.0 * 2.0, spot.1 * 2.0);
                cv.line(x - 3, y - 8, x + 3, y - 8, 'V');
                cv.line(x, y - 11, x, y - 5, 'T');
            }
        }
        for (i, &(kind, spot)) in c::SIGILS.iter().enumerate() {
            if kind != run.room().kind || ![state.bookshelf, state.stars, state.familiar][i] {
                continue;
            }
            let (x, y) = at(spot.0 * 2.0, spot.1 * 2.0);
            let ink = if state.sigils & (1 << i) > 0 {
                'u'
            } else {
                'V'
            };
            cv.line(x - 4, y, x, y - 6, ink);
            cv.line(x, y - 6, x + 4, y, ink);
            cv.line(x + 4, y, x, y + 5, ink);
            cv.line(x, y + 5, x - 4, y, ink);
            cv.put(x, y, 'T');
        }
        if run.room().kind == RoomKind::WizardStudy {
            let (x, y) = at(c::FAMILIAR.0 * 2.0, c::FAMILIAR.1 * 2.0);
            let mut cat = Img::from_rows(&[
                ".rr....rr..",
                ".rRr..rRr..",
                ".rRRRRRRr..",
                "..RYRRYR...",
                "..RRrRRR...",
                "...RRRR..r.",
                "..rRRRRrr..",
                ".rrr..rrr..",
            ]);
            if (run.tick / 45) % 2 == 1 {
                cat.put(3, 3, 'R');
                cat.put(6, 3, 'R');
            }
            let breathe = (run.tick / 20 % 2) as i32;
            cv.stamp(&cat, x - 5, y - 7 - breathe);
            let tail = (run.tick / 12 % 3) as i32;
            cv.line(x + 3, y - 2 - breathe, x + 7, y - 4 - tail, 'r');
        }
    } else if run.dungeon.depth > 0 {
        let c = run.college_progress();
        if let Some(spot) = c::field_spot(run.room()) {
            let active = (c.active("tome")
                && run.dungeon.pack == crate::rules::Pack::Archive
                && run.room().kind == RoomKind::Treasure
                && !c.tome)
                || (c.active("spores")
                    && run.dungeon.pack == crate::rules::Pack::Fungal
                    && c.samples.len() < 3)
                || (c.active("together") && run.dungeon.depth >= 3 && !c.trip);
            if active {
                let (x, y) = at(spot.0 * 2.0, spot.1 * 2.0);
                book(cv, x - 9, y - 7);
                if run.room().cleared {
                    sign(cv, x, y + 9, "E: Field Notes");
                }
            }
        }
    }
}

/// The header and footer live outside the playable floor. No figure or
/// second sign can occupy either band, even when a party crowds a target.
pub fn prompt(cv: &mut Img, run: &Run, focus: Option<u32>, legible: bool) {
    if !c::is_room(run.room().kind) {
        return;
    }
    for y in (0..TILE).chain(cv.h - TILE..cv.h) {
        for x in 0..cv.w {
            cv.clear(x, y);
        }
    }
    if !legible || run.college.page != c::Page::Closed {
        return;
    }
    let focus = focus.filter(|id| run.players.contains_key(id));
    let mut distance = f32::INFINITY;
    let mut label = None;
    // A seat sees its own nearest target. The shared camera sees the
    // nearest target available to any living member of the party.
    for (&id, h) in &run.players {
        if focus.is_some_and(|focus| focus != id) || h.hp == 0 || h.stone || h.privy != 0 {
            continue;
        }
        if let Some((target, d)) = c::nearest(run, h) {
            if d < distance {
                distance = d;
                label = Some(format!("E: {}", target.label()));
            }
        }
        for e in c::DOORS.iter().filter(|e| e.room == run.room().kind) {
            let (x, y, w, height) = e.at;
            let d = (h.x / 2.0 - (x as f32 + w as f32 / 2.0))
                .hypot(h.y / 2.0 - (y as f32 + height as f32 / 2.0));
            if d < 2.5 && d < distance {
                distance = d;
                let destination = if run.returns_to_kingdom(e.id) {
                    "Kingdom"
                } else {
                    e.label
                };
                label = Some(format!("To: {destination}"));
            }
        }
    }
    if let Some(label) = label {
        super::super::chrome::band(cv, (0, cv.h - TILE, cv.w, TILE), super::super::chrome::rim_of('P')[1]);
        cv.text(12, cv.h - 11, &label, 'i');
    }
}
fn wrapped(line: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut row = String::new();
    for word in line.split_whitespace() {
        if row.len() + word.len() + 1 > max && !row.is_empty() {
            out.push(row);
            row = String::new();
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
pub fn dialogue(cv: &mut Img, run: &Run) {
    use c::Page;
    if run.college.page == Page::Closed {
        return;
    }
    let state = run.college_progress();
    let selected = run.college.selected as usize;
    let (title, lines, hint) = match run.college.page {
        Page::Menu => (
            "Bartosz the Wizard",
            vec![
                "Ask about the Labyrinth".into(),
                "Research quests".into(),
                "Herald's Bestiary".into(),
                "Goodbye".into(),
            ],
            "UP/DOWN: choose  E: talk",
        ),
        Page::Topics => (
            "Ask about the Labyrinth",
            c::TOPICS.iter().map(|(s, _)| s.to_string()).collect(),
            "UP/DOWN: choose | E: read | LEFT: back",
        ),
        Page::Topic(i) => {
            let (t, lines) = c::TOPICS[usize::from(i)];
            (t, lines.map(String::from).to_vec(), "E: topics  LEFT: back")
        }
        Page::Quests => (
            "Research quests",
            c::QUESTS
                .iter()
                .map(|q| {
                    format!(
                        "{}{}",
                        q.title,
                        if run.home.college.done.contains(q.id) {
                            " [done]"
                        } else if state.done.contains(q.id) {
                            " [pending]"
                        } else if state.active(q.id) {
                            " [active]"
                        } else {
                            ""
                        }
                    )
                })
                .collect(),
            "UP/DOWN: choose | E: read | LEFT: back",
        ),
        Page::Quest(i) => {
            let q = &c::QUESTS[usize::from(i)];
            let mut lines = q.asks.map(String::from).to_vec();
            lines.push(format!("Reward: {}", c::reward(q)));
            lines.push(format!("Notes: {}/{}", state.have(q), q.need));
            let hint = if run.home.college.done.contains(q.id) {
                "Paid. E: quests  LEFT: back"
            } else if state.done.contains(q.id) {
                "Hand-in pending save. LEFT: back"
            } else if !state.active(q.id) {
                "E: accept  LEFT: back"
            } else if state.have(q) >= q.need {
                "E: hand in  LEFT: back"
            } else {
                "E: quests  LEFT: back"
            };
            (q.title, lines, hint)
        }
        Page::Bestiary => {
            let lines = vec![
                "The Herald records creatures felled.".into(),
                format!(
                    "Bat: {}  Skeleton: {}  Slime: {}",
                    run.home.bestiary.get("Bat").unwrap_or(&0),
                    run.home.bestiary.get("Skeleton").unwrap_or(&0),
                    run.home.bestiary.get("Slime").unwrap_or(&0)
                ),
                format!("Our living observations: {}/3", state.observed.len()),
                "Observe with E. A kill is a different record.".into(),
            ];
            ("Herald's Bestiary", lines, "E: back")
        }
        _ => (
            "Bartosz the Wizard",
            vec!["Bring a question. Leave with two.".into()],
            "E: back",
        ),
    };
    let menu = matches!(run.college.page, Page::Menu | Page::Topics | Page::Quests);
    let rows: Vec<(String, bool)> = lines
        .iter()
        .enumerate()
        .flat_map(|(i, s)| {
            wrapped(s, 46)
                .into_iter()
                .map(move |s| (s, menu && selected == i))
        })
        .collect();
    let h = (rows.len() as i32 * 12 + 43).max(101);
    let y = (cv.h - h - 7).max(2);
    super::super::chrome::panel(cv, (6, y, cv.w - 12, h), super::super::chrome::Tone::FRAMED);
    super::super::chrome::ring(cv, (6, y, cv.w - 12, h), 4);
    let portrait = wizard(run.tick as u32);
    // The room sprite is larger; the portrait still fits beside the text
    // and above the dialogue's controls at its shortest panel height.
    for yy in 0..31 {
        for xx in 0..26 {
            if let Some(rgb) = portrait.get(xx * portrait.w / 26, yy * portrait.h / 31) {
                if rgb != super::super::ink::BLACK {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            cv.set(15 + xx * 2 + dx, y + 22 + yy * 2 + dy, rgb);
                        }
                    }
                }
            }
        }
    }
    cv.text(16, y + 9, title, 'H');
    for (i, (s, selected)) in rows.into_iter().enumerate() {
        let yy = y + 26 + i as i32 * 12;
        if selected {
            super::super::chrome::highlight(cv, (71, yy - 2, cv.w - 87, 11));
            cv.text(73, yy, ">", 'T');
        }
        cv.text(83, yy, &s, if selected { 'T' } else { 'i' });
    }
    cv.text(16, y + h - 15, hint, 'O');
}
#[cfg(test)]
#[path = "../../../../tests/render/overworld__college_shots.rs"]
mod tests;
