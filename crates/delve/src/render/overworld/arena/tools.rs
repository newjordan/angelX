//! The tools of the trades, drawn in a knight's hand (the Works,
//! `rules::trades`): a pick, a chisel, a hod on the shoulder, a mallet, a
//! carpenter's hammer, a master's dividers and the clerk's ledger, and the
//! rest of the economy's tools in the same hand. Each is drawn along a line
//! from the hand toward where the rig sends it (as `blades` draws an arm), so
//! a pick reads as a pick however it is swung, and a hod rides the shoulder
//! with its load.
//!
//! Wood is the timber inks (`r` `R` `o`), iron the stone inks (`v` `V` `W`),
//! cloth and mortar the pale golds and greys. No signal ink: a tool is no
//! game state.

use super::super::super::ink::Img;
use crate::rules::trades::Tool;

/// The tool a `Hero::tool` code names, and the load on a hod.
pub fn of(code: u8) -> Option<(Tool, u8)> {
    Tool::unpack(code)
}

/// A code for a tool and a load.
pub fn code(tool: Tool, load: u8) -> u8 {
    tool.pack(load)
}

fn dot(cv: &mut Img, (x, y): (f32, f32), ink: char) {
    cv.put(x.round() as i32, y.round() as i32, ink);
}

/// `tool` from the hand at `hand` toward `dir` (any length), on `cv`; `load`
/// is what a hod holds; `struck`: the tool mid-stroke, its iron catching the
/// light.
pub fn draw(cv: &mut Img, tool: Tool, load: u8, hand: (f32, f32), dir: (f32, f32), struck: bool) {
    let n = dir.0.hypot(dir.1);
    if n <= 0.0 {
        return;
    }
    let (ux, uy) = (dir.0 / n, dir.1 / n);
    // Across the line: `up` is the side facing up the picture.
    let (mut px, mut py) = (-uy, ux);
    if py > 0.0 || (py == 0.0 && px < 0.0) {
        (px, py) = (-px, -py);
    }
    let at = |t: f32, s: f32| (hand.0 + ux * t + px * s, hand.1 + uy * t + py * s);
    let steps = |from: f32, to: f32| {
        let k = ((to - from) * 2.0).ceil().max(1.0) as i32;
        (0..=k).map(move |i| from + (to - from) * i as f32 / k as f32)
    };
    let iron = if struck { 'W' } else { 'V' };
    let haft = |cv: &mut Img, from: f32, to: f32| {
        for t in steps(from, to) {
            dot(cv, at(t, 0.0), if (t as i32) % 3 == 0 { 'R' } else { 'r' });
        }
    };
    match tool {
        Tool::Pick => {
            // A haft, and a head across it ending in two points: the pick.
            haft(cv, -2.0, 9.0);
            for s in steps(-4.0, 4.0) {
                dot(cv, at(9.5, s), if s.abs() < 1.0 { 'U' } else { iron });
            }
            dot(cv, at(10.5, -4.6), 'v');
            dot(cv, at(10.5, 4.6), 'v');
            dot(cv, at(11.2, -5.2), 'W');
            dot(cv, at(11.2, 5.2), 'W');
        }
        Tool::Chisel => {
            // A short cold chisel, a wooden grip, a flat bright tip.
            for t in steps(-1.5, 1.0) {
                dot(cv, at(t, 0.0), 'o');
            }
            for t in steps(1.5, 6.0) {
                dot(cv, at(t, 0.0), 'v');
                dot(cv, at(t, 0.7), iron);
            }
            dot(cv, at(6.8, 0.0), 'W');
        }
        Tool::Mallet => {
            // A haft and a heavy block of a head.
            haft(cv, -2.0, 6.0);
            for t in steps(5.5, 8.5) {
                for s in steps(-2.0, 2.0) {
                    dot(cv, at(t, s), if s > 0.5 { 'o' } else { 'R' });
                }
            }
            dot(cv, at(5.2, 2.2), 'O');
            dot(cv, at(8.8, 0.0), 'O');
        }
        Tool::Hammer => {
            haft(cv, -2.0, 7.0);
            for t in steps(6.5, 8.5) {
                for s in steps(-1.8, 1.8) {
                    dot(cv, at(t, s), if s > 0.4 { 'V' } else { 'U' });
                }
            }
            dot(cv, at(9.0, 0.0), iron);
        }
        Tool::Dividers => {
            // Two legs hinged at the top, spread to a point each.
            for t in steps(0.0, 8.0) {
                dot(cv, at(t, t * 0.16), 'V');
                dot(cv, at(t, -t * 0.16), 'v');
            }
            dot(cv, at(0.0, 0.0), 'o');
            dot(cv, at(8.5, 1.4), 'W');
            dot(cv, at(8.5, -1.4), 'W');
        }
        Tool::Ledger => {
            // A small book, cloth cover, held out.
            for t in steps(1.0, 6.0) {
                for s in steps(-2.0, 2.0) {
                    dot(cv, at(t, s), if s > 1.2 { 'T' } else { 'B' });
                }
            }
            dot(cv, at(3.5, 0.0), 'o');
        }
        Tool::Tongs => {
            haft(cv, -1.0, 6.0);
            for t in steps(6.0, 9.0) {
                dot(cv, at(t, 0.8), 'v');
                dot(cv, at(t, -0.8), 'v');
            }
            dot(cv, at(9.4, 0.0), 'W');
        }
        Tool::Axe => {
            haft(cv, -2.0, 9.0);
            for t in steps(6.0, 10.0) {
                dot(cv, at(t, 2.2 + (t - 6.0) * 0.2), iron);
                dot(cv, at(t, 1.2), 'U');
            }
        }
        Tool::Saw => {
            for t in steps(-1.0, 9.0) {
                dot(cv, at(t, 0.0), 'v');
                dot(cv, at(t, 1.0), if (t as i32) % 2 == 0 { 'V' } else { 'U' });
            }
            dot(cv, at(-1.5, 0.0), 'o');
        }
        Tool::Rake => {
            haft(cv, -2.0, 9.0);
            for s in steps(-3.0, 3.0) {
                dot(cv, at(9.5, s), 'U');
            }
            for s in [-3.0, -1.0, 1.0, 3.0] {
                dot(cv, at(10.5, s), 'v');
            }
        }
        Tool::Rope => {
            // A coil at the hip: loops of cord.
            for k in 0..3 {
                let r = 2.5 + k as f32 * 0.9;
                for a in 0..10 {
                    let t = a as f32 / 10.0 * std::f32::consts::TAU;
                    dot(cv, (hand.0 + 3.0 + t.cos() * r * 0.8, hand.1 + 2.0 + t.sin() * r * 0.6), if k == 1 { 'O' } else { 'o' });
                }
            }
        }
        Tool::Ladle => {
            haft(cv, -2.0, 7.0);
            for s in steps(-1.8, 1.8) {
                dot(cv, at(7.5, s), 'U');
            }
            dot(cv, at(8.2, 1.5), 'v');
            dot(cv, at(8.2, -1.5), 'v');
        }
        Tool::Crook => {
            haft(cv, -2.0, 11.0);
            for k in 0..5 {
                let t = k as f32 / 4.0 * std::f32::consts::PI;
                dot(cv, at(11.0 + t.sin() * 2.0, -t.cos() * 2.0 + 2.0), 'R');
            }
        }
        Tool::Sack => {
            // A sack over the shoulder, tied at the neck.
            for t in steps(0.0, 5.0) {
                for s in steps(-2.5, 2.5) {
                    dot(cv, at(t, s), if s > 0.8 { 'T' } else { '$' });
                }
            }
            dot(cv, at(0.5, 0.0), 'o');
        }
        Tool::Knife => {
            for t in steps(-1.0, 1.5) {
                dot(cv, at(t, 0.0), 'B');
            }
            for t in steps(2.0, 6.0) {
                dot(cv, at(t, 0.0), iron);
            }
        }
        Tool::Hod => {
            // A pole on the shoulder, and the hod at its end: a trough of
            // boards, its load heaped in it.
            haft(cv, -2.0, 6.0);
            let (cx, cy) = at(6.0, -1.0);
            let (cx, cy) = (cx.round() as i32, cy.round() as i32);
            // The trough: a V of boards.
            cv.rect(cx - 3, cy - 1, 7, 4, 'r');
            cv.rect(cx - 3, cy - 1, 7, 1, 'o');
            cv.put(cx - 3, cy + 3, 'B');
            cv.put(cx + 3, cy + 3, 'B');
            cv.rect(cx - 2, cy + 3, 5, 1, 'B');
            match load {
                1 => {
                    // A cut stone, pale, with a lit top.
                    cv.rect(cx - 2, cy - 4, 5, 3, 'U');
                    cv.rect(cx - 2, cy - 4, 5, 1, 'V');
                }
                2 => {
                    // Mortar, a grey heap.
                    cv.rect(cx - 2, cy - 3, 5, 2, 'G');
                    cv.rect(cx - 1, cy - 4, 3, 1, 'h');
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_carries_the_tool_and_what_is_on_the_hod() {
        for t in Tool::ALL {
            for load in 0..=2 {
                let (back, got) = of(code(t, load)).expect("a code reads");
                assert_eq!((back, got), (t, load));
            }
        }
        assert!(of(0).is_none());
    }

    #[test]
    fn every_tool_leaves_ink_in_the_hand_and_none_off_a_small_canvas() {
        for t in Tool::ALL {
            for dir in [(1.0, 0.0), (0.0, -1.0), (-1.0, -1.0), (0.75, -1.0), (1.0, 1.0)] {
                let mut cv = Img::new(40, 40);
                draw(&mut cv, t, 1, (20.0, 20.0), dir, false);
                let inked = (0..40).flat_map(|y| (0..40).map(move |x| (x, y))).filter(|&(x, y)| cv.solid(x, y)).count();
                assert!(inked >= 4, "{t:?} {dir:?}: {inked} pixels");
            }
        }
    }
}
