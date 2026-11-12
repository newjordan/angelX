//! Rollerways and swapes: the continuous line.
//!
//! A rollerway is laid tile by tile, each tile of it a piece with a way it
//! faces, and goods stream along it one slot at a time. Nothing is carried by a
//! vehicle and nothing hops by magic: a tile holds [`SLOTS`] goods, a good moves
//! on to the next slot only when that slot is empty, and the line behind it
//! waits. So a line that cannot unload backs up tile by tile from its end, and a
//! splitter, a priority gate or a second place to unload clears it again.
//!
//! - **Pieces.** The hand-chain (a labourer a tile: slow, free to lay, the first
//!   automation), the roller (log rollers: the plain line), the splitter (a
//!   house that deals goods out to its left, front and right in turn), the merge
//!   (three ways in, one out, each taking its turn), the priority gate (front
//!   first, to the sides only while the front is full) and the trestle (two
//!   lines crossing without mixing).
//! - **Swapes.** A swape is a work, not a piece: the pivot arm that lifts one
//!   good at a time from what stands behind it to what stands in front of it,
//!   facing the way the goods go. Between a bench and a rollerway it loads; between
//!   a rollerway and a bench, a store or the camp it unloads; between two
//!   rollerways it moves goods across; it can also lift straight from one
//!   building to another. A boy works it, or a drive shaft, which is faster.
//!   A swape may be set to one good (`Building::filter`), and an unloading
//!   one takes only what its building will take.
//! - **Time.** The line moves four beats a second; a piece steps on as many of
//!   them as its speed (`PieceSpec::beats`). Real rates are all data in the
//!   catalogue: a roller's is two slots a second, a hand-chain's one.
//!
//! It is all integers, deterministic, and costs a few operations a good a beat:
//! a few hundred tiles and a few thousand goods are nothing to a browser seat.
use crate::economy::{add, covers, take};
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Goods a tile holds, from its entry end (0) to its exit end.
pub const SLOTS: usize = 3;
/// Beats of the line in a sim second.
pub const BEATS: u32 = 4;
/// The most tiles of rollerway a kingdom may lay.
pub const MAX_CELLS: usize = 2_000;
/// Seconds a good at a tile's exit must have been unable to move before the
/// tile is called stuck.
pub const JAM_SECONDS: u16 = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    N,
    #[default]
    E,
    S,
    W,
}
impl Dir {
    pub const ALL: [Dir; 4] = [Dir::N, Dir::E, Dir::S, Dir::W];
    pub fn delta(self) -> (i16, i16) {
        match self {
            Dir::N => (0, -1),
            Dir::E => (1, 0),
            Dir::S => (0, 1),
            Dir::W => (-1, 0),
        }
    }
    pub fn right(self) -> Dir {
        match self {
            Dir::N => Dir::E,
            Dir::E => Dir::S,
            Dir::S => Dir::W,
            Dir::W => Dir::N,
        }
    }
    pub fn left(self) -> Dir {
        self.right().right().right()
    }
    pub fn back(self) -> Dir {
        self.right().right()
    }
    pub fn step(self, (x, y): Tile) -> Tile {
        let (dx, dy) = self.delta();
        (x + dx, y + dy)
    }
    /// 0 north, 1 east, 2 south, 3 west: the way a knight's face is counted.
    pub fn index(self) -> u8 {
        self as u8
    }
    pub fn from_index(i: u8) -> Dir {
        Dir::ALL[usize::from(i % 4)]
    }
    /// The way from `a` to the tile beside it, if it is beside it.
    pub fn between(a: Tile, b: Tile) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| d.step(a) == b)
    }
    pub fn name(self) -> &'static str {
        match self {
            Dir::N => "north",
            Dir::E => "east",
            Dir::S => "south",
            Dir::W => "west",
        }
    }
}

