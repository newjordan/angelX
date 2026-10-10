//! Beaumains, drawn: a kitchen boy in a mail shirt under a white apron,
//! auburn hair, a carving knife in his hand. In the tavern he waits by the
//! west table with his plate before him; in a delve he fights beside the
//! party, and when he's felled he lies where he fell until the fight's over.

use super::super::ink::Img;
use super::{TILE, at, home, sprites, stand};
use crate::drive::together_shooter::Run;
use crate::drive::together_shooter::hireling::{BEAUMAINS_AT, HIRE_PLATE, WAGE, hire_at};
use crate::drive::together_shooter::home::{BUY_HOLD, Station};

pub(super) const BEAUMAINS: [&str; 17] = [
    "....rRRr......",
    "...rRrRRr.....",
    "..rROOOORr....",
    "..rOKOOKOr....",
    "...OOOOOO.....",
    "....OrrO......",
    "...jGGGGj.....",
    "..jGiiiiGj.i..",
    "..jGiiiiGj.i..",
    ".OjGiiiiGjOB..",
    ".O.GiiiiG..B..",
    "...bbbbbb.....",
    "...iiiiii.....",
    "...iiiiii.....",
    "....II.II.....",
    "....II.II.....",
    "...nnn.nnn....",
];

/// Beaumains by the west table (until he's hired: then he's waiting at the
/// stair), and his plate lit under a knight.
pub(super) fn tavern(cv: &mut Img, run: &Run) {
    if run.home.hire.is_none() {
        let (x, y) = at(BEAUMAINS_AT.0 * 2.0, BEAUMAINS_AT.1 * 2.0);
        stand(cv, &Img::from_rows(&BEAUMAINS), x, y);
    }
    if run.players.values().any(|h| h.hp > 0 && hire_at(h.x, h.y)) {
        home::plate(cv, HIRE_PLATE, true, Station::Hire);
    }
}

/// To a knight on his plate: his terms, and the hold.
pub(super) fn board(cv: &mut Img, run: &Run) {
    let Some(hero) = run.players.values().find(|h| h.hp > 0 && hire_at(h.x, h.y)) else {
        return;
    };
    let lines = if run.home.hire.is_some() {
        vec![
            ("BEAUMAINS".to_string(), '9'),
            ("HIRED FOR THE NEXT DELVE".to_string(), 'h'),
            ("HE'LL BE AT THE STAIR".to_string(), '5'),
        ]
    } else {
        let filled = hero.buying * 10 / BUY_HOLD;
        vec![
            ("BEAUMAINS".to_string(), '9'),
            ("THE KITCHEN KNIGHT".to_string(), 'h'),
            (format!("{WAGE} GOLD FOR ONE DELVE"), '5'),
            (
                format!("HOLD F {}", "#".repeat(filled.min(10) as usize)),
                'O',
            ),
        ]
    };
    let board = home::board(&lines);
    let (c, r, w, h) = HIRE_PLATE;
    let bx = ((c * TILE + w * TILE / 2) - board.w / 2).clamp(2, cv.w - board.w - 2);
    cv.stamp(&board, bx, ((r + h) * TILE + 4).min(cv.h - board.h - 2));
}

/// Hired, Beaumains waits by the Winding Stair with his knife.
pub(super) fn at_the_stair(cv: &mut Img, run: &Run) {
    if run.home.hire.is_none() {
        return;
    }
    let (cx, cy) = home::stair_centre();
    stand(cv, &Img::from_rows(&BEAUMAINS), cx - 44, cy + 30);
}

/// His plate before the west table, unlit.
pub(super) fn plate(cv: &mut Img) {
    home::plate(cv, HIRE_PLATE, false, Station::Hire);
}

/// Beaumains in a delve: facing what he fights, blinking through a hit's
/// grace, or lying where he fell.
pub(super) fn delve(cv: &mut Img, run: &Run) {
    let Some(hire) = &run.hireling else {
        return;
    };
    let (x, y) = at(hire.x, hire.y);
    let mut im = Img::from_rows(&BEAUMAINS);
    if hire.down() {
        im = sprites::quarter_turn(&im);
        stand(cv, &im, x, y + 3);
        return;
    }
    if hire.facing < 0.0 {
        im = im.flip_h();
    }
    let blink = hire.invulnerable > 0 && (run.tick / 2).is_multiple_of(2);
    if !blink {
        stand(cv, &im, x, y);
    }
}
