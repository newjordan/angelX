//! The clearing uses cached woodland scenery; only eight fireflies and
//! three rune lights animate. Browser guests receive the same host picture.

use super::super::ink::{Img, hash};
use super::super::material as m;
use super::super::kit;
use super::{TILE, at, home};
use crate::rules::{HZ, Run, forest};

pub(super) fn scenery(cv: &mut Img) {
    // The grove's old trees: dark oaks and yews at night.
    let keep = |_: i32, _: i32| true;
    let mut trees: Vec<_> = forest::TREES.iter().copied().collect();
    trees.sort_by_key(|t| t.1);
    for (col, row) in trees {
        let (x, foot) = (col * TILE + TILE / 2, row * TILE + TILE - 1);
        let v = hash(col, row, 250);
        let leaf = if v % 3 == 0 { m::Leaf::Pine } else { m::Leaf::Oak };
        m::tree(cv, x, foot + 2, 12.0 + (v >> 4) as f32 % 2.0, leaf, v, &keep);
    }
}

fn ring(cv: &mut Img, x: i32, y: i32, radius: f32, ink: char) {
    for i in 0..48 {
        let a = i as f32 * std::f32::consts::TAU / 48.0;
        cv.put(
            x + (a.cos() * radius) as i32,
            y + (a.sin() * radius) as i32,
            ink,
        );
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let game = &run.forest;
    for (pad, ink) in [(forest::LANTERN, 'O'), (forest::MEMORY, 'u')] {
        let (x, y) = at(pad.0 * 2.0, pad.1 * 2.0);
        ring(cv, x, y, 11.0, ink);
        cv.line(x - 4, y, x + 4, y, ink);
        cv.line(x, y - 4, x, y + 4, ink);
    }
    let lamp = kit::lantern(true);
    let (x, y) = at(forest::LANTERN.0 * 2.0, forest::LANTERN.1 * 2.0);
    cv.stamp(&lamp, x - lamp.w / 2, y - lamp.h - 12);
    for (i, (x, y)) in forest::RUNES.into_iter().enumerate() {
        let (x, y) = at(x * 2.0, y * 2.0);
        let lit = game.lit_rune() == Some(i);
        let ink = if lit { '9' } else { ['g', 'u', 'O'][i] };
        ring(cv, x, y, if lit { 14.0 } else { 10.0 }, ink);
        cv.line(x - 5, y, x, y - 7, ink);
        cv.line(x, y - 7, x + 5, y, ink);
        cv.line(x + 5, y, x, y + 7, ink);
        cv.line(x, y + 7, x - 5, y, ink);
    }
    // At rest the flies still show what the lantern starts; caught flies
    // disappear during the round, never affecting collision or the network.
    for i in 0..forest::FLY_COUNT {
        if game.hunt_left > 0 && game.caught & (1 << i) != 0 {
            continue;
        }
        let (x, y) = forest::firefly(i, run.tick);
        let (x, y) = at(x, y);
        let ink = if (run.tick / 8 + i as u64).is_multiple_of(2) {
            '9'
        } else {
            'O'
        };
        cv.line(x - 2, y, x + 2, y, ink);
        cv.line(x, y - 2, x, y + 2, ink);
        cv.put(x - 3, y - 1, 'g');
        cv.put(x + 3, y - 1, 'g');
    }
}

pub(super) fn boards(cv: &mut Img, run: &Run) {
    let game = &run.forest;
    let title = home::board(&[
        ("FIREFLY GROVE".into(), '9'),
        ("OLD WATCH SOUTH".into(), 'h'),
    ]);
    cv.stamp(&title, (cv.w - title.w) / 2, 4);
    let hunt = if game.hunt_left > 0 {
        format!(
            "{}/8  {}S",
            game.caught.count_ones(),
            game.hunt_left.div_ceil(HZ)
        )
    } else {
        format!("BEST {}/8  WINS {}", game.best, game.hunts)
    };
    let song = if game.show_left > 0 {
        "WATCH THE FOUR FLASHES".into()
    } else if game.singing {
        format!("YOUR TURN {}/4", game.note)
    } else {
        format!("SONGS SOLVED {}", game.songs)
    };
    let left = home::board(&[
        ("FIREFLY HUNT".into(), '9'),
        ("STEP ON GOLD RING".into(), 'O'),
        ("30S: WALK INTO 8 LIGHTS".into(), 'h'),
        (hunt, '9'),
    ]);
    let right = home::board(&[
        ("WOODLAND MEMORY".into(), '9'),
        ("STEP ON BLUE RING".into(), 'u'),
        ("REPEAT FLASHES BY WALKING".into(), 'h'),
        (song, '9'),
    ]);
    // Each game's rules show while it plays or a knight is at its ring.
    use crate::rules::forest::{LANTERN, MEMORY};
    if game.hunt_left > 0 || super::knight_near(run, LANTERN, 3.5) {
        cv.stamp(&left, 4, cv.h - left.h - 4);
    }
    if game.show_left > 0 || game.singing || super::knight_near(run, MEMORY, 3.5) {
        cv.stamp(&right, cv.w - right.w - 4, cv.h - right.h - 4);
    }
}
