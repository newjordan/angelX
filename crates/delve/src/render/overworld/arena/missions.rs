//! The places of a knight's missions (`rules::missions`), on that knight's
//! own view: a pennant in the missions' amber over the door, the stone or
//! the spot where each can be done, and an arrowhead in the same amber at
//! the view's edge toward one out of sight. Only the mark nearest the
//! knight carries its name, so a fresh knight on a campaign far along (news
//! at every teacher's door) still reads the map. A delve floor marks its
//! rooms on the map instead (`dungeon_map`).

use super::super::chrome::{self, Tone};
use super::*;

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

pub(super) fn doors(
    over: &mut Img,
    run: &Run,
    id: u32,
    origin: (i32, i32),
    tick: u32,
    avoid: &[(i32, i32, i32, i32)],
) -> Vec<(i32, i32, i32, i32)> {
    let mut taken = avoid.to_vec();
    let marks: Vec<(String, (i32, i32))> = run
        .mission_doors(id)
        .into_iter()
        .map(|(name, (tx, ty))| (name, (tx * TILE + TILE / 2 - 12 - origin.0, ty * TILE - 2 - origin.1)))
        .collect();
    let named = nearest(run, id, origin, &marks);
    for (i, (name, at)) in marks.iter().enumerate() {
        mark(over, *at, (named == Some(i)).then_some(name), tick, NATIVE_H / 2 - 64, &mut taken);
    }
    taken.split_off(avoid.len())
}

pub(crate) fn spots(over: &mut Img, run: &Run, focus: Option<u32>, origin: (i32, i32)) {
    let id = focus.unwrap_or_else(|| run.playtest.local_seat());
    let mut taken = Vec::new();
    let marks: Vec<(String, (i32, i32))> = run
        .mission_spots(id)
        .into_iter()
        .map(|(name, (sx, sy))| {
            let (px, py) = at(sx * TILE_UNITS, sy * TILE_UNITS);
            (name, (px - origin.0, py - 8 - origin.1))
        })
        .collect();
    let named = nearest(run, id, origin, &marks);
    for (i, (name, at)) in marks.iter().enumerate() {
        mark(over, *at, (named == Some(i)).then_some(name), run.tick as u32, NATIVE_H / 2 - 40, &mut taken);
    }
}

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
        vec![(fx - w - 5, fy + 1), (fx + flag.w + 4, fy + 1), (fx + flag.w / 2 - w / 2, fy - 12)]
    } else {
        let (cx, cy) = ((NATIVE_W / 2) as f32, (NATIVE_H / 2) as f32);
        let (dx, dy) = (x as f32 - cx, y as f32 - cy);
        let reach_y = if dy > 0.0 { reach_y } else { NATIVE_H / 2 - 8 };
        let k = ((NATIVE_W / 2 - 8) as f32 / dx.abs().max(0.01)).min(reach_y as f32 / dy.abs().max(0.01));
        let len = dx.hypot(dy).max(0.01);
        let (ux, uy) = (dx / len, dy / len);
        let (mut ex, mut ey) = ((cx + dx * k).round() as i32, (cy + dy * k).round() as i32);
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

fn overlaps(taken: &[(i32, i32, i32, i32)], (x, y, w, h): (i32, i32, i32, i32)) -> bool {
    taken
        .iter()
        .any(|&(x0, y0, w0, h0)| x < x0 + w0 && x + w > x0 && y < y0 + h0 && y + h > y0)
}

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
