//! Freedom wishes, drawn (`rules::freedom`). The safe room's brazier of the
//! genie's gold, burning teal while a knight there holds a freedom wish, and
//! along the south wall what the realm has wished into it: a stone hearth, a
//! steaming hot spring, a feast table, a war table under its map, a
//! featherbed, and curios painted as their words say. In the Sozzled Griffin
//! the weapons locker: a tall armoire, its doors thrown open on six bays, a
//! lamp on its crest. Every wished thing keeps a small gold plaque, read up
//! close; each keeps a little life of its own (a flame, steam, a spinning
//! wheel, a fuse's spark). A wish landing is the genie's own moment: his smoke
//! pours from the spot to the place, the thing shimmers into being in a
//! burst of gold, and he pops out over it to wave.

use super::super::chrome::{self, GOLD_RIM, Tone};
use super::super::ink::{Img, hash, text_width};
use super::{at, in_view_x};
use crate::render::genie::{self as genie_art, Mood};
use crate::rules::freedom::{self as f, Gift, Place, Upgrade};
use crate::rules::{Run, TILE_UNITS};

const TAU: f32 = std::f32::consts::TAU;

/// The weapons locker's face in native pixels: left, top, width, height.
fn locker_rect() -> (i32, i32, i32, i32) {
    let (x, y) = at(f::LOCKER_AT.0 * TILE_UNITS, f::LOCKER_AT.1 * TILE_UNITS);
    (x - 50, y - 60, 100, 60)
}

/// A bay of the locker, `i` from 0 (three across, two down): its inside.
fn bay_rect(i: usize) -> (i32, i32, i32, i32) {
    let (x0, y0, _, _) = locker_rect();
    let (c, r) = ((i % 3) as i32, (i / 3) as i32);
    (x0 + 7 + c * 29, y0 + 11 + r * 23, 28, 20)
}

/// Where the `i`th wish of `place` stands: its foot, in native pixels.
fn slot_px(run: &Run, place: Place, i: usize) -> (i32, i32) {
    match place {
        Place::Safe => {
            let (x, y) = f::slot_at(run.room(), i);
            at(x, y)
        }
        Place::Locker => {
            let (x, y, w, h) = bay_rect(i);
            (x + w / 2, y + h - 1)
        }
    }
}

/// Where the genie's smoke comes from at `place`: the brazier's bowl, or
/// the lamp on the locker's crest.
fn source_px(place: Place, spot: (f32, f32)) -> (i32, i32) {
    match place {
        Place::Safe => {
            let (x, y) = at(spot.0, spot.1);
            (x, y - 24)
        }
        Place::Locker => {
            let (x0, y0, w, _) = locker_rect();
            (x0 + w / 2 + 10, y0 - 9)
        }
    }
}

/// A knight here holds a freedom wish: the spot burns bright.
fn awake(run: &Run) -> bool {
    run.players.values().any(|h| !h.absent && h.hp > 0 && h.freedom_wishes > 0)
}

/// How far into its landing a wish is, when it is `place`'s `slot`.
fn landing(run: &Run, place: Place, slot: usize) -> Option<u32> {
    let l = run.freedom.landed.as_ref()?;
    (l.place == place && usize::from(l.slot) == slot).then(|| run.tick.saturating_sub(l.tick) as u32)
}

/// When a landing thing has shimmered in whole.
const REVEALED: u32 = 46;
/// When its shimmer begins.
const REVEAL_FROM: u32 = 14;

// ─── on the floor ────────────────────────────────────────────────────────────

/// The place's spot and what has been wished into it, on the floor.
pub(super) fn ground(cv: &mut Img, run: &Run) {
    let Some((place, spot)) = run.freedom_spot() else {
        return;
    };
    let tick = run.tick as u32;
    let wished = run.home.wished.of(place);
    match place {
        Place::Safe => {
            for (i, u) in wished.iter().enumerate() {
                let (x, y) = slot_px(run, place, i);
                drawn_in(cv, landing(run, place, i), (x, y), |im, x, y| safe_thing(im, u, x, y, tick + i as u32 * 37));
                plaque(cv, x, y + 2);
            }
            let (x, y) = at(spot.0, spot.1);
            let lit = if run.freedom.landed.is_some() { 3 } else if awake(run) { 2 } else { 1 };
            brazier(cv, x, y, tick, lit);
        }
        Place::Locker => locker(cv, run, wished, tick),
    }
}

/// Draw a thing with `paint` at its foot, shimmering into being while it
/// lands (`age` ticks in): a dissolve up from its foot, its newest pixels
/// white gold.
fn drawn_in(cv: &mut Img, age: Option<u32>, (x, y): (i32, i32), paint: impl Fn(&mut Img, i32, i32)) {
    let Some(age) = age.filter(|&a| a < REVEALED) else {
        paint(cv, x, y);
        return;
    };
    if age < REVEAL_FROM {
        return;
    }
    let (w, h) = (72, 72);
    let mut im = Img::new(w, h);
    paint(&mut im, w / 2, h - 8);
    let shown = ((age - REVEAL_FROM) * 100 / (REVEALED - REVEAL_FROM)) as i32;
    for yy in 0..h {
        for xx in 0..w {
            let Some(c) = im.get(xx, yy) else { continue };
            let rise = 100 - yy * 100 / h;
            let at = rise / 2 + (hash(xx, yy, 0xf5ee) % 40) as i32;
            let (px, py) = (x - w / 2 + xx, y - (h - 8) + yy);
            if at < shown - 6 {
                cv.set(px, py, c);
            } else if at < shown {
                cv.put(px, py, if (xx + yy + age as i32) % 3 == 0 { 'w' } else { '6' });
            }
        }
    }
}

/// The small gold plaque under a wished thing ("wished by", read up close
/// on its board).
fn plaque(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x - 6, y, 13, 4, 'a');
    cv.rect(x - 5, y, 11, 3, '4');
    cv.line(x - 5, y, x + 5, y, '5');
    cv.line(x - 3, y + 1, x + 3, y + 1, 'a');
    cv.put(x - 5, y, '6');
}

/// The genie's brazier: a tripod of gold, a bowl of coals, and his teal fire
/// over it. `lit`: 1 embers, 2 burning for a knight with a freedom wish, 3 a
/// wish landing.
fn brazier(cv: &mut Img, x: i32, y: i32, tick: u32, lit: u8) {
    // Its glow on the floor.
    let glow = i32::from(lit) * 7;
    for i in 0..glow {
        let a = i as f32 * 2.39996;
        let r = 9.0 + (i as f32 * 1.9) % (5.0 + f32::from(lit) * 4.0);
        if (i + tick as i32 / 6) % 3 == 0 {
            cv.put((x as f32 + a.cos() * r) as i32, (y as f32 + a.sin() * r * 0.4) as i32, if lit > 1 { '1' } else { '0' });
        }
    }
    cv.ellipse(x as f32 + 0.5, y as f32, 9.0, 2.2, 'K');
    // Three legs, splayed, with ball feet.
    for (fx, top) in [(-7, -3), (7, 3), (0, 0)] {
        cv.line(x + fx, y - 1, x + top, y - 12, if fx == 0 { '4' } else { 'a' });
        if fx != 0 {
            cv.line(x + fx + i32::signum(fx), y - 1, x + top + i32::signum(fx), y - 11, 'a');
        }
        cv.rect(x + fx - 1, y - 1, 2, 2, '4');
        cv.put(x + fx - 1, y - 1, '5');
    }
    // The stem's ring, and the bowl: dark gold under, lit gold over, a band
    // of studs, the lamp's mark on its face.
    cv.rect(x - 3, y - 14, 7, 2, '4');
    cv.line(x - 3, y - 14, x + 3, y - 14, '5');
    cv.ellipse(x as f32 + 0.5, y as f32 - 18.0, 9.5, 4.6, 'a');
    cv.ellipse(x as f32 + 0.5, y as f32 - 18.5, 8.5, 3.8, '4');
    cv.ellipse(x as f32 - 2.0, y as f32 - 19.5, 4.0, 1.6, '5');
    for k in -3..=3 {
        cv.put(x + k * 2, y - 17, if k == 0 { '6' } else { 'a' });
    }
    cv.line(x - 1, y - 16, x + 2, y - 16, '6');
    cv.put(x + 3, y - 17, '6');
    // The rim and the coals in it.
    cv.ellipse(x as f32 + 0.5, y as f32 - 22.0, 9.5, 2.2, '5');
    cv.ellipse(x as f32 + 0.5, y as f32 - 22.0, 8.0, 1.5, '8');
    for k in 0..6 {
        let cx = x - 6 + k * 2 + (hash(k, 3, 9) % 2) as i32;
        let hot = (tick / 7 + k as u32) % 4 == 0;
        cv.put(cx, y - 22, if hot { '@' } else { '7' });
    }
    cv.put(x - 8, y - 23, '6');
    // His fire: teal tongues over the coals, taller while it calls.
    let (tongues, tall) = match lit {
        1 => (2, 4),
        2 => (3, 14),
        _ => (3, 19),
    };
    for k in [0, 2, 1].into_iter().take(tongues as usize) {
        let fx = x - 4 + k * 4 + if tongues == 2 { 2 } else { 0 };
        let flick = (hash(k, (tick / 3 + k as u32) as i32, 0x9e) % 5) as i32;
        let h = tall - (k - 1).abs() * 4 + flick;
        teal_flame(cv, fx, y - 22, h, tick, k);
    }
    if lit > 1 {
        // Sparks lifting off, and a curl of his smoke.
        for k in 0..4u32 {
            let t = (tick + k * 17) % 40;
            let sx = x - 4 + (hash(k as i32, (tick / 40) as i32, 5) % 9) as i32 + ((t as f32 * 0.3 + k as f32).sin() * 2.0) as i32;
            let sy = y - 30 - t as i32 / 2;
            if t < 30 {
                cv.put(sx, sy, if t < 12 { 'w' } else { '3' });
            }
        }
        let rise = 16 + i32::from(lit) * 4;
        for i in 0..rise {
            let sway = ((tick as f32 * 0.15) - i as f32 * 0.5).sin() * (1.0 + i as f32 * 0.12);
            let (sx, sy) = (x + 1 + sway.round() as i32, y - 36 - i);
            if (i + tick as i32 / 4) % 5 != 0 {
                cv.put(sx, sy, if i % 3 == 0 { '2' } else { '1' });
            }
        }
    }
}

