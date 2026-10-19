//! What a scripted knight presses. A bot reads the same state a player sees
//! (it is handed the session, not the future): it works down a list of goals,
//! the first one not yet met, and drives toward it with a small reactive
//! controller: run, hop what is in the way, hop foes and bell boulders, jump
//! gaps at their lips. Where the way needs a move the controller does not
//! guess (the triple-jump lane, a long jump, a hop up a stair of ledges, the
//! wheel's buckets, the wall-kick shafts) the map carries a hint glyph in the
//! air cell (`layout::Info::hints`), or the goal names the move.
//!
//! It holds no memory of its own: what it needs to remember it reads from the
//! knight (the keys of the last tick, which way it faces).
use super::body::{Body, Cmd, Knobs, Move, Q};
use super::boss::{self, BOSS_ID, Beat as BossBeat};
use super::knight::{B_DOWN, B_JUMP, Knight};
use super::layout::{self, HEIGHT, HUB, MARSH, MILL, SHAFT, SNOW, SUMMIT, cell_centre, info};
use super::session::{Phase, Session, View};
use super::world::{Grid, TILE, Tile};
use crate::rules::{Hero, Input};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The shipped numbers, for looking ahead.
const KNOBS: Knobs = Knobs {
    run: crate::rules::bannerpeak::RUN as i32,
    hop: crate::rules::bannerpeak::HOP,
    grace: crate::rules::bannerpeak::COYOTE,
    slam: crate::rules::bannerpeak::SLAM as i32,
    ledge: crate::rules::bannerpeak::LEDGE as i32,
};

fn hints() -> &'static BTreeMap<(i32, i32), char> {
    static H: OnceLock<BTreeMap<(i32, i32), char>> = OnceLock::new();
    H.get_or_init(|| info().hints.iter().map(|m| ((m.x, m.y), m.k)).collect())
}

/// Where the next step of the way is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    /// Stand at feet column `x` pixels.
    Walk(i32),
    /// Jump and slam down onto the floor at `x`.
    Slam(i32),
    /// Stand on the hatch at `x` and crouch.
    Hatch(i32),
    /// Walk to `x`, and drop off any ledge on the way: the goal is below.
    Down(i32),
    /// Stop short at `x` and backflip.
    Backflip(i32),
    /// Ride the waterwheel up and over, picking up its spark on the way.
    Wheel,
    /// Climb the walls toward this cell.
    Climb((i32, i32)),
    /// Walk to the gate at `x` and press E for the course board.
    Board(i32),
    /// King Cinder.
    Fight,
}

fn keys(x: i8, jump: bool, down: bool) -> Input {
    Input {
        move_x: x,
        dash: jump,
        move_y: i8::from(down),
        ..Default::default()
    }
}

impl Session {
    /// What a bot presses for knight `id`.
    pub fn bot_input(&self, id: u32, _hero: &Hero) -> Input {
        let Some(k) = self.seats.get(&id) else {
            return Input::default();
        };
        match self.phase {
            Phase::Select => {
                return Input {
                    interact: self.elapsed % 2 == 0,
                    ..Default::default()
                };
            }
            Phase::Finale => {
                return Input {
                    interact: self.completed.is_some() && self.elapsed % 2 == 0,
                    ..Default::default()
                };
            }
            Phase::Play => {}
        }
        if let Some(b) = k.bubble {
            // Steer the bubble toward the nearest friend.
            let (bx, by) = (b.x.div_euclid(Q), b.y.div_euclid(Q));
            let near = self
                .seats
                .iter()
                .filter(|(o, f)| **o != id && f.alive() && f.free())
                .map(|(_, f)| f.body.foot())
                .min_by_key(|&(x, y)| (x - bx).abs() + (y - by).abs());
            return match near {
                Some((x, y)) => Input {
                    move_x: (x - bx).signum() as i8,
                    move_y: (y - 12 - by).signum() as i8,
                    ..Default::default()
                },
                None => Input::default(),
            };
        }
        if !k.free() {
            return Input::default();
        }
        self.drive(k, self.goal(k))
    }

