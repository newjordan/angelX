//! King Cinder: a round coal-black king in a charred red cloak, a jagged ember crown with red gems
//! and a bronze horn-headed staff. His body is soft and never frightening; what hurts is the
//! loudest thing on the screen for the whole of its tell. He is drawn from the fight's state (beat,
//! attack, tick, crown, plates) every frame, so the host and a friend's mirror see him alike.
use super::super::super::ink::{Img, bayer};
use super::{W, on_screen};
use crate::rules::bannerpeak::{
    boss::{self, Attack, Beat, Boss},
    session::Session,
};

/// A lit ball: an ellipse shaded from the top left by an ordered dither between four inks
/// (`tones` from the lit side to the shaded one).
fn ball(im: &mut Img, cx: i32, cy: i32, rx: i32, ry: i32, tones: [char; 4]) {
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let (u, v) = (dx as f32 / rx as f32, dy as f32 / ry as f32);
            let r2 = u * u + v * v;
            if r2 > 1.0 {
                continue;
            }
            let lit = (-u * 0.55 - v * 0.65 + 0.2).clamp(-1.0, 1.0);
            let k = (1.0 - (lit * 0.5 + 0.5)) * 3.0 + r2 * 0.6 - 0.3;
            let i = (k + (bayer(cx + dx, cy + dy) - 0.5) * 0.9).round().clamp(0.0, 3.0) as usize;
            im.put(cx + dx, cy + dy, tones[i]);
        }
    }
}

fn outline_ball(im: &mut Img, cx: i32, cy: i32, rx: i32, ry: i32, tones: [char; 4]) {
    ball(im, cx, cy, rx + 1, ry + 1, ['k'; 4]);
    ball(im, cx, cy, rx, ry, tones);
}

/// The crown's spikes along a band, one plate each, the gems red.
fn crown(im: &mut Img, cx: i32, base: i32, plates: i32, tick: u32, glow: bool) {
    let n = plates.clamp(0, 6);
    if n == 0 {
        return;
    }
    let width = 30;
    im.rect(cx - width / 2, base - 3, width, 4, 'k');
    im.rect(cx - width / 2 + 1, base - 2, width - 2, 2, 'o');
    im.rect(cx - width / 2 + 1, base - 2, width - 2, 1, 't');
    let step = width / n;
    for i in 0..n {
        let x = cx - width / 2 + i * step + step / 2;
        let tall = 6 + (i % 2) * 3 + if n <= 3 { 2 } else { 0 };
        for r in 0..tall {
            let half = ((tall - r) / 3).max(0);
            im.rect(x - half - 1, base - 3 - r, half * 2 + 3, 1, 'k');
        }
        for r in 0..tall {
            let half = ((tall - r) / 3).max(0);
            im.rect(x - half, base - 3 - r, half * 2 + 1, 1, if r > tall - 3 { 'T' } else { 't' });
        }
        // A red gem in the foot of each spike, a glint when the tell is on.
        im.rect(x - 1, base - 3, 3, 2, '7');
        im.put(x - 1, base - 3, if glow && (tick / 2 + i as u32) % 2 == 0 { 'w' } else { 'c' });
        // An ember on the tip.
        if (tick / 3 + i as u32) % 3 == 0 {
            im.put(x, base - 4 - tall, '6');
        }
    }
}

