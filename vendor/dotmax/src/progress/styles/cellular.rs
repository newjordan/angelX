//! Cellular-automata progress bars.
//!
//! Every style in this module is a pure function of `(ctx.eased, ctx.time)` —
//! no mutable state is stored between frames. The CA state at each frame is
//! recomputed from a fixed initial condition, with the number of generations
//! evolved being a deterministic function of `eased` (for progress) and/or
//! `time` (for animation).
//!
//! Included algorithms:
//! - Wolfram elementary 1D CA (Rule 30, Rule 90, Rule 110, Rule 184, Rule 54, Rule 150)
//! - Conway's Game of Life (glider / r-pentomino seeds)
//! - Brian's Brain (3-state excitable CA)
//! - Langton's Ant
//! - Gray-Scott reaction-diffusion (approximated)
//! - Cyclic / rock-paper-scissors CA
//! - Wireworld electron loop
//! - Forest fire model

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};

// ---------------------------------------------------------------------------
// Deterministic hash (no external crates)
// ---------------------------------------------------------------------------

#[inline]
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

#[inline]
fn hash_f(n: u32) -> f32 {
    (hash(n) % 1000) as f32 / 1000.0
}

/// Mix two seeds into a hash (useful for 2D coordinates).
#[inline]
fn hash2(x: u32, y: u32) -> u32 {
    hash(x.wrapping_mul(1_000_003).wrapping_add(y))
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Theme tint — petri-dish green.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(96, 226, 130);
const TINT_END: Color = Color::rgb(24, 128, 88);

/// Sample the theme gradient at `t` in `0.0..=1.0`.
fn sample_tint(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
    Color::rgb(
        lerp(TINT_START.r, TINT_END.r),
        lerp(TINT_START.g, TINT_END.g),
        lerp(TINT_START.b, TINT_END.b),
    )
}

/// Applies the theme's signature gradient to every cell the inner style drew,
/// drifting slowly with `time`. Styles stay monochrome-safe underneath: drop
/// the wrapper in [`styles`] for uncolored output.
struct Tinted<S>(S);

impl<S: ProgressStyle> ProgressStyle for Tinted<S> {
    fn name(&self) -> &str {
        self.0.name()
    }
    fn theme(&self) -> &str {
        self.0.theme()
    }
    fn describe(&self) -> &str {
        self.0.describe()
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        self.0.render(grid, ctx)?;
        grid.enable_color_support();
        let (w, h) = grid.dimensions();
        for y in 0..h {
            for x in 0..w {
                let ch = grid.get_char(x, y);
                if ch != '\u{2800}' && ch != ' ' {
                    let t = (x as f32 / w.max(1) as f32 + ctx.time * 0.05).fract();
                    let tri = 1.0 - (2.0 * t - 1.0).abs();
                    let _ = grid.set_cell_color(x, y, sample_tint(tri));
                }
            }
        }
        Ok(())
    }
}

/// All styles in the `cellular` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per distinct cellular-automaton style.
/// Every style's `theme()` returns `"cellular"` and every `name()` is unique
/// within this theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(WolframRule { rule: 30 })),
        Box::new(Tinted(WolframRule { rule: 90 })),
        Box::new(Tinted(WolframRule { rule: 110 })),
        Box::new(Tinted(WolframRule { rule: 184 })),
        Box::new(Tinted(WolframRule { rule: 54 })),
        Box::new(Tinted(WolframRule { rule: 150 })),
        Box::new(Tinted(GameOfLife {
            seed: GoLSeed::Glider,
        })),
        Box::new(Tinted(GameOfLife {
            seed: GoLSeed::RPentomino,
        })),
        Box::new(Tinted(BriansBrain)),
        Box::new(Tinted(LangtonsAnt)),
        Box::new(Tinted(CyclicCA)),
        Box::new(Tinted(Wireworld)),
        Box::new(Tinted(GrayScott)),
        Box::new(Tinted(ForestFire)),
    ]
}

// ===========================================================================
// 1–6: Wolfram Elementary 1D CA (Rules 30, 90, 110, 184, 54, 150)
// ===========================================================================
//
// Layout: the dot grid is treated as a 1D tape (width) evolving over time
// (height). Row 0 is the initial condition (single centre-cell alive). Each
// subsequent row is one generation. We reveal rows top-to-bottom up to
// `ceil(eased * h)` rows. Time shifts a horizontal scroll offset so the
// pattern slowly drifts when the bar is held at a fixed progress.