/// What a tile of rollerway is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Piece {
    /// A labourer a tile passing goods hand to hand: slow, and free to lay.
    Chain,
    /// Log rollers: the plain line.
    #[default]
    Roller,
    /// A house that deals goods out to its front, left and right in turn.
    Splitter,
    /// Three ways in (behind, left, right), one way out, each in its turn.
    Merge,
    /// Front first; to the sides only while the front is full.
    Gate,
    /// A low trestle: one line over another. The first runs the way it faces,
    /// the second a quarter turn to the right of it.
    Trestle,
}
impl Piece {
    pub const ALL: [Piece; 6] = [Piece::Chain, Piece::Roller, Piece::Splitter, Piece::Merge, Piece::Gate, Piece::Trestle];
    pub fn key(self) -> &'static str {
        match self {
            Piece::Chain => "chain",
            Piece::Roller => "roller",
            Piece::Splitter => "splitter",
            Piece::Merge => "merge",
            Piece::Gate => "gate",
            Piece::Trestle => "trestle",
        }
    }
    pub fn from_key(key: &str) -> Option<Piece> {
        Piece::ALL.into_iter().find(|p| p.key() == key)
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Never equal, never different: a derived cache that a save need not keep and a
/// comparison need not see.
#[derive(Clone, Debug, Default)]
pub struct Cache<T>(pub T);
impl<T> PartialEq for Cache<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl<T> Eq for Cache<T> {}

/// One tile of rollerway.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cell {
    pub piece: Piece,
    /// The way it faces: the way goods leave it (a splitter's and a gate's front).
    pub dir: Dir,
    /// The goods it holds, entry end first; 0 is a free slot, else a good's number in `Rollers::goods`.
    pub slots: [u8; SLOTS],
    /// A trestle's second line.
    pub across: [u8; SLOTS],
    /// Whose turn it is where goods meet (a splitter's next output, a merge's next input).
    pub rr: u8,
    /// Beats of the last second in which something in it moved, for drawing the stream.
    pub moved: u8,
    /// Seconds the good at its exit has stood, unable to go on.
    pub stuck: u16,
    /// Beats of this second so far in which something in it moved.
    pub pending: u8,
}
impl Default for Cell {
    fn default() -> Self {
        Self { piece: Piece::Roller, dir: Dir::E, slots: [0; SLOTS], across: [0; SLOTS], rr: 0, moved: 0, stuck: 0, pending: 0 }
    }
}
impl Cell {
    pub fn new(piece: Piece, dir: Dir) -> Self {
        Self { piece, dir, ..Self::default() }
    }
    pub fn lanes(&self) -> u8 {
        if self.piece == Piece::Trestle { 2 } else { 1 }
    }
    pub fn lane(&self, lane: u8) -> &[u8; SLOTS] {
        if lane == 0 { &self.slots } else { &self.across }
    }
    pub fn lane_mut(&mut self, lane: u8) -> &mut [u8; SLOTS] {
        if lane == 0 { &mut self.slots } else { &mut self.across }
    }
    /// How many goods it holds.
    pub fn held(&self) -> u32 {
        self.slots.iter().chain(&self.across).filter(|g| **g != 0).count() as u32
    }
    /// How many it can hold.
    pub fn room(&self) -> u32 {
        (SLOTS as u32) * u32::from(self.lanes())
    }
    /// The way goods travelling `travel` would go in, if this piece takes them from that side.
    fn lane_for(&self, travel: Dir) -> Option<u8> {
        match self.piece {
            Piece::Chain | Piece::Roller | Piece::Merge => (travel != self.dir.back()).then_some(0),
            Piece::Splitter | Piece::Gate => (travel == self.dir).then_some(0),
            Piece::Trestle => {
                if travel == self.dir {
                    Some(0)
                } else if travel == self.dir.right() {
                    Some(1)
                } else {
                    None
                }
            }
        }
    }
}

/// Where a lane of the cell at `t` hands its goods, best first: tile and direction of travel.
fn outputs(t: Tile, c: &Cell, lane: u8) -> ([(Tile, Dir); 3], usize) {
    let mut out = [((0, 0), Dir::N); 3];
    let d = if c.piece == Piece::Trestle && lane == 1 { c.dir.right() } else { c.dir };
    out[0] = (d.step(t), d);
    match c.piece {
        Piece::Splitter | Piece::Gate => {
            out[1] = (d.left().step(t), d.left());
            out[2] = (d.right().step(t), d.right());
            (out, 3)
        }
        _ => (out, 1),
    }
}

/// The tiles that can feed a cell facing `d`: behind, then the two sides, with the way goods travel
/// from each into it.
fn feeders(t: Tile, d: Dir) -> [(Tile, Dir); 3] {
    [(d.back().step(t), d), (d.left().step(t), d.right()), (d.right().step(t), d.left())]
}

mod as_list {
    use super::*;
    pub fn serialize<S: serde::Serializer>(m: &BTreeMap<Tile, Cell>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(m.iter())
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<Tile, Cell>, D::Error> {
        let v: Vec<(Tile, Cell)> = Vec::deserialize(d)?;
        Ok(v.into_iter().collect())
    }
}

/// A place where a rollerway's goods stood too long.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jam {
    /// The tile at the head of the jam: the last that cannot move.
    pub tile: Tile,
    pub good: String,
    /// Seconds it has stood.
    pub seconds: u16,
    /// Tiles stood behind it.
    pub queue: usize,
    pub why: String,
    /// Whether the stoppage has reached the start of the line, so that nothing gets on it: a jam. A full
    /// branch that the rest of the line goes round is only that, and does not count as one.
    pub blocking: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollerState {
    /// Nothing on it.
    Empty,
    Flowing,
    /// Full over most of its length and still moving: it is the limit.
    Backed,
    Jammed,
}
impl RollerState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "EMPTY",
            Self::Flowing => "FLOWING",
            Self::Backed => "BACKED UP",
            Self::Jammed => "JAMMED",
        }
    }
}

/// What the player reads off a rollerway to find where it chokes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollerRow {
    /// The first tile of it (the north-west one): its name.
    pub id: Tile,
    pub tiles: usize,
    pub items: u32,
    pub room: u32,
    /// Goods a minute its swapes took off it in the last minute.
    pub per_min: u32,
    pub state: RollerState,
    pub swapes: usize,
    pub jams: Vec<Jam>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rollers {
    #[serde(with = "as_list")]
    pub cells: BTreeMap<Tile, Cell>,
    /// The goods on the line, by number minus one.
    pub goods: Vec<String>,
    /// Goods the swapes have taken off lines since the first was laid.
    pub unloaded: u64,
    #[serde(skip)]
    cache: Cache<Option<Vec<(Tile, u8)>>>,
    /// Seconds and tiles of the lifts of the last minute.
    #[serde(skip)]
    lifts: Cache<VecDeque<(u64, Tile)>>,
}

