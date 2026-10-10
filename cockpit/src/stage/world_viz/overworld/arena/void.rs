//! A Black Hole on the floor. While it warns, a ring at its reach draws in
//! toward the middle; open, it's a dark core with a cold rim, three arms
//! turning in, and motes of the floor being dragged along them.

use super::super::ink::Img;
use super::{UNIT, at};
use crate::drive::together_shooter::Run;
use crate::drive::together_shooter::hollow::{HOLE_CORE, HOLE_REACH, HOLE_WARN};
use std::f32::consts::TAU;

pub(super) fn holes(cv: &mut Img, run: &Run) {
    let tick = run.tick as f32;
    for hole in &run.holes {
        let (x, y) = at(hole.x, hole.y);
        let reach = HOLE_REACH * UNIT;
        let core = HOLE_CORE * UNIT;
        if hole.warn > 0 {
            // The warning: a ring drawing in, a dark seed at the middle.
            let k = 1.0 - hole.warn as f32 / HOLE_WARN as f32;
            let r = reach * (1.0 - 0.45 * k);
            let n = (TAU * r / 2.0) as i32;
            for i in 0..n {
                if (i + tick as i32 / 2) % 3 == 0 {
                    continue;
                }
                let a = i as f32 / n as f32 * TAU + tick * 0.05;
                cv.put(x + (a.cos() * r) as i32, y + (a.sin() * r) as i32, '1');
            }
            disc(cv, (x, y), core * k, 'k');
            continue;
        }
        // Open: the arms first, then the core over them.
        let fade = (hole.left as f32 / 12.0).min(1.0);
        for arm in 0..3 {
            let base = arm as f32 * TAU / 3.0 - tick * 0.08;
            let mut r = reach;
            while r > core {
                // A spiral: the angle turns as it nears the middle.
                let a = base + (reach - r) / reach * 3.2;
                let ink = if r > reach * 0.66 {
                    'q'
                } else if r > reach * 0.33 {
                    'Q'
                } else {
                    '1'
                };
                if fade > 0.5 || ((r as i32) % 2 == 0) {
                    cv.put(x + (a.cos() * r) as i32, y + (a.sin() * r) as i32, ink);
                }
                r -= 1.4;
            }
        }
        // Motes of the floor, dragged in along the arms.
        for m in 0..10 {
            let life = ((tick * 0.6 + m as f32 * 13.0) % 40.0) / 40.0;
            let r = reach * (1.0 - life) + core * life;
            let a = m as f32 * 2.3 + life * 4.0 - tick * 0.05;
            cv.put(x + (a.cos() * r) as i32, y + (a.sin() * r) as i32, 'J');
        }
        disc(cv, (x, y), core, 'k');
        // The rim: a cold light round the dark, flickering.
        let n = (TAU * core) as i32 + 6;
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU;
            let ink = if (i + tick as i32 / 3) % 4 == 0 {
                '3'
            } else {
                '2'
            };
            cv.put(
                x + (a.cos() * core) as i32,
                y + (a.sin() * core) as i32,
                ink,
            );
        }
    }
}

fn disc(cv: &mut Img, (x, y): (i32, i32), r: f32, ink: char) {
    let ri = r.ceil() as i32;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            if ((dx * dx + dy * dy) as f32) <= r * r {
                cv.put(x + dx, y + dy, ink);
            }
        }
    }
}
