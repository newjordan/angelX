//! Rescues, drawn: a cage in a treasure room with someone in it (its lock
//! gold until it's opened, then the door hanging wide and its prisoner
//! gone home), and the people who live in the Undercroft now, each at their
//! own spot.

use super::super::ink::Img;
use super::{at, stand};
use crate::rules::Run;
use crate::rules::rescues::RESIDENTS;

pub(super) fn cage(cv: &mut Img, run: &Run) {
    let Some(captive) = run.captive.as_ref().filter(|c| c.room == run.at) else {
        return;
    };
    if captive.who == crate::rules::swamp::HORSE {
        super::swamp::pen(cv, run, captive);
        return;
    }
    let (x, y) = at(captive.x, captive.y);
    let gone = captive
        .freed
        .is_some_and(|at| run.tick.saturating_sub(at) > 45);
    if !gone {
        let im = Img::from_rows(sprite(&captive.who));
        let dx = captive
            .freed
            .map_or(0, |at| (run.tick.saturating_sub(at) as i32 / 3).min(14));
        stand(cv, &im, x + dx, y);
    }
    let bars = if captive.freed.is_some() {
        &OPEN
    } else {
        &CAGE
    };
    let im = Img::from_rows(bars);
    cv.stamp(&im, x - im.w / 2, y - im.h + 3);
}

pub(super) fn residents(cv: &mut Img, run: &Run) {
    let dug = run
        .dungeon
        .rooms
        .iter()
        .any(|r| r.kind == crate::rules::RoomKind::Tavern);
    for resident in RESIDENTS
        .iter()
        .filter(|r| run.home.residents.contains(r.id))
        .filter(|r| !(dug && r.id == "maud"))
    {
        let (x, y) = at(resident.home_at.0 * 2.0, resident.home_at.1 * 2.0);
        stand(cv, &Img::from_rows(sprite(resident.id)), x, y);
    }
}

pub(super) fn sprite_of(who: &str) -> Img {
    Img::from_rows(sprite(who))
}

fn sprite(who: &str) -> &'static [&'static str] {
    match who {
        "mabel" => &MABEL,
        "anselm" => &ANSELM,
        "pip" => &PIP,
        _ => &MAUD,
    }
}

const MABEL: [&str; 16] = [
    "....WWWWW....",
    "...WVVVVVW...",
    "...WWWWWWW...",
    "....OOOOO....",
    "...OKOOOKO...",
    "...OROOORO...",
    "....OOOOO....",
    "...rrrrrrr...",
    "..rrcccccrr..",
    ".OrrcccccrrO.",
    ".O.rcccccr.Oh",
    "...rcccccr..h",
    "...rrrrrrr...",
    "....rr.rr....",
    "....rr.rr....",
    "...KKK.KKK...",
];

const ANSELM: [&str; 16] = [
    "....OOOOO....",
    "...bOOOOOb...",
    "...bOOOOOb...",
    "...OKOOOKO...",
    "....OOOOO....",
    "....POOOP....",
    "...PPPPPPP...",
    "..PPPPPPPPP..",
    ".OPPPPPPPPPO.",
    ".O.PPTTTPP.O.",
    "...PPPPPPP...",
    "...PPPPPPP...",
    "..PPPPPPPPP..",
    "..PPPPPPPPP..",
    "...nn...nn...",
    "..nnn...nnn..",
];

const PIP: [&str; 16] = [
    "...RRRRR.....",
    "..RRRRRRR....",
    "..ROOOOOR....",
    "..OKOOKOO....",
    "..OOkOOOO....",
    "...OOOOO.....",
    "...QQQQQ.....",
    "..QzzzzzQ.cc.",
    ".OQzzzzzQOcc.",
    ".O.zzzzz..c..",
    "...zzzzz.....",
    "...QQQQQ.....",
    "...bb.bb.....",
    "...bb.bb.....",
    "..KKK.KKK....",
    ".............",
];

const MAUD: [&str; 16] = [
    "....YYYYY....",
    "...YYYYYYY...",
    "...YOOOOOY...",
    "...OKOOOKO...",
    "....OOOOO....",
    "...EEEEEEE...",
    "..EllllllE...",
    ".OEllllllEOWW",
    ".O.EllllE.OJJ",
    "...EllllE..JJ",
    "..EllllllE...",
    "..EllllllE...",
    ".EllllllllE..",
    ".EEEEEEEEEE..",
    "...nn...nn...",
    "..nnn...nnn..",
];

const CAGE: [&str; 20] = [
    "hJJJJJJJJJJJJJJJJh",
    "JhhhhhhhhhhhhhhhhJ",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J55J..J..J",
    "J.J..J..J55J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "J.J..J..J..J..J..J",
    "JhhhhhhhhhhhhhhhhJ",
    "gJJJJJJJJJJJJJJJJg",
];

const OPEN: [&str; 20] = [
    "hJJJJJJJJJJJJJJJJh",
    "JhhhhhhhhhhhhhhhhJ",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "J.J..J..J........J",
    "JhhhhhhhhhhhhhhhhJ",
    "gJJJJJJJJJJJJJJJJg",
];