impl Rollers {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    pub fn good_name(&self, id: u8) -> Option<&str> {
        (id != 0).then(|| self.goods.get(usize::from(id) - 1).map(String::as_str)).flatten()
    }
    fn intern(&mut self, good: &str) -> Option<u8> {
        if let Some(i) = self.goods.iter().position(|g| g == good) {
            return Some(i as u8 + 1);
        }
        if self.goods.len() >= 250 {
            return None;
        }
        self.goods.push(good.to_string());
        Some(self.goods.len() as u8)
    }
    /// Goods of `resource` on the line.
    pub fn holding(&self, resource: &str) -> u32 {
        let Some(i) = self.goods.iter().position(|g| g == resource) else { return 0 };
        let id = i as u8 + 1;
        self.cells.values().flat_map(|c| c.slots.iter().chain(&c.across)).filter(|g| **g == id).count() as u32
    }
    /// Every good on the line, by name.
    pub fn contents(&self) -> BTreeMap<String, u32> {
        let mut out = BTreeMap::new();
        for c in self.cells.values() {
            for &g in c.slots.iter().chain(&c.across) {
                if let Some(name) = self.good_name(g) {
                    *out.entry(name.to_string()).or_default() += 1;
                }
            }
        }
        out
    }
    /// Tiles of hand-chain: a labourer each.
    pub fn chain_tiles(&self) -> usize {
        self.cells.values().filter(|c| c.piece == Piece::Chain).count()
    }
    /// The cache of the order tiles are moved in, to be rebuilt.
    pub(crate) fn touch(&mut self) {
        self.cache.0 = None;
    }

    /// Whether the cell at `target` takes goods travelling `travel` into a lane whose entry slot is free,
    /// and which lane.
    fn open_lane(&self, target: Tile, travel: Dir) -> Option<u8> {
        let c = self.cells.get(&target)?;
        let lane = c.lane_for(travel)?;
        (c.lane(lane)[0] == 0).then_some(lane)
    }
    /// Whether a cell takes goods from that side at all, free or not.
    fn takes(&self, target: Tile, travel: Dir) -> bool {
        self.cells.get(&target).is_some_and(|c| c.lane_for(travel).is_some())
    }
    /// Whether the cell at `from` has a good waiting at an exit that leads into `target` the way `travel`.
    fn offering(&self, from: Tile, target: Tile, travel: Dir) -> bool {
        let Some(f) = self.cells.get(&from) else { return false };
        (0..f.lanes()).any(|l| {
            f.lane(l)[SLOTS - 1] != 0 && {
                let (outs, n) = outputs(from, f, l);
                outs[..n].iter().any(|&(t, d)| t == target && d == travel)
            }
        })
    }

    /// Tiles of every lane, in the order they are moved: downstream first, so that a full line moves
    /// all at once and a blocked one not at all.
    fn compute_order(&self) -> Vec<(Tile, u8)> {
        let edges = |node: (Tile, u8)| -> Vec<(Tile, u8)> {
            let c = &self.cells[&node.0];
            let (outs, n) = outputs(node.0, c, node.1);
            outs[..n]
                .iter()
                .filter_map(|&(t, d)| self.cells.get(&t).and_then(|x| x.lane_for(d)).map(|l| (t, l)))
                .collect()
        };
        let mut seen: BTreeSet<(Tile, u8)> = BTreeSet::new();
        let mut order = Vec::with_capacity(self.cells.len());
        for (&t, c) in &self.cells {
            for l in 0..c.lanes() {
                if !seen.insert((t, l)) {
                    continue;
                }
                let mut stack: Vec<((Tile, u8), Vec<(Tile, u8)>, usize)> = vec![((t, l), edges((t, l)), 0)];
                while let Some((node, next, i)) = stack.last_mut() {
                    if *i < next.len() {
                        let n = next[*i];
                        *i += 1;
                        if seen.insert(n) {
                            let e = edges(n);
                            stack.push((n, e, 0));
                        }
                    } else {
                        order.push(*node);
                        stack.pop();
                    }
                }
            }
        }
        order
    }

    /// Whether the feeder `from` may put its good into `target` now: the cell takes it, and nobody who has
    /// the better claim is waiting.
    fn may_enter(&self, from: Tile, target: Tile, travel: Dir) -> bool {
        let Some(c) = self.cells.get(&target) else { return false };
        match c.piece {
            Piece::Chain | Piece::Roller => {
                if travel == c.dir {
                    return true;
                }
                // A side feeder waits for the line behind, and takes its turn with the other side.
                let f = feeders(target, c.dir);
                if self.offering(f[0].0, target, f[0].1) {
                    return false;
                }
                let other = if from == f[1].0 { f[2] } else { f[1] };
                if self.offering(other.0, target, other.1) {
                    let mine_first = (c.rr % 2 == 0) == (from == f[1].0);
                    return mine_first;
                }
                true
            }
            Piece::Merge => {
                let f = feeders(target, c.dir);
                let ready: Vec<Tile> = f.iter().filter(|(t, d)| self.offering(*t, target, *d)).map(|(t, _)| *t).collect();
                ready.get(usize::from(c.rr) % ready.len().max(1)) == Some(&from)
            }
            _ => true,
        }
    }

