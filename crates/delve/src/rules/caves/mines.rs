//! Ore bodies in the living cave, and the mines sunk on them.
//!
//! A body is a blob of rock grown from a rock cell that touches open floor,
//! two in each region, the same for a seed on every machine. Painters ask
//! `vein_at` next round; this round only the rules read it.

use super::world::{self, Cave, Cell, MAP_H, MAP_W};
use super::{blades_cell, exit_cell, grip_cell, spawn_cell, Field};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// Proven cells that make a body worth a mine.
pub const PROVE_AT: u8 = 8;
/// No body cell this close to the spawn, the exit, the boots or a mouth.
pub const CLEAR_OF: i32 = 6;
/// How far a knight's lamp marks a vein cell seen.
pub const SIGHT: i32 = 6;
/// A body is at least this many rock cells.
pub const BODY_MIN: usize = 14;
/// A body is at most this many rock cells.
pub const BODY_MAX: usize = 24;
/// How far a blob may spread from the rock it grew from, so a lamp on the head still sees it.
const GROW: i32 = 4;
/// No body starts within this many cells of the map's edge.
const EDGE: i32 = 4;
/// A region's second body starts at least this many cells from its first.
const SPREAD: i32 = 18;

/// One fixed size for each body, in region order, first then second.
const BODY_SIZES: [usize; 8] = [16, 20, 14, 22, 18, 24, 15, 21];

/// One surface site per body, indexed by `body.index`. Each stands beside the
/// hillside's fieldstone and does not share a two-by-two footprint.
pub const SURFACE_SITE: [(i32, i32); 8] = [
    (147, 96),
    (150, 96),
    (153, 96),
    (147, 92),
    (150, 92),
    (153, 92),
    (147, 88),
    (150, 88),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body {
    pub index: u8,
    pub region: u8,
    pub cells: Vec<(i32, i32)>,
    pub head: (i32, i32),
    pub depth: u8,
}

/// What the realm keeps of a sunk mine. Left out of the home while none are sunk.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SunkMine {
    pub region: u8,
    pub head_x: i32,
    pub head_y: i32,
    pub who: u32,
    pub tick: u64,
    pub claim: String,
}

pub(crate) struct Laid {
    pub(crate) bodies: Vec<Body>,
    pub(crate) at: Vec<Option<u8>>,
}

impl Laid {
    pub(crate) fn vein(&self, x: i32, y: i32) -> Option<u8> {
        let i = cell_index(x, y)?;
        self.at.get(i).copied().flatten()
    }

    pub(crate) fn region_of(&self, index: u8) -> u8 {
        self.bodies.iter().find(|b| b.index == index).map(|b| b.region).unwrap_or(0)
    }
}

fn cache() -> &'static Mutex<Option<(u32, std::sync::Arc<Laid>)>> {
    static CACHE: OnceLock<Mutex<Option<(u32, std::sync::Arc<Laid>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

pub(crate) fn bodies(seed: u32) -> std::sync::Arc<Laid> {
    let mut slot = cache().lock().expect("ore bodies");
    if let Some((kept, laid)) = slot.as_ref() {
        if *kept == seed {
            return laid.clone();
        }
    }
    let laid = std::sync::Arc::new(lay(seed));
    *slot = Some((seed, laid.clone()));
    laid
}

pub fn vein_at(seed: u32, x: i32, y: i32) -> Option<u8> {
    let laid = bodies(seed);
    let i = cell_index(x, y)?;
    laid.at.get(i).copied().flatten()
}

pub fn body(seed: u32, index: u8) -> Option<Body> {
    bodies(seed).bodies.iter().find(|b| b.index == index).cloned()
}

pub fn beside(at: (i32, i32), head: (i32, i32)) -> bool {
    (at.0 - head.0).abs() <= 1 && (at.1 - head.1).abs() <= 1
}

pub fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

pub fn pack_of(region: u8) -> crate::rules::Pack {
    use crate::rules::Pack;
    match region {
        0 => Pack::Cavern,
        1 => Pack::Crypt,
        2 => Pack::Hellforge,
        _ => Pack::Archive,
    }
}

pub fn stone_name(region: u8) -> &'static str {
    match region {
        0 => "Limestone",
        1 => "Flint",
        2 => "Basalt",
        _ => "Freestone",
    }
}

