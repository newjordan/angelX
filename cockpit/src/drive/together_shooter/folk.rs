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

/// Who someone is, for their figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Folk {
    King,
    Guard,
    /// A miner on his way down, his pick on his shoulder; on his way up,
    /// a sack of ore on his back.
    Miner,
    /// A builder: a dressed stone on his shoulder going, empty coming back.
    Builder,
    /// A carter with the realm's share of the ore, out to market.
    Carter,
    Smith,
}

/// What a figure is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pose {
    Walk,
    /// At a task: hammering, setting a stone, dropping a sack.
    Work,
    Idle,
    Seated,
}

/// One of the folk, where they are this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Figure {
    pub(crate) folk: Folk,
    /// Arena units.
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) east: bool,
    pub(crate) pose: Pose,
    /// Carrying: a sack, a stone, a full cart.
    pub(crate) loaded: bool,
    /// Ticks into what they are doing, for their stride or their swing.
    pub(crate) t: u32,
}

/// A leg of a route.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// Walk the path (tiles) through `room`, carrying or not.
    Walk(RoomKind, &'static [(f32, f32)], bool),
    /// Stay at a spot (tiles) for so many ticks.
    Stay(RoomKind, (f32, f32), u32, Pose, bool),
    /// Out of sight for so many ticks: below, or away to market.
    Away(u32),
}

/// How fast the folk walk: two tiles a second.
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

/// Where a route's walker is `t` ticks into it, if they are in sight.
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

// The paths (tiles). The shaft's mouth is at the adit, the stockpile in the
// yard by the King's camp, the road south through the mine-head's door and
// on through the gate courtyard to the realm.
const DOWN_TO_SHAFT: &[(f32, f32)] = &[(9.6, 11.2), (9.6, 7.8), (12.0, 7.8), (12.0, 3.8)];
const UP_FROM_SHAFT: &[(f32, f32)] = &[(12.0, 3.8), (12.0, 7.8), (9.6, 7.8), (9.6, 11.2)];
const CART_OUT_HEAD: &[(f32, f32)] = &[(10.4, 11.8), (12.0, 12.4), (12.0, 13.95)];
const CART_OUT_GATE: &[(f32, f32)] = &[(12.0, 0.05), (12.0, 12.6)];
const CART_IN_GATE: &[(f32, f32)] = &[(12.0, 12.6), (12.0, 0.05)];
const CART_IN_HEAD: &[(f32, f32)] = &[(12.0, 13.95), (12.0, 12.4), (10.4, 11.8)];

/// The builders' places along the ruin's south face, and their paths there
/// from the shaft and back.
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
/// A builder bound for a forge below walks from the camp to the shaft.
const CAMP_TO_SHAFT: &[(f32, f32)] = &[(8.6, 10.4), (12.0, 10.4), (12.0, 3.8)];

/// Where the smiths stand at the anvils of a forge-hall, and where its
/// builders work round the furnace (tiles).
pub(crate) const SMITH_SPOTS: [(f32, f32); 2] = [(6.4, 9.2), (17.6, 9.2)];
pub(crate) const FORGE_BUILDERS: [(f32, f32); 5] = [
    (9.4, 7.6),
    (14.6, 7.6),
    (9.4, 4.4),
    (14.6, 4.4),
    (12.0, 7.8),
];

/// Everyone's route, and where in it they start.
fn walkers(home: &Home) -> Vec<(Folk, Vec<Step>, u32)> {
    let b = &home.barony;
    let mut out: Vec<(Folk, Vec<Step>, u32)> = Vec::new();
    if !b.here() {
        return out;
    }
    let sworn = b.court == Court::Sworn;
    // The King: at his camp by the ruin, then on his seat.
    out.push((
        Folk::King,
        vec![if sworn {
            Step::Stay(RoomKind::KingsHall, SEAT_AT, HZ * 60, Pose::Seated, false)
        } else {
            Step::Stay(RoomKind::MineHead, CAMP_KING, HZ * 60, Pose::Idle, false)
        }],
        0,
    ));
    // His two guards: at the camp, then either side of the seat.
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
    // In the gate courtyard, the main way in: the King's man waits by the
    // signpost for whoever comes; once the workings are his, a miner off
    // his shift rests by the well.
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
    // Once the hall stands a sentry keeps its door outside.
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
    // Miners down the shaft, porters up it with the ore.
    let miners = b.miners();
    for i in 0..miners {
        let route = vec![
            Step::Walk(RoomKind::MineHead, DOWN_TO_SHAFT, false),
            Step::Away(HZ * 14),
            Step::Walk(RoomKind::MineHead, UP_FROM_SHAFT, true),
            Step::Stay(RoomKind::MineHead, (9.6, 11.2), HZ * 2, Pose::Work, true),
        ];
        let period: u32 = route.iter().map(ticks).sum();
        out.push((Folk::Miner, route, i * period / miners.max(1)));
    }
    // Builders while a work goes up: at the ruin, or down to a forge.
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
    // A carter with the realm's share, once a forge burns: out through the
    // gate courtyard to the realm's road, and back.
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

/// The folk in sight in a room of the world (or a forge-hall below, by
/// `pack`), this tick.
pub(crate) fn figures(home: &Home, room: RoomKind, pack: Pack, tick: u64) -> Vec<Figure> {
    let mut out = Vec::new();
    if room == RoomKind::Forge {
        // In a forge-hall: its builders round the furnace while it goes
        // up, its smiths at the anvils once it burns.
        let b = &home.barony;
        let Some(f) = barony::forge_of(pack) else {
            return out;
        };
        let work = f.work.and_then(barony::work);
        if b.is_lit(f.id) {
            for (i, &(x, y)) in SMITH_SPOTS.iter().enumerate() {
                out.push(Figure {
                    folk: Folk::Smith,
                    x: x * TILE_UNITS,
                    y: y * TILE_UNITS,
                    east: i == 0,
                    pose: Pose::Work,
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
    // Stand the nearer ones in front.
    out.sort_by(|a, b| a.y.total_cmp(&b.y));
    out
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_folk__tests.rs"]
mod tests;
