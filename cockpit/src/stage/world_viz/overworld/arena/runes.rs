//! Power runes, drawn. A rune is a small orb hovering over a ring of light
//! on the floor, its colour and its sign its kind: Haste's rust bolt, Double
//! Damage's blue crossed blades, Regeneration's green cross, Arcane's teal
//! star, Invisibility's ghost-grey eye (the orb itself half there),
//! Illusion's brass four, Bounty's gold with a gem in it, Wisdom's
//! parchment with an open book.
//! Just before it wells up, motes gather where it will be. A knight carrying
//! one has its motes circling their feet; under Invisibility, the knight is
//! only half there too.

use super::super::ink::Img;
use super::at;
use crate::drive::together_shooter::Run;
use crate::drive::together_shooter::runes::RuneKind;

/// A kind's inks: outline, the orb's shadowed and lit halves, its sign,
/// and the glint.
fn inks(kind: RuneKind) -> [char; 5] {
    match kind {
        RuneKind::Haste => ['n', 'p', 'R', 'T', 't'],
        RuneKind::DoubleDamage => ['s', 'q', 'Q', 'W', 'z'],
        RuneKind::Regeneration => ['f', 'l', 'C', 'Y', 'y'],
        RuneKind::Arcane => ['0', '1', '2', '3', 'w'],
        RuneKind::Invisibility => ['x', 'u', 'U', 'W', 'V'],
        RuneKind::Illusion => ['n', 'I', 'o', 'T', 'O'],
        RuneKind::Bounty => ['a', '4', '5', 'a', '6'],
        RuneKind::Wisdom => ['P', '$', 'c', 'B', '9'],
    }
}

/// A kind's sign, five by five, set in the orb.
fn sign(kind: RuneKind) -> [&'static str; 5] {
    match kind {
        RuneKind::Haste => ["..##.", ".##..", "#####", "..##.", ".##.."],
        RuneKind::DoubleDamage => ["#...#", ".#.#.", "..#..", ".#.#.", "#...#"],
        RuneKind::Regeneration => ["..#..", "..#..", "#####", "..#..", "..#.."],
        RuneKind::Arcane => ["..#..", "#.#.#", ".###.", "#.#.#", "..#.."],
        RuneKind::Invisibility => [".....", ".###.", "#.#.#", ".###.", "....."],
        RuneKind::Illusion => ["##.##", "##.##", ".....", "##.##", "##.##"],
        RuneKind::Bounty => ["..#..", ".###.", "#####", ".###.", "..#.."],
        RuneKind::Wisdom => [".....", "#####", "#.#.#", "#####", "....."],
    }
}

/// The orb: eleven across, lit from above, its sign in the middle.
pub(super) fn orb(kind: RuneKind) -> Img {
    let [line, dark, lit, mark, glint] = inks(kind);
    let mut im = Img::new(11, 11);
    for y in 0..11 {
        for x in 0..11 {
            let d = ((x as f32 - 5.0).powi(2) + (y as f32 - 5.0).powi(2)).sqrt();
            if d > 5.6 {
                continue;
            }
            let ink = if d > 4.6 {
                line
            } else if (x, y) == (3, 2) || (x, y) == (2, 3) || (x, y) == (3, 3) {
                glint
            } else if y <= 4 {
                lit
            } else {
                dark
            };
            im.put(x, y, ink);
        }
    }
    if kind == RuneKind::Invisibility {
        // The orb half there; its eye, whole.
        im = veiled(&im);
    }
    for (row, cells) in sign(kind).iter().enumerate() {
        for (col, cell) in cells.chars().enumerate() {
            if cell == '#' {
                im.put(3 + col as i32, 3 + row as i32, mark);
            }
        }
    }
    im
}

/// Half there: every other pixel, in a checker.
pub(super) fn veiled(im: &Img) -> Img {
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if (x + y) % 2 == 0
                && let Some(c) = im.get(x, y)
            {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// The room's rune: gathering motes just before it wells up, then the orb,
/// bobbing over its ring.
pub(super) fn floor(cv: &mut Img, run: &Run) {
    let Some(rune) = run.rune else {
        return;
    };
    let [_, _, lit, mark, _] = inks(rune.kind);
    let (x, y) = at(rune.x, rune.y);
    let tick = run.tick;
    if tick < rune.at {
        let coming = rune.at - tick;
        if coming > 24 {
            return;
        }
        let r = coming as f32 * 0.6 + 2.0;
        for k in 0..6 {
            let a = k as f32 / 6.0 * std::f32::consts::TAU + coming as f32 * 0.1;
            cv.put(
                x + (a.cos() * r).round() as i32,
                y - 6 + (a.sin() * r * 0.6).round() as i32,
                mark,
            );
        }
        return;
    }
    // The ring on the floor, turning.
    let turn = (tick / 3) as i32;
    for k in 0..24 {
        if (k + turn) % 4 == 0 {
            continue;
        }
        let a = k as f32 / 24.0 * std::f32::consts::TAU;
        cv.put(
            x + (a.cos() * 7.0).round() as i32,
            y + (a.sin() * 2.5).round() as i32,
            lit,
        );
    }
    let bob = if (tick / 10).is_multiple_of(2) { 0 } else { 1 };
    let im = orb(rune.kind);
    cv.stamp(&im, x - im.w / 2, y - 14 + bob);
}

/// A knight's rune: its motes circling their feet, flickering in the last
/// three seconds.
pub(super) fn auras(cv: &mut Img, run: &Run) {
    let hz = crate::drive::together_shooter::HZ;
    for hero in run
        .players
        .values()
        .filter(|h| h.hp > 0 && h.privy == 0 && !h.stone)
    {
        if hero.singing {
            // Sir Dinadan's song, heard: a note above the knight.
            let (x, y) = at(hero.x, hero.y);
            let bob = ((run.tick / 8) % 4) as i32;
            for (dy, row) in ["..h", "..h", "hhh", "hh."].iter().enumerate() {
                for (dx, cell) in row.chars().enumerate() {
                    if cell == 'h' {
                        cv.put(x + 7 + dx as i32, y - 24 - bob + dy as i32, 'h');
                    }
                }
            }
        }
        if hero.immune > 0 {
            // A Pendragon Sceptre: a golden ring turning the other way.
            let (x, y) = at(hero.x, hero.y);
            let spin = -(run.tick as f32) * 0.12;
            for k in 0..6 {
                let a = spin + k as f32 * std::f32::consts::TAU / 6.0;
                cv.put(
                    x + (a.cos() * 10.0).round() as i32,
                    y - 6 + (a.sin() * 9.0).round() as i32,
                    if k % 2 == 0 { '6' } else { '5' },
                );
            }
        }
        let Some(held) = hero.rune else {
            continue;
        };
        if held.left < 3 * hz && (run.tick / 4).is_multiple_of(2) {
            continue;
        }
        let [_, _, lit, mark, _] = inks(held.kind);
        let (x, y) = at(hero.x, hero.y);
        let spin = run.tick as f32 * 0.15;
        for k in 0..4 {
            let a = spin + k as f32 * std::f32::consts::TAU / 4.0;
            let (mx, my) = (
                x + (a.cos() * 8.0).round() as i32,
                y + 1 + (a.sin() * 3.0).round() as i32,
            );
            cv.rect(mx, my, 2, 2, lit);
            cv.put(mx, my, mark);
        }
    }
}
