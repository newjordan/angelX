//! The marks round the table that are pictures: each seat's card in the race
//! column (keeper's health up its edge, three lamps, the leader's crown), the
//! keeper's pods, the three lamps, and the bell with the bar that ticks to it.
//! The words on them are the plan's (`claims`); nothing here shakes.

use super::super::super::ink::{Img, bayer, ink};
use super::super::{juice, knights};
use super::claims::{self, Plan};
use crate::rules::HZ;
use crate::rules::mossbell::{LENGTH, Session};
use crate::rules::mossbell_show::POP_LIFE;
use std::sync::OnceLock;

/// A seed pod, whole and cracked, a hollow and a flash: eight by eight.
fn pods() -> &'static [Img; 4] {
    static PODS: OnceLock<[Img; 4]> = OnceLock::new();
    PODS.get_or_init(|| {
        let whole = Img::from_rows(&[
            "..FFFF..",
            ".FLLYLF.",
            "FLLYLLLF",
            "FLLLLLEF",
            "FLLLLEEF",
            "FELLEEEF",
            ".FEEEEF.",
            "..FFFF..",
        ]);
        let hurt = Img::from_rows(&[
            "..FFFF..",
            ".FLLYLF.",
            "FLLYLLLF",
            "FLLkLLEF",
            "FLLkkEEF",
            "FELLkEEF",
            ".FEEEEF.",
            "..FFFF..",
        ]);
        let hollow = Img::from_rows(&[
            "..ssss..",
            ".s....s.",
            "s......s",
            "s......s",
            "s......s",
            "s......s",
            ".s....s.",
            "..ssss..",
        ]);
        let flash = Img::from_rows(&[
            "..HHHH..",
            ".HHHHHH.",
            "HHHHHHHH",
            "HHHHHHHH",
            "HHHHHHHH",
            "HHHHHHHH",
            ".HHHHHH.",
            "..HHHH..",
        ]);
        [whole, hurt, hollow, flash]
    })
}

/// A lamp, unlit, lit, bright and hot: twelve by twelve.
fn lamp_set() -> &'static [Img; 4] {
    static LAMPS: OnceLock<[Img; 4]> = OnceLock::new();
    LAMPS.get_or_init(|| {
        let dark = Img::from_rows(&[
            "...gggggg...",
            "..gjjjjjjg..",
            ".gjkkkkkkjg.",
            "gjkkkkkkkkjg",
            "gjkkkkkkkkjg",
            "gjkkkkkkkkjg",
            "gjkkkkkkkkjg",
            "gjkkkkkkkkjg",
            "gjkkkkkkkkjg",
            ".gjkkkkkkjg.",
            "..gjjjjjjg..",
            "...gggggg...",
        ]);
        let lit = Img::from_rows(&[
            "...BBBBBB...",
            "..BooooooB..",
            ".BooottttooB",
            "BoottttttooB",
            "BootttTTtttB",
            "BoottTTTTttB",
            "BoottTTTTttB",
            "BootttTTtttB",
            "BooottttoooB",
            ".BooooooooB.",
            "..BooooooB..",
            "...BBBBBB...",
        ]);
        let bright = Img::from_rows(&[
            "...pppppp...",
            "..p@@@@@@p..",
            ".p@@6666@@p.",
            "p@@66TT66@@p",
            "p@66TTTT66@p",
            "p@6TTHHTT6@p",
            "p@6TTHHTT6@p",
            "p@66TTTT66@p",
            "p@@66TT66@@p",
            ".p@@6666@@p.",
            "..p@@@@@@p..",
            "...pppppp...",
        ]);
        let hot = Img::from_rows(&[
            "...RRRRRR...",
            "..R@@@@@@R..",
            ".R@@6666@@R.",
            "R@@66HH66@@R",
            "R@66HHHH66@R",
            "R@6HHHHHH6@R",
            "R@6HHHHHH6@R",
            "R@66HHHH66@R",
            "R@@66HH66@@R",
            ".R@@6666@@R.",
            "..R@@@@@@R..",
            "...RRRRRR...",
        ]);
        [dark, lit, bright, hot]
    })
}

/// A little lamp pip for a card, unlit and lit, and the leader's three-point crown.
fn pips() -> &'static [Img; 3] {
    static PIPS: OnceLock<[Img; 3]> = OnceLock::new();
    PIPS.get_or_init(|| {
        [
            Img::from_rows(&[".gg.", "gkkg", "gkkg", ".gg."]),
            Img::from_rows(&[".oo.", "otto", "otto", ".oo."]),
            Img::from_rows(&["T.T.T", "TTTTT", ".ooo."]),
        ]
    })
}

/// The brass bell, still and swung either way: fourteen by fourteen.
fn bells() -> &'static [Img; 3] {
    static BELLS: OnceLock<[Img; 3]> = OnceLock::new();
    BELLS.get_or_init(|| [bell(0), bell(1), bell(-1)])
}

