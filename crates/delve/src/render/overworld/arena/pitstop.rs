//! The Sanctuary's privy theatre: a seat-local, oversized pixel window.
//!
//! The arena keeps only the stalls, their rattling doors and occupied signs.
//! The focused seat's six-stage visit plays at twice the arena sprite scale
//! in an ornate realm-ink window. Simulation ticks drive every frame, flight,
//! sparkle and transition, including browser frames and replayed snapshots.
//! A seat outside the privy never inherits another knight's window.

use super::super::chrome::{self, Tone};
use super::super::ink::{Img, hash, ink, text_width};
use super::{NATIVE_H, NATIVE_W, at, knights, sprites};
use crate::rules::pitstop::{
    CHOOSE_WAIT, FLIGHT_TICKS, GOOD_BAND, PERFECT_BAND, Quality, RESULT_TICKS, STRAPS, Stage, Stop,
    seconds,
};
use crate::rules::{HZ, Hero, Run};

pub(super) fn privy_open(tick: u32) -> Img {
    let mut im = sprites::privy();
    im.rect(11, 9, 8, 16, 'k');
    im.rect(10, 9, 2, 16, 'R');
    im.rect(12, 19, 6, 2, 'P');
    im.rect(14, 19, 2, 1, 'k');
    im.put(
        17,
        13,
        if (tick / 5).is_multiple_of(2) {
            'O'
        } else {
            'T'
        },
    );
    im.put(17, 14, 'r');
    im
}

pub(super) fn flies(cv: &mut Img, x: i32, y: i32, tick: u32) {
    for i in 0..3 {
        let t = tick as f32 * (0.21 + i as f32 * 0.05) + i as f32 * 2.1;
        fly(
            cv,
            x + (t.sin() * (9.0 + i as f32 * 3.0)) as i32,
            y + ((t * 1.3).cos() * (4.0 + i as f32)) as i32,
            tick,
        );
    }
}

fn fly(cv: &mut Img, x: i32, y: i32, tick: u32) {
    cv.put(x, y, 'k');
    cv.put(x + 1, y, 'G');
    cv.put(x - 1, y - 1, if tick.is_multiple_of(2) { 'i' } else { 'J' });
    cv.put(x + 2, y - 1, 'J');
}

/// Each physical stall has its own door and restrained occupancy indicator.
pub(super) fn booth(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for (lane, (px, py)) in run.room().privy_stalls().into_iter().enumerate() {
        let (x, y) = at(px, py);
        let stop = run
            .players
            .values()
            .filter_map(|h| h.pit.stop.as_ref())
            .find(|s| {
                s.lane as usize == lane && s.stage != Stage::Strip && s.stage != Stage::Dress
            });
        let inside = stop.is_some_and(|s| s.stage == Stage::Inside);
        let open =
            stop.is_some_and(|s| matches!(s.stage, Stage::Choose | Stage::RunIn | Stage::RunOut));
        let im = if open {
            privy_open(tick)
        } else {
            sprites::privy()
        };
        let shake = if inside {
            [0, 1, 0, -1][(tick / 3 % 4) as usize]
        } else {
            0
        };
        cv.stamp(&im, x - im.w / 2 + shake, y - im.h);
        if inside {
            let sign = sprites::occupied();
            cv.stamp(
                &sign,
                x - sign.w / 2,
                y - im.h - sign.h + 2 - lane as i32 * 12,
            );
        }
        flies(cv, x, y - im.h + 5, tick);
    }
}