/// One tongue of the genie's teal fire, `h` tall, its foot at `(x, y)`: a
/// round root with a white heart, licking up to a wavering point, and now
/// and then a lick torn off its tip.
fn teal_flame(cv: &mut Img, x: i32, y: i32, h: i32, tick: u32, k: i32) {
    let h = h.max(2);
    for yy in 0..h {
        let t = yy as f32 / h as f32;
        let half = if t < 0.22 { 1.6 + t * 6.0 } else { 2.9 * ((1.0 - t) / 0.78).powf(1.25) };
        let lean = ((tick as f32 * 0.4 + k as f32 * 1.7) - yy as f32 * 0.5).sin() * t * t * 3.0;
        let r = half.round() as i32;
        for xx in -r..=r {
            let d = xx.abs() as f32 / half.max(0.6);
            let ink = if d < 0.5 && t < 0.45 {
                'w'
            } else if d < 0.7 && t < 0.75 {
                '3'
            } else if t < 0.9 && d < 1.0 {
                '2'
            } else {
                '1'
            };
            cv.put(x + xx + lean.round() as i32, y - yy, ink);
        }
    }
    // A lick torn off the tip, rising and going out.
    let t = (tick + k as u32 * 7) % 16;
    if t < 8 {
        let lean = ((tick as f32 * 0.4 + k as f32 * 1.7) - h as f32 * 0.5).sin() * 3.0;
        let (lx, ly) = (x + lean.round() as i32, y - h - 1 - t as i32 / 2);
        cv.put(lx, ly, if t < 4 { '3' } else { '2' });
        cv.put(lx, ly - 1, '1');
    }
}

// ─── the safe room's things ──────────────────────────────────────────────────

fn safe_thing(cv: &mut Img, u: &Upgrade, x: i32, y: i32, tick: u32) {
    match u.gift {
        Gift::Hearth => hearth(cv, x, y, tick),
        Gift::Spring => spring(cv, x, y, tick),
        Gift::Feast => feast(cv, x, y, tick),
        Gift::WarMap => war_table(cv, x, y, tick),
        Gift::Featherbed => featherbed(cv, x, y, tick),
        _ => curio(cv, &u.words, x, y, tick, true),
    }
}

/// A free-standing stone hearth: an arched firebox in coursed stone, a
/// mantel with a candle and a cup on it, a short chimney, logs ablaze, and
/// now and then a little heart rising from the fire.
fn hearth(cv: &mut Img, x: i32, y: i32, tick: u32) {
    // The hearthstone it stands on.
    cv.rect(x - 16, y - 3, 33, 4, 'u');
    cv.line(x - 16, y - 3, x + 16, y - 3, 'v');
    cv.line(x - 16, y, x + 16, y, 'S');
    // Coursed stone, lit from the left.
    let (l, top, w, h) = (x - 14, y - 29, 29, 26);
    for yy in 0..h {
        let course = yy / 5;
        let off = if course % 2 == 0 { 0 } else { 4 };
        for xx in 0..w {
            let block = (xx + off) / 7;
            let mortar = yy % 5 == 0 || (xx + off) % 7 == 0;
            let ink = if mortar {
                'g'
            } else if xx < 2 {
                'h'
            } else if xx > w - 3 {
                'j'
            } else {
                ['G', 'J', 'G', 'j'][(hash(block, course, 0x4ea) % 4) as usize]
            };
            cv.put(l + xx, top + yy, ink);
        }
    }
    // The chimney stack over it.
    cv.rect(x - 5, y - 41, 11, 10, 'G');
    cv.frame(x - 5, y - 41, 11, 10, 'g');
    cv.line(x - 5, y - 41, x + 5, y - 41, 'J');
    cv.line(x - 4, y - 37, x + 4, y - 37, 'g');
    cv.rect(x - 6, y - 43, 13, 2, 'j');
    // The arch and the dark of the firebox.
    cv.ellipse(x as f32 + 0.5, y as f32 - 15.0, 8.5, 6.0, 'U');
    cv.rect(x - 8, y - 15, 17, 12, 'U');
    cv.ellipse(x as f32 + 0.5, y as f32 - 14.0, 7.0, 5.0, 'k');
    cv.rect(x - 6, y - 14, 13, 11, 'k');
    cv.rect(x - 5, y - 12, 11, 8, 'n');
    // The logs, crossed.
    cv.rect(x - 6, y - 6, 13, 3, 'B');
    cv.line(x - 6, y - 6, x + 6, y - 6, 'r');
    cv.line(x - 4, y - 8, x + 4, y - 5, 'p');
    cv.line(x - 4, y - 9, x + 4, y - 6, 'R');
    // The fire.
    for k in 0..3 {
        let flick = (hash(k, (tick / 3) as i32, 0x4f1) % 4) as i32;
        fire_tongue(cv, x - 3 + k * 3, y - 7, 6 + flick - (k - 1).abs() * 2, tick, k);
    }
    // Its light spilling onto the hearthstone.
    for k in 0..5 {
        if (tick / 4 + k) % 3 != 0 {
            cv.put(x - 6 + k as i32 * 3, y - 2, if k % 2 == 0 { 'O' } else { 'o' });
        }
    }
    // The mantel, and what stands on it.
    cv.rect(x - 17, y - 31, 35, 3, 'P');
    cv.line(x - 17, y - 31, x + 17, y - 31, 'O');
    cv.line(x - 17, y - 29, x + 17, y - 29, 'n');
    cv.rect(x - 13, y - 36, 2, 5, 'T');
    cv.put(x - 13, y - 37, if (tick / 5) % 3 == 0 { 't' } else { 'O' });
    cv.put(x - 12, y - 38, if (tick / 5) % 3 == 1 { 'T' } else { 't' });
    cv.rect(x + 9, y - 34, 4, 3, 'o');
    cv.put(x + 13, y - 33, 'o');
    cv.line(x + 9, y - 34, x + 12, y - 34, 't');
    // A heart, now and then, rising out of the fire: it mends.
    let t = tick % 70;
    if t < 34 {
        let (hx, hy) = (x - 1 + ((t as f32 * 0.25).sin() * 2.0) as i32, y - 12 - t as i32);
        for (dx, dy) in [(-1, 0), (1, 0), (-2, 1), (0, 1), (2, 1), (-1, 2), (0, 2), (1, 2), (0, 3)] {
            if hy + dy < y - 31 || hy + dy > y - 14 {
                cv.put(hx + dx, hy + dy, if t < 20 { '7' } else { '8' });
            }
        }
    }
}

/// One tongue of an ordinary fire, `h` tall.
fn fire_tongue(cv: &mut Img, x: i32, y: i32, h: i32, tick: u32, k: i32) {
    let sway = ((tick / 5 + k as u32 * 3) % 4) as i32 - 1;
    for yy in 0..h.max(1) {
        let t = yy as f32 / h.max(1) as f32;
        let half = ((1.0 - t) * 2.4).round() as i32;
        let lean = (t * t * sway as f32 * 1.5) as i32;
        for xx in -half..=half {
            let core = xx.abs() * 2 < half.max(1);
            let ink = if t < 0.35 {
                if core { 'T' } else { 'O' }
            } else if core {
                't'
            } else {
                'R'
            };
            cv.put(x + xx + lean, y - yy, ink);
        }
    }
}

/// A hot spring: an oval pool in a ring of rounded stones, its water
/// rippling and its steam rising and drifting; a bucket and ladle beside.
fn spring(cv: &mut Img, x: i32, y: i32, tick: u32) {
    let (cx, cy) = (x as f32 + 0.5, y as f32 - 8.0);
    cv.ellipse(cx, cy + 1.5, 17.0, 7.5, 'S');
    cv.ellipse(cx, cy, 16.5, 7.0, 'u');
    // The ring of stones.
    for k in 0..18 {
        let a = k as f32 / 18.0 * TAU;
        let (sx, sy) = (cx + a.cos() * 14.5, cy + a.sin() * 5.8);
        let r = 1.8 + (hash(k, 1, 0x57) % 3) as f32 * 0.5;
        cv.ellipse(sx, sy, r + 0.6, r * 0.75, 'U');
        cv.ellipse(sx - 0.5, sy - 0.5, r * 0.6, r * 0.4, 'v');
    }
    // The water: deep in the middle, a lit band, glints.
    cv.ellipse(cx, cy, 12.0, 4.6, 'q');
    cv.ellipse(cx - 1.0, cy - 0.6, 10.0, 3.4, 'Q');
    cv.ellipse(cx - 2.0, cy - 1.2, 6.0, 1.6, 'z');
    let ring = (tick / 3 % 16) as f32;
    for i in 0..24 {
        let a = i as f32 / 24.0 * TAU;
        let r = 2.0 + ring * 0.55;
        if r < 10.5 && i % 2 == 0 {
            cv.put((cx + 3.0 + a.cos() * r) as i32, (cy + 0.5 + a.sin() * r * 0.38) as i32, if ring < 8.0 { 'W' } else { 'z' });
        }
    }
    for k in 0..3 {
        if (tick / 9 + k) % 3 == 0 {
            cv.put(x - 7 + k as i32 * 6, y - 9 + (k as i32 % 2), 'W');
        }
    }
    // Steam: soft puffs rising off the water, leaning and thinning.
    steam(cv, x - 7, y - 11, tick, 3, 7);
    // A bucket and its ladle on the rim.
    let (bx, by) = (x + 14, y - 3);
    cv.rect(bx - 2, by - 5, 6, 5, 'r');
    cv.line(bx - 2, by - 5, bx + 3, by - 5, 'o');
    cv.line(bx - 2, by - 3, bx + 3, by - 3, 'G');
    cv.line(bx + 1, by - 6, bx + 4, by - 10, 'o');
}