    /// The goal of knight `k`: the first of the list not yet met.
    pub fn goal(&self, k: &Knight) -> Goal {
        let i = info();
        let (fx, fy) = k.body.foot();
        let cx = fx.div_euclid(TILE);
        let b = self.banners;
        let row = |s: &layout::Sec, r: i32| (s.oy + r) * TILE;
        match layout::section_of(cx) {
            0 => {
                let in_cellar = fy > row(&HUB, 18) + 2
                    && (HUB.ox + 13..HUB.ox + 34).contains(&cx);
                if b & 1 == 0 && !in_cellar {
                    // The tower banner: the lane, then the roof.
                    let (bx, _) = cell_centre(i.banners[0].unwrap());
                    let on_roof = fy <= row(&HUB, 11) + 2 && fx >= (HUB.ox + 56) * TILE;
                    return Goal::Walk(if on_roof { bx } else { (HUB.ox + 58) * TILE });
                }
                if b & 2 == 0 {
                    if in_cellar {
                        // Down the steps (a fall through the floor lands on the top
                        // one), then along the floor to the banner.
                        let (bx, _) = cell_centre(i.banners[1].unwrap());
                        return if fy >= row(&HUB, 28) - 1 {
                            Goal::Down(bx)
                        } else {
                            Goal::Down((HUB.ox + 13) * TILE + 8)
                        };
                    }
                    return Goal::Slam((HUB.ox + 24) * TILE);
                }
                if in_cellar {
                    // Out by the steps: left to their foot, then right and up.
                    return if fx > (HUB.ox + 13) * TILE + 12 && fy >= row(&HUB, 28) - 1 {
                        Goal::Walk((HUB.ox + 13) * TILE + 6)
                    } else {
                        Goal::Walk((HUB.ox + 30) * TILE)
                    };
                }
                Goal::Walk((HUB.ox + 82) * TILE)
            }
            1 => {
                let room_spark = i
                    .sparks
                    .iter()
                    .position(|s| MARSH.contains_x(s.0) && s.1 > MARSH.oy + 18);
                let in_room = fy > row(&MARSH, 16);
                if let Some(n) = room_spark {
                    if self.sparks & (1 << n) == 0 && (in_room || cx < MARSH.ox + 61) {
                        if in_room {
                            return Goal::Walk(cell_centre(i.sparks[n]).0);
                        }
                        return Goal::Hatch(cell_centre(MARSH.at(44, 12)).0);
                    }
                }
                if in_room {
                    return Goal::Hatch(cell_centre(MARSH.at(56, 26)).0);
                }
                // A ring spark missed over the bog: back across for another try.
                let over_bog = i
                    .sparks
                    .iter()
                    .position(|s| MARSH.contains_x(s.0) && s.1 <= MARSH.oy + 18);
                if let Some(n) = over_bog {
                    let (sx, _) = cell_centre(i.sparks[n]);
                    if self.sparks & (1 << n) == 0 && k.pole == 0 && fx > sx + 3 * TILE {
                        return Goal::Walk(sx - 11 * TILE);
                    }
                }
                let (px, _) = cell_centre(i.pole);
                if b & 4 == 0 || k.pole > 0 {
                    return Goal::Walk(px);
                }
                Goal::Walk((MARSH.ox + MARSH.w + 4) * TILE)
            }
            2 => self.mill_goal(k),
            3 => self.shaft_goal(k),
            4 => self.snow_goal(k),
            _ => self.summit_goal(k),
        }
    }

    /// The index of the spark at world cell `c`.
    fn spark_at(&self, c: (i32, i32)) -> usize {
        info().sparks.iter().position(|&s| s == c).expect("a spark there")
    }
    fn spark_missing(&self, c: (i32, i32)) -> bool {
        self.sparks & (1 << self.spark_at(c)) == 0
    }

