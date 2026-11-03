//! The demon hand. A few seconds after a guardian falls and its loot has
//! dropped, the ground under the body cracks and glows; a great hand climbs
//! out of the crack, claws spread; it closes on the body and drags it down
//! into the depths while the ground shakes, and the crack shuts over the
//! place. The loot, lying round the body, stays.
//!
//! Everything is drawn from the claim's clock (`spectacle::Beat`), in ink
//! on black paper: a glowing, dithered patch of ground, a jagged crack, a
//! hole, then the hand. The hand is painted (`assets/demon-hand/hand.px.js`,
//! in pxl, in the game's inks): a clawed forearm and hand, palm toward us,
//! in ten frames (spread and twitching as it rises, closing, gripping), each
//! frame in two strips, what stands behind the body and the fingers that
//! come forward over it. It comes in three sizes, picked by the body's. The
//! body is the guardian's own sprite, greyed and laid on its side.

use std::sync::OnceLock;

use super::super::ink::{Img, bayer, hash};
use super::{at, sprites, stature};
use crate::rules::spectacle::{Beat, Corpse, PULL, RISE};
use crate::rules::{EnemyKind, Run};

/// The painted hand at one size: its palm's width in pixels, where its wrist
/// is in a frame, and its frames behind and in front of the body.
struct Painted {
    palm: i32,
    wrist: (i32, i32),
    back: Vec<Img>,
    front: Vec<Img>,
}

/// Frames in each painted strip.
const FRAMES: i32 = 10;

/// The sizes the hand is painted at, small to large (`SIZES` in
/// `hand.px.js`), with where it lays the wrist in a frame: across, the
/// middle; down, `round(2.2 S) + 3`, the open claws above it.
fn painted() -> &'static [Painted] {
    static HANDS: OnceLock<Vec<Painted>> = OnceLock::new();
    HANDS.get_or_init(|| {
        let strips: [(i32, (i32, i32), &[u8], &[u8]); 3] = [
            (
                20,
                (27, 47),
                include_bytes!("../../../../assets/demon-hand/hand_back_20.png"),
                include_bytes!("../../../../assets/demon-hand/hand_front_20.png"),
            ),
            (
                28,
                (36, 65),
                include_bytes!("../../../../assets/demon-hand/hand_back_28.png"),
                include_bytes!("../../../../assets/demon-hand/hand_front_28.png"),
            ),
            (
                38,
                (48, 87),
                include_bytes!("../../../../assets/demon-hand/hand_back_38.png"),
                include_bytes!("../../../../assets/demon-hand/hand_front_38.png"),
            ),
        ];
        strips
            .iter()
            .filter_map(|&(palm, wrist, back, front)| {
                Some(Painted {
                    palm,
                    wrist,
                    back: frames(back)?,
                    front: frames(front)?,
                })
            })
            .collect()
    })
}

/// A strip of `FRAMES` frames, side by side.
fn frames(png: &[u8]) -> Option<Vec<Img>> {
    let png = image::load_from_memory(png).ok()?.to_rgba8();
    let mut strip = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, c) in png.enumerate_pixels() {
        if c[3] > 127 {
            strip.set(x as i32, y as i32, [c[0], c[1], c[2]]);
        }
    }
    let w = strip.w / FRAMES;
    Some(
        (0..FRAMES)
            .map(|i| Img::from_view(&strip, i * w, 0, w, strip.h))
            .collect(),
    )
}

/// The body's lightness, on the structure greys a notch darker than a
/// statue's: a thing that has burned out.
fn charred(im: &Img) -> Img {
    const RAMP: [char; 7] = ['k', 'X', 'g', 'j', 'G', 'J', 'h'];
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(x, y, RAMP[(l as usize * RAMP.len() / 256).min(RAMP.len() - 1)]);
            }
        }
    }
    out
}

/// The body, charred and laid on its side.
fn body(run: &Run, c: &Corpse) -> Img {
    // The Lawn Flamingo lies as what it always was (`garden`).
    if let Some(im) = super::garden::flamingo_corpse(run, c) {
        return im;
    }
    let im = match c.boss.and_then(|i| stature::boss_figure(run, i)) {
        Some(im) => im,
        None if c.kind == EnemyKind::Slime => stature::monster(
            c.kind,
            sprites::enemy(c.kind, 0),
            c.radius / sprites::SLIME_REACH,
        ),
        None => stature::monster(c.kind, sprites::enemy(c.kind, 0), 1.0),
    };
    let mut lying = charred(&im);
    if lying.h > lying.w {
        lying = sprites::quarter_turn(&lying);
    }
    lying.outline_outside('k');
    lying
}