/// Steam rising from `(x, y)`: `n` columns `gap` apart, each a run of soft
/// puffs that grow, lean and thin as they climb.
fn steam(cv: &mut Img, x: i32, y: i32, tick: u32, n: i32, gap: i32) {
    for k in 0..n {
        for p in 0..3u32 {
            let t = (tick + p * 17 + k as u32 * 11) % 51;
            let tf = t as f32 / 51.0;
            let lean = ((tick as f32 * 0.04 + k as f32 * 1.9 + p as f32).sin() * 2.5 * tf) as i32;
            let (px, py) = (x + k * gap + lean, y - (tf * 22.0) as i32);
            let r = 1.2 + tf * 2.2;
            let ink = if tf < 0.35 { 'H' } else if tf < 0.7 { 'i' } else { 'h' };
            let (rx, ry) = (r.round() as i32, (r * 0.8).round() as i32);
            for dy in -ry..=ry {
                for dx in -rx..=rx {
                    let inside = (dx * dx) as f32 / (r * r) + (dy * dy) as f32 / (r * r * 0.64) <= 1.0;
                    // Thinner as it climbs: a sparser dither.
                    let keep = (hash(px + dx, py + dy, t) % 10) as f32 > tf * 9.0;
                    if inside && keep {
                        cv.put(px + dx, py + dy, ink);
                    }
                }
            }
        }
    }
}

/// A feast table: a trestle under a white cloth, a roast on its platter
/// steaming, a pie, a loaf, a bowl of apples, goblets, and two candles.
fn feast(cv: &mut Img, x: i32, y: i32, tick: u32) {
    // Legs, and the trestle's foot rail.
    for lx in [x - 13, x + 12] {
        cv.rect(lx, y - 9, 2, 9, 'B');
        cv.put(lx, y - 9, 'R');
    }
    cv.line(x - 12, y - 3, x + 12, y - 3, 'b');
    // The top, and the cloth over it falling down the front.
    cv.rect(x - 16, y - 16, 33, 6, 'o');
    cv.rect(x - 16, y - 16, 33, 5, 'H');
    cv.line(x - 16, y - 16, x + 16, y - 16, 'W');
    cv.rect(x - 16, y - 11, 33, 3, 'i');
    for k in 0..9 {
        cv.put(x - 15 + k * 4, y - 9, 'i');
        cv.put(x - 15 + k * 4, y - 8, 'h');
    }
    cv.line(x - 16, y - 11, x + 16, y - 11, 'h');
    // The roast on its platter, glazed, a bone showing.
    cv.ellipse(x as f32 - 2.0, y as f32 - 16.0, 7.0, 2.2, 'V');
    cv.ellipse(x as f32 - 2.0, y as f32 - 18.0, 5.0, 3.0, 'p');
    cv.ellipse(x as f32 - 2.5, y as f32 - 18.6, 4.0, 2.2, 'R');
    cv.ellipse(x as f32 - 3.5, y as f32 - 19.4, 1.8, 1.0, 'o');
    cv.put(x - 4, y - 20, 't');
    cv.line(x + 3, y - 20, x + 5, y - 22, 'H');
    cv.put(x + 6, y - 23, 'W');
    // Steam off the roast.
    steam(cv, x - 2, y - 21, tick + 5, 1, 0);
    // The pie.
    cv.ellipse(x as f32 + 10.0, y as f32 - 16.0, 4.5, 1.8, 'R');
    cv.ellipse(x as f32 + 10.0, y as f32 - 16.7, 4.0, 1.4, 'O');
    cv.put(x + 9, y - 17, 'R');
    cv.put(x + 11, y - 17, 'R');
    cv.put(x + 10, y - 16, 'R');
    // The loaf, and the apples in their bowl.
    cv.ellipse(x as f32 - 12.0, y as f32 - 16.5, 3.5, 1.8, 'O');
    cv.line(x - 13, y - 17, x - 11, y - 17, 't');
    cv.ellipse(x as f32 + 4.0, y as f32 - 14.0, 2.8, 1.2, 'r');
    for (dx, ink) in [(-1, '7'), (1, 'y'), (0, '7')] {
        cv.put(x + 4 + dx, y - 15 - i32::from(dx == 0), ink);
    }
    // Goblets, catching the light.
    for gx in [x - 8, x + 15] {
        cv.rect(gx, y - 20, 2, 2, 'O');
        cv.put(gx, y - 20, 't');
        cv.line(gx, y - 18, gx + 1, y - 18, 'r');
        cv.put(gx, y - 17, 'O');
    }
    // Two candles, flickering.
    for (k, cx) in [x - 15, x + 13].into_iter().enumerate() {
        cv.rect(cx, y - 22, 1, 5, 'T');
        let flick = (tick / 4 + k as u32 * 2) % 3;
        cv.put(cx, y - 23, if flick == 0 { 'O' } else { 't' });
        cv.put(cx, y - 24 - i32::from(flick == 1), 'T');
    }
}

/// A war table: a parchment map pinned flat on a stout table, a river and
/// woods inked on it, a red route marching across to a flag that flutters,
/// little pins standing in it, a compass in its corner.
fn war_table(cv: &mut Img, x: i32, y: i32, tick: u32) {
    // The table: its legs and front, its top.
    for lx in [x - 14, x + 13] {
        cv.rect(lx, y - 8, 2, 8, 'B');
    }
    cv.rect(x - 16, y - 11, 33, 3, 'R');
    cv.line(x - 16, y - 9, x + 16, y - 9, 'B');
    cv.rect(x - 16, y - 22, 33, 11, 'r');
    cv.line(x - 16, y - 22, x + 16, y - 22, 'o');
    // The parchment on it, curling at its corners.
    let (mx, my, mw, mh) = (x - 14, y - 21, 29, 10);
    cv.rect(mx, my, mw, mh, 'T');
    cv.frame(mx, my, mw, mh, 't');
    cv.put(mx, my, 'o');
    cv.put(mx + mw - 1, my + mh - 1, 'o');
    // A river winding through, the woods, the hills.
    for k in 0..mw - 2 {
        let ry = my + 4 + ((k as f32 * 0.5).sin() * 2.0) as i32;
        cv.put(mx + 1 + k, ry, 'Q');
    }
    for (dx, dy) in [(3, 2), (5, 3), (4, 1), (20, 6), (22, 7), (21, 5)] {
        cv.put(mx + dx, my + dy, 'l');
    }
    for (hx, hy) in [(10, 8), (14, 8)] {
        cv.put(mx + hx, my + hy - 1, 'P');
        cv.put(mx + hx - 1, my + hy, 'P');
        cv.put(mx + hx + 1, my + hy, 'P');
    }
    // The route, marching: red dashes toward the flag.
    let march = (tick / 6 % 3) as i32;
    for k in 0..12 {
        if (k + march) % 3 != 0 {
            cv.put(mx + 3 + k * 2, my + 7 - k / 3, '7');
        }
    }
    // The flag at its end, fluttering, and pins standing in the map.
    let (fx, fy) = (mx + mw - 4, my + 2);
    cv.line(fx, fy - 6, fx, fy + 1, 'k');
    let flap = (tick / 7 % 2) as i32;
    cv.rect(fx + 1, fy - 6, 4, 2, '7');
    cv.put(fx + 5, fy - 5 + flap, '7');
    cv.put(fx + 1, fy - 4, '8');
    for (k, (px, py)) in [(6, 3), (12, 6), (17, 2)].into_iter().enumerate() {
        let bob = ((tick / 10 + k as u32) % 6 == 0) as i32;
        cv.line(mx + px, my + py - 3 - bob, mx + px, my + py, 'j');
        cv.put(mx + px, my + py - 4 - bob, ['7', 'Q', 'O'][k]);
    }
    // The compass rose in the corner, its needle glinting.
    let (cx, cy) = (mx + 3, my + mh - 3);
    cv.put(cx, cy - 1, 'n');
    cv.put(cx, cy + 1, 'n');
    cv.put(cx - 1, cy, 'n');
    cv.put(cx + 1, cy, 'n');
    cv.put(cx, cy, if (tick / 12) % 4 == 0 { 'W' } else { 'o' });
    // A spyglass on the table's edge.
    cv.line(x + 3, y - 10, x + 10, y - 12, 'O');
    cv.put(x + 10, y - 12, 't');
}

/// A featherbed: a low timber frame, its headboard up at the west end, a
/// fat pillow, a patchwork quilt, and sleep rising off it in little Zs.
fn featherbed(cv: &mut Img, x: i32, y: i32, tick: u32) {
    // The frame's front and feet.
    cv.rect(x - 15, y - 7, 31, 5, 'R');
    cv.line(x - 15, y - 7, x + 15, y - 7, 'o');
    cv.line(x - 15, y - 3, x + 15, y - 3, 'B');
    for fx in [x - 15, x + 14] {
        cv.rect(fx, y - 3, 2, 3, 'B');
    }
    // The mattress and the quilt over most of it, in patches.
    cv.rect(x - 13, y - 15, 28, 8, 'H');
    cv.line(x - 13, y - 15, x + 14, y - 15, 'W');
    for py in 0..7 {
        for px in 0..20 {
            let ink = match (px / 5 + py / 4) % 3 {
                0 => 'Q',
                1 => 'z',
                _ => 'T',
            };
            cv.put(x - 5 + px, y - 14 + py, if py == 6 { 'q' } else { ink });
        }
    }
    cv.line(x - 5, y - 14, x + 14, y - 14, 'W');
    for k in 0..4 {
        cv.put(x - 4 + k * 5, y - 11, 'W');
    }
    // The pillow, plumped.
    cv.ellipse(x as f32 - 9.5, y as f32 - 13.5, 4.5, 2.6, 'V');
    cv.ellipse(x as f32 - 10.0, y as f32 - 14.0, 3.5, 1.8, 'W');
    // The headboard at the west end, and the footboard low at the east.
    cv.rect(x - 17, y - 21, 3, 19, 'P');
    cv.line(x - 17, y - 21, x - 17, y - 3, 'O');
    cv.ellipse(x as f32 - 15.5, y as f32 - 21.0, 2.0, 1.5, 'o');
    cv.rect(x + 15, y - 12, 2, 10, 'P');
    cv.put(x + 15, y - 13, 'o');
    // Sleep, rising.
    for k in 0..3u32 {
        let t = (tick + k * 26) % 78;
        if t > 60 {
            continue;
        }
        let (zx, zy) = (x - 6 + (k as i32) * 3 + ((t as f32 * 0.1).sin() * 3.0) as i32, y - 18 - t as i32 / 3);
        let s = 2 + i32::from(t > 30);
        let ink = if t < 40 { 'H' } else { 'h' };
        cv.line(zx, zy, zx + s, zy, ink);
        cv.line(zx + s, zy, zx, zy + s, ink);
        cv.line(zx, zy + s, zx + s, zy + s, ink);
    }
}

