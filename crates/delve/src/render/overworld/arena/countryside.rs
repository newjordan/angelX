//! Surface places, quest markers and paid building sites. Terrain and scenery
//! share authored footprints; dynamic milestones never change collision paths.
use super::super::ink::Img;
use super::super::{kit, light::Light, material as m};
use super::super::ink::{hash, vnoise};
use super::{TILE, at, fire, home};
use crate::rules::{
    RoomKind, Run,
    countryside::{self, LAMPS, PLOTS, RELICS, SURVEY},
};

fn ring(cv: &mut Img, x: i32, y: i32, ink: char) {
    for i in 0..40 {
        let a = i as f32 * std::f32::consts::TAU / 40.0;
        cv.put(
            x + (a.cos() * 11.0) as i32,
            y + (a.sin() * 11.0) as i32,
            ink,
        );
    }
}

pub(super) fn scenery(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    western_scenery(cv, run);
    super::eastern::scenery(cv, run);
    super::northern::scenery(cv, run);
    super::coastal::scenery(cv, run);
    super::woodland::scenery(cv, run);
    super::wetland::scenery(cv, run);
    super::aerie::scenery(cv, run);
    super::mummers::scenery(cv, run);
    super::hippodrome::scenery(cv, run);
    super::windcountry::scenery(cv, run);
    super::bowcountry::scenery(cv, run);
    super::town::scenery(cv, run);
    match kind {
        RoomKind::Windmill => {
            // A stone mill under a thatch cap, its sails turning slowly.
            let (mx, my) = (8 * TILE, 2 * TILE - 6);
            m::block(cv, (6 * TILE + 5, my, 4 * TILE - 10, 3 * TILE + 6), m::ramp::STONE, false, 31);
            let keep = |_: i32, _: i32| true;
            m::lobe(cv, (mx as f32, (my + 2) as f32), (27.0, 10.0), m::ramp::THATCH, 32, 1.7, 0.1, &keep);
            cv.rect(mx - 4, 5 * TILE - 13, 8, 13, 'n');
            cv.rect(mx - 3, 5 * TILE - 12, 6, 12, 'b');
            cv.put(mx + 2, 5 * TILE - 6, 'o');
            let turn = (run.tick / 6) as f32 * 0.04;
            for k in 0..4 {
                let a = turn + k as f32 * std::f32::consts::FRAC_PI_2 + 0.6;
                let (dx, dy) = (a.cos(), a.sin() * 0.8);
                for t in 3..32 {
                    let (sx, sy) = (mx as f32 + dx * t as f32, (my + 8) as f32 + dy * t as f32);
                    cv.put(sx as i32, sy as i32, if t % 6 == 0 { 'B' } else { 'P' });
                    cv.put(sx as i32 + 1, sy as i32, 'b');
                    if t > 9 {
                        // The sail cloth, lattice-ruled.
                        for w in 1..8 {
                            let (cx, cy) = (sx - dy * w as f32, sy + dx * w as f32 * 0.8);
                            cv.put(cx as i32, cy as i32, if (t + w) % 4 == 0 { 'h' } else { 'i' });
                        }
                    }
                }
            }
            cv.rect(mx - 1, my + 7, 3, 3, 'B');
            // The granary, boarded under thatch.
            m::house(cv, (17 * TILE + 2, 2 * TILE - 14, 4 * TILE - 4, 30), &m::House { roof: m::ramp::THATCH, stone: false, windows: 1 }, 33);
            // Standing wheat; scenery, not unadvertised blockers.
            for x in (3 * TILE..6 * TILE).step_by(3) {
                for y in (4 * TILE + 6..6 * TILE).step_by(6) {
                    let v = hash(x, y, 34);
                    let lean = (v % 3) as i32 - 1;
                    let top = y - 5 - (v >> 3) as i32 % 2;
                    cv.line(x, y, x + lean, top, if v % 4 == 0 { 'P' } else { 'r' });
                    cv.put(x + lean, top - 1, 'O');
                    cv.put(x + lean, top - 2, if v % 3 == 0 { 't' } else { 'o' });
                }
            }
        }
        RoomKind::Ruins => {
            // Broken walls of the old watch, moss on their tops, rubble at
            // their feet.
            for (k, (c, r, w, h)) in [(4, 2, 2, 1), (6, 2, 4, 3), (18, 3, 2, 2), (6, 10, 2, 2)].into_iter().enumerate() {
                let (x, y) = (c * TILE + 1, r * TILE - 8);
                let (pw, ph) = (w * TILE - 2, h * TILE + 8);
                m::block(cv, (x, y, pw, ph), m::ramp::STONE, false, 40 + k as u32);
                // Crenels, some fallen.
                for cx in (x..x + pw).step_by(6) {
                    if hash(cx, k as i32, 41) % 3 != 0 {
                        m::block(cv, (cx, y - 4, 4, 6), m::ramp::STONE, false, 42);
                    }
                }
                // Moss down the top face.
                for mx in x..x + pw {
                    if vnoise(mx as f32 / 3.0, k as f32, 43) > 0.55 {
                        cv.put(mx, y + 1, 'l');
                        cv.put(mx, y + 2, 'E');
                        if hash(mx, k as i32, 44) % 3 == 0 {
                            cv.put(mx, y + 3, 'F');
                        }
                    }
                }
                // A doorway into the dark.
                let dx = x + pw / 2 - 3;
                cv.rect(dx, y + ph - 11, 7, 11, 'k');
                cv.line(dx, y + ph - 12, dx + 6, y + ph - 12, 'J');
                for n in 0..3 {
                    let v = hash(k as i32, n, 45);
                    m::boulder(cv, x - 4 + (v % (pw as u32 + 8)) as i32, y + ph + 4 + (v >> 8) as i32 % 3, 2.0 + (v >> 12) as f32 % 2.0, m::ramp::STONE, v);
                }
            }
        }
        RoomKind::Meadow => {
            // The coach stop: a boarded platform and a bench.
            m::boards(cv, (19 * TILE + 1, 10 * TILE + 3, 2 * TILE - 2, 10), false, 50);
            cv.line(19 * TILE, 10 * TILE, 21 * TILE - 1, 10 * TILE, 'r');
            for (x, y, v) in [(6 * TILE, 9 * TILE, 51), (15 * TILE, 4 * TILE, 52), (17 * TILE, 11 * TILE, 53), (4 * TILE, 11 * TILE, 55), (9 * TILE, 12 * TILE, 56), (20 * TILE, 4 * TILE, 57), (16 * TILE, 8 * TILE, 58), (14 * TILE, 12 * TILE, 59)] {
                m::flowers(cv, x, y, v);
                m::flowers(cv, x + 6, y + 3, v + 9);
            }
            m::boulder(cv, 22 * TILE - 4, 12 * TILE + 8, 4.0, m::ramp::STONE, 60);
            m::hay(cv, 16 * TILE, 3 * TILE - 2, 7.0, 54);
        }
        RoomKind::Orchard => {
            // Four old apple trees, apples among the leaves and in the grass.
            for (k, (c, r, w, h)) in [(4, 2, 3, 3), (17, 2, 3, 3), (4, 10, 3, 2), (17, 10, 3, 2)].into_iter().enumerate() {
                let keep = |_: i32, _: i32| true;
                let (x, foot) = (c * TILE + w * TILE / 2, (r + h) * TILE - 2);
                let rad = if h == 3 { 20.0 } else { 16.0 };
                m::tree(cv, x, foot, rad, m::Leaf::Focal, 60 + k as u32, &keep);
                for n in 0..16 {
                    let v = hash(k as i32, n, 61);
                    let (ax, ay) = (x - rad as i32 + (v % (2 * rad as u32)) as i32, foot - (rad * 2.0) as i32 + (v >> 8) as i32 % (rad as i32 + 4));
                    if cv.get(ax, ay).is_some_and(|c| c != super::super::ink::BLACK) {
                        cv.rect(ax, ay, 3, 3, 'R');
                        cv.put(ax + 2, ay + 2, 'p');
                        cv.put(ax + 1, ay + 2, 'p');
                        cv.put(ax, ay, 'o');
                    }
                }
                for n in 0..3 {
                    let v = hash(k as i32, n, 62);
                    let (fx, fy) = (x - 10 + (v % 20) as i32, foot + 2 + (v >> 6) as i32 % 3);
                    cv.rect(fx, fy, 2, 2, 'R');
                    cv.put(fx, fy, 'o');
                }
            }
            // The scarecrow minding the apologies.
            let (sx, sy) = (14 * TILE + 1, 7 * TILE);
            cv.rect(sx, sy - 2, 2, 24, 'P');
            cv.line(sx + 2, sy - 2, sx + 2, sy + 21, 'b');
            cv.line(sx - 9, sy + 4, sx + 11, sy + 4, 'P');
            cv.line(sx - 9, sy + 5, sx + 11, sy + 5, 'b');
            cv.rect(sx - 5, sy + 3, 12, 9, 'B');
            cv.line(sx - 5, sy + 3, sx + 6, sy + 3, 'r');
            cv.ellipse((sx + 1) as f32, (sy - 5) as f32, 5.0, 5.0, 'o');
            cv.ellipse(sx as f32, (sy - 6) as f32, 2.5, 2.5, 'O');
            cv.rect(sx - 7, sy - 12, 17, 3, 'p');
            cv.rect(sx - 3, sy - 16, 9, 4, 'p');
            cv.line(sx - 3, sy - 16, sx + 5, sy - 16, 'R');
        }
        RoomKind::Marsh => {
            // Rushes standing in the water, lily pads on it.
            for (c, r) in [(3, 2), (8, 3), (19, 2), (2, 11), (10, 12), (14, 2), (6, 12)] {
                m::reeds(cv, c * TILE, r * TILE + 8, (c * 31 + r) as u32);
            }
            for (c, r) in [(5, 1), (12, 2), (17, 3), (4, 11), (8, 10)] {
                let (x, y) = (c * TILE + 6, r * TILE + 9);
                cv.ellipse(x as f32, y as f32, 3.5, 2.0, 'E');
                cv.put(x - 1, y - 1, 'l');
                cv.put(x + 2, y, 'F');
            }
        }
        _ => {}
    }
    if matches!(kind, RoomKind::Gate | RoomKind::Meadow) && run.home.countryside.has(2) {
        coach(cv);
    }
    for e in crate::rules::world::ENTRANCES
        .iter()
        .filter(|e| e.room == kind && e.id.ends_with("ferry"))
    {
        ferry(cv, e.at, run.home.countryside.has(3));
    }
    for (i, lamp) in LAMPS.iter().enumerate().filter(|(_, l)| l.room == kind) {
        let (x, y) = at(lamp.at.0 * 2.0, lamp.at.1 * 2.0);
        let im = kit::lantern(run.home.countryside.lamps & (1 << i) != 0);
        cv.rect(x - 1, y - 10, 1, 12, 'P');
        cv.rect(x, y - 10, 2, 12, 'b');
        cv.line(x - 2, y + 2, x + 3, y + 2, 'K');
        cv.stamp(&im, x - im.w / 2, y - im.h - 8);
    }
    for (i, plot) in PLOTS.iter().enumerate().filter(|(_, p)| p.room == kind) {
        let (c, r, w, h) = plot.footprint;
        if !run.home.countryside.has(i) {
            // A staked-out site: corner stakes, a surveyor's line, a stack
            // of boards and a heap of stone waiting.
            let (x0, y0, x1, y1) = (c * TILE + 2, r * TILE + 6, (c + w) * TILE - 3, (r + h) * TILE - 3);
            for (sx, sy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                cv.line(sx, sy, sx, sy - 6, 'P');
                cv.put(sx, sy - 7, 'o');
                cv.put(sx + 1, sy + 1, 'K');
            }
            for x in (x0..=x1).step_by(2) {
                cv.put(x, y0 - 4, 'r');
                cv.put(x, y1 - 4, 'r');
            }
            for y in (y0 - 4..=y1 - 4).step_by(2) {
                cv.put(x0, y, 'r');
                cv.put(x1, y, 'r');
            }
            m::boards(cv, (x0 + 5, y1 - 10, 16, 6), false, i as u32);
            for n in 0..3 {
                m::boulder(cv, x1 - 8 + n * 3, y1 - 2 - (n % 2) * 3, 2.5, m::ramp::STONE, 70 + n as u32);
            }
            continue;
        }
        if plot.id == "beacon" {
            // A stone beacon tower with its fire basket.
            let (x, y) = (c * TILE + 10, r * TILE - 4);
            m::block(cv, (x, y, w * TILE - 20, h * TILE + 4), m::ramp::STONE, false, 80);
            m::boards(cv, (x - 2, y - 4, w * TILE - 16, 5), false, 81);
            cv.line(x + (w * TILE - 20) / 2, y - 4, x + (w * TILE - 20) / 2, y - 12, 'O');
            cv.line(x + (w * TILE - 20) / 2 - 3, y - 8, x + (w * TILE - 20) / 2 + 3, y - 8, 'R');
        } else if i == 4 {
            m::boards(cv, (c * TILE, r * TILE, w * TILE, h * TILE), false, 82);
            for x in [c * TILE + 6, (c + w) * TILE - 7] {
                cv.line(x, r * TILE - 7, x, (r + h) * TILE, 'o');
            }
            cv.line(
                c * TILE + 6,
                r * TILE - 5,
                (c + w) * TILE - 7,
                r * TILE - 5,
                'O',
            );
            cv.text(c * TILE + 4, r * TILE + 7, "GOAT LIFT", 'h');
            cv.rect(c * TILE + 20, r * TILE + 17, 12, 6, 'h');
            cv.line(
                c * TILE + 31,
                r * TILE + 17,
                c * TILE + 34,
                r * TILE + 12,
                'O',
            );
        } else if i == 6 {
            // Cable shed, not another pigeon loft. Its price plate and
            // four-knight arrival fan remain on walkable ground.
            m::block(cv, (c * TILE + 2, r * TILE + 4, w * TILE - 4, h * TILE - 4), m::ramp::TIMBER, true, 83);
            cv.line(
                c * TILE + 2,
                r * TILE + 8,
                (c + w) * TILE - 3,
                r * TILE + 8,
                'h',
            );
            for x in [c * TILE + 8, (c + w) * TILE - 9] {
                cv.ellipse(x as f32, (r * TILE + 17) as f32, 6.0, 6.0, 'G');
                cv.line(x - 4, r * TILE + 17, x + 4, r * TILE + 17, 'O');
            }
            cv.line(
                c * TILE + 8,
                r * TILE + 11,
                (c + w) * TILE - 9,
                r * TILE + 11,
                'h',
            );
            cv.text(c * TILE + 4, (r + h) * TILE - 10, "ROPEWAY", '9');
        } else if i == 7 {
            // A harbour office and small cargo crane, rather than an inn roof.
            m::house(cv, (c * TILE + 2, r * TILE - 6, w * TILE - 4, h * TILE + 6), &m::House { roof: m::ramp::SLATE, stone: true, windows: 1 }, 84);
            cv.line(
                c * TILE + 3,
                r * TILE + 7,
                (c + w) * TILE - 4,
                r * TILE + 7,
                'o',
            );
            
            
            cv.line(
                (c + w) * TILE - 8,
                r * TILE - 8,
                (c + w) * TILE - 8,
                (r + h) * TILE - 1,
                'o',
            );
            cv.line(
                (c + w) * TILE - 8,
                r * TILE - 8,
                (c + w) * TILE + 8,
                r * TILE - 8,
                'o',
            );
            cv.line(
                (c + w) * TILE + 8,
                r * TILE - 8,
                (c + w) * TILE + 8,
                r * TILE + 5,
                'h',
            );
            cv.text(c * TILE + 4, (r + h) * TILE - 9, "HARBOUR", '9');
        } else if plot.id == "bothy" {
            m::house(cv, (c * TILE + 3, r * TILE - 4, w * TILE - 6, h * TILE + 2), &m::House { roof: m::ramp::THATCH, stone: false, windows: 1 }, 85);
            cv.text(c * TILE + 8, (r + h) * TILE - 8, "BOTHY", '9');
        } else if plot.id == "aerie" {
            m::block(cv, (c * TILE + 2, r * TILE + 2, w * TILE - 4, h * TILE - 2), m::ramp::TIMBER, true, 86);
            for n in 0..5 {
                cv.rect((c + n) * TILE + 5, r * TILE + 15, 9, 10, 'n');
                cv.line((c + n) * TILE + 5, r * TILE + 15, (c + n) * TILE + 13, r * TILE + 15, 'O');
                cv.line((c + n) * TILE + 5, r * TILE + 25, (c + n) * TILE + 13, r * TILE + 25, 'P');
            }
            cv.text(c * TILE + 4, (r + h) * TILE - 9, "AIR-MAIL", '9');
        } else {
            m::house(cv, (c * TILE + 3, r * TILE - 6, w * TILE - 6, h * TILE + 4), &m::House { roof: m::ramp::TILE, stone: true, windows: 2 }, 87 + i as u32);
        }
    }
}

