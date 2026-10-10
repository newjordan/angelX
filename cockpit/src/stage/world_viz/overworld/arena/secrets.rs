//! A cracked wall, drawn: dark fissures across the two stones where a
//! doorway would be, and chips of stone fallen at the wall's foot. Once a
//! bomb finds it, it is an ordinary doorway, and the scenery says so.

use super::super::ink::Img;
use super::{at, sprites, stand};
use crate::drive::together_shooter::EnemyKind;
use crate::drive::together_shooter::secrets::snibbet_at;

/// The fissures, for a wall that runs east to west (the north and south
/// walls); turned for the east and west. `k` the gap, `h` its broken,
/// lit edge.
const FISSURE: [&str; 16] = [
    "...........h....................",
    "..........hk..........h.........",
    "...........kh........hk.........",
    "...........kk.......hkk.........",
    ".....h......kh......kk..........",
    "......kkh...kh.....hk...........",
    ".......kkh.hkkh...hkk...........",
    "........kkkkhkkh.hk.............",
    "..............kkkk......hh......",
    ".............hk.kkh....hkk......",
    "............hk....kkhhhkk.......",
    "...........hk......kkkkh........",
    "..........hkk.......hkh.........",
    ".........hkk.........hk.........",
    "........hk............kh........",
    "................................",
];

/// The crack at `(cx, cy)` (the middle of the doorway it hides, arena
/// units) in the wall on `side` (north, east, south, west).
pub(super) fn crack(cv: &mut Img, (cx, cy): (f32, f32), side: usize) {
    let (x, y) = at(cx, cy);
    let across = Img::from_rows(&FISSURE);
    let im = if side.is_multiple_of(2) {
        across
    } else {
        let mut turned = Img::new(across.h, across.w);
        for ty in 0..across.h {
            for tx in 0..across.w {
                if let Some(c) = across.get(tx, ty) {
                    turned.set(ty, tx, c);
                }
            }
        }
        turned
    };
    cv.stamp(&im, x - im.w / 2, y - im.h / 2);
    // Stones fallen at the wall's foot, inside the room.
    let (ix, iy) = inward(side, 11);
    for (k, (dx, dy)) in [(-8, -1), (-4, 1), (1, 0), (5, 2), (9, -1), (-1, 3)]
        .iter()
        .enumerate()
    {
        let (px, py) = if side.is_multiple_of(2) {
            (*dx, *dy)
        } else {
            (*dy, *dx)
        };
        let (lit, dark) = if k % 2 == 0 { ('J', 'g') } else { ('h', 'G') };
        cv.put(x + ix + px, y + iy + py, lit);
        cv.put(x + ix + px + 1, y + iy + py, lit);
        cv.put(x + ix + px, y + iy + py + 1, dark);
        cv.put(x + ix + px + 1, y + iy + py + 1, dark);
    }
}

/// A step from the wall on `side` into the room, `by` pixels.
fn inward(side: usize, by: i32) -> (i32, i32) {
    match side {
        0 => (0, by),
        1 => (-by, 0),
        2 => (0, -by),
        _ => (by, 0),
    }
}

/// The draught through the crack: pale motes drifting into the room, a
/// wall that breathes.
pub(super) fn draught(cv: &mut Img, run: &crate::drive::together_shooter::Run) {
    let Some((host, cx, cy)) = run.dungeon.crack() else {
        return;
    };
    if host != run.at {
        return;
    }
    let side = run.dungeon.secret.map_or(0, |s| s.side);
    let (x, y) = at(cx, cy);
    for k in 0..4u64 {
        let age = ((run.tick + k * 23) % 90) as i32;
        let (ox, oy) = inward(side, 6 + age / 3);
        let sway = ((age as f32 * 0.15 + k as f32 * 1.7).sin() * 3.0).round() as i32;
        let (sx, sy) = if side.is_multiple_of(2) {
            (sway + (k as i32 - 2) * 4, 0)
        } else {
            (0, sway + (k as i32 - 2) * 4)
        };
        cv.put(x + ox + sx, y + oy + sy, if age < 45 { 'v' } else { 'u' });
    }
}

/// Snibbet in his vault, a retired loot goblin on his sack; once he has
/// paid, he waves the party off.
pub(super) fn snibbet(cv: &mut Img, run: &crate::drive::together_shooter::Run) {
    let Some(secret) = run
        .dungeon
        .secret
        .filter(|s| s.snibbet && run.at == s.vault)
    else {
        return;
    };
    let (sx, sy) = snibbet_at(run.room());
    let (x, y) = at(sx, sy);
    let tick = if secret.paid { run.tick as u32 } else { 0 };
    stand(cv, &sprites::enemy(EnemyKind::Goblin, tick), x, y);
}
