//! Dame Fortune's hall, drawn: her great wheel against the north wall with
//! red drapes either side, the audience on its benches, and Fortune herself
//! beside the wheel, blindfold on, crown bright. The wheel is live state —
//! it spins, and the wedge it stops on is the delve's mode — so its pointer
//! and the winning wedge glow; the rest is the realm's material inks.

use std::f32::consts::{FRAC_PI_2, TAU};

use super::super::ink::{Img, hash, text_width};
use super::super::kit::{self, RockKind, Tiles};
use super::{TILE, at, dusk, fire, flags, home, stand};
use crate::drive::together_shooter::feats::Tier;
use crate::drive::together_shooter::fortune::{
    self, BENCHES, COFFER, COFFER_PLATE, FORTUNE_AT, FORTUNE_PLATE, LEVER, Mode, SPIN_TICKS,
    WHEEL_STAND,
};
use crate::drive::together_shooter::home::Station;
use crate::drive::together_shooter::{HZ, Run, Tile};
use crate::stage::world_viz::overworld::light::Light;

const RADIUS: f32 = 40.0;

/// The wheel's hub, in native pixels.
fn hub() -> (i32, i32) {
    let (c, r, w, _) = WHEEL_STAND;
    ((c * 2 + w) * TILE / 2, r * TILE + 42)
}

/// The hall at rest: flags, a medallion before the wheel, the walls and
/// their braziers, the drapes, the wheel's stand, the benches, the plates.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let tick = run.tick as u32;
    let tiles = Tiles::get();
    let (pw, ph) = (room.cols as i32 * TILE, room.rows as i32 * TILE);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    for y in 0..ph {
        for x in 0..pw {
            if matches!(room.tile(x / TILE, y / TILE), Tile::Floor | Tile::Door)
                && let Some(ch) = flags(x, y)
            {
                cv.put(x, y, ch);
            }
        }
    }
    medallion(&mut cv);
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            if room.tile(col, row) != Tile::Wall {
                continue;
            }
            let (x, y) = (col * TILE, row * TILE);
            let corner = (col == 0 || col == room.cols as i32 - 1)
                && (row == 0 || row == room.rows as i32 - 1);
            if corner {
                let im = kit::brazier(tick / 4, (col * 3 + row) as u32);
                cv.stamp(&im, x + (TILE - im.w) / 2, y + TILE - im.h);
                lights.push(fire(x + TILE / 2, y + 4, 56.0, 0.32));
            } else {
                cv.stamp(tiles.rock(RockKind::Slate, hash(col, row, 9)), x, y);
            }
        }
    }
    // The doorway home, with its two torches.
    for (k, ty) in [(0, 6 * TILE - 6), (1, 8 * TILE + 2)] {
        let im = kit::torch(tick / 4, k);
        cv.stamp(&im, 4, ty);
        lights.push(fire(6, ty + 1, 36.0, 0.3));
    }
    drapes(&mut cv);
    stand_frame(&mut cv);
    for bench in BENCHES {
        let (c, r, w, h) = bench;
        let (x, y, bw, bh) = (c * TILE, r * TILE, w * TILE, h * TILE);
        for k in 0..2 {
            let by = y + 6 + k * 15;
            cv.rect(x + 2, by, bw - 4, 4, 'P');
            cv.line(x + 2, by, x + bw - 3, by, 'O');
            cv.line(x + 2, by + 4, x + bw - 3, by + 4, 'b');
            for lx in [x + 4, x + bw - 6] {
                cv.line(lx, by + 5, lx, by + 7, 'b');
            }
        }
        let _ = bh;
    }
    for (plate, station) in [
        (LEVER, None),
        (FORTUNE_PLATE, Some(Station::Wheel)),
        (COFFER_PLATE, Some(Station::Coffer)),
    ] {
        slab(&mut cv, plate, false, station);
    }
    pedestal(&mut cv);
    let (cx, cy) = coffer_top();
    lights.push(fire(cx, cy - 6, 34.0, 0.25));
    lights.push(fire(hub().0, hub().1, 70.0, 0.4));
    dusk(&mut cv, &lights, ('x', 'I'));
    cv
}

