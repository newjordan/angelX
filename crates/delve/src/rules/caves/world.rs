//! Cave cells, generation, the materials' step, the shovel, the blast, ice and the pack.
//! The host owns this state. Drawing reads it from `render::overworld::cave_seat`.

use crate::ink::hash;

/// The view's size in pixels: 48 cells by 28.
pub(crate) const NATIVE_W: i32 = 384;
pub(crate) const NATIVE_H: i32 = 224;

pub const CELL: i32 = 8;
pub const MAP_W: i32 = (NATIVE_W / CELL) * 4;
pub const MAP_H: i32 = (NATIVE_H / CELL) * 3;
pub const DEMO_SEED: u32 = 7;

pub(crate) const SHOVEL_SWINGS: u8 = 3;
pub(crate) const SHOVEL_YIELD: u8 = 1;
pub(crate) const CARRY_LIMIT: u8 = 6;
pub(crate) const MAGIC_MAX: u8 = 12;
pub(crate) const BLAST_COST: u8 = 4;
pub(crate) const SPELL_COST: u8 = 4;
pub(crate) const REFREEZE_TICKS: u8 = 40;
/// A caught fire burns this many ticks, then wood, grass or fungus is ash.
pub(crate) const FIRE_LIFE: u8 = 40;
/// Water on a flame leaves this much steam and spends one water.
pub(crate) const QUENCH_STEAM: u8 = 8;
/// Lava on ice throws this much steam, the same cloud a cooled plug leaves.
pub(crate) const LAVA_STEAM: u8 = 10;
/// A melted ice cell holds this much water, the same pool a spell leaves.
pub(crate) const MELT_POOL: u8 = 4;
pub(crate) const SLIDE_ICE: i32 = 5;
/// Blades on ice coast this many cells. Plain boots coast `SLIDE_ICE` (5).
pub(crate) const SLIDE_BLADES: i32 = 8;
pub(crate) const SLIDE_DRY: i32 = 0;

pub(crate) const FL_WALL: u16 = 1;
pub(crate) const FL_BRICK: u16 = 2;
pub(crate) const FL_WOOD: u16 = 4;
pub(crate) const FL_GRASS: u16 = 8;
pub(crate) const FL_FUNGUS: u16 = 16;
pub(crate) const FL_ASH: u16 = 32;
pub(crate) const FL_SEALED: u16 = 64;
pub(crate) const FL_FRESH: u16 = 128;
pub(crate) const FL_SEEN: u16 = 256;
pub(crate) const FL_LOOSE: u16 = 512;
pub(crate) const FL_HARD: u16 = 1024;
pub(crate) const FL_ICE: u16 = 2048;
pub(crate) const FL_THIN: u16 = 4096;
pub(crate) const FL_SMOOTH: u16 = 8192;
pub(crate) const FL_MOSS: u16 = 16384;
pub(crate) const FL_RUBBLE: u16 = 32768;

pub(crate) const FLUID_NONE: u8 = 0;
pub(crate) const FLUID_WATER: u8 = 1;
pub(crate) const FLUID_ACID: u8 = 2;
pub(crate) const FLUID_LAVA: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    Wall,
    Brick,
    Floor,
    Grass,
    Fungus,
    Wood,
    Water,
    Acid,
    Lava,
    Sealed,
}

