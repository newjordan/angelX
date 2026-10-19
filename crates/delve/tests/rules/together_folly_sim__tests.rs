//! The Wax Maze played: bots and scripted thumbs through a whole maze for
//! one to four knights, what happened printed, and shots of it.
//!
//! `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve wax_maze_sim -- --ignored --nocapture`
use super::*;
use std::collections::VecDeque;

const NAMES: [&str; 4] = ["You", "Wren", "Bors", "Enid"];

// The maze's API as the harness reads it.
fn w_out(w: &folly::Wraith) -> bool {
    w.roaming()
}
fn w_pos(_m: &folly::WaxMaze, w: &folly::Wraith) -> (f32, f32) {
    w.pos()
}
fn w_pale(m: &folly::WaxMaze, w: &folly::Wraith) -> bool {
    m.pale_now(w)
}
fn w_caught(_m: &folly::WaxMaze, w0: &folly::Wraith, w: &folly::Wraith) -> Option<u32> {
    (!w0.eyes && w.eyes).then_some(0)
}
fn w_home_bound(m: &folly::WaxMaze, w: &folly::Wraith) -> bool {
    w.eyes || m.in_crypt(w.at) || m.in_crypt(w.to)
}
fn pale_turned(m: &folly::WaxMaze, before: &folly::WaxMaze) -> bool {
    m.pale == folly::PALE || m.scatter() != before.scatter()
}
const PALE_WARN: u32 = folly::PALE_WARN;

/// How a knight is driven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Thumb {
    /// Path-finds to the nearest wax away from calm wraiths, pressing
    /// diagonals to line up as a bot would.
    Bot,
    /// One arrow key at a time, the next one pressed a little before the
    /// turn and the last let go, as a person plays.
    Human,
    /// As `Human`, firing, bombing and casting as it goes.
    Rowdy,
    /// As `Human`, to the nearest wax with no mind for the wraiths.
    Careless,
    /// Presses nothing.
    Idle,
}

#[derive(Debug, Default)]
pub(super) struct Report {
    pub size: (i32, i32),
    pub wax_start: usize,
    pub wax_left: usize,
    pub unreachable_wax: usize,
    pub unreachable_floor: usize,
    pub eaten: BTreeMap<u32, u16>,
    pub touched: BTreeMap<u32, u32>,
    pub caught: BTreeMap<u32, u32>,
    pub gold: BTreeMap<u32, i64>,
    pub laurel: Vec<u32>,
    pub snags: BTreeMap<u32, u32>,
    pub snag_ticks: BTreeMap<u32, u32>,
    pub knight_in_stone: u32,
    pub wraith_in_stone: u32,
    pub wraith_stuck: u32,
    pub wraith_stacked: u32,
    pub wraith_reversals: u32,
    pub door_recatch: u32,
    pub ended_at: Option<u32>,
    pub found: String,
    /// Each temper's distance to the nearest knight while hunting (tiles,
    /// summed) and the ticks it was summed over.
    pub hunt: [(f32, u32); 4],
    /// Knights fallen by the end (nothing in the maze hurts).
    pub fallen: usize,
}

impl Report {
    pub fn line(&self, knights: u32, thumbs: &[Thumb]) -> String {
        format!(
            "{}k {:?} {}x{} wax {}/{} unreach {}w/{}f | eaten {:?} touched {:?} caught {:?} gold {:?} crown {:?} | snags {:?} ({:?} t) | stone k{} w{} stuck {} stacked {} rev {} recatch {} | hunt H{:.1} C{:.1} F{:.1} D{:.1} ({}s hunting) | fallen {} | end {:?}s {}",
            knights,
            thumbs,
            self.size.0,
            self.size.1,
            self.wax_start - self.wax_left,
            self.wax_start,
            self.unreachable_wax,
            self.unreachable_floor,
            self.eaten,
            self.touched,
            self.caught,
            self.gold,
            self.laurel,
            self.snags,
            self.snag_ticks,
            self.knight_in_stone,
            self.wraith_in_stone,
            self.wraith_stuck,
            self.wraith_stacked,
            self.wraith_reversals,
            self.door_recatch,
            self.hunt[0].0 / self.hunt[0].1.max(1) as f32,
            self.hunt[1].0 / self.hunt[1].1.max(1) as f32,
            self.hunt[2].0 / self.hunt[2].1.max(1) as f32,
            self.hunt[3].0 / self.hunt[3].1.max(1) as f32,
            self.hunt.iter().map(|h| h.1).sum::<u32>() / HZ,
            self.fallen,
            self.ended_at.map(|t| t as f32 / HZ as f32),
            self.found
        )
    }
}

