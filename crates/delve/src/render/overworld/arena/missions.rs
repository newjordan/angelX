//! The places of a knight's missions (`rules::missions`), on that knight's
//! own view: a pennant in the missions' amber over the door, the stone or
//! the spot where each can be done, and an arrowhead in the same amber at
//! the view's edge toward one out of sight. Only the mark nearest the
//! knight carries its name, so a fresh knight on a campaign far along (news
//! at every teacher's door) still reads the map. A delve floor marks its
//! rooms on the map instead (`dungeon_map`).

use super::super::chrome::{self, Tone};
use super::*;

/// The pennant: a flag on its pole, signal amber in the UI's near-black
/// rim, small enough to sit beside a door without hiding it. Its foot is
/// the bottom row.
const PENNANT: &[&str] = &[
    "KKK.....",
    "KJ@KK...",
    "KJ@@@KK.",
    "KJ@@@@@K",
    "KJ4444KK",
    "KJ44KK..",
    "KJKK....",
    "KJK.....",
    "KJK.....",
    "KJK.....",
    "KKK.....",
];

/// Knight `id`'s missions over the kingdom's doors, on a view whose
/// top-left is world pixel `origin`, their names clear of the marks in
/// `avoid` (the gold arrow's). Returns the rectangles the marks take, so
/// the plaques make way for them.
pub(super) fn doors(
    over: &mut Img,
    run: &Run,
    id: u32,
    origin: (i32, i32),
    tick: u32,
    avoid: &[(i32, i32, i32, i32)],
) -> Vec<(i32, i32, i32, i32)> {
    let mut taken = avoid.to_vec();
    // Beside the door, left of where the gold arrow points down at it.
    let marks: Vec<(String, (i32, i32))> = run
        .mission_doors(id)
        .into_iter()
        .map(|(name, (tx, ty))| (name, (tx * TILE + TILE / 2 - 12 - origin.0, ty * TILE - 2 - origin.1)))
        .collect();
    let named = nearest(run, id, origin, &marks);
    for (i, (name, at)) in marks.iter().enumerate() {
        // Below, the edge marks stop above the footer and the banners' names.
        mark(over, *at, (named == Some(i)).then_some(name), tick, NATIVE_H / 2 - 64, &mut taken);
    }
    taken.split_off(avoid.len())
}

/// Knight `focus`'s missions with a spot in the room the party stands in,
/// at home: a teacher, a game's ring.
pub(crate) fn spots(over: &mut Img, run: &Run, focus: Option<u32>, origin: (i32, i32)) {
    let id = focus.unwrap_or_else(|| run.playtest.local_seat());
    let mut taken = Vec::new();
    let marks: Vec<(String, (i32, i32))> = run
        .mission_spots(id)
        .into_iter()
        .map(|(name, (sx, sy))| {
            let (px, py) = at(sx * TILE_UNITS, sy * TILE_UNITS);
            // Just over the head of whoever stands there (a teacher's
            // figure is seven rows tall, `teachers`).
            (name, (px - origin.0, py - 8 - origin.1))
        })
        .collect();
    let named = nearest(run, id, origin, &marks);
    for (i, (name, at)) in marks.iter().enumerate() {
        mark(over, *at, (named == Some(i)).then_some(name), run.tick as u32, NATIVE_H / 2 - 40, &mut taken);
    }
}

/// Which of `marks` (view points) is nearest knight `id`, the one named.
fn nearest(run: &Run, id: u32, origin: (i32, i32), marks: &[(String, (i32, i32))]) -> Option<usize> {
    let (hx, hy) = run.players.get(&id).map_or((NATIVE_W / 2, NATIVE_H / 2), |h| {
        let (px, py) = at(h.x, h.y);
        (px - origin.0, py - origin.1)
    });
    (0..marks.len()).min_by_key(|&i| {
        let (x, y) = marks[i].1;
        (x - hx).pow(2) + (y - hy).pow(2)
    })
}