pub fn proven(field: &Field, body: &Body) -> u8 {
    body.cells.iter().filter(|cell| field.proven.binary_search(cell).is_ok()).count() as u8
}

pub fn proven_cell(cell: Cell) -> bool {
    cell.seen() || !cell.wall()
}

fn cell_index(x: i32, y: i32) -> Option<usize> {
    if x < 0 || y < 0 || x >= MAP_W || y >= MAP_H {
        return None;
    }
    Some((y * MAP_W + x) as usize)
}

fn lay(seed: u32) -> Laid {
    let cave = super::base(seed).cave.clone();
    let ban = banned(seed);
    let mut used = vec![false; (MAP_W * MAP_H) as usize];
    let mut bodies = Vec::with_capacity(8);
    for region in 0..4u8 {
        for which in 0..2u8 {
            let index = region * 2 + which;
            let want = BODY_SIZES[index as usize];
            let first = bodies.last().filter(|b: &&Body| b.region == region).map(|b| b.cells[0]);
            let Some(cells) = find_body(seed, &cave, &ban, &used, region, which, want, first) else {
                continue;
            };
            for &(x, y) in &cells {
                if let Some(i) = cell_index(x, y) {
                    used[i] = true;
                }
            }
            let head = head_of(&cave, &cells);
            bodies.push(Body { index, region, cells, head, depth: which + 1 });
        }
    }
    let mut at = vec![None; (MAP_W * MAP_H) as usize];
    for body in &bodies {
        for &(x, y) in &body.cells {
            if let Some(i) = cell_index(x, y) {
                at[i] = Some(body.index);
            }
        }
    }
    Laid { bodies, at }
}

fn banned(seed: u32) -> Vec<bool> {
    let n = (MAP_W * MAP_H) as usize;
    let mut mark = vec![false; n];
    for at in [spawn_cell(seed), exit_cell(seed), grip_cell(seed), blades_cell(seed)] {
        if let Some(i) = cell_index(at.0, at.1) {
            mark[i] = true;
        }
    }
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if world::frost_mouth(x, y, seed) {
                if let Some(i) = cell_index(x, y) {
                    mark[i] = true;
                }
            }
        }
    }
    let src = mark.clone();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            let Some(i) = cell_index(x, y) else { continue };
            if !src[i] {
                continue;
            }
            for dy in -CLEAR_OF..=CLEAR_OF {
                for dx in -CLEAR_OF..=CLEAR_OF {
                    if let Some(j) = cell_index(x + dx, y + dy) {
                        mark[j] = true;
                    }
                }
            }
        }
    }
    mark
}

#[allow(clippy::too_many_arguments)]
fn find_body(
    seed: u32,
    cave: &Cave,
    ban: &[bool],
    used: &[bool],
    region: u8,
    which: u8,
    want: usize,
    first: Option<(i32, i32)>,
) -> Option<Vec<(i32, i32)>> {
    let far = |(x, y): (i32, i32)| first.is_none_or(|(fx, fy)| (x - fx).abs() + (y - fy).abs() >= SPREAD);
    let mut found = Vec::new();
    for y in EDGE..MAP_H - EDGE {
        for x in EDGE..MAP_W - EDGE {
            if eligible(cave, ban, used, region, x, y) && far((x, y)) {
                found.push((x, y));
            }
        }
    }
    if found.is_empty() {
        return None;
    }
    // The seed picks where the body starts; if that rock will not grow one, the scan walks on from there.
    let start = crate::ink::hash(i32::from(region), i32::from(which), seed) as usize % found.len();
    let order = (start..found.len()).chain(0..start).collect::<Vec<_>>();
    for i in order {
        if let Some(cells) = grow(cave, ban, used, region, found[i], want) {
            return Some(cells);
        }
    }
    None
}

