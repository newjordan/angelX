//! Bounded routing shared by research maps and the lower tunnels.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

/// Includes both endpoints. Zero costs and invalid edges are skipped; oversize
/// maps fail closed. Stable heap ties make a seeded world replay the same walk.
pub(crate) fn shortest_path(
    count: usize,
    start: usize,
    goal: usize,
    mut neighbours: impl FnMut(usize) -> Vec<(usize, u32)>,
) -> Option<Vec<usize>> {
    if count == 0 || count > 262_144 || start >= count || goal >= count {
        return None;
    }
    let mut distances = vec![u32::MAX; count];
    let mut previous = vec![usize::MAX; count];
    let mut queue = BinaryHeap::from([Reverse((0, 0usize, start))]);
    let mut order = 0usize;
    distances[start] = 0;
    while let Some(Reverse((distance, _, at))) = queue.pop() {
        if distance != distances[at] {
            continue;
        }
        if at == goal {
            let mut path = vec![goal];
            let mut cursor = goal;
            while cursor != start {
                cursor = previous[cursor];
                path.push(cursor);
            }
            path.reverse();
            return Some(path);
        }
        for (next, cost) in neighbours(at) {
            if next >= count || cost == 0 {
                continue;
            }
            let Some(candidate) = distance.checked_add(cost) else {
                continue;
            };
            if candidate < distances[next] {
                distances[next] = candidate;
                previous[next] = at;
                order += 1;
                queue.push(Reverse((candidate, order, next)));
            }
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TunnelVariant {
    Gallery,
    Warrens,
    Karst,
}

impl TunnelVariant {
    pub(crate) fn for_depth(depth: u32) -> Self {
        match depth.saturating_sub(3) % 3 {
            0 => Self::Gallery,
            1 => Self::Warrens,
            _ => Self::Karst,
        }
    }
}

/// Grow a connected, finite floor. A frontier is consumed on every pass;
/// unlike rejection sampling this terminates even when the grid fills.
/// The seed affects geography only, never learned reward or player power.
pub(crate) fn tunnel_cells(
    variant: TunnelVariant,
    seed: u64,
    width: i32,
    requested: usize,
) -> Vec<(i32, i32)> {
    if !(1..=16).contains(&width) || requested == 0 {
        return Vec::new();
    }
    let want = requested.min((width * width) as usize);
    let mut cells = vec![(width / 2, width / 2)];
    let mut occupied = BTreeSet::from([cells[0]]);
    let mut random = seed | 1;
    while cells.len() < want {
        let mut frontier = BTreeSet::new();
        for &(x, y) in &cells {
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let next = (x + dx, y + dy);
                if (0..width).contains(&next.0)
                    && (0..width).contains(&next.1)
                    && !occupied.contains(&next)
                {
                    frontier.insert(next);
                }
            }
        }
        let mut frontier: Vec<_> = frontier.into_iter().collect();
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        let degree = |&(x, y): &(i32, i32)| {
            [(0, -1), (1, 0), (0, 1), (-1, 0)]
                .into_iter()
                .filter(|&(dx, dy)| occupied.contains(&(x + dx, y + dy)))
                .count()
        };
        match variant {
            TunnelVariant::Gallery => {
                // Favor a long gallery with spurs, avoiding early loops.
                let last = *cells.last().unwrap();
                frontier.sort_by_key(|cell| {
                    (
                        degree(cell),
                        (cell.0 - last.0).abs() + (cell.1 - last.1).abs(),
                        cell.1,
                    )
                });
                let best = (
                    degree(&frontier[0]),
                    (frontier[0].0 - last.0).abs() + (frontier[0].1 - last.1).abs(),
                );
                frontier.retain(|cell| {
                    (
                        degree(cell),
                        (cell.0 - last.0).abs() + (cell.1 - last.1).abs(),
                    ) == best
                });
            }
            TunnelVariant::Warrens => {}
            TunnelVariant::Karst => {
                // Compact chambers develop alternate routes around pockets.
                let best = frontier.iter().map(degree).max().unwrap();
                frontier.retain(|cell| degree(cell) == best);
            }
        }
        let cell = frontier[(random % frontier.len() as u64) as usize];
        occupied.insert(cell);
        cells.push(cell);
    }
    cells
}