/// The painted frame a beat shows, how far the hand has climbed (0..1), how
/// far it has come down onto the body (0..1) and how far it has dragged it
/// under (0..1).
fn pose(beat: Beat) -> (usize, f32, f32, f32) {
    let ticks = |t: f32, len: u32| (t * len as f32) as u32;
    match beat {
        Beat::Lying | Beat::Cracking(_) | Beat::Closing(_) | Beat::Gone => (0, 0.0, 0.0, 0.0),
        // Fast out of the ground and slowing, the claws twitching.
        Beat::Rising(t) => {
            let frame = [0, 1, 0, 2][(ticks(t, RISE) / 5 % 4) as usize];
            (frame, 1.0 - (1.0 - t).powi(3), 0.0, 0.0)
        }
        // A beat spread wide, then the claws snap shut as the hand comes down.
        Beat::Grabbing(t) => {
            let frame = match t {
                t if t < 0.3 => 3,
                t if t < 0.45 => 4,
                t if t < 0.6 => 5,
                t if t < 0.75 => 6,
                _ => 7,
            };
            let down = ((t - 0.3) / 0.45).clamp(0.0, 1.0);
            (frame, 1.0, down * down, 0.0)
        }
        // The grip squeezes and eases while it drags, slow and then all at once.
        Beat::Pulling(t) => (8 + (ticks(t, PULL) / 4 % 2) as usize, 1.0, 1.0, t * t),
    }
}

/// How glowing the ground is (0..1) and how far open the hole is (0..1).
fn glow_and_hole(beat: Beat) -> (f32, f32) {
    match beat {
        Beat::Lying | Beat::Gone => (0.0, 0.0),
        Beat::Cracking(t) => (t, ((t - 0.55) / 0.45).clamp(0.0, 1.0)),
        Beat::Rising(_) | Beat::Grabbing(_) | Beat::Pulling(_) => (1.0, 1.0),
        Beat::Closing(t) => (1.0 - t, 1.0 - t),
    }
}

/// A claim's ground: glow, crack, hole and embers. `(cx, cy)` is the hole's
/// middle in native pixels.
fn ground(cv: &mut Img, (cx, cy): (i32, i32), s: f32, beat: Beat, tick: u32, seed: i32) {
    let (glow, hole) = glow_and_hole(beat);
    if glow <= 0.0 {
        return;
    }
    // The glowing ground, dithered out to its edge.
    let (rx, ry) = (s * 1.25 * (0.4 + 0.6 * glow), s * 0.5 * (0.4 + 0.6 * glow));
    let pulse = 0.85 + 0.15 * (tick as f32 * 0.22).sin();
    for y in (cy as f32 - ry) as i32..=(cy as f32 + ry) as i32 {
        for x in (cx as f32 - rx) as i32..=(cx as f32 + rx) as i32 {
            let d = (((x - cx) as f32 / rx).powi(2) + ((y - cy) as f32 / ry).powi(2)).sqrt();
            if d > 1.0 {
                continue;
            }
            let k = (1.0 - d) * glow * pulse;
            if bayer(x, y) < k * 1.3 {
                cv.put(x, y, if k > 0.6 { '@' } else if k > 0.32 { '7' } else { '8' });
            }
        }
    }
    // The crack: seven jagged arms from the middle.
    let grow = match beat {
        Beat::Cracking(t) => t,
        _ => 1.0,
    };
    let cooling = matches!(beat, Beat::Closing(_));
    for k in 0..7 {
        let ang = k as f32 * std::f32::consts::TAU / 7.0 + 0.35 + (hash(k, seed, 3) % 50) as f32 / 100.0;
        let len = s * (0.6 + (hash(k, seed, 9) % 55) as f32 / 100.0) * grow;
        let (mut px, mut py) = (cx as f32, cy as f32);
        let segments = 7;
        for seg in 0..segments {
            let a = ang + ((hash(k * 17 + seg, seed, 11) % 100) as f32 / 100.0 - 0.5) * 1.1;
            let step = len / segments as f32;
            let (nx, ny) = (px + a.cos() * step, py + a.sin() * step * 0.55);
            if let Beat::Closing(t) = beat
                && t > 0.55
                && (hash(k, seg, 5) % 100) as f32 / 100.0 < (t - 0.55) / 0.45
            {
                break;
            }
            cv.line(px as i32, py as i32, nx as i32, ny as i32, 'k');
            cv.line(px as i32, py as i32 + 1, nx as i32, ny as i32 + 1, 'k');
            let lava = if cooling {
                'p'
            } else if seg < 3 {
                '@'
            } else {
                '7'
            };
            cv.line(px as i32, py as i32, nx as i32, ny as i32, lava);
            (px, py) = (nx, ny);
        }
    }
    // The hole.
    if hole > 0.0 {
        let (hx, hy) = (s * 0.62 * hole, s * 0.24 * hole);
        cv.ellipse(cx as f32, cy as f32, hx + 1.5, hy + 1.5, '7');
        cv.ellipse(cx as f32, cy as f32, hx, hy, 'k');
        cv.ellipse(cx as f32, cy as f32 + hy * 0.25, hx * 0.8, hy * 0.6, '8');
        cv.ellipse(cx as f32, cy as f32 + hy * 0.35, hx * 0.55, hy * 0.35, 'k');
    }
    // Embers climbing off it.
    if glow > 0.3 {
        for n in 0..14 {
            let h = hash(n, seed, 23);
            let life = 36 + h % 20;
            let t = (tick + h % 40) % life;
            let x = cx + (h % (s as u32 * 2 + 1)) as i32 - s as i32;
            let y = cy - (t as f32 * 0.9) as i32 + 2;
            let fade = t as f32 / life as f32;
            if fade < 0.8 {
                cv.put(x, y, if fade < 0.35 { '5' } else { '@' });
            } else {
                cv.put(x, y, '7');
            }
        }
    }
}