fn eligible(cave: &Cave, ban: &[bool], used: &[bool], region: u8, x: i32, y: i32) -> bool {
    let Some(i) = cell_index(x, y) else { return false };
    if ban[i] || used[i] {
        return false;
    }
    if world::region_of(x, y).0 != region {
        return false;
    }
    let Some(cell) = cave.at(x, y) else { return false };
    if !cell.wall() {
        return false;
    }
    // The starting rock's south face is open floor, so the body always has a
    // south head. Other open sides do not qualify a start.
    cave.at(x, y + 1).is_some_and(|n| n.open())
}

fn grow(
    cave: &Cave,
    ban: &[bool],
    used: &[bool],
    region: u8,
    start: (i32, i32),
    want: usize,
) -> Option<Vec<(i32, i32)>> {
    let mut cells = Vec::new();
    let mut seen = vec![false; (MAP_W * MAP_H) as usize];
    let mut q = VecDeque::new();
    if let Some(i) = cell_index(start.0, start.1) {
        seen[i] = true;
        q.push_back(start);
    }
    while let Some((x, y)) = q.pop_front() {
        if cells.len() >= want.min(BODY_MAX) {
            break;
        }
        cells.push((x, y));
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (x + dx, y + dy);
            let Some(i) = cell_index(n.0, n.1) else { continue };
            if seen[i] || ban[i] || used[i] {
                continue;
            }
            if (n.0 - start.0).abs() > GROW || (n.1 - start.1).abs() > GROW {
                continue;
            }
            if world::region_of(n.0, n.1).0 != region {
                continue;
            }
            if !cave.at(n.0, n.1).is_some_and(|c| c.wall()) {
                continue;
            }
            seen[i] = true;
            q.push_back(n);
        }
    }
    (cells.len() >= BODY_MIN).then_some(cells)
}

/// The open floor directly south of the body cell nearest the body's middle
/// that has open floor south of it. Starts are south-facing, so this finds one.
fn head_of(cave: &Cave, cells: &[(i32, i32)]) -> (i32, i32) {
    let n = cells.len() as i32;
    let (sx, sy) = cells.iter().fold((0, 0), |a, c| (a.0 + c.0, a.1 + c.1));
    let mid = (sx / n, sy / n);
    let mut south: Option<(i32, (i32, i32))> = None;
    for &(x, y) in cells {
        let head = (x, y + 1);
        if cells.contains(&head) {
            continue;
        }
        if !cave.at(head.0, head.1).is_some_and(|c| c.open()) {
            continue;
        }
        let score = (x - mid.0) * (x - mid.0) + (y - mid.1) * (y - mid.1);
        if south.is_none_or(|(old, _)| score < old) {
            south = Some((score, head));
        }
    }
    if let Some((_, head)) = south {
        return head;
    }
    let mut best: Option<(i32, i32, i32)> = None;
    for &(x, y) in cells {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let h = (x + dx, y + dy);
            if cells.contains(&h) {
                continue;
            }
            if !cave.at(h.0, h.1).is_some_and(|c| c.open()) {
                continue;
            }
            let score = (h.0 - mid.0) * (h.0 - mid.0) + (h.1 - mid.1) * (h.1 - mid.1);
            let key = (score, h.1, h.0);
            if best.is_none_or(|old| key < old) {
                best = Some((score, h.1, h.0));
            }
        }
    }
    best.map(|(_, y, x)| (x, y)).unwrap_or(cells[0])
}

/// The body's cells, for the map test. Not a painter.
pub fn cells_of(seed: u32) -> Vec<Body> {
    bodies(seed).bodies.clone()
}