/// The staff: a bronze pole with a horned head and a gem that flares on a tell.
fn staff(im: &mut Img, from: (i32, i32), to: (i32, i32), glow: bool, tick: u32) {
    // The pole, three wide with a dark edge.
    im.line(from.0 - 1, from.1, to.0 - 1, to.1, 'k');
    im.line(from.0 + 2, from.1, to.0 + 2, to.1, 'k');
    im.line(from.0, from.1, to.0, to.1, 'P');
    im.line(from.0 + 1, from.1, to.0 + 1, to.1, 'r');
    // The horns, curling either side of the head, and the gem between them.
    let (hx, hy) = to;
    for (dx, dy) in [(-6, 0), (-7, -2), (-7, -4), (-6, -6), (-4, 2), (6, 0), (7, -2), (7, -4), (6, -6), (4, 2), (-5, -7), (5, -7)] {
        im.put(hx + dx, hy + dy, 'k');
    }
    for (dx, dy) in [(-5, 0), (-6, -1), (-6, -2), (-6, -3), (-6, -4), (-5, -5), (-3, 1), (-4, 1), (5, 0), (6, -1), (6, -2), (6, -3), (6, -4), (5, -5), (3, 1), (4, 1)] {
        im.put(hx + dx, hy + dy, if dy < -2 { 't' } else { 'o' });
    }
    im.rect(hx - 2, hy - 4, 6, 7, 'k');
    im.rect(hx - 1, hy - 3, 4, 5, 'o');
    let gem = if glow { if tick / 2 % 2 == 0 { 'w' } else { '6' } } else { '7' };
    im.rect(hx, hy - 2, 2, 3, gem);
    if glow {
        for (dx, dy) in [(-3, -6), (4, -6), (0, -8), (-6, -3), (7, -3), (1, -10)] {
            if (tick / 2 + (dx + 9) as u32) % 3 != 0 {
                im.put(hx + dx, hy + dy, '6');
            }
        }
    }
}

/// How far the fight is into the tell: 0 at its start, 1 at the strike.
fn tell_k(b: &Boss) -> f32 {
    (b.t as f32 / 30.0).clamp(0.0, 1.0)
}

/// The hazards marked on the ground and falling from the sky: a warning that closes in, a wave of
/// flame along the floor, a column of falling coals.
fn hazards(im: &mut Img, b: &Boss, cam: (i32, i32), tick: u32) {
    for h in &b.hazards {
        let (l, t, r, bt) = h.rect();
        let (x, y) = (l - cam.0, t - cam.1);
        let (w, hh) = (r - l, bt - t);
        if x > W + 20 || x + w < -20 {
            continue;
        }
        if h.tell > 0 {
            // A warning on the ground: an ordered hatch that fills in as the blow nears, a bright
            // rim that blinks, a bar of ember along the foot.
            let fill = 0.18 + 0.5 * (1.0 - h.tell as f32 / 45.0).clamp(0.0, 1.0);
            for yy in 0..hh {
                for xx in 0..w {
                    if bayer(x + xx, y + yy) < fill * if hh > 40 { 0.55 } else { 1.0 } {
                        im.put(x + xx, y + yy, if (xx + yy) % 5 == 0 { 'O' } else { 'R' });
                    }
                }
            }
            if tick / 3 % 2 == 0 {
                im.frame(x, y, w, hh, 'O');
            }
            im.rect(x, y + hh - 2, w, 2, 'O');
            if hh > 40 {
                // A column of coals about to fall: a chevron stack over its head.
                for c in 0..3 {
                    let cy = y + 4 + ((tick as i32 / 2 + c * 9) % 27);
                    im.put(x + w / 2 - 2, cy, '6');
                    im.put(x + w / 2 - 1, cy + 1, '6');
                    im.put(x + w / 2, cy + 2, '6');
                    im.put(x + w / 2 + 1, cy + 1, '6');
                    im.put(x + w / 2 + 2, cy, '6');
                }
            }
        } else if hh > 40 {
            // A column of coals falling: a faint body of heat, bright coals streaking down it, a
            // glow where it strikes the floor.
            for yy in 0..hh {
                let near = yy as f32 / hh as f32;
                for xx in 0..w {
                    if bayer(x + xx, y + yy) < 0.18 + 0.3 * near {
                        im.put(x + xx, y + yy, if (xx + yy) % 4 == 0 { 'O' } else { 'R' });
                    }
                }
            }
            for c in 0..5i32 {
                let fall = ((tick as i32 * 7 + c * 37) % (hh + 24)) - 24;
                let cx = x + 3 + (c * 5) % (w - 5).max(1);
                for t in 0..10 {
                    let yy = y + fall - t;
                    if yy >= y && yy < y + hh {
                        im.put(cx, yy, if t < 2 { 'w' } else if t < 5 { '6' } else if t < 8 { 'O' } else { 'R' });
                        if t < 4 {
                            im.put(cx + 1, yy, 'O');
                        }
                    }
                }
            }
            im.rect(x, y + hh - 4, w, 4, '7');
            im.rect(x + 2, y + hh - 3, w - 4, 2, '6');
        } else {
            // A wave or a sweep along the floor: tongues of flame that flicker, a hot base.
            im.rect(x, y + hh - 3, w, 3, '7');
            for tx in 0..w {
                let tongue = 2 + ((tx * 5 + tick as i32 * 2) % 7) % 5 + if h.vx != 0 { 0 } else { (hh - 4).max(0) * i32::from(tx % 3 == 0) / 2 };
                let top = (y + hh - 3 - tongue).max(y);
                im.rect(x + tx, top, 1, (y + hh - 3) - top, if tongue > 5 { '@' } else { 'O' });
            }
            im.rect(x, y + hh - 4, w, 1, '6');
            im.rect(x + w / 2 - 2, y + hh - 6, 4, 3, 'T');
        }
    }
}

