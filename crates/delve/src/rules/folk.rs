//! The realm's folk at work: who is about in the rooms of the world (and in
//! the forge-halls below), where each is going, and what they carry.
//!
//! Every one of them follows a route: walks through rooms along a path,
//! stays somewhere a while at a task, or is away (down the shaft, off to
//! market) for a time, then round again. Where anyone is, is a pure
//! function of the realm's state (`Home`) and the tick: nothing about them
//! is saved or sent, a friend's mirror shows the same traffic, and a test
//! can ask where everyone is at any moment. Who is about comes from what
//! the realm has done: King Brannoc's camp once he has come, his miners
//! once the Upper Workings are his, his builders while a work goes up, his
//! carters and smiths once a forge burns.

use super::barony::{self, CAMP_GUARDS, CAMP_KING, Court, HALL_GUARDS, SEAT_AT};
use super::home::Home;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Folk {
    King,
    Student,
    Wizard,
    Guard,
    Miner,
    Builder,
    Carter,
    Smith,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Walk,
    Work,
    Idle,
    Seated,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Figure {
    pub folk: Folk,
    pub x: f32,
    pub y: f32,
    pub east: bool,
    pub pose: Pose,
    pub loaded: bool,
    pub t: u32,
}

#[derive(Clone, Copy, Debug)]
enum Step {
    Walk(RoomKind, &'static [(f32, f32)], bool),
    Stay(RoomKind, (f32, f32), u32, Pose, bool),
    Away(u32),
}

const PACE: f32 = 2.0 / HZ as f32;

fn path_len(path: &[(f32, f32)]) -> f32 {
    path.windows(2)
        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
        .sum()
}

fn ticks(step: &Step) -> u32 {
    match *step {
        Step::Walk(_, path, _) => (path_len(path) / PACE).ceil() as u32,
        Step::Stay(_, _, t, _, _) => t,
        Step::Away(t) => t,
    }
}

fn place(folk: Folk, route: &[Step], t: u32) -> Option<(RoomKind, Figure)> {
    let period: u32 = route.iter().map(ticks).sum();
    let mut t = t % period.max(1);
    for step in route {
        let n = ticks(step);
        if t >= n {
            t -= n;
            continue;
        }
        return match *step {
            Step::Away(_) => None,
            Step::Stay(room, (x, y), _, pose, loaded) => Some((
                room,
                Figure {
                    folk,
                    x: x * TILE_UNITS,
                    y: y * TILE_UNITS,
                    east: true,
                    pose,
                    loaded,
                    t,
                },
            )),
            Step::Walk(room, path, loaded) => {
                let mut d = t as f32 * PACE;
                let n = path.len().saturating_sub(1);
                for i in 0..n {
                    let (a, b) = (path[i], path[i + 1]);
                    let len = (b.0 - a.0).hypot(b.1 - a.1);
                    if d <= len || i + 1 == n {
                        let k = if len > 0.0 { (d / len).min(1.0) } else { 0.0 };
                        let east = if (b.0 - a.0).abs() > 0.01 {
                            b.0 > a.0
                        } else {
                            true
                        };
                        return Some((
                            room,
                            Figure {
                                folk,
                                x: (a.0 + (b.0 - a.0) * k) * TILE_UNITS,
                                y: (a.1 + (b.1 - a.1) * k) * TILE_UNITS,
                                east,
                                pose: Pose::Walk,
                                loaded,
                                t,
                            },
                        ));
                    }
                    d -= len;
                }
                None
            }
        };
    }
    None
}

const DOWN_TO_SHAFT: &[(f32, f32)] = &[(9.6, 11.2), (9.6, 7.8), (12.0, 7.8), (12.0, 3.8)];
const UP_FROM_SHAFT: &[(f32, f32)] = &[(12.0, 3.8), (12.0, 7.8), (9.6, 7.8), (9.6, 11.2)];
const CART_OUT_HEAD: &[(f32, f32)] = &[(10.4, 11.8), (12.0, 12.4), (12.0, 13.95)];
const CART_OUT_GATE: &[(f32, f32)] = &[(12.0, 0.05), (12.0, 12.6)];
const CART_IN_GATE: &[(f32, f32)] = &[(12.0, 12.6), (12.0, 0.05)];
const CART_IN_HEAD: &[(f32, f32)] = &[(12.0, 13.95), (12.0, 12.4), (10.4, 11.8)];

const HALL_SPOTS: [(f32, f32); 5] = [(3.0, 7.7), (7.6, 7.7), (4.4, 7.7), (8.6, 7.7), (2.4, 7.7)];
const TO_SITE: [&[(f32, f32)]; 5] = [
    &[(12.0, 3.8), (12.0, 7.7), (3.0, 7.7)],
    &[(12.0, 3.8), (12.0, 7.7), (7.6, 7.7)],
    &[(12.0, 3.8), (12.0, 7.7), (4.4, 7.7)],
    &[(12.0, 3.8), (12.0, 7.7), (8.6, 7.7)],
    &[(12.0, 3.8), (12.0, 7.7), (2.4, 7.7)],
];
const FROM_SITE: [&[(f32, f32)]; 5] = [
    &[(3.0, 7.7), (12.0, 7.7), (12.0, 3.8)],
    &[(7.6, 7.7), (12.0, 7.7), (12.0, 3.8)],
    &[(4.4, 7.7), (12.0, 7.7), (12.0, 3.8)],
    &[(8.6, 7.7), (12.0, 7.7), (12.0, 3.8)],
    &[(2.4, 7.7), (12.0, 7.7), (12.0, 3.8)],
];
const CAMP_TO_SHAFT: &[(f32, f32)] = &[(8.6, 10.4), (12.0, 10.4), (12.0, 3.8)];

pub const SMITH_SPOTS: [(f32, f32); 2] = [(6.4, 9.2), (17.6, 9.2)];
pub const FORGE_BUILDERS: [(f32, f32); 5] = [
    (9.4, 7.6),
    (14.6, 7.6),
    (9.4, 4.4),
    (14.6, 4.4),
    (12.0, 7.8),
];

fn walkers(home: &Home) -> Vec<(Folk, Vec<Step>, u32)> {
    let b = &home.barony;
    let mut out: Vec<(Folk, Vec<Step>, u32)> = Vec::new();
    out.push((
        Folk::Wizard,
        vec![Step::Stay(
            RoomKind::WizardStudy,
            super::college::WIZARD,
            HZ * 60,
            Pose::Idle,
            false,
        )],
        0,
    ));
    for (room, path, reading) in [
        (
            RoomKind::CollegeGrounds,
            &[
                (6.0, 6.0),
                (6.0, 9.0),
                (16.0, 9.0),
                (16.0, 6.0),
                (16.0, 9.0),
                (6.0, 9.0),
                (6.0, 6.0),
            ][..],
            (6.0, 6.0),
        ),
        (
            RoomKind::CollegeHall,
            &[(6.0, 6.5), (6.0, 7.0), (17.5, 7.0), (17.5, 6.5), (6.0, 6.5)][..],
            (6.0, 6.5),
        ),
        (
            RoomKind::CollegeLibrary,
            &[(8.5, 5.5), (8.5, 8.5), (15.5, 8.5), (15.5, 5.5), (8.5, 5.5)][..],
            (8.5, 5.5),
        ),
    ] {
        for i in 0..3 {
            out.push((
                Folk::Student,
                vec![
                    Step::Walk(room, path, false),
                    Step::Stay(room, reading, HZ * 5, Pose::Idle, false),
                ],
                i * HZ * 4,
            ));
        }
    }
    if !b.here() {
        return out;
    }
    let sworn = b.court == Court::Sworn;
    out.push((
        Folk::King,
        vec![if sworn {
            Step::Stay(RoomKind::KingsHall, SEAT_AT, HZ * 60, Pose::Seated, false)
        } else {
            Step::Stay(RoomKind::MineHead, CAMP_KING, HZ * 60, Pose::Idle, false)
        }],
        0,
    ));
    let posts = if sworn { HALL_GUARDS } else { CAMP_GUARDS };
    let room = if sworn {
        RoomKind::KingsHall
    } else {
        RoomKind::MineHead
    };
    for (i, &at) in posts.iter().enumerate() {
        out.push((
            Folk::Guard,
            vec![Step::Stay(room, at, HZ * 60, Pose::Idle, false)],
            i as u32 * 17,
        ));
    }
    out.push((
        Folk::Guard,
        vec![Step::Stay(
            RoomKind::Gate,
            (17.0, 6.4),
            HZ * 60,
            Pose::Idle,
            false,
        )],
        5,
    ));
    if b.holds_workings() {
        out.push((
            Folk::Miner,
            vec![Step::Stay(
                RoomKind::Gate,
                (19.8, 5.4),
                HZ * 60,
                Pose::Idle,
                true,
            )],
            9,
        ));
    }
    if sworn {
        out.push((
            Folk::Guard,
            vec![Step::Stay(
                RoomKind::MineHead,
                (8.6, 8.2),
                HZ * 60,
                Pose::Idle,
                false,
            )],
            3,
        ));
    }
    let miners = b.miners();
    for i in 0..miners {
        let route = if home.goods.mine_full() {
            vec![Step::Stay(
                RoomKind::MineHead,
                (6.4 + (i % 5) as f32 * 0.8, 11.9 + (i / 5) as f32 * 0.45),
                HZ * 60,
                Pose::Idle,
                false,
            )]
        } else {
            vec![
                Step::Walk(RoomKind::MineHead, DOWN_TO_SHAFT, false),
                Step::Away(HZ * 14),
                Step::Walk(RoomKind::MineHead, UP_FROM_SHAFT, true),
                Step::Stay(RoomKind::MineHead, (9.6, 11.2), HZ * 2, Pose::Work, true),
            ]
        };
        let period: u32 = route.iter().map(ticks).sum();
        out.push((Folk::Miner, route, i * period / miners.max(1)));
    }
    if let Some(def) = b.building() {
        let crew = barony::crew(b).min(5);
        for i in 0..crew as usize {
            let route = match def.site {
                barony::Site::KingsHall => vec![
                    Step::Walk(RoomKind::MineHead, TO_SITE[i], true),
                    Step::Stay(RoomKind::MineHead, HALL_SPOTS[i], HZ * 6, Pose::Work, false),
                    Step::Walk(RoomKind::MineHead, FROM_SITE[i], false),
                    Step::Away(HZ * 8),
                ],
                barony::Site::Forge(_) => vec![
                    Step::Walk(RoomKind::MineHead, CAMP_TO_SHAFT, true),
                    Step::Away(HZ * 40),
                ],
            };
            let period: u32 = route.iter().map(ticks).sum();
            out.push((Folk::Builder, route, i as u32 * period / crew.max(1)));
        }
    }
    if !b.lit.is_empty() {
        out.push((
            Folk::Carter,
            vec![
                Step::Stay(RoomKind::MineHead, (10.4, 11.8), HZ * 3, Pose::Work, true),
                Step::Walk(RoomKind::MineHead, CART_OUT_HEAD, true),
                Step::Walk(RoomKind::Gate, CART_OUT_GATE, true),
                Step::Away(HZ * 16),
                Step::Walk(RoomKind::Gate, CART_IN_GATE, false),
                Step::Walk(RoomKind::MineHead, CART_IN_HEAD, false),
            ],
            0,
        ));
    }
    out
}

pub fn figures(home: &Home, room: RoomKind, pack: Pack, tick: u64) -> Vec<Figure> {
    let mut out = Vec::new();
    if room == RoomKind::Forge {
        let b = &home.barony;
        let Some(f) = barony::forge_of(pack) else {
            return out;
        };
        let work = f.work.and_then(barony::work);
        if b.is_lit(f.id) {
            let pose = if matches!(home.goods.burn(b, f.id), super::goods::Burn::Working) {
                Pose::Work
            } else {
                Pose::Idle
            };
            for (i, &(x, y)) in SMITH_SPOTS.iter().enumerate() {
                out.push(Figure {
                    folk: Folk::Smith,
                    x: x * TILE_UNITS,
                    y: y * TILE_UNITS,
                    east: i == 0,
                    pose,
                    loaded: false,
                    t: (tick as u32).wrapping_add(i as u32 * 7),
                });
            }
        } else if work.is_some_and(|w| b.building().is_some_and(|d| d.id == w.id)) {
            let crew = barony::crew(b).min(5) as usize;
            for (i, &(x, y)) in FORGE_BUILDERS.iter().take(crew).enumerate() {
                out.push(Figure {
                    folk: Folk::Builder,
                    x: x * TILE_UNITS,
                    y: y * TILE_UNITS,
                    east: x < 12.0,
                    pose: Pose::Work,
                    loaded: false,
                    t: (tick as u32).wrapping_add(i as u32 * 11),
                });
            }
        }
        return out;
    }
    for (folk, route, offset) in walkers(home) {
        let t = (tick % (1 << 31)) as u32 + offset;
        if let Some((r, fig)) = place(folk, &route, t)
            && r == room
        {
            out.push(fig);
        }
    }
    out.sort_by(|a, b| a.y.total_cmp(&b.y));
    out
}

#[cfg(test)]
#[path = "../../tests/rules/together_folk__tests.rs"]
mod tests;
