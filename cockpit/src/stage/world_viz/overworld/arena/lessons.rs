//! Sir Ector's corner of the Training Yard, drawn: two lecterns with their
//! books open, a plate before each, Sir Ector between them with his staff;
//! and, to a knight standing at a lectern, the lesson it teaches.

use super::super::ink::Img;
use super::{TILE, at, home, stand};
use crate::drive::together_shooter::Run;
use crate::drive::together_shooter::home::{BUY_HOLD, numeral};
use crate::drive::together_shooter::talents::{ECTOR_AT, LECTERNS, LEVELS, knight_key, lectern_at};

/// The lecterns and their plates, as the yard's scenery has them.
pub(super) fn furniture(cv: &mut Img) {
    let stand_im = Img::from_rows(&LECTERN);
    for (i, &plate) in LECTERNS.iter().enumerate() {
        let (c, r, w, _) = plate;
        let cx = c * TILE + w * TILE / 2;
        cv.stamp(&stand_im, cx - stand_im.w / 2, r * TILE - stand_im.h);
        slate(cv, plate, false, i);
    }
}

/// A lectern's plate: dark, its numeral's dots in brass, gold when stood on.
fn slate(cv: &mut Img, (c, r, w, h): (i32, i32, i32, i32), lit: bool, side: usize) {
    let (x, y, pw, ph) = (c * TILE + 1, r * TILE + 1, w * TILE - 2, h * TILE - 2);
    cv.rect(x, y, pw, ph, 'X');
    cv.frame(x, y, pw, ph, if lit { '5' } else { 'o' });
    let dot = if lit { '6' } else { 'O' };
    for k in 0..=side as i32 {
        cv.rect(
            x + pw / 2 - 2 * side as i32 + 4 * k - 1,
            y + ph / 2 - 1,
            2,
            3,
            dot,
        );
    }
}

/// Sir Ector, and the plates knights stand on.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let (x, y) = at(ECTOR_AT.0 * 2.0, ECTOR_AT.1 * 2.0);
    stand(cv, &Img::from_rows(&ECTOR), x, y);
    for hero in run.players.values().filter(|h| h.hp > 0) {
        if let Some(side) = lectern_at(hero.x, hero.y) {
            slate(cv, LECTERNS[side], true, side);
        }
    }
}

/// The lesson a knight standing at a lectern reads: its talent, what it
/// does, and whether it is theirs to learn yet.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let Some((hero, side)) = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .find_map(|h| Some((h, lectern_at(h.x, h.y)?)))
    else {
        return;
    };
    let prowess = run.home.prowess(&knight_key(hero));
    let mut lines = vec![(
        format!("{}, LEVEL {}", hero.name.to_uppercase(), prowess.level()),
        'h',
    )];
    let ready = prowess.waiting() > 0;
    match prowess.next() {
        None => lines.push(("ALL FOUR LESSONS LEARNED".to_string(), '9')),
        Some(lesson) => {
            let talent = &lesson[side];
            let n = prowess.learned.len();
            lines.push((
                format!(
                    "LESSON {}: {}",
                    numeral(n as u8 + 1),
                    talent.name.to_uppercase()
                ),
                '9',
            ));
            lines.push((talent.does.to_uppercase(), 'H'));
            lines.push(if ready {
                ("HOLD F TO LEARN".to_string(), 'O')
            } else {
                (
                    format!(
                        "AT LEVEL {} ({} XP)",
                        n + 2,
                        LEVELS[n.min(LEVELS.len() - 1)]
                    ),
                    'h',
                )
            });
        }
    }
    let holding = ready && prowess.next().is_some();
    if holding {
        // A row for the hold's bar.
        lines.push((String::new(), 'h'));
    }
    let mut board = home::board(&lines);
    if holding {
        // The hold, filling under its words.
        let bar_w = 40;
        let (bx, by) = ((board.w - bar_w) / 2, board.h - 8);
        board.frame(bx, by - 1, bar_w, 3, 'b');
        let fill = (hero.buying.min(BUY_HOLD) * (bar_w as u32 - 2) / BUY_HOLD) as i32;
        if fill > 0 {
            board.rect(bx + 1, by, fill, 1, '5');
        }
    }
    let (c, r, w, _) = LECTERNS[side];
    let x = (c * TILE + w * TILE / 2 - board.w / 2).clamp(2, cv.w - board.w - 2);
    let y = (r * TILE - 34 - board.h).max(2);
    cv.stamp(&board, x, y);
}

/// A lectern: a book open on a stand.
const LECTERN: [&str; 9] = [
    ".TTTTTTTT.",
    "TccccTcccT",
    "TcJJcTcJcT",
    "TccccTcccT",
    ".TTTBBTTT.",
    "....BB....",
    "....BB....",
    "...bBBb...",
    "..bbbbbb..",
];

/// Sir Ector: white-bearded, in mail and leather, his staff beside him.
pub(super) const ECTOR: [&str; 17] = [
    "....HHHH.......",
    "...HiiiiH......",
    "...HOOOOH......",
    "...KOOOOK......",
    "...OKOOKO......",
    "...HHOOHH......",
    "....HHHH....t..",
    "...JHHHHJ...B..",
    "..JhhHHhhJ..B..",
    ".OJhhhhhhJO.B..",
    ".OJhPPPPhJOBB..",
    "..JhPPPPhJ..B..",
    "..JbbbbbbJ..B..",
    "...hhh.hhh..B..",
    "...hhh.hhh..B..",
    "...nnn.nnn..b..",
    "..Knnn.nnnK....",
];