/// A round medallion inlaid in the floor before the wheel.
fn medallion(cv: &mut Img) {
    let (cx, cy) = (hub().0, 9 * TILE + 4);
    for y in cy - 22..cy + 22 {
        for x in cx - 34..cx + 34 {
            let (dx, dy) = ((x - cx) as f32 / 34.0, (y - cy) as f32 / 22.0);
            let r = dx.hypot(dy);
            if r > 1.0 {
                continue;
            }
            let a = dy.atan2(dx);
            let ink = if r > 0.93 {
                'o'
            } else if r > 0.86 {
                'B'
            } else if ((a / TAU * 16.0).fract() < 0.12) && r > 0.3 {
                'p'
            } else if r < 0.18 {
                'O'
            } else {
                continue;
            };
            cv.put(x, y, ink);
        }
    }
}

/// Red drapes hung either side of the wheel, worked in timber reds.
fn drapes(cv: &mut Img) {
    let (hx, _) = hub();
    for (x0, dir) in [(hx - 64, 1), (hx + 52, -1)] {
        for y in 1..92 {
            let sway = ((y as f32 / 9.0).sin() * 1.5) as i32;
            for k in 0..12 {
                let x = x0 + k + sway * dir;
                let fold = (k + y / 14) % 4;
                let ink = match fold {
                    0 => 'b',
                    1 => 'B',
                    2 => 'p',
                    _ => 'B',
                };
                if y > 84 && (k * 7 + y) % 3 == 0 {
                    continue;
                }
                cv.put(x, y, ink);
            }
        }
        // The tie-back.
        cv.line(x0 - 1, 60, x0 + 12, 60, 'O');
    }
    // The valance across the top.
    for x in hx - 66..hx + 66 {
        let scallop = ((x - hx).rem_euclid(12) - 6).abs() / 2;
        for y in 1..6 - scallop {
            cv.put(x, y, if y == 1 { 'O' } else { 'p' });
        }
    }
}

/// The timber A-frame the wheel turns on.
fn stand_frame(cv: &mut Img) {
    let (hx, hy) = hub();
    for (dx, ink) in [(-1, 'b'), (0, 'P'), (1, 'b')] {
        cv.line(hx + dx - 2, hy, hx - 30 + dx, hy + 50, ink);
        cv.line(hx + dx + 2, hy, hx + 30 + dx, hy + 50, ink);
    }
    cv.rect(hx - 36, hy + 50, 72, 4, 'I');
    cv.line(hx - 36, hy + 50, hx + 35, hy + 50, 'P');
}

/// A plate on the hall's floor: the lever's (a spin) or Fortune's own.
fn slab(cv: &mut Img, (c, r, w, h): (i32, i32, i32, i32), lit: bool, station: Option<Station>) {
    let (x, y, pw, ph) = (c * TILE + 1, r * TILE + 1, w * TILE - 2, h * TILE - 2);
    cv.rect(x, y, pw, ph, 'X');
    cv.frame(x, y, pw, ph, if lit { '5' } else { 'o' });
    let ink = if lit { '6' } else { 'O' };
    let mark = match station {
        Some(Station::Coffer) => Img::from_rows(&["OOOOO", "O.O.O", "OOOOO", "O...O", "OOOOO"]),
        Some(_) => Img::from_rows(&[".OOO.", "O.O.O", "OOOOO", "O.O.O", ".OOO."]),
        None => Img::from_rows(&["..O..", ".OOO.", "OOOOO", "..O..", "..O.."]),
    }
    .recolor(&[('O', ink)]);
    cv.stamp(&mark, x + (pw - mark.w) / 2, y + (ph - mark.h) / 2);
}

/// The wheel's turn now, radians clockwise: still where it last stopped.
fn turn(run: &Run) -> f32 {
    run.spin.map_or(0.0, |spin| spin.angle(run.tick))
}