fn bell(swing: i32) -> Img {
    let rows = [
        "......oo......",
        ".....tttt.....",
        "....tTTttt....",
        "...tTTtttot...",
        "...tTttttoo...",
        "..tTtttttooo..",
        "..tTtttttooo..",
        ".tTttttttoooo.",
        ".tTttttttoooo.",
        "tTtttttttooooo",
        "oooooooooooooo",
        "BBBBBBBBBBBBBB",
        "......oo......",
        ".......o......",
    ];
    let still = Img::from_rows(&rows);
    if swing == 0 {
        return still;
    }
    // A swing shears the lower rows a pixel either way.
    let mut out = Img::new(still.w + 2, still.h);
    for y in 0..still.h {
        let shift = 1 + swing * (y * 2 / still.h);
        for x in 0..still.w {
            if let Some(c) = still.get(x, y) {
                out.set(x + shift, y, c);
            }
        }
    }
    out
}

/// The seat's mark in its cloth: the bar up a card's edge.
fn cloth(id: u32) -> char {
    knights::seat_mark(id)
}

/// The race column's pictures: a card for each knight in the order of the
/// race, the keeper's health up its edge, the three lamps, the leader crowned,
/// and a pulse under the viewer's card when their score has just grown.
pub(super) fn cards(im: &mut Img, run: &crate::rules::Run, s: &Session, me: u32, now: u32) {
    let rows = claims::order(s);
    let n = rows.len();
    let rh = claims::card_height(n);
    let gain = s.show.pops.iter().filter(|p| u32::from(p.who) == me).map(|p| p.age(now)).min();
    for (k, &id) in rows.iter().enumerate() {
        let y0 = 52 + k as i32 * rh;
        let b = &s.tables[&id].ball;
        if id == me {
            let lift = gain.map_or(0.0, |a| juice::bump(a, 10));
            let tint = ink('S').unwrap();
            for y in y0 - 1..y0 + rh - 2 {
                for x in 4..56 {
                    if bayer(x, y) < 0.1 + lift * 0.3 {
                        im.set(x, y, tint);
                    }
                }
            }
        }
        // The keeper's health, up the card's edge in the seat's cloth.
        let bar = rh - 4;
        let full = bar * i32::from(b.hp) / 6;
        im.rect(3, y0 + 1, 2, bar, 'g');
        im.rect(3, y0 + 1 + bar - full, 2, full, cloth(id));
        // Three lamps, lit as the table's are.
        let (ly, lx) = if n <= 4 { (y0 + 11, 7) } else { (y0 + 10, 7) };
        for lamp in 0..3 {
            im.stamp(&pips()[usize::from(b.lit & (1 << lamp) != 0)], lx + lamp * 6, ly);
        }
        if k == 0 && n > 1 && s.points(id) > 0 {
            im.stamp(&pips()[2], 3, y0 - 3);
        }
        let _ = run;
    }
}

/// The keeper's six pods, the three lamps and the bell, over the right panel.
pub(super) fn panel(im: &mut Img, s: &Session, me: u32, plan: &Plan, now: u32) {
    let b = &s.tables[&me].ball;
    let pods = pods();
    for i in 0..6i32 {
        let (x, y) = (334 + (i % 3) * 14, 61 + (i / 3) * 11);
        let sprite = if i < i32::from(b.hp) {
            if b.hp <= 2 && (now / 6) % 2 == 0 && i == i32::from(b.hp) - 1 { &pods[1] } else { &pods[0] }
        } else if i == i32::from(b.hp) && b.hit > 5 {
            &pods[3]
        } else {
            &pods[2]
        };
        im.stamp(sprite, x, y);
    }
    let lamps = lamp_set();
    for i in 0..3 {
        let lit = b.lit & (1 << i) != 0;
        let flash = b.flash[i as usize] > 6;
        let sprite = if flash { &lamps[3] } else if !lit { &lamps[0] } else if (now / 8 + i as u32) % 3 == 0 { &lamps[2] } else { &lamps[1] };
        im.stamp(sprite, 332 + i * 16, 98);
    }
    // The bell, and the bar that ticks to it (or runs down while it rings).
    let age = LENGTH - s.left;
    let (ringing, left, span) = claims::bell_clock(age);
    let swing = if ringing { if (now / 4) % 2 == 0 { 1 } else { -1 } } else { 0 };
    let icon = &bells()[match swing { 0 => 0, 1 => 1, _ => 2 }];
    im.stamp(icon, 332 - i32::from(swing < 0), 170);
    let (bx, by, bw) = (350, 176, 28);
    im.rect(bx, by, bw, 4, 's');
    if s.intro == 0 {
        let fill = if ringing { left as f32 / span as f32 } else if span > 1 || left > 0 { 1.0 - (left as f32 / span as f32).min(1.0) } else { 0.0 };
        let w = (bw as f32 * fill) as i32;
        im.rect(bx, by, w, 4, if ringing { 't' } else { 'o' });
        im.rect(bx, by, w, 1, if ringing { 'T' } else { 't' });
    }
    let _ = (plan, HZ, POP_LIFE);
}