// ─── curios ──────────────────────────────────────────────────────────────────

/// What a curio looks like, read from its words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Shape {
    Statue,
    Plant,
    Banner,
    Crystal,
    Fishbowl,
    Trophy,
    Candles,
    Harp,
    Critter,
    Bird,
    Urn,
}

/// The shape and the colours a curio's words ask for: a word of a family
/// names it, a colour word paints it, and the words' own hash picks for
/// words that say neither.
pub(super) fn curio_look(words: &str) -> (Shape, [char; 4]) {
    const SHAPES: [(Shape, &[&str]); 11] = [
        (Shape::Statue, &["statue", "bust", "idol", "sculpture", "gargoyle", "figure", "effigy", "carving", "knight", "king", "queen"]),
        (Shape::Plant, &["plant", "tree", "flower", "fern", "rose", "bonsai", "vine", "moss", "herb", "cactus", "ivy", "garden", "lily", "tulip"]),
        (Shape::Banner, &["banner", "flag", "tapestry", "rug", "carpet", "curtain", "painting", "portrait", "picture", "poster", "pennant"]),
        (Shape::Crystal, &["crystal", "gem", "jewel", "diamond", "orb", "ruby", "emerald", "sapphire", "geode", "star", "moon", "sun"]),
        (Shape::Fishbowl, &["fish", "aquarium", "goldfish", "koi", "bowl", "tank"]),
        (Shape::Trophy, &["trophy", "cup", "chalice", "grail", "crown", "treasure", "coin", "hoard", "medal"]),
        (Shape::Candles, &["candle", "lamp", "lantern", "chandelier", "light", "torch", "candelabra"]),
        (Shape::Harp, &["harp", "lute", "music", "piano", "organ", "drum", "bell", "song", "jukebox"]),
        (Shape::Bird, &["owl", "parrot", "bird", "raven", "crow", "hawk", "falcon", "eagle", "dove", "pigeon", "chicken", "duck", "goose", "phoenix"]),
        (Shape::Critter, &["cat", "dog", "frog", "dragon", "pet", "kitten", "puppy", "toad", "bear", "fox", "rabbit", "mouse", "rat", "pig", "wolf", "lion"]),
        (Shape::Urn, &["urn", "vase", "pot", "jar", "amphora", "jug", "vessel"]),
    ];
    const COLOURS: [(&[&str], [char; 4]); 13] = [
        (&["gold", "golden", "brass"], ['a', '4', '5', '6']),
        (&["silver", "steel", "iron", "white", "pearl", "marble"], ['G', 'h', 'i', 'W']),
        (&["pink", "rose", "magenta", "fuchsia", "blush", "flamingo"], ['?', ';', '%', '+']),
        (&["purple", "violet", "lilac", "lavender", "plum", "amethyst", "grape"], ['(', ')', ']', '^']),
        (&["red", "ruby", "crimson", "scarlet"], ['8', '7', 'R', 't']),
        (&["blue", "sapphire", "azure", "sky"], ['q', 'Q', 'z', 'W']),
        (&["green", "emerald", "jade", "leaf", "forest"], ['E', 'l', 'A', 'Y']),
        (&["teal", "turquoise", "aqua", "magic", "genie"], ['0', '1', '2', '3']),
        (&["wood", "wooden", "oak", "brown", "bronze", "copper"], ['b', 'B', 'R', 'o']),
        (&["stone", "granite", "rock", "grey", "gray"], ['S', 'u', 'U', 'v']),
        (&["black", "obsidian", "shadow", "dark", "night"], ['K', 'X', 'g', 'j']),
        (&["orange", "amber", "honey", "fire"], ['p', 'R', 'O', 't']),
        (&["cream", "ivory", "bone", "sand"], ['o', 'O', 't', 'T']),
    ];
    let lower = words.to_lowercase();
    let toks: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).collect();
    let said = |list: &[&str]| toks.iter().any(|t| list.iter().any(|w| t == w || t.strip_suffix('s') == Some(*w)));
    let seed = words.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let shape = SHAPES
        .iter()
        .find(|(_, list)| said(list))
        .map_or_else(|| [Shape::Urn, Shape::Statue, Shape::Crystal, Shape::Plant, Shape::Trophy][(seed % 5) as usize], |s| s.0);
    let colours = COLOURS.iter().find(|(list, _)| said(list)).map_or_else(
        || [['a', '4', '5', '6'], ['q', 'Q', 'z', 'W'], ['E', 'l', 'A', 'Y'], ['8', '7', 'R', 't'], ['G', 'h', 'i', 'W']][((seed >> 8) % 5) as usize],
        |c| c.1,
    );
    (shape, colours)
}

/// A curio, painted from its words on a little plinth (`big`: the safe
/// room's size; else a locker bay's).
fn curio(cv: &mut Img, words: &str, x: i32, y: i32, tick: u32, big: bool) {
    let (shape, [dark, mid, lit, glint]) = curio_look(words);
    let k = if big { 1.0 } else { 0.7 };
    let p = |n: f32| (n * k).round() as i32;
    // The plinth.
    let (pw, ph) = (p(14.0), p(7.0));
    cv.rect(x - pw / 2, y - ph, pw + 1, ph, 'u');
    cv.line(x - pw / 2, y - ph, x + pw / 2, y - ph, 'v');
    cv.line(x - pw / 2 - 1, y - 1, x + pw / 2 + 1, y - 1, 'S');
    cv.line(x + pw / 2, y - ph, x + pw / 2, y - 1, 'S');
    let top = y - ph;
    let fx = x as f32 + 0.5;
    match shape {
        Shape::Statue => {
            let subject = curio_look(&words.to_lowercase().replace("statue", "").replace("bust", "")).0;
            if matches!(subject, Shape::Critter | Shape::Bird) {
                // A statue of the creature its words name, on a taller socle.
                cv.rect(x - p(5.0), top - p(3.0), p(10.0) + 1, p(3.0), dark);
                cv.line(x - p(5.0), top - p(3.0), x + p(5.0), top - p(3.0), lit);
                creature(cv, subject, x, top - p(3.0), tick, k, [dark, mid, lit, glint], false);
            } else {
                bust(cv, x, top, k, [dark, mid, lit, glint]);
            }
            if (tick / 9) % 8 == 0 {
                cv.put(x - p(3.0), top - p(12.0), 'w');
            }
        }
        Shape::Plant => {
            cv.rect(x - p(4.0), top - p(6.0), p(8.0) + 1, p(6.0), 'R');
            cv.line(x - p(4.0), top - p(6.0), x + p(4.0), top - p(6.0), 'o');
            for (i, (lx, ly)) in [(-6.0, -14.0), (5.0, -16.0), (-3.0, -20.0), (3.0, -22.0), (0.0, -12.0), (-7.0, -19.0), (7.0, -21.0)]
                .into_iter()
                .enumerate()
            {
                let sway = ((tick as f32 * 0.05 + i as f32).sin() * 0.8) as i32;
                let (lx, ly) = (x + p(lx) + sway, top + p(ly));
                cv.line(x, top - p(6.0), lx, ly + 2, 'l');
                cv.ellipse(lx as f32, ly as f32, 2.6 * k, 1.6 * k, if i % 2 == 0 { 'A' } else { 'l' });
                cv.put(lx - 1, ly - 1, 'Y');
            }
            for (i, (bx, by)) in [(-5.0, -17.0), (4.0, -19.0), (0.0, -24.0)].into_iter().enumerate() {
                let open = (tick / 20 + i as u32) % 5 != 0;
                cv.put(x + p(bx), top + p(by), if open { lit } else { mid });
                cv.put(x + p(bx) + 1, top + p(by), mid);
            }
        }
        Shape::Banner => {
            let (bw, bh) = (p(14.0), p(18.0));
            cv.line(x - bw / 2 - 1, top - bh - 2, x + bw / 2 + 1, top - bh - 2, 'o');
            cv.line(x - bw / 2 - 1, top - bh - 2, x - bw / 2 - 1, top, 'B');
            let wave = (tick / 8 % 2) as i32;
            for yy in 0..bh {
                for xx in 0..bw {
                    let edge = xx == 0 || xx == bw - 1;
                    let ink = if edge { dark } else if (xx + yy / 3) % 5 == 0 { lit } else { mid };
                    cv.put(x - bw / 2 + xx + if yy > bh / 2 { wave } else { 0 }, top - bh - 1 + yy, ink);
                }
            }
            for xx in (0..bw).step_by(3) {
                cv.put(x - bw / 2 + xx + wave, top - 1, glint);
            }
            cv.ellipse(fx, (top - bh / 2 - 1) as f32, 3.0 * k, 3.0 * k, glint);
            cv.ellipse(fx, (top - bh / 2 - 1) as f32, 1.6 * k, 1.6 * k, dark);
        }
        Shape::Crystal => {
            let bob = ((tick as f32 * 0.08).sin() * 1.5).round() as i32;
            let cy = top - p(13.0) + bob;
            for yy in -p(9.0)..=p(9.0) {
                let half = (p(6.0) - yy.abs() * 2 / 3).max(0);
                for xx in -half..=half {
                    let ink = if xx < -half / 2 { lit } else if xx > half / 2 { dark } else { mid };
                    cv.put(x + xx, cy + yy, ink);
                }
            }
            cv.line(x - 1, cy - p(6.0), x - 3, cy + 2, glint);
            if (tick / 8) % 5 == 0 {
                cv.put(x - 2, cy - 3, 'w');
                cv.put(x - 3, cy - 3, glint);
                cv.put(x - 1, cy - 3, glint);
            }
            cv.ellipse(fx, (top - 1) as f32, 4.0 * k, 1.2, dark);
        }
        Shape::Fishbowl => {
            let cy = (top - p(9.0)) as f32;
            cv.ellipse(fx, cy, 8.0 * k, 8.0 * k, 'h');
            cv.ellipse(fx, cy + 1.0, 7.0 * k, 6.5 * k, 'q');
            cv.ellipse(fx, cy + 0.5, 6.5 * k, 4.0 * k, 'Q');
            cv.line(x - p(6.0), cy as i32 - p(4.0), x + p(6.0), cy as i32 - p(4.0), 'z');
            let swim = ((tick as f32 * 0.06).sin() * 4.0 * k) as i32;
            let face = if (tick / 100) % 2 == 0 { 1 } else { -1 };
            let (sx, sy) = (x + swim, cy as i32 + 1);
            cv.ellipse(sx as f32 + 0.5, sy as f32, 2.4, 1.4, mid);
            cv.put(sx - face * 3, sy, lit);
            cv.put(sx - face * 3, sy - 1, lit);
            cv.put(sx + face, sy, 'k');
            cv.put(x - p(4.0), cy as i32 - p(5.0), 'W');
            if (tick / 6) % 4 == 0 {
                cv.put(x + p(2.0), cy as i32 - p(2.0) - (tick / 6 % 3) as i32, 'W');
            }
        }
        Shape::Trophy => {
            cv.rect(x - p(4.0), top - p(3.0), p(8.0) + 1, p(3.0), dark);
            cv.line(x, top - p(9.0), x, top - p(3.0), mid);
            cv.ellipse(fx, (top - p(15.0)) as f32, 6.0 * k, 5.5 * k, mid);
            cv.rect(x - p(6.0), top - p(21.0), p(12.0) + 1, p(6.0), mid);
            cv.ellipse(fx - 2.0 * k, (top - p(16.0)) as f32, 2.0 * k, 3.0 * k, lit);
            cv.line(x - p(6.0), top - p(21.0), x + p(6.0), top - p(21.0), lit);
            for side in [-1.0f32, 1.0] {
                cv.ellipse(fx + side * 7.5 * k, (top - p(16.0)) as f32, 2.2 * k, 3.0 * k, dark);
                cv.ellipse(fx + side * 7.5 * k, (top - p(16.0)) as f32, 1.0 * k, 1.8 * k, 'K');
            }
            cv.put(x - p(3.0), top - p(18.0), if (tick / 7) % 6 == 0 { 'w' } else { glint });
        }
        Shape::Candles => {
            cv.rect(x - p(5.0), top - p(3.0), p(10.0) + 1, p(3.0), dark);
            cv.line(x, top - p(14.0), x, top - p(3.0), mid);
            cv.line(x - p(6.0), top - p(12.0), x + p(6.0), top - p(12.0), mid);
            for (k2, cx) in [x - p(6.0), x, x + p(6.0)].into_iter().enumerate() {
                let tall = if k2 == 1 { p(8.0) } else { p(6.0) };
                cv.rect(cx, top - p(12.0) - tall, 2, tall, 'T');
                let flick = (tick / 4 + k2 as u32) % 3;
                cv.put(cx, top - p(13.0) - tall, if flick == 0 { 'O' } else { 't' });
                cv.put(cx + i32::from(flick == 2), top - p(14.0) - tall, 'T');
            }
        }
        Shape::Harp => {
            let (bx, by) = (x - p(5.0), top - 1);
            cv.line(bx, by, bx, by - p(20.0), dark);
            cv.line(bx + 1, by, bx + 1, by - p(20.0), mid);
            for i in 0..p(12.0) {
                let cy = by - p(20.0) - ((i as f32 / p(12.0) as f32 * std::f32::consts::PI).sin() * 3.0) as i32 + i / 3;
                cv.put(bx + 1 + i, cy, mid);
                cv.put(bx + 1 + i, cy + 1, lit);
            }
            cv.line(bx, by, bx + p(12.0), by - p(14.0), mid);
            for s in 0..5 {
                let sx = bx + 2 + s * p(2.0);
                let pluck = (tick / 9) as i32 % 5 == s;
                cv.line(sx, by - 1 - s * p(2.0) / 2, sx, by - p(17.0) + s, if pluck { 'W' } else { 'h' });
            }
        }
        Shape::Critter | Shape::Bird => creature(cv, shape, x, top, tick, k, [dark, mid, lit, glint], true),
        Shape::Urn => {
            cv.rect(x - p(3.0), top - p(3.0), p(6.0) + 1, p(3.0), dark);
            cv.ellipse(fx, (top - p(10.0)) as f32, 6.0 * k, 7.0 * k, mid);
            cv.ellipse(fx - 2.0 * k, (top - p(11.0)) as f32, 2.5 * k, 4.0 * k, lit);
            cv.rect(x - p(3.0), top - p(19.0), p(6.0) + 1, p(3.0), mid);
            cv.line(x - p(4.0), top - p(19.0), x + p(4.0), top - p(19.0), lit);
            cv.line(x - p(6.0), top - p(10.0), x + p(6.0), top - p(10.0), dark);
            if (tick / 9) % 7 == 0 {
                cv.put(x - p(3.0), top - p(13.0), glint);
            }
        }
    }
}

