//! The wish altar (`rules::altar`): a two-step stone plinth with a carved
//! rune ring on its face, a red runner over its slab and two candles, and
//! on it the genie's lamp, the art team's own (`render::genie`). While the
//! host's runners are writing wishes the lamp smokes in the genie's teal
//! and its ring burns; when a grant lands the lamp flares and the knight it
//! landed on is lit by a falling column of sparks. E at the lamp rubs it.

use super::super::ink::{Img, text_width};
use super::at;
use crate::rules::altar::FLARE_TICKS;
use crate::rules::{HZ, Run};

const W: i32 = 44;
const H: i32 = 54;
/// The slab's top, where the lamp stands, from the sprite's top.
const SLAB: i32 = 27;

/// The altar as it stands at `tick`: `smoke` wishes in flight, `flare` the
/// ticks since a grant landed (while it flares).
pub(crate) fn sprite(tick: u32, smoke: usize, flare: Option<u32>) -> Img {
    let mut im = Img::new(W, H);
    let cx = W / 2;
    // Two steps, widest at the foot: a lit lip, a shadowed riser, worn joints.
    for (k, (half, top)) in [(21, H - 6), (17, H - 11)].into_iter().enumerate() {
        im.rect(cx - half, top, half * 2, 5, 'u');
        im.line(cx - half, top, cx + half - 1, top, 'v');
        im.line(cx - half, top + 4, cx + half - 1, top + 4, 'S');
        im.put(cx - half, top, 'U');
        im.put(cx + half - 1, top, 'U');
        for x in (cx - half + 4 + 2 * k as i32..cx + half - 3).step_by(8) {
            im.put(x, top + 2, 'S');
        }
    }
    // The block: lit on the left, in shadow on the right.
    let (bl, br, bt, bb) = (cx - 12, cx + 11, SLAB + 3, H - 12);
    im.rect(bl, bt, br - bl + 1, bb - bt + 1, 'S');
    im.rect(bl + 2, bt, br - bl - 3, bb - bt + 1, 'u');
    im.line(bl, bt, bl, bb, 'U');
    im.line(br, bt, br, bb, 's');
    im.line(br - 1, bt, br - 1, bb, 'x');
    // The rune ring carved in its face: dim at rest, burning while wishes
    // are written, white at a grant.
    let lit = flare.is_some();
    let (ring, rune) = if lit {
        ('3', 'w')
    } else if smoke > 0 {
        ('1', '2')
    } else {
        ('0', '1')
    };
    let (ry0, rx, ry) = ((bt + bb) as f32 / 2.0, 6.5, 5.0);
    for i in 0..48 {
        let a = i as f32 / 48.0 * std::f32::consts::TAU;
        im.put((cx as f32 + a.cos() * rx).round() as i32, (ry0 + a.sin() * ry).round() as i32, ring);
    }
    let turn = tick as f32 * if smoke > 0 { 0.12 } else { 0.03 };
    for k in 0..6 {
        let a = turn + k as f32 / 6.0 * std::f32::consts::TAU;
        let (x, y) = ((cx as f32 + a.cos() * rx).round() as i32, (ry0 + a.sin() * ry).round() as i32);
        im.put(x, y, rune);
    }
    // The eye in the ring's middle: the genie's mark.
    im.put(cx, ry0 as i32, rune);
    im.put(cx - 1, ry0 as i32, ring);
    im.put(cx + 1, ry0 as i32, ring);
    // The slab: a pale lip over a dark underside, wider than the block.
    im.rect(cx - 15, SLAB, 30, 3, 'v');
    im.line(cx - 15, SLAB, cx + 14, SLAB, 'V');
    im.line(cx - 15, SLAB + 3, cx + 14, SLAB + 3, 's');
    im.put(cx - 15, SLAB, 'U');
    // The runner: red cloth over the slab, down the face, a gold fringe.
    im.rect(cx - 4, SLAB, 8, 3, '7');
    im.rect(cx - 4, SLAB + 3, 8, 6, '8');
    im.line(cx - 4, SLAB + 3, cx + 3, SLAB + 3, '7');
    im.line(cx - 3, SLAB + 4, cx - 3, SLAB + 8, '7');
    for x in (cx - 4..cx + 4).step_by(2) {
        im.put(x, SLAB + 9, '4');
    }
    // Candles at the slab's ends, their flames flickering.
    for x in [cx - 13, cx + 12] {
        im.line(x, SLAB - 5, x, SLAB - 1, 'c');
        im.put(x, SLAB - 1, '$');
        let flick = (tick / 5 + x as u32).is_multiple_of(3);
        im.put(x, SLAB - 6, if flick { '6' } else { '5' });
        im.put(x, SLAB - 7, '@');
    }
    // The lamp, sitting on the runner.
    let lamp = crate::render::genie::lamp_small();
    let (lx, ly) = (cx - lamp.w / 2, SLAB - lamp.h + 1);
    im.stamp(lamp, lx, ly);
    // Its spout: where the smoke and the flare come out.
    let spout = (lx + lamp.w - 2, ly + lamp.h / 3);
    if smoke > 0 {
        // A curl of the genie's smoke, taller for more wishes in flight.
        let rise = (8 + 5 * smoke.min(4) as i32).min(spout.1);
        for i in 0..rise {
            let t = i as f32 / rise as f32;
            let sway = ((tick as f32 * 0.18) - i as f32 * 0.55).sin() * (1.0 + t * 3.0);
            let x = spout.0 + 1 + sway.round() as i32;
            let y = spout.1 - 1 - i;
            let ink = if (i + (tick / 3) as i32) % 4 == 0 { '3' } else { '2' };
            im.put(x, y, ink);
            im.put(x + 1, y, if t > 0.3 { '2' } else { '3' });
            if t > 0.5 {
                im.put(x - 1, y, '1');
                im.put(x + 2, y, '1');
            }
        }
    }
    if let Some(age) = flare {
        // A ring of gold and white sparks bursting off the lamp.
        let life = FLARE_TICKS as f32;
        let k = age as f32 / life;
        let r = 4.0 + k * 16.0;
        for i in 0..14 {
            let a = i as f32 / 14.0 * std::f32::consts::TAU + k;
            let (x, y) = (spout.0 as f32 - 6.0 + a.cos() * r, spout.1 as f32 + a.sin() * r * 0.7);
            let ink = if i % 3 == 0 { 'w' } else if i % 3 == 1 { '6' } else { '5' };
            im.put(x.round() as i32, y.round() as i32, ink);
        }
        if age < HZ / 2 {
            // The first moment: the lamp itself burns white-gold.
            for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1), (0, 0)] {
                im.put(spout.0 + dx, spout.1 + dy, 'w');
            }
        }
    }
    im
}