struct WolframRule {
    rule: u8,
}

impl WolframRule {
    /// Apply one generation of the elementary CA rule.
    /// `row` must be length `w`; returns a new row of the same length.
    fn step(row: &[u8], rule: u8) -> Vec<u8> {
        let w = row.len();
        (0..w)
            .map(|i| {
                let l = if i == 0 { row[w - 1] } else { row[i - 1] };
                let c = row[i];
                let r = if i + 1 == w { row[0] } else { row[i + 1] };
                let pattern = (l << 2) | (c << 1) | r;
                (rule >> pattern) & 1
            })
            .collect()
    }
}

impl ProgressStyle for WolframRule {
    fn name(&self) -> &str {
        match self.rule {
            30 => "ca-rule30",
            90 => "ca-rule90",
            110 => "ca-rule110",
            184 => "ca-rule184",
            54 => "ca-rule54",
            150 => "ca-rule150",
            _ => "ca-wolfram",
        }
    }

    fn theme(&self) -> &str {
        "cellular"
    }

    fn describe(&self) -> &str {
        match self.rule {
            30 => "Rule 30: chaotic braille texture — Wolfram's fractal entropy engine",
            90 => "Rule 90: Sierpinski triangle — XOR diffusion fills the bar row by row",
            110 => "Rule 110: Turing-complete edge of chaos — complex local structures emerge",
            184 => "Rule 184: traffic flow CA — particles drift rightward with eased density",
            54 => "Rule 54: nested gliders — quasi-periodic wave fronts cascade down the bar",
            150 => "Rule 150: additive XOR diffusion — Pascal's triangle mod 2 in braille",
            _ => "Wolfram elementary CA",
        }
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let w = w.max(1);
        let h = h.max(1);

        // Rows to reveal: 0 = empty, h = fully revealed.
        let reveal = (ctx.eased * h as f32).ceil() as usize;
        // Slow horizontal drift driven by time (wraps around tape).
        let scroll = (ctx.time * 0.3) as usize % w.max(1);

        // Seed: single live cell at the centre.
        let mut row = vec![0u8; w];
        row[w / 2] = 1;

        for gen in 0..reveal.min(h) {
            // Draw this generation into dot row `gen`.
            for x in 0..w {
                let sx = (x + scroll) % w;
                if row[sx] == 1 {
                    draw::dot(grid, x, gen);
                }
            }
            row = Self::step(&row, self.rule);
        }

        Ok(())
    }
}

// ===========================================================================
// 7–8: Conway's Game of Life
// ===========================================================================
//
// We run GoL on a cell grid of `w × h` dot-pixels (not braille cells).
// Seed is placed at a canonical position. Generations = floor(time * speed).
// `eased` gates how much of the grid is rendered (left-to-right reveal mask).

#[derive(Clone, Copy)]
enum GoLSeed {
    Glider,
    RPentomino,
}

struct GameOfLife {
    seed: GoLSeed,
}

impl GameOfLife {
    fn make_board(w: usize, h: usize, seed: GoLSeed) -> Vec<Vec<bool>> {
        let mut board = vec![vec![false; w]; h];
        match seed {
            GoLSeed::Glider => {
                // Standard glider pattern, placed top-left.
                // . X .
                // . . X
                // X X X
                let patterns: &[(usize, usize)] = &[(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)];
                for &(px, py) in patterns {
                    if px < w && py < h {
                        board[py][px] = true;
                    }
                }
            }
            GoLSeed::RPentomino => {
                // r-pentomino: . X X / X X . / . X .  — centred.
                let cx = w / 2;
                let cy = h / 2;
                let patterns: &[(i32, i32)] = &[(1, -1), (2, -1), (0, 0), (1, 0), (1, 1)];
                for &(dx, dy) in patterns {
                    let px = cx as i32 + dx;
                    let py = cy as i32 + dy;
                    if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                        board[py as usize][px as usize] = true;
                    }
                }
            }
        }
        board
    }

    fn step(board: &[Vec<bool>]) -> Vec<Vec<bool>> {
        let h = board.len();
        let w = if h == 0 { 0 } else { board[0].len() };
        let mut next = vec![vec![false; w]; h];
        for y in 0..h {
            for x in 0..w {
                let mut n = 0u8;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                        let ny = (y as i32 + dy).rem_euclid(h as i32) as usize;
                        if board[ny][nx] {
                            n += 1;
                        }
                    }
                }
                next[y][x] = if board[y][x] {
                    n == 2 || n == 3
                } else {
                    n == 3
                };
            }
        }
        next
    }
}