/// A carved bust on its socle: broad shoulders, a neck, a head with its
/// brow and nose, a laurel round it.
fn bust(cv: &mut Img, x: i32, top: i32, k: f32, [dark, mid, lit, glint]: [char; 4]) {
    let p = |n: f32| (n * k).round() as i32;
    cv.rect(x - p(3.0), top - p(3.0), p(6.0) + 1, p(3.0), dark);
    // The shoulders and chest, a fold of drapery across.
    for yy in 0..p(8.0) {
        let half = p(4.0) + yy * p(4.0) / p(8.0).max(1);
        for xx in -half..=half {
            let ink = if xx < -half / 3 { lit } else if xx > half / 2 { dark } else { mid };
            cv.put(x + xx, top - p(3.0) - p(8.0) + yy, ink);
        }
    }
    cv.line(x - p(6.0), top - p(7.0), x + p(3.0), top - p(10.0), dark);
    // The neck, and the head.
    cv.rect(x - p(1.5), top - p(14.0), p(3.0) + 1, p(3.0), mid);
    let (hx, hy) = (x as f32 + 0.5, (top - p(18.0)) as f32);
    cv.ellipse(hx, hy, 3.8 * k, 4.4 * k, mid);
    cv.ellipse(hx - 1.2 * k, hy - 0.8 * k, 2.2 * k, 2.8 * k, lit);
    cv.line(x - p(2.0), hy as i32 - 1, x + p(2.0), hy as i32 - 1, dark);
    cv.put(x + 1, hy as i32 + 1, dark);
    cv.put(x, hy as i32 + p(2.5), dark);
    // The laurel.
    for i in 0..7 {
        let a = std::f32::consts::PI * (0.15 + i as f32 / 7.0 * 0.7);
        cv.put(x - (a.cos() * 4.2 * k) as i32, hy as i32 - (a.sin() * 4.6 * k) as i32, if i % 2 == 0 { 'A' } else { 'l' });
    }
    cv.put(x - p(2.0), hy as i32 - p(4.0), glint);
}

/// A creature, curled on a cushion (`cushion`) or carved on its socle: a
/// cat, a dog, a frog curled up with its tail flicking, or a bird perched,
/// blinking.
#[allow(clippy::too_many_arguments)]
fn creature(cv: &mut Img, shape: Shape, x: i32, top: i32, tick: u32, k: f32, [dark, mid, lit, glint]: [char; 4], cushion: bool) {
    let p = |n: f32| (n * k).round() as i32;
    let fx = x as f32 + 0.5;
    let blink = (tick / 4) % 30 == 0;
    let base = if cushion {
        cv.ellipse(fx, (top - 2) as f32, 7.0 * k, 2.5 * k, '7');
        cv.ellipse(fx, (top - 3) as f32, 6.0 * k, 1.8 * k, 'R');
        cv.put(x - p(6.0), top - 2, '6');
        cv.put(x + p(6.0), top - 2, '6');
        top - 3
    } else {
        top
    };
    if shape == Shape::Bird {
        // On its perch: a round body, a pale breast, ear tufts, big eyes.
        cv.line(x - p(6.0), base - 1, x + p(6.0), base - 1, 'P');
        let (bx, by) = (fx, (base - p(8.0)) as f32);
        cv.ellipse(bx, by, 4.5 * k, 6.0 * k, mid);
        cv.ellipse(bx, by + 1.5 * k, 2.8 * k, 4.0 * k, lit);
        for (dy, dx) in [(0.0, -4.0), (2.0, -4.5), (0.0, 4.0), (2.0, 4.5)] {
            cv.put(x + p(dx), by as i32 + p(dy), dark);
        }
        let hy = by as i32 - p(5.0);
        cv.put(x - p(3.0), hy - 2, dark);
        cv.put(x + p(3.0), hy - 2, dark);
        for ex in [x - p(1.6), x + p(1.6) + 1] {
            cv.put(ex, hy, if blink { mid } else { glint });
            cv.put(ex, hy + 1, if blink { mid } else { 'k' });
        }
        cv.put(x, hy + 2, 'O');
        cv.put(x - 1, base - 1, 'O');
        cv.put(x + 1, base - 1, 'O');
        return;
    }
    cv.ellipse(fx, (base - p(4.0)) as f32, 5.5 * k, 4.0 * k, mid);
    cv.ellipse(fx - 1.0, (base - p(5.0)) as f32, 3.5 * k, 2.4 * k, lit);
    let (hx, hy) = (x + p(4.0), base - p(8.0));
    cv.ellipse(hx as f32, hy as f32, 3.2 * k, 2.8 * k, mid);
    cv.ellipse(hx as f32 - 0.6, hy as f32 - 0.6, 1.8 * k, 1.4 * k, lit);
    cv.put(hx - 2, hy - 3, mid);
    cv.put(hx + 2, hy - 3, mid);
    cv.put(hx - 2, hy - 2, dark);
    cv.put(hx + 2, hy - 2, dark);
    cv.put(hx - 1, hy, if blink { mid } else { 'k' });
    cv.put(hx + 1, hy, if blink { mid } else { 'k' });
    cv.put(hx, hy + 1, dark);
    let flick = ((tick as f32 * 0.2).sin() * 2.0) as i32;
    cv.line(x - p(5.0), base - p(2.0), x - p(8.0), base - p(6.0) + flick, dark);
    cv.put(x - p(8.0), base - p(6.0) + flick - 1, mid);
}

// ─── the weapons locker ──────────────────────────────────────────────────────

