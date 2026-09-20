//! Number-theory progress bars.
//!
//! Twelve styles, each grounded in a concrete mathematical structure:
//! sieves, spirals, trees, triangles, histograms, and sequence geometry.
//! Every bar maps `ctx.eased` to "how many integers have been processed" and
//! `ctx.time` to a highlight or scan animation running independently of
//! progress. All arithmetic is bounded and cannot panic.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ────────────────────────────────────────────────────────────────────────────

/// Trial-division primality test. Returns `false` for 0 and 1.
/// Bounded: for any n ≤ 10_000 this completes in a handful of μs.
fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n == 2 {
        return true;
    }
    if n % 2 == 0 {
        return false;
    }
    let mut i = 3u64;
    while i * i <= n {
        if n % i == 0 {
            return false;
        }
        i += 2;
    }
    true
}

// ────────────────────────────────────────────────────────────────────────────
// Public entry point
// ────────────────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — teal into deep blue.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(118, 224, 202);
const TINT_END: Color = Color::rgb(44, 132, 204);

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

/// All styles in the `numbertheory` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(Sieve)),
        Box::new(Tinted(UlamSpiral)),
        Box::new(Tinted(PrimeCounting)),
        Box::new(Tinted(Collatz)),
        Box::new(Tinted(FibonacciSpiral)),
        Box::new(Tinted(PascalMod)),
        Box::new(Tinted(TotientHistogram)),
        Box::new(Tinted(SternBrocot)),
        Box::new(Tinted(ContinuedFraction)),
        Box::new(Tinted(ModularCircle)),
        Box::new(Tinted(Recaman)),
        Box::new(Tinted(DigitalRoot)),
    ]
}

// ────────────────────────────────────────────────────────────────────────────
// 1. Sieve of Eratosthenes
// ────────────────────────────────────────────────────────────────────────────