    fn mill_goal(&self, k: &Knight) -> Goal {
        let i = info();
        let (fx, fy) = k.body.foot();
        let cx = fx.div_euclid(TILE);
        let row = |r: i32| (MILL.oy + r) * TILE;
        let ledge_spark = MILL.at(68, 8);
        let on_wheel = k.body.plank.is_some_and(|id| id >= layout::WHEEL_ID);
        // The race, by where the knight is, not by what a friend has done.
        let past_house = fx >= (MILL.ox + 38) * TILE && fy <= row(8) + 2;
        if !past_house && cx < MILL.ox + 38 {
            // The piers and the wheel: run to the second pier's edge, then ride.
            if on_wheel || (cx >= MILL.ox + 24 && fy >= row(0)) {
                return Goal::Wheel;
            }
            return Goal::Walk((MILL.ox + 28) * TILE + 12);
        }
        if past_house && self.banners & 8 == 0 {
            return Goal::Walk(cell_centre(i.banners[3].unwrap()).0);
        }
        if self.spark_missing(ledge_spark) && cx >= MILL.ox + 46 {
            // The backflip: stop short of the ledge, on the bank.
            let on_ledge = fy <= row(9) + 2 && fx < (MILL.ox + 70) * TILE;
            if on_ledge {
                return Goal::Walk((MILL.ox + 68) * TILE + 8);
            }
            return Goal::Backflip((MILL.ox + 71) * TILE + 8);
        }
        Goal::Walk((MILL.ox + MILL.w + 3) * TILE)
    }

    fn shaft_goal(&self, k: &Knight) -> Goal {
        let i = info();
        let (fx, fy) = k.body.foot();
        let top = (SHAFT.oy + 6) * TILE;
        let b = &k.body;
        let up = b.ground && fy <= top && fx >= (SHAFT.ox + 10) * TILE;
        if up {
            return Goal::Walk((SHAFT.ox + SHAFT.w + 3) * TILE);
        }
        // In through the door at the foot, to the middle of the shaft.
        let foot = (SHAFT.oy + 34) * TILE;
        if b.ground && fy == foot && fx < (SHAFT.ox + 6) * TILE {
            return Goal::Walk((SHAFT.ox + 8) * TILE);
        }
        // The lower alcove's spark, the notch's, then the bell.
        for c in [SHAFT.at(4, 26), SHAFT.at(10, 13)] {
            if self.spark_missing(c) {
                return Goal::Climb(c);
            }
        }
        Goal::Climb(i.banners[4].unwrap())
    }

    fn snow_goal(&self, k: &Knight) -> Goal {
        let i = info();
        let (fx, fy) = k.body.foot();
        let cx = fx.div_euclid(TILE);
        let row = |r: i32| (SNOW.oy + r) * TILE;
        let shelf_spark = SNOW.at(25, 3);
        let chute_spark = SNOW.at(73, 0);
        if self.spark_missing(shelf_spark) {
            // Up among the shelves (on one, or between): at the spark; else along
            // the floor to the first step.
            let up = fy <= row(7) + 1 && (SNOW.ox + 20..SNOW.ox + 37).contains(&cx);
            if up {
                return Goal::Walk(cell_centre(shelf_spark).0);
            }
            return Goal::Walk((SNOW.ox + 31) * TILE);
        }
        if self.spark_missing(chute_spark) {
            let in_cols = (SNOW.ox + 72..SNOW.ox + 76).contains(&cx);
            let inside = in_cols && (fy < row(8) || !k.body.ground);
            if inside || (k.body.mode == Move::Wall && (SNOW.ox + 70..SNOW.ox + 78).contains(&cx)) {
                return Goal::Climb(chute_spark);
            }
            return Goal::Walk(cell_centre((SNOW.ox + 73, 9)).0);
        }
        if self.banners & 32 == 0 {
            return Goal::Walk(cell_centre(i.banners[5].unwrap()).0);
        }
        if self.doors & 2 == 0 {
            // The summit door stays shut: a banner is still to find. Back to
            // the last gate for the course board, which sends the party to it.
            return Goal::Board(cell_centre(SNOW.at(54, 9)).0);
        }
        Goal::Walk((SUMMIT.ox + 6) * TILE)
    }

    fn summit_goal(&self, k: &Knight) -> Goal {
        let i = info();
        if self.boss.defeated() && self.banners & 128 == 0 {
            return Goal::Walk(cell_centre(i.banners[7].unwrap()).0);
        }
        let _ = k;
        Goal::Fight
    }