/// Where the lamp's spout is in the room, in native pixels: the smoke and
/// the genie come out of it (`magic`).
pub(super) fn spout(run: &Run) -> Option<(f32, f32)> {
    let (ax, ay) = run.wish_altar()?;
    let (x, y) = at(ax, ay);
    let lamp = crate::render::genie::lamp_small();
    let (lx, ly) = (x - W / 2 + W / 2 - lamp.w / 2, y - H + 14 + SLAB - lamp.h + 1);
    Some(((lx + lamp.w - 2) as f32, (ly + lamp.h / 3) as f32))
}

/// The altar where it stands in the room, on the floor under the knights.
pub(super) fn ground(cv: &mut Img, run: &Run) {
    let Some((ax, ay)) = run.wish_altar() else {
        return;
    };
    let tick = run.tick as u32;
    let flare = run.lamp.flaring(run.tick).map(|l| (run.tick - l.tick) as u32);
    let im = sprite(tick, run.lamp.wishing.len(), flare);
    let (x, y) = at(ax, ay);
    // A soft pool of the lamp's light on the floor round it.
    let glow = if flare.is_some() { 3 } else if run.lamp.wishing.is_empty() { 1 } else { 2 };
    for i in 0..(10 * glow) {
        let a = i as f32 * 2.39996;
        let r = 14.0 + (i as f32 * 1.7) % (6.0 * glow as f32);
        let (gx, gy) = (x as f32 + a.cos() * r, y as f32 + 8.0 + a.sin() * r * 0.45);
        if (i + tick as i32 / 6) % 3 == 0 {
            cv.put(gx.round() as i32, gy.round() as i32, if glow > 1 { '1' } else { '0' });
        }
    }
    cv.stamp(&im, x - W / 2, y - H + 14);
}