fn carved(run: &Run, w: i32, names: usize) -> Img {
    let ranked = run.pits.ranked();
    let h = 19 + ranked.len().clamp(1, 4) as i32 * 10;
    let mut im = Img::new(w, h);
    chrome::inset_panel(&mut im, (0, 0, w, h), Tone::TIMBER);
    // The planks' seams, a shade under the wood.
    for y in (9..h - 3).step_by(7) {
        im.line(3, y, w - 4, y, 'b');
    }
    centered(&mut im, 5, "PARTY BEST", 'O');
    im.line(4, 14, w - 5, 14, 'P');
    if ranked.is_empty() {
        centered(&mut im, 19, "NO TIMES", 'J');
    }
    for (i, e) in ranked.iter().take(4).enumerate() {
        let y = 19 + i as i32 * 10;
        let name: String = e.name.to_uppercase().chars().take(names).collect();
        im.text(5, y, &name, if i == 0 { 'O' } else { 'h' });
        let t = seconds(e.best);
        im.text(w - 5 - text_width(&t), y, &t, 'i');
    }
    for (x, y) in [(3, 3), (w - 4, 3), (3, h - 4), (w - 4, h - 4)] {
        im.put(x, y, 'J');
    }
    im
}

pub(super) fn board(cv: &mut Img, run: &Run) {
    let Some((px, py)) = run.room().privy_stalls().last().copied() else {
        return;
    };
    let im = carved(run, 108, 8);
    let (x, y) = at(px, py);
    let bx = (x + 20).min(cv.w - im.w - 5);
    cv.rect(bx + 8, y - 8, 3, 12, 'B');
    cv.rect(bx + im.w - 11, y - 8, 3, 12, 'B');
    cv.stamp(&im, bx, (y - im.h - 7).max(2));
}

pub(super) fn bob(hero: &Hero, tick: u32) -> i32 {
    if hero.pit.stop.is_some() {
        [0, 1, 2, 1][(tick / 3 % 4) as usize]
    } else {
        0
    }
}

/// Nearest-neighbour sprite transform: every enlarged texel remains crisp.
fn resized(im: &Img, w: i32, h: i32) -> Img {
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = im.get(x * im.w / w, y * im.h / h) {
                out.set(x, y, c);
            }
        }
    }
    out
}
fn scaled(im: &Img, scale: i32) -> Img {
    resized(im, im.w * scale, im.h * scale)
}
fn centered(im: &mut Img, y: i32, text: &str, ink: char) {
    im.text((im.w - text_width(text)) / 2, y, text, ink);
}
fn big_text(cv: &mut Img, cx: i32, y: i32, text: &str, ink: char) {
    let mut im = Img::new(text_width(text), 7);
    im.text(0, 0, text, ink);
    let im = scaled(&im, 2);
    cv.stamp(&im, cx - im.w / 2, y);
}
fn sparkle(cv: &mut Img, x: i32, y: i32, tick: u32) {
    let ink = ['O', 'T', 'i', 'h'][(tick / 3 % 4) as usize];
    let r = [1, 2, 3, 2][(tick / 4 % 4) as usize];
    cv.line(x - r, y, x + r, y, ink);
    cv.line(x, y - r, x, y + r, ink);
    cv.put(x, y, 'T');
}

/// The window's frame: one framed brass board (`chrome`), a brass stud in
/// each corner, and grooves under the header and over the result.
fn frame(im: &mut Img) {
    let (w, h) = (im.w, im.h);
    chrome::inset_panel(im, (0, 0, w, h), Tone::FRAMED);
    chrome::ring(im, (0, 0, w, h), 4);
    for (x, y) in [(5, 5), (w - 6, 5), (5, h - 6), (w - 6, h - 6)] {
        im.put(x, y, 'O');
        im.put(x + 1, y, 'o');
        im.put(x, y + 1, 'o');
        im.put(x + 1, y + 1, 'r');
    }
    chrome::rule(im, 8, w - 9, 27);
    chrome::rule(im, 8, w - 9, 146);
}