fn coach(cv: &mut Img) {
    let (c, r, w, _) = countryside::COACH;
    m::block(cv, (c * TILE - 4, r * TILE - 22, w * TILE + 8, 24), m::ramp::TIMBER, true, 90);
    cv.rect(c * TILE + 2, r * TILE - 12, 8, 6, 'n');
    cv.rect(c * TILE + w * TILE - 10, r * TILE - 12, 8, 6, 'n');
    cv.line(c * TILE - 4, r * TILE - 23, (c + w) * TILE + 3, r * TILE - 23, 'O');
    for x in [c * TILE + 2, (c + w) * TILE - 8] {
        cv.ellipse((x + 4) as f32, (r * TILE + 4) as f32, 4.5, 4.5, 'n');
        cv.ellipse((x + 4) as f32, (r * TILE + 4) as f32, 3.0, 3.0, 'P');
        cv.put(x + 4, r * TILE + 4, 'n');
        cv.put(x + 3, r * TILE + 2, 'o');
    }
}

fn ferry(cv: &mut Img, (c, r, w, _): (i32, i32, i32, i32), built: bool) {
    // A jetty on posts.
    m::boards(cv, (c * TILE, r * TILE - 6, w * TILE, TILE + 6), true, 91);
    for x in [c * TILE + 1, (c + w) * TILE - 2] {
        cv.line(x, r * TILE + TILE, x, r * TILE + TILE + 3, 'n');
    }
    if built {
        // The ferry: a lit hull and a dark well, its rope to the bank.
        let keep = |_: i32, _: i32| true;
        let x = (c * TILE + w * TILE / 2) as f32;
        m::lobe(cv, (x, (r * TILE - 5) as f32), ((w * TILE / 2 + 5) as f32, 8.0), m::ramp::TIMBER, 92, 6.0, 0.0, &keep);
        cv.ellipse(x, (r * TILE - 7) as f32, (w * TILE / 2 + 1) as f32, 4.5, 'n');
        cv.line(c * TILE - 8, r * TILE - 14, (c + w) * TILE + 8, r * TILE - 3, 'O');
    }
}