/// The wedges as Fortune's ladder has opened them.
fn opened(run: &Run) -> Vec<u8> {
    fortune::open_wedges(run.home.level(Station::Wheel))
}

/// A wedge's colour: muted material inks, neighbours apart.
fn wedge_ink(mode: Mode) -> (char, char) {
    match mode {
        Mode::LongWayDown => ('u', 'U'),
        Mode::LightsOut => ('X', 'g'),
        Mode::Collapse => ('r', 'R'),
        Mode::GiantsFeast => ('N', 'L'),
        Mode::HoldTheStair => ('S', 'u'),
        Mode::MimicFair => ('P', 'o'),
        Mode::GlassJaw => ('B', 'p'),
        Mode::Gauntlet => ('I', 'P'),
        Mode::Turbo => ('q', 'Q'),
        Mode::RuneRush => ('e', 'C'),
        Mode::AllRandom => ('r', 'o'),
        Mode::Ironman => ('g', 'J'),
        Mode::HollowWalls => ('P', 'o'),
        Mode::Sponsors => ('a', '4'),
    }
}

/// A wedge's mark, at its middle.
fn icon(mode: Mode) -> Img {
    Img::from_rows(match mode {
        Mode::LongWayDown => &[
            ".......", ".HHHHH.", ".....H.", ".HHH.H.", ".H...H.", ".H.HHH.", ".......",
        ],
        Mode::LightsOut => &[
            "..G....", "...G...", "..jc...", "..c9c..", "..c9c..", "..c9c..", ".$$$$$.",
        ],
        Mode::Collapse => &[
            "..hh...", ".hJJh..", ".JJJJ..", "..hh...", "..7....", ".7.7...", "7...7..",
        ],
        Mode::GiantsFeast => &[
            ".HHHHH.", "HHHHHHH", "HKHHHKH", "HHHHHHH", ".HKHKH.", ".HHHHH.", ".......",
        ],
        Mode::HoldTheStair => &[
            "H.H.H.H", "HHHHHHH", ".HHHHH.", ".HH.HH.", ".HH.HH.", ".HHHHH.", ".......",
        ],
        Mode::MimicFair => &[
            ".ooooo.", "oOOOOOo", "oHoHoHo", "o.....o", "oHoHoHo", "oOOOOOo", ".ooooo.",
        ],
        Mode::GlassJaw => &[
            ".77.77.", "7777777", "7777.77", "777.777", ".77.77.", "..777..", "...7...",
        ],
        Mode::Gauntlet => &[
            "5.5.5.5", "5555555", "5.555.5", "5555555", ".55555.", ".......", ".......",
        ],
        Mode::Turbo => &[
            "H..H...", ".H..H..", "..H..H.", "...H..H", "..H..H.", ".H..H..", "H..H...",
        ],
        Mode::RuneRush => &[
            "..222..", ".23332.", "2332332", "2222222", "2332332", ".23332.", "..222..",
        ],
        Mode::AllRandom => &[
            ".HHHHH.", "HKHHHKH", "HHHHHHH", "HHHKHHH", "HHHHHHH", "HKHHHKH", ".HHHHH.",
        ],
        Mode::Ironman => &[
            ".HHHHH.", "HHHHHHH", "HKKKKKH", "HHHKHHH", "HHHKHHH", "HHHHHHH", ".HHHHH.",
        ],
        Mode::HollowWalls => &[
            "HHH.HHH", "H.J.H.H", "HHJHHHH", "H.HJ..H", "HHH.JHH", "H.H.H.H", "HHHHHHH",
        ],
        Mode::Sponsors => &[
            "..666..", ".6...6.", "6.....6", ".6.5.6.", "..555..", "..5H5..", "..555..",
        ],
    })
}

