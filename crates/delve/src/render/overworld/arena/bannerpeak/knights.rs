//! The knights as sprites: a pose from the sim picks a frame, the seat's tones go on, and the
//! body squashes on a landing, stretches on a jump, flashes when struck. A friend's knight looks
//! the same as yours (the camera is yours); a small pip over them says which seat they are.
use super::super::super::ink::Img;
use super::super::juice;
use super::art::{self, Sprite};
use super::{H, W, banner_ink, ground_below, seat_ink, tri};
use crate::rules::bannerpeak::{
    body::{Move, Pose, Q},
    knight::{Knight, Size},
    session::Session,
    show::Kind,
};
use crate::rules::juice::Weight;

/// Which frame the sim's pose is: the set's name, the frame in it, and whether it faces left.
fn choose(k: &Knight, tick: u32, seat: u32) -> (&'static str, usize, bool) {
    let b = &k.body;
    let face_left = b.face < 0;
    let air = |rise: &'static str| -> &'static str {
        if b.vy.abs() < 520 { "apex" } else if b.vy < 0 { rise } else { "fall" }
    };
    let pose = if k.pole > 0 { Pose::Pole } else if k.pose > 0 { Pose::Item } else { b.pose };
    match pose {
        Pose::Idle => ("idle", ((tick / 22 + seat) % 2) as usize, face_left),
        Pose::Run | Pose::RunB => ("run", ((b.run_t / 20) % 6) as usize, face_left),
        Pose::Skid => ("skid", 0, face_left),
        Pose::Crouch => ("crouch", 0, face_left),
        Pose::Slide => ("slide", 0, face_left),
        Pose::Rise => (air("rise"), 0, face_left),
        Pose::Fall => (air("fall"), 0, face_left),
        Pose::Second => (if b.vy > 400 { "fall" } else { "second" }, 0, face_left),
        Pose::Spin => ("spin", ((tick / 2) % 4) as usize, face_left),
        Pose::Long => ("long", 0, face_left),
        Pose::Back => ("back", ((tick / 2) % 4) as usize, face_left),
        // Back to the wall, hands on it: the wall is on the side it clings to.
        Pose::Wall => ("wall", 0, b.wall > 0),
        Pose::Wind => ("wind", 0, face_left),
        Pose::Slam => ("slam", ((tick / 2) % 4) as usize, face_left),
        Pose::Land => ("crouch", 0, face_left),
        Pose::Dive => ("dive", 0, face_left),
        Pose::Belly => ("belly", 0, face_left),
        Pose::Hurt => ("hurt", 0, b.vx > 0),
        Pose::Brace => ("brace", 0, face_left),
        Pose::Item => ("apex", 0, face_left),
        Pose::Pole => ("wall", 0, face_left),
        Pose::Bubble => ("wind", 0, face_left),
    }
}

/// How the body is squashed (`sx`, `sy`) this tick: flat and wide on a landing, thin and tall as
/// it leaves the ground and as it falls fast.
fn squash(s: &Session, seat: u32, k: &Knight, now: u32) -> (f32, f32) {
    let b = &k.body;
    let mut sx = 1.0;
    let mut sy = 1.0;
    if let Some(m) = s.show.latest(Kind::Land, seat) {
        let age = m.age(now);
        if age < 6 {
            let hard = (f32::from(m.n) / 12.0).clamp(0.2, 1.0);
            let k = [1.0, 0.8, 0.5, 0.28, 0.12, 0.04][age as usize] * hard;
            sy -= 0.34 * k;
            sx += 0.30 * k;
        }
    }
    if let Some(m) = s.show.latest(Kind::Hop, seat) {
        let age = m.age(now);
        if age < 4 {
            let k = [1.0, 0.8, 0.45, 0.2][age as usize];
            sy += 0.16 * k;
            sx -= 0.14 * k;
        }
    }
    if let Some(m) = s.show.latest(Kind::Slam, seat) {
        let age = m.age(now);
        if age < 6 {
            let k = [1.0, 0.85, 0.6, 0.35, 0.18, 0.06][age as usize];
            sy -= 0.4 * k;
            sx += 0.36 * k;
        }
    }
    if !b.ground && !matches!(b.mode, Move::Wind | Move::Pound | Move::Dive | Move::Belly | Move::Hurt) {
        let fast = (b.vy.abs() as f32 / 3600.0).clamp(0.0, 1.0);
        sy += 0.10 * fast;
        sx -= 0.07 * fast;
    }
    (sx, sy)
}

