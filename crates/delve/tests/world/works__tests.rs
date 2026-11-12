//! The baked great tower and what is solid about it at every stage of the
//! build: the asset reads and is small, the gate and the way to the Delve and
//! every camp stay open from the first stone to the last, the ditch and the
//! walls close as they are dug and laid.

use super::*;
use crate::world::plan::tabula;

fn tower() -> &'static Castle {
    castle(KEEP).expect("the great tower is baked")
}

#[test]
fn the_baked_asset_is_the_great_tower_and_is_small() {
    let bytes = include_bytes!("../../assets/works/great-tower.castle");
    assert!(bytes.len() < 300 * 1024, "{} bytes", bytes.len());
    let c = tower();
    assert_eq!(c.id, "great-tower");
    assert_eq!(c.stones.len(), 11_266);
    assert_eq!(c.stages.len(), 18);
    assert_eq!(c.falsework.len(), 358);
    assert_eq!(c.earth.len(), 2);
    assert_eq!(c.solid.len(), (GRID * GRID) as usize);
    // Stages cover the stones in order with no gap.
    let mut next = 0;
    for s in &c.stages {
        assert_eq!(s.first, next, "{} begins where the last ended", s.id);
        next += s.count;
    }
    assert_eq!(next, c.len());
    // A stone is a box with its low corner under its high one, or eight corners.
    for (i, s) in c.stones.iter().enumerate() {
        if let Shape::Box(b) = s.shape {
            assert!(b[0] < b[3] && b[1] < b[4] && b[2] < b[5], "stone {i} is {b:?}");
        }
    }
}

#[test]
fn the_asset_bakes_back_to_itself() {
    let c = tower();
    let again = Castle::decode(&c.encode()).expect("reads");
    assert_eq!(&again, c);
    // And a damaged file is refused, not read half.
    let bytes = c.encode();
    assert!(Castle::decode(&bytes[..bytes.len() / 2]).is_err());
    assert!(Castle::decode(b"nope").is_err());
}