    // ------------------------------------------------------------ driving

    /// The reactive controller: keys that take knight `k` toward `goal`.
    pub fn drive(&self, k: &Knight, goal: Goal) -> Input {
        let b = &k.body;
        let (fx, fy) = b.foot();
        let held_jump = k.prev & B_JUMP != 0;
        let held_down = k.prev & B_DOWN != 0;
        // A slam landing, a hit: nothing to press.
        if matches!(b.mode, Move::Recover | Move::Hurt) {
            return Input::default();
        }
        if matches!(b.mode, Move::Wind | Move::Pound) {
            return keys(0, false, true);
        }
        let ground = b.ground && matches!(b.mode, Move::Ground | Move::Brace | Move::Belly);
        let tx = match goal {
            Goal::Walk(x)
            | Goal::Slam(x)
            | Goal::Hatch(x)
            | Goal::Down(x)
            | Goal::Backflip(x)
            | Goal::Board(x) => x,
            Goal::Climb(c) => cell_centre(c).0,
            Goal::Wheel | Goal::Fight => fx,
        };
        let dx = tx - fx;
        let d: i8 = if dx.abs() < 3 { 0 } else { dx.signum() as i8 };
        match goal {
            Goal::Slam(_) => {
                // Stand over it, jump, and slam at the top.
                if ground {
                    if dx.abs() > 40 {
                        return self.walk_with_hops(k, d);
                    }
                    // Come to rest over it, then straight up.
                    if dx.abs() <= 5 && b.vx.abs() < 120 {
                        return keys(0, !held_jump, false);
                    }
                    let creep = if b.vx.abs() < 150 { d } else { 0 };
                    return keys(if dx.abs() <= 5 { 0 } else { creep }, false, false);
                }
                if b.vy > 0 && b.mode == Move::Air {
                    return keys(0, false, !held_down);
                }
                return keys(0, held_jump && b.vy < 0, false);
            }
            Goal::Hatch(_) => {
                if ground {
                    if dx.abs() > 4 {
                        return self.walk_with_hops(k, d);
                    }
                    return keys(0, false, !held_down);
                }
                return keys(d, held_jump && b.vy < 0, false);
            }
            Goal::Board(_) => {
                if ground {
                    if dx.abs() > 8 {
                        return self.walk_with_hops(k, d);
                    }
                    return Input {
                        interact: k.prev & super::knight::B_BRACE == 0,
                        ..Default::default()
                    };
                }
                return keys(d, held_jump && b.vy < 0, false);
            }
            Goal::Backflip(_) => {
                if ground {
                    if dx.abs() > 40 {
                        return self.walk_with_hops(k, d);
                    }
                    if b.crouch {
                        return keys(0, !held_jump, true);
                    }
                    if dx.abs() <= 4 && b.vx.abs() < 150 {
                        return keys(0, false, true);
                    }
                    let creep = if b.vx.abs() < 150 { d } else { 0 };
                    return keys(if dx.abs() <= 4 { 0 } else { creep }, false, false);
                }
                // In the air the flip carries itself.
                return keys(0, held_jump && b.vy < 0, false);
            }
            Goal::Wheel => return self.ride(k),
            Goal::Climb(c) => return self.climb(k, c),
            Goal::Fight => return self.fight(k),
            Goal::Walk(_) | Goal::Down(_) => {}
        }
        if !ground {
            // Up through the hole out of the cellar: straight up, then over.
            let hole = (HUB.ox + 22) * TILE..(HUB.ox + 26) * TILE;
            if self.banners & 2 != 0 && hole.contains(&fx) && fy > (HUB.oy + 17) * TILE {
                return keys(if b.vy < 0 { 0 } else { 1 }, held_jump && b.vy < 0, false);
            }
            // In the air: keep going, hold the jump while rising.
            return keys(d, held_jump && b.vy < 0, false);
        }
        // Last of the cellar's steps: straight up through the hole.
        if self.banners & 2 != 0
            && fy == (HUB.oy + 21) * TILE
            && (HUB.ox + 23..HUB.ox + 26).contains(&fx.div_euclid(TILE))
        {
            if b.vx.abs() > 200 {
                return keys(-(b.vx.signum() as i8), false, false);
            }
            return keys(0, !held_jump, false);
        }
        // Hints in the air cell: the way's own instructions.
        if let Some(h) = self.hint_here(b) {
            match h {
                '&' if d > 0 => {
                    // The triple lane: hop at speed, and keep hopping.
                    if b.vx.abs() >= 1100 || (b.hop_t > 0 && b.hop_next >= 2) {
                        return keys(1, !held_jump, false);
                    }
                }
                // A stair's hop is made slowly, from near a stand.
                '>' | '<' => {
                    let dir: i8 = if h == '>' { 1 } else { -1 };
                    if d == dir || d == 0 {
                        if b.vx.abs() < 320 {
                            return keys(dir, !held_jump, false);
                        }
                        return keys(-(b.vx.signum() as i8), false, false);
                    }
                }
                '|' => return keys(d, !held_jump, false),
                '/' if d > 0 => {
                    if b.crouch {
                        return keys(1, !held_jump, true);
                    }
                    if b.vx.abs() >= 760 {
                        return keys(1, false, true);
                    }
                }
                _ => {}
            }
        }
        // The lane: a failed run at the tower goes back for another.
        let lane = (HUB.oy + 18) * TILE;
        if self.banners & 1 == 0 && fy == lane && fx > (HUB.ox + 55) * TILE && fx < (HUB.ox + 60) * TILE {
            return keys(-1, false, false);
        }
        if self.banners & 1 == 0
            && fy == lane
            && b.face < 0
            && fx > (HUB.ox + 27) * TILE
            && fx < (HUB.ox + 56) * TILE
        {
            return keys(-1, false, false);
        }
        if let Goal::Down(_) = goal {
            // Wall and foes, but never the lip of a drop.
            let held_jump = k.prev & B_JUMP != 0;
            if d != 0 && self.obstacle_ahead(k, d, false) {
                return keys(d, !held_jump, false);
            }
            return keys(d, false, false);
        }
        self.walk_with_hops(k, d)
    }