/// The weapons locker: a tall armoire of dark oak, its doors thrown open on
/// six bays, brass at its hinges, and on its crest the genie's lamp on a
/// gold cartouche, smoking teal while a knight here holds a freedom wish.
fn locker(cv: &mut Img, run: &Run, wished: &[Upgrade], tick: u32) {
    let (x0, y0, w, h) = locker_rect();
    let lit = if run.freedom.landed.is_some() { 3 } else if awake(run) { 2 } else { 1 };
    // The shadow on the floor, and the plinth.
    cv.ellipse((x0 + w / 2) as f32, (y0 + h) as f32, (w / 2 + 6) as f32, 3.0, 'K');
    cv.rect(x0 - 2, y0 + h - 5, w + 4, 5, 'B');
    cv.line(x0 - 2, y0 + h - 5, x0 + w + 1, y0 + h - 5, 'R');
    cv.line(x0 - 2, y0 + h - 1, x0 + w + 1, y0 + h - 1, 'n');
    // The doors, thrown open to either side: planked, brass-hinged.
    for (side, dx) in [(-1, x0 - 6), (1, x0 + w)] {
        let (hinge, edge) = if side < 0 { (dx + 5, dx) } else { (dx, dx + 5) };
        cv.rect(dx, y0 + 9, 6, h - 15, 'P');
        for yy in (y0 + 11..y0 + h - 7).step_by(4) {
            cv.line(dx, yy, dx + 5, yy, 'r');
        }
        cv.line(edge, y0 + 9, edge, y0 + h - 7, 'O');
        for hy in [y0 + 13, y0 + h - 12] {
            cv.rect(hinge - 1, hy, 2, 3, 'O');
            cv.put(hinge - 1, hy, 't');
        }
        cv.put(if side < 0 { dx + 1 } else { dx + 4 }, y0 + h / 2 + 3, 't');
    }
    // The carcass: sides, the dark back of each bay, the shelves.
    cv.rect(x0, y0 + 8, w, h - 13, 'B');
    cv.rect(x0 + 2, y0 + 8, 2, h - 13, 'R');
    cv.rect(x0 + w - 4, y0 + 8, 2, h - 13, 'p');
    for i in 0..f::MOST_PER_PLACE {
        let (bx, by, bw, bh) = bay_rect(i);
        cv.rect(bx, by, bw, bh, 'n');
        for py in (by + 2..by + bh).step_by(5) {
            cv.line(bx, py, bx + bw - 1, py, 'b');
        }
        cv.line(bx, by, bx + bw - 1, by, 'k');
        // Its shelf, and the brass tag on its lip.
        cv.rect(bx - 1, by + bh, bw + 2, 2, 'P');
        cv.line(bx - 1, by + bh, bx + bw, by + bh, 'O');
        let filled = i < wished.len();
        cv.rect(bx + bw / 2 - 3, by + bh + 1, 7, 2, if filled { '4' } else { 'r' });
        if filled {
            cv.put(bx + bw / 2 - 3, by + bh + 1, '6');
        } else {
            // An empty bay: two pegs waiting.
            for px in [bx + 8, bx + bw - 9] {
                cv.rect(px, by + 5, 2, 2, 'r');
                cv.put(px, by + 5, 'o');
            }
        }
    }
    // The uprights between the bays.
    for c in 1..3 {
        let (bx, _, _, _) = bay_rect(c);
        cv.rect(bx - 2, y0 + 9, 1, h - 15, 'P');
    }
    // The crown: a moulded cornice, and the gold cartouche with the lamp.
    cv.rect(x0 - 4, y0 + 2, w + 8, 7, 'R');
    cv.line(x0 - 4, y0 + 2, x0 + w + 3, y0 + 2, 'o');
    cv.line(x0 - 3, y0 + 4, x0 + w + 2, y0 + 4, 'P');
    cv.line(x0 - 4, y0 + 8, x0 + w + 3, y0 + 8, 'n');
    for k in 0..(w + 8) / 6 {
        cv.put(x0 - 2 + k * 6, y0 + 6, 'O');
    }
    let cx = x0 + w / 2;
    cv.ellipse(cx as f32 + 0.5, y0 as f32 + 3.0, 13.0, 5.5, 'a');
    cv.ellipse(cx as f32 + 0.5, y0 as f32 + 3.0, 12.0, 4.6, '4');
    cv.ellipse(cx as f32 - 2.0, y0 as f32 + 1.5, 7.0, 1.6, '5');
    // Crossed blades engraved on it.
    cv.line(cx - 6, y0 + 6, cx + 6, y0, 'a');
    cv.line(cx - 6, y0, cx + 6, y0 + 6, 'a');
    cv.put(cx, y0 + 3, '6');
    // The lamp on the crest: its foot, its belly lit along the top, the
    // long spout curling up, the handle's loop, the lid and its knob.
    let (lx, ly) = (cx, y0 - 3);
    cv.rect(lx - 2, ly + 1, 5, 2, 'a');
    cv.line(lx - 3, ly + 2, lx + 3, ly + 2, '4');
    cv.ellipse(lx as f32 + 0.5, ly as f32 - 1.0, 6.0, 2.4, 'a');
    cv.ellipse(lx as f32 + 0.5, ly as f32 - 1.5, 5.0, 1.8, '4');
    cv.line(lx - 3, ly - 3, lx + 3, ly - 3, '5');
    cv.put(lx - 2, ly - 2, '6');
    cv.line(lx + 5, ly - 1, lx + 9, ly - 3, '4');
    cv.line(lx + 9, ly - 3, lx + 10, ly - 5, '5');
    cv.put(lx + 10, ly - 6, '6');
    for (dx, dy) in [(-6, -1), (-7, -2), (-7, -3), (-6, -4), (-5, -4)] {
        cv.put(lx + dx, ly + dy, 'a');
    }
    cv.rect(lx - 1, ly - 5, 3, 2, '4');
    cv.put(lx, ly - 6, '6');
    if (tick / 10) % 6 == 0 {
        cv.put(lx - 1, ly - 2, 'w');
    }
    let (lx, ly) = (lx + 2, ly - 2);
    if lit > 1 {
        let rise = 10 + i32::from(lit) * 4;
        for i in 0..rise {
            let sway = ((tick as f32 * 0.15) - i as f32 * 0.5).sin() * (1.0 + i as f32 * 0.15);
            let (sx, sy) = (lx + 9 + sway.round() as i32, ly - 4 - i);
            if (i + tick as i32 / 4) % 5 != 0 {
                cv.put(sx, sy, if i % 3 == 0 { '3' } else { '2' });
            }
        }
        // His glow in the bays still to be filled.
        for i in wished.len()..f::MOST_PER_PLACE {
            let (bx, by, bw, bh) = bay_rect(i);
            if (tick / 10 + i as u32) % 4 == 0 {
                cv.put(bx + bw / 2, by + bh / 2, '1');
            }
        }
    }
    // What has been wished into it, each in its bay.
    for (i, u) in wished.iter().enumerate() {
        let (bx, by, bw, bh) = bay_rect(i);
        drawn_in(cv, landing(run, Place::Locker, i), (bx + bw / 2, by + bh - 1), |im, x, y| {
            locker_thing(im, u, x, y, tick + i as u32 * 29)
        });
    }
}

fn locker_thing(cv: &mut Img, u: &Upgrade, x: i32, y: i32, tick: u32) {
    match u.gift {
        Gift::Whetstone => whetstone(cv, x, y, tick),
        Gift::Fletching => fletching(cv, x, y, tick),
        Gift::PowderKeg => keg(cv, x, y, tick),
        Gift::ArmourStand => armour(cv, x, y, tick),
        Gift::Dummy => dummy(cv, x, y, tick),
        _ => curio(cv, &u.words, x, y, tick, false),
    }
}

/// A grinding wheel on its frame, turning, sparks off a blade held to it.
fn whetstone(cv: &mut Img, x: i32, y: i32, tick: u32) {
    // The frame and its trough.
    cv.line(x - 8, y, x - 4, y - 10, 'P');
    cv.line(x + 8, y, x + 4, y - 10, 'P');
    cv.rect(x - 7, y - 3, 15, 3, 'r');
    cv.line(x - 7, y - 3, x + 7, y - 3, 'o');
    // The wheel, its spokes turning.
    let (cx, cy) = (x as f32 + 0.5, y as f32 - 10.0);
    cv.ellipse(cx, cy, 6.5, 6.5, 'G');
    cv.ellipse(cx, cy, 5.5, 5.5, 'J');
    cv.ellipse(cx - 1.5, cy - 1.5, 2.5, 2.5, 'h');
    let turn = tick as f32 * 0.25;
    for k in 0..4 {
        let a = turn + k as f32 * TAU / 4.0;
        cv.put((cx + a.cos() * 4.0) as i32, (cy + a.sin() * 4.0) as i32, 'g');
    }
    cv.put(x, y - 10, 'O');
    // The crank.
    cv.line(x, y - 10, x + (turn.cos() * 3.0) as i32 + 7, y - 10 + (turn.sin() * 3.0) as i32, 'o');
    // A blade to the wheel, and its sparks.
    cv.line(x - 11, y - 13, x - 5, y - 15, 'W');
    cv.line(x - 11, y - 12, x - 5, y - 14, 'V');
    cv.line(x - 13, y - 12, x - 11, y - 12, 'P');
    for k in 0..4u32 {
        let t = (tick * 2 + k * 5) % 12;
        cv.put(x - 5 + (t as i32) / 2, y - 16 - (t as i32) / 3 + (k as i32 % 2), if t < 5 { 'w' } else { '6' });
    }
}

/// A fletcher's rack: a quiver crammed with fletched arrows, a bow on its
/// pegs behind, a loose feather drifting.
fn fletching(cv: &mut Img, x: i32, y: i32, tick: u32) {
    // The bow on its pegs.
    for i in 0..17 {
        let t = i as f32 / 16.0;
        let bx = x + 5 + ((t * std::f32::consts::PI).sin() * 3.0) as i32;
        cv.put(bx, y - 18 + i, 'R');
    }
    cv.line(x + 5, y - 18, x + 5, y - 2, 'H');
    cv.put(x + 5, y - 18, 'o');
    cv.put(x + 5, y - 2, 'o');
    // The quiver: leather, a strap, arrows bristling.
    cv.rect(x - 6, y - 12, 7, 12, 'p');
    cv.line(x - 6, y - 12, x, y - 12, 'R');
    cv.line(x - 6, y - 7, x, y - 7, 'O');
    cv.line(x - 6, y - 12, x - 6, y - 1, 'B');
    for (k, (ax, ink)) in [(-5, 'H'), (-3, '7'), (-1, 'H'), (-4, '7'), (-2, 'W')].into_iter().enumerate() {
        let top = y - 17 - (k as i32 % 2) * 2;
        cv.line(x + ax, top, x + ax, y - 12, 'o');
        cv.put(x + ax - 1, top, ink);
        cv.put(x + ax + 1, top, ink);
        cv.put(x + ax, top - 1, ink);
    }
    // A feather, drifting down and up again.
    let t = (tick % 90) as f32 / 90.0;
    let (fx, fy) = (x - 9 + ((t * TAU).sin() * 2.0) as i32, y - 16 + (t * 10.0) as i32);
    cv.put(fx, fy, 'W');
    cv.put(fx + 1, fy - 1, 'H');
}

