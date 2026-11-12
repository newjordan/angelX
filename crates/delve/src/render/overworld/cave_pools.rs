// Pool surfaces. World pixels and the run tick only: nothing here is written
// on the wire. Included into `cave_paint`, so names resolve there.

#[cfg(feature = "testing")]
use std::cell::Cell as Flag;

/// Before-shots in the proof ask for the flat fluid squares. A live seat never sets this.
#[cfg(feature = "testing")]
thread_local! {
    static FLAT_POOLS: Flag<bool> = const { Flag::new(false) };
}

#[cfg(feature = "testing")]
pub(crate) fn flat_pools(on: bool) {
    FLAT_POOLS.with(|flag| flag.set(on));
}

pub(crate) fn pools_flat() -> bool {
    #[cfg(feature = "testing")]
    {
        return FLAT_POOLS.with(|flag| flag.get());
    }
    #[cfg(not(feature = "testing"))]
    {
        false
    }
}

/// The proof steps a cave the change log cannot keep (fire, steam, integrity)
/// and asks the seat to paint that cell state. Empty unless a proof set it.
#[cfg(feature = "testing")]
thread_local! {
    static PAINT_OVER: RefCell<Option<Vec<Cell>>> = const { RefCell::new(None) };
}

#[cfg(feature = "testing")]
pub(crate) fn set_paint_over(cells: Option<Vec<Cell>>) {
    PAINT_OVER.with(|slot| *slot.borrow_mut() = cells);
}

#[cfg(feature = "testing")]
pub(crate) fn apply_paint_over(cave: &mut Cave) {
    PAINT_OVER.with(|slot| {
        let held = slot.borrow();
        let Some(cells) = held.as_ref() else {
            return;
        };
        if cells.len() != cave.world.cells.len() {
            return;
        }
        for (dst, src) in cave.world.cells.iter_mut().zip(cells.iter()) {
            dst.fire = src.fire;
            dst.steam = src.steam;
            dst.integrity = src.integrity;
            dst.flags = src.flags;
            dst.fluid = src.fluid;
            dst.amount = src.amount;
            dst.melt = src.melt;
        }
    });
}

pub(crate) fn pool_or_floor(cave: &Cave, wx: i32, wy: i32, cell: &Cell, tick: u32) -> Option<char> {
    if cell.wall() || cell.sealed() || cell.amount == 0 {
        return None;
    }
    if !matches!(cell.fluid, FLUID_LAVA | FLUID_WATER | FLUID_ACID) {
        return None;
    }
    if shallow_floor(cave, wx, wy, cell) {
        return None;
    }
    Some(match cell.fluid {
        FLUID_LAVA => lava_pool(cave, wx, wy, cell, tick),
        FLUID_WATER => water_pool(cave, wx, wy, tick),
        _ => acid_pool(cave, wx, wy, tick),
    })
}

fn drift_px(tick: u32) -> i32 {
    (tick / 12) as i32
}

fn blob(wx: i32, wy: i32, tick: u32, scale: i32, seed: u32) -> f32 {
    let x = (wx - drift_px(tick)) as f32 / scale as f32;
    let y = wy as f32 / scale as f32;
    vnoise(x, y, seed)
}

fn lava_cooling(cave: &Cave, wx: i32, wy: i32, cell: &Cell) -> bool {
    if cell.melt > 0 || cell.steam > 0 {
        return true;
    }
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (2, 0), (-2, 0), (0, 2), (0, -2), (3, 0), (-3, 0), (0, 3), (0, -3)] {
        let nx = wx + dx;
        let ny = wy + dy;
        let ncx = nx.div_euclid(CELL);
        let ncy = ny.div_euclid(CELL);
        if cave.at(ncx, ncy).is_some_and(|c| c.fluid == FLUID_WATER && c.amount > 0 && !c.wall()) {
            return true;
        }
    }
    false
}

