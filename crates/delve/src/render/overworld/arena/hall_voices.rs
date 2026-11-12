//! The hall's talk and the gate's dragon, drawn: lines said over the keeper,
//! the patrons and the vendors for everyone in the room to read
//! (`rules::hall_voices`); the name of a place on coming into it; and the
//! dragon's head over the inn's door, whose eye follows the knights in the
//! courtyard and whose nostril smokes when the party comes in or stands
//! near.

use super::super::chrome::{self, Tone};
use super::super::ink::{BLACK, Img, bayer, text_width};
use super::{TILE, alehouse, at, notices};
use crate::rules::hall_voices::{CALL_TICKS, TITLE_TICKS};
use crate::rules::{RoomKind, Run};

/// The most letters of a said line on one row of its plate.
const ROW_LETTERS: usize = 24;
/// How far over a speaker's feet their head is (pixels): a knight's body.
const HEAD: i32 = 26;

/// Where a plate sits against its speaker.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Above,
    Right,
    Left,
    Below,
    /// Clear of every board, tailless: when the speaker's own sides are taken.
    Free,
}

/// A said line on its plate: black paper in a timber rim, amber letters,
/// a small tail toward the speaker on the `side` it hangs from.
fn plate_img(text: &str, side: Side) -> (Img, i32, i32) {
    let lines = alehouse::wrap(text, ROW_LETTERS);
    let w = lines.iter().map(|l| text_width(l)).max().unwrap_or(0) + 9;
    let h = lines.len() as i32 * 9 + 4;
    // The box's own place inside the picture, which leaves room for the tail.
    let (ox, oy, iw, ih) = match side {
        Side::Free => (0, 0, w, h),
        Side::Above => (0, 0, w, h + 3),
        Side::Below => (0, 3, w, h + 3),
        Side::Right => (3, 0, w + 3, h),
        Side::Left => (0, 0, w + 3, h),
    };
    let mut im = Img::new(iw, ih);
    chrome::panel(&mut im, (ox, oy, w, h), Tone::NIGHT_SIGN);
    for (i, line) in lines.iter().enumerate() {
        im.text(ox + 4, oy + 3 + i as i32 * 9, line, 't');
    }
    let (mx, my) = (ox + w / 2, oy + h / 2);
    match side {
        Side::Free => {}
        Side::Above => {
            im.line(mx - 1, h, mx + 1, h, 'K');
            im.put(mx, h + 1, 'K');
        }
        Side::Below => {
            im.line(mx - 1, 2, mx + 1, 2, 'K');
            im.put(mx, 1, 'K');
        }
        Side::Right => {
            im.line(2, my - 1, 2, my + 1, 'K');
            im.put(1, my, 'K');
        }
        Side::Left => {
            im.line(w + 0, my - 1, w + 0, my + 1, 'K');
            im.put(w + 1, my, 'K');
        }
    }
    (im, iw, ih)
}

fn meets(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 - 2 < b.0 + b.2 && b.0 < a.0 + a.2 + 2 && a.1 - 2 < b.1 + b.3 && b.1 < a.1 + a.3 + 2
}

/// Every line up in the room, on a plate by its speaker: over their head if
/// that is clear, else beside them or under; kept clear of the `taken`
/// boards, and added to them. A plate pops up into place, holds to be
/// read, and dithers away (`notices::board_from`). `origin` is the room
/// pixel at the picture's top-left.
pub(super) fn plates(cv: &mut Img, run: &Run, origin: (i32, i32), taken: &mut Vec<(i32, i32, i32, i32)>) {
    for call in run.calls_up() {
        let (fx, fy) = call.voice.feet();
        let (x, y) = at(fx * 2.0, fy * 2.0);
        let (x, y) = (x - origin.0, y - origin.1);
        if !(0..cv.w).contains(&x) || !(0..cv.h + HEAD).contains(&y) {
            continue;
        }
        let head = y - HEAD;
        let clamp = |px: i32, py: i32, w: i32, h: i32| ((px).clamp(3, (cv.w - w - 3).max(3)), py.clamp(2, (cv.h - h - 2).max(2)));
        let mut chosen = None;
        for side in [Side::Above, Side::Right, Side::Left, Side::Below, Side::Free] {
            let (im, w, h) = plate_img(&call.text, side);
            let (mut px, mut py) = match side {
                Side::Above => clamp(x - w / 2, head - h - 1, w, h),
                Side::Right => clamp(x + 8, head + 2, w, h),
                Side::Left => clamp(x - 8 - w, head + 2, w, h),
                Side::Below => clamp(x - w / 2, y + 3, w, h),
                Side::Free => clamp(x - w / 2, 2, w, h),
            };
            if side == Side::Free {
                // Down past whatever is in the way.
                while let Some(t) = taken.iter().find(|&&t| meets((px, py, w, h), t)) {
                    py = t.1 + t.3 + 3;
                }
                px = px.min((cv.w - w - 3).max(3));
            }
            let rect = (px, py, w, h);
            if !taken.iter().any(|&t| meets(rect, t)) && py + h < cv.h {
                chosen = Some((im, rect));
                break;
            }
        }
        let Some((im, rect)) = chosen else { continue };
        let age = run.tick.saturating_sub(call.at) as f32;
        notices::board_from(cv, &im, (rect.0, rect.1), age, Some(CALL_TICKS as f32), 5.0);
        taken.push(rect);
    }
}