/// A run of `knights` knights about to walk into the floor's Wax Maze.
pub(super) fn party(knights: u32, seed: u64) -> (Run, usize) {
    let mut run = Run::new(seed, 1, None);
    for id in 2..=knights {
        run.join_seat(id, NAMES[id as usize - 1]);
    }
    run.enemies.clear();
    run.pick_folly();
    let index = run.folly_room.expect("a folly room");
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 100);
    }
    (run, index)
}

fn open(room: &Room, (x, y): (i32, i32)) -> bool {
    matches!(room.tile(x, y), Tile::Floor)
}

/// Tiles reachable on foot from `from`.
fn reach(room: &Room, from: (i32, i32)) -> BTreeSet<(i32, i32)> {
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let n = (x + dx, y + dy);
            if open(room, n) && seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen
}

/// The first step from `from` toward the nearest tile `goal` likes,
/// keeping off `danger`.
fn path_step(
    room: &Room,
    from: (i32, i32),
    goal: impl Fn((i32, i32)) -> bool,
    danger: &BTreeSet<(i32, i32)>,
) -> Option<(i32, i32)> {
    let mut prev: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::new();
    let mut queue = VecDeque::from([from]);
    prev.insert(from, from);
    while let Some(at) = queue.pop_front() {
        if at != from && goal(at) {
            let mut step = at;
            while prev[&step] != from {
                step = prev[&step];
            }
            return Some(step);
        }
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let n = (at.0 + dx, at.1 + dy);
            if open(room, n) && !danger.contains(&n) && !prev.contains_key(&n) {
                prev.insert(n, at);
                queue.push_back(n);
            }
        }
    }
    None
}

fn tile_of(h: &Hero) -> (i32, i32) {
    ((h.x / TILE_UNITS).floor() as i32, (h.y / TILE_UNITS).floor() as i32)
}

/// Per-knight memory of the human thumb: the key held.
#[derive(Clone, Copy, Default)]
struct Held {
    key: (i8, i8),
}

fn drive(run: &Run, id: u32, thumb: Thumb, held: &mut Held, tick: u32) -> Input {
    let Some(maze) = run.folly.as_ref() else {
        return Input::default();
    };
    let hero = &run.players[&id];
    if thumb == Thumb::Idle || hero.hp == 0 {
        return Input::default();
    }
    let room = run.room();
    let cols = room.cols as i32;
    let here = tile_of(hero);
    let pale_now = |w: &folly::Wraith| w_pale(maze, w);
    let mut danger = BTreeSet::new();
    for w in maze.wraiths.iter().filter(|w| w_out(w) && !pale_now(w)) {
        let (wx, wy) = w_pos(maze, w);
        let wt = (wx.floor() as i32, wy.floor() as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if (dx as i32).abs() + (dy as i32).abs() <= 1 {
                    danger.insert((wt.0 + dx, wt.1 + dy));
                }
            }
        }
    }
    danger.remove(&here);
    let prey: BTreeSet<(i32, i32)> = maze
        .wraiths
        .iter()
        .filter(|w| w_out(w) && pale_now(w))
        .map(|w| {
            let (wx, wy) = w_pos(maze, w);
            (wx.floor() as i32, wy.floor() as i32)
        })
        .collect();
    let wax = |t: (i32, i32)| maze.wax.contains(&((t.1 * cols + t.0) as u16)) || maze.holy.contains(&((t.1 * cols + t.0) as u16));
    if thumb == Thumb::Careless {
        danger.clear();
    }
    let next = path_step(room, here, |t| prey.contains(&t), &danger)
        .filter(|_| !prey.is_empty())
        .or_else(|| path_step(room, here, wax, &danger))
        .or_else(|| path_step(room, here, wax, &BTreeSet::new()));
    let Some(next) = next else {
        return Input::default();
    };
    let (cx, cy) = ((next.0 as f32 + 0.5) * TILE_UNITS, (next.1 as f32 + 0.5) * TILE_UNITS);
    let mut input = Input::default();
    match thumb {
        Thumb::Bot => {
            let (dx, dy) = (cx - hero.x, cy - hero.y);
            input.move_x = if dx.abs() > 0.2 { dx.signum() as i8 } else { 0 };
            input.move_y = if dy.abs() > 0.2 { dy.signum() as i8 } else { 0 };
        }
        Thumb::Human | Thumb::Rowdy | Thumb::Careless => {
            // The way to the next tile, one key: pressed as soon as the
            // knight is within half a tile of the turn.
            let want = ((next.0 - here.0).signum() as i8, (next.1 - here.1).signum() as i8);
            let (hx, hy) = ((here.0 as f32 + 0.5) * TILE_UNITS, (here.1 as f32 + 0.5) * TILE_UNITS);
            let along_x = held.key.0 != 0;
            let turning = (want.0 != 0) != along_x || held.key == (0, 0);
            if turning {
                // A person turns a little early: when the knight is within
                // a third of a tile of the middle of the turn's tile.
                let off = if along_x { (hero.x - hx).abs() } else { (hero.y - hy).abs() };
                if held.key == (0, 0) || off < 0.7 * TILE_UNITS / 2.0 * 2.0 / 3.0 + 0.2 || want == (-held.key.0, -held.key.1) {
                    held.key = want;
                }
            } else {
                held.key = want;
            }
            (input.move_x, input.move_y) = held.key;
            if thumb == Thumb::Rowdy {
                input.fire = tick % 9 < 3;
                input.aim_x = held.key.0;
                input.aim_y = held.key.1;
                input.bomb = tick % 97 == 0;
                input.cast = if tick % 131 == 0 { 1 } else { 0 };
                input.swing = tick % 53 == 0;
                input.dash = tick % 71 == 0;
            }
        }
        Thumb::Idle => {}
    }
    input
}