/// Slow distant clouds, nearer tree silhouettes, and a textured flagstone bay.
fn backdrop(cv: &mut Img, tick: u32, seed: u32) {
    cv.rect(0, 0, cv.w, cv.h, 's');
    cv.rect(0, 32, cv.w, 45, 'x');
    cv.rect(0, 64, cv.w, 30, 'D');
    cv.ellipse(36.0, 21.0, 10.0, 10.0, 'U');
    cv.ellipse(40.0, 18.0, 9.0, 9.0, 's');
    for i in 0..5 {
        let x = (i * 83 + tick as i32 / 18).rem_euclid(cv.w + 50) - 25;
        cv.ellipse(x as f32, 27.0 + (i % 2) as f32 * 9.0, 27.0, 3.0, 'x');
    }
    // Far pines move slowly; the nearer oak sprites carry the realm's leaf ink.
    for i in 0..10 {
        let x = (i * 39 - tick as i32 / 70).rem_euclid(cv.w + 40) - 20;
        cv.rect(x, 52, 3, 35, 'f');
        for row in 0..4 {
            cv.ellipse(
                (x + 1) as f32,
                (42 + row * 8) as f32,
                (6 + row * 4) as f32,
                6.0,
                'D',
            );
        }
    }
    for i in 0..7 {
        let x = (i * 57 - tick as i32 / 35).rem_euclid(cv.w + 50) - 25;
        let tree = super::super::kit::Tiles::get()
            .tree(seed.wrapping_add(i as u32))
            .recolor(&[('E', 'F'), ('l', 'E'), ('L', 'l'), ('A', 'e')]);
        cv.stamp(&scaled(&tree, 3), x - 24, 37 + i % 2 * 4);
    }
    // A mossy stone garden wall with chipped caps, behind the changing bay.
    cv.rect(0, 79, cv.w, 8, 'X');
    cv.rect(0, 78, cv.w, 2, 'G');
    for x in (0..cv.w).step_by(18) {
        cv.line(x, 81, x, 86, 'g');
        cv.rect(x + 2, 78, 7, 2, 'm');
        cv.put(x + 10, 79, 'j');
    }
    cv.rect(0, 87, cv.w, cv.h - 87, 'g');
    for y in 88..cv.h {
        for x in 0..cv.w {
            let n = hash(x / 2, y / 2, seed);
            if n % 23 == 0 {
                cv.put(x, y, 'j');
            }
            if y % 13 == 0 || (x + (y / 13 % 2) * 17) % 34 == 0 {
                cv.put(x, y, 'X');
            }
        }
    }
    for x in (6..cv.w).step_by(29) {
        cv.line(x, 84, x + 2, 87, 'm');
        cv.put(x - 1, 85, 'E');
        cv.put(x + 3, 86, 'C');
    }
    for i in 0..16 {
        let x = (hash(i, 8, seed) % cv.w as u32) as i32;
        let y = 41 + (hash(i, 9, seed) % 37) as i32;
        if (tick / 6 + i as u32) % 9 < 2 {
            cv.put(x, y, 'm');
        }
    }
}

fn flag(cv: &mut Img, x: i32, tick: u32, left: bool) {
    cv.rect(x, 65, 2, 48, 'P');
    cv.put(x, 64, 'T');
    for col in 0..17 {
        let ripple = [0, 1, 2, 1, 0, -1][((col / 3 + tick as i32 / 3) % 6) as usize];
        let px = x + if left { -col } else { col };
        let height = 13 - col / 2;
        for y in 0..height {
            cv.put(
                px,
                68 + y + ripple,
                if (col / 4 + y / 4) % 2 == 0 { 'i' } else { 's' },
            );
        }
    }
    cv.rect(x - 3, 112, 8, 2, 'X');
}

fn finish_line(cv: &mut Img, tick: u32) {
    for y in 91..114 {
        for x in 144..156 {
            cv.put(
                x,
                y,
                if ((x - 140) / 4 + (y - 91) / 4) % 2 == 0 {
                    'i'
                } else {
                    's'
                },
            );
        }
    }
    flag(cv, 140, tick, true);
    flag(cv, 153, tick + 5, false);
}

fn armour(piece: usize) -> Img {
    armour_rows(piece).recolor(super::super::kit::hero::STEEL)
}

