//! What a fallen knight leaves: the knight turns to a skeleton that comes
//! apart where they stood, the bones hopping out and settling on the floor,
//! a scrap of their colours lying beside them. The bones stay until the
//! knight is revived or the party leaves the room.
//!
//! Every piece is hand-drawn ink rows; each has a start where it stood in
//! the skeleton, a place it comes to rest (by the knight's seat, so the
//! same knight always falls the same way), and a short bouncing arc between.

use super::super::ink::{Img, hash};
use super::knights;
use super::sprites;
use crate::rules::{Hero, HZ};

/// Ticks the bones take to settle.
const FALL: u32 = HZ * 7 / 10;

fn rows(rows: &[&str]) -> Img {
    Img::from_rows(rows)
}

/// The skull: lying with the bones, carried over a friend's head, or
/// marking a monster weak enough to raise a knight in.
pub(super) fn skull() -> Img {
    rows(&[
        ".iHHi.", "iHHHHi", "HkHHkH", "iHHHHi", ".iHHi.", "..hh..",
    ])
}

fn ribs() -> Img {
    rows(&[
        "..hiHih..",
        ".hi.H.ih.",
        ".hi.H.ih.",
        "..hiHih..",
        "...hHh...",
    ])
}

fn pelvis() -> Img {
    rows(&["hiHHih", ".hhhh.", "..hh.."])
}

/// A long bone `len` pixels, knobbed at both ends.
fn bone(len: i32) -> Img {
    let mut im = Img::new(len, 3);
    for x in 1..len - 1 {
        im.put(x, 1, 'i');
    }
    for x in [0, len - 1] {
        im.put(x, 0, 'H');
        im.put(x, 2, 'H');
        im.put(x, 1, 'H');
    }
    im
}

/// A scrap of the knight's tabard, in their colours.
fn cloth() -> Img {
    rows(&[".7777.", "775577", "7755587", ".78888."])
}

/// One piece of the skeleton: its picture, where it stood (from the feet,
/// the picture's middle), where it comes to rest, how high it hops, and how
/// many ticks it waits to let go.
struct Piece {
    im: Img,
    from: (i32, i32),
    to: (i32, i32),
    hop: f32,
    delay: u32,
    turn: bool,
}

fn pieces(id: u32, hero: &Hero) -> Vec<Piece> {
    // The same knight always falls the same way; seats differ.
    let h = |n: i32| (hash(n, id as i32, 61) % 7) as i32 - 3;
    let side = if hero.aim_x < 0.0 { -1 } else { 1 };
    let mut out = vec![
        Piece {
            im: skull(),
            from: (0, -13),
            to: (side * (6 + h(1) / 2), 1 + h(2) / 3),
            hop: 11.0,
            delay: 0,
            turn: true,
        },
        Piece {
            im: ribs(),
            from: (0, -8),
            to: (-side * 1, 0),
            hop: 6.0,
            delay: 2,
            turn: false,
        },
        Piece {
            im: pelvis(),
            from: (0, -4),
            to: (-side * (4 + h(3) / 3), 3),
            hop: 5.0,
            delay: 4,
            turn: false,
        },
    ];
    for (n, (from, len, dx, dy)) in [
        ((-5, -8), 6, -8, 2),
        ((5, -8), 6, 4, 4),
        ((-2, -2), 8, 9, 3),
        ((2, -2), 8, -6, 5),
    ]
    .into_iter()
    .enumerate()
    {
        out.push(Piece {
            im: bone(len),
            from,
            to: (side * dx + h(10 + n as i32) / 3, dy + h(20 + n as i32).abs() / 4),
            hop: 5.0 + n as f32,
            delay: 1 + n as u32 * 2,
            turn: n % 2 == 1,
        });
    }
    out
}

/// A fallen knight's bones with their feet at `(x, y)`; the skull among
/// them unless `skull` is false (a friend carries it, or it was dropped
/// elsewhere).
pub(super) fn draw(cv: &mut Img, hero: &Hero, id: u32, tick: u64, wiped: bool, x: i32, y: i32, skull: bool) {
    // A wiped party's clock has stopped: its bones are already down.
    let age = if hero.fell == 0 || wiped {
        u32::MAX
    } else {
        tick.saturating_sub(hero.fell).min(u64::from(u32::MAX - 1)) as u32
    };
    for (n, p) in pieces(id, hero).iter().enumerate() {
        if n == 0 && !skull {
            continue;
        }
        let t = age.saturating_sub(p.delay) as f32 / FALL as f32;
        let (fx, fy) = (x + p.from.0, y + p.from.1);
        let (tx, ty) = (x + p.to.0, y + p.to.1);
        let (px, py) = if t >= 1.0 {
            (tx, ty)
        } else {
            // A hop out, then a smaller one as it lands.
            let u = t.clamp(0.0, 1.0);
            let ease = u * (2.0 - u);
            let bounce = (u * std::f32::consts::PI).sin() * p.hop
                + if u > 0.7 {
                    ((u - 0.7) / 0.3 * std::f32::consts::PI).sin() * 2.0
                } else {
                    0.0
                };
            (
                fx + ((tx - fx) as f32 * ease).round() as i32,
                fy + ((ty - fy) as f32 * ease).round() as i32 - bounce.round() as i32,
            )
        };
        let mut im = p.im.clone();
        if p.turn && t >= 0.5 {
            im = sprites::quarter_turn(&im);
        }
        // A cloud where each piece lands.
        if (1.0..1.35).contains(&t) && n < 3 {
            let puff = sprites::dust(((t - 1.0) * 30.0) as u32 + 4);
            cv.stamp(&puff, tx - puff.w / 2, ty - puff.h / 2 + 2);
        }
        cv.stamp(&im, px - im.w / 2, py - im.h / 2);
    }
    // The cloth, in the knight's colours, falling last beside the ribs.
    let colours = if hero.colours().is_empty() {
        knights::seat_colours(id)
    } else {
        hero.colours()
    };
    let rag = cloth().recolor(colours);
    let side = if hero.aim_x < 0.0 { 1 } else { -1 };
    let land = age.saturating_sub(5) as f32 / FALL as f32;
    let drop = if land >= 1.0 {
        0
    } else {
        ((1.0 - land) * -9.0) as i32
    };
    cv.stamp(&rag, x + side * 7 - rag.w / 2, y + 5 - rag.h / 2 + drop);
}