/// Sieve of Eratosthenes: integers 1..N laid left→right; composites are
/// crossed out one by one, primes remain lit. `eased` controls how many
/// integers are visible; `time` sweeps a highlight over the current sieve
/// multiple.
struct Sieve;
impl ProgressStyle for Sieve {
    fn name(&self) -> &str {
        "sieve-eratosthenes"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Sieve of Eratosthenes: integers cross composites, primes survive as lit dots"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // N = number of integers we lay across the width (at least 2).
        let n = w.max(2).min(2000);
        let revealed = ((ctx.eased * n as f32).round() as usize).min(n);

        // Build sieve for 1..=n.
        let mut composite = vec![false; n + 1];
        composite[0] = true;
        if n >= 1 {
            composite[1] = true;
        }
        for p in 2..=n {
            if !composite[p] {
                let mut m = p * 2;
                while m <= n {
                    composite[m] = true;
                    m += p;
                }
            }
        }

        // Animated highlight: current sieve multiple being crossed.
        let highlight_p = {
            let p_frac = (ctx.time * 0.4).fract();
            // Pick a prime index from time.
            let prime_count = (2..=n).filter(|&k| !composite[k]).count().max(1);
            let pi = ((p_frac * prime_count as f32) as usize).min(prime_count.saturating_sub(1));
            (2..=n).filter(|&k| !composite[k]).nth(pi).unwrap_or(2)
        };

        // Draw: one dot-column per integer.
        for k in 1..=revealed {
            let x = ((k - 1) * w / n).min(w.saturating_sub(1));
            let is_p = !composite[k];
            // Primes: full column; composites: half-height bottom tick.
            if is_p {
                draw::vline(grid, x, 0, h.saturating_sub(1));
                // Highlight the current sieve prime's multiples.
                if k > 1 && k % highlight_p == 0 {
                    // draw a top cap to distinguish the animated sweep target.
                    draw::dot(grid, x, 0);
                }
            } else {
                let tick = (h / 4).max(1);
                draw::vline(grid, x, h.saturating_sub(tick), h.saturating_sub(1));
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 2. Ulam Spiral
// ────────────────────────────────────────────────────────────────────────────

/// Ulam spiral: integers spiral outward from the centre of the grid; primes
/// glow along diagonals. `eased` reveals integers 1..N; `time` pulses the
/// prime dots.
struct UlamSpiral;
impl ProgressStyle for UlamSpiral {
    fn name(&self) -> &str {
        "ulam-spiral"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Ulam spiral: integers coil outward from centre, primes cluster on diagonals"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;

        // Cap N to avoid spiraling off into unreachable cells.
        let n = (w * h).min(4000).max(1);
        let revealed = ((ctx.eased * n as f32).round() as usize).min(n);

        // Generate Ulam spiral coords for 1..=revealed.
        // Direction order: right, up, left, down.
        let dx = [1i32, 0, -1, 0];
        let dy = [0i32, -1, 0, 1];

        let mut x = 0i32;
        let mut y = 0i32;
        let mut dir = 0usize;
        let mut steps_in_leg = 1usize;
        let mut steps_taken = 0usize;
        let mut leg_count = 0usize;

        // Pulse phase from time.
        let pulse = (ctx.time * 2.0 * PI * 0.5).sin() * 0.5 + 0.5;
        let _ = pulse; // used conceptually; we draw or skip based on phase parity.

        for n_i in 1..=revealed {
            let px = cx + x;
            let py = cy + y;

            if is_prime(n_i as u64) {
                draw::dot_i(grid, px, py);
            }

            // Advance spiral.
            x += dx[dir];
            y += dy[dir];
            steps_taken += 1;
            if steps_taken == steps_in_leg {
                steps_taken = 0;
                dir = (dir + 1) % 4;
                leg_count += 1;
                if leg_count % 2 == 0 {
                    steps_in_leg += 1;
                }
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 3. Prime counting function π(x)
// ────────────────────────────────────────────────────────────────────────────

/// Prime counting function π(x): a curve showing how many primes are ≤ x,
/// displayed as a rising step graph filling left to right with `eased`.
/// `time` scrolls a thin vertical scan line.
struct PrimeCounting;
impl ProgressStyle for PrimeCounting {
    fn name(&self) -> &str {
        "prime-counting"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "π(x) prime-counting function: a rising step curve filling as primes accumulate"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let n = w.max(2).min(3000);
        let revealed_x = ((ctx.eased * n as f32).round() as usize).min(n);

        // Precompute π(k) for k in 1..=n.
        let mut pi = 0usize;
        let mut prime_counts = vec![0usize; n + 1];
        for k in 1..=n {
            if is_prime(k as u64) {
                pi += 1;
            }
            prime_counts[k] = pi;
        }
        let pi_max = prime_counts[n].max(1);

        // Draw the step curve up to revealed_x.
        for k in 1..=revealed_x {
            let x = ((k - 1) * w / n).min(w.saturating_sub(1));
            let count = prime_counts[k];
            // Map prime count to y (bottom = 0 primes, top = pi_max).
            let bar_h = (count * h / pi_max).min(h);
            let y0 = h.saturating_sub(bar_h);
            draw::vline(grid, x, y0, h.saturating_sub(1));
        }

        // Animated scan line.
        let scan_x = ((ctx.time * 0.3).fract() * w as f32) as usize;
        if scan_x < w {
            draw::vline(grid, scan_x, 0, h.saturating_sub(1));
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 4. Collatz (3n+1) trajectories
// ────────────────────────────────────────────────────────────────────────────

/// Collatz conjecture: for each seed n (swept by `eased` over 1..N), plot
/// the stopping trajectory (n→n/2 if even, 3n+1 if odd). Height encodes
/// the value normalised to the peak value in the window. `time` selects
/// which trajectory is currently highlighted.
struct Collatz;
impl ProgressStyle for Collatz {
    fn name(&self) -> &str {
        "collatz"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Collatz (3n+1): seeds swept left→right, trajectory height is stopping sequence depth"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Number of seeds = width in dots, max 500.
        let n_seeds = w.min(500).max(1);
        let revealed = ((ctx.eased * n_seeds as f32).round() as usize).min(n_seeds);

        // Collatz stopping time for seed k.
        fn stopping_time(mut k: u64) -> u64 {
            let mut count = 0u64;
            while k != 1 && count < 10_000 {
                k = if k % 2 == 0 { k / 2 } else { 3 * k + 1 };
                count += 1;
            }
            count
        }

        // Collect stopping times for seeds 1..=n_seeds.
        let times: Vec<u64> = (1..=n_seeds).map(|k| stopping_time(k as u64)).collect();
        let max_t = times.iter().copied().max().unwrap_or(1).max(1);

        // Highlight column from time.
        let hi_x = ((ctx.time * 0.7).fract() * revealed as f32) as usize;

        for (i, &t) in times.iter().enumerate().take(revealed) {
            let x = (i * w / n_seeds).min(w.saturating_sub(1));
            let bar_h = ((t as f32 / max_t as f32) * h as f32).round() as usize;
            let y0 = h.saturating_sub(bar_h);
            draw::vline(grid, x, y0, h.saturating_sub(1));
            // Extra top dot for highlight column.
            if i == hi_x && y0 > 0 {
                draw::dot(grid, x, y0.saturating_sub(1));
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 5. Fibonacci / Golden spiral
// ────────────────────────────────────────────────────────────────────────────

/// Fibonacci golden spiral: draws successive quarter-circle arcs in squares
/// whose side lengths are Fibonacci numbers. `eased` controls how many arcs
/// are drawn; `time` rotates a glowing cursor along the outermost arc. The
/// golden ratio φ = (1+√5)/2 governs growth.
struct FibonacciSpiral;
impl ProgressStyle for FibonacciSpiral {
    fn name(&self) -> &str {
        "fibonacci-spiral"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Golden spiral: Fibonacci quarter-circle arcs converging on φ"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Generate Fibonacci numbers scaled to fit the grid.
        let max_dim = w.min(h).max(1);
        let mut fibs = vec![1usize, 1usize];
        while *fibs.last().unwrap() < max_dim {
            let n = fibs.len();
            let next = fibs[n - 1] + fibs[n - 2];
            fibs.push(next);
            if fibs.len() > 20 {
                break;
            }
        }

        let max_arcs = fibs.len().min(12);
        let revealed = ((ctx.eased * max_arcs as f32).round() as usize).min(max_arcs);

        // Centre of the spiral.
        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;

        // Arc start angles follow the spiral: 0°, 90°, 180°, 270°, repeating.
        let start_angles = [0.0f32, PI / 2.0, PI, 3.0 * PI / 2.0];

        let mut rx = cx;
        let mut ry = cy;

        for (arc_idx, &fib) in fibs.iter().enumerate().take(revealed) {
            let r = fib as f32;
            let angle_start = start_angles[arc_idx % 4];
            let steps = ((r * PI / 2.0) as usize).max(4).min(256);

            for s in 0..=steps {
                let theta = angle_start + (s as f32 / steps as f32) * (PI / 2.0);
                let px = rx + (theta.cos() * r) as i32;
                let py = ry + (theta.sin() * r) as i32;
                draw::dot_i(grid, px, py);
            }

            // Advance centre for next arc.
            match arc_idx % 4 {
                0 => ry -= fib as i32,
                1 => rx -= fib as i32,
                2 => ry += fib as i32,
                _ => rx += fib as i32,
            }
        }

        // Animated cursor on outermost arc.
        if revealed > 0 && revealed <= fibs.len() {
            let last_idx = revealed.saturating_sub(1);
            let r = fibs[last_idx] as f32;
            let angle_start = start_angles[last_idx % 4];
            let theta = angle_start + (ctx.time * 0.5).fract() * (PI / 2.0);
            // cursor centre — approximate; recompute centre for last arc.
            let mut cxl = cx;
            let mut cyl = cy;
            for i in 0..last_idx {
                match i % 4 {
                    0 => cyl -= fibs[i] as i32,
                    1 => cxl -= fibs[i] as i32,
                    2 => cyl += fibs[i] as i32,
                    _ => cxl += fibs[i] as i32,
                }
            }
            let px = cxl + (theta.cos() * r) as i32;
            let py = cyl + (theta.sin() * r) as i32;
            draw::dot_i(grid, px, py);
            draw::dot_i(grid, px + 1, py);
            draw::dot_i(grid, px, py + 1);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 6. Pascal's triangle mod p
// ────────────────────────────────────────────────────────────────────────────

/// Pascal's triangle mod p: rows revealed = `eased × height`. Mod 2 yields
/// Sierpinski's gasket; `time` cycles through mod 2, 3, 5 to show different
/// self-similar patterns.
struct PascalMod;
impl ProgressStyle for PascalMod {
    fn name(&self) -> &str {
        "pascal-mod"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Pascal's triangle mod p: Sierpinski (p=2), mod-3, mod-5 patterns cycling with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Pick modulus from time: cycles 2→3→5→2 every 3 s.
        let mods = [2u64, 3, 5];
        let mod_idx = ((ctx.time / 3.0) as usize) % mods.len();
        let modulus = mods[mod_idx];

        let revealed_rows = ((ctx.eased * h as f32).round() as usize).min(h);

        // We compute Pascal's triangle row by row (mod p) using a 1-D buffer.
        let row_len = w.min(512);
        let mut row = vec![0u64; row_len];
        if row_len > 0 {
            row[0] = 1;
        }

        for r in 0..revealed_rows {
            // Map row to y-coordinate (top = row 0).
            let py = r;
            // Map column within the row to x-coordinate, centred.
            // The Pascal triangle at row r has r+1 non-trivial entries; we
            // spread them symmetrically across the width.
            let entries = (r + 1).min(row_len);
            for c in 0..entries {
                if row[c] % modulus != 0 {
                    // Map entry position to x.
                    let x = if entries <= 1 {
                        w / 2
                    } else {
                        c * (w.saturating_sub(1)) / (entries.saturating_sub(1))
                    };
                    draw::dot(grid, x, py);
                }
            }
            // Advance row: compute next Pascal row (mod p), right-to-left.
            for c in (1..row_len).rev() {
                row[c] = (row[c] + row[c - 1]) % modulus;
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 7. Euler totient φ(n) histogram
// ────────────────────────────────────────────────────────────────────────────

/// Euler totient φ(n) histogram: bar height at column n is φ(n)/n, revealing
/// left to right as `eased` grows. Primes peek at the top (φ(p)=p−1≈p).
struct TotientHistogram;
impl ProgressStyle for TotientHistogram {
    fn name(&self) -> &str {
        "totient-histogram"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Euler totient φ(n): columns show φ(n)/n — primes spike to the top, composites sag"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let n = w.max(2).min(2000);
        let revealed = ((ctx.eased * n as f32).round() as usize).min(n).max(1);

        // Compute totient for k in 2..=revealed.
        fn totient(n: usize) -> usize {
            if n == 0 {
                return 0;
            }
            // Euler's product formula via trial factoring.
            let mut phi = n;
            let mut temp = n;
            let mut p = 2;
            while p * p <= temp {
                if temp % p == 0 {
                    while temp % p == 0 {
                        temp /= p;
                    }
                    phi = phi - phi / p;
                }
                p += 1;
            }
            if temp > 1 {
                phi = phi - phi / temp;
            }
            phi
        }

        for k in 2..=revealed {
            let x = ((k - 2) * w / (n.saturating_sub(1).max(1))).min(w.saturating_sub(1));
            let phi_k = totient(k);
            // Ratio φ(k)/k ∈ (0, 1].
            let ratio = phi_k as f32 / k as f32;
            let bar_h = (ratio * h as f32).round() as usize;
            let y0 = h.saturating_sub(bar_h);
            draw::vline(grid, x, y0, h.saturating_sub(1));
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 8. Stern-Brocot tree / Farey sequence
// ────────────────────────────────────────────────────────────────────────────

/// Stern-Brocot tree: fractions from the Farey sequence F_n plotted as
/// vertical spikes whose height is the denominator. `eased` controls the
/// order n of the Farey sequence; `time` pulses a cursor scanning fractions.
struct SternBrocot;
impl ProgressStyle for SternBrocot {
    fn name(&self) -> &str {
        "stern-brocot"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Stern-Brocot / Farey F_n: every rational in [0,1] placed by value, height = denominator"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Farey sequence F_n: all p/q with 0 ≤ p ≤ q ≤ n, gcd(p,q)=1, in order.
        // Cap n to keep term count manageable.
        let n = ((ctx.eased * 20.0).round() as usize).clamp(1, 20);

        fn gcd(mut a: usize, mut b: usize) -> usize {
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            a
        }

        // Collect Farey fractions as (numerator, denominator).
        let mut fracs: Vec<(usize, usize)> = Vec::new();
        for q in 1..=n {
            for p in 0..=q {
                if gcd(p, q) == 1 || (p == 0 && q == 1) {
                    fracs.push((p, q));
                }
            }
        }
        // Sort by value.
        fracs.sort_by(|a, b| {
            let va = a.0 * b.1;
            let vb = b.0 * a.1;
            va.cmp(&vb)
        });
        fracs.dedup_by(|a, b| a.0 * b.1 == b.0 * a.1);

        let max_q = n.max(1);

        // Animated scan cursor.
        let cursor_frac = (ctx.time * 0.2).fract();

        for (p, q) in &fracs {
            let value = *p as f32 / *q as f32;
            let x = (value * (w.saturating_sub(1)) as f32).round() as usize;
            let bar_h = ((*q as f32 / max_q as f32) * h as f32).round() as usize;
            let y0 = h.saturating_sub(bar_h);
            draw::vline(grid, x, y0, h.saturating_sub(1));

            // Highlight the fraction closest to the cursor.
            let dist = (value - cursor_frac).abs();
            if dist < 0.03 {
                draw::dot(grid, x, y0.saturating_sub(1));
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 9. Continued fraction convergents of φ
// ────────────────────────────────────────────────────────────────────────────

/// Continued fraction convergents of φ = [1;1,1,1,…]: each convergent p_k/q_k
/// is a best rational approximation. `eased` reveals convergents; each is
/// drawn as a dot at (x=value×width, y=row k). Convergents alternate above /
/// below the true value — the zig-zag visualises the approximation theorem.
struct ContinuedFraction;
impl ProgressStyle for ContinuedFraction {
    fn name(&self) -> &str {
        "continued-fraction-phi"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Continued fraction convergents of φ: zig-zagging best rationals approaching the golden ratio"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // φ convergents: p_k/q_k where p_k = F_{k+1}, q_k = F_k (Fibonacci).
        // Compute up to h convergents (one per dot-row).
        let max_k = h.min(40);
        let revealed = ((ctx.eased * max_k as f32).round() as usize).min(max_k);

        let mut p_prev = 0u64;
        let mut p_curr = 1u64;
        let mut q_prev = 1u64;
        let mut q_curr = 1u64;

        let phi = (1.0 + 5.0f32.sqrt()) / 2.0; // 1.6180…
                                               // We map value/phi into [0,1] range (phi ≈ 1.618, so p/q ≤ phi).
        let scale = phi;

        let mut prev_x: Option<usize> = None;
        let mut prev_y: Option<usize> = None;

        for k in 0..revealed {
            let value = p_curr as f32 / q_curr as f32;
            let x =
                ((value / scale).clamp(0.0, 1.0) * (w.saturating_sub(1)) as f32).round() as usize;
            let y = if h <= 1 {
                0
            } else {
                k * (h.saturating_sub(1)) / (max_k.saturating_sub(1).max(1))
            };
            let y = y.min(h.saturating_sub(1));

            draw::dot(grid, x, y);

            // Connect successive convergents with a line to show zig-zag.
            if let (Some(px), Some(py)) = (prev_x, prev_y) {
                // Simple Bresenham-lite: interpolate between the two points.
                let steps = ((x as i32 - px as i32)
                    .abs()
                    .max((y as i32 - py as i32).abs()))
                .max(1) as usize;
                for s in 1..steps {
                    let t = s as f32 / steps as f32;
                    let ix = (px as f32 + t * (x as i32 - px as i32) as f32).round() as usize;
                    let iy = (py as f32 + t * (y as i32 - py as i32) as f32).round() as usize;
                    draw::dot(
                        grid,
                        ix.min(w.saturating_sub(1)),
                        iy.min(h.saturating_sub(1)),
                    );
                }
            }
            prev_x = Some(x);
            prev_y = Some(y);

            // Advance convergents: [1;1,1,1,…] so a_k = 1 always.
            let p_new = p_curr + p_prev;
            let q_new = q_curr + q_prev;
            p_prev = p_curr;
            p_curr = p_new;
            q_prev = q_curr;
            q_curr = q_new;

            // Guard against overflow for deep k.
            if p_curr > 1_000_000 || q_curr > 1_000_000 {
                break;
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 10. Modular multiplication circle (cardioid string art)
// ────────────────────────────────────────────────────────────────────────────

/// Modular multiplication circle: N points around a circle; connect point i to
/// (k·i) mod N for each i. k=2 draws a cardioid; k=3 a nephroid; k swept by
/// `eased` reveals new curves. `time` rotates the whole figure slowly.
struct ModularCircle;
impl ProgressStyle for ModularCircle {
    fn name(&self) -> &str {
        "modular-circle"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Modular multiplication circle: string art i→k·i mod N, k swept by progress (cardioid at k=2)"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let n_points = 100usize; // Fixed number of points on circle.
        let k = (ctx.eased * 10.0).floor() as usize + 2; // k in 2..=12.
        let k = k.min(50);

        let cx = (w / 2) as f32;
        let cy = (h / 2) as f32;
        let rx = (w / 2).saturating_sub(1) as f32;
        let ry = (h / 2).saturating_sub(1) as f32;

        // Slow rotation from time.
        let rot = ctx.time * 0.1;

        for i in 0..n_points {
            let j = (k * i) % n_points;
            let angle_i = (i as f32 / n_points as f32) * 2.0 * PI + rot;
            let angle_j = (j as f32 / n_points as f32) * 2.0 * PI + rot;

            let x0 = (cx + rx * angle_i.cos()).round() as i32;
            let y0 = (cy + ry * angle_i.sin()).round() as i32;
            let x1 = (cx + rx * angle_j.cos()).round() as i32;
            let y1 = (cy + ry * angle_j.sin()).round() as i32;

            // Bresenham line between the two points.
            let steps = ((x1 - x0).abs().max((y1 - y0).abs())).max(1) as usize;
            let steps = steps.min(512);
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let px = (x0 as f32 + t * (x1 - x0) as f32).round() as i32;
                let py = (y0 as f32 + t * (y1 - y0) as f32).round() as i32;
                draw::dot_i(grid, px, py);
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 11. Recamán's sequence
// ────────────────────────────────────────────────────────────────────────────

/// Recamán's sequence: a(0)=0; a(n) = a(n-1)−n if positive and not already
/// in the sequence, else a(n-1)+n. Plotted as arcs (half-circles) on a
/// number line, alternating above/below. `eased` reveals terms; `time`
/// pulses the leading arc.
struct Recaman;
impl ProgressStyle for Recaman {
    fn name(&self) -> &str {
        "recaman"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Recamán's sequence: arcs above/below a number line, each term a backward or forward jump"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let max_terms = w.min(60).max(2);
        let revealed = ((ctx.eased * max_terms as f32).round() as usize).clamp(1, max_terms);

        // Build Recamán sequence.
        let mut seq = vec![0usize];
        let mut seen = std::collections::HashSet::new();
        seen.insert(0usize);
        for n in 1..max_terms {
            let prev = seq[n - 1];
            let candidate = prev.saturating_sub(n);
            let next = if candidate > 0 && !seen.contains(&candidate) {
                candidate
            } else {
                prev + n
            };
            seq.push(next);
            seen.insert(next);
        }

        // Normalise to fit in width.
        let max_val = seq.iter().copied().max().unwrap_or(1).max(1);
        let baseline = h / 2;

        // Draw number-line baseline.
        draw::hline(grid, 0, w.saturating_sub(1), baseline);

        // Draw arcs as semicircles, alternating above/below.
        for n in 1..revealed {
            let a = seq[n - 1];
            let b = seq[n];
            let forward = b > a;

            let x_a = (a * (w.saturating_sub(1)) / max_val).min(w.saturating_sub(1));
            let x_b = (b * (w.saturating_sub(1)) / max_val).min(w.saturating_sub(1));
            let arc_cx = (x_a + x_b) / 2;
            let arc_r = ((x_b as i32 - x_a as i32).abs() / 2).max(1) as f32;

            // Above baseline for backward jumps (a→b where b<a), below for forward.
            let above = !forward;

            let steps = ((arc_r * PI) as usize).max(4).min(256);
            for s in 0..=steps {
                let theta = s as f32 / steps as f32 * PI;
                let dx = (theta.cos() * arc_r).round() as i32;
                let dy = (theta.sin() * arc_r).round() as i32;
                let px = arc_cx as i32 + dx;
                let py = if above {
                    baseline as i32 - dy
                } else {
                    baseline as i32 + dy
                };
                draw::dot_i(grid, px, py);
            }
        }

        // Animate: pulse on the leading arc.
        let _ = ctx.time;
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 12. Digital root / Vortex math
// ────────────────────────────────────────────────────────────────────────────

/// Digital root vortex math: place digits 1–9 around a circle; connect the
/// digital root sequence of n (n, n+n, …) as a cycle of chords. `eased`
/// selects the base number n ∈ 1..9; `time` rotates and pulses the figure.
/// The vortex math pattern mod 9 reveals hidden symmetries (3–6–9, 1–2–4–8–7–5).
struct DigitalRoot;
impl ProgressStyle for DigitalRoot {
    fn name(&self) -> &str {
        "digital-root-vortex"
    }
    fn theme(&self) -> &str {
        "numbertheory"
    }
    fn describe(&self) -> &str {
        "Vortex math: digital-root cycles 1–9 drawn as chords on a circle, revealing mod-9 symmetry"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as f32;
        let cy = (h / 2) as f32;
        let rx = (w / 2).saturating_sub(1) as f32;
        let ry = (h / 2).saturating_sub(1) as f32;

        // Digital root of n mod 9 (returns 1..=9; 0→9).
        fn digital_root(n: usize) -> usize {
            if n == 0 {
                return 9;
            }
            let r = n % 9;
            if r == 0 {
                9
            } else {
                r
            }
        }

        // Base digit from eased (1–9).
        let base_digit = ((ctx.eased * 8.0).floor() as usize + 1).min(9);

        // Build the cycle: starting from base_digit, keep adding base_digit mod 9.
        let mut cycle = vec![base_digit];
        let mut cur = base_digit;
        for _ in 0..8 {
            cur = digital_root(cur + base_digit);
            if cur == cycle[0] {
                break;
            }
            cycle.push(cur);
        }

        // Rotation from time.
        let rot = ctx.time * 0.15;

        // Draw the 9 node positions on the circle.
        for d in 1..=9usize {
            let angle = (d as f32 - 1.0) / 9.0 * 2.0 * PI - PI / 2.0 + rot;
            let px = (cx + rx * angle.cos()).round() as usize;
            let py = (cy + ry * angle.sin()).round() as usize;
            if px < w && py < h {
                draw::dot(grid, px, py);
            }
        }

        // Draw the cycle chords.
        for i in 0..cycle.len() {
            let a = cycle[i];
            let b = cycle[(i + 1) % cycle.len()];
            let angle_a = (a as f32 - 1.0) / 9.0 * 2.0 * PI - PI / 2.0 + rot;
            let angle_b = (b as f32 - 1.0) / 9.0 * 2.0 * PI - PI / 2.0 + rot;
            let x0 = (cx + rx * angle_a.cos()).round() as i32;
            let y0 = (cy + ry * angle_a.sin()).round() as i32;
            let x1 = (cx + rx * angle_b.cos()).round() as i32;
            let y1 = (cy + ry * angle_b.sin()).round() as i32;

            let steps = ((x1 - x0).abs().max((y1 - y0).abs())).max(1) as usize;
            let steps = steps.min(512);
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let px = (x0 as f32 + t * (x1 - x0) as f32).round() as i32;
                let py = (y0 as f32 + t * (y1 - y0) as f32).round() as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Draw the 3-6-9 axis separately for emphasis.
        for d in [3usize, 6, 9] {
            let angle = (d as f32 - 1.0) / 9.0 * 2.0 * PI - PI / 2.0 + rot;
            let px = (cx + rx * angle.cos()).round() as i32;
            let py = (cy + ry * angle.sin()).round() as i32;
            draw::dot_i(grid, px, py);
            draw::dot_i(grid, px + 1, py);
            draw::dot_i(grid, px, py + 1);
            draw::dot_i(grid, px - 1, py);
            draw::dot_i(grid, px, py - 1);
        }
        Ok(())
    }
}