impl ProgressStyle for GameOfLife {
    fn name(&self) -> &str {
        match self.seed {
            GoLSeed::Glider => "ca-gol-glider",
            GoLSeed::RPentomino => "ca-gol-rpentomino",
        }
    }

    fn theme(&self) -> &str {
        "cellular"
    }

    fn describe(&self) -> &str {
        match self.seed {
            GoLSeed::Glider =>
                "Game of Life glider — a diagonal spaceship animates across the bar as progress fills",
            GoLSeed::RPentomino =>
                "Game of Life r-pentomino — explosive chaotic growth, progress gates the reveal column",
        }
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let w = w.max(1);
        let h = h.max(1);

        // Speed: glider moves ~1 cell/5 gens; r-pentomino expands faster.
        let speed = match self.seed {
            GoLSeed::Glider => 4.0f32,
            GoLSeed::RPentomino => 8.0f32,
        };
        let gens = (ctx.time * speed) as usize;
        // Cap at a reasonable bound to prevent runaway cost.
        let gens = gens.min(500);

        let mut board = Self::make_board(w, h, self.seed);
        for _ in 0..gens {
            board = Self::step(&board);
        }

        // eased controls a left-to-right reveal column.
        let reveal_x = (ctx.eased * w as f32).round() as usize;

        for y in 0..h.min(board.len()) {
            for x in 0..reveal_x.min(w) {
                if x < board[y].len() && board[y][x] {
                    draw::dot(grid, x, y);
                }
            }
        }

        // A sparse live population can leave the bar unreadable, so a dotted
        // baseline plus an edge tick carries the progress reading.
        for x in (0..reveal_x.min(w)).step_by(3) {
            draw::dot(grid, x, h - 1);
        }
        let edge = reveal_x.min(w.saturating_sub(1));
        draw::vline(grid, edge, h.saturating_sub(3), h - 1);

        Ok(())
    }
}

// ===========================================================================
// 9: Brian's Brain
// ===========================================================================
//
// 3-state excitable CA: OFF(0), FIRING(1), REFRACTORY(2).
// A cell fires if exactly 2 FIRING neighbours; fires→refractory→off.
// Seeded with a deterministic hash pattern. Generations driven by time.

struct BriansBrain;

impl BriansBrain {
    fn initial(w: usize, h: usize) -> Vec<Vec<u8>> {
        let mut board = vec![vec![0u8; w]; h];
        // Scatter some firing seeds using the hash function.
        for y in 0..h {
            for x in 0..w {
                let v = hash2(x as u32, y as u32) % 5;
                board[y][x] = if v == 0 { 1 } else { 0 };
            }
        }
        board
    }

    fn step(board: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let h = board.len();
        let w = if h == 0 { 0 } else { board[0].len() };
        let mut next = vec![vec![0u8; w]; h];
        for y in 0..h {
            for x in 0..w {
                let state = board[y][x];
                next[y][x] = match state {
                    1 => 2, // firing → refractory
                    2 => 0, // refractory → off
                    _ => {
                        // off → firing if exactly 2 firing neighbours
                        let mut n = 0u8;
                        for dy in -1i32..=1 {
                            for dx in -1i32..=1 {
                                if dx == 0 && dy == 0 {
                                    continue;
                                }
                                let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                                let ny = (y as i32 + dy).rem_euclid(h as i32) as usize;
                                if board[ny][nx] == 1 {
                                    n += 1;
                                }
                            }
                        }
                        if n == 2 {
                            1
                        } else {
                            0
                        }
                    }
                };
            }
        }
        next
    }
}