fn armour_rows(piece: usize) -> Img {
    Img::from_rows(match piece {
        0 => &[
            "..iHHHi..",
            ".iJJJJJh.",
            "hHHHHHHh",
            "hssssssG",
            ".GJjjJG.",
            "..GggG..",
        ],
        1 => &[".iHHHi.", "hJJJJJh", "hHHHJJG", ".GjjJG.", "..ggg.."],
        _ => &[
            ".iHHHHHi.",
            "hJJJJJJJh",
            "hJ77777Jh",
            "hJ77577Jh",
            ".h77777h.",
            "..GjjjG..",
            "...ggg...",
        ],
    })
}
fn turn(im: &Img, quarter: u32) -> Img {
    let swap = quarter % 2 == 1;
    let mut out = Img::new(
        if swap { im.h } else { im.w },
        if swap { im.w } else { im.h },
    );
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                let (dx, dy) = match quarter % 4 {
                    1 => (im.h - 1 - y, x),
                    2 => (im.w - 1 - x, im.h - 1 - y),
                    3 => (y, im.w - 1 - x),
                    _ => (x, y),
                };
                out.set(dx, dy, c);
            }
        }
    }
    out
}
fn armour_flights(cv: &mut Img, stop: &Stop, hero: &Hero, id: u32) {
    for piece in 0..3 {
        let Some(toss) = stop.tossed[piece].or((stop.stage != Stage::Strip).then_some(0)) else {
            continue;
        };
        let age = stop.total.saturating_sub(toss);
        let target = (35 + piece as i32 * 31, 107);
        let fitted = stop.fitted[piece];
        if fitted.is_some_and(|t| stop.total.saturating_sub(t) >= FLIGHT_TICKS) {
            continue;
        }
        let dressing = fitted.is_some();
        let age = fitted.map_or(age, |t| stop.total.saturating_sub(t));
        let k = (age as f32 / FLIGHT_TICKS as f32).min(1.0);
        let start = (82, 64 + piece as i32 * 10);
        // Hit the floor at two thirds, then bounce there. Dressing traverses
        // the exact same path backwards, including the little floor bounce.
        let flight = if dressing { 1.0 - k } else { k };
        let x = start.0 as f32 + (target.0 - start.0) as f32 * flight;
        let y = if flight < 0.67 {
            let rise = flight / 0.67;
            start.1 as f32 + (target.1 - start.1) as f32 * rise
                - (rise * std::f32::consts::PI).sin() * 29.0
        } else {
            target.1 as f32 - ((flight - 0.67) / 0.33 * std::f32::consts::PI).sin() * 7.0
        };
        let kit = armour(piece).recolor(if hero.colours().is_empty() {
            knights::seat_colours(id)
        } else {
            hero.colours()
        });
        let spin = if dressing {
            FLIGHT_TICKS.saturating_sub(age) / 3
        } else {
            age / 3
        };
        let kit = scaled(
            &turn(&kit, if k < 1.0 { spin } else { piece as u32 % 2 }),
            2,
        );
        cv.ellipse(target.0 as f32, target.1 as f32 + 3.0, 10.0, 2.0, 'X');
        cv.stamp(&kit, x as i32 - kit.w / 2, y as i32 - kit.h / 2);
        if age == 13 || age == 16 {
            cv.line(x as i32 - 12, y as i32, x as i32 - 16, y as i32 - 3, 'J');
        }
    }
}

