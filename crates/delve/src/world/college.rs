//! Where Labyrinth College stands: its lot beside the Delve's gate, the
//! door before it, and its short approach to the road. The hall itself is
//! drawn by `render::overworld::college`.

use super::wishes::{self, Lot};
use std::sync::OnceLock;

/// The College's ground, west of the Delve's gate above the North Road.
pub const LOT: Lot = Lot {
    tx: 80,
    ty: 32,
    tw: 8,
    th: 4,
};

pub fn lot() -> Lot {
    LOT
}

pub fn stand() -> (i32, i32) {
    let l = lot();
    (l.tx + 2, l.ty + l.th)
}

/// A short free approach through the cleared woods, with planks over streams.
/// It joins the existing road and avoids every authored building and gate.
pub fn approach() -> &'static [(i32, i32)] {
    use super::map::{MAP_H, MAP_W, Realm, STRUCTURES, place_tile};
    use std::collections::{BTreeMap, VecDeque};
    static PATH: OnceLock<Vec<(i32, i32)>> = OnceLock::new();
    PATH.get_or_init(|| {
        let realm = Realm::get();
        let lot = lot();
        let gate = wishes::delve_gate();
        let start = stand();
        let obstacles: Vec<_> = STRUCTURES
            .iter()
            .map(|&(x, y, w, h)| {
                let (x, y) = place_tile(x, y);
                Lot {
                    tx: x,
                    ty: y,
                    tw: w,
                    th: h,
                }
            })
            .collect();
        let mut queue = VecDeque::from([start]);
        let mut parents = BTreeMap::from([(start, start)]);
        let mut goal = None;
        while let Some(tile) = queue.pop_front() {
            if matches!(realm.at(tile.0, tile.1), b'=' | b':' | b'H') {
                goal = Some(tile);
                break;
            }
            for next in [
                (tile.0 - 1, tile.1),
                (tile.0, tile.1 - 1),
                (tile.0 + 1, tile.1),
                (tile.0, tile.1 + 1),
            ] {
                if next.0 < 0
                    || next.1 < 0
                    || next.0 >= MAP_W
                    || next.1 >= MAP_H
                    || next.0.abs_diff(start.0) + next.1.abs_diff(start.1) > 20
                    || lot.contains(next.0, next.1)
                    || gate.contains(next.0, next.1)
                    || obstacles.iter().any(|l| l.contains(next.0, next.1))
                    || matches!(realm.at(next.0, next.1), b'L' | b'F')
                    || parents.contains_key(&next)
                {
                    continue;
                }
                parents.insert(next, tile);
                queue.push_back(next);
            }
        }
        let mut out = Vec::new();
        if let Some(mut tile) = goal {
            out.push(tile);
            while tile != start {
                tile = parents[&tile];
                out.push(tile);
            }
            out.reverse();
        }
        out
    })
}