impl Terrain {
    pub fn is_open(self) -> bool {
        matches!(
            self,
            Terrain::Floor
                | Terrain::Grass
                | Terrain::Fungus
                | Terrain::Water
                | Terrain::Acid
                | Terrain::Lava
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Cell {
    pub(crate) flags: u16,
    pub(crate) fire: u8,
    pub(crate) fluid: u8,
    pub(crate) amount: u8,
    pub(crate) integrity: u8,
    pub(crate) steam: u8,
    pub(crate) height: u8,
    pub(crate) stone: u8,
    pub(crate) melt: u8,
    pub(crate) swings: u8,
}

impl Cell {
    pub(crate) fn wall(self) -> bool {
        self.flags & FL_WALL != 0
    }
    pub(crate) fn wood(self) -> bool {
        self.flags & FL_WOOD != 0
    }
    pub(crate) fn grass(self) -> bool {
        self.flags & FL_GRASS != 0
    }
    pub(crate) fn fungus(self) -> bool {
        self.flags & FL_FUNGUS != 0
    }
    pub(crate) fn ash(self) -> bool {
        self.flags & FL_ASH != 0
    }
    pub(crate) fn sealed(self) -> bool {
        self.flags & FL_SEALED != 0
    }
    #[cfg(test)]
    pub(crate) fn fresh(self) -> bool {
        self.flags & FL_FRESH != 0
    }
    pub(crate) fn loose(self) -> bool {
        self.flags & FL_LOOSE != 0
    }
    pub(crate) fn hard(self) -> bool {
        self.flags & FL_HARD != 0
    }
    pub(crate) fn ice(self) -> bool {
        self.flags & FL_ICE != 0
    }
    pub(crate) fn thin(self) -> bool {
        self.flags & FL_THIN != 0
    }
    pub(crate) fn rubble(self) -> bool {
        self.flags & FL_RUBBLE != 0
    }
    pub(crate) fn brick(self) -> bool {
        self.flags & FL_BRICK != 0
    }
    pub(crate) fn seen(self) -> bool {
        self.flags & FL_SEEN != 0
    }
    pub(crate) fn open(self) -> bool {
        !self.wall() && !self.sealed() && !self.wood()
    }
    pub(crate) fn solid(self) -> bool {
        self.wall() || self.wood()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct World {
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) cells: Vec<Cell>,
    pub(crate) awake: Vec<bool>,
    pub(crate) steps: u32,
}

impl World {
    pub(crate) fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            None
        } else {
            Some((y * self.w + x) as usize)
        }
    }

    pub(crate) fn step(&mut self) {
        let n = self.cells.len();
        let snap = self.cells.clone();
        let awake = self.awake.clone();
        let mut next = snap.clone();
        let mut delta = vec![0i8; n];
        let mut quench = vec![false; n];

        for i in 0..n {
            if !awake[i] {
                continue;
            }
            let x = (i as i32) % self.w;
            let y = (i as i32) / self.w;
            let c = snap[i];
            if c.fire > 0 {
                if c.fire % 2 == 0 {
                    for (nx, ny) in neighbors(x, y) {
                        let Some(j) = self.idx(nx, ny) else { continue };
                        if fuel(snap[j]) && next[j].fire == 0 && !next[j].ash() {
                            next[j].fire = FIRE_LIFE;
                        }
                    }
                }
                let left = c.fire - 1;
                next[i].fire = left;
                if left == 0 && fuel(c) {
                    ash(&mut next[i]);
                }
            }
            if c.fluid == FLUID_WATER && c.amount > 0 {
                let mut hit = c.fire > 0;
                if hit {
                    next[i].fire = 0;
                }
                for (nx, ny) in neighbors(x, y) {
                    let Some(j) = self.idx(nx, ny) else { continue };
                    if snap[j].fire > 0 || next[j].fire > 0 {
                        next[j].fire = 0;
                        next[j].steam = next[j].steam.max(QUENCH_STEAM);
                        quench[j] = true;
                        hit = true;
                    }
                }
                if hit {
                    quench[i] = true;
                    next[i].amount = c.amount.saturating_sub(1);
                    if next[i].amount == 0 {
                        next[i].fluid = FLUID_NONE;
                    }
                    next[i].steam = next[i].steam.max(QUENCH_STEAM);
                }
            }
            if c.fluid == FLUID_ACID && c.amount > 0 {
                for (nx, ny) in neighbors(x, y) {
                    let Some(j) = self.idx(nx, ny) else { continue };
                    if snap[j].wall() && !snap[j].wood() && snap[j].integrity > 0 {
                        let left = next[j].integrity.saturating_sub(1);
                        next[j].integrity = left;
                        if left == 0 {
                            next[j].flags &= !(FL_WALL | FL_BRICK | FL_HARD);
                        }
                    }
                }
            }
            if c.fluid == FLUID_LAVA && c.amount > 0 {
                for (nx, ny) in neighbors(x, y) {
                    let Some(j) = self.idx(nx, ny) else { continue };
                    meet_pair(&mut next, i, j, snap[j]);
                }
            }
        }

        for i in 0..n {
            if !awake[i] {
                continue;
            }
            let x = (i as i32) % self.w;
            let y = (i as i32) / self.w;
            let c = next[i];
            if c.amount == 0 || c.wall() {
                continue;
            }
            let flow = c.fluid == FLUID_WATER || (c.fluid == FLUID_LAVA && self.steps % 2 == 0);
            if !flow {
                continue;
            }
            let Some(j) = downhill(&next, self.w, self.h, x, y) else {
                continue;
            };
            if delta[i] > -1 {
                delta[i] -= 1;
                delta[j] += 1;
            }
        }
        for i in 0..n {
            if delta[i] == 0 {
                continue;
            }
            let kind = if next[i].fluid != FLUID_NONE {
                next[i].fluid
            } else if delta[i] > 0 {
                // filled from a neighbour; kind applied below
                FLUID_NONE
            } else {
                FLUID_NONE
            };
            let _ = kind;
        }
        // Apply flow from the post-reaction grid, one unit, no cascade.
        let mid = next.clone();
        for i in 0..n {
            if delta[i] >= 0 {
                continue;
            }
            let x = (i as i32) % self.w;
            let y = (i as i32) / self.w;
            let Some(j) = downhill(&mid, self.w, self.h, x, y) else {
                continue;
            };
            if next[i].amount == 0 {
                continue;
            }
            let kind = next[i].fluid;
            next[i].amount -= 1;
            if next[i].amount == 0 {
                next[i].fluid = FLUID_NONE;
            }
            if next[j].wall() || next[j].sealed() {
                next[i].amount += 1;
                next[i].fluid = kind;
                continue;
            }
            if next[j].fluid == FLUID_NONE || next[j].fluid == kind {
                next[j].fluid = kind;
                next[j].amount = next[j].amount.saturating_add(1).min(6);
            } else {
                next[i].amount += 1;
                next[i].fluid = kind;
            }
        }

        for i in 0..n {
            if next[i].steam > 0 && next[i].steam == snap[i].steam && awake[i] && !quench[i] {
                next[i].steam -= 1;
                if next[i].steam == 0 && next[i].rubble() && next[i].fire == 0 && next[i].fluid == FLUID_NONE {
                    next[i].flags &= !FL_RUBBLE;
                }
            }
            if next[i].melt > 0 && awake[i] {
                next[i].melt -= 1;
                if next[i].melt == 0 && next[i].fluid == FLUID_WATER {
                    next[i].fluid = FLUID_NONE;
                    next[i].amount = 0;
                    next[i].flags |= FL_ICE;
                    next[i].flags &= !(FL_WALL | FL_BRICK | FL_HARD);
                }
            }
        }
        for i in 0..n {
            if quench[i] {
                next[i].fire = 0;
                next[i].steam = next[i].steam.max(QUENCH_STEAM);
            }
        }
        loop {
            let mut opened = false;
            for i in 0..n {
                if next[i].sealed() && !next[i].wall() {
                    let x = (i as i32) % self.w;
                    let y = (i as i32) / self.w;
                    let touch = neighbors(x, y).any(|(nx, ny)| {
                        self.idx(nx, ny).is_some_and(|j| next[j].open() && !next[j].sealed())
                    });
                    if touch {
                        next[i].flags &= !FL_SEALED;
                        opened = true;
                    }
                }
            }
            if !opened {
                break;
            }
        }

        self.awake.fill(false);
        for i in 0..n {
            let changed = next[i] != snap[i];
            let live = active(&next[i]);
            if changed || live {
                self.awake[i] = true;
                let x = (i as i32) % self.w;
                let y = (i as i32) / self.w;
                for (nx, ny) in neighbors(x, y) {
                    if let Some(j) = self.idx(nx, ny) {
                        if changed {
                            self.awake[j] = true;
                        }
                    }
                }
            }
        }
        self.cells = next;
        self.steps += 1;
    }
}

fn idx_of(w: i32, h: i32, x: i32, y: i32) -> Option<usize> {
    if x < 0 || y < 0 || x >= w || y >= h {
        None
    } else {
        Some((y * w + x) as usize)
    }
}

fn downhill_get(w: i32, h: i32, x: i32, y: i32, get: &mut dyn FnMut(usize) -> Cell) -> Option<usize> {
    let i = (y * w + x) as usize;
    let h0 = get(i).height;
    let mut best: Option<(u8, usize)> = None;
    for (nx, ny) in neighbors(x, y) {
        let Some(j) = idx_of(w, h, nx, ny) else { continue };
        let n = get(j);
        if n.wall() || n.sealed() || n.wood() || n.height >= h0 {
            continue;
        }
        if best.is_none_or(|(bh, _)| n.height < bh) {
            best = Some((n.height, j));
        }
    }
    best.map(|(_, j)| j)
}

/// The same step as [`World::step`], but only `touch` (the awake cells) and
/// `sealed` are copied. Neighbours of `touch` are copied so a flow or a
/// quench can land. Cells outside that set are read through `get` and are
/// not written. `steps` is the world's step counter at the start of the step.
/// The host path passes 0, because a fresh view's counter is 0.
pub(crate) fn react_touch(
    w: i32,
    h: i32,
    steps: u32,
    mut get: impl FnMut(usize) -> Cell,
    touch: &[usize],
    sealed: &[usize],
) -> Vec<(usize, Cell)> {
    let mut keys = Vec::with_capacity(touch.len().saturating_mul(5).saturating_add(sealed.len()));
    keys.extend_from_slice(touch);
    for &i in touch {
        let x = (i as i32) % w;
        let y = (i as i32) / w;
        for (nx, ny) in neighbors(x, y) {
            if let Some(j) = idx_of(w, h, nx, ny) {
                keys.push(j);
            }
        }
    }
    keys.extend_from_slice(sealed);
    keys.sort_unstable();
    keys.dedup();
    if keys.is_empty() {
        return Vec::new();
    }
    let mut touch_sorted = touch.to_vec();
    touch_sorted.sort_unstable();
    touch_sorted.dedup();
    let mut awake = vec![false; keys.len()];
    for (s, &k) in keys.iter().enumerate() {
        if touch_sorted.binary_search(&k).is_ok() {
            awake[s] = true;
        }
    }
    let mut snap = Vec::with_capacity(keys.len());
    let mut next = Vec::with_capacity(keys.len());
    for &i in &keys {
        let c = get(i);
        snap.push(c);
        next.push(c);
    }
    let mut delta = vec![0i8; keys.len()];
    let mut quench = vec![false; keys.len()];
    for s in 0..keys.len() {
        if !awake[s] {
            continue;
        }
        let i = keys[s];
        let x = (i as i32) % w;
        let y = (i as i32) / w;
        let c = snap[s];
        if c.fire > 0 {
            if c.fire % 2 == 0 {
                for (nx, ny) in neighbors(x, y) {
                    let Some(j) = idx_of(w, h, nx, ny) else { continue };
                    let nb = cell_at(&keys, &snap, j, &mut get);
                    if fuel(nb) && !cell_at(&keys, &next, j, &mut get).ash() && cell_at(&keys, &next, j, &mut get).fire == 0 {
                        if let Ok(js) = keys.binary_search(&j) {
                            next[js].fire = FIRE_LIFE;
                        }
                    }
                }
            }
            let left = c.fire - 1;
            next[s].fire = left;
            if left == 0 && fuel(c) {
                ash(&mut next[s]);
            }
        }
        if c.fluid == FLUID_WATER && c.amount > 0 {
            let mut hit = c.fire > 0;
            if hit {
                next[s].fire = 0;
            }
            for (nx, ny) in neighbors(x, y) {
                let Some(j) = idx_of(w, h, nx, ny) else { continue };
                let nb = cell_at(&keys, &snap, j, &mut get);
                let nb_next = cell_at(&keys, &next, j, &mut get);
                if nb.fire > 0 || nb_next.fire > 0 {
                    if let Ok(js) = keys.binary_search(&j) {
                        next[js].fire = 0;
                        next[js].steam = next[js].steam.max(QUENCH_STEAM);
                        quench[js] = true;
                        hit = true;
                    }
                }
            }
            if hit {
                let left = c.amount - 1;
                next[s].amount = left;
                next[s].steam = next[s].steam.max(QUENCH_STEAM);
                quench[s] = true;
                if left == 0 {
                    next[s].fluid = FLUID_NONE;
                }
            }
        }
        if c.fluid == FLUID_ACID && c.amount > 0 {
            for (nx, ny) in neighbors(x, y) {
                let Some(j) = idx_of(w, h, nx, ny) else { continue };
                let nb = cell_at(&keys, &snap, j, &mut get);
                if nb.wall() && !nb.wood() && nb.integrity > 0 {
                    if let Ok(js) = keys.binary_search(&j) {
                        let left = next[js].integrity.saturating_sub(1);
                        next[js].integrity = left;
                        if left == 0 {
                            next[js].flags &= !(FL_WALL | FL_BRICK | FL_HARD);
                        }
                    }
                }
            }
        }
        if c.fluid == FLUID_LAVA && c.amount > 0 {
            for (nx, ny) in neighbors(x, y) {
                let Some(j) = idx_of(w, h, nx, ny) else { continue };
                let nb = cell_at(&keys, &snap, j, &mut get);
                if let Ok(js) = keys.binary_search(&j) {
                    meet_pair(&mut next, s, js, nb);
                }
            }
        }
    }
    for s in 0..keys.len() {
        if !awake[s] {
            continue;
        }
        let c = next[s];
        if c.amount == 0 || c.wall() {
            continue;
        }
        let flow = c.fluid == FLUID_WATER || (c.fluid == FLUID_LAVA && steps % 2 == 0);
        if !flow {
            continue;
        }
        let i = keys[s];
        let x = (i as i32) % w;
        let y = (i as i32) / w;
        let Some(j) = downhill_get(w, h, x, y, &mut |k| cell_at(&keys, &next, k, &mut get)) else {
            continue;
        };
        if delta[s] > -1 {
            delta[s] -= 1;
            if let Ok(js) = keys.binary_search(&j) {
                delta[js] = delta[js].saturating_add(1);
            }
        }
    }
    let mid = next.clone();
    for s in 0..keys.len() {
        if delta[s] >= 0 {
            continue;
        }
        let i = keys[s];
        let x = (i as i32) % w;
        let y = (i as i32) / w;
        let Some(j) = downhill_get(w, h, x, y, &mut |k| cell_at(&keys, &mid, k, &mut get)) else {
            delta[s] = 0;
            continue;
        };
        let Ok(js) = keys.binary_search(&j) else {
            delta[s] = 0;
            continue;
        };
        let dest = mid[js];
        if dest.wall() || dest.sealed() || (dest.amount > 0 && dest.fluid != mid[s].fluid) {
            delta[s] = 0;
            continue;
        }
        let kind = mid[s].fluid;
        let left = mid[s].amount - 1;
        next[s].amount = left;
        if left == 0 {
            next[s].fluid = FLUID_NONE;
        }
        let got = mid[js].amount.saturating_add(1).min(6);
        next[js].amount = got;
        next[js].fluid = kind;
    }
    for s in 0..keys.len() {
        if snap[s].steam > 0 && next[s].steam == snap[s].steam && awake[s] && !quench[s] {
            next[s].steam -= 1;
            if next[s].steam == 0 && next[s].rubble() && next[s].fire == 0 && next[s].fluid == FLUID_NONE {
                next[s].flags &= !FL_RUBBLE;
            }
        }
        if next[s].melt > 0 && awake[s] {
            next[s].melt -= 1;
            if next[s].melt == 0 && next[s].fluid == FLUID_WATER {
                next[s].fluid = FLUID_NONE;
                next[s].amount = 0;
                next[s].flags |= FL_ICE;
                next[s].flags &= !(FL_WALL | FL_BRICK | FL_HARD);
            }
        }
    }
    for s in 0..keys.len() {
        if quench[s] {
            next[s].fire = 0;
            next[s].steam = next[s].steam.max(QUENCH_STEAM);
        }
    }
    let mut sealed_left = sealed.to_vec();
    sealed_left.sort_unstable();
    sealed_left.dedup();
    loop {
        let mut opened = false;
        for &i in &sealed_left {
            let Ok(s) = keys.binary_search(&i) else { continue };
            if !(next[s].sealed() && !next[s].wall()) {
                continue;
            }
            let x = (i as i32) % w;
            let y = (i as i32) / w;
            let mut open_nb = false;
            for (nx, ny) in neighbors(x, y) {
                let Some(j) = idx_of(w, h, nx, ny) else { continue };
                let nb = cell_at(&keys, &next, j, &mut get);
                if nb.open() && !nb.sealed() {
                    open_nb = true;
                    break;
                }
            }
            if open_nb {
                next[s].flags &= !FL_SEALED;
                opened = true;
            }
        }
        if !opened {
            break;
        }
    }
    keys.into_iter().zip(next).collect()
}

fn cell_at(keys: &[usize], cells: &[Cell], i: usize, get: &mut dyn FnMut(usize) -> Cell) -> Cell {
    if let Ok(s) = keys.binary_search(&i) {
        cells[s]
    } else {
        get(i)
    }
}

pub(crate) fn plug(c: &mut Cell) {
    c.fluid = FLUID_NONE;
    c.amount = 0;
    c.fire = 0;
    c.flags |= FL_WALL | FL_FRESH | FL_HARD;
    c.flags &= !(FL_SEALED | FL_LOOSE | FL_ICE);
    c.steam = c.steam.max(LAVA_STEAM);
    c.integrity = 255;
    c.swings = 0;
}

/// Wood, grass and fungus all catch and all burn down to ash.
pub(crate) fn fuel(c: Cell) -> bool {
    c.wood() || c.grass() || c.fungus()
}

fn ash(c: &mut Cell) {
    c.flags &= !(FL_WOOD | FL_GRASS | FL_FUNGUS);
    c.flags |= FL_ASH;
}

/// Split two cells so lava can touch its neighbour without aliasing the borrow.
fn meet_pair(next: &mut [Cell], lava_at: usize, nb_at: usize, nb_snap: Cell) {
    if lava_at == nb_at {
        return;
    }
    let (lo, hi) = if lava_at < nb_at {
        (lava_at, nb_at)
    } else {
        (nb_at, lava_at)
    };
    let (left, right) = next.split_at_mut(hi);
    if lava_at < nb_at {
        lava_meets(&mut left[lo], nb_snap, &mut right[0]);
    } else {
        lava_meets(&mut right[0], nb_snap, &mut left[lo]);
    }
}

/// Lava against one neighbour. Water, and ice melted into water, quench the
/// same way: both cells become a cooled plug with a steam cloud. Wood, grass
/// and fungus catch a full burn.
fn lava_meets(lava: &mut Cell, nb_snap: Cell, nb: &mut Cell) {
    let water = nb_snap.fluid == FLUID_WATER && nb_snap.amount > 0;
    if nb_snap.ice() && !water {
        nb.flags &= !(FL_WALL | FL_BRICK | FL_HARD | FL_LOOSE | FL_ICE | FL_THIN | FL_SEALED);
        nb.fluid = FLUID_WATER;
        nb.amount = MELT_POOL;
        nb.steam = nb.steam.max(LAVA_STEAM);
        plug(lava);
        plug(nb);
        return;
    }
    if water {
        plug(lava);
        plug(nb);
        return;
    }
    if fuel(nb_snap) && nb.fire == 0 {
        nb.fire = FIRE_LIFE;
    }
}

pub(crate) fn active(c: &Cell) -> bool {
    c.fire > 0 || c.steam > 0 || c.melt > 0 || (c.amount > 0 && c.fluid != FLUID_NONE)
}

pub(crate) fn neighbors(x: i32, y: i32) -> impl Iterator<Item = (i32, i32)> {
    [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().map(move |(dx, dy)| (x + dx, y + dy))
}

pub(crate) fn downhill(cells: &[Cell], w: i32, h: i32, x: i32, y: i32) -> Option<usize> {
    let i = (y * w + x) as usize;
    let h0 = cells[i].height;
    let mut best: Option<(u8, i32, i32, usize)> = None;
    for (nx, ny) in neighbors(x, y) {
        if nx < 0 || ny < 0 || nx >= w || ny >= h {
            continue;
        }
        let j = (ny * w + nx) as usize;
        let n = cells[j];
        if n.wall() || n.sealed() || n.wood() || n.height >= h0 {
            continue;
        }
        let key = (n.height, ny, nx, j);
        if best.is_none_or(|b| key < b) {
            best = Some(key);
        }
    }
    best.map(|b| b.3)
}

pub(crate) fn spine_y(x: i32) -> i32 {
    let t = x.rem_euclid(40);
    let tri = if t < 20 { t / 4 } else { (40 - t) / 4 };
    42 + tri - 2
}

pub(crate) fn ellipse_open(x: i32, y: i32, cx: i32, cy: i32, rx: i32, ry: i32, seed: u32) -> bool {
    if rx <= 0 || ry <= 0 {
        return false;
    }
    let dx = (x - cx) as f32 / rx as f32;
    let dy = (y - cy) as f32 / ry as f32;
    let d = dx * dx + dy * dy;
    if d <= 0.62 {
        return true;
    }
    if d > 1.0 {
        return false;
    }
    let (tx, ty) = (x + (cx - x).signum(), y + (cy - y).signum());
    let tdx = (tx - cx) as f32 / rx as f32;
    let tdy = (ty - cy) as f32 / ry as f32;
    tdx * tdx + tdy * tdy <= 0.62 && hash(x, y, seed) % 3 != 0
}

pub(crate) fn in_rect(x: i32, y: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    x >= x0 && x < x1 && y >= y0 && y < y1
}

pub(crate) fn spine_half(x: i32, seed: u32) -> i32 {
    5 + ((hash(x, 19, seed) >> 1) % 2) as i32
}

pub(crate) fn on_spine(x: i32, y: i32, seed: u32) -> bool {
    let sy = spine_y(x);
    let half = spine_half(x, seed);
    let dy = (y - sy).abs();
    if dy < half {
        return true;
    }
    dy == half && hash(x, y, seed) % 4 != 0
}

pub(crate) fn frost_hall(x: i32, y: i32, seed: u32) -> bool {
    ellipse_open(x, y, 104, 8, 34, 8, seed)
        || ellipse_open(x, y, 70, 10, 14, 6, seed)
        || ellipse_open(x, y, 132, 11, 12, 6, seed)
}

pub(crate) fn frost_mouth(x: i32, y: i32, seed: u32) -> bool {
    if y < 12 || y > 46 {
        return false;
    }
    let wob = ((hash(y, 3, seed) >> 2) % 5) as i32 - 2;
    let mid = 102 + wob;
    (x - mid).abs() <= 3 || (x - 102).abs() <= 1
}

pub(crate) fn frost_cell(x: i32, y: i32) -> bool {
    y >= 2 && y < 20 && x >= 60 && x < 146
}

pub(crate) fn carved(x: i32, y: i32, seed: u32) -> bool {
    if on_spine(x, y, seed) {
        return true;
    }
    if frost_hall(x, y, seed) || frost_mouth(x, y, seed) {
        return true;
    }
    if ellipse_open(x, y, 16, spine_y(16), 12, 8, seed) {
        return true;
    }
    if ellipse_open(x, y, 42, 28, 12, 9, seed) || in_rect(x, y, 41, 28, 44, spine_y(x).max(28)) {
        return true;
    }
    if ellipse_open(x, y, 24, 16, 8, 5, seed) || in_rect(x, y, 24, 15, 42, 18) {
        return true;
    }
    if in_rect(x, y, 18, 24, 34, 31) || in_rect(x, y, 30, 31, 33, spine_y(x).max(31)) || in_rect(x, y, 39, 17, 43, 30)
    {
        return true;
    }
    if ellipse_open(x, y, 96, 40, 18, 12, seed) {
        return true;
    }
    if ellipse_open(x, y, 114, 56, 8, 6, seed) || in_rect(x, y, 112, 38, 118, 56) {
        return true;
    }
    if ellipse_open(x, y, 162, 42, 16, 10, seed) {
        return true;
    }
    if ellipse_open(x, y, 150, 24, 7, 5, seed)
        || in_rect(x, y, 148, 24, 154, 36)
        || in_rect(x, y, 146, 22, 171, 25)
        || in_rect(x, y, 166, 22, 173, 44)
    {
        return true;
    }
    if in_rect(x, y, 148, 38, 154, 48)
        || ellipse_open(x, y, 168, 54, 7, 4, seed)
        || in_rect(x, y, 150, 52, 170, 58)
        || in_rect(x, y, 156, 46, 164, 56)
    {
        return true;
    }
    if ellipse_open(x, y, 52, 50, 10, 6, seed) || ellipse_open(x, y, 78, 34, 10, 7, seed) {
        return true;
    }
    if !(0..MAP_W).contains(&x) || !(0..MAP_H).contains(&y) {
        let cx = x.div_euclid(24) * 24 + 12;
        let cy = spine_y(cx);
        if ellipse_open(x, y, cx, cy, 7, 5, seed) {
            return true;
        }
    }
    false
}

pub(crate) fn is_prop(x: i32, y: i32) -> bool {
    matches!(
        (x, y),
        (38, 28) | (46, 28) | (42, 26) | (42, 30) | (36, 27) | (48, 27) | (22, 27) | (26, 27) | (32, 27)
    )
}

pub(crate) fn is_brick_ruin(x: i32, y: i32) -> bool {
    in_rect(x, y, 154, 38, 160, 40) || in_rect(x, y, 168, 46, 173, 48) || in_rect(x, y, 158, 46, 161, 50)
}

/// The terrain a seed gives a world cell, before the halls are worn through.
pub fn terrain(seed: u32, x: i32, y: i32) -> Terrain {
    if in_rect(x, y, 12, 33, 16, 37) || in_rect(x, y, 18, 33, 22, 37) {
        return Terrain::Wall;
    }
    if in_rect(x, y, 12, 37, 16, 42) || in_rect(x, y, 18, 37, 22, 42) {
        return Terrain::Floor;
    }
    if in_rect(x, y, 21, 21, 29, 24) || in_rect(x, y, 147, 36, 154, 38) || in_rect(x, y, 110, 61, 119, 62) {
        return if in_rect(x, y, 147, 36, 154, 38) { Terrain::Brick } else { Terrain::Wall };
    }
    if in_rect(x, y, 22, 18, 27, 21) {
        return Terrain::Water;
    }
    if is_prop(x, y) && carved(x, y, seed) {
        return Terrain::Wood;
    }
    if ellipse_open(x, y, 114, 68, 5, 3, seed) || in_rect(x, y, 111, 62, 118, 68) {
        return Terrain::Sealed;
    }
    if carved(x, y, seed) {
        if ellipse_open(x, y, 24, 16, 5, 3, seed) {
            return Terrain::Water;
        }
        if in_rect(x, y, 112, 55, 117, 58) {
            return Terrain::Acid;
        }
        if in_rect(x, y, 149, 23, 152, 27) {
            return Terrain::Lava;
        }
        if in_rect(x, y, 149, 42, 152, 47) || ellipse_open(x, y, 168, 54, 4, 2, seed) {
            return Terrain::Water;
        }
        if in_rect(x, y, 20, 26, 33, 30) {
            return Terrain::Grass;
        }
        if (90..104).contains(&x) && (34..46).contains(&y) && hash(x, y, seed.wrapping_add(11)) % 5 == 0 {
            return Terrain::Fungus;
        }
        return Terrain::Floor;
    }
    if is_brick_ruin(x, y) || ((150..176).contains(&x) && hash(x, y, seed) % 7 == 0 && !carved(x, y, seed)) {
        return Terrain::Brick;
    }
    Terrain::Wall
}

pub(crate) fn cell_from(kind: Terrain, x: i32, y: i32) -> Cell {
    let mut flags = 0;
    let mut fluid = FLUID_NONE;
    let mut amount = 0;
    let mut integrity = 255;
    match kind {
        Terrain::Wall => flags |= FL_WALL | FL_HARD,
        Terrain::Brick => flags |= FL_WALL | FL_BRICK | FL_HARD,
        Terrain::Wood => flags |= FL_WOOD,
        Terrain::Grass => flags |= FL_GRASS,
        Terrain::Fungus => flags |= FL_FUNGUS,
        Terrain::Water => {
            fluid = FLUID_WATER;
            amount = 4;
        }
        Terrain::Acid => {
            fluid = FLUID_ACID;
            amount = 4;
        }
        Terrain::Lava => {
            fluid = FLUID_LAVA;
            amount = 5;
        }
        Terrain::Sealed => flags |= FL_SEALED,
        Terrain::Floor => {}
    }
    if in_rect(x, y, 110, 61, 119, 62) {
        integrity = 12;
    }
    if kind == Terrain::Sealed {
        flags |= FL_FUNGUS;
    }
    let region = region_of(x, y).0;
    let stone = match region {
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 0,
    };
    let mut swings = 0;
    if flags & FL_WALL != 0 && in_rect(x, y, 12, 33, 16, 37) {
        flags |= FL_LOOSE;
        flags &= !FL_HARD;
        swings = SHOVEL_SWINGS;
    }
    if region == 3 {
        flags |= FL_ICE;
        if kind.is_open() && in_rect(x, y, 88, 6, 94, 10) {
            flags |= FL_THIN;
        }
        if kind.is_open() && in_rect(x, y, 106, 8, 110, 12) {
            flags &= !FL_ICE;
            flags |= FL_RUBBLE;
        }
    } else if region == 1 && kind == Terrain::Floor {
        flags |= FL_MOSS;
    } else if region == 2 && kind == Terrain::Floor {
        flags |= FL_SMOOTH;
    }
    Cell {
        flags,
        fire: 0,
        fluid,
        amount,
        integrity,
        steam: 0,
        height: y as u8,
        stone,
        melt: 0,
        swings,
    }
}

fn interior(x: i32, y: i32) -> bool {
    x > 0 && y > 0 && x < MAP_W - 1 && y < MAP_H - 1
}

fn wear_floor(cell: &mut Cell, x: i32, y: i32) {
    cell.flags &= !(FL_WALL | FL_HARD | FL_SEALED | FL_BRICK | FL_LOOSE | FL_RUBBLE | FL_WOOD);
    if region_of(x, y).0 == 3 {
        cell.flags |= FL_ICE;
    } else {
        cell.flags &= !FL_ICE;
    }
    cell.fluid = FLUID_NONE;
    cell.amount = 0;
    cell.fire = 0;
    cell.integrity = 0;
}

fn wear_at(world: &mut World, x: i32, y: i32) {
    if !interior(x, y) {
        return;
    }
    if let Some(i) = world.idx(x, y) {
        wear_floor(&mut world.cells[i], x, y);
    }
}

fn carve_rect(world: &mut World, x0: i32, y0: i32, x1: i32, y1: i32) {
    for y in y0..y1 {
        for x in x0..x1 {
            wear_at(world, x, y);
        }
    }
}

fn wear_line(world: &mut World, a: (i32, i32), b: (i32, i32)) {
    let mut x = a.0;
    let mut y = a.1;
    let dx = (b.0 - a.0).abs();
    let dy = (b.1 - a.1).abs();
    let sx = if a.0 < b.0 { 1 } else { -1 };
    let sy = if a.1 < b.1 { 1 } else { -1 };
    let mut err = dx - dy;
    loop {
        wear_at(world, x, y);
        if x == b.0 && y == b.1 {
            break;
        }
        let e2 = err * 2;
        let step_x = e2 > -dy;
        let step_y = e2 < dx;
        if step_x && step_y {
            wear_at(world, x + sx, y);
        }
        if step_x {
            err -= dy;
            x += sx;
        }
        if step_y {
            err += dx;
            y += sy;
        }
    }
}

fn flood_open(world: &World, start: (i32, i32)) -> Vec<bool> {
    let n = world.cells.len();
    let mut seen = vec![false; n];
    let Some(i) = world.idx(start.0, start.1) else {
        return seen;
    };
    if !world.cells[i].open() {
        return seen;
    }
    let mut q = vec![start];
    seen[i] = true;
    while let Some((x, y)) = q.pop() {
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            let Some(j) = world.idx(nx, ny) else {
                continue;
            };
            if seen[j] || !world.cells[j].open() {
                continue;
            }
            seen[j] = true;
            q.push((nx, ny));
        }
    }
    seen
}

fn anchor_of(region: u8) -> Option<(i32, i32)> {
    for y in 6..MAP_H - 6 {
        for x in 8..MAP_W - 8 {
            if region_of(x, y).0 != region {
                continue;
            }
            let mut ok = true;
            for dy in -4..=4 {
                for dx in -6..=6 {
                    if region_of(x + dx, y + dy).0 != region || !interior(x + dx, y + dy) {
                        ok = false;
                    }
                }
            }
            if ok {
                return Some((x, y));
            }
        }
    }
    None
}

/// Worn halls and passages, part of the seed's world. No changed-cell record.

fn open_worn_hall(world: &mut World) {
    if let Some(i) = world.idx(48, 36) {
        world.cells[i].flags |= FL_WALL | FL_LOOSE;
        world.cells[i].flags &= !(FL_HARD | FL_ICE | FL_THIN | FL_SEALED);
    }
    if let Some(i) = world.idx(6, 37) {
        world.cells[i].flags &= !(FL_WALL | FL_HARD | FL_SEALED | FL_LOOSE);
    }
    if let Some(i) = world.idx(6, 36) {
        world.cells[i].flags |= FL_WALL | FL_HARD;
        world.cells[i].flags &= !FL_LOOSE;
    }
    let start = (8, spine_y(8));
    for _ in 0..64 {
        let seen = flood_open(world, start);
        let mut other = None;
        for y in 1..world.h - 1 {
            for x in 1..world.w - 1 {
                let i = (y * world.w + x) as usize;
                if !seen[i] && world.cells[i].open() {
                    other = Some((x, y));
                    break;
                }
            }
            if other.is_some() {
                break;
            }
        }
        let Some(other) = other else {
            break;
        };
        wear_line(world, start, other);
    }
    if let Some(i) = world.idx(48, 36) {
        world.cells[i].flags |= FL_WALL | FL_LOOSE;
        world.cells[i].flags &= !(FL_HARD | FL_ICE | FL_THIN | FL_SEALED);
    }
    if let Some(i) = world.idx(6, 37) {
        world.cells[i].flags &= !(FL_WALL | FL_HARD | FL_SEALED | FL_LOOSE);
    }
    if let Some(i) = world.idx(6, 36) {
        world.cells[i].flags |= FL_WALL | FL_HARD;
        world.cells[i].flags &= !FL_LOOSE;
    }
}
fn link_regions(world: &mut World) {
    carve_rect(world, 8, 37, 52, 48);
    for region in 1..4u8 {
        let Some((cx, cy)) = anchor_of(region) else {
            continue;
        };
        for dy in -4..=4 {
            for dx in -6..=6 {
                let x = cx + dx;
                let y = cy + dy;
                if region_of(x, y).0 == region {
                    wear_at(world, x, y);
                }
            }
        }
        wear_line(world, (20, 40), (cx, cy));
    }
    let hub = (20, 40);
    for _ in 0..24 {
        let seen = flood_open(world, hub);
        let mut other = None;
        for y in 1..world.h - 1 {
            for x in 1..world.w - 1 {
                let i = (y * world.w + x) as usize;
                if !seen[i] && world.cells[i].open() {
                    other = Some((x, y));
                    break;
                }
            }
            if other.is_some() {
                break;
            }
        }
        let Some(other) = other else {
            break;
        };
        wear_line(world, hub, other);
    }
    if let Some(i) = world.idx(24, 48) {
        let cell = &mut world.cells[i];
        cell.flags |= FL_WALL | FL_LOOSE;
        cell.flags &= !(FL_HARD | FL_SEALED | FL_WOOD | FL_BRICK | FL_ICE);
        cell.fluid = FLUID_NONE;
        cell.amount = 0;
        cell.swings = SHOVEL_SWINGS;
    }
}

/// Loose rock, which a shovel opens: patches of the two cells of wall nearest
/// the open cave. Everything deeper stays hard and takes magic. Brick, the
/// thin wall acid eats and the hard block beside the first loose patch are
/// left as they are.
fn soften_crust(world: &mut World, seed: u32) {
    let near_open = |world: &World, x: i32, y: i32| {
        (-2..=2i32).any(|dy| {
            (-2..=2i32).any(|dx| {
                dx.abs() + dy.abs() <= 2 && world.idx(x + dx, y + dy).is_some_and(|i| world.cells[i].open())
            })
        })
    };
    for y in 1..world.h - 1 {
        for x in 1..world.w - 1 {
            let i = (y * world.w + x) as usize;
            let cell = world.cells[i];
            if !cell.wall() || !cell.hard() || cell.brick() || cell.integrity != 255 {
                continue;
            }
            if in_rect(x, y, 18, 33, 22, 37) {
                continue;
            }
            if hash(x.div_euclid(5), y.div_euclid(3), seed.wrapping_add(71)) % 5 >= 2 {
                continue;
            }
            if !near_open(world, x, y) {
                continue;
            }
            let cell = &mut world.cells[i];
            cell.flags |= FL_LOOSE;
            cell.flags &= !FL_HARD;
            cell.swings = SHOVEL_SWINGS;
        }
    }
}

pub(crate) fn close_pockets(world: &mut World) {
    let start = (8, spine_y(8));
    let n = world.cells.len();
    let mut seen = vec![false; n];
    let mut q = vec![start];
    if let Some(i) = world.idx(start.0, start.1) {
        seen[i] = true;
    }
    while let Some((x, y)) = q.pop() {
        for (nx, ny) in neighbors(x, y) {
            let Some(j) = world.idx(nx, ny) else { continue };
            if seen[j] || !world.cells[j].open() {
                continue;
            }
            seen[j] = true;
            q.push((nx, ny));
        }
    }
    for (i, cell) in world.cells.iter_mut().enumerate() {
        if cell.open() && !seen[i] {
            cell.flags |= FL_WALL | FL_HARD;
            cell.fluid = FLUID_NONE;
            cell.amount = 0;
        }
    }
}

/// One cave: every cell of the seed's map, as generated or as the host has changed it.
#[derive(Clone)]
pub struct Cave {
    pub seed: u32,
    pub(crate) world: World,
}

impl Cave {
    pub fn generate(seed: u32) -> Self {
        let n = (MAP_W * MAP_H) as usize;
        let mut cells = Vec::with_capacity(n);
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                cells.push(cell_from(terrain(seed, x, y), x, y));
            }
        }
        let mut awake = vec![false; n];
        for (i, c) in cells.iter().enumerate() {
            if active(c) {
                awake[i] = true;
            }
        }
        let mut world = World { w: MAP_W, h: MAP_H, cells, awake, steps: 0 };
        close_pockets(&mut world);
        link_regions(&mut world);
        open_worn_hall(&mut world);
        soften_crust(&mut world, seed);
        Self { seed, world }
    }