/// A hit knight in one flat light for the first ticks after the blow.
fn struck(s: &Session, seat: u32, now: u32) -> bool {
    s.show.latest(Kind::Hurt, seat).is_some_and(|m| m.age(now) < 3)
}

pub(super) fn draw(im: &mut Img, s: &Session, seat: u32, k: &Knight, cam: (i32, i32), mine: bool, tick: u32) {
    let ink = seat_ink(seat);
    if let Some(b) = k.bubble {
        bubble(im, seat, b.x.div_euclid(Q) - cam.0, b.y.div_euclid(Q) - cam.1, b.t, ink, tick);
        return;
    }
    if k.pit > 0 {
        return;
    }
    // A knight in the grace after a hit blinks, as every platformer's does.
    if k.hurt > 0 && tick / 3 % 2 == 0 && k.pose == 0 && !struck(s, seat, tick) {
        return;
    }
    let (fx, fy) = k.body.foot();
    let (sx, sy) = (fx - cam.0, fy - cam.1);
    if sx < -40 || sx > W + 40 || sy < -60 || sy > H + 60 {
        return;
    }
    // The shadow of a knight in the air, on whatever is below: a dithered oval that thins as it rises.
    if !k.body.ground {
        if let Some(ground) = ground_below(s, fx, fy) {
            let gy = ground - cam.1;
            let gap = gy - sy;
            if gap > 0 && gap < 120 {
                let half = 6 - (gap / 24).min(3);
                for dx in -half..=half {
                    if (dx + gy) % 2 == 0 || half > 4 {
                        im.put(sx + dx, gy - 1, 'k');
                    }
                }
                im.rect(sx - half + 2, gy - 2, (half - 2) * 2 + 1, 1, 'k');
            }
        }
    }
    let big = k.big();
    let (mut name, index, flip) = choose(k, tick, seat);
    // At the Great Pole the knights cheer: each hops in its own time, the plume up, the shield high.
    let mut lift = 0;
    if s.phase == crate::rules::bannerpeak::session::Phase::Finale && s.phase_t > 24 {
        let t = (s.phase_t + seat * 7) % 22;
        if t < 12 {
            lift = (t * (12 - t) / 4) as i32;
            name = if t < 6 { "rise" } else { "fall" };
        } else {
            name = "idle";
        }
    }
    let Some(base) = art::knight(big, name, index, seat, flip) else {
        return;
    };
    let (qx, qy) = squash(s, seat, k, tick);
    let mut sprite = art::stretched(&base, qx, qy);
    if struck(s, seat, tick) {
        sprite = Sprite { img: juice::impact_frame(&sprite.img, 'c'), dx: sprite.dx, dy: sprite.dy };
    }
    // A big knight leaning into a skid or a turn needs nothing more: the frame has it. A friend a
    // touch dimmer than you, so you read first on a busy screen.
    art::stamp(im, &sprite, sx, sy - lift);
    // Extras the frames do not carry.
    let face = i32::from(k.body.face);
    if k.size == Size::Flame {
        lantern(im, sx + face * 11, sy - 16, tick, seat);
    }
    if k.pose > 0 && k.pole == 0 {
        // The item-get pose: the banner held high in the knight's own hand.
        let col = s.announce.map_or(0, |a| usize::from(a.banner));
        let top = sy - 38 - (if big { 10 } else { 0 });
        im.rect(sx + face * 4, top, 1, 14, 'P');
        im.rect(sx + face * 4 + (if face > 0 { 1 } else { -9 }), top, 9, 6, banner_ink(col));
        im.frame(sx + face * 4 + (if face > 0 { 1 } else { -9 }), top, 9, 6, 'k');
        im.put(sx + face * 4 + (if face > 0 { 4 } else { -6 }), top + 2, 'T');
    }
    // Who it is: a small seat pip floats over a friend; yours has an arrow at the start.
    let head = sy - sprite.img.h.max(18) - 3;
    if mine {
        if s.elapsed < 150 && tick / 8 % 2 == 0 {
            let ay = head - 6;
            im.rect(sx - 2, ay, 5, 1, ink);
            im.rect(sx - 1, ay + 1, 3, 1, ink);
            im.put(sx, ay + 2, ink);
        }
    } else {
        // A small diamond in the seat's colour, bobbing: it says who without a word to collide with.
        let py = head - 3 - tri(tick as i32 + seat as i32 * 5, 16) / 4;
        for (dx, dy) in [(0, -2), (-1, -1), (1, -1), (-2, 0), (2, 0), (-1, 1), (1, 1), (0, 2)] {
            im.put(sx + dx, py + dy, 'k');
        }
        im.rect(sx - 1, py - 1, 3, 3, ink);
        im.put(sx, py, 'k');
        im.put(sx, py, 'k');
        im.put(sx - 1, py - 1, 'H');
    }
}