/// Play the floor's maze through to its end with `thumbs` driving each
/// knight; `shots` names moments to draw.
pub(super) fn play(knights: u32, seed: u64, thumbs: &[Thumb], mut shot: impl FnMut(&Run, &str)) -> Report {
    let (mut run, index) = party(knights, seed);
    let gold0: BTreeMap<u32, i64> = run.players.iter().map(|(&id, h)| (id, i64::from(h.carried.get(Spoil::Gold)))).collect();
    run.enter_for_test(index);
    let mut report = Report::default();
    let Some(maze) = run.folly.clone() else {
        report.found = "NO MAZE".into();
        return report;
    };
    let room = run.room().clone();
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    report.size = (cols, rows);
    report.wax_start = maze.wax.len() + maze.holy.len();
    let start = tile_of(run.players.values().find(|h| !h.absent).unwrap());
    let reached = reach(&room, start);
    report.unreachable_wax = maze
        .wax
        .iter()
        .chain(maze.holy.iter().copied())
        .filter(|&i| !reached.contains(&(i as i32 % cols, i as i32 / cols)))
        .count();
    report.unreachable_floor = (0..rows)
        .flat_map(|y| (0..cols).map(move |x| (x, y)))
        .filter(|&t| open(&room, t) && !reached.contains(&t))
        .count();
    shot(&run, "title");
    let mut held: BTreeMap<u32, Held> = BTreeMap::new();
    let mut still: BTreeMap<u32, u32> = BTreeMap::new();
    let mut wraith_still = vec![0u32; maze.wraiths.len()];
    let mut was_dir: Vec<(i32, i32)> = vec![(0, 0); maze.wraiths.len()];
    let mut shots_taken = 0;
    let limit = folly::TITLE + folly::MAZE_TIME + 3 * HZ;
    for t in 0..limit {
        let inputs: BTreeMap<u32, Input> = run
            .players
            .keys()
            .copied()
            .filter(|id| (*id as usize) <= thumbs.len())
            .map(|id| {
                let h = held.entry(id).or_default();
                (id, drive(&run, id, thumbs[id as usize - 1], h, t))
            })
            .collect();
        let before: BTreeMap<u32, (f32, f32)> = run.players.iter().map(|(&id, h)| (id, (h.x, h.y))).collect();
        let spooked0 = run.folly.as_ref().map(|m| m.spooked.clone()).unwrap_or_default();
        let maze0 = run.folly.clone();
        let wraiths0 = run.folly.as_ref().map(|m| m.wraiths.clone()).unwrap_or_default();
        run.step(&inputs);
        let Some(maze) = run.folly.clone() else {
            break;
        };
        // Knights: snagged (a key held, going nowhere), in stone.
        for (&id, h) in &run.players {
            if h.hp == 0 || h.stone {
                continue;
            }
            if !open(run.room(), tile_of(h)) && !matches!(run.room().tile(tile_of(h).0, tile_of(h).1), Tile::Door) {
                report.knight_in_stone += 1;
            }
            let pressed = inputs.get(&id).is_some_and(|i| i.move_x != 0 || i.move_y != 0);
            let moved = before.get(&id).is_some_and(|&(x, y)| (x - h.x).abs() + (y - h.y).abs() > 0.01);
            if pressed && !moved && !maze.titled() && !maze.done {
                *report.snag_ticks.entry(id).or_default() += 1;
                let s = still.entry(id).or_default();
                *s += 1;
                if *s == 6 {
                    *report.snags.entry(id).or_default() += 1;
                }
            } else {
                still.insert(id, 0);
            }
        }
        // Touches and catches.
        for (&id, &until) in &maze.spooked {
            if spooked0.get(&id) != Some(&until) {
                *report.touched.entry(id).or_default() += 1;
                if let Some(&prev) = spooked0.get(&id)
                    && maze.t < prev + 2 * HZ
                {
                    report.door_recatch += 1;
                }
            }
        }
        for (k, w) in maze.wraiths.iter().enumerate() {
            let w0 = &wraiths0[k];
            if let Some(by) = w_caught(&maze, w0, w) {
                *report.caught.entry(by).or_default() += 1;
            }
            if !w_out(w) || maze.titled() || maze.done {
                wraith_still[k] = 0;
                continue;
            }
            let (wx, wy) = w_pos(&maze, w);
            if !w_home_bound(&maze, w) && !maze.pale_now(w) && !maze.scatter() {
                let near = run
                    .players
                    .values()
                    .filter(|h| h.hp > 0 && !h.stone)
                    .map(|h| (h.x / TILE_UNITS - wx).hypot(h.y / TILE_UNITS - wy))
                    .fold(f32::MAX, f32::min);
                if near < f32::MAX {
                    report.hunt[k].0 += near;
                    report.hunt[k].1 += 1;
                }
            }
            if !open(run.room(), (wx.floor() as i32, wy.floor() as i32)) && !w_home_bound(&maze, w) {
                report.wraith_in_stone += 1;
            }
            if w.at == w0.at && w.to == w0.to && w.step == w0.step {
                wraith_still[k] += 1;
                if wraith_still[k] == HZ {
                    report.wraith_stuck += 1;
                    if std::env::var_os("WAX_DEBUG").is_some() {
                        println!("stuck t{} k{k} {:?} others {:?}", maze.t, w, maze.wraiths.iter().map(|o| (o.temper, o.at, o.to, o.step, o.rest, o.eyes)).collect::<Vec<_>>());
                    }
                }
            } else {
                wraith_still[k] = 0;
            }
            let dir = (w.to.0 - w.at.0, w.to.1 - w.at.1);
            if dir != was_dir[k] && dir == (-was_dir[k].0, -was_dir[k].1) && dir != (0, 0) && !maze0.as_ref().is_some_and(|b| pale_turned(&maze, b)) {
                report.wraith_reversals += 1;
            }
            if dir != (0, 0) {
                was_dir[k] = dir;
            }
            for (j, o) in maze.wraiths.iter().enumerate().skip(k + 1) {
                let (ox, oy) = w_pos(&maze, o);
                if w_out(o) && !w_home_bound(&maze, o) && !w_home_bound(&maze, w) && (ox - wx).hypot(oy - wy) < 0.5 {
                    let _ = j;
                    report.wraith_stacked += 1;
                    if std::env::var_os("WAX_DEBUG").is_some() {
                        println!("stack t{} {:?} pale{} | {:?} pale{}", maze.t, w, maze.pale_now(w), o, maze.pale_now(o));
                    }
                }
            }
        }
        let moment = match shots_taken {
            0 if maze.t == folly::TITLE + 4 * HZ => Some("play"),
            1 if maze.pale > 0 && maze.pale < folly::PALE - HZ => Some("pale"),
            2 if maze.pale > 0 && maze.pale < PALE_WARN => Some("pale-ending"),
            3 if maze.t >= folly::TITLE + 30 * HZ => Some("late"),
            _ => None,
        };
        if let Some(name) = moment {
            shot(&run, name);
            shots_taken += 1;
        }
        if maze.done {
            report.ended_at = Some(maze.t.saturating_sub(folly::TITLE));
            report.wax_left = maze.wax.len();
            report.eaten = maze.eaten.clone();
            shot(&run, "end");
            break;
        }
        report.wax_left = maze.wax.len();
        report.eaten = maze.eaten.clone();
    }
    report.found = run.found.as_ref().map(|f| f.2.clone()).unwrap_or_default();
    report.gold = run
        .players
        .iter()
        .map(|(&id, h)| (id, i64::from(h.carried.get(Spoil::Gold)) - gold0.get(&id).copied().unwrap_or(0)))
        .collect();
    report.laurel = run.players.iter().filter(|(_, h)| h.laurel).map(|(&id, _)| id).collect();
    report.fallen = run.players.values().filter(|h| h.hp == 0).count();
    report
}