/// A powder keg on its cradle, three black bombs beside it, the fuse
/// spitting.
fn keg(cv: &mut Img, x: i32, y: i32, tick: u32) {
    let (cx, cy) = (x as f32 - 2.5, y as f32 - 7.0);
    cv.ellipse(cx, cy, 6.0, 7.0, 'B');
    cv.ellipse(cx - 1.0, cy - 0.5, 4.5, 6.0, 'R');
    cv.ellipse(cx - 2.0, cy - 1.0, 2.0, 4.0, 'o');
    for hy in [cy as i32 - 4, cy as i32 + 4] {
        cv.line(x - 8, hy, x + 3, hy, 'G');
    }
    for (dx, dy) in [(0, 0), (1, 1), (2, 2), (2, 0), (0, 2)] {
        cv.put(x - 4 + dx, y - 8 + dy, 'k');
    }
    // The fuse, curling up, spitting at its end.
    cv.line(x - 2, y - 14, x, y - 17, 'P');
    cv.line(x, y - 17, x + 3, y - 18, 'P');
    let sp = (tick / 2) % 4;
    for (dx, dy) in [(0, 0), (-1, -1), (1, -1), (0, -2), (1, 1)].into_iter().take(2 + sp as usize) {
        cv.put(x + 4 + dx, y - 19 + dy, if (dx + dy + sp as i32) % 2 == 0 { 'w' } else { '6' });
    }
    // The bombs.
    for (bx, by) in [(x + 7, y - 3), (x + 4, y - 3), (x + 6, y - 7)] {
        cv.ellipse(bx as f32, by as f32, 2.6, 2.6, 'K');
        cv.put(bx - 1, by - 1, 'g');
        cv.put(bx, by - 3, 'G');
    }
}

/// An armour stand: a great helm on a post, a mail coif under it, a
/// cuirass with its pauldrons on the cross-tree, a glint passing over the
/// steel and a red plume on top.
fn armour(cv: &mut Img, x: i32, y: i32, tick: u32) {
    cv.line(x, y - 2, x, y - 4, 'P');
    cv.rect(x - 4, y - 2, 9, 2, 'B');
    cv.line(x - 4, y - 2, x + 4, y - 2, 'R');
    // The cuirass: a breastplate lit on the left, a ridge down its middle,
    // the faulds below, the pauldrons rounded over the shoulders.
    for yy in 0..7 {
        let half = 5 - yy / 3;
        for xx in -half..=half {
            let ink = if xx < -2 { 'V' } else if xx > 2 { 'u' } else { 'U' };
            cv.put(x + xx, y - 11 + yy, ink);
        }
    }
    cv.line(x, y - 10, x, y - 6, 'v');
    cv.line(x - 3, y - 4, x + 3, y - 4, 'S');
    for side in [-1, 1] {
        cv.ellipse((x + side * 6) as f32 + 0.5, y as f32 - 10.5, 2.4, 1.6, 'U');
        cv.put(x + side * 6 - 1, y - 11, 'V');
    }
    // The coif, and the great helm: flat-topped, its eye slit and its
    // breaths, the plume.
    cv.rect(x - 3, y - 13, 7, 2, 'h');
    cv.put(x - 2, y - 13, 'i');
    cv.put(x + 1, y - 12, 'i');
    cv.rect(x - 4, y - 19, 9, 6, 'U');
    cv.rect(x - 4, y - 19, 3, 6, 'V');
    cv.line(x - 4, y - 19, x + 4, y - 19, 'W');
    cv.line(x + 4, y - 19, x + 4, y - 14, 'u');
    cv.line(x - 3, y - 17, x + 3, y - 17, 'k');
    cv.line(x, y - 16, x, y - 14, 'v');
    cv.put(x - 2, y - 15, 'k');
    cv.put(x + 2, y - 15, 'k');
    for (dx, dy) in [(0, -20), (1, -20), (1, -21), (2, -21), (3, -21)] {
        cv.put(x + dx, y + dy, if dx < 2 { '7' } else { '8' });
    }
    // A glint, sweeping over it.
    let g = (tick % 60) as i32;
    if g < 14 {
        let gx = x - 6 + g;
        for dy in 0..3 {
            cv.put(gx - dy, y - 18 + dy * 3, 'W');
        }
        cv.put(gx, y - 17, 'w');
    }
}

/// A practice dummy: a straw-stuffed sack on a post, a target painted on
/// its chest with an arrow in it, rocking a little from the last blow.
fn dummy(cv: &mut Img, x: i32, y: i32, tick: u32) {
    let rock = ((tick as f32 * 0.12).sin() * 1.2).round() as i32;
    cv.line(x, y, x, y - 6, 'P');
    cv.rect(x - 3, y - 1, 7, 1, 'B');
    cv.line(x - 6, y - 12 + rock, x + 6, y - 12 - rock, 'P');
    // The body and head, straw sticking out.
    cv.ellipse(x as f32 + 0.5 + rock as f32 * 0.5, y as f32 - 10.0, 4.5, 5.0, 'o');
    cv.ellipse(x as f32 - 0.5 + rock as f32 * 0.5, y as f32 - 11.0, 3.0, 3.5, 't');
    cv.ellipse(x as f32 + 0.5 + rock as f32, y as f32 - 17.5, 3.0, 3.0, 'o');
    cv.put(x - 1 + rock, y - 18, 'k');
    cv.put(x + 1 + rock, y - 18, 'k');
    for (dx, dy) in [(-4, -14), (5, -13), (-3, -20), (3, -21)] {
        cv.put(x + dx + rock, y + dy, 'T');
    }
    // The target and the arrow in it.
    let (tx, ty) = (x + rock / 2, y - 10);
    cv.ellipse(tx as f32 + 0.5, ty as f32, 2.6, 2.6, 'H');
    cv.ellipse(tx as f32 + 0.5, ty as f32, 1.6, 1.6, '7');
    cv.put(tx, ty, 'H');
    cv.line(tx + 1, ty - 1, tx + 6, ty - 4, 'o');
    cv.put(tx + 6, ty - 5, 'W');
    cv.put(tx + 7, ty - 4, 'W');
}

// ─── over the figures ────────────────────────────────────────────────────────

/// Over the figures: a wish landing's moment, the spot's board for the
/// seat's knight, a gold star over each knight holding a freedom wish, and
/// the plaque of whatever the seat's knight stands by.
pub(super) fn over(cv: &mut Img, run: &Run, legible: bool, view: (i32, i32), focus: Option<u32>) {
    let Some((place, spot)) = run.freedom_spot() else {
        return;
    };
    let tick = run.tick as u32;
    // Who holds a freedom wish: a little gold star twinkling over them.
    for hero in run.players.values().filter(|h| h.freedom_wishes > 0 && h.hp > 0 && !h.absent) {
        let (x, y) = at(hero.x, hero.y);
        let bob = [0, 1, 1, 0][(tick / 8 % 4) as usize];
        star(cv, x + 7, y - 27 - bob, tick, 1);
    }
    if let Some(l) = run.freedom.landed.as_ref().filter(|l| l.place == place) {
        moment(cv, run, place, spot, l, legible, view);
        return;
    }
    if !legible {
        return;
    }
    let me = focus.unwrap_or(1);
    let Some(hero) = run.players.get(&me).filter(|h| h.hp > 0) else {
        return;
    };
    let (hx, hy) = at(hero.x, hero.y);
    if run.near_freedom_spot(me) {
        prompt(cv, run, place, spot, me, view);
        return;
    }
    // The plaque of the wished thing the knight stands nearest, close by.
    let wished = run.home.wished.of(place);
    let near = (0..wished.len())
        .map(|i| (i, slot_px(run, place, i)))
        .map(|(i, (x, y))| (i, (x, y), ((x - hx) as f32).hypot((y - hy) as f32)))
        .filter(|&(_, _, d)| d < if place == Place::Safe { 30.0 } else { 34.0 })
        .min_by(|a, b| a.2.total_cmp(&b.2));
    if let Some((i, (x, y), _)) = near {
        plaque_board(cv, &wished[i], (x, y - if place == Place::Safe { 44 } else { 26 }), view);
    }
}

/// A four-pointed star, `r` its reach, twinkling.
fn star(cv: &mut Img, x: i32, y: i32, tick: u32, r: i32) {
    let tw = (tick / 5 % 4) as i32;
    let reach = r + i32::from(tw == 1 || tw == 2);
    cv.put(x, y, 'w');
    for d in 1..=reach {
        let ink = if d == reach { '5' } else { '6' };
        cv.put(x - d, y, ink);
        cv.put(x + d, y, ink);
        cv.put(x, y - d, ink);
        cv.put(x, y + d, ink);
    }
}

