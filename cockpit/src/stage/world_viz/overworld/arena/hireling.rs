//! Beaumains, drawn: a kitchen boy in a mail shirt under a white apron,
//! auburn hair, a carving knife in his hand. In the tavern he waits by the
//! west table with his plate before him; in a delve he fights beside the
//! party, and when he's felled he lies where he fell until the fight's over.
//! A minion a card calls into his place is drawn the same way, each its own
//! small creature (its card shows the thing that called it).

use super::super::ink::Img;
use super::{TILE, at, home, sprites, stand};
use crate::drive::together_shooter::Run;
use crate::drive::together_shooter::hireling::{AllyKind, BEAUMAINS_AT, HIRE_PLATE, WAGE, hire_at};
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

/// A hearth brownie: a small shaggy hearth spirit in brown and ochre,
/// bare-footed, no knife. It catches a shot instead.
pub(super) const BROWNIE: [&str; 12] = [
    "...pBBBp...",
    "..pBRrRBp..",
    ".pBRrRrRBp.",
    ".BRtOOOtRB.",
    ".BOnOtOnOB.",
    "..OOoooOO..",
    "..BrrrrrB..",
    ".oBrRRRrBo.",
    ".o.rrRrr.o.",
    "...BIIIB...",
    "...o...o...",
    "..oo...oo..",
];

/// A rime moth, from above: frost-pale wings with blue eye-spots on a
/// slate body. Still, so an idle moth does not repaint the room.
pub(super) const MOTH: [&str; 10] = [
    "...u.....u...",
    "....u...u....",
    ".vVVv.S.vVVv.",
    "vVWVVvSvVVWVv",
    "vVzVVUSUVVzVv",
    ".vVVvUSUvVVv.",
    "..vvUuSuUvv..",
    ".vVVu.S.uVVv.",
    ".vVv..S..vVv.",
    "..v.......v..",
];

/// A salt wisp: a pale drifting curl with two dim eyes, its tail
/// trailing off in grains.
pub(super) const WISP: [&str; 12] = [
    "....hh.....",
    "...hii.....",
    "..hiHii....",
    ".hiHVHii...",
    ".igHHHgih..",
    ".hiHHHHii..",
    "..iiHHHih..",
    "...hiiHii..",
    ".....hiih..",
    "......hih..",
    "....h..hi..",
    ".....hhh...",
];

/// A glass mite, from above: a faceted cyan body on six thin legs.
pub(super) const MITE: [&str; 10] = [
    "...J.....J...",
    "....J.0.J....",
    "..J..030..J..",
    "...J0U320.J..",
    "J..0U3U20..J.",
    ".JU02U2U0UJ..",
    "...0U2U2U0...",
    ".J.0UU2U0.J..",
    "J...0000...J.",
    ".............",
];

/// An ash sprite: a sooty puff of charcoal with ember eyes, ember
/// cracks, and a thread of smoke going up.
pub(super) const ASH_SPRITE: [&str; 12] = [
    ".....j.....",
    "....j.g....",
    ".....gj....",
    "...jXgXj...",
    "..jXXXXXg..",
    ".jX@XXX@Xg.",
    ".gXXXaXXXg.",
    ".gXa7@7aXg.",
    "..gXXaXXg..",
    ".a.gXXXg.a.",
    "....g.g....",
    "...gg.gg...",
];

/// A lantern mote: a warm spark with a soft ring of light. Still, so
/// an idle mote does not repaint the room.
pub(super) const MOTE: [&str; 9] = [
    "....a....",
    "..a.o.a..",
    "...o4o...",
    ".ao464oa.",
    "ao46564oa",
    ".ao464oa.",
    "...o4o...",
    "..a.o.a..",
    "....a....",
];

/// A reed newt, from above: moss-green, a pale stripe down its back,
/// four splayed feet and a long tail.
pub(super) const NEWT: [&str; 9] = [
    "...E.......E....",
    "...EE.....EE....",
    "....Em....mE....",
    ".....mmmmmmmmmK.",
    "llmmmAAyyyyAAAAm",
    ".....mmmmmmmmmK.",
    "....Em....mE....",
    "...EE.....EE....",
    "...E.......E....",
];

/// A linnet: a small warm-brown bird with a red breast and crown, a
/// short dark beak.
pub(super) const LINNET: [&str; 9] = [
    "......BB7B..",
    ".....BRR77B.",
    ".....BRnRRBj",
    "....BBRRR77.",
    "..BBBRRB777.",
    ".bBBBBRB77..",
    "bb.BBBRRtt..",
    "b.....n.n...",
    ".....nn.nn..",
];

/// A leech, from above: fat and dark red, ridged in segments, a wet
/// sheen along its back and its sucker at the front.
pub(super) const LEECH: [&str; 6] = [
    "...8888888....",
    ".88p7p7p7pp8..",
    "8pp8p8p8p8pp87",
    "8pp8p8p8p8pp8.",
    ".88p8p8p8pp8..",
    "...8888888....",
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
    let mut im = Img::from_rows(match hire.kind {
        AllyKind::Beaumains => &BEAUMAINS[..],
        AllyKind::Brownie => &BROWNIE[..],
        AllyKind::Moth => &MOTH[..],
        AllyKind::Wisp => &WISP[..],
        AllyKind::Mite => &MITE[..],
        AllyKind::AshSprite => &ASH_SPRITE[..],
        AllyKind::Mote => &MOTE[..],
        AllyKind::Newt => &NEWT[..],
        AllyKind::Linnet => &LINNET[..],
        AllyKind::Leech => &LEECH[..],
    });
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