/// Draw `src` onto `cv` at `(x, y)`, showing only rows above `max_y`: what
/// has gone below the lip of the hole is gone.
fn stamp_above(cv: &mut Img, src: &Img, x: i32, y: i32, max_y: i32) {
    for sy in 0..src.h {
        if y + sy > max_y {
            break;
        }
        for sx in 0..src.w {
            if let Some(c) = src.get(sx, sy) {
                cv.set(x + sx, y + sy, c);
            }
        }
    }
}

/// What each claim has to draw this tick.
struct Claim {
    lying: Img,
    body: (i32, i32),
    hole: (i32, i32),
    hand: &'static Painted,
    beat: Beat,
    seed: i32,
}

/// The hand for a body: the largest whose closed fist holds it and whose
/// open claws, raised over it, stand inside the picture.
fn sized(lying: &Img, top: i32) -> &'static Painted {
    let hands = painted();
    let want = (lying.h as f32 * 1.2).max(lying.w as f32 * 0.7);
    let room = (top - 2) as f32 / 1.75;
    hands
        .iter()
        .rev()
        .find(|p| p.palm as f32 <= want.min(room))
        .unwrap_or(&hands[0])
}

fn claims(run: &Run) -> Vec<Claim> {
    let mut out = Vec::new();
    if painted().is_empty() {
        return out;
    }
    for (n, c) in run.spectacle.corpses.iter().enumerate() {
        let beat = c.beat(run.tick);
        if c.room != run.at || matches!(beat, Beat::Gone) {
            continue;
        }
        let lying = body(run, c);
        let (bx, by) = at(c.x, c.y);
        let hand = sized(&lying, by - lying.h);
        out.push(Claim {
            body: (bx - lying.w / 2, by - lying.h),
            hole: (bx, by - lying.h / 4),
            hand,
            beat,
            seed: n as i32 * 7 + c.born as i32 % 11,
            lying,
        });
    }
    out
}

/// The cracking ground, glow and hole: under the loot lying on it.
pub(super) fn ground_layer(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for c in claims(run) {
        // The ground opens as wide as the hand that comes up through it.
        ground(cv, c.hole, c.hand.palm as f32 * 1.35, c.beat, tick, c.seed);
    }
}

/// The body and the hand over it, over the loot but under the knights.
pub(super) fn over(cv: &mut Img, run: &Run) {
    for c in claims(run) {
        if matches!(c.beat, Beat::Closing(_)) {
            // Gone below: only the shutting ground remains.
            continue;
        }
        let (frame, climb, down, sink) = pose(c.beat);
        // What is behind the body goes down into the hole's dark middle; the
        // body and the fist over it go down past the hole's near edge.
        let lip = c.hole.1 + 1;
        let near = c.body.1 + c.lying.h;
        let palm = c.hand.palm as f32;
        let top = c.body.1;
        let drop = (sink * ((lip - top) as f32 + 0.25 * palm + 2.0)) as i32;
        if climb <= 0.0 {
            cv.stamp(&c.lying, c.body.0, c.body.1);
            continue;
        }
        // Where the knuckles stand: below the lip before it climbs, high over
        // the body with the claws spread, then down on it with the fist shut.
        let hidden = lip as f32 + 1.35 * palm;
        let raised = (top as f32 - 0.45 * palm).max(1.3 * palm + 1.0);
        // Gripped over its top half, so the body shows under the claws.
        let gripped = top as f32 - 0.32 * palm;
        let knuckles = hidden + (raised - hidden) * climb + (gripped - raised) * down;
        let p = c.hand;
        let shake = i32::from(sink > 0.0 && (run.tick / 2) % 2 == 0);
        let ox = c.hole.0 - p.wrist.0 + shake;
        let oy = knuckles as i32 + (0.9 * palm) as i32 - p.wrist.1 + drop;
        let frame = frame.min(p.back.len() - 1);
        stamp_above(cv, &p.back[frame], ox, oy, lip);
        stamp_above(cv, &c.lying, c.body.0, c.body.1 + drop, near);
        stamp_above(cv, &p.front[frame], ox, oy, near);
    }
}