pub(super) fn lights(lights: &mut Vec<Light>, run: &Run) {
    super::bowcountry::lights(lights, run);
    super::town::lights(lights, run);
    let country = &run.home.countryside;
    for (i, lamp) in LAMPS
        .iter()
        .enumerate()
        .filter(|(_, l)| l.room == run.room().kind)
    {
        if country.lamps & (1 << i) != 0 {
            let (x, y) = at(lamp.at.0 * 2.0, lamp.at.1 * 2.0);
            lights.push(fire(x, y - 16, 52.0, 0.4));
        }
    }
    if run.room().kind == RoomKind::Ruins && country.has(1) {
        let (c, r, w, _) = PLOTS[1].footprint;
        lights.push(fire(c * TILE + w * TILE / 2, r * TILE - 4, 68.0, 0.5));
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    super::eastern::figures(cv, run);
    super::northern::figures(cv, run);
    super::coastal::figures(cv, run);
    super::woodland::figures(cv, run);
    super::wetland::figures(cv, run);
    super::aerie::figures(cv, run);
    super::mummers::figures(cv, run);
    super::hippodrome::figures(cv, run);
    super::windcountry::figures(cv, run);
    super::bowcountry::figures(cv, run);
    super::town::figures(cv, run);
    for (i, &(_, stake)) in SURVEY
        .iter()
        .enumerate()
        .filter(|(_, (room, _))| *room == kind)
    {
        let (x, y) = at(stake.0 * 2.0, stake.1 * 2.0);
        let signed = survey_signed(run, i);
        // The larger pennant stays above and right of a signing knight.
        cv.line(x + 10, y - 23, x + 10, y + 3, 'o');
        cv.rect(x + 10, y - 23, 14, 9, if signed { 'g' } else { 'O' });
        if signed {
            // A tick on the pennant.
            cv.line(x + 13, y - 19, x + 15, y - 17, '9');
            cv.line(x + 15, y - 17, x + 20, y - 22, '9');
        } else {
            super::near_prompt(cv, run, (x, y), "SIGN");
        }
    }
    for (i, relic) in RELICS.iter().enumerate().filter(|(_, r)| r.room == kind) {
        if run.home.countryside.relics & (1 << i) != 0 {
            continue;
        }
        let (x, y) = at(relic.at.0 * 2.0, relic.at.1 * 2.0);
        let ink = if (run.tick / 12) % 2 == 0 { 'O' } else { 'h' };
        cv.line(x - 5, y, x + 5, y, ink);
        cv.line(x, y - 5, x, y + 5, ink);
        cv.rect(x - 2, y - 2, 5, 5, 'u');
    }
    for (i, plot) in PLOTS.iter().enumerate().filter(|(_, p)| p.room == kind) {
        let (x, y) = at(plot.at.0 * 2.0, plot.at.1 * 2.0);
        ring(
            cv,
            x,
            y,
            if run.home.countryside.has(i) {
                'g'
            } else {
                'O'
            },
        );
        if !run.home.countryside.has(i) {
            super::near_prompt(cv, run, (x, y), "BUILD");
        }
    }
    if kind == RoomKind::Windmill {
        // A few lines, not a rebuilt sprite or world mesh each frame.
        let (x, y) = (8 * TILE, 3 * TILE + 4);
        let a = (run.tick % (30 * 8)) as f32 / (30 * 8) as f32 * std::f32::consts::TAU;
        for i in 0..4 {
            let a = a + i as f32 * std::f32::consts::FRAC_PI_2;
            cv.line(
                x,
                y,
                x + (a.cos() * 23.0) as i32,
                y + (a.sin() * 23.0) as i32,
                'o',
            );
        }
        cv.rect(x - 2, y - 2, 5, 5, 'O');
    }
}

pub(super) fn boards(cv: &mut Img, run: &Run) {
    super::coastal::boards(cv, run);
    super::woodland::boards(cv, run);
    super::wetland::boards(cv, run);
    super::aerie::boards(cv, run);
    super::mummers::boards(cv, run);
    super::hippodrome::boards(cv, run);
    super::windcountry::boards(cv, run);
    super::bowcountry::boards(cv, run);
    super::town::boards(cv, run);
    let kind = run.room().kind;
    if !countryside::PLACES.contains(&kind)
        && !countryside::WEST_PLACES.contains(&kind)
        && !countryside::EAST_PLACES.contains(&kind)
        && !countryside::NORTH_PLACES.contains(&kind)
        && !crate::rules::coastal::PLACES.contains(&kind)
        && !crate::rules::woodland::PLACES.contains(&kind)
        && !crate::rules::wetland::PLACES.contains(&kind)
        && !crate::rules::aerie::PLACES.contains(&kind)
        && !crate::rules::mummers::PLACES.contains(&kind)
        && !crate::rules::hippodrome::PLACES.contains(&kind)
        && !crate::rules::windcountry::PLACES.contains(&kind)
        && !crate::rules::bowcountry::PLACES.contains(&kind)
    {
        return;
    }
    // Arcade rooms have their own lower-corner ledgers. Adventure clues live
    // beside the artefact, never obscure the course or a start plate.
    let near = |p: (f32, f32)| {
        run.players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - p.0 * 2.0).hypot(h.y - p.1 * 2.0) < 4.0)
    };
    if let Some((i, p)) = PLOTS
        .iter()
        .enumerate()
        .find(|(_, p)| p.room == kind && near(p.at))
    {
        let country = &run.home.countryside;
        let status = if country.has(i) {
            p.says.into()
        } else if let Some(reason) = p.blocked(country) {
            reason.to_uppercase()
        } else {
            "HOLD E ON GOLD PLATE TO BUILD".into()
        };
        let price = countryside::cost(p);
        let board = home::board(&[
            (p.name.to_uppercase(), '9'),
            (
                if country.has(i) {
                    "BUILT BY THE REALM".into()
                } else {
                    format!("PRICE: {}", price.label()).to_uppercase()
                },
                'O',
            ),
            (status, 'h'),
            (
                format!("TREASURY: {}", run.treasury.label()).to_uppercase(),
                'h',
            ),
        ]);
        // layers() stamps the tray later. Keep plot prices and travel text
        // above its full wrapped height, including another knight's focus.
        let tray_h = super::tray::max_height(run, cv.w - 8);
        let bottom = if tray_h == 0 {
            cv.h - 8
        } else {
            cv.h - 12 - tray_h - 4
        };
        cv.stamp(&board, cv.w - board.w - 4, (bottom - board.h).max(0));
    } else if let Some((i, r)) = RELICS
        .iter()
        .enumerate()
        .find(|(_, r)| r.room == kind && near(r.at))
    {
        let board = home::board(&[
            (r.name.to_uppercase(), '9'),
            (
                if run.home.countryside.relics & (1 << i) == 0 {
                    "WALK INTO THE SPARK TO RECOVER IT"
                } else {
                    "RECOVERED FOR WREN'S LOST PROPERTY"
                }
                .into(),
                'O',
            ),
        ]);
        cv.stamp(&board, 4, cv.h - board.h - 4);
    } else if let Some((i, l)) = LAMPS
        .iter()
        .enumerate()
        .find(|(_, l)| l.room == kind && near(l.at))
    {
        let c = &run.home.countryside;
        let status = if c.lamps & (1 << i) != 0 {
            "LIT. THE FROGS APPROVE"
        } else if !c.has(1) {
            "BUILD THE OLD WATCH BEACON FIRST"
        } else if c.relics & 2 == 0 {
            "RECOVER THE BELL AT THE OLD WATCH"
        } else {
            "E BESIDE THE POST: RING THE BELL"
        };
        let board = home::board(&[
            (l.name.to_uppercase(), '9'),
            (status.into(), 'O'),
            (format!("ROAD LANTERNS {}/3", c.lamps.count_ones()), 'h'),
            ("ALL THREE: 75 GOLD + 4 WAX ONCE".into(), 'h'),
        ]);
        cv.stamp(&board, 4, cv.h - board.h - 4);
    } else if run.pastimes.round.is_none()
        && let Some(board) = super::northern::board(run)
    {
        let tray_h = super::tray::max_height(run, cv.w - 8);
        let bottom = if tray_h == 0 {
            cv.h - 8
        } else {
            cv.h - 16 - tray_h
        };
        // Keep the quest panel above the lower clerk and GOOSE lens.
        let y = if matches!(run.room().kind, RoomKind::Rookery | RoomKind::Observatory) {
            13
        } else {
            (bottom - board.h).max(0)
        };
        cv.stamp(&board, 4, y);
    } else if run.pastimes.round.is_none()
        && let Some(board) = super::eastern::board(run)
    {
        cv.stamp(&board, 4, cv.h - board.h - 4);
    } else if run.pastimes.round.is_none()
        && let Some(board) = survey_board(run)
    {
        cv.stamp(&board, 4, cv.h - board.h - 4);
    } else if matches!(kind, RoomKind::Orchard | RoomKind::Marsh) {
        let c = &run.home.countryside;
        let board = home::board(&[
            ("THE LAMPLIGHTER'S WALK".into(), '9'),
            ("FIND THE BELL".into(), 'O'),
            (
                format!("E AT THREE ROAD LANTERNS: {}/3", c.lamps.count_ones()),
                'h',
            ),
            ("THEN BUILD SIR DAMP'S FERRY OFFICE".into(), 'h'),
            (
                if kind == RoomKind::Orchard {
                    "MARSH WEST"
                } else {
                    "ORCHARD EAST"
                }
                .into(),
                'h',
            ),
        ]);
        cv.stamp(&board, 4, cv.h - board.h - 4);
    } else if matches!(
        kind,
        RoomKind::Windmill | RoomKind::Ruins | RoomKind::Meadow
    ) {
        let c = &run.home.countryside;
        let mut rows = vec![
            ("WREN'S GRAND RAMBLE".into(), '9'),
            (
                format!(
                    "PLACES {}/8  RELICS {}/3",
                    c.visited.count_ones(),
                    c.relics.count_ones()
                ),
                'O',
            ),
            ("6 PLACES: 150 GOLD ONCE".into(), 'h'),
            ("3 RELICS: 100 GOLD + 4 ORE ONCE".into(), 'h'),
        ];
        if let Some(r) = RELICS.iter().find(|r| r.room == kind) {
            rows.push((r.clue.into(), 'h'));
        }
        if kind == RoomKind::Ruins {
            rows.push(("BEACON + BELL: LIGHT THREE ROAD LANTERNS".into(), 'h'));
        }
        if kind == RoomKind::Meadow {
            rows.push(("BUILD THE INN".into(), 'h'));
        }
        let board = home::board(&rows);
        cv.stamp(&board, 4, cv.h - board.h - 4);
    }
}