/// The name of the place the party has just come into, on black paper at
/// the top of a seat's view (under a prompt board when one is up), with a
/// line of what is true there. It comes up, holds, and dithers away.
pub(super) fn title(cv: &mut Img, run: &Run, taken: &mut Vec<(i32, i32, i32, i32)>) {
    let Some(age) = run.title_age() else { return };
    let Some((name, line)) = run.place_title() else { return };
    let w = text_width(name).max(text_width(&line)) + 12;
    let h = if line.is_empty() { 15 } else { 24 };
    let mut im = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            im.set(x, y, BLACK);
        }
    }
    im.line(1, 0, w - 2, 0, 'g');
    im.text((w - text_width(name)) / 2, 4, name, 't');
    if !line.is_empty() {
        im.text((w - text_width(&line)) / 2, 14, &line, 'G');
    }
    let x = (cv.w - w) / 2;
    let y = taken.iter().filter(|&&t| meets((x, 6, w, h), t)).map(|t| t.1 + t.3 + 3).max().unwrap_or(6);
    notices::board_from(cv, &im, (x, y), age as f32, Some(TITLE_TICKS as f32), 6.0);
    taken.push((x, y, w, h));
}

/// Where the inn's front stands in the courtyard (its top-left, pixels).
fn inn_origin() -> (i32, i32) {
    let (dc, dr, dw, dh) = crate::rules::alehouse::COURTYARD_DOOR;
    (dc * TILE + dw * TILE / 2 - alehouse::INN_W / 2, (dr + dh) * TILE - alehouse::INN_H)
}

/// A soft warm lift round `(x, y)`, `rx` wide and `ry` tall at most, thinning
/// to the edge through the dither, drawn only where it adds light. A fire's
/// glow on what is near it; `power` (0 to 1) is how strong.
pub(super) fn ember_glow(cv: &mut Img, x: i32, y: i32, rx: i32, ry: i32, power: f32) {
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let d = ((dx as f32 / rx as f32).powi(2) + (dy as f32 / ry as f32).powi(2)).sqrt();
            if d >= 1.0 || bayer(x + dx, y + dy) >= (1.0 - d) * power * 0.75 {
                continue;
            }
            cv.put(x + dx, y + dy, if d < 0.45 { 'o' } else if d < 0.75 { 'R' } else { 'a' });
        }
    }
}

/// The dragon's head over the inn's door, awake. Its eye blinks now and
/// then; with a knight in the courtyard the eye brightens, its slit turns
/// after the nearest and a warm glow rings it; the nostril smokes in a curl
/// when the party has just come in, and again now and then while a knight
/// stands near.
pub(super) fn dragon(cv: &mut Img, run: &Run) {
    if run.room().kind != RoomKind::Gate {
        return;
    }
    let (x0, y0) = inn_origin();
    // The eye and the nostril as the head draws them (head at 16, 16).
    let (ex, ey) = (x0 + 16 + 28, y0 + 16 + 15);
    let nose = (x0 + 16 + 5, y0 + 16 + 16);
    let tick = run.tick;
    // The nearest knight standing in the courtyard, from the eye.
    let near = run
        .players
        .values()
        .filter(|h| h.hp > 0 && !h.stone && !h.absent)
        .map(|h| {
            let (kx, ky) = at(h.x, h.y);
            (kx - ex, ky - ey)
        })
        .min_by_key(|&(dx, dy)| dx * dx + dy * dy);
    let watching = near.filter(|&(dx, dy)| dx.abs() < 9 * TILE && dy.abs() < 9 * TILE);
    let blinking = (tick + 40) % 160 < 4;
    let (cx, cy) = (ex as f32, ey as f32 + 0.5);
    if blinking {
        // The lid comes down over the eye.
        cv.ellipse(cx, cy, 3.6, 2.2, 'f');
        cv.ellipse(cx, cy, 2.8, 1.6, 'l');
        cv.line(ex - 3, ey, ex + 3, ey, 'f');
    } else {
        if watching.is_some() {
            ember_glow(cv, ex, ey, 9, 6, 0.8);
        }
        cv.ellipse(cx, cy, 3.6, 2.2, 'f');
        cv.ellipse(cx, cy, 2.8, 1.6, if watching.is_some() { '5' } else { 't' });
        cv.line(ex - 2, ey, ex + 2, ey, if watching.is_some() { '6' } else { 'T' });
        let turn = watching.map_or(0, |(dx, _)| dx.signum());
        cv.line(ex + turn, ey - 1, ex + turn, ey + 2, 'k');
        cv.put(ex - 1, ey - 1, 'H');
    }
    // The smoke: puffs off the snout, growing as they thin and lean away.
    let arrived = run.title_age().is_some_and(|age| age < 75);
    let snorting = watching.is_some() && tick % 180 < 60;
    if arrived || snorting {
        let age = if arrived { run.title_age().unwrap_or(0) } else { tick % 180 } as i32;
        for k in 0..6i32 {
            let t = age - k * 5;
            if !(0..46).contains(&t) {
                continue;
            }
            let (sx, sy) = (nose.0 - 1 - t * 3 / 5 - ((t as f32 * 0.3 + k as f32).sin() * 1.5) as i32, nose.1 - 1 - t / 2);
            let size = 1 + t / 14;
            let ink = if t < 14 { 'i' } else if t < 28 { 'h' } else { 'J' };
            for dy in 0..size {
                for dx in 0..size {
                    if bayer(sx + dx, sy + dy) < 1.15 - t as f32 / 48.0 {
                        cv.put(sx + dx, sy + dy, ink);
                    }
                }
            }
        }
    }
}