    // ---------------------------------------------------------- looking ahead

    /// The body `b` stepped `n` ticks with `keys(n)`, on the map and the moving
    /// platforms as they will be: whether `want` of where it comes to rest.
    fn foresee(
        &self,
        k: &Knight,
        limit: usize,
        keys_at: &dyn Fn(usize) -> (i8, bool),
        settle: bool,
        want: &dyn Fn(&Body) -> bool,
    ) -> bool {
        let mut b = k.body.clone();
        let view = View {
            world: layout::world(),
            changed: &self.changed,
        };
        let mut out = Vec::new();
        let mut prev = k.prev & B_JUMP != 0;
        for n in 0..limit {
            let (x, jump) = keys_at(n);
            let cmd = Cmd {
                x,
                jump,
                jump_edge: jump && !prev,
                ..Default::default()
            };
            prev = jump;
            let now = layout::movers(self.elapsed + 1 + n as u32);
            let before = layout::movers(self.elapsed + n as u32);
            out.clear();
            b.step(cmd, &KNOBS, &view, &now, &before, &mut out);
            if n > 1 && b.ground {
                if settle {
                    // Brake on what it landed on, and see whether it stays there.
                    for m in 0..16 {
                        let brake = if b.vx.abs() > 150 { -(b.vx.signum() as i8) } else { 0 };
                        let cmd = Cmd {
                            x: brake,
                            ..Default::default()
                        };
                        let step = (n + 1 + m) as u32;
                        let now = layout::movers(self.elapsed + 1 + step);
                        let before = layout::movers(self.elapsed + step);
                        out.clear();
                        b.step(cmd, &KNOBS, &view, &now, &before, &mut out);
                    }
                }
                return want(&b);
            }
            if b.foot().1 > HEIGHT * TILE {
                return false;
            }
        }
        false
    }