/// The live hall: the wheel turning, Fortune, the audience, and the plates
/// knights stand on.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    wheel(cv, run);
    let spinning = run.spin.is_some_and(|s| !s.done(run.tick));
    let landed_late = run.spin.is_some_and(|s| {
        s.done(run.tick) && run.tick.saturating_sub(s.started) < u64::from(SPIN_TICKS + 4 * HZ)
    });
    // Fortune raises her hand while the wheel turns.
    let (fx, fy) = at(FORTUNE_AT.0 * 2.0, FORTUNE_AT.1 * 2.0);
    let fortune = if spinning || (landed_late && (tick / 8).is_multiple_of(2)) {
        Img::from_rows(&FORTUNE_UP)
    } else {
        Img::from_rows(&FORTUNE)
    };
    stand(cv, &fortune, fx, fy);
    // The audience: on their feet and cheering while it turns and lands.
    let tunics = ['p', 'E', 'u', 'R', 'B', 'q', 'r', 'e'];
    for (b, bench) in BENCHES.iter().enumerate() {
        let (c, r, w, _) = *bench;
        for row in 0..2 {
            for seat in 0..6 {
                let id = (b * 12 + row * 6 + seat) as i32;
                let x = c * TILE + 6 + seat as i32 * (w * TILE - 12) / 6;
                let y = r * TILE + 6 + row as i32 * 15;
                let excited = spinning || landed_late;
                let cheer = excited && (tick / 5 + hash(id, 7, 3)).is_multiple_of(2);
                let fidget = !excited && hash(id, (tick / 45) as i32, 5).is_multiple_of(9);
                let im = if cheer { &FAN_CHEER } else { &FAN };
                let tunic = tunics[(hash(id, 1, 77) % tunics.len() as u32) as usize];
                let face = if hash(id, 2, 78).is_multiple_of(3) {
                    'o'
                } else {
                    'O'
                };
                let hair = ['h', 'b', 'n', 'r', 'T'][(hash(id, 3, 79) % 5) as usize];
                let fan = Img::from_rows(im).recolor(&[('p', tunic), ('O', face), ('h', hair)]);
                let lift = i32::from(cheer) * 2 + i32::from(fidget);
                cv.stamp(&fan, x, y - fan.h + 3 - lift);
            }
        }
    }
    // The lever beside its plate: upright, or thrown while the wheel turns.
    let (lc, lr, lw, _) = LEVER;
    let (lx, ly) = ((lc + lw) * TILE + 3, lr * TILE + 14);
    let thrown = spinning;
    let (tx, ty) = if thrown {
        (lx + 7, ly - 8)
    } else {
        (lx, ly - 12)
    };
    cv.line(lx, ly, tx, ty, 'P');
    cv.line(lx + 1, ly, tx + 1, ty, 'b');
    cv.rect(tx - 1, ty - 2, 3, 3, '7');
    cv.rect(lx - 2, ly, 6, 2, 'G');
    let mut reading_coffer = false;
    for hero in run.players.values().filter(|h| h.hp > 0) {
        let on = |p: (i32, i32, i32, i32)| {
            let (c, r) = (hero.x / 2.0, hero.y / 2.0);
            c >= p.0 as f32 && c < (p.0 + p.2) as f32 && r >= p.1 as f32 && r < (p.1 + p.3) as f32
        };
        if on(LEVER) {
            slab(cv, LEVER, true, None);
        }
        if on(FORTUNE_PLATE) {
            slab(cv, FORTUNE_PLATE, true, Some(Station::Wheel));
        }
        if on(COFFER_PLATE) {
            slab(cv, COFFER_PLATE, true, Some(Station::Coffer));
            reading_coffer = true;
        }
    }
    coffer(cv, run, reading_coffer);
}