    /// The good at the exit of a lane goes on, if it can.
    fn push(&mut self, t: Tile, lane: u8) -> bool {
        let c = &self.cells[&t];
        let (outs, n) = outputs(t, c, lane);
        let piece = c.piece;
        let rr = c.rr;
        // The outputs to try, in order, as indexes; and where the splitter's turn begins.
        let mut live_len = 1usize;
        let mut start = 0usize;
        let order: Vec<usize> = match piece {
            Piece::Splitter => {
                let live: Vec<usize> = (0..n).filter(|&i| self.takes(outs[i].0, outs[i].1)).collect();
                if live.is_empty() {
                    return false;
                }
                live_len = live.len();
                start = usize::from(rr) % live_len;
                (0..live_len).map(|k| live[(start + k) % live_len]).collect()
            }
            Piece::Gate => {
                let (a, b) = if rr % 2 == 0 { (1, 2) } else { (2, 1) };
                vec![0, a, b]
            }
            _ => vec![0],
        };
        for (k, i) in order.into_iter().enumerate() {
            let (target, travel) = outs[i];
            let Some(tl) = self.open_lane(target, travel) else { continue };
            if !self.may_enter(t, target, travel) {
                continue;
            }
            let good = {
                let c = self.cells.get_mut(&t).unwrap();
                std::mem::take(&mut c.lane_mut(lane)[SLOTS - 1])
            };
            {
                let tc = self.cells.get_mut(&target).unwrap();
                // Whose turn it is next where goods meet.
                match tc.piece {
                    Piece::Merge => tc.rr = tc.rr.wrapping_add(1),
                    Piece::Chain | Piece::Roller if travel != tc.dir => tc.rr = tc.rr.wrapping_add(1),
                    _ => {}
                }
                tc.lane_mut(tl)[0] = good;
            }
            let c = self.cells.get_mut(&t).unwrap();
            match piece {
                Piece::Splitter => c.rr = ((start + k + 1) % live_len) as u8,
                Piece::Gate if i != 0 => c.rr = c.rr.wrapping_add(1),
                _ => {}
            }
            return true;
        }
        false
    }

    fn step(&mut self, t: Tile, lane: u8) {
        let mut moved = false;
        if self.cells[&t].lane(lane)[SLOTS - 1] != 0 && self.push(t, lane) {
            moved = true;
        }
        let c = self.cells.get_mut(&t).unwrap();
        let slots = c.lane_mut(lane);
        for i in (0..SLOTS - 1).rev() {
            if slots[i] != 0 && slots[i + 1] == 0 {
                slots[i + 1] = slots[i];
                slots[i] = 0;
                moved = true;
            }
        }
        if moved {
            c.pending = c.pending.saturating_add(1);
        }
    }

    /// One beat of the line. `speed` is the beats a second each piece moves on, `chain_on` whether the
    /// hand-chain's labourers are at work this second.
    pub(crate) fn beat(&mut self, beat: u32, speed: &[u8; 6], chain_on: bool) {
        if self.cells.is_empty() {
            return;
        }
        let order = match self.cache.0.take() {
            Some(o) => o,
            None => self.compute_order(),
        };
        for &(t, lane) in &order {
            let Some(piece) = self.cells.get(&t).map(|c| c.piece) else { continue };
            let b = u32::from(speed[piece.index()]);
            if b == 0 || (beat * b) % BEATS >= b || (piece == Piece::Chain && !chain_on) {
                continue;
            }
            self.step(t, lane);
        }
        self.cache.0 = Some(order);
    }

    /// The end of a second: what moved, what stood.
    pub(crate) fn settle(&mut self) {
        for c in self.cells.values_mut() {
            c.moved = c.pending.min(BEATS as u8);
            c.pending = 0;
            let waiting = c.slots[SLOTS - 1] != 0 || (c.piece == Piece::Trestle && c.across[SLOTS - 1] != 0);
            c.stuck = if waiting && c.moved == 0 { c.stuck.saturating_add(1) } else { 0 };
        }
    }

    fn lifted(&mut self, second: u64, from: Tile) {
        self.unloaded += 1;
        let q = &mut self.lifts.0;
        q.push_back((second, from));
        while q.front().is_some_and(|(s, _)| *s + 60 <= second) {
            q.pop_front();
        }
    }

    /// The good nearest a cell's exit that `want` takes, leaving the slot free.
    fn pick(&mut self, from: Tile, want: impl Fn(&str) -> bool) -> Option<u8> {
        let goods = self.goods.clone();
        let c = self.cells.get_mut(&from)?;
        if c.piece == Piece::Trestle {
            return None;
        }
        for i in (0..SLOTS).rev() {
            let g = c.slots[i];
            if g != 0 && goods.get(usize::from(g) - 1).is_some_and(|n| want(n)) {
                c.slots[i] = 0;
                return Some(g);
            }
        }
        None
    }
    /// The goods in a cell, from the exit end.
    fn goods_at(&self, from: Tile) -> Vec<&str> {
        self.cells
            .get(&from)
            .filter(|c| c.piece != Piece::Trestle)
            .map(|c| c.slots.iter().rev().filter_map(|g| self.good_name(*g)).collect())
            .unwrap_or_default()
    }

    /// Whether some tile of rollerway hands goods into the one at `t`.
    pub fn fed_by_a_tile(&self, t: Tile) -> bool {
        Dir::ALL.into_iter().any(|d| {
            let f = d.back().step(t);
            self.cells.get(&f).is_some_and(|fc| {
                (0..fc.lanes()).any(|l| {
                    let (outs, n) = outputs(f, fc, l);
                    outs[..n].iter().any(|&(o, od)| o == t && od == d && self.takes(o, od))
                })
            })
        })
    }

    /// Settled: the stuck heads of lines and why, with the lines.
    pub fn components(&self) -> Vec<Vec<Tile>> {
        let mut seen: BTreeSet<Tile> = BTreeSet::new();
        let mut out = Vec::new();
        for &t in self.cells.keys() {
            if !seen.insert(t) {
                continue;
            }
            let mut group = vec![t];
            let mut queue = VecDeque::from([t]);
            while let Some(u) = queue.pop_front() {
                let c = &self.cells[&u];
                let mut near: Vec<Tile> = Vec::new();
                for l in 0..c.lanes() {
                    let (outs, n) = outputs(u, c, l);
                    near.extend(outs[..n].iter().filter(|&&(o, d)| self.takes(o, d)).map(|&(o, _)| o));
                }
                // And whoever feeds it.
                for d in Dir::ALL {
                    let f = d.back().step(u);
                    if let Some(fc) = self.cells.get(&f) {
                        for l in 0..fc.lanes() {
                            let (outs, n) = outputs(f, fc, l);
                            if outs[..n].iter().any(|&(o, od)| o == u && od == d) {
                                near.push(f);
                            }
                        }
                    }
                }
                for n in near {
                    if self.cells.contains_key(&n) && seen.insert(n) {
                        group.push(n);
                        queue.push_back(n);
                    }
                }
            }
            group.sort();
            out.push(group);
        }
        out
    }
}