/// The spot's board for knight `me` beside it: what it is, what E does, and
/// how full the place is.
fn prompt(cv: &mut Img, run: &Run, place: Place, spot: (f32, f32), me: u32, view: (i32, i32)) {
    let hero = &run.players[&me];
    let (x, y) = source_px(place, spot);
    let held = hero.freedom_wishes;
    let count = run.home.wished.of(place).len();
    let head = match place {
        Place::Safe => "THE GENIE'S BRAZIER",
        Place::Locker => "THE WEAPONS LOCKER",
    };
    let key = if hero.offer.is_some() {
        "1 SEAL IT   ESC KEEP IT"
    } else if run.home.wished.full(place) {
        "FULL OF WISHES"
    } else if held > 0 {
        "E: MAKE A FREEDOM WISH"
    } else {
        "FREE THE GENIE FOR A WISH"
    };
    let room = if place == Place::Safe { "SAFE ROOM" } else { "BAYS" };
    let line = format!("{room} {count}/{}   HELD {held}", f::MOST_PER_PLACE);
    let w = text_width(head).max(text_width(key)).max(text_width(&line)) + 8;
    let h = 30;
    let px = in_view_x(x, w, view, cv.w);
    let py = (y - h - 8).max(view.1 + 2);
    chrome::panel(cv, (px, py, w, h), Tone::FRAMED.rimmed(GOLD_RIM));
    cv.text(px + (w - text_width(head)) / 2, py + 3, head, '6');
    cv.text(px + (w - text_width(key)) / 2, py + 12, key, if held > 0 { 'w' } else { 'i' });
    cv.text(px + (w - text_width(&line)) / 2, py + 21, &line, '3');
}

/// A wished thing's plaque, read up close: its name, its words, what it
/// does, and who wished it.
fn plaque_board(cv: &mut Img, u: &Upgrade, (x, y): (i32, i32), view: (i32, i32)) {
    let mut lines: Vec<(String, char)> = vec![(fit(&u.name().to_uppercase(), 30), '6')];
    if u.gift != Gift::Curio {
        lines.push((fit(&u.words.to_uppercase(), 30), 'h'));
    }
    lines.push((u.does().to_uppercase(), 'i'));
    lines.push((format!("WISHED BY {}", fit(&u.by.to_uppercase(), 18)), '5'));
    let w = lines.iter().map(|(l, _)| text_width(l)).max().unwrap_or(0) + 8;
    let h = lines.len() as i32 * 9 + 4;
    let px = in_view_x(x, w, view, cv.w);
    let py = (y - h).max(view.1 + 2);
    chrome::panel(cv, (px, py, w, h), Tone::FRAMED.rimmed(GOLD_RIM));
    for (i, (l, ink)) in lines.iter().enumerate() {
        cv.text(px + (w - text_width(l)) / 2, py + 3 + i as i32 * 9, l, *ink);
    }
}

/// Text cut to `most` characters, an ellipsis when it is cut.
fn fit(s: &str, most: usize) -> String {
    if s.chars().count() <= most {
        return s.to_string();
    }
    let mut t: String = s.chars().take(most.saturating_sub(2)).collect();
    t.push_str("..");
    t
}

/// A freedom wish landing: the genie's smoke pouring from the spot to the
/// place, a ring of gold bursting where the thing shimmers in, sparks
/// raining, the genie himself popping out over it to wave, and its banner.
fn moment(cv: &mut Img, run: &Run, place: Place, spot: (f32, f32), l: &f::Landed, legible: bool, view: (i32, i32)) {
    let tick = run.tick as u32;
    let age = run.tick.saturating_sub(l.tick) as u32;
    let life = f::LANDING_TICKS as u32;
    let (sx, sy) = source_px(place, spot);
    let (tx, ty) = slot_px(run, place, usize::from(l.slot));
    let mid_y = ty - if place == Place::Safe { 16 } else { 10 };
    // The smoke, poured along an arc from the spot to the place: soft
    // puffs of his teal, fattest mid-arc, swirling as they go.
    if age <= 44 {
        let pour = (age as f32 / 18.0).min(1.0);
        let gone = ((age as f32 - 26.0) / 18.0).clamp(0.0, 1.0);
        let lift = 26.0 + ((sx - tx).abs() as f32 * 0.12);
        let n = 28;
        for i in 0..n {
            let t = i as f32 / n as f32;
            if t > pour || t < gone {
                continue;
            }
            let bx = sx as f32 + (tx - sx) as f32 * t;
            let by = sy as f32 + (mid_y - sy) as f32 * t - (t * (1.0 - t) * 4.0) * lift;
            let swirl = ((tick as f32 * 0.3) + i as f32 * 0.9).sin() * (1.5 + t * 2.5);
            let r = 1.6 + (t * std::f32::consts::PI).sin() * 2.6;
            let (px, py) = (bx.round() as i32, (by + swirl).round() as i32);
            let ri = r.ceil() as i32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let d = ((dx * dx + dy * dy) as f32).sqrt() / r;
                    if d > 1.0 {
                        continue;
                    }
                    let ink = if d < 0.35 { '3' } else if d < 0.75 { '2' } else { '1' };
                    if d < 0.75 || (dx + dy + i as i32 + tick as i32) % 2 == 0 {
                        cv.put(px + dx, py + dy, ink);
                    }
                }
            }
            if i % 5 == (tick / 2 % 5) as usize {
                cv.put(px - 1, py - 1, 'w');
            }
        }
    }
    // The flash where it lands, as the thing begins to shimmer in.
    if (REVEAL_FROM - 2..REVEAL_FROM + 8).contains(&age) {
        let k = (age + 2 - REVEAL_FROM) as f32 / 10.0;
        let r = 4.0 + k * 10.0;
        for dy in -(r as i32)..=(r as i32) {
            for dx in -(r as i32 * 2)..=(r as i32 * 2) {
                let d = ((dx as f32 / 2.0).powi(2) + (dy as f32).powi(2)).sqrt() / r;
                if d <= 1.0 && (hash(dx, dy, age) % 100) as f32 > d * 100.0 + k * 40.0 {
                    cv.put(tx + dx, mid_y + dy, if d < 0.4 { 'w' } else { '6' });
                }
            }
        }
    }
    // The burst: a ring of gold spreading on the floor, rays, a flash.
    if (REVEAL_FROM - 4..REVEALED + 30).contains(&age) {
        let k = (age + 4 - REVEAL_FROM) as f32 / 50.0;
        let r = 6.0 + k * 34.0;
        for i in 0..40 {
            let a = i as f32 / 40.0 * TAU;
            if (i + age as i32) % 3 != 0 {
                cv.put((tx as f32 + a.cos() * r) as i32, (ty as f32 + a.sin() * r * 0.35) as i32, if i % 2 == 0 { '6' } else { '5' });
            }
        }
        if age < REVEALED {
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU + age as f32 * 0.05;
                let (r0, r1) = (8.0 + k * 10.0, 14.0 + k * 22.0);
                cv.line(
                    (tx as f32 + a.cos() * r0) as i32,
                    (mid_y as f32 + a.sin() * r0 * 0.8) as i32,
                    (tx as f32 + a.cos() * r1) as i32,
                    (mid_y as f32 + a.sin() * r1 * 0.8) as i32,
                    if i % 2 == 0 { '5' } else { '4' },
                );
            }
        }
    }
    // Sparkles raining round it, the whole while.
    for i in 0..16u32 {
        let t = (age + i * 11) % 50;
        let x = tx - 22 + (hash(i as i32, 0, 0x5a) % 45) as i32 + ((t as f32 * 0.2 + i as f32).sin() * 2.0) as i32;
        let y = ty - 40 + t as i32 - (hash(i as i32, 1, 0x5a) % 10) as i32;
        if age + 10 < life {
            if i % 4 == 0 && t < 30 {
                star(cv, x, y, tick + i, 1);
            } else if t < 40 {
                cv.put(x, y, if i % 3 == 0 { 'w' } else { '6' });
            }
        }
    }
    // The genie, out of his smoke over the thing: he waves, then he's off.
    if (22..life - 12).contains(&age) {
        let pop = age - 22;
        let full = if place == Place::Safe { 50 } else { 46 };
        let rows = if pop < 6 { full * (pop as i32 + 2) / 8 } else { full };
        let mood = if run.genie.free() { Mood::Free } else { Mood::Cheer };
        let g = genie_art::figure(mood, run.tick, rows);
        let bob = ((tick as f32 * 0.12).sin() * 2.0) as i32;
        // Beside the thing, his tail curling down to it: by the safe room's
        // things, or out at the locker's side.
        let (gx, gy) = match place {
            Place::Safe => (tx + 30, ty - 26 - g.h + bob),
            Place::Locker => {
                let (x0, y0, w, _) = locker_rect();
                (x0 + w + 20, y0 + 30 - g.h + bob)
            }
        };
        // His smoke tail curling down to the thing.
        for i in 0..14 {
            let sway = ((tick as f32 * 0.2) + i as f32 * 0.6).sin() * 2.0;
            cv.put(gx + (sway as i32) - i / 3, gy + g.h - 4 + i, if i % 3 == 0 { '3' } else { '2' });
        }
        cv.stamp(&g, gx - g.w / 2, gy.max(view.1));
        if age > life - 26 {
            // A puff, and he's gone.
            let k = (age - (life - 26)) as f32 / 14.0;
            for i in 0..18 {
                let a = i as f32 / 18.0 * TAU;
                let r = 4.0 + k * 16.0;
                cv.put(gx + (a.cos() * r) as i32, gy + g.h / 2 + (a.sin() * r * 0.8) as i32, if i % 2 == 0 { 'H' } else { '3' });
            }
        }
    }
    // Its banner, for everyone.
    if legible && (18..life - 6).contains(&age) {
        let Some(u) = run.home.wished.of(place).get(usize::from(l.slot)) else {
            return;
        };
        let head = "FREEDOM WISH GRANTED!";
        let name = fit(&u.name().to_uppercase(), 30);
        let by = format!("WISHED BY {} INTO {}", fit(&u.by.to_uppercase(), 14), place.name().to_uppercase());
        let w = text_width(head).max(text_width(&name)).max(text_width(&by)) + 10;
        let h = 31;
        let x = in_view_x(tx, w, view, cv.w);
        let y = (ty - if place == Place::Safe { 110 } else { 92 }).max(view.1 + 4);
        chrome::panel(cv, (x, y, w, h), Tone::FRAMED.rimmed(GOLD_RIM));
        let shine = (tick / 4 % 2) == 0;
        cv.text(x + (w - text_width(head)) / 2, y + 3, head, if shine { 'w' } else { '6' });
        cv.text(x + (w - text_width(&name)) / 2, y + 12, &name, '6');
        cv.text(x + (w - text_width(&by)) / 2, y + 21, &by, '3');
        for (dx, dy) in [(-3, 2), (w + 2, 4), (-2, h - 3), (w + 1, h - 5)] {
            star(cv, x + dx, y + dy, tick + (dx + 8) as u32, 1);
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__freedom__shots.rs"]
mod shots;