/// The wheel: wedges round a hub, pegs on the rim, each opened wedge's
/// mark, the pointer at the top, and the winning wedge glowing once it
/// stops.
fn wheel(cv: &mut Img, run: &Run) {
    let (hx, hy) = hub();
    let rot = turn(run);
    let open = opened(run);
    let n = Mode::WHEEL.len() as f32;
    let w = TAU / n;
    let landed = run.spin.filter(|s| s.done(run.tick)).map(|s| s.wedge);
    let blink = (run.tick / 6).is_multiple_of(2);
    for y in hy - 42..hy + 42 {
        for x in hx - 42..hx + 42 {
            let (dx, dy) = (x as f32 + 0.5 - hx as f32, y as f32 + 0.5 - hy as f32);
            let r = dx.hypot(dy);
            if r > RADIUS {
                continue;
            }
            // Clockwise from the top, in the wheel's own turn.
            let a = (dy.atan2(dx) + FRAC_PI_2 - rot).rem_euclid(TAU);
            let index = ((a / w) as usize).min(Mode::WHEEL.len() - 1);
            let mode = Mode::WHEEL[index];
            let edge = (a / w).fract() < 0.04 || (a / w).fract() > 0.96;
            let ink = if r > RADIUS - 2.5 {
                if edge { 'H' } else { 'O' }
            } else if r > RADIUS - 4.0 {
                'b'
            } else if r < 5.0 {
                if r < 2.5 { '6' } else { '4' }
            } else if r < 7.0 || edge {
                'b'
            } else if !open.contains(&(index as u8)) {
                if (x + y) % 2 == 0 { 'K' } else { 'k' }
            } else if landed == Some(index as u8) && r > RADIUS - 7.0 && blink {
                '6'
            } else {
                let (dark, light) = wedge_ink(mode);
                if r < RADIUS * 0.55 { dark } else { light }
            };
            cv.put(x, y, ink);
        }
    }
    // Each wedge's mark (or a question for a closed one), upright.
    for (i, mode) in Mode::WHEEL.iter().enumerate() {
        let mid = (i as f32 + 0.5) * w + rot - FRAC_PI_2;
        let (mx, my) = (
            hx + (mid.cos() * RADIUS * 0.62).round() as i32,
            hy + (mid.sin() * RADIUS * 0.62).round() as i32,
        );
        if open.contains(&(i as u8)) {
            let im = icon(*mode);
            cv.stamp(&im, mx - im.w / 2, my - im.h / 2);
        } else {
            cv.text(mx - 2, my - 3, "?", 'G');
        }
    }
    // The pointer, gold, over the top of the rim.
    for k in 0..6 {
        cv.line(
            hx - (5 - k),
            hy - 44 + k,
            hx + (5 - k),
            hy - 44 + k,
            if k < 2 { '6' } else { '5' },
        );
    }
    cv.put(hx, hy - 38, '4');
}