/// King Cinder, whole: the ground he marks first, then the king.
pub(super) fn draw(im: &mut Img, s: &Session, cam: (i32, i32), tick: u32, toward: i32) {
    let b = &s.boss;
    let floor = boss::floor();
    if !on_screen(b.x, floor - 60, cam, 160) {
        return;
    }
    hazards(im, b, cam, tick);
    let dormant = b.beat == Beat::Dormant;
    let down = b.beat == Beat::Down;
    let kneel = b.beat == Beat::Wince;
    let rage = b.beat == Beat::Rage;
    let tell = b.beat == Beat::Tell;
    let strike = b.beat == Beat::Strike;
    let face = if toward >= 0 { 1 } else { -1 };
    // A king thrown by a plate shakes; a flinch shakes the body about its feet.
    let jolt = if b.flinch > 0 { ((tick as i32 * 7) % 5) - 2 } else { 0 };
    let rise = if tell && b.kind == Attack::Stomp { (tell_k(b) * 4.0) as i32 } else { 0 };
    let hop = if strike && b.kind == Attack::Stomp && b.t < 4 { -4 + b.t as i32 * 1 } else { 0 };
    let cx = b.x - cam.0 + jolt;
    let fy = floor - cam.1;
    let h = b.head - rise + hop;
    let head_cy = fy - h + 12;
    let breathe = if dormant || down { 0 } else { ((tick / 12) % 2) as i32 };

    // The cloak: a bell to the floor, charred, its hem torn, a clasp at the throat.
    let cloak = if rage && tick / 2 % 2 == 0 { ['@', 'O', 'R', '7'] } else if down { ['j', 'g', 'X', 'K'] } else { ['R', '7', 'p', '8'] };
    let shoulder = fy - h + 24;
    for yy in shoulder..fy {
        let t = (yy - shoulder) as f32 / (fy - shoulder).max(1) as f32;
        let half = (15.0 + t * 8.0).round() as i32;
        for xx in -half..=half {
            let (u, vv) = (xx as f32 / half as f32, t);
            let lit = -u * 0.5 + (1.0 - vv) * 0.3;
            let k = (1.0 - (lit * 0.5 + 0.5)) * 3.0;
            let i = (k + (bayer(cx + xx, yy) - 0.5) * 1.0).round().clamp(0.0, 3.0) as usize;
            let hem = yy > fy - 3 && (xx + yy).rem_euclid(5) < 2;
            if !hem {
                im.put(cx + xx, yy, if yy == fy - 3 && !down { if xx % 2 == 0 { 'o' } else { 'k' } } else { cloak[i] });
            }
        }
        let half = (15.0 + t * 8.0).round() as i32;
        im.put(cx - half - 1, yy, 'k');
        im.put(cx + half + 1, yy, 'k');
    }
    // Folds down the cloak.
    for fold in [-9, -2, 6] {
        for yy in (shoulder + 4)..(fy - 2) {
            if (yy + fold) % 3 != 0 {
                im.put(cx + fold + (yy - shoulder) / 9, yy, cloak[3]);
            }
        }
    }
    // The clasp.
    im.rect(cx - 2, shoulder + 2, 5, 5, 'k');
    im.rect(cx - 1, shoulder + 3, 3, 3, if tell { '6' } else { 'o' });
    im.put(cx, shoulder + 4, if tell { 'w' } else { 't' });

    // Arms and the staff.
    let side = -face; // the staff is in the far hand, the near arm hangs
    let hand_x = cx + side * 24;
    let (from, to, glow) = match (b.beat, b.kind) {
        (Beat::Tell, Attack::Stomp) => ((hand_x, fy - 8), (hand_x - side * 4, fy - h - 26 - rise), true),
        (Beat::Strike, Attack::Stomp) => ((hand_x, fy - 8), (hand_x, fy - 6), false),
        (Beat::Tell, Attack::Sweep) => ((hand_x - side * 6, fy - 14), (hand_x + side * 10, fy - 56), true),
        (Beat::Strike, Attack::Sweep) => ((hand_x - side * 12, fy - 10), (hand_x - face * 34, fy - 5), false),
        (Beat::Tell, Attack::Embers) => ((hand_x, fy - 8), (hand_x - side * 6, fy - h - 34), true),
        (Beat::Strike, Attack::Embers) => ((hand_x, fy - 8), (hand_x - side * 6, fy - h - 30), true),
        (Beat::Wince, _) => ((hand_x, fy - 4), (hand_x + side * 4, fy - 40), false),
        (Beat::Down, _) => ((hand_x + side * 8, fy - 3), (hand_x + side * 34, fy - 8), false),
        _ => ((hand_x, fy - 8), (hand_x, fy - 66), false),
    };
    if !dormant {
        staff(im, from, to, glow, tick);
        // The hand on it.
        ball(im, from.0, from.1 - 2, 3, 3, ['g', 'X', 'K', 'K']);
    }
    // The body above the cloak: a round coal with a lit rim.
    let (coal, eyes_ink) = if down {
        (['J', 'G', 'j', 'g'], 'h')
    } else if rage {
        (['j', 'g', 'X', 'K'], 'w')
    } else if dormant {
        (['g', 'X', 'K', 'K'], 'x')
    } else {
        (['j', 'g', 'X', 'K'], '6')
    };
    outline_ball(im, cx, shoulder + 3 + breathe, 15, 8, coal);
    // An ermine collar round the neck: cream fur flecked with black.
    for dx in -16..=16i32 {
        let top = shoulder - 3 + dx * dx / 60 + breathe;
        for t in 0..4 {
            let ink = if (dx + t + (dx / 3)).rem_euclid(4) == 0 { 'k' } else if t == 0 { 'i' } else { '$' };
            im.put(cx + dx, top + t, ink);
        }
    }
    // Cracks of ember across the chest: the warmer the angrier.
    if !dormant && !down {
        for (dx, dy) in [(-7, 1), (-6, 2), (-5, 3), (6, 0), (7, 1), (5, 3)] {
            im.put(cx + dx, shoulder + 3 + dy, if tell || rage { '6' } else { 'R' });
        }
    }
    // Hands and arms on a strike: the near arm swings.
    let arm_up = tell && b.kind == Attack::Embers || strike && b.kind == Attack::Embers;
    let arm_y = if arm_up { fy - h - 12 } else { shoulder + 10 };
    ball(im, cx + face * 19, arm_y, 4, 5, ['j', 'g', 'X', 'K']);
    // The head, with the weight of a kneeling king in it.
    outline_ball(im, cx, head_cy + breathe, 14, 12, coal);
    // The face: eyes banked in sleep, aflame on a tell; nostrils flare.
    let ey = head_cy - 1 + breathe;
    let eye_w = if tell { 4 } else { 3 };
    for ex in [-7, 4] {
        let x = cx + ex + face;
        if down {
            // He weeps: the eyes shut, a tear running down.
            im.rect(x, ey + 1, eye_w + 1, 1, 'k');
            im.put(x - 1, ey, 'k');
            let t = (tick as i32 / 2 + ex) % 10;
            im.put(x + 1, ey + 2 + t.min(8), 'z');
            im.put(x + 1, ey + 3 + t.min(8), 'Q');
        } else {
            im.rect(x, ey, eye_w, 3, 'k');
            im.rect(x, ey + 1, eye_w, 2, eyes_ink);
            if tell && tick / 2 % 2 == 0 {
                im.put(x + 1, ey, 'w');
            }
        }
    }
    // A mouth: a flat line, a roar on a strike.
    if strike || rage {
        im.rect(cx - 5 + face * 2, head_cy + 6 + breathe, 10, 3, 'k');
        im.rect(cx - 4 + face * 2, head_cy + 7 + breathe, 8, 1, '6');
    } else if !down {
        im.rect(cx - 4 + face * 2, head_cy + 7 + breathe, 8, 1, 'k');
    } else {
        im.rect(cx - 3 + face * 2, head_cy + 8, 6, 1, 'k');
        im.put(cx - 4 + face * 2, head_cy + 9, 'k');
        im.put(cx + 3 + face * 2, head_cy + 9, 'k');
    }
    // Nostrils: steam on a tell.
    if tell {
        let k = (tick / 2) % 4;
        for (dx, sign) in [(-2, -1i32), (3, 1)] {
            im.put(cx + dx + face * 3, head_cy + 4, 'k');
            im.put(cx + dx + face * 3 + sign * (k as i32), head_cy + 3 - k as i32, 'i');
        }
    }
    // The crown: a plate per knight, the gems alight on a tell; low and in reach when he kneels.
    if !b.defeated() {
        let crown_base = fy - h + 1;
        crown(im, cx, crown_base, i32::from(b.plates).max(1), tick, tell);
        if kneel {
            // The window: a ring that pulses round the crown and an arrow that drops on it.
            let pulse = (tick as i32 / 3) % 8;
            im.frame(cx - 19 - pulse / 2, crown_base - 15 - pulse / 2, 38 + pulse, 20 + pulse, if tick / 3 % 2 == 0 { 'T' } else { 'O' });
            for k in 0..2 {
                let ay = crown_base - 38 + ((tick as i32 / 2 + k * 8) % 16);
                // A fat arrow pointing down at the crown, edged dark so it reads on the sky.
                for (ax, ay2, w) in [(-1, 0, 3), (-1, 1, 3), (-1, 2, 3), (-3, 3, 7), (-2, 4, 5), (-1, 5, 3)] {
                    im.rect(cx + ax - 1, ay + ay2 - 1, w + 2, 3, 'k');
                    let _ = w;
                }
                for (ax, ay2, w) in [(-1, 0, 3), (-1, 1, 3), (-1, 2, 3), (-3, 3, 7), (-2, 4, 5), (-1, 5, 3)] {
                    im.rect(cx + ax, ay + ay2, w, 1, 'T');
                }
            }
        }
    } else {
        // The crown has fallen: it lies beside him, dull, a last gem cooling.
        let x = cx - face * 30;
        im.rect(x - 8, fy - 5, 16, 5, 'k');
        im.rect(x - 7, fy - 4, 14, 3, 'o');
        im.rect(x - 7, fy - 4, 14, 1, 't');
        for dx in [-5, 0, 5] {
            im.rect(x + dx - 1, fy - 8, 3, 3, 'o');
            im.put(x + dx, fy - 7, '7');
        }
    }
}