/// One mission's mark for view point `(x, y)`, the pennant's foot: the
/// pennant bobbing there with the name beside it when it is in view, an
/// arrowhead at the edge toward it when it is not (no lower than `reach_y`
/// under the middle), moved along the edge off a mark already there (the
/// gold arrow to the same door). The name, when it has one, takes the
/// first of its places that lands on no other mark, or is left out; the
/// pennant or the arrow still shows.
fn mark(over: &mut Img, (x, y): (i32, i32), name: Option<&String>, tick: u32, reach_y: i32, taken: &mut Vec<(i32, i32, i32, i32)>) {
    let label = name.map(|name| name.to_uppercase()).unwrap_or_default();
    let w = crate::render::overworld::ink::text_width(&label);
    let inside = (12..NATIVE_W - 12).contains(&x) && (24..NATIVE_H - 8).contains(&y);
    let places: Vec<(i32, i32)> = if inside {
        let bob = [0, 1, 2, 1][(tick / 6 % 4) as usize];
        let flag = Img::from_rows(PENNANT);
        let (fx, fy) = (x - 1, y - flag.h - bob);
        over.stamp(&flag, fx, fy);
        taken.push((fx, fy, flag.w, flag.h));
        // Left of the pennant, right of it, over it.
        vec![(fx - w - 5, fy + 1), (fx + flag.w + 4, fy + 1), (fx + flag.w / 2 - w / 2, fy - 12)]
    } else {
        let (cx, cy) = ((NATIVE_W / 2) as f32, (NATIVE_H / 2) as f32);
        let (dx, dy) = (x as f32 - cx, y as f32 - cy);
        let reach_y = if dy > 0.0 { reach_y } else { NATIVE_H / 2 - 8 };
        let k = ((NATIVE_W / 2 - 8) as f32 / dx.abs().max(0.01)).min(reach_y as f32 / dy.abs().max(0.01));
        let len = dx.hypot(dy).max(0.01);
        let (ux, uy) = (dx / len, dy / len);
        let (mut ex, mut ey) = ((cx + dx * k).round() as i32, (cy + dy * k).round() as i32);
        // Along the edge it lies on, toward the middle, while it would
        // land on another mark.
        let along = if (cx + dx * k - cx).abs() >= (NATIVE_W / 2 - 9) as f32 {
            (0, if ey > cy as i32 { -16 } else { 16 })
        } else {
            (if ex > cx as i32 { -16 } else { 16 }, 0)
        };
        for _ in 0..3 {
            if !overlaps(taken, (ex - 6, ey - 6, 13, 13)) {
                break;
            }
            (ex, ey) = (ex + along.0, ey + along.1);
        }
        arrowhead(over, (ex, ey), (ux, uy));
        taken.push((ex - 6, ey - 6, 13, 13));
        // The name toward the middle of the view, off the arrow.
        let lx = if ux > 0.5 {
            ex - w - 10
        } else if ux < -0.5 {
            ex + 10
        } else {
            ex - w / 2
        };
        let ly = if uy > 0.5 {
            ey - 18
        } else if uy < -0.5 {
            ey + 10
        } else {
            ey - 3
        };
        // Else nearer the middle, else beside the arrow either way.
        vec![(lx, ly), (lx, ly + 12 * -uy.signum() as i32), (ex - w - 10, ey - 3), (ex + 10, ey - 3)]
    };
    if name.is_none() {
        return;
    }
    for (lx, ly) in places {
        let tag = (lx.clamp(4, NATIVE_W - w - 4), ly.clamp(4, NATIVE_H - 13));
        if !overlaps(taken, chrome::label_rect(tag, &label)) {
            taken.push(chrome::label(over, tag, &label, '@', Tone::SLATE));
            return;
        }
    }
}

/// Whether `rect` lands on any of `taken`.
fn overlaps(taken: &[(i32, i32, i32, i32)], (x, y, w, h): (i32, i32, i32, i32)) -> bool {
    taken
        .iter()
        .any(|&(x0, y0, w0, h0)| x < x0 + w0 && x + w > x0 && y < y0 + h0 && y + h > y0)
}

/// A small arrowhead at `(x, y)` pointing along `(ux, uy)`, the way the
/// view's edge marks point (`beyond`), rimmed so it reads over any ground.
fn arrowhead(over: &mut Img, (x, y): (i32, i32), (ux, uy): (f32, f32)) {
    for (ink, grow) in [('K', 1.2), ('@', 0.0)] {
        for py in -6..=6 {
            for px in -6..=6 {
                let along = px as f32 * ux + py as f32 * uy;
                let across = (py as f32 * ux - px as f32 * uy).abs();
                if (-3.5 - grow..=4.0 + grow).contains(&along) && across <= (4.0 + grow - along) * 0.6 + grow * 0.5 {
                    over.put(x + px, y + py, ink);
                }
            }
        }
    }
}