fn survey_signed(run: &Run, i: usize) -> bool {
    run.home.countryside.survey & (1 << i) != 0
        || run.marks.contains_key(&format!("country:survey:{i}"))
}

fn western_scenery(cv: &mut Img, run: &Run) {
    match run.room().kind {
        RoomKind::Downs => {
            // Two long hay ricks, each with its board.
            for c in [4, 16] {
                for k in 0..5 {
                    m::hay(cv, c * TILE + 6 + k * 13, 3 * TILE - 1, 8.5, (c * 7 + k) as u32);
                }
                m::sign(cv, c * TILE + 22, 3 * TILE + 4, if c == 4 { "WHEY" } else { "NAY" }, 'T');
            }
            for (c, r) in [(8, 9), (13, 4), (20, 10)] {
                m::bush(cv, c * TILE, r * TILE, 4.5, m::Leaf::Hedge, (c + r) as u32);
            }
        }
        RoomKind::Cliffs => {
            for c in [1, 15] {
                // The cliff's lip above the sea, railed.
                // The land ends at a lit lip; below it the cliff face drops
                // in shadow to the sea.
                m::block(cv, (c * TILE, 10 * TILE + 2, 8 * TILE, 16), m::ramp::STONE, false, 93 + c as u32);
                for x in c * TILE..(c + 8) * TILE {
                    cv.put(x, 10 * TILE + 2, 'i');
                    cv.put(x, 10 * TILE + 3, 'h');
                    for y in 10 * TILE + 10..11 * TILE + 2 {
                        if let Some(px) = cv.get(x, y) {
                            cv.set(x, y, m::darker(px));
                        }
                    }
                }
                m::fence(cv, c * TILE, (c + 8) * TILE - 2, 10 * TILE + 2);
            }
            m::sign(cv, 16 * TILE, 9 * TILE + 2, "NO DIVING", 'T');
        }
        _ => {}
    }
    if matches!(run.room().kind, RoomKind::Windmill | RoomKind::Cliffs) {
        let (c, r, w, _) = countryside::LIFT;
        m::boards(cv, (c * TILE, r * TILE, w * TILE, TILE), false, 94);
        for x in [c * TILE, (c + w) * TILE - 2] {
            cv.line(x, r * TILE - 14, x, (r + 1) * TILE, 'P');
            cv.line(x + 1, r * TILE - 14, x + 1, (r + 1) * TILE, 'b');
        }
        cv.line(c * TILE, r * TILE - 14, (c + w) * TILE - 1, r * TILE - 14, 'O');
        cv.line(c * TILE, r * TILE - 13, (c + w) * TILE - 1, r * TILE - 13, 'B');
        if run.home.countryside.has(4) {
            super::open_arrow(cv, c * TILE + w * TILE / 2, r * TILE + 3);
        } else {
            super::shut_planks(cv, c * TILE, r * TILE, w * TILE);
        }
    }
}

