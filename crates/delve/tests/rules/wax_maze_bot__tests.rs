//! The Wax Maze's bots, for the proof reels and the frame-time bench: four
//! knights who eat the nearest wax, drink the holy water when a wraith
//! closes (every other seat, so the water is shared) and chase the pale
//! wraiths. They press only what a player can.
use super::*;
use crate::rules::folly::WaxMaze;
use std::collections::{HashMap, VecDeque};

pub(crate) type Keys = BTreeMap<u32, Input>;

fn tile(x: f32, y: f32) -> (i32, i32) {
    ((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32)
}

/// Breadth-first over the lanes from `from`: the first step toward the
/// nearest tile `goal` accepts, and its distance.
fn first_step(run: &Run, from: (i32, i32), goal: impl Fn((i32, i32)) -> bool) -> Option<((i32, i32), u32)> {
    let room = run.room();
    let mut seen: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut queue = VecDeque::from([(from, 0u32)]);
    seen.insert(from, from);
    while let Some((at, d)) = queue.pop_front() {
        if at != from && goal(at) {
            let mut step = at;
            while seen[&step] != from {
                step = seen[&step];
            }
            return Some((step, d));
        }
        for (dx, dy) in [(0, -1), (-1, 0), (0, 1), (1, 0)] {
            let next = (at.0 + dx, at.1 + dy);
            if room.tile(next.0, next.1) == Tile::Floor && !seen.contains_key(&next) {
                seen.insert(next, at);
                queue.push_back((next, d + 1));
            }
        }
    }
    None
}

/// What a bot presses this tick: the pale wraith near, the holy water when a
/// calm wraith closes on them (every other seat, so the water is shared), or
/// the nearest wax.
fn bot(run: &Run, maze: &WaxMaze, id: u32) -> Input {
    let Some(hero) = run.players.get(&id) else { return Input::default() };
    let me = tile(hero.x, hero.y);
    let near = |w: &crate::rules::folly::Wraith| {
        let (x, y) = w.pos();
        (x - hero.x / TILE_UNITS).hypot(y - hero.y / TILE_UNITS)
    };
    let pale: Vec<(i32, i32)> = maze.wraiths.iter().filter(|w| w.roaming() && maze.pale_now(w)).map(|w| (w.pos().0.round() as i32, w.pos().1.round() as i32)).collect();
    let threatened = maze.wraiths.iter().any(|w| w.roaming() && !maze.pale_now(w) && !w.eyes && near(w) < 9.0);
    let holy = &maze.holy;
    let cols = run.room().cols as i32;
    let step = if let Some((s, _)) = (!pale.is_empty()).then(|| first_step(run, me, |t| pale.iter().any(|&p| (p.0 - t.0).abs() + (p.1 - t.1).abs() <= 1))).flatten().filter(|&(_, d)| d < 16) {
        Some(s)
    } else if threatened && id % 2 == 1 && !holy.is_empty() {
        first_step(run, me, |t| holy.contains(&((t.1 * cols + t.0) as u16))).map(|(s, _)| s)
    } else {
        first_step(run, me, |t| maze.wax.contains(&((t.1 * cols + t.0) as u16))).map(|(s, _)| s)
    };
    match step {
        Some((sx, sy)) => Input { move_x: (sx - me.0) as i8, move_y: (sy - me.1) as i8, ..Default::default() },
        None => Input::default(),
    }
}

pub(crate) fn fresh() -> Run {
    fresh_for(4)
}

/// The maze with `seats` knights at the table.
pub(crate) fn fresh_for(seats: u32) -> Run {
    let mut r = Run::new(41, 1, None);
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.enemies.clear();
    for (id, name) in [(1, "Aldric"), (2, "Brienne"), (3, "Cedric"), (4, "Dara")] {
        if let Some(h) = r.players.get_mut(&id) {
            h.name = name.into();
        }
    }
    r.pick_folly();
    for h in r.players.values_mut() {
        h.carried.add(Spoil::Gold, 50);
    }
    r.enter_for_test(r.folly_room.expect("a folly room"));
    r
}

pub(crate) fn keys(run: &Run) -> Keys {
    let Some(maze) = run.folly.as_ref() else { return Keys::new() };
    if maze.titled() || maze.done {
        return Keys::new();
    }
    run.players.keys().map(|&id| (id, bot(run, maze, id))).collect()
}

