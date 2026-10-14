//! The server's side: the game's table of things, and the world published
//! from it once a tick for every writer to read.

use super::{MAX_SPAWN, MAX_STATE};
use std::fmt;
use std::sync::Arc;

/// A thing's changing bytes, kept inline so a published world is one copy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct State {
    len: u8,
    bytes: [u8; MAX_STATE],
}

impl State {
    /// Panics past `MAX_STATE`: a game's state is a few bytes by design.
    pub fn new(bytes: &[u8]) -> State {
        assert!(bytes.len() <= MAX_STATE, "entity state is {} bytes, past MAX_STATE", bytes.len());
        let mut state = State { len: bytes.len() as u8, bytes: [0; MAX_STATE] };
        state.bytes[..bytes.len()].copy_from_slice(bytes);
        state
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

impl Default for State {
    fn default() -> State {
        State { len: 0, bytes: [0; MAX_STATE] }
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_slice())
    }
}

/// One thing in the world.
#[derive(Clone, Debug)]
pub struct Entity {
    pub id: u32,
    pub kind: u16,
    /// Where it is, in the game's own units: only interest reads it.
    pub x: i32,
    pub y: i32,
    /// Bytes fixed at its appearance, sent once to each client that sees it.
    pub spawn: Arc<[u8]>,
    pub state: State,
    /// The tick its state last changed.
    pub version: u32,
    /// The tick it appeared, or last took a new kind or spawn bytes: a
    /// client holding an older one is sent it whole again.
    pub born: u32,
}

/// The game's things, kept between ticks so each change is stamped with
/// the tick it happened on.
pub struct Table {
    cell: i32,
    tick: u32,
    /// Sorted by id.
    entities: Vec<Entity>,
}

impl Table {
    /// `cell` is the interest grid's cell size in the game's units: about a
    /// quarter of a client's view across is right.
    pub fn new(cell: i32) -> Table {
        assert!(cell > 0, "the interest grid needs a positive cell size");
        Table { cell, tick: 0, entities: Vec::new() }
    }

    /// Changes from here on happened on `tick`.
    pub fn begin(&mut self, tick: u32) {
        self.tick = tick;
    }

    pub fn tick(&self) -> u32 {
        self.tick
    }

    fn find(&self, id: u32) -> Result<usize, usize> {
        self.entities.binary_search_by_key(&id, |e| e.id)
    }

    /// A thing appears, or an existing one takes this kind and these spawn
    /// bytes. The same kind and spawn bytes again only set its place and
    /// state, so a game may call this every tick.
    pub fn spawn(&mut self, id: u32, kind: u16, x: i32, y: i32, spawn: &[u8], state: &[u8]) {
        assert!(spawn.len() <= MAX_SPAWN, "entity spawn bytes are {} long, past MAX_SPAWN", spawn.len());
        let tick = self.tick;
        match self.find(id) {
            Ok(i) => {
                let e = &mut self.entities[i];
                if e.kind != kind || *e.spawn != *spawn {
                    e.kind = kind;
                    e.spawn = Arc::from(spawn);
                    e.born = tick;
                    e.version = tick;
                    e.state = State::new(state);
                }
                e.x = x;
                e.y = y;
                let state = State::new(state);
                if e.state != state {
                    e.state = state;
                    e.version = tick;
                }
            }
            Err(i) => self.entities.insert(
                i,
                Entity {
                    id,
                    kind,
                    x,
                    y,
                    spawn: Arc::from(spawn),
                    state: State::new(state),
                    version: tick,
                    born: tick,
                },
            ),
        }
    }

    /// A thing moved or changed. Unchanged bytes keep their version, so a
    /// client holding them is sent nothing. False: there is no such thing.
    pub fn update(&mut self, id: u32, x: i32, y: i32, state: &[u8]) -> bool {
        let tick = self.tick;
        let Ok(i) = self.find(id) else {
            return false;
        };
        let e = &mut self.entities[i];
        e.x = x;
        e.y = y;
        let state = State::new(state);
        if e.state != state {
            e.state = state;
            e.version = tick;
        }
        true
    }

    pub fn remove(&mut self, id: u32) -> bool {
        match self.find(id) {
            Ok(i) => {
                self.entities.remove(i);
                true
            }
            Err(_) => false,
        }
    }

    pub fn get(&self, id: u32) -> Option<&Entity> {
        self.find(id).ok().map(|i| &self.entities[i])
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.entities.iter()
    }

    /// This tick's world, for the writers: one copy of the records (spawn
    /// bytes are shared, not copied) and the interest grid built over them.
    pub fn publish(&self) -> World {
        let entities = self.entities.clone();
        let mut grid: Vec<(u64, u32)> = entities
            .iter()
            .enumerate()
            .map(|(i, e)| (cell_key(e.x.div_euclid(self.cell), e.y.div_euclid(self.cell)), i as u32))
            .collect();
        grid.sort_unstable();
        World { tick: self.tick, cell: self.cell, entities, grid }
    }
}

/// A cell's place in the grid's sort order: by row, then column. The sign
/// bit is flipped so negative coordinates sort below positive ones.
fn cell_key(cx: i32, cy: i32) -> u64 {
    (u64::from(cy as u32 ^ 0x8000_0000) << 32) | u64::from(cx as u32 ^ 0x8000_0000)
}

/// The row of a cell's key.
fn cell_row(key: u64) -> i32 {
    ((key >> 32) as u32 ^ 0x8000_0000) as i32
}

/// A tick's things, read-only, as every writer sees them.
#[derive(Clone, Debug)]
pub struct World {
    tick: u32,
    cell: i32,
    /// Sorted by id.
    entities: Vec<Entity>,
    /// (cell, index into `entities`), sorted: the things in a row of cells
    /// are one run.
    grid: Vec<(u64, u32)>,
}

impl Default for World {
    fn default() -> World {
        World { tick: 0, cell: 1, entities: Vec::new(), grid: Vec::new() }
    }
}

impl World {
    pub fn tick(&self) -> u32 {
        self.tick
    }

    pub fn get(&self, id: u32) -> Option<&Entity> {
        self.entities.binary_search_by_key(&id, |e| e.id).ok().map(|i| &self.entities[i])
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Every thing inside the rectangle (edges included), through the grid:
    /// only the cells it touches are looked at.
    pub fn within(&self, x0: i32, y0: i32, x1: i32, y1: i32, mut found: impl FnMut(&Entity)) {
        let (Some(&(first, _)), Some(&(last, _))) = (self.grid.first(), self.grid.last()) else {
            return;
        };
        let (cx0, cx1) = (x0.div_euclid(self.cell), x1.div_euclid(self.cell));
        // Only the rows that hold anything: a huge rectangle costs no more
        // than the world is tall.
        let cy0 = y0.div_euclid(self.cell).max(cell_row(first));
        let cy1 = y1.div_euclid(self.cell).min(cell_row(last));
        for cy in cy0..=cy1 {
            let (lo, hi) = (cell_key(cx0, cy), cell_key(cx1, cy));
            let start = self.grid.partition_point(|&(key, _)| key < lo);
            for &(key, i) in &self.grid[start..] {
                if key > hi {
                    break;
                }
                let e = &self.entities[i as usize];
                if (x0..=x1).contains(&e.x) && (y0..=y1).contains(&e.y) {
                    found(e);
                }
            }
        }
    }
}