fn daisy(cv: &mut Img, x: i32, y: i32) {
    for (dx, dy) in [(0, -2), (0, 2), (-2, 0), (2, 0)] {
        cv.rect(x + dx - 1, y + dy - 1, 2, 2, 'i');
    }
    cv.rect(x - 1, y - 1, 2, 2, 'O');
}
fn celebration(cv: &mut Img, quality: Quality, age: u32, x: i32, y: i32) {
    match quality {
        Quality::Royal => {
            for i in 0..5 {
                daisy(cv, x - 15 + i * 7, y - 45 - ((i - 2) * (i - 2) / 2));
            }
            for i in 0..7 {
                let t = age as f32 * 0.09 + i as f32;
                sparkle(
                    cv,
                    x + (t.sin() * 37.0) as i32,
                    y - 23 + (t.cos() * 26.0) as i32,
                    age + i as u32 * 3,
                );
            }
            // Tiny trumpet, flared bell and a bobbing ribbon.
            let trumpet = Img::from_rows(&[
                "..........OO",
                ".........OTT",
                "..OOOOOOOTTT",
                ".OTTTTTTTTTT",
                "..OOOOOOOTTT",
                ".....O...OTT",
                "....OOO...OO",
            ]);
            cv.stamp(&scaled(&trumpet, 2), x + 45, y - 27 + (age / 4 % 3) as i32);
            cv.line(x + 52, y - 9, x + 51, y - 3, 'R');
        }
        Quality::Fresh => {
            cv.line(x + 33, y - 2, x + 33, y - 22, 'm');
            cv.line(x + 33, y - 9, x + 39, y - 13, 'C');
            daisy(cv, x + 33, y - 24 + (age / 6 % 2) as i32);
            sparkle(cv, x - 27, y - 32, age);
        }
        Quality::Whiffy => {
            let t = age as f32 * 0.16;
            let mut bug = Img::new(7, 5);
            fly(&mut bug, 3, 3, age);
            let bug = scaled(&bug, 2);
            cv.stamp(
                &bug,
                x + (t.cos() * 31.0) as i32 - 7,
                y - 27 + (t.sin() * 15.0) as i32 - 5,
            );
            for i in 0..2 {
                for row in 0..13 {
                    let dx = ((row as f32 * 0.6 + age as f32 * 0.2).sin() * 2.0) as i32;
                    cv.put(x - 28 + i * 9 + dx, y - 16 - row, 'm');
                }
            }
        }
        Quality::Disaster => {
            // Unrolling parchment catches a boot and ripples along the floor.
            cv.line(x + 7, y - 5, x + 12, y + 1, 'T');
            for col in 0..48 {
                let dy = ((col as f32 * 0.3 + age as f32 * 0.25).sin() * 2.0) as i32;
                cv.rect(
                    x + 11 + col,
                    y - 1 + dy,
                    1,
                    4,
                    if col % 12 == 0 { 'P' } else { 'T' },
                );
            }
            cv.ellipse((x + 62) as f32, y as f32, 6.0, 4.0, 'O');
            cv.ellipse((x + 62) as f32, y as f32, 3.0, 2.0, 'b');
        }
    }
}