/// Over everything: Fortune's ledger, the lever's word, and the wheel's
/// verdict.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let on = |p: (i32, i32, i32, i32)| {
        run.players
            .iter()
            .filter(|(_, h)| h.hp > 0)
            .find_map(|(&id, h)| {
                let (c, r) = (h.x / 2.0, h.y / 2.0);
                (c >= p.0 as f32
                    && c < (p.0 + p.2) as f32
                    && r >= p.1 as f32
                    && r < (p.1 + p.3) as f32)
                    .then_some(id)
            })
    };
    let (hx, hy) = hub();
    if let Some((at, tier, label)) = &run.unboxed
        && run.tick.saturating_sub(*at) < 4 * u64::from(HZ)
    {
        let board = home::board(&[
            (tier.name().to_uppercase(), '6'),
            (label.to_uppercase(), 'h'),
        ]);
        let (cx, cy) = coffer_top();
        cv.stamp(&board, (cx - 8).max(2), cy - 46);
    }
    if let Some(spin) = run.spin.filter(|s| s.done(run.tick)) {
        // The verdict, for a few seconds after it lands; then a plaque.
        let mode = Mode::WHEEL[usize::from(spin.wedge)];
        let fresh = run.tick.saturating_sub(spin.started) < u64::from(SPIN_TICKS + 4 * HZ);
        let lines = if fresh {
            vec![
                (mode.name().to_uppercase(), '6'),
                (mode.says().to_uppercase(), 'h'),
            ]
        } else {
            vec![(format!("THIS DELVE: {}", mode.name().to_uppercase()), '9')]
        };
        let board = home::board(&lines);
        let y = if fresh { hy - board.h / 2 } else { hy + 47 };
        cv.stamp(&board, hx - board.w / 2, y);
    }
    if on(COFFER_PLATE).is_some() {
        let n = run.home.boxes.len();
        let lines = if n == 0 {
            vec![
                ("THE HERALD'S COFFER".to_string(), '9'),
                ("EMPTY: ACHIEVEMENTS FILL IT".to_string(), 'h'),
            ]
        } else {
            vec![
                ("THE HERALD'S COFFER".to_string(), '9'),
                (
                    format!(
                        "{n} BOX{} TO OPEN, NEXT: {}",
                        if n == 1 { "" } else { "ES" },
                        run.home.boxes[0].name().to_uppercase()
                    ),
                    'h',
                ),
                ("HOLD F TO OPEN".to_string(), 'O'),
            ]
        };
        let board = home::board(&lines);
        let (c, r, w, h) = COFFER_PLATE;
        cv.stamp(
            &board,
            (c * TILE + w * TILE / 2 - board.w / 2).max(2),
            (r + h) * TILE + 4,
        );
    }
    if let Some(id) = on(FORTUNE_PLATE) {
        let buying = run.players.get(&id).map_or(0, |h| h.buying);
        let board = home::ledger_board(run, Station::Wheel, buying);
        let (c, r, w, _) = FORTUNE_PLATE;
        let x = (c * TILE + w * TILE / 2 - board.w / 2).clamp(2, cv.w - board.w - 2);
        cv.stamp(&board, x, r * TILE - board.h - 4);
    } else if let Some(id) = on(LEVER)
        && run.spin.is_none()
    {
        let buying = run.players.get(&id).map_or(0, |h| h.buying);
        let mut board = home::board(&[
            ("FORTUNE'S WHEEL".to_string(), '9'),
            ("ONE SPIN A DELVE".to_string(), 'h'),
            (String::new(), 'h'),
        ]);
        let label = "HOLD F";
        let bar_w = 40;
        let start = (board.w - (text_width(label) + 4 + bar_w)) / 2;
        let after = board.text(start, 21, label, 'O');
        board.frame(after + 4, 22, bar_w, 5, 'b');
        let fill = (buying.min(crate::drive::together_shooter::home::BUY_HOLD) * (bar_w as u32 - 2)
            / crate::drive::together_shooter::home::BUY_HOLD) as i32;
        if fill > 0 {
            board.rect(after + 5, 23, fill, 3, '5');
        }
        let (c, r, w, _) = LEVER;
        cv.stamp(&board, c * TILE + w * TILE / 2 - board.w / 2, r * TILE + 22);
    }
}

/// Where the top box on the coffer's pedestal sits.
fn coffer_top() -> (i32, i32) {
    let (c, r, w, _) = COFFER;
    ((c * 2 + w) * TILE / 2, r * TILE + 6)
}

/// The Herald's coffer: a stone pedestal, the Herald's mark cut in it.
fn pedestal(cv: &mut Img) {
    let (c, r, w, h) = COFFER;
    let (x, y, pw, ph) = (c * TILE + 4, r * TILE + 8, w * TILE - 8, h * TILE - 8);
    cv.rect(x, y, pw, ph, 'U');
    cv.frame(x, y, pw, ph, 'u');
    cv.line(x + 1, y, x + pw - 2, y, 'V');
    cv.rect(x - 2, y + ph - 3, pw + 4, 3, 'S');
    // The Herald's mark: a trumpet.
    let (mx, my) = (x + pw / 2, y + ph / 2);
    cv.line(mx - 5, my, mx + 3, my, 'x');
    cv.line(mx + 3, my - 2, mx + 3, my + 2, 'x');
    cv.put(mx + 4, my - 3, 'x');
    cv.put(mx + 4, my + 3, 'x');
}