/// How one lift of a swape came out.
enum Lift {
    Done,
    /// Nothing there to lift, or nothing wanted.
    Nothing,
    /// Goods there, and nowhere to put them.
    Blocked,
}

#[derive(Clone, Copy)]
enum End {
    Cell(Tile),
    Place(u32),
    Bare,
}

impl Kingdom {
    /// Whether a building is a swape.
    pub fn is_swape(&self, c: &Catalog, id: u32) -> bool {
        self.buildings.get(&id).is_some_and(|b| c.plans[&b.plan].swape > 0)
    }

    /// What stands at a tile for a swape's purposes: a tile of rollerway, a work, or nothing.
    fn end_of(&self, t: Tile) -> End {
        if self.rollers.cells.get(&t).is_some_and(|c| c.piece != Piece::Trestle) {
            End::Cell(t)
        } else if let Some(id) = self.at(t) {
            End::Place(id)
        } else {
            End::Bare
        }
    }

    /// The two ends of a swape: what it lifts from and what it lifts to.
    pub fn swape_ends(&self, id: u32) -> Option<(Tile, Tile)> {
        let b = self.buildings.get(&id)?;
        let d = Dir::from_index(b.facing);
        Some((d.back().step(b.tile), d.step(b.tile)))
    }

    /// Whether a swape's building at `t` may be lifted from or to.
    fn usable(&self, c: &Catalog, id: u32) -> bool {
        self.buildings.get(&id).is_some_and(|b| b.construction == 0 && !b.paused && c.plans[&b.plan].swape == 0)
    }

    /// Room for one unit of `r` at a building, as a haul reckons it.
    fn room_for(&self, c: &Catalog, id: u32, r: &str, capacity: u32) -> u32 {
        if !self.usable(c, id) && id != 1 {
            return 0;
        }
        let have = self.have_at(c, id, r);
        let cap = self.cap_at(c, id, capacity);
        let limit = match self.target_levels(c, id) {
            Some(levels) => levels.get(r).copied().unwrap_or(0).min(cap),
            None => cap,
        };
        limit.saturating_sub(have)
    }

    fn lift(&mut self, c: &Catalog, travel: Dir, src: End, dst: End, filter: Option<&str>, capacity: u32) -> Lift {
        let ok = |g: &str| filter.is_none_or(|f| f == g);
        match (src, dst) {
            (End::Place(a), End::Cell(to)) => {
                if !self.usable(c, a) && a != 1 {
                    return Lift::Nothing;
                }
                // The good there is most of, or the one asked for.
                let mut best: Option<(u32, String)> = None;
                for r in self.store_resources(c, a) {
                    if !ok(&r) {
                        continue;
                    }
                    let have = self.avail(c, a, &r, 0);
                    if have > best.as_ref().map_or(0, |b| b.0) {
                        best = Some((have, r));
                    }
                }
                let Some((_, r)) = best else { return Lift::Nothing };
                let Some(entry) = self.rollers.cells.get(&to).and_then(|x| x.lane_for(travel)) else { return Lift::Blocked };
                if self.rollers.cells[&to].lane(entry)[0] != 0 {
                    return Lift::Blocked;
                }
                let Some(g) = self.rollers.intern(&r) else { return Lift::Blocked };
                self.draw(a, &r, 1);
                self.rollers.cells.get_mut(&to).unwrap().lane_mut(entry)[0] = g;
                Lift::Done
            }
            (End::Cell(from), End::Place(b)) => {
                let room: BTreeMap<String, u32> = self.rollers.goods_at(from).into_iter().map(|g| (g.to_string(), 0)).collect();
                if room.is_empty() {
                    return Lift::Nothing;
                }
                let want: BTreeSet<String> = room.keys().filter(|g| ok(g) && self.room_for(c, b, g, capacity) > 0).cloned().collect();
                let Some(g) = self.rollers.pick(from, |n| want.contains(n)) else { return Lift::Blocked };
                let name = self.rollers.good_name(g).unwrap_or_default().to_string();
                self.give(c, b, &name, 1);
                let second = self.second;
                self.rollers.lifted(second, from);
                Lift::Done
            }
            (End::Cell(from), End::Cell(to)) => {
                if self.rollers.goods_at(from).is_empty() {
                    return Lift::Nothing;
                }
                let Some(entry) = self.rollers.cells.get(&to).and_then(|x| x.lane_for(travel)) else { return Lift::Blocked };
                if self.rollers.cells[&to].lane(entry)[0] != 0 {
                    return Lift::Blocked;
                }
                let Some(g) = self.rollers.pick(from, ok) else { return Lift::Blocked };
                self.rollers.cells.get_mut(&to).unwrap().lane_mut(entry)[0] = g;
                Lift::Done
            }
            (End::Place(a), End::Place(b)) => {
                if a == b || (!self.usable(c, a) && a != 1) {
                    return Lift::Nothing;
                }
                let mut best: Option<(u32, String)> = None;
                let mut any = false;
                for r in self.store_resources(c, a) {
                    if !ok(&r) {
                        continue;
                    }
                    let have = self.avail(c, a, &r, b);
                    any |= have > 0;
                    if have > 0 && self.room_for(c, b, &r, capacity) > 0 && have > best.as_ref().map_or(0, |x| x.0) {
                        best = Some((have, r));
                    }
                }
                match best {
                    Some((_, r)) => {
                        self.draw(a, &r, 1);
                        self.give(c, b, &r, 1);
                        Lift::Done
                    }
                    None if any => Lift::Blocked,
                    None => Lift::Nothing,
                }
            }
            _ => Lift::Nothing,
        }
    }

