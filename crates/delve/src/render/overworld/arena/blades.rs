//! A knight's melee arm, drawn along a line from the hand to the tip: a
//! sword's blade with its bright edge, the guard across it, the grip and
//! the pommel; an axe's bearded head; a maul's block; a spear's leaf; a
//! cleaver's slab. A rig's grip-to-tip (`knights`), a swing's sweep
//! (`melee`) and a figure at rest share the one picture, so a sword reads
//! as a sword however it is turned.
//!
//! Steel is the stone inks, brightest on the edge that faces up the
//! picture; brass is the timber's pale golds; hafts and grips its browns.
//! No signal ink: a weapon is no game state.

use super::super::ink::Img;
use crate::rules::Hero;

/// What a knight swings: the rules' own reading of the arm
/// (`rules::melee::Blade`), so a stroke sounds as the arm it draws.
pub(super) use crate::rules::melee::Blade as Arm;

/// Whether the figure a knight is drawn as already carries its melee arm
/// in its own strike (a champion, the Swordsman, the Berserker, the
/// Gravecaller): their swing draws only the slash, never a second blade.
pub(super) fn drawn_by_body(hero: &Hero) -> bool {
    use super::class_looks::Look;
    crate::rules::champions::of(hero).is_some()
        || matches!(Look::of(hero), Some(Look::Swordsman | Look::Berserker | Look::Gravecaller))
}

/// One pixel of the arm, kept within the canvas.
fn dot(cv: &mut Img, (x, y): (f32, f32), ink: char) {
    cv.put(x.round() as i32, y.round() as i32, ink);
}

/// The arm `arm` from the hand at `hand` toward `dir` (any length), its
/// tip `len` pixels out, on `cv`. `lit`: the blade catching the light, as
/// it does mid-swing.
pub(super) fn draw(cv: &mut Img, arm: Arm, hand: (f32, f32), dir: (f32, f32), len: f32, lit: bool) {
    let n = dir.0.hypot(dir.1);
    if n <= 0.0 || len < 3.0 {
        return;
    }
    let (ux, uy) = (dir.0 / n, dir.1 / n);
    // Across the arm: `up` is the side facing up the picture, where the
    // edge catches the light.
    let (mut px, mut py) = (-uy, ux);
    if py > 0.0 || (py == 0.0 && px < 0.0) {
        (px, py) = (-px, -py);
    }
    let at = |t: f32, s: f32| (hand.0 + ux * t + px * s, hand.1 + uy * t + py * s);
    let steps = |from: f32, to: f32| {
        let k = ((to - from) * 2.0).ceil().max(1.0) as i32;
        (0..=k).map(move |i| from + (to - from) * i as f32 / k as f32)
    };
    let edge = if lit { 'W' } else { 'V' };
    match arm {
        Arm::Sword => {
            // Pommel and grip behind the hand, the guard across just ahead
            // of it, then the blade: an edge, a core and a dark flat, to a
            // bright point.
            let wide = len >= 10.0;
            dot(cv, at(-2.5, 0.0), 'O');
            for t in steps(-1.5, 0.5) {
                dot(cv, at(t, 0.0), 'B');
            }
            for s in steps(-2.0, 2.0) {
                dot(cv, at(1.2, s), if s.abs() < 0.6 { 't' } else { 'O' });
            }
            for t in steps(2.0, len - 1.5) {
                dot(cv, at(t, 0.0), 'v');
                dot(cv, at(t, 0.8), edge);
                if wide {
                    dot(cv, at(t, -0.8), 'U');
                }
            }
            dot(cv, at(len - 1.0, 0.4), edge);
            dot(cv, at(len, 0.0), 'W');
        }
        Arm::Spear => {
            // A long haft, a collar, a leaf point.
            for t in steps(-3.0, len - 3.5) {
                dot(cv, at(t, 0.0), if (t as i32) % 3 == 0 { 'R' } else { 'r' });
            }
            dot(cv, at(len - 3.5, 0.0), 'O');
            for t in steps(len - 3.0, len) {
                let w = ((len - t) / 3.0).min(1.0) * 1.1;
                dot(cv, at(t, 0.0), 'v');
                dot(cv, at(t, w), edge);
                dot(cv, at(t, -w), 'U');
            }
            dot(cv, at(len + 0.6, 0.0), 'W');
        }
        Arm::Axe => {
            // The haft, and a bearded head off its far end: the edge is the
            // head's outer curve, a spike behind.
            for t in steps(-2.0, len) {
                dot(cv, at(t, 0.0), if (t as i32) % 3 == 0 { 'R' } else { 'r' });
            }
            let head = len - 3.5;
            for t in steps(head, len) {
                let beard = 1.0 + (t - head) * 0.35;
                for s in steps(0.8, 1.0 + beard) {
                    dot(cv, at(t, s), 'v');
                }
                dot(cv, at(t, 1.6 + beard), edge);
            }
            dot(cv, at(len - 1.5, -1.0), 'U');
            dot(cv, at(head - 0.5, 1.2), 'u');
        }
        Arm::Maul => {
            // A heavy haft, and an iron block across its end, lit on top.
            for t in steps(-2.0, len - 2.0) {
                dot(cv, at(t, 0.0), 'r');
                dot(cv, at(t, -0.6), 'B');
            }
            let head = len - 3.5;
            for t in steps(head, len) {
                for s in steps(-2.5, 2.5) {
                    let ink = if s > 1.6 { edge } else if s < -1.6 { 'u' } else { 'U' };
                    dot(cv, at(t, s), ink);
                }
            }
            dot(cv, at(len, 2.5), 'W');
        }
        Arm::Cleaver => {
            // A short grip, then a slab hanging off one side of the spine.
            for t in steps(-1.5, 1.0) {
                dot(cv, at(t, 0.0), 'B');
            }
            dot(cv, at(1.4, 0.0), 'O');
            for t in steps(2.0, len) {
                dot(cv, at(t, 0.0), 'U');
                for s in steps(-2.2, -0.6) {
                    dot(cv, at(t, s), 'v');
                }
                dot(cv, at(t, -2.8), edge);
            }
            dot(cv, at(len - 1.5, -1.6), 'u');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_arm_draws_steel_brass_or_wood_and_no_signal_ink() {
        use super::super::super::ink::is_signal;
        for arm in [Arm::Sword, Arm::Axe, Arm::Maul, Arm::Spear, Arm::Cleaver] {
            for k in 0..16 {
                let a = k as f32 / 16.0 * std::f32::consts::TAU;
                let mut cv = Img::new(40, 40);
                draw(&mut cv, arm, (20.0, 20.0), (a.cos(), a.sin()), 12.0, k % 2 == 0);
                let mut inked = 0;
                for y in 0..40 {
                    for x in 0..40 {
                        if let Some(c) = cv.get(x, y) {
                            inked += 1;
                            assert!(!is_signal(c), "{arm:?} at {k}: signal ink");
                        }
                    }
                }
                assert!(inked >= 12, "{arm:?} at {k}: {inked} pixels");
            }
        }
    }
}