    /// Ride the waterwheel: board it from the second pier, rise with it, take
    /// its spark at the top, and hop to the mill's roof.
    fn ride(&self, k: &Knight) -> Input {
        let b = &k.body;
        let fx = b.foot().0;
        let held_jump = k.prev & B_JUMP != 0;
        let bucket = |bb: &Body| bb.plank.is_some_and(|id| id >= layout::WHEEL_ID);
        let roof = (MILL.oy + 8) * TILE;
        let on_roof = |bb: &Body| bb.ground && bb.plank.is_none() && bb.foot().1 == roof && bb.foot().0 > (MILL.ox + 38) * TILE;
        let (cx, cy) = layout::WHEEL;
        let spark = MILL.at(33, 5);
        if !b.ground {
            // The jump for the spark goes straight up; every other goes on to the right.
            let up = self.spark_missing(spark) && b.vx.abs() < 300 && (fx - cx).abs() < 30;
            return keys(if up { 0 } else { 1 }, held_jump && b.vy < 0, false);
        }
        if bucket(b) {
            // Landed at speed: brake, or run off the far end.
            if b.vx.abs() > 450 {
                return keys(-(b.vx.signum() as i8), false, false);
            }
            // At the top, for the spark above.
            if self.spark_missing(spark) {
                let plank = layout::movers(self.elapsed)
                    .into_iter()
                    .find(|m| Some(m.id) == b.plank)
                    .map(|m| m.p);
                if let Some(p) = plank {
                    let near_top = (p.x + p.w / 2 - cx).abs() <= 24 && p.y <= cy - 30;
                    if near_top {
                        if (fx - cx).abs() <= 9 {
                            return keys(0, !held_jump, false);
                        }
                        // Walk along the bucket to the spark's column, never off it.
                        let d = (cx - fx).signum() as i8;
                        if fx + i32::from(d) * 4 > p.x + 4 && fx + i32::from(d) * 4 < p.x + p.w - 4 {
                            // A creep: never fast enough to leave the bucket.
                            return keys(if b.vx.abs() < 200 { d } else { 0 }, false, false);
                        }
                    }
                }
                return Input::default();
            }
            if self.foresee(k, 50, &|n| (1, n < 17), false, &on_roof) {
                return keys(1, !held_jump, false);
            }
            return Input::default();
        }
        // On the pier: run at the bucket as it comes, and jump when the jump would
        // land on one; if not, brake at the edge and take another run.
        let (left, right) = ((MILL.ox + 24) * TILE, (MILL.ox + 29) * TILE);
        if b.vx > 700 && self.foresee(k, 45, &|n| (1, n < 16), true, &bucket) {
            return keys(1, !held_jump, false);
        }
        if fx >= right - 15 && b.vx > 300 {
            return keys(-1, false, false);
        }
        if b.vx < -100 && fx > left + 14 {
            return keys(-1, false, false);
        }
        keys(1, false, false)
    }

    /// Climb between two close walls by kicks, timing a kick to pass through
    /// the spark or banner at `target` when one is close.
    fn climb(&self, k: &Knight, target: (i32, i32)) -> Input {
        let b = &k.body;
        let held_jump = k.prev & B_JUMP != 0;
        if b.ground {
            // A lunge at the right wall; out of a notch, a lunge back across.
            let (lo, hi) = if SHAFT.contains_x(b.foot().0.div_euclid(TILE)) {
                ((SHAFT.ox + 6) * TILE, (SHAFT.ox + 10) * TILE)
            } else {
                ((SNOW.ox + 72) * TILE, (SNOW.ox + 76) * TILE)
            };
            let x = b.foot().0;
            let d = if x < lo + 6 {
                1
            } else if x > hi - 6 {
                -1
            } else {
                1
            };
            return keys(d, !held_jump, false);
        }
        if b.mode == Move::Wall {
            let side = b.wall;
            // A key still down from the lunge is let go first, leaning on the wall.
            if held_jump {
                return keys(side, false, false);
            }
            let (tx, ty) = cell_centre(target);
            let (_, fy) = b.foot();
            // Past a spark still to be had: slide back down to it, not on up.
            let spark = info().sparks.contains(&target);
            if spark && fy + 26 < ty {
                return keys(side, false, false);
            }
            // Kick now, unless a short wait makes the arc pass through the target.
            if (ty - fy).abs() < 90 && (tx - b.foot().0).abs() < 110 {
                for wait in 0..=56 {
                    if self.foresee_kick(k, wait, (tx, ty)) {
                        return if wait == 0 {
                            keys(-side, true, false)
                        } else {
                            keys(side, false, false)
                        };
                    }
                }
            }
            return keys(-side, true, false);
        }
        // In the air: on toward the wall faced, holding what was begun.
        keys(b.face, held_jump && b.vy < 0, false)
    }