    /// One second of every swape and four beats of every rollerway.
    pub(crate) fn rollers_tick(&mut self, c: &Catalog, report: &Report) {
        let swapes: Vec<u32> = self
            .buildings
            .iter()
            .filter(|(_, b)| c.plans[&b.plan].swape > 0 && b.construction == 0 && !b.paused)
            .map(|(&id, _)| id)
            .collect();
        if self.rollers.cells.is_empty() && swapes.is_empty() {
            return;
        }
        let capacity = report.capacity;
        // Whoever goes first changes with the second, so no swape owns the line.
        let turn = if swapes.is_empty() { 0 } else { (self.second % swapes.len() as u64) as usize };
        for k in 0..swapes.len() {
            let id = swapes[(turn + k) % swapes.len()];
            let (plan, tile, facing, filter, paused) = {
                let b = &self.buildings[&id];
                (&c.plans[&b.plan], b.tile, b.facing, b.filter.clone(), b.paused)
            };
            let crewed = report.population.assigned.get(&id).copied().unwrap_or(0) >= plan.workers;
            let gain = u32::from(self.energy_multiplier(c, id, &report.energy));
            let need = u32::from(plan.swape) * 100;
            if paused || !crewed || gain == 0 {
                continue;
            }
            let b = self.buildings.get_mut(&id).unwrap();
            b.clock = (u32::from(b.clock) + gain).min(need.max(gain)) as u16;
            if u32::from(b.clock) < need {
                continue;
            }
            let d = Dir::from_index(facing);
            let (src, dst) = (self.end_of(d.back().step(tile)), self.end_of(d.step(tile)));
            let outcome = self.lift(c, d, src, dst, filter.as_deref(), capacity);
            let b = self.buildings.get_mut(&id).unwrap();
            match outcome {
                Lift::Done => {
                    b.clock = (u32::from(b.clock) - need) as u16;
                    b.lift = 0;
                    b.stats.worked = b.stats.worked.saturating_add(1);
                }
                Lift::Nothing => b.lift = 1,
                Lift::Blocked => b.lift = 2,
            }
        }
        // The hand-chain works as long as there are hands for it.
        let need = self.rollers.chain_tiles() as u32;
        let chain_on = if need == 0 {
            false
        } else {
            let hands = u32::from(report.population.assigned.get(&1).copied().unwrap_or(0)).min(need);
            // The labourers at it, spread over the seconds: `hands` seconds in every `need`.
            let s = self.second as u32;
            (s * hands) / need != (s.saturating_sub(1) * hands) / need || hands == need
        };
        let speed = self.piece_speeds(c);
        for beat in 0..BEATS {
            self.rollers.beat(beat, &speed, chain_on);
        }
        self.rollers.settle();
    }

    /// Beats a second each piece moves on.
    pub(crate) fn piece_speeds(&self, c: &Catalog) -> [u8; 6] {
        let mut speed = [0u8; 6];
        for p in Piece::ALL {
            speed[p.index()] = c.rollers.get(p.key()).map_or(0, |s| s.beats);
        }
        speed
    }

    /// Labourers the hand-chain wants this second: one a tile.
    pub fn chain_crew(&self, c: &Catalog) -> u16 {
        let per = c.rollers.get("chain").map_or(1, |s| u16::from(s.crew));
        (self.rollers.chain_tiles() as u16 * per).min(60)
    }

    // ---- laying -----------------------------------------------------------

    /// Why a piece may not be laid on a tile, if it may not.
    pub fn check_roller_site(&self, c: &Catalog, piece: Piece, tile: Tile) -> Result<(), String> {
        let _ = c;
        if !self.world.is_hillside() {
            return Err("rollerways run on the hillside".into());
        }
        if !self.inside(tile) || !self.world.owned(tile) {
            return Err("that ground is not yours to lay on".into());
        }
        if !self.world.terrain(tile).walkable() {
            return Err("a rollerway cannot cross the river".into());
        }
        if tile == self.gate() || tile == self.camp() {
            return Err("that ground is kept clear".into());
        }
        if let Some(why) = self.reserved_ground(tile) {
            return Err(why);
        }
        if self.at(tile).is_some() {
            return Err("a building stands here".into());
        }
        if PowerKind::ALL.into_iter().any(|kind| self.power.contains(kind, tile)) {
            return Err("a drive shaft runs here".into());
        }
        if self.track_tiles().contains(&tile) {
            return Err("a line's track runs here".into());
        }
        if self.roads.contains(&tile) && piece != Piece::Trestle {
            return Err("a road runs here: a trestle can cross it".into());
        }
        if self.rails.cells.contains_key(&tile) && piece != Piece::Trestle {
            return Err("a rail runs here: a trestle can carry the line over it".into());
        }
        Ok(())
    }