fn shots_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("ANGEL_ARENA_SHOTS").map(|d| std::path::PathBuf::from(d).join("wax-maze"))
}

pub(super) fn save_seat(dir: &std::path::Path, name: &str, run: &Run, id: u32) {
    use crate::render::overworld::arena;
    let (w, h) = (arena::NATIVE_W, arena::NATIVE_H + arena::BAND_H);
    let img = arena::seat_scene(run, w, h, Some(id)).compose();
    let k = 3;
    let rgba = img.rgba_scaled(k);
    image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
        .unwrap()
        .save(dir.join(format!("{name}.png")))
        .unwrap();
}

#[test]
#[ignore]
fn wax_maze_sim() {
    let dir = shots_dir();
    if let Some(dir) = &dir {
        std::fs::create_dir_all(dir).unwrap();
    }
    use Thumb::*;
    let cases: &[(u32, &[Thumb])] = &[
        (1, &[Bot]),
        (1, &[Human]),
        (2, &[Bot, Human]),
        (2, &[Human, Rowdy]),
        (3, &[Bot, Human, Rowdy]),
        (3, &[Human, Idle, Human]),
        (4, &[Bot, Human, Rowdy, Bot]),
        (4, &[Human, Human, Human, Human]),
        (1, &[Careless]),
        (2, &[Careless, Careless]),
        (4, &[Careless, Careless, Careless, Careless]),
    ];
    for &(knights, thumbs) in cases {
        for seed in [41u64, 7] {
            let tag = format!("{knights}k-{}-s{seed}", thumbs.iter().map(|t| format!("{t:?}").to_lowercase()).collect::<Vec<_>>().join("-"));
            let report = play(knights, seed, thumbs, |run, moment| {
                if let Some(dir) = &dir
                    && seed == 41
                {
                    for id in 1..=knights.min(2) {
                        save_seat(dir, &format!("{tag}-{moment}-seat{id}"), run, id);
                    }
                }
            });
            println!("{}", report.line(knights, thumbs));
        }
    }
}