    /// Whether, sliding `wait` more ticks and then kicking off, the body passes
    /// through the box around `(tx, ty)`.
    pub fn foresee_kick(&self, k: &Knight, wait: usize, (tx, ty): (i32, i32)) -> bool {
        let mut b = k.body.clone();
        let view = View {
            world: layout::world(),
            changed: &self.changed,
        };
        let side = b.wall;
        let mut out = Vec::new();
        for n in 0..wait + 36 {
            let kick = n == wait;
            let x = if n < wait { side } else { -side };
            let cmd = Cmd {
                x,
                jump: kick,
                jump_edge: kick,
                ..Default::default()
            };
            out.clear();
            b.step(cmd, &KNOBS, &view, &[], &[], &mut out);
            let (l, t, r, bt) = b.rect();
            let (l, t, r, bt) = (l / Q, t / Q, r / Q, bt / Q);
            if l < tx + 8 && r > tx - 8 && t < ty + 8 && bt > ty - 8 {
                return n >= wait;
            }
            // The arc ends at the next wall or floor.
            if n > wait && (b.ground || b.mode == Move::Wall) {
                return false;
            }
        }
        false
    }

    /// King Cinder: keep out of what is marked, and when his crown is low, go
    /// up onto it and slam.
    fn fight(&self, k: &Knight) -> Input {
        let bo = &self.boss;
        let b = &k.body;
        let (fx, fy) = b.foot();
        let held_jump = k.prev & B_JUMP != 0;
        let held_down = k.prev & B_DOWN != 0;
        let floor = boss::floor();
        let ground = b.ground && matches!(b.mode, Move::Ground | Move::Belly);
        let lo = (SUMMIT.ox + 5) * TILE;
        let hi = (SUMMIT.ox + 36) * TILE;
        let on_head = b.plank == Some(BOSS_ID);
        if bo.beat == BossBeat::Dormant {
            // Walk in to wake him.
            return keys(if fx < boss::home() - 120 { 1 } else { 0 }, false, false);
        }
        // What is marked: the knight steps off it, or hops a wave.
        let mut flee: Option<i8> = None;
        for h in &bo.hazards {
            if h.live == 0 && h.tell == 0 {
                continue;
            }
            let (l, _, r, _) = h.rect();
            if h.vx != 0 {
                // A wave: hop it when it is near.
                let c = (l + r) / 2;
                let toward = (fx - c) * h.vx > 0;
                if toward && (fx - c).abs() < 40 && ground && (fy - floor).abs() < 3 && h.tell == 0 {
                    return keys(0, !held_jump, false);
                }
                continue;
            }
            if fx > l - 18 && fx < r + 18 {
                let mid = (l + r) / 2;
                let away: i8 = if fx >= mid { 1 } else { -1 };
                // A column is narrow; a sweep is long: leave by the near end.
                flee = Some(away);
            }
        }
        if let Some(d) = flee {
            let d = if (d > 0 && fx > hi - 10) || (d < 0 && fx < lo + 10) { -d } else { d };
            return keys(d, ground && !held_jump && (b.vy == 0) && false, false);
        }
        let dx = bo.x - fx;
        match bo.beat {
            BossBeat::Wince if bo.plates > 0 => {
                if !ground && !on_head {
                    // Over the crown: come down on it.
                    if b.mode == Move::Air && b.vy > 0 && dx.abs() <= 20 {
                        return keys(0, false, !held_down);
                    }
                    if b.mode == Move::Air && b.vy > 0 && fy < floor - 4 && dx.abs() > 20 {
                        return keys(dx.signum() as i8, false, false);
                    }
                    return keys(dx.signum() as i8, held_jump && b.vy < 0, false);
                }
                if on_head {
                    // Another blow from where it stands.
                    return keys(0, !held_jump, false);
                }
                if dx.abs() > 60 {
                    return keys(dx.signum() as i8, false, false);
                }
                // A running jump onto the head.
                return keys(dx.signum() as i8, !held_jump, false);
            }
            BossBeat::Rage | BossBeat::Down => keys(0, false, false),
            _ => {
                // Keep a distance, on the side held, and off his head.
                if on_head {
                    return keys(if dx >= 0 { -1 } else { 1 }, false, false);
                }
                let want = if fx >= bo.x { bo.x + 120 } else { bo.x - 120 };
                let want = want.clamp(lo, hi);
                let d = (want - fx).signum() as i8;
                keys(if (want - fx).abs() < 6 { 0 } else { d }, false, false)
            }
        }
    }

