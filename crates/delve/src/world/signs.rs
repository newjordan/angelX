//! Fingerposts: a signpost on the verge at every road crossing of a biome
//! border and at every junction of the kingdom's roads, its arms naming
//! what lies each way. The arms are worked out from the road graph: down
//! each road that leaves the post, the first other biome within
//! [`REACH`] road tiles names the arm, else the nearest place that way.
//! Fixed for the realm, worked out once.

use super::biome;
use super::kingdom::{self, Geography};
use super::map::{MAP_H, MAP_W};
use super::roads::{self, ROADS};
use crate::rules::{RoomKind, Tile};
use std::collections::VecDeque;
use std::sync::OnceLock;

/// Which way an arm points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Way {
    East,
    North,
    West,
    South,
}

/// A fingerpost: the tile it stands on, the road tile it marks, and its
/// arms.
#[derive(Clone, Debug, PartialEq)]
pub struct Post {
    pub at: (i32, i32),
    pub road: (i32, i32),
    pub arms: Vec<(Way, String)>,
}

/// No two posts nearer than this, in tiles.
pub const SPACING: i32 = 10;
/// How far down a road an arm looks for another biome, in road tiles.
pub const REACH: u32 = 60;
/// How near (tiles) a knight comes before the arms read.
pub const READ: f32 = 6.0;
/// How far out from the post the roads are told apart, in tiles.
const FORK: i32 = 4;

/// Every fingerpost in the realm.
pub fn posts() -> &'static [Post] {
    static POSTS: OnceLock<Vec<Post>> = OnceLock::new();
    POSTS.get_or_init(|| build(&kingdom::geography(&[])))
}

fn idx((x, y): (i32, i32)) -> usize {
    (y * MAP_W + x) as usize
}

fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

fn build(geo: &Geography) -> Vec<Post> {
    let courses: Vec<Vec<(i32, i32)>> = ROADS.iter().map(|r| roads::course(r.points)).collect();
    // Where to put a post: each road tile where the road crosses into
    // another biome, and each road's end that meets another road; with
    // the way the road runs there.
    let mut marks: Vec<((i32, i32), (i32, i32), bool)> = Vec::new();
    for course in &courses {
        for w in course.windows(2) {
            if biome::at(w[0].0, w[0].1) != biome::at(w[1].0, w[1].1) {
                marks.push((w[1], (w[1].0 - w[0].0, w[1].1 - w[0].1), true));
            }
        }
    }
    for (i, course) in courses.iter().enumerate() {
        let n = course.len();
        if n < 2 {
            continue;
        }
        for (end, inner) in [(course[0], course[1.min(n - 1)]), (course[n - 1], course[n.saturating_sub(2)])] {
            let joins = courses
                .iter()
                .enumerate()
                .any(|(j, other)| j != i && other.iter().any(|&t| chebyshev(t, end) <= 2));
            if joins {
                marks.push((end, (end.0 - inner.0, end.1 - inner.1), false));
            }
        }
    }
    let road = |t: (i32, i32)| t.0 > 0 && t.1 > 0 && t.0 < MAP_W - 1 && t.1 < MAP_H - 1 && kingdom::is_road(geo, t);
    let mut posts: Vec<Post> = Vec::new();
    for (mark, (dx, dy), crossing) in marks {
        if posts.iter().any(|p| chebyshev(p.road, mark) < SPACING) {
            continue;
        }
        // On the verge beside the road: above or below a road running
        // across, left or right of one running down, as near as the
        // ground allows.
        let across = dx.abs() >= dy.abs();
        let mut sides: Vec<(i32, i32)> = (-4..=4)
            .flat_map(|a: i32| (2..=4).flat_map(move |d: i32| [(a, -d), (a, d)]))
            .map(|(a, d)| if across { (a, d) } else { (d, a) })
            .collect();
        sides.sort_by_key(|&(ox, oy)| if across { (oy.abs(), ox.abs(), oy) } else { (ox.abs(), oy.abs(), -ox) });
        let spot = sides.iter().map(|&(ox, oy)| (mark.0 + ox, mark.1 + oy)).find(|&t| {
            t.0 > 1
                && t.1 > 1
                && t.0 < MAP_W - 2
                && t.1 < MAP_H - 2
                && geo.room.tile(t.0, t.1) == Tile::Floor
                && !road(t)
                && geo.sites.iter().all(|s| chebyshev(s.stand, t) > 2 && s.lot.is_none_or(|l| !l.contains(t.0, t.1)))
        });
        let Some(at) = spot else {
            continue;
        };
        let arms = arms(geo, mark, crossing, &road);
        if arms.len() >= 2 {
            posts.push(Post { at, road: mark, arms });
        }
    }
    posts
}