    pub(crate) fn at(&self, x: i32, y: i32) -> Option<&Cell> {
        self.world.idx(x, y).map(|i| &self.world.cells[i])
    }

    pub(crate) fn at_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        self.world.idx(x, y).map(|i| &mut self.world.cells[i])
    }

    pub fn step_sim(&mut self) {
        self.world.step();
    }

    pub(crate) fn wake(&mut self, x: i32, y: i32) {
        if let Some(i) = self.world.idx(x, y) {
            self.world.awake[i] = true;
            for (nx, ny) in neighbors(x, y) {
                if let Some(j) = self.world.idx(nx, ny) {
                    self.world.awake[j] = true;
                }
            }
        }
    }

    pub fn fingerprint(&self) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        for c in &self.world.cells {
            for b in [
                c.flags as u8,
                (c.flags >> 8) as u8,
                c.fire,
                c.fluid,
                c.amount,
                c.integrity,
                c.steam,
                c.height,
                c.stone,
                c.melt,
                c.swings,
            ] {
                h ^= b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
        h
    }

    pub(crate) fn shovel(&mut self, pack: &mut Pack, x: i32, y: i32) -> Dig {
        let Some(cell) = self.at(x, y) else {
            return Dig::Miss;
        };
        let wall = cell.wall();
        let loose = cell.loose() || (wall && !cell.hard());
        let rubble = cell.rubble() && !wall;
        if wall && cell.hard() && !cell.loose() {
            return Dig::Ring;
        }
        if !loose && !rubble {
            return Dig::Miss;
        }
        let swings = if cell.swings == 0 { SHOVEL_SWINGS } else { cell.swings };
        let left = swings - 1;
        if left > 0 {
            if let Some(c) = self.at_mut(x, y) {
                c.swings = left;
            }
            self.wake(x, y);
            return Dig::Swing(left);
        }
        let kind = cell.stone;
        let above = self.at(x, y - 1).is_some_and(|n| n.wall() && n.hard() && !n.loose());
        if let Some(c) = self.at_mut(x, y) {
            c.flags &= !(FL_WALL | FL_BRICK | FL_HARD | FL_LOOSE | FL_SEALED);
            c.flags |= FL_RUBBLE;
            c.swings = SHOVEL_SWINGS;
            c.integrity = 0;
            if c.steam < 18 {
                c.steam = 18;
            }
        }
        if above {
            if let Some(n) = self.at_mut(x, y - 1) {
                n.flags |= FL_LOOSE;
                n.flags &= !FL_HARD;
                n.swings = SHOVEL_SWINGS;
            }
            self.wake(x, y - 1);
        }
        self.wake(x, y);
        let got = pack.add(kind, SHOVEL_YIELD);
        Dig::Broke { kind, got }
    }

    pub(crate) fn blast(&mut self, pack: &mut Pack, x: i32, y: i32) -> bool {
        if pack.magic < BLAST_COST {
            return false;
        }
        let faced = self.at(x, y).is_some_and(|c| c.wall());
        if !faced {
            return false;
        }
        pack.magic -= BLAST_COST;
        for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = x + dx;
            let ny = y + dy;
            let Some(c) = self.at(nx, ny) else { continue };
            if !c.wall() {
                continue;
            }
            let kind = c.stone;
            let above = self.at(nx, ny - 1).is_some_and(|n| n.wall() && n.hard());
            if let Some(c) = self.at_mut(nx, ny) {
                c.flags &= !(FL_WALL | FL_BRICK | FL_HARD | FL_LOOSE | FL_SEALED);
                c.integrity = 0;
                c.flags |= FL_RUBBLE;
            }
            let _ = pack.add(kind, SHOVEL_YIELD);
            if above {
                if let Some(n) = self.at_mut(nx, ny - 1) {
                    if n.wall() {
                        n.flags |= FL_LOOSE;
                        n.flags &= !FL_HARD;
                        n.swings = SHOVEL_SWINGS;
                    }
                }
            }
            self.wake(nx, ny);
        }
        true
    }

    pub(crate) fn frost_bridge(&mut self, pack: &mut Pack, x: i32, y: i32) -> bool {
        if pack.magic < SPELL_COST {
            return false;
        }
        let Some(cell) = self.at(x, y) else {
            return false;
        };
        if cell.fluid != FLUID_WATER && cell.fluid != FLUID_LAVA {
            return false;
        }
        pack.magic -= SPELL_COST;
        if let Some(c) = self.at_mut(x, y) {
            c.fluid = FLUID_NONE;
            c.amount = 0;
            c.fire = 0;
            c.steam = 0;
            c.melt = 0;
            c.flags &= !(FL_WALL | FL_BRICK | FL_SEALED | FL_HARD);
            c.flags |= FL_ICE;
        }
        self.wake(x, y);
        true
    }

    pub fn melt_ice(&mut self, x: i32, y: i32) {
        let Some(cell) = self.at(x, y) else {
            return;
        };
        if !cell.ice() && !cell.thin() {
            return;
        }
        if let Some(c) = self.at_mut(x, y) {
            c.flags &= !(FL_WALL | FL_BRICK | FL_HARD | FL_LOOSE | FL_ICE | FL_THIN | FL_SEALED);
            c.fluid = FLUID_WATER;
            c.amount = MELT_POOL;
            c.fire = 0;
            c.melt = REFREEZE_TICKS;
        }
        self.wake(x, y);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Footwear {
    Plain,
    Grip,
    Blades,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dig {
    Miss,
    Ring,
    Swing(u8),
    Broke { kind: u8, got: u8 },
}

pub(crate) struct Pack {
    pub(crate) magic: u8,
    pub(crate) stone: [u8; 4],
    pub(crate) footwear: Footwear,
}

impl Pack {
    pub(crate) fn total(&self) -> u8 {
        self.stone.iter().sum()
    }

    pub(crate) fn full(&self) -> bool {
        self.total() >= CARRY_LIMIT
    }

    pub(crate) fn add(&mut self, kind: u8, n: u8) -> u8 {
        let room = CARRY_LIMIT.saturating_sub(self.total());
        let got = n.min(room);
        let i = (kind as usize).min(3);
        self.stone[i] = self.stone[i].saturating_add(got);
        got
    }
}

pub(crate) fn slippery(flags: u16, footwear: Footwear) -> bool {
    if footwear == Footwear::Grip {
        return false;
    }
    let ice = flags & FL_ICE != 0;
    let smooth = flags & FL_SMOOTH != 0;
    ice || (footwear == Footwear::Blades && smooth)
}

pub(crate) fn coast_cells(flags: u16, footwear: Footwear) -> i32 {
    if footwear == Footwear::Grip {
        return SLIDE_DRY;
    }
    if flags & FL_ICE != 0 && footwear == Footwear::Blades {
        return SLIDE_BLADES;
    }
    if slippery(flags, footwear) { SLIDE_ICE } else { SLIDE_DRY }
}

pub(crate) fn region_of(x: i32, y: i32) -> (u8, u8) {
    if frost_cell(x, y) {
        return (3, 0);
    }
    let wobble = (hash(0, y, DEMO_SEED) % 5) as i32 - 2;
    let a = 64 + wobble;
    let b = 128 + wobble;
    if x < a - 2 {
        (0, 0)
    } else if x < a + 2 {
        (0, (x - (a - 2) + 1) as u8)
    } else if x < b - 2 {
        (1, 0)
    } else if x < b + 2 {
        (1, (x - (b - 2) + 1) as u8)
    } else {
        (2, 0)
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/caves_world__tests.rs"]
mod tests;