/// A loot box of a grade: timber, steel, gold, or something that glows.
fn loot_box(tier: Tier) -> Img {
    let (body, band, lid) = match tier {
        Tier::Bronze => ('r', 'o', 'R'),
        Tier::Silver => ('u', 'V', 'v'),
        Tier::Gold => ('4', '6', '5'),
        Tier::Legendary => ('1', '3', '2'),
    };
    let mut im = Img::new(12, 9);
    im.rect(0, 3, 12, 6, body);
    im.rect(0, 0, 12, 3, lid);
    im.line(0, 3, 11, 3, band);
    im.line(5, 0, 5, 8, band);
    im.line(6, 0, 6, 8, band);
    im.frame(0, 0, 12, 9, 'K');
    im.put(5, 4, 'w');
    im
}

/// The boxes waiting on the coffer, the top one lifting while a knight
/// reads it, and a box just opened throwing out what it held.
fn coffer(cv: &mut Img, run: &Run, reading: bool) {
    let tick = run.tick as u32;
    let (cx, cy) = coffer_top();
    for (i, tier) in run.home.boxes.iter().take(5).enumerate().rev() {
        let im = loot_box(*tier);
        let lift = if i == 0 && reading {
            ((tick / 4) % 3) as i32
        } else {
            0
        };
        cv.stamp(
            &im,
            cx - im.w / 2 + (i as i32 % 2) * 3,
            cy - i as i32 * 5 - lift,
        );
        if i == 0 && (*tier == Tier::Legendary || reading) && (tick / 5).is_multiple_of(2) {
            cv.put(cx - 7, cy - 2, '6');
            cv.put(cx + 7, cy + 1, '6');
        }
    }
    let Some((at, tier, _)) = &run.unboxed else {
        return;
    };
    let age = run.tick.saturating_sub(*at) as f32;
    if age > 75.0 {
        return;
    }
    // The lid flies, and out it comes: coins and gems in arcs.
    let lid = loot_box(*tier);
    let up = (age * 1.4).min(30.0) as i32;
    cv.stamp(&lid, cx - lid.w / 2 + up / 3, cy - 10 - up);
    for k in 0..14 {
        let a = -std::f32::consts::FRAC_PI_2 + (k as f32 - 6.5) * 0.18;
        let speed = 1.2 + (k % 3) as f32 * 0.35;
        let x = cx as f32 + a.cos() * speed * age;
        let y = cy as f32 - 6.0 + a.sin() * speed * age + 0.045 * age * age;
        let ink = match (k % 4, tier) {
            (0, Tier::Legendary) => 'w',
            (0, _) | (2, _) => '5',
            (1, _) => '6',
            _ => '3',
        };
        cv.put(x as i32, y as i32, ink);
        cv.put(x as i32 + 1, y as i32, ink);
    }
}

const FORTUNE: [&str; 17] = [
    "...6.5.6.5....",
    "...565656.....",
    "..R6555555R...",
    ".RtOOOOOOtR...",
    ".RVVVVVVVVR...",
    ".ROOOOOOOOR...",
    ".R.OOrROO.R...",
    ".R..OOOO..R...",
    "..VVVWxxss....",
    ".VVWWWxsssS...",
    ".OVWWWxsss.O..",
    ".OVWWVxsSs.O..",
    "..VWWVxsSs....",
    "..VWWVxsSs....",
    ".VWWWVxsSss...",
    ".VVWVVxsSss...",
    "..KK....KK....",
];

const FORTUNE_UP: [&str; 17] = [
    "...6.5.6.5....",
    "...565656..O..",
    "..R6555555RO..",
    ".RtOOOOOOtRs..",
    ".RVVVVVVVVRs..",
    ".ROOOOOOOORs..",
    ".R.OOrROOss...",
    ".R..OOOOs.R...",
    "..VVVWxxss....",
    ".VVWWWxsss....",
    ".OVWWWxsss....",
    ".OVWWVxsSs....",
    "..VWWVxsSs....",
    "..VWWVxsSs....",
    ".VWWWVxsSss...",
    ".VVWVVxsSss...",
    "..KK....KK....",
];

const FAN: [&str; 6] = ["..hh..", ".hOOh.", ".OOOO.", "..OO..", ".pppp.", "pppppp"];

const FAN_CHEER: [&str; 6] = ["O....O", "OhhhhO", ".hOOh.", ".OOOO.", "..OO..", ".pppp."];