fn lava_pool(cave: &Cave, wx: i32, wy: i32, cell: &Cell, tick: u32) -> char {
    // Molten gold channels between dark crust plates, a red rim where crust meets melt.
    // Scale 4 so even a one- or two-cell pool carries several plates.
    let seed = cave.seed.max(1);
    let n = blob(wx, wy, tick, 4, seed ^ 0x1a7a);
    if lava_cooling(cave, wx, wy, cell) {
        // Cooling: the plates grow and darken into a glassy crust, the melt sinks to dull red.
        if hash(wx, wy, 7) % 41 == 0 {
            return 'U';
        }
        if n > 0.38 {
            return if n > 0.62 { 'g' } else { 'X' };
        }
        return if n > 0.30 { '8' } else { '7' };
    }
    if n > 0.64 {
        return 'Z';
    }
    if n > 0.52 {
        return '8';
    }
    if n > 0.46 {
        return '7';
    }
    // the melt: gold, a few hotter flecks that move with the drift
    if hash(wx - drift_px(tick), wy, 5) % 23 == 0 {
        return '5';
    }
    '4'
}

fn water_pool(cave: &Cave, wx: i32, wy: i32, tick: u32) -> char {
    if water_ripple(wx, wy, tick) {
        return 'U';
    }
    if water_against_rock(cave, wx, wy) {
        return 's';
    }
    'S'
}

fn water_against_rock(cave: &Cave, wx: i32, wy: i32) -> bool {
    solid_pixel(cave, wx - 1, wy)
        || solid_pixel(cave, wx + 1, wy)
        || solid_pixel(cave, wx, wy - 1)
        || solid_pixel(cave, wx, wy + 1)
}

fn water_ripple(wx: i32, wy: i32, tick: u32) -> bool {
    let phase = tick / 16;
    let drift = (tick / 4) as i32;
    let x = wx - drift;
    let h = hash(x.div_euclid(8), wy.div_euclid(6), phase ^ 0x51);
    if h % 5 != 0 {
        return false;
    }
    if tick % 16 >= 12 {
        return false;
    }
    if tick.wrapping_add(h) % 3 == 0 {
        return false;
    }
    let len = 2 + (h % 2) as i32;
    let ox = (h % 4) as i32;
    let row = ((h / 11) % 5) as i32;
    let lx = x.rem_euclid(8);
    let ly = wy.rem_euclid(6);
    ly == row && lx >= ox && lx < ox + len
}

fn acid_pool(cave: &Cave, wx: i32, wy: i32, tick: u32) -> char {
    let n = blob(wx, wy, tick, 3, cave.seed.max(1) ^ 0xac1d);
    if hash(wx, wy, tick / 8) % 19 == 0 {
        return 'Y';
    }
    if n > 0.62 {
        'M'
    } else {
        'E'
    }
}

fn shallow_floor(cave: &Cave, wx: i32, wy: i32, cell: &Cell) -> bool {
    let amount = cell.amount;
    if amount >= 6 {
        return false;
    }
    let band = match amount {
        1 => 3,
        2 => 2,
        3 => 2,
        4 => 1,
        _ => 1,
    };
    if rim_dist(cave, wx, wy, cell.fluid) >= band {
        return false;
    }
    let cut = match amount {
        1 => 0.22,
        2 => 0.38,
        3 => 0.52,
        4 => 0.70,
        _ => 0.82,
    };
    bayer(wx, wy) > cut
}

fn rim_dist(cave: &Cave, wx: i32, wy: i32, fluid: u8) -> i32 {
    let mut best = 4;
    for dy in -3i32..=3 {
        for dx in -3i32..=3 {
            let d = dx.abs().max(dy.abs());
            if d == 0 || d >= best {
                continue;
            }
            let nx = wx + dx;
            let ny = wy + dy;
            let same = cave.at(nx.div_euclid(CELL), ny.div_euclid(CELL)).is_some_and(|c| {
                c.fluid == fluid && c.amount > 0 && !c.wall()
            });
            if !same {
                best = d;
            }
        }
    }
    best
}