/// The checkpoints a test walks the build through: nothing laid, every
/// stage's first and last stone, and the middle of each stage.
fn checkpoints(c: &Castle) -> Vec<u32> {
    let mut out = vec![0];
    for s in c.stages.iter().filter(|s| s.count > 0) {
        out.extend([s.first, s.first + s.count / 2, s.first + s.count]);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Whether tile `(x, y)` is open to a knight with `laid` stones standing: the
/// plan's own idea, less the site's walls and ditch.
fn open(c: &Castle, x: i32, y: i32, laid: u32) -> bool {
    tabula::walkable(x, y) && !c.blocked(x, y, laid)
}

#[test]
fn the_gate_the_spawn_the_delve_and_every_camp_stay_reachable_at_every_stage() {
    use crate::world::map::{MAP_H, MAP_W};
    use std::collections::VecDeque;
    let c = tower();
    let idx = |(x, y): (i32, i32)| (y * MAP_W + x) as usize;
    let mut goals = vec![("the Delve's stand", tabula::delve_stand())];
    goals.extend(tabula::CAMPS.iter().map(|&r| ("a camp", tabula::camp_stand(r))));
    for laid in checkpoints(c) {
        assert!(open(c, tabula::KEEP_STAND.0, tabula::KEEP_STAND.1, laid), "the spawn is shut at {laid}");
        let mut seen = vec![false; (MAP_W * MAP_H) as usize];
        let mut queue = VecDeque::from([tabula::KEEP_STAND]);
        seen[idx(tabula::KEEP_STAND)] = true;
        while let Some((x, y)) = queue.pop_front() {
            for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if next.0 >= 0 && next.1 >= 0 && next.0 < MAP_W && next.1 < MAP_H && open(c, next.0, next.1, laid) && !seen[idx(next)] {
                    seen[idx(next)] = true;
                    queue.push_back(next);
                }
            }
        }
        for (what, g) in &goals {
            assert!(seen[idx(*g)], "{what} {g:?} cannot be walked to from the spawn with {laid} stones laid");
        }
    }
}

#[test]
fn the_gate_passage_and_its_causeway_are_never_shut() {
    let c = tower();
    // The two tiles either side of the castle's middle line, from the bailey
    // out past the ditch to the end of the drawbridge.
    let (ox, oy) = ORIGIN;
    for ty in oy + 8..oy + 14 {
        for tx in [ox - 1, ox] {
            assert_eq!(c.need(tx, ty), NEVER, "{tx},{ty} shuts at {}", c.need(tx, ty));
        }
    }
}

#[test]
fn the_ditch_is_dug_from_the_first_stone_and_is_a_pit_and_the_walls_are_walls() {
    let c = tower();
    let (ox, oy) = ORIGIN;
    // A tile of the ditch's ring on the east side, the middle of its width.
    let ring = (ox + 12, oy);
    assert!(c.blocked(ring.0, ring.1, 0), "the ditch is dug before a stone is laid");
    assert!(c.is_ditch(ring.0, ring.1));
    // Stone stops a knight once laid and not before.
    let wall = (0..GRID).flat_map(|j| (0..GRID).map(move |i| (ox - HALF + i, oy - HALF + j))).find(|&(x, y)| c.need(x, y) != NEVER && !c.is_ditch(x, y) && c.need(x, y) > 0);
    let (wx, wy) = wall.expect("a wall tile");
    let n = c.need(wx, wy);
    assert!(!c.blocked(wx, wy, n - 1) && c.blocked(wx, wy, n));
    // Finished, the keep stands solid and the bailey is not all shut.
    let shut = c.solid.iter().filter(|&&w| w != NEVER).count();
    assert!((400..700).contains(&shut), "{shut} tiles shut of {}", c.solid.len());
    // Off the grid is no one's.
    assert_eq!(c.need(ox + 40, oy), NEVER);
}

#[test]
fn the_site_is_the_plans_keep_and_the_stand_and_the_delve_are_inside_the_bailey() {
    let c = tower();
    let (x, y, w, h) = c.tiles();
    assert_eq!((x, y, w, h), tabula::KEEP);
    let inside = |(tx, ty): (i32, i32)| tx >= x && ty >= y && tx < x + w && ty < y + h;
    assert!(inside(tabula::KEEP_STAND));
    assert!(inside(tabula::delve_stand()));
    // The gate faces south: the spawn stands north of the gate's mouth.
    assert!(tabula::KEEP_STAND.1 < ORIGIN.1 + 11);
}

/// The tiles a knight must be able to stand on and walk to at every stage of
/// the build for the Delve's own fixtures to work: the shaft head's stand at
/// the gate's mouth, the stockyard where a block is set down, and the winch
/// that raises the shaft. Each is on the open bailey and joined to the spawn.
#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::works -- --include-ignored"]
fn the_winch_the_stockyard_and_the_shaft_head_stay_reachable_at_every_stage() {
    use crate::world::map::{MAP_H, MAP_W};
    use std::collections::VecDeque;
    let c = tower();
    let idx = |(x, y): (i32, i32)| (y * MAP_W + x) as usize;
    let tile_of = |(ux, uy): (f32, f32)| ((ux / crate::rules::TILE_UNITS).floor() as i32, (uy / crate::rules::TILE_UNITS).floor() as i32);
    let geo = crate::world::kingdom::geography(&[]);
    let winch = crate::rules::trinkets::placed(&geo)
        .into_iter()
        .find(|(t, _, _)| *t == crate::rules::trinkets::Thing::Winch)
        .map(|(_, _, at)| at)
        .expect("the winch stands by the gate");
    let yard = tile_of(crate::rules::quarrying::yard_spot());
    let stand = tabula::delve_stand();
    // The stockyard's block-setting reach, in tiles, round its spot: some tile
    // within it is open and joined to the spawn, and the spot's own is.
    let goals = [("the shaft head's stand", stand), ("the stockyard", yard), ("the winch", winch)];
    println!("the Delve's stand {stand:?}, the yard {yard:?}, the winch {winch:?}");
    for laid in checkpoints(c) {
        let mut seen = vec![false; (MAP_W * MAP_H) as usize];
        let mut queue = VecDeque::from([tabula::KEEP_STAND]);
        seen[idx(tabula::KEEP_STAND)] = true;
        while let Some((x, y)) = queue.pop_front() {
            for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if next.0 >= 0 && next.1 >= 0 && next.0 < MAP_W && next.1 < MAP_H && open(c, next.0, next.1, laid) && !seen[idx(next)] {
                    seen[idx(next)] = true;
                    queue.push_back(next);
                }
            }
        }
        for (what, g) in &goals {
            assert!(open(c, g.0, g.1, laid), "{what} {g:?} is shut at {laid} stones");
            assert!(seen[idx(*g)], "{what} {g:?} cannot be walked to from the spawn with {laid} stones laid");
        }
    }
}

/// Prints the Keep's ground by when it closes (throwaway, for the hillside lane's siting).
#[test]
#[ignore]
fn print_the_keeps_ground() {
    let c = tower();
    let (x0, y0, w, h) = c.tiles();
    for y in y0..y0 + h {
        let mut row = String::new();
        for x in x0..x0 + w {
            let n = c.need(x, y);
            row.push(if n == NEVER { '.' } else if c.is_ditch(x, y) { 'd' } else if n == 0 { '#' } else if n < 1500 { 'w' } else { 'W' });
        }
        println!("{y:3} {row}");
    }
    println!("x0={x0}");
}