impl ProgressStyle for BriansBrain {
    fn name(&self) -> &str {
        "ca-brians-brain"
    }
    fn theme(&self) -> &str {
        "cellular"
    }
    fn describe(&self) -> &str {
        "Brian's Brain: 3-state excitable CA — firing waves pulse and swirl as progress advances"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let w = w.max(1);
        let h = h.max(1);

        let gens = ((ctx.time * 6.0) as usize).min(300);
        let mut board = Self::initial(w, h);
        for _ in 0..gens {
            board = Self::step(&board);
        }

        // progress controls a diagonal reveal: cells with (x+y)/max_sum <= eased are shown.
        let max_sum = (w + h).saturating_sub(2).max(1);
        for y in 0..h.min(board.len()) {
            for x in 0..w {
                let reveal_frac = (x + y) as f32 / max_sum as f32;
                if reveal_frac <= ctx.eased {
                    if x < board[y].len() && board[y][x] == 1 {
                        draw::dot(grid, x, y);
                    }
                }
            }
        }

        Ok(())
    }
}

// ===========================================================================
// 10: Langton's Ant
// ===========================================================================
//
// Ant turns right on white (0), left on black (1), flips cell, moves forward.
// We run `eased * MAX_STEPS` steps from a blank grid, then render the trail.
// `time` adds a small periodic "wander" offset to produce animation.

struct LangtonsAnt;

impl LangtonsAnt {
    const MAX_STEPS: usize = 2000;
}

impl ProgressStyle for LangtonsAnt {
    fn name(&self) -> &str {
        "ca-langtons-ant"
    }
    fn theme(&self) -> &str {
        "cellular"
    }
    fn describe(&self) -> &str {
        "Langton's Ant: deterministic trail grows with progress — the highway emerges near 10,000 steps"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let w = w.max(1);
        let h = h.max(1);

        let steps = (ctx.eased * Self::MAX_STEPS as f32) as usize;
        // Time animates a slow starting offset (cycles through small offsets).
        let time_offset = (ctx.time * 0.5) as usize % 8;
        let steps = steps.saturating_add(time_offset).min(Self::MAX_STEPS + 8);

        let mut cells = vec![0u8; w * h];
        // Ant starts at centre.
        let mut ax = (w / 2) as i32;
        let mut ay = (h / 2) as i32;
        // Direction: 0=up, 1=right, 2=down, 3=left.
        let mut dir = 0i32;

        for _ in 0..steps {
            // Clamp ant to grid (toroidal wrap).
            ax = ax.rem_euclid(w as i32);
            ay = ay.rem_euclid(h as i32);
            let idx = ay as usize * w + ax as usize;
            let c = cells[idx];
            if c == 0 {
                dir = (dir + 1).rem_euclid(4); // turn right
                cells[idx] = 1;
            } else {
                dir = (dir - 1).rem_euclid(4); // turn left
                cells[idx] = 0;
            }
            // Move forward.
            match dir {
                0 => ay -= 1,
                1 => ax += 1,
                2 => ay += 1,
                _ => ax -= 1,
            }
        }

        for y in 0..h {
            for x in 0..w {
                if cells[y * w + x] == 1 {
                    draw::dot(grid, x, y);
                }
            }
        }

        Ok(())
    }
}

// ===========================================================================
// 11: Cyclic / Rock-Paper-Scissors CA
// ===========================================================================
//
// Each cell holds a state in [0, N). A cell advances (+1 mod N) if at least
// one of its 8 neighbours is in the next state. Spiral waves emerge.
// N=8 states. Driven by time for animation, eased for density of initial fill.

struct CyclicCA;

impl CyclicCA {
    const N_STATES: u8 = 8;
    const THRESHOLD: usize = 3; // neighbours needed to advance

    fn initial(w: usize, h: usize, density_seed: u32) -> Vec<Vec<u8>> {
        let n = Self::N_STATES;
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| {
                        // Use density_seed to vary initial state distribution.
                        let v = hash2(x as u32 ^ density_seed, y as u32);
                        (v % n as u32) as u8
                    })
                    .collect()
            })
            .collect()
    }

    fn step(board: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let h = board.len();
        let w = if h == 0 { 0 } else { board[0].len() };
        let n = CyclicCA::N_STATES;
        let thresh = CyclicCA::THRESHOLD;
        let mut next = board.to_vec();
        for y in 0..h {
            for x in 0..w {
                let cur = board[y][x];
                let target = (cur + 1) % n;
                let mut count = 0usize;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                        let ny = (y as i32 + dy).rem_euclid(h as i32) as usize;
                        if board[ny][nx] == target {
                            count += 1;
                        }
                    }
                }
                if count >= thresh {
                    next[y][x] = target;
                }
            }
        }
        next
    }
}