fn scene(run: &Run, id: u32, hero: &Hero) -> Img {
    let tick = run.tick as u32;
    let stop = hero.pit.stop.as_ref();
    let mut cv = Img::new(336, 113);
    backdrop(
        &mut cv,
        tick,
        hash(run.room().cell.0, run.room().cell.1, run.dungeon.depth),
    );
    // The finish pennants belong to the race; clear the stage for grade props.
    if stop.is_some() {
        finish_line(&mut cv, tick);
    }
    let inside = stop.is_some_and(|s| s.stage == Stage::Inside);
    let open =
        stop.is_some_and(|s| matches!(s.stage, Stage::RunIn | Stage::RunOut | Stage::Choose));
    let shake = if inside {
        [0, 2, -2, 1, -1, 0][(tick / if stop.unwrap().poop { 2 } else { 4 } % 6) as usize]
    } else {
        0
    };
    let mut booth = if open {
        privy_open(tick)
    } else {
        sprites::privy()
    };
    if inside {
        for y in 10..16 {
            for x in 13..19 {
                if booth.get(x, y) == super::super::ink::ink('k') {
                    booth.put(x, y, if (tick / 5) % 2 == 0 { 'T' } else { 'O' });
                }
            }
        }
    }
    let booth = scaled(&booth, 2);
    cv.ellipse(213.0, 105.0, 33.0, 5.0, 'X');
    cv.stamp(&booth, 183 + shake, 104 - booth.h);
    // Ivy, step and moon-window light make the booth part of the realm.
    cv.rect(191, 104, 43, 3, 'P');
    cv.line(191, 104, 232, 104, 'O');
    for i in 0..4 {
        cv.ellipse(186.0, (62 + i * 8) as f32, 3.0, 2.0, 'm');
    }
    flies(&mut cv, 220, 47, tick);
    let ledger = carved(run, 86, 5);
    cv.rect(265, 87, 3, 20, 'B');
    cv.rect(327, 87, 3, 20, 'B');
    cv.stamp(&ledger, 249, 28);
    if inside {
        if let Some(s) = stop {
            armour_flights(&mut cv, s, hero, id);
        }
        let swing = [-3, -1, 2, 3, 1, -2][(tick / 4 % 6) as usize];
        cv.line(203, 84, 202 + swing, 92, 'P');
        cv.line(221, 84, 222 + swing, 92, 'P');
        let sign = sprites::occupied();
        cv.stamp(&sign, 213 - sign.w / 2 + swing, 92);
        let s = stop.unwrap();
        let word = if s.t < 8 {
            "SLAM!"
        } else if s.poop {
            ["PRRRT!", "PLOP!", "OOF!"][(s.t / 13 % 3) as usize]
        } else {
            "PSSS!"
        };
        let puff_y = 53 - (s.t % 13) as i32;
        cv.ellipse(159.0, puff_y as f32 + 4.0, 24.0, 9.0, 'u');
        cv.text(159 - text_width(word) / 2, puff_y, word, 'i');
    } else {
        let k = stop.map_or(0.0, Stop::progress);
        let x = match stop.map(|s| s.stage) {
            Some(Stage::RunIn) => (82.0 + 131.0 * k) as i32,
            Some(Stage::RunOut) => (213.0 - 131.0 * k) as i32,
            Some(Stage::Choose) => 213,
            _ => 82,
        };
        let running = stop.is_some_and(|s| matches!(s.stage, Stage::RunIn | Stage::RunOut));
        let cycle = (tick / if running { 2 } else { 4 } % 6) as usize;
        let hop = if running {
            [0, 2, 4, 3, 1, 0][cycle]
        } else {
            [0, 1, 4, 5, 2, 0][cycle]
        };
        cv.ellipse(x as f32, 107.0, 15.0, 3.0, 'X');
        if running {
            for i in 0..3 {
                let age = (tick + i * 4) % 12;
                let side = if stop.unwrap().stage == Stage::RunIn {
                    -1
                } else {
                    1
                };
                cv.ellipse(
                    (x + side * (15 + age as i32 * 2)) as f32,
                    (105 - age / 4) as f32,
                    (4 - age / 4) as f32,
                    2.0,
                    'J',
                );
            }
        }
        let figure = knights::pit_figure(hero, id, tick);
        let width = if cycle == 0 { 42 } else { 40 };
        let height = if cycle == 0 {
            42
        } else if cycle == 3 {
            46
        } else {
            44
        };
        let figure = resized(&figure, width, height);
        cv.stamp(&figure, x - width / 2, 105 - height - hop);
        if let Some(s) = stop {
            armour_flights(&mut cv, s, hero, id);
            if s.stage == Stage::Choose {
                for (sx, word, ink, left) in
                    [(9, "QUICK PEE", 'u', true), (105, "BIG POOP", 'P', false)]
                {
                    cv.rect(sx + 42, 78, 4, 26, 'B');
                    chrome::panel(
                        &mut cv,
                        (sx, 51, 91, 26),
                        Tone::TIMBER.rimmed(chrome::rim_of(ink)),
                    );
                    cv.text(sx + (91 - text_width(word)) / 2, 57, word, 'O');
                    let ax = sx + 45;
                    cv.line(ax - 7, 69, ax + 7, 69, 'i');
                    let end = if left { ax - 7 } else { ax + 7 };
                    cv.line(end, 69, end + if left { 3 } else { -3 }, 66, 'i');
                    cv.line(end, 69, end + if left { 3 } else { -3 }, 72, 'i');
                }
            }
            if let Some(split) = s.finish.filter(|&t| s.total.saturating_sub(t) < HZ) {
                let age = s.total - split;
                for i in 0..8 {
                    let t = i as f32 * std::f32::consts::TAU / 8.0;
                    sparkle(
                        &mut cv,
                        146 + (t.cos() * (age + 8) as f32) as i32,
                        91 + (t.sin() * (age / 2 + 5) as f32) as i32,
                        tick + i,
                    );
                }
                big_text(
                    &mut cv,
                    87,
                    16 + pop_offset(age),
                    &format!("{}s", seconds(split)),
                    'T',
                );
            }
        }
        if let Some(result) = &hero.pit.result {
            celebration(
                &mut cv,
                result.quality,
                RESULT_TICKS.saturating_sub(result.left),
                x,
                105 - hop,
            );
        }
    }
    cv
}