    fn hint_here(&self, b: &Body) -> Option<char> {
        let (fx, fy) = b.foot();
        let (cx, cy) = (fx.div_euclid(TILE), (fy - 1).div_euclid(TILE));
        // The cellar's steps are for the way out.
        if (HUB.ox + 13..HUB.ox + 34).contains(&cx) && cy >= HUB.oy + 20 && self.banners & 2 == 0 {
            return None;
        }
        hints()
            .get(&(cx, cy))
            .or_else(|| hints().get(&(cx, cy - 1)))
            .copied()
    }

    /// Walk toward `d`, hopping what is ahead.
    fn walk_with_hops(&self, k: &Knight, d: i8) -> Input {
        let held_jump = k.prev & B_JUMP != 0;
        if d == 0 {
            return Input::default();
        }
        if self.obstacle_ahead(k, d, true) {
            return keys(d, !held_jump, false);
        }
        keys(d, false, false)
    }

    /// Whether a hop is wanted now: a wall, a gap's lip, spikes, a foe, a boulder.
    fn obstacle_ahead(&self, k: &Knight, d: i8, gaps: bool) -> bool {
        let b = &k.body;
        let (fx, fy) = b.foot();
        let view = self.view();
        let d32 = i32::from(d);
        let half = b.dims().0 / 2;
        // A wall right ahead.
        let lead = fx + d32 * (half + 3);
        for dy in [4, 12, 22] {
            if view.tile(lead.div_euclid(TILE), (fy - dy).div_euclid(TILE)).solid() {
                return true;
            }
        }
        // Spikes ahead.
        for dist in [8, 16, 24] {
            let x = (fx + d32 * dist).div_euclid(TILE);
            if view.tile(x, (fy - 1).div_euclid(TILE)) == Tile::Spikes {
                return true;
            }
        }
        // The lip of a gap: no footing a step ahead.
        let foot_ahead = fx + d32 * (half + 2);
        let under = |x: i32| {
            let t = view.tile(x.div_euclid(TILE), fy.div_euclid(TILE));
            t.solid() || t.one_way()
        };
        if gaps && b.ground && b.plank.is_none() && !under(foot_ahead) && !under(fx + d32 * (half + 8)) {
            // Only a real gap: no floor within a short drop either.
            let drop_floor = (1..=3).any(|r| {
                view.tile(foot_ahead.div_euclid(TILE), fy.div_euclid(TILE) + r)
                    .solid()
            });
            if !drop_floor {
                return true;
            }
        }
        // A foe ahead on our level.
        for f in &self.foes {
            if !f.alive() {
                continue;
            }
            let (fxp, fyp) = (f.x.div_euclid(Q), f.y.div_euclid(Q));
            let ahead = (fxp - fx) * d32;
            if (4..42).contains(&ahead) && (fyp - fy).abs() < 22 {
                return true;
            }
        }
        // A bell boulder rolling at us.
        for bl in &self.boulders {
            let bx = bl.x.div_euclid(Q);
            let ahead = (bx - fx) * d32;
            if (0..78).contains(&ahead) && (layout::SNOW_FLOOR * TILE - fy).abs() < 8 {
                return true;
            }
        }
        false
    }
}