impl ProgressStyle for CyclicCA {
    fn name(&self) -> &str {
        "ca-cyclic"
    }
    fn theme(&self) -> &str {
        "cellular"
    }
    fn describe(&self) -> &str {
        "Cyclic CA (rock-paper-scissors): spiral waves of 8 states — progress advances the generation count"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let w = w.max(1);
        let h = h.max(1);

        // Wave density is roughly constant at any generation count, so the
        // generation count alone cannot carry the progress reading. Run the
        // waves on time and gate the field behind a left-to-right reveal.
        let gens = 8 + (ctx.time * 3.0) as usize;
        let seed = 7u32;
        let mut board = Self::initial(w, h, seed);
        for _ in 0..gens.min(80) {
            board = Self::step(&board);
        }

        let reveal_x = (ctx.eased * w as f32).round() as usize;

        // Draw cells that are in "high" states (upper half of N_STATES range).
        let half = Self::N_STATES / 2;
        for y in 0..h.min(board.len()) {
            for x in 0..reveal_x.min(w) {
                if x < board[y].len() && board[y][x] >= half {
                    draw::dot(grid, x, y);
                }
            }
        }
        let edge = reveal_x.min(w.saturating_sub(1));
        draw::vline(grid, edge, 0, h - 1);

        Ok(())
    }
}

// ===========================================================================
// 12: Wireworld
// ===========================================================================
//
// 4 states: EMPTY(0), CONDUCTOR(1), ELECTRON_HEAD(2), ELECTRON_TAIL(3).
// Rules:
//   EMPTY       → EMPTY
//   CONDUCTOR   → ELECTRON_HEAD if 1 or 2 electron heads in neighbourhood, else CONDUCTOR
//   ELECTRON_HEAD → ELECTRON_TAIL
//   ELECTRON_TAIL → CONDUCTOR
//
// We draw a loop of conductor around the perimeter of the dot grid, inject
// an electron, and let it chase itself. `eased` controls how much of the
// perimeter the conductor covers; `time` drives the electron position.

struct Wireworld;

impl Wireworld {
    const EMPTY: u8 = 0;
    const CONDUCTOR: u8 = 1;
    const HEAD: u8 = 2;
    const TAIL: u8 = 3;

    fn build_grid(w: usize, h: usize, fill_frac: f32) -> Vec<Vec<u8>> {
        let mut board = vec![vec![Self::EMPTY; w]; h];
        // Perimeter loop: top, right, bottom (reversed), left (reversed).
        let perimeter: Vec<(usize, usize)> = {
            let mut p = Vec::new();
            // Top row left→right.
            for x in 0..w {
                p.push((x, 0));
            }
            // Right column top→bottom (skip corners already added).
            for y in 1..h {
                p.push((w.saturating_sub(1), y));
            }
            // Bottom row right→left (skip corner).
            if h > 1 {
                let y = h - 1;
                for x in (0..w.saturating_sub(1)).rev() {
                    p.push((x, y));
                }
            }
            // Left column bottom→top (skip corners).
            if w > 1 && h > 1 {
                for y in (1..h.saturating_sub(1)).rev() {
                    p.push((0, y));
                }
            }
            p
        };
        let total = perimeter.len().max(1);
        let lit = (fill_frac * total as f32) as usize;

        for i in 0..lit.min(total) {
            let (x, y) = perimeter[i];
            board[y][x] = Self::CONDUCTOR;
        }
        board
    }