/// What each road leaving `from` leads to. At a biome crossing each arm
/// names the biome its road runs into; elsewhere, the first other biome
/// down that road within [`REACH`], else the nearest place it reaches
/// (the March's castles go by their biome's name).
fn arms(geo: &Geography, from: (i32, i32), crossing: bool, road: &impl Fn((i32, i32)) -> bool) -> Vec<(Way, String)> {
    let own = biome::at(from.0, from.1);
    let n = (MAP_W * MAP_H) as usize;
    let mut dist = vec![u32::MAX; n];
    let mut way: Vec<Option<Way>> = vec![None; n];
    // Where each road leaves the ring round the post, and into which biome.
    let mut ring: Vec<(Way, u32, biome::Biome)> = Vec::new();
    let mut queue = VecDeque::from([from]);
    dist[idx(from)] = 0;
    while let Some(at) = queue.pop_front() {
        for next in [(at.0 + 1, at.1), (at.0 - 1, at.1), (at.0, at.1 + 1), (at.0, at.1 - 1)] {
            if !road(next) || dist[idx(next)] != u32::MAX {
                continue;
            }
            dist[idx(next)] = dist[idx(at)] + 1;
            way[idx(next)] = way[idx(at)].or_else(|| {
                (chebyshev(next, from) >= FORK).then(|| {
                    let (dx, dy) = (next.0 - from.0, next.1 - from.1);
                    let w = if dx.abs() >= dy.abs() {
                        if dx > 0 { Way::East } else { Way::West }
                    } else if dy > 0 {
                        Way::South
                    } else {
                        Way::North
                    };
                    ring.push((w, dist[idx(next)], biome::at(next.0, next.1)));
                    w
                })
            });
            queue.push_back(next);
        }
    }
    let mut out: Vec<(Way, String, u32)> = Vec::new();
    for w in [Way::East, Way::North, Way::West, Way::South] {
        let entered = ring.iter().filter(|r| r.0 == w).min_by_key(|r| r.1).map(|r| (r.1, r.2.name()));
        let mut biome_hit: Option<(u32, &str)> = None;
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let i = idx((x, y));
                if way[i] != Some(w) || dist[i] > REACH {
                    continue;
                }
                let b = biome::at(x, y);
                if b != own && biome_hit.is_none_or(|(d, _)| dist[i] < d) {
                    biome_hit = Some((dist[i], b.name()));
                }
            }
        }
        let place = geo
            .sites
            .iter()
            .filter(|s| s.stand.0 >= 0 && s.stand.1 >= 0 && s.stand.0 < MAP_W && s.stand.1 < MAP_H)
            .filter(|s| biome::at(s.stand.0, s.stand.1) != biome::Biome::March)
            .filter(|s| way[idx(s.stand)] == Some(w))
            .min_by_key(|s| dist[idx(s.stand)])
            .map(|s| {
                let name = if s.room == RoomKind::Gate { "The Delve" } else { s.name.as_str() };
                (dist[idx(s.stand)], name)
            });
        let pick = if crossing { entered.or(biome_hit).or(place) } else { biome_hit.or(place) };
        if let Some((d, name)) = pick {
            if let Some(same) = out.iter_mut().find(|(_, n, _)| n == name) {
                if d < same.2 {
                    *same = (w, name.to_string(), d);
                }
            } else {
                out.push((w, name.to_string(), d));
            }
        }
    }
    out.sort_by_key(|(w, _, _)| *w);
    out.into_iter().map(|(w, name, _)| (w, name)).collect()
}

#[cfg(test)]
#[path = "../../tests/world/signs__tests.rs"]
mod tests;