    /// Lay a piece of rollerway on `tile` facing `dir`; laid over another piece it replaces it, goods and all.
    pub(crate) fn lay_roller(&mut self, c: &Catalog, piece: Piece, tile: Tile, dir: Dir) -> Result<String, String> {
        let spec = c.rollers.get(piece.key()).ok_or("this catalogue has no such piece")?;
        if !spec.requires.iter().all(|r| self.researched.contains(r)) {
            return Err(format!("research {} first", spec.requires.join(", ")));
        }
        self.check_roller_site(c, piece, tile)?;
        if let Some(old) = self.rollers.cells.get(&tile) {
            if old.piece == piece && old.dir == dir {
                return Err("it is laid so already".into());
            }
        } else if self.rollers.cells.len() >= MAX_CELLS {
            return Err("too many tiles of rollerway".into());
        }
        let replaced = self.rollers.cells.get(&tile).map(|o| o.piece);
        // Laid over the same piece is only turned; over another is paid for as new.
        let cost = if replaced == Some(piece) { Stock::new() } else { spec.cost.clone() };
        if !covers(&self.stock, &cost) {
            return Err(format!("a {} needs {}", spec.name.to_lowercase(), crate::economy::missing(&self.stock, &cost)));
        }
        take(&mut self.stock, &cost);
        self.world.clear_forest(tile);
        match self.rollers.cells.get_mut(&tile) {
            Some(cell) => {
                cell.piece = piece;
                cell.dir = dir;
                if piece != Piece::Trestle {
                    // What a trestle's second line held has nowhere to go.
                    let held = std::mem::take(&mut cell.across);
                    for g in held {
                        if let Some(n) = self.rollers.goods.get(usize::from(g).wrapping_sub(1)).cloned() {
                            add(&mut self.stock, &n, 1);
                        }
                    }
                }
            }
            None => {
                self.rollers.cells.insert(tile, Cell::new(piece, dir));
            }
        }
        self.rollers.touch();
        Ok(format!("{} laid", spec.name))
    }

    /// Take a tile of rollerway up: the goods on it go to the camp.
    pub(crate) fn lift_roller(&mut self, tile: Tile) -> bool {
        let Some(cell) = self.rollers.cells.remove(&tile) else { return false };
        for g in cell.slots.into_iter().chain(cell.across) {
            if let Some(name) = self.rollers.good_name(g).map(str::to_string) {
                add(&mut self.stock, &name, 1);
            }
        }
        self.rollers.touch();
        true
    }

    /// Turn what stands on `tile` (a tile of rollerway, a swape) to face `dir`.
    pub(crate) fn face(&mut self, c: &Catalog, tile: Tile, dir: Dir) -> Result<String, String> {
        if let Some(cell) = self.rollers.cells.get_mut(&tile) {
            cell.dir = dir;
            self.rollers.touch();
            return Ok(format!("It faces {}", dir.name()));
        }
        if let Some(id) = self.at(tile).filter(|id| self.is_swape(c, *id)) {
            self.buildings.get_mut(&id).unwrap().facing = dir.index();
            return Ok(format!("The swape faces {}", dir.name()));
        }
        // A swape still a plan faces its way when it is raised.
        if let Some(g) = self.plans.ghosts.values_mut().find(|g| g.tile == tile && c.plans.get(&g.plan).is_some_and(|p| p.swape > 0)) {
            g.face = Some(dir.index());
            return Ok(format!("The swape will face {}", dir.name()));
        }
        Err("nothing here turns".into())
    }

    /// Set a swape to lift one good only, or any.
    pub(crate) fn filter(&mut self, c: &Catalog, id: u32, good: Option<String>) -> Result<String, String> {
        if !self.is_swape(c, id) {
            return Err("only a swape takes a good to lift".into());
        }
        if let Some(g) = &good {
            if !c.resources.contains_key(g) {
                return Err("no such good".into());
            }
        }
        let b = self.buildings.get_mut(&id).unwrap();
        b.filter = good.clone();
        b.lift = 0;
        Ok(match good {
            Some(g) => format!("It lifts only {}", c.resources[&g].name.to_lowercase()),
            None => "It lifts whatever comes".into(),
        })
    }

    // ---- reading ------------------------------------------------------------

    /// The goods a swape might be set to lift: what the work behind it makes and holds, what the one
    /// in front of it takes (every good the camp holds, for the camp).
    pub fn swape_goods(&self, c: &Catalog, id: u32) -> Vec<String> {
        let mut goods: BTreeSet<String> = BTreeSet::new();
        let Some((src, dst)) = self.swape_ends(id) else { return Vec::new() };
        for (t, from) in [(src, true), (dst, false)] {
            let Some(bid) = self.at(t).filter(|b| *b != id) else { continue };
            if bid == 1 {
                goods.extend(self.stock.keys().cloned());
                continue;
            }
            let b = &self.buildings[&bid];
            if from {
                goods.extend(b.output.keys().cloned());
                if let Some((_, out, _)) = self.job(c, b) {
                    goods.extend(out.keys().cloned());
                }
            } else {
                goods.extend(self.target_levels(c, bid).unwrap_or_default().keys().cloned());
                if let Some((input, _, _)) = self.job(c, b) {
                    goods.extend(input.keys().cloned());
                }
            }
        }
        goods.into_iter().filter(|g| c.resources.contains_key(g)).collect()
    }