    fn inject_electron(board: &mut Vec<Vec<u8>>, electron_pos: usize) {
        // Walk the perimeter to find the conductor cell at position `electron_pos`.
        let h = board.len();
        let w = if h == 0 { 0 } else { board[0].len() };
        let mut idx = 0usize;
        'outer: for pass in 0..2usize {
            // Top row.
            for x in 0..w {
                if board[0][x] == Self::CONDUCTOR {
                    if idx == electron_pos {
                        board[0][x] = if pass == 0 { Self::HEAD } else { Self::TAIL };
                        break 'outer;
                    }
                    idx += 1;
                }
            }
            // Right column.
            for y in 1..h {
                let rx = w.saturating_sub(1);
                if board[y][rx] == Self::CONDUCTOR {
                    if idx == electron_pos {
                        board[y][rx] = if pass == 0 { Self::HEAD } else { Self::TAIL };
                        break 'outer;
                    }
                    idx += 1;
                }
            }
            // Bottom row reversed.
            if h > 1 {
                let by = h - 1;
                for x in (0..w.saturating_sub(1)).rev() {
                    if board[by][x] == Self::CONDUCTOR {
                        if idx == electron_pos {
                            board[by][x] = if pass == 0 { Self::HEAD } else { Self::TAIL };
                            break 'outer;
                        }
                        idx += 1;
                    }
                }
            }
            // Left column reversed.
            if w > 1 && h > 1 {
                for y in (1..h.saturating_sub(1)).rev() {
                    if board[y][0] == Self::CONDUCTOR {
                        if idx == electron_pos {
                            board[y][0] = if pass == 0 { Self::HEAD } else { Self::TAIL };
                            break 'outer;
                        }
                        idx += 1;
                    }
                }
            }
            // Only one pass needed — if we're here electron_pos > total conductors.
            break;
        }
    }

    fn step(board: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let h = board.len();
        let w = if h == 0 { 0 } else { board[0].len() };
        let mut next = board.to_vec();
        for y in 0..h {
            for x in 0..w {
                next[y][x] = match board[y][x] {
                    Self::HEAD => Self::TAIL,
                    Self::TAIL => Self::CONDUCTOR,
                    Self::CONDUCTOR => {
                        let mut heads = 0u8;
                        for dy in -1i32..=1 {
                            for dx in -1i32..=1 {
                                if dx == 0 && dy == 0 {
                                    continue;
                                }
                                let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                                let ny = (y as i32 + dy).rem_euclid(h as i32) as usize;
                                if board[ny][nx] == Self::HEAD {
                                    heads += 1;
                                }
                            }
                        }
                        if heads == 1 || heads == 2 {
                            Self::HEAD
                        } else {
                            Self::CONDUCTOR
                        }
                    }
                    _ => Self::EMPTY,
                };
            }
        }
        next
    }
}