/// The lantern a flame knight carries: a small cage with a flame that never holds still.
fn lantern(im: &mut Img, x: i32, y: i32, tick: u32, seat: u32) {
    let flick = (tick / 2 + seat) % 3;
    im.rect(x - 2, y, 5, 6, 'k');
    im.rect(x - 1, y + 1, 3, 4, 'a');
    im.rect(x - 1, y + 2 - (flick as i32 % 2), 3, 3, 'O');
    im.put(x, y + 1 - (flick as i32 % 2), '6');
    im.put(x, y + 3, 'T');
    im.rect(x - 1, y - 2, 3, 2, 'o');
    im.put(x, y - 3, 'o');
}

/// A knight in a bubble: curled up inside it, the bubble thinning as the time runs out, so a
/// friend can see how long they have.
fn bubble(im: &mut Img, seat: u32, x: i32, y: i32, t: u32, ink: char, tick: u32) {
    let wob = tri(tick as i32 + seat as i32 * 7, 24) / 6 - 2;
    let y = y + wob;
    if let Some(ball) = art::knight(false, "wind", 0, seat, false) {
        art::stamp(im, &ball, x, y + 9);
    }
    // The skin: a ring of light with a glint, an ordered dither of paler air inside.
    let left = (330u32.saturating_sub(t)) as f32 / 330.0;
    let urgent = left < 0.25;
    for deg in (0..360).step_by(6) {
        let (c, sn) = (crate::rules::bannerpeak::layout::cosine(deg), crate::rules::bannerpeak::layout::sine(deg));
        let (px, py) = (x + 13 * c / 1024, y + 13 * sn / 1024);
        let lit = deg as f32 / 360.0 <= left;
        if urgent && tick / 4 % 2 == 0 && !lit {
            continue;
        }
        im.put(px, py, if lit { 'z' } else { 'U' });
        if lit && deg % 18 == 0 {
            im.put(px, py, 'W');
        }
    }
    for dy in -12..=12i32 {
        for dx in -12..=12i32 {
            let d2 = dx * dx + dy * dy;
            if d2 < 144 && (dx + dy).rem_euclid(5) == 0 && (dx - dy).rem_euclid(3) == 0 {
                im.put(x + dx, y + dy, 'V');
            }
        }
    }
    im.put(x - 6, y - 8, 'W');
    im.put(x - 5, y - 8, 'W');
    im.put(x - 6, y - 7, 'W');
    // Whose it is: the seat's diamond, as over a friend; no digit to stand on other words.
    for (dx, dy) in [(0, -2), (-1, -1), (1, -1), (-2, 0), (2, 0), (-1, 1), (1, 1), (0, 2)] {
        im.put(x + dx, y - 22 + dy, 'k');
    }
    im.rect(x - 1, y - 23, 3, 3, ink);
    im.put(x - 1, y - 23, 'H');
    // Come and get me: a small mark over the bubble that bobs.
    let b = tri(tick as i32, 20) / 5;
    for (dx, dy) in [(0, 0), (-1, -1), (1, -1), (-2, -2), (2, -2)] {
        im.put(x + dx, y - 31 - b + dy, 'T');
    }
}

/// The weight a blow near this seat shakes the screen with: a blow of yours, or anything heavy,
/// or any blow close by.
pub(super) fn shakes(s: &Session, me: Option<&Knight>, id: u32) -> Vec<(Weight, u32)> {
    let now = s.elapsed;
    let foot = me.map(|k| k.body.foot());
    s.show
        .juice
        .blows
        .iter()
        .filter_map(|b| {
            let near = foot.is_some_and(|(fx, fy)| (b.x - fx as f32).hypot(b.y - fy as f32) < 150.0);
            let weight = if b.owner == id || near {
                b.weight
            } else if b.weight >= Weight::Kill {
                Weight::Crushing
            } else {
                return None;
            };
            Some((weight, b.age(now)))
        })
        .collect()
}