/// One knight's way through the maze, a line every third of a second:
/// `WAX_K` knights, `WAX_SEED`, knight `WAX_WHO`.
#[test]
#[ignore]
fn wax_maze_trace() {
    use Thumb::*;
    let knights = std::env::var("WAX_K").ok().and_then(|v| v.parse().ok()).unwrap_or(4u32);
    let seed = std::env::var("WAX_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(7u64);
    let who = std::env::var("WAX_WHO").ok().and_then(|v| v.parse().ok()).unwrap_or(2u32);
    let (mut run, index) = party(knights, seed);
    run.enter_for_test(index);
    let thumbs = [Human; 4];
    let mut held: BTreeMap<u32, Held> = BTreeMap::new();
    for t in 0..folly::TITLE + 20 * HZ {
        let inputs: BTreeMap<u32, Input> = run
            .players
            .keys()
            .copied()
            .map(|id| {
                let h = held.entry(id).or_default();
                (id, drive(&run, id, thumbs[id as usize - 1], h, t))
            })
            .collect();
        run.step(&inputs);
        if t % 10 == 0 {
            let h = &run.players[&who];
            let m = run.folly.as_ref().unwrap();
            println!("t{t} pos ({:.2},{:.2}) tile {:?} in {:?} way {:?} eaten {:?}", h.x, h.y, tile_of(h), (inputs[&who].move_x, inputs[&who].move_y), m.ways.get(&who), m.eaten.get(&who));
        }
    }
}