impl ProgressStyle for Wireworld {
    fn name(&self) -> &str {
        "ca-wireworld"
    }
    fn theme(&self) -> &str {
        "cellular"
    }
    fn describe(&self) -> &str {
        "Wireworld: an electron chases itself around a conductor perimeter — conductor grows with progress"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let w = w.max(1);
        let h = h.max(1);

        // Build conductor loop sized by eased.
        let mut board = Self::build_grid(w, h, ctx.eased);

        // Perimeter length (approximate) — used to position electron.
        let perimeter_len = (2 * (w + h)).saturating_sub(4).max(1);

        // Place HEAD at position driven by time.
        let head_pos = (ctx.time * 8.0) as usize % perimeter_len;
        let tail_pos = head_pos.saturating_sub(1) % perimeter_len;
        Self::inject_electron(&mut board, head_pos);
        Self::inject_electron(&mut board, tail_pos);

        // Step a few generations so the electron is properly propagated.
        for _ in 0..2 {
            board = Self::step(&board);
        }

        for y in 0..h.min(board.len()) {
            for x in 0..w {
                if x < board[y].len() {
                    match board[y][x] {
                        Self::CONDUCTOR => draw::dot(grid, x, y),
                        Self::HEAD => {
                            // Draw HEAD brighter by also dotting adjacent positions.
                            draw::dot(grid, x, y);
                            if x + 1 < w {
                                draw::dot(grid, x + 1, y);
                            }
                            if x > 0 {
                                draw::dot(grid, x.saturating_sub(1), y);
                            }
                        }
                        Self::TAIL => {} // tail is invisible (just went dark)
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }
}

// ===========================================================================
// 13: Gray-Scott reaction-diffusion
// ===========================================================================
//
// Two chemicals u, v diffuse and react: u + 2v -> 3v, with feed rate F and
// kill rate k. Seeded with a central blob and integrated for a number of
// steps that grows with `eased` — coral / mitosis labyrinths emerge. Stateless:
// the whole simulation is recomputed from the same seed every frame.

struct GrayScott;
impl ProgressStyle for GrayScott {
    fn name(&self) -> &str {
        "ca-gray-scott"
    }
    fn theme(&self) -> &str {
        "cellular"
    }
    fn describe(&self) -> &str {
        "Gray-Scott reaction-diffusion: coral/mitosis labyrinths grow as progress feeds the reaction"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let w = w.max(1);
        let h = h.max(1);
        let n = w * h;
        let mut u = vec![1.0f32; n];
        let mut v = vec![0.0f32; n];
        // Seed a small central square of v.
        let cx = w / 2;
        let cy = h / 2;
        for dy in 0..3 {
            for dx in 0..3 {
                let x = (cx + dx).saturating_sub(1);
                let y = (cy + dy).saturating_sub(1);
                if x < w && y < h {
                    v[y * w + x] = 1.0;
                    u[y * w + x] = 0.5;
                }
            }
        }
        const F: f32 = 0.055;
        const K: f32 = 0.062;
        const DU: f32 = 0.16;
        const DV: f32 = 0.08;
        let iters = ((ctx.eased * 60.0) as usize + 1).min(90);
        let mut un = u.clone();
        let mut vn = v.clone();
        for _ in 0..iters {
            for y in 0..h {
                let ym = if y == 0 { h - 1 } else { y - 1 };
                let yp = if y + 1 == h { 0 } else { y + 1 };
                for x in 0..w {
                    let xm = if x == 0 { w - 1 } else { x - 1 };
                    let xp = if x + 1 == w { 0 } else { x + 1 };
                    let i = y * w + x;
                    let lap_u =
                        u[y * w + xm] + u[y * w + xp] + u[ym * w + x] + u[yp * w + x] - 4.0 * u[i];
                    let lap_v =
                        v[y * w + xm] + v[y * w + xp] + v[ym * w + x] + v[yp * w + x] - 4.0 * v[i];
                    let uvv = u[i] * v[i] * v[i];
                    un[i] = (u[i] + DU * lap_u - uvv + F * (1.0 - u[i])).clamp(0.0, 1.0);
                    vn[i] = (v[i] + DV * lap_v + uvv - (F + K) * v[i]).clamp(0.0, 1.0);
                }
            }
            std::mem::swap(&mut u, &mut un);
            std::mem::swap(&mut v, &mut vn);
        }
        for y in 0..h {
            for x in 0..w {
                if v[y * w + x] > 0.25 {
                    draw::dot(grid, x, y);
                }
            }
        }
        Ok(())
    }
}

// ===========================================================================
// 14: Forest-fire model
// ===========================================================================
//
// Trees are scattered by a fixed hash; a fire front sweeps left-to-right with
// `eased` (the burn extent = progress). Trees ahead of the front stand;
// trees at the front flicker as flames (animated by `time`); behind it is ash.

struct ForestFire;
impl ProgressStyle for ForestFire {
    fn name(&self) -> &str {
        "ca-forest-fire"
    }
    fn theme(&self) -> &str {
        "cellular"
    }
    fn describe(&self) -> &str {
        "Forest-fire CA: a flame front burns through scattered trees as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let w = w.max(1);
        let h = h.max(1);
        let front = (ctx.eased * w as f32) as i32;
        for y in 0..h {
            for x in 0..w {
                // Tree if the hash for this cell clears the density threshold.
                if hash_f((y * w + x) as u32 ^ 0x9e37) >= 0.55 {
                    continue;
                }
                let d = x as i32 - front;
                if d > 1 {
                    // Standing tree ahead of the fire.
                    draw::dot(grid, x, y);
                    if y > 0 && hash_f((x * 7 + y) as u32) > 0.5 {
                        draw::dot(grid, x, y - 1);
                    }
                } else if d >= -2 {
                    // Flame front: flicker upward with time.
                    let flick = ((ctx.time * 12.0 + (x + y) as f32).sin() * 2.0) as i32;
                    draw::dot_i(grid, x as i32, y as i32 - flick.abs());
                    draw::dot(grid, x, y);
                }
                // Behind the front (d < -2): ash, drawn empty.
            }
        }
        Ok(())
    }
}