/// Over the figures: the key at the lamp for the seat's knight, a curl of
/// smoke over each knight whose wish is being written, and on the knight a
/// grant just landed on a column of light, a ring at their feet and what
/// they were given, for everyone to see.
pub(super) fn over(cv: &mut Img, run: &Run, legible: bool, view: (i32, i32), focus: Option<u32>) {
    let tick = run.tick as u32;
    for (&id, hero) in &run.players {
        if hero.hp == 0 || hero.absent {
            continue;
        }
        let (x, y) = at(hero.x, hero.y);
        let pending = run.lamp.pending(id).count();
        if pending > 0 {
            // The genie's smoke curling up over the head: their wish is coming.
            let rise = 8 + 3 * pending.min(3) as i32;
            for i in 0..rise {
                let sway = ((tick as f32 * 0.2) - i as f32 * 0.6).sin() * 2.0;
                let (sx, sy) = (x + 7 + sway.round() as i32, y - 24 - i);
                cv.put(sx, sy, if (i + tick as i32 / 3) % 4 == 0 { '3' } else { '2' });
                if i > 2 && i < rise - 2 {
                    cv.put(sx + 1, sy, '1');
                }
            }
        }
        let Some(landed) = run.lamp.landed.iter().rev().find(|l| l.knight == id) else {
            continue;
        };
        let age = run.tick.saturating_sub(landed.tick);
        if age >= FLARE_TICKS {
            continue;
        }
        let age = age as i32;
        let life = FLARE_TICKS as i32;
        // A column of the genie's light falling on them, thinning as it goes.
        let top = (y - 70).max(0);
        for py in top..y - 6 {
            if (py + age) % (1 + age * 3 / life) != 0 {
                continue;
            }
            let core = if age < life / 3 { 'w' } else { '3' };
            cv.put(x, py, core);
            if age < life / 2 {
                cv.put(x - 1, py, '2');
                cv.put(x + 1, py, '2');
            }
        }
        // A ring of gold spreading at their feet.
        let r = 5.0 + age as f32 * 16.0 / life as f32;
        for i in 0..24 {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            if (i + age) % 3 != 0 {
                cv.put((x as f32 + a.cos() * r).round() as i32, (y as f32 + a.sin() * r * 0.45).round() as i32, if i % 2 == 0 { '6' } else { '5' });
            }
        }
        // Sparks rising round them.
        for k in 0..10 {
            let sx = x - 9 + (k * 7 + age) % 19;
            let sy = y - 6 - (age * 2 + k * 9) % 36;
            cv.put(sx, sy, if k % 3 == 0 { 'w' } else { '6' });
        }
        // What they were given, over their head, for the first moment.
        if legible && age < life * 3 / 4 {
            let name: String = landed.name.split('.').next().unwrap_or("").to_uppercase().chars().take(22).collect();
            let head = "WISH GRANTED";
            let w = text_width(head).max(text_width(&name)) + 6;
            let px = super::in_view_x(x, w, view, cv.w);
            let py = (y - 58).max(0);
            super::super::chrome::panel(cv, (px, py, w, 21), super::super::chrome::Tone::KEY);
            cv.text(px + (w - text_width(head)) / 2, py + 3, head, '6');
            cv.text(px + (w - text_width(&name)) / 2, py + 12, &name, 'i');
        }
    }
    let Some((ax, ay)) = run.wish_altar() else {
        return;
    };
    let Some(me) = focus.or(Some(1)).filter(|&id| run.near_altar(id)) else {
        return;
    };
    // The genie stands over his lamp a moment after a rub (`magic`): the
    // lamp's own board waits for him to go.
    if run.lamp.rubbed.values().any(|&t| run.tick.saturating_sub(t) < 60) {
        return;
    }
    // Their own grant's banner stands over them now: the lamp's waits.
    if legible && run.lamp.landed.iter().any(|l| l.knight == me && run.tick.saturating_sub(l.tick) < FLARE_TICKS * 3 / 4) {
        return;
    }
    let (x, y) = at(ax, ay);
    // Over the lamp's flame, the smoke's room above it left clear.
    let py = (y - H + 18).max(0);
    if !legible {
        for (dx, dy) in [(0, 0), (-1, -1), (1, -1), (0, -1)] {
            cv.put(x + dx, py + dy, '6');
        }
        return;
    }
    let mine = run.lamp.pending(me).count();
    let (head, key) = if run.players.get(&me).is_some_and(|h| h.offer.is_some()) {
        ("THE SCROLL IS OPEN", "1 2 3 CHOOSE")
    } else if run.can_wish(me) {
        ("THE GENIE'S LAMP", "E RUB IT AND WISH")
    } else {
        ("THE GENIE'S LAMP", "NO WISH HELD")
    };
    let line = if mine > 0 { format!("{mine} WISH COMING") } else { String::new() };
    let w = text_width(head).max(text_width(key)).max(text_width(&line)) + 6;
    let h = if line.is_empty() { 21 } else { 30 };
    let px = super::in_view_x(x, w, view, cv.w);
    let py = (py - h).max(0);
    super::super::chrome::panel(cv, (px, py, w, h), super::super::chrome::Tone::KEY);
    cv.text(px + (w - text_width(head)) / 2, py + 3, head, 'i');
    cv.text(px + (w - text_width(key)) / 2, py + 12, key, 'O');
    if !line.is_empty() {
        cv.text(px + (w - text_width(&line)) / 2, py + 21, &line, '3');
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__altar__tests.rs"]
mod tests;