fn pop_offset(age: u32) -> i32 {
    [9, 3, -3, -5, -3, 0, 2, 1, 0][(age / 2).min(8) as usize]
}

/// The whole visit is one large window, including the result and its close.
pub(super) fn panel(cv: &mut Img, run: &Run, focus: Option<u32>, _legible: bool) {
    let selected = focus.or_else(|| run.players.keys().next().copied());
    let Some((id, hero)) = selected.and_then(|id| run.players.get(&id).map(|h| (id, h))) else {
        return;
    };
    if hero.pit.stop.is_none() && hero.pit.result.is_none() {
        return;
    }
    let mut window = Img::new(352, 202);
    frame(&mut window);
    window.text(14, 13, "PRIVY DASH", 'O');
    let name = hero
        .knight_name()
        .unwrap_or_else(|| hero.name.clone())
        .to_uppercase();
    let name: String = name.chars().take(16).collect();
    window.text(137, 13, &name, 'h');
    let total = hero
        .pit
        .stop
        .as_ref()
        .map_or_else(|| hero.pit.result.as_ref().unwrap().total, |s| s.total);
    let time = format!("{}s", seconds(total));
    window.text(338 - text_width(&time), 13, &time, 'O');
    window.stamp(&scene(run, id, hero), 8, 31);
    if let Some(stop) = &hero.pit.stop {
        let label = if stop.waiting {
            "HOLD TIGHT"
        } else {
            stop.stage.name()
        };
        centered(&mut window, 151, label, 'i');
        match stop.stage {
            Stage::Strip | Stage::Dress => {
                let (bx, by, bw, bh) = (37, 166, 278, 17);
                chrome::inset_panel(&mut window, (bx - 2, by - 2, bw + 4, bh + 4), Tone::WELL.rimmed(chrome::rim_of('u')));
                window.rect(bx, by, bw, bh, 'k');
                for (half, ink) in [(GOOD_BAND, 'l'), (PERFECT_BAND, 'Y')] {
                    window.rect(
                        bx + ((0.5 - half) * bw as f32) as i32,
                        by,
                        (half * 2.0 * bw as f32) as i32,
                        bh,
                        ink,
                    );
                }
                window.line(bx, by, bx + bw - 1, by, 'j');
                let mx = bx + (stop.marker() * (bw - 6) as f32) as i32;
                window.rect(mx - 1, by - 2, 8, bh + 4, 'k');
                window.rect(mx, by - 3, 6, bh + 6, if stop.lock > 0 { 'o' } else { 'i' });
                window.rect(mx + 1, by - 2, 2, bh + 4, 'H');
                for i in 0..STRAPS {
                    window.rect(
                        16,
                        166 + i as i32 * 7,
                        8,
                        4,
                        if i < stop.straps { 'Y' } else { 'g' },
                    );
                }
                let feedback = stop
                    .last
                    .filter(|(_, t)| stop.total.saturating_sub(*t) < 18);
                let hint = feedback.map_or(
                    if stop.waiting {
                        "YOUR TURN SOON"
                    } else {
                        "HIT GREEN"
                    },
                    |(g, _)| g.word(),
                );
                centered(&mut window, 189, hint, 'h');
                // A drawn button cap, with the controls branch's fire key.
                chrome::inset_panel(&mut window, (322, 167, 15, 15), Tone::SLATE);
                window.text(327, 171, "F", 'i');
            }
            Stage::Choose => {
                centered(&mut window, 169, "PICK A SIDE", 'O');
                chrome::meter(
                    &mut window,
                    (67, 185, 218, 5),
                    (216.0 * (1.0 - stop.t as f32 / CHOOSE_WAIT as f32).max(0.0)) as i32,
                    ink('u').unwrap_or_default(),
                );
            }
            _ => {
                let word = match stop.stage {
                    Stage::Inside => "SIT TIGHT",
                    _ => "GO GO!",
                };
                // Something to read while sitting: every other beat (two
                // thirds of a second) a line off the wall stands where the
                // words were.
                let read = (stop.stage == Stage::Inside && !run.home.privy_wall.is_empty())
                    .then(|| stop.t as usize / 20)
                    .filter(|beat| beat % 2 == 1);
                match read {
                    Some(beat) => {
                        let wall = &run.home.privy_wall;
                        let line = &wall[(beat / 2 + id as usize) % wall.len()];
                        centered(&mut window, 169, &format!("\"{line}\""), 'J');
                    }
                    None => centered(&mut window, 169, word, 'O'),
                }
                chrome::meter(
                    &mut window,
                    (67, 185, 218, 5),
                    (216.0 * stop.progress()) as i32,
                    ink('u').unwrap_or_default(),
                );
            }
        }
    } else if let Some(result) = &hero.pit.result {
        let age = RESULT_TICKS.saturating_sub(result.left);
        big_text(
            &mut window,
            176,
            155 + pop_offset(age),
            result.quality.word(),
            'T',
        );
        centered(
            &mut window,
            182,
            &if result.party_record && age < 2 * HZ {
                format!("PARTY RECORD!  {}s", seconds(result.total))
            } else {
                format!("{}s", seconds(result.total))
            },
            if result.party_record && age < 2 * HZ && (age / 6).is_multiple_of(2) {
                'T'
            } else {
                'i'
            },
        );
    }
    let opening = hero
        .pit
        .stop
        .as_ref()
        .map_or(1.0, |s| (s.total as f32 / 12.0).min(1.0));
    let closing = hero
        .pit
        .result
        .as_ref()
        .map_or(1.0, |r| (r.left as f32 / 10.0).min(1.0));
    // Opening decelerates into place; closing accelerates out to a thin slit.
    let ease = if closing < 1.0 {
        closing * closing
    } else {
        1.0 - (1.0 - opening).powi(3)
    };
    let w = (352.0 * (0.72 + 0.28 * ease)) as i32;
    let h = (202.0 * ease).max(2.0) as i32;
    let window = resized(&window, w, h);
    // Dim only the seat's own arena; an opaque dither veil keeps the palette.
    for y in 0..cv.h {
        for x in 0..cv.w {
            if (x + y) % 4 != 0 {
                cv.put(x, y, 'k');
            }
        }
    }
    let slam = hero
        .pit
        .stop
        .as_ref()
        .filter(|s| s.stage == Stage::Inside && s.t < 7)
        .map_or(0, |s| [2, -3, 2, -1, 1, 0, 0][s.t as usize]);
    let x = (NATIVE_W - w) / 2 + slam;
    let y = (NATIVE_H - h) / 2 + (10.0 * (1.0 - ease)) as i32;
    cv.rect(x + 3, y + 4, w, h, 'k');
    cv.stamp(&window, x, y);
}