    /// What a swape is doing, in words.
    pub fn swape_words(&self, c: &Catalog, id: u32) -> String {
        let Some(b) = self.buildings.get(&id) else { return String::new() };
        let Some((src, dst)) = self.swape_ends(id) else { return String::new() };
        let name = |t: Tile| match self.end_of(t) {
            End::Cell(_) => "the rollerway".to_string(),
            End::Place(i) => self.buildings.get(&i).map_or("nothing".into(), |x| c.plans[&x.plan].name.to_lowercase()),
            End::Bare => "nothing".to_string(),
        };
        let what = b.filter.as_ref().map_or("whatever comes".to_string(), |g| c.resources.get(g).map_or(g.clone(), |r| r.name.to_lowercase()));
        let state = match b.lift {
            1 => ": nothing to lift",
            2 => ": nowhere to put it",
            _ => "",
        };
        format!("lifts {what} from {} to {}{state}", name(src), name(dst))
    }

    /// Every rollerway, read as a bottleneck hunter would.
    pub fn roller_report(&self, c: &Catalog) -> Vec<RollerRow> {
        let comps = self.rollers.components();
        let mut rows = Vec::new();
        for group in comps {
            let set: BTreeSet<Tile> = group.iter().copied().collect();
            let cells: Vec<(&Tile, &Cell)> = group.iter().map(|t| (t, &self.rollers.cells[t])).collect();
            let items: u32 = cells.iter().map(|(_, x)| x.held()).sum();
            let room: u32 = cells.iter().map(|(_, x)| x.room()).sum();
            let per_min = self.rollers.lifts.0.iter().filter(|(s, t)| s + 60 > self.second && set.contains(t)).count() as u32;
            let swapes = self
                .buildings
                .iter()
                .filter(|(id, b)| c.plans[&b.plan].swape > 0 && self.swape_ends(**id).is_some_and(|(a, z)| set.contains(&a) || set.contains(&z)))
                .count();
            let jams = self.jams_in(c, &set);
            let moving = cells.iter().any(|(_, x)| x.moved > 0);
            let state = if jams.iter().any(|j| j.blocking) {
                RollerState::Jammed
            } else if items == 0 {
                RollerState::Empty
            } else if items * 100 >= room * 60 {
                RollerState::Backed
            } else if moving {
                RollerState::Flowing
            } else {
                RollerState::Backed
            };
            rows.push(RollerRow { id: group[0], tiles: group.len(), items, room, per_min, state, swapes, jams });
        }
        rows
    }

    /// The heads of the jams among these tiles: tiles whose good has stood too long and which have
    /// nowhere further to send it.
    fn jams_in(&self, c: &Catalog, set: &BTreeSet<Tile>) -> Vec<Jam> {
        let mut jams = Vec::new();
        for t in set {
            let cell = &self.rollers.cells[t];
            if cell.stuck < JAM_SECONDS {
                continue;
            }
            let lane = if cell.slots[SLOTS - 1] != 0 { 0 } else { 1 };
            let (outs, n) = outputs(*t, cell, lane);
            let live = outs[..n].iter().any(|&(o, d)| self.rollers.takes(o, d));
            if live {
                continue;
            }
            let good = self.rollers.good_name(cell.lane(lane)[SLOTS - 1]).unwrap_or("goods").to_string();
            let name = c.resources.get(&good).map_or(good.clone(), |r| r.name.to_lowercase());
            // A swape that picks from this tile, and what it says.
            let taker = self.buildings.iter().find(|(id, b)| c.plans[&b.plan].swape > 0 && self.swape_ends(**id).is_some_and(|(a, _)| a == *t));
            let why = match taker {
                Some((id, b)) if b.lift == 2 => {
                    let beyond = self.swape_ends(*id).and_then(|(_, z)| self.at(z)).and_then(|i| self.buildings.get(&i));
                    format!("the swape cannot put {name} into the {}", beyond.map_or("place beyond", |x| c.plans[&x.plan].name.as_str()).to_lowercase())
                }
                Some(_) => format!("the swape here lifts nothing of {name}"),
                None => format!("the line ends and nobody takes {name}"),
            };
            // The queue behind it: stuck tiles that lead here.
            let queue = set.iter().filter(|u| self.rollers.cells[*u].stuck >= JAM_SECONDS).count().saturating_sub(1);
            // The line is blocked if a tile nothing feeds (the start of it) cannot move its good either.
            let blocking = set.iter().any(|u| self.rollers.cells[u].stuck >= JAM_SECONDS && !self.rollers.fed_by_a_tile(*u));
            jams.push(Jam { tile: *t, good, seconds: cell.stuck, queue, why, blocking });
        }
        jams
    }

    /// Whether this tile holds a rollerway's piece.
    pub fn roller_at(&self, tile: Tile) -> Option<&Cell> {
        self.rollers.cells.get(&tile)
    }

    /// Whether the rollerways are sound: tiles on ground that suits them, goods that exist.
    pub(crate) fn rollers_valid(&self, c: &Catalog) -> bool {
        let r = &self.rollers;
        if r.cells.len() > MAX_CELLS || r.goods.len() > 250 || (!r.cells.is_empty() && !self.world.is_hillside()) {
            return false;
        }
        if !r.goods.iter().all(|g| c.resources.contains_key(g)) {
            return false;
        }
        let covered: BTreeSet<Tile> = self.buildings.values().flat_map(|b| self.covered(b)).collect();
        r.cells.iter().all(|(&t, cell)| {
            self.inside(t)
                && self.world.terrain(t).walkable()
                && !covered.contains(&t)
                && (cell.piece == Piece::Trestle || !self.roads.contains(&t))
                && cell.slots.iter().chain(&cell.across).all(|&g| usize::from(g) <= r.goods.len())
                && (cell.piece == Piece::Trestle || cell.across.iter().all(|&g| g == 0))
                && c.rollers.contains_key(cell.piece.key())
        })
    }
}