pub(super) fn survey_near(run: &Run) -> Option<usize> {
    SURVEY.iter().position(|(room, stake)| {
        *room == run.room().kind
            && run.players.values().any(|h| {
                h.hp > 0
                    && !h.stone
                    && h.privy == 0
                    && (h.x - stake.0 * 2.0).hypot(h.y - stake.1 * 2.0) < 4.0
            })
    })
}

fn survey_board(run: &Run) -> Option<Img> {
    let kind = run.room().kind;
    if let Some(i) = survey_near(run) {
        Some(home::board(&[
            (format!("SURVEY {}", countryside::name(kind)), '9'),
            (
                if survey_signed(run, i) {
                    "SIGNED. THE GOAT DOUBTS IT"
                } else {
                    "TAP E BESIDE THIS SURVEY STAKE"
                }
                .into(),
                'O',
            ),
            (
                match kind {
                    RoomKind::Windmill => "DOWNS WEST",
                    RoomKind::Downs => "CLIFFS NORTH",
                    _ => "DOWNS SOUTH",
                }
                .into(),
                'h',
            ),
            ("ALL THREE: 45 GOLD".into(), 'h'),
            ("THEN BUILD THE GOAT LIFT AT CLIFFS".into(), 'h'),
        ]))
    } else if kind == RoomKind::Cliffs {
        Some(home::board(&[
            ("GULLIBLE CLIFFS".into(), '9'),
            ("THREE STAKES".into(), 'O'),
            (
                format!(
                    "SIGNED {}/3",
                    run.home.countryside.survey.count_ones()
                ),
                'h',
            ),
            ("LIFT CONNECTS TO WONKY WINDMILL".into(), 'h'),
            ("SEA BELOW. NOT A DARE".into(), 'h'),
        ]))
    } else {
        None
    }
}
