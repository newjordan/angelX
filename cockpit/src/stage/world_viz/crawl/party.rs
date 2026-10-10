//! The party's way through a floor: a route of halls from the one it starts
//! in to the one with the stairs down, one hall for each turn of the loop;
//! the walk between, a cell at a time with quarter turns, as the old games
//! went; and where it stands and looks once it's there.

use super::dungeon::Dungeon;
use crate::drive::together_shooter::RoomKind;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Halls on a floor's route, at most: the start, a few on the way, the
/// stairs.
pub(crate) const STATIONS: usize = 6;
/// World ticks (40 a second) for a step, and for a quarter turn.
pub(crate) const STEP: u32 = 16;
pub(crate) const TURN: u32 = 10;
/// The longest a walk between halls may take; a long one is walked faster.
const LONGEST: u32 = 40 * 24;

/// The halls the party visits, in order: the start, the nearest halls by
/// doorway, and the stairs (or the lair) last.
pub(crate) fn route(d: &Dungeon) -> Vec<usize> {
    let start = d.start();
    let mut seen = vec![false; d.halls.len()];
    let mut order = vec![start];
    seen[start] = true;
    let mut i = 0;
    while i < order.len() {
        for &next in &d.halls[order[i]].ways {
            if !seen[next] {
                seen[next] = true;
                order.push(next);
            }
        }
        i += 1;
    }
    let last = order
        .iter()
        .position(|&h| matches!(d.halls[h].kind, RoomKind::Stairs | RoomKind::Lair));
    let stairs = last.map(|k| order.remove(k));
    order.truncate(STATIONS - usize::from(stairs.is_some()));
    order.extend(stairs);
    order
}

/// Where the party stands in a hall: a walkable cell as near its middle as
/// can be found.
pub(crate) fn spot(d: &Dungeon, hall: usize) -> (i32, i32) {
    let (cx, cy) = d.halls[hall].centre();
    for r in 0..8i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let p = (cx + dx, cy + dy);
                if d.walkable(p) {
                    return p;
                }
            }
        }
    }
    (cx, cy)
}

/// A heading for a step from `a` to the next cell `b`.
fn heading((ax, ay): (i32, i32), (bx, by): (i32, i32)) -> f32 {
    match (bx - ax, by - ay) {
        (1, _) => 0.0,
        (_, 1) => FRAC_PI_2,
        (-1, _) => PI,
        _ => 3.0 * FRAC_PI_2,
    }
}

/// The turn from heading `a` to `b`, the short way round.
fn turn(a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(TAU);
    if d > PI { d - TAU } else { d }
}

/// A walk, laid out in time: each piece a quarter turn or a step.
pub(crate) struct Walk {
    pieces: Vec<Piece>,
    /// Ticks per step and per turn: a long walk goes faster.
    step: u32,
    turn: u32,
}

#[derive(Clone, Copy, Debug)]
enum Piece {
    Turn {
        at: (i32, i32),
        from: f32,
        by: f32,
    },
    Step {
        from: (i32, i32),
        to: (i32, i32),
        heading: f32,
    },
}

impl Walk {
    /// The walk along `cells`, setting out facing `facing`.
    pub(crate) fn along(cells: &[(i32, i32)], facing: f32) -> Walk {
        let mut pieces = Vec::new();
        let mut now = facing;
        for pair in cells.windows(2) {
            let want = heading(pair[0], pair[1]);
            let by = turn(now, want);
            if by.abs() > 0.01 {
                pieces.push(Piece::Turn {
                    at: pair[0],
                    from: now,
                    by,
                });
            }
            pieces.push(Piece::Step {
                from: pair[0],
                to: pair[1],
                heading: want,
            });
            now = want;
        }
        let (steps, turns) = pieces.iter().fold((0u32, 0u32), |(s, t), p| match p {
            Piece::Step { .. } => (s + 1, t),
            Piece::Turn { by, .. } => (s, t + (by.abs() / FRAC_PI_2).round() as u32),
        });
        let full = steps * STEP + turns * TURN;
        let (step, turn) = if full > LONGEST {
            (
                (STEP * LONGEST / full.max(1)).max(3),
                (TURN * LONGEST / full.max(1)).max(2),
            )
        } else {
            (STEP, TURN)
        };
        Walk { pieces, step, turn }
    }

    pub(crate) fn ticks(&self) -> u32 {
        self.pieces.iter().map(|p| self.length(p)).sum()
    }

    fn length(&self, piece: &Piece) -> u32 {
        match piece {
            Piece::Step { .. } => self.step,
            Piece::Turn { by, .. } => self.turn * (by.abs() / FRAC_PI_2).round().max(1.0) as u32,
        }
    }

    /// Where the party is `t` ticks into the walk, which way it faces, and
    /// how far through a step it is (for the bob); none once it's arrived.
    pub(crate) fn at(&self, t: u32) -> Option<((f32, f32), f32, f32)> {
        let mut left = t;
        for piece in &self.pieces {
            let len = self.length(piece);
            if left < len {
                let k = left as f32 / len as f32;
                let ease = k * k * (3.0 - 2.0 * k);
                return Some(match *piece {
                    Piece::Turn { at, from, by } => (centre(at), from + by * ease, 0.0),
                    Piece::Step { from, to, heading } => {
                        let (a, b) = (centre(from), centre(to));
                        (
                            (a.0 + (b.0 - a.0) * ease, a.1 + (b.1 - a.1) * ease),
                            heading,
                            k,
                        )
                    }
                });
            }
            left -= len;
        }
        None
    }

    /// The way the party faces at the end.
    pub(crate) fn last_heading(&self, facing: f32) -> f32 {
        self.pieces
            .iter()
            .rev()
            .map(|p| match p {
                Piece::Step { heading, .. } => *heading,
                Piece::Turn { from, by, .. } => from + by,
            })
            .next()
            .unwrap_or(facing)
    }
}

pub(crate) fn centre((x, y): (i32, i32)) -> (f32, f32) {
    (x as f32 + 0.5, y as f32 + 0.5)
}

/// Which way from `at` there's the most open floor to look into: the way a
/// party arriving in a hall turns to face it.
pub(crate) fn widest(d: &Dungeon, at: (i32, i32)) -> f32 {
    let mut best = (0, 0.0);
    for (k, (dx, dy)) in [(1, 0), (0, 1), (-1, 0), (0, -1)].into_iter().enumerate() {
        let mut n = 0;
        let (mut x, mut y) = at;
        while n < 12 {
            x += dx;
            y += dy;
            if !d.walkable((x, y)) {
                break;
            }
            n += 1;
        }
        if n > best.0 {
            best = (n, k as f32 * FRAC_PI_2);
        }
    }
    best.1
}

/// How far the floor runs open straight ahead of `at` facing `heading`.
pub(crate) fn open_ahead(d: &Dungeon, at: (i32, i32), heading: f32) -> i32 {
    let (dx, dy) = (heading.cos().round() as i32, heading.sin().round() as i32);
    let mut n = 0;
    let (mut x, mut y) = at;
    while n < 12 {
        x += dx;
        y += dy;
        if !d.walkable((x, y)) {
            break;
        }
        n += 1;
    }
    n
}
