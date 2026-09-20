//! Yantra / Mandala sacred-geometry progress styles.
//!
//! Eleven structurally distinct radially symmetric styles drawn entirely with
//! braille dots via `draw::dot_i` and the Bresenham line helper defined below.
//! Each style has a unique construction principle:
//!
//! - `sri-yantra`            — 9 interlocking triangles + lotus + bhupura gates
//! - `lotus-mandala`         — concentric rings of opening lotus petals (arc pairs)
//! - `rose-window`           — Gothic cathedral radial tracery with foil cusps
//! - `bagua`                 — eight trigrams around a yin-yang center
//! - `enneagram`             — 9-point star (1-4-2-8-5-7 cycle + triangle)
//! - `rangoli`               — dot-grid kolam with loops around pulli points
//! - `chartres-labyrinth`    — 11-circuit circular labyrinth path winding inward
//! - `mandala-tessellation`  — n-fold rotational tiling growing outward with progress
//! - `dharma-wheel`          — 8-spoked wheel with hub, rim and decorative felloes
//! - `vesica-rosette`        — 6-petal rosette from overlapping vesica piscis arcs
//! - `star-of-david`         — Star of David hexagram nested with concentric circles

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ─────────────────────────────────────────────────────────────────────────────
// Registry
// ─────────────────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — saffron into gold.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(255, 126, 84);
const TINT_END: Color = Color::rgb(255, 204, 64);

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

/// All styles in the `yantra` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per sacred-geometry style. The eleven
/// styles span distinct construction methods — triangle systems, petal arcs,
/// tracery, trigrams, star polygons, kolam loops, labyrinth paths, rotational
/// tilings, spoked wheels, vesica arcs, and nested hexagrams — ensuring no two
/// styles are structurally alike.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(SriYantra)),
        Box::new(Tinted(LotusMandala)),
        Box::new(Tinted(RoseWindow)),
        Box::new(Tinted(Bagua)),
        Box::new(Tinted(Enneagram)),
        Box::new(Tinted(Rangoli)),
        Box::new(Tinted(ChartresLabyrinth)),
        Box::new(Tinted(MandalaTessellation)),
        Box::new(Tinted(DharmaWheel)),
        Box::new(Tinted(VesicaRosette)),
        Box::new(Tinted(StarOfDavid)),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared geometry helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Grid center in dot-space.
#[inline]
fn center(dw: usize, dh: usize) -> (f32, f32) {
    (dw as f32 / 2.0, dh as f32 / 2.0)
}

/// Largest radius that fits within the dot grid with 1-dot padding.
#[inline]
fn fit_radius(dw: usize, dh: usize) -> f32 {
    let hw = (dw as f32 / 2.0 - 1.0).max(1.0);
    let hh = (dh as f32 / 2.0 - 1.0).max(1.0);
    hw.min(hh)
}

/// Convert polar coordinates centered at (cx, cy) to dot-space integers.
#[inline]
fn polar(cx: f32, cy: f32, r: f32, angle: f32) -> (i32, i32) {
    let x = cx + r * angle.cos();
    let y = cy - r * angle.sin(); // screen y-axis is flipped
    (x.round() as i32, y.round() as i32)
}

/// Step-bounded Bresenham line between two signed dot-space points.
/// Out-of-bounds dots are silently discarded by `draw::dot_i`.
fn line(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x < x1 { 1 } else { -1 };
    let sy: i32 = if y < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let max_steps = (dx.abs() + dy.abs() + 2) as usize;
    let mut steps = 0usize;
    loop {
        draw::dot_i(grid, x, y);
        if x == x1 && y == y1 {
            break;
        }
        steps += 1;
        if steps > max_steps {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Draw a circular arc from `a_start` to `a_end` (radians, CCW) at radius `r`
/// centered at (cx, cy).  Step count is proportional to arc length.
fn arc(grid: &mut BrailleGrid, cx: f32, cy: f32, r: f32, a_start: f32, a_end: f32) {
    if r < 0.5 {
        return;
    }
    let span = (a_end - a_start).abs();
    let steps = ((r * span).ceil() as usize).max(4).min(1024);
    let mut prev: Option<(i32, i32)> = None;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let a = a_start + (a_end - a_start) * t;
        let p = polar(cx, cy, r, a);
        if let Some(q) = prev {
            line(grid, q.0, q.1, p.0, p.1);
        }
        prev = Some(p);
    }
}

/// Draw a full circle at radius `r` centered at (cx, cy).
fn circle(grid: &mut BrailleGrid, cx: f32, cy: f32, r: f32) {
    arc(grid, cx, cy, r, 0.0, 2.0 * PI);
}

/// Draw a triangle through three dot-space points.
fn triangle(grid: &mut BrailleGrid, ax: i32, ay: i32, bx: i32, by: i32, cx: i32, cy: i32) {
    line(grid, ax, ay, bx, by);
    line(grid, bx, by, cx, cy);
    line(grid, cx, cy, ax, ay);
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Sri Yantra
// ─────────────────────────────────────────────────────────────────────────────

struct SriYantra;
impl ProgressStyle for SriYantra {
    fn name(&self) -> &str {
        "sri-yantra"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Sri Yantra: 9 interlocking triangles (4 upward Shiva + 5 downward Shakti) \
         forming 43 small triangles, with an outer lotus ring and square bhupura gates — \
         the triangles are revealed layer by layer as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r = fit_radius(dw, dh);
        let rot = ctx.time * 0.15;

        // 9 triangles defined as (apex_angle, base_angle_L, base_angle_R, radius_scale)
        // Upward triangles (Shiva) point up (apex at top = PI/2), alternating angles.
        // Downward triangles (Shakti) apex at bottom (= -PI/2).
        // We approximate the classic proportions with 5 size levels.
        let triangles: &[(f32, f32, f32)] = &[
            // (apex_angle from center, base half-angle_spread, radius_fraction)
            // 4 upward (apex = PI/2):
            (PI / 2.0, 0.95, 1.00), // T1 largest upward
            (PI / 2.0, 0.70, 0.75), // T2
            (PI / 2.0, 0.50, 0.55), // T3
            (PI / 2.0, 0.30, 0.35), // T4 smallest upward
            // 5 downward (apex = -PI/2):
            (-PI / 2.0, 0.88, 0.92), // T5 largest downward
            (-PI / 2.0, 0.65, 0.68), // T6
            (-PI / 2.0, 0.45, 0.48), // T7
            (-PI / 2.0, 0.28, 0.30), // T8
            (-PI / 2.0, 0.15, 0.16), // T9 smallest downward
        ];

        let total = triangles.len();
        let reveal = (ctx.eased * (total + 6) as f32).round() as usize; // +6 for lotus+bhupura

        // Draw bhupura (square gates) — three nested squares
        if reveal >= total + 3 {
            for k in 0..3usize {
                let s = r * (1.15 + k as f32 * 0.10);
                // Square corners
                let corners = [
                    (cx - s, cy - s * 0.5),
                    (cx + s, cy - s * 0.5),
                    (cx + s, cy + s * 0.5),
                    (cx - s, cy + s * 0.5),
                ];
                for i in 0..4 {
                    let (ax, ay) = corners[i];
                    let (bx, by) = corners[(i + 1) % 4];
                    line(
                        grid,
                        ax.round() as i32,
                        ay.round() as i32,
                        bx.round() as i32,
                        by.round() as i32,
                    );
                }
            }
        }

        // Draw outer lotus ring (8 petals as arcs)
        if reveal >= total + 1 {
            let petal_r = r * 0.18;
            let ring_r = r * 1.05;
            let n_petals = 8usize;
            for k in 0..n_petals {
                let angle = rot + 2.0 * PI * k as f32 / n_petals as f32;
                let pcx = cx + ring_r * angle.cos();
                let pcy = cy - ring_r * angle.sin();
                arc(grid, pcx, pcy, petal_r, angle + PI * 0.3, angle + PI * 1.7);
            }
        }

        // Enclosing circle
        if reveal >= total {
            circle(grid, cx, cy, r);
        }

        // Draw triangles (revealed one at a time with eased)
        for (idx, &(apex_a, half_spread, rfrac)) in triangles.iter().enumerate() {
            if idx >= reveal {
                break;
            }
            let tr = r * rfrac;
            let apex_a = apex_a + rot;
            let (ax, ay) = polar(cx, cy, tr, apex_a);
            let (bx, by) = polar(cx, cy, tr * 0.7, apex_a + PI - half_spread);
            let (dx, dy) = polar(cx, cy, tr * 0.7, apex_a + PI + half_spread);
            triangle(grid, ax, ay, bx, by, dx, dy);
        }

        // Bindu (central dot)
        draw::dot_i(grid, cx.round() as i32, cy.round() as i32);

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Lotus Mandala
// ─────────────────────────────────────────────────────────────────────────────

struct LotusMandala;
impl ProgressStyle for LotusMandala {
    fn name(&self) -> &str {
        "lotus-mandala"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Lotus mandala: concentric rings of lotus petals drawn as paired arcs, \
         opening outward petal-by-petal as progress rises — inner buds swell \
         into full blooms, outer rings shimmer with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r_max = fit_radius(dw, dh);
        let rot = ctx.time * 0.12;

        // Concentric rings of petals: (n_petals, ring_frac, petal_open_frac)
        let rings: &[(usize, f32)] = &[
            (1, 0.0), // center bindu
            (8, 0.25),
            (12, 0.50),
            (16, 0.75),
            (24, 1.00),
        ];

        let total_rings = rings.len();
        let reveal_frac = ctx.eased;

        for (ri, &(n_petals, ring_frac)) in rings.iter().enumerate() {
            // Each ring appears after a fraction of progress
            let ring_threshold = ri as f32 / total_rings as f32;
            if reveal_frac < ring_threshold {
                break;
            }
            // How far this ring is "open"
            let ring_open = ((reveal_frac - ring_threshold) * total_rings as f32).min(1.0);

            if n_petals == 1 {
                // Bindu
                draw::dot_i(grid, cx.round() as i32, cy.round() as i32);
                continue;
            }

            let ring_r = r_max * ring_frac;
            let petal_r = ring_r * 0.35;
            // Time shimmer offset per ring
            let shimmer = rot * (1.0 + ri as f32 * 0.3);

            for k in 0..n_petals {
                // Reveal petals one by one within the ring
                let petal_thresh = k as f32 / n_petals as f32;
                if ring_open < petal_thresh {
                    break;
                }
                let petal_open = ((ring_open - petal_thresh) * n_petals as f32).min(1.0);

                let base_angle = 2.0 * PI * k as f32 / n_petals as f32 + shimmer;
                let pcx = cx + ring_r * base_angle.cos();
                let pcy = cy - ring_r * base_angle.sin();

                // Two arcs per petal (left and right lobe) opening symmetrically
                let half_span = PI * 0.45 * petal_open;
                arc(
                    grid,
                    pcx,
                    pcy,
                    petal_r,
                    base_angle + PI - half_span,
                    base_angle + PI + half_span,
                );
                // Inner arc (cusp line back toward center)
                let inner_r = petal_r * 0.5 * petal_open;
                arc(
                    grid,
                    cx,
                    cy,
                    ring_r - inner_r,
                    base_angle - half_span * 0.4,
                    base_angle + half_span * 0.4,
                );
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Rose Window (Gothic cathedral tracery)
// ─────────────────────────────────────────────────────────────────────────────

struct RoseWindow;
impl ProgressStyle for RoseWindow {
    fn name(&self) -> &str {
        "rose-window"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Rose window: Gothic cathedral radial tracery — spokes divide the circle \
         into 12 lancets each filled with trefoil cusps, the tracery spun by \
         time while progress reveals ring after ring of foils"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r = fit_radius(dw, dh);
        let rot = ctx.time * 0.10;

        let n_spokes = 12usize;
        let reveal = ctx.eased;

        // Outer rim
        if reveal > 0.0 {
            circle(grid, cx, cy, r);
        }

        // Hub circle
        let hub_r = r * 0.12;
        if reveal > 0.05 {
            circle(grid, cx, cy, hub_r);
        }

        // Radial spokes
        let spoke_count = (reveal * n_spokes as f32).ceil() as usize;
        for k in 0..spoke_count.min(n_spokes) {
            let a = rot + 2.0 * PI * k as f32 / n_spokes as f32;
            let (x0, y0) = polar(cx, cy, hub_r, a);
            let (x1, y1) = polar(cx, cy, r, a);
            line(grid, x0, y0, x1, y1);
        }

        // Concentric rings of trefoil foils at 3 radii
        let foil_radii = [0.40, 0.65, 0.87];
        let foil_counts = [6usize, 12, 24];
        for (fi, (&fr, &fc)) in foil_radii.iter().zip(foil_counts.iter()).enumerate() {
            let ring_thresh = (fi as f32 + 1.0) / 4.0;
            if reveal < ring_thresh {
                continue;
            }
            let ring_r = r * fr;
            let foil_r = r * 0.10;
            let shimmer = rot * (1.0 + fi as f32 * 0.5);
            for k in 0..fc {
                let a = shimmer + 2.0 * PI * k as f32 / fc as f32;
                let fx = cx + ring_r * a.cos();
                let fy = cy - ring_r * a.sin();
                // Trefoil = three small circles at 120° offsets
                for leaf in 0..3usize {
                    let la = a + 2.0 * PI * leaf as f32 / 3.0;
                    let lx = fx + foil_r * 0.55 * la.cos();
                    let ly = fy - foil_r * 0.55 * la.sin();
                    circle(grid, lx, ly, foil_r * 0.45);
                }
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Bagua (Eight Trigrams)
// ─────────────────────────────────────────────────────────────────────────────

struct Bagua;
impl ProgressStyle for Bagua {
    fn name(&self) -> &str {
        "bagua"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Bagua: the eight trigrams of the I-Ching arranged around a yin-yang \
         center — each trigram is three parallel lines (broken or solid) appearing \
         as progress rises, slowly rotating with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r = fit_radius(dw, dh);
        let rot = ctx.time * 0.08;

        // Eight trigrams: bit pattern (3 bits, MSB = top line), 0=broken 1=solid.
        // King Wen arrangement: Qian(7) Dui(6) Li(5) Zhen(4) Xun(3) Kan(2) Gen(1) Kun(0)
        let trigrams: [u8; 8] = [7, 6, 5, 4, 3, 2, 1, 0];
        let reveal_count = (ctx.eased * 8.0).ceil() as usize;

        // Yin-yang circle (two arcs + S-curve)
        let yy_r = r * 0.18;
        if ctx.eased > 0.0 {
            circle(grid, cx, cy, yy_r);
            // Upper half: small filled circle
            circle(grid, cx, cy - yy_r * 0.5, yy_r * 0.25);
            // Lower half: small empty circle
            circle(grid, cx, cy + yy_r * 0.5, yy_r * 0.25);
            // S-curve: two arcs through center
            arc(grid, cx, cy - yy_r * 0.5, yy_r * 0.5, -PI / 2.0, PI / 2.0);
            arc(
                grid,
                cx,
                cy + yy_r * 0.5,
                yy_r * 0.5,
                PI / 2.0,
                3.0 * PI / 2.0,
            );
        }

        // Outer ring
        if ctx.eased > 0.05 {
            circle(grid, cx, cy, r * 0.85);
        }

        for (idx, &bits) in trigrams.iter().enumerate() {
            if idx >= reveal_count {
                break;
            }
            let angle = rot + 2.0 * PI * idx as f32 / 8.0 - PI / 2.0;

            // Position trigram at outer ring
            let tr_cx = cx + r * 0.68 * angle.cos();
            let tr_cy = cy - r * 0.68 * angle.sin();

            // Three lines per trigram, stacked perpendicular to the radial direction
            let perp = angle + PI / 2.0; // perpendicular direction
            let line_len = r * 0.10;
            let line_gap = r * 0.08;

            for line_idx in 0..3usize {
                let offset = (line_idx as f32 - 1.0) * line_gap;
                let lx = tr_cx + offset * angle.cos();
                let ly = tr_cy - offset * angle.sin();
                let solid = (bits >> (2 - line_idx)) & 1 == 1;

                let (x0, y0) = polar(lx, ly, line_len, perp);
                let (x1, y1) = polar(lx, ly, line_len, perp + PI);
                if solid {
                    line(grid, x0, y0, x1, y1);
                } else {
                    // Broken line: two halves with gap
                    let (xm0, ym0) = polar(lx, ly, line_len * 0.3, perp);
                    let (xm1, ym1) = polar(lx, ly, line_len * 0.3, perp + PI);
                    line(grid, x0, y0, xm0, ym0);
                    line(grid, x1, y1, xm1, ym1);
                }
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Enneagram
// ─────────────────────────────────────────────────────────────────────────────

struct Enneagram;
impl ProgressStyle for Enneagram {
    fn name(&self) -> &str {
        "enneagram"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Enneagram: 9 points on a circle connected by the 1-4-2-8-5-7 hexad \
         and a separate 3-6-9 triangle — two interlocked figures appear chord \
         by chord as progress rises, spinning slowly with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r = fit_radius(dw, dh);
        let rot = ctx.time * 0.10 - PI / 2.0; // apex at top

        // 9 evenly spaced vertices
        let verts: Vec<(i32, i32)> = (0..9)
            .map(|k| polar(cx, cy, r, rot + 2.0 * PI * k as f32 / 9.0))
            .collect();

        // Enclosing circle
        if ctx.eased > 0.0 {
            circle(grid, cx, cy, r);
        }

        // Hexad sequence: 1→4→2→8→5→7→1 (0-indexed: 0→3→1→7→4→6→0)
        let hexad: &[usize] = &[0, 3, 1, 7, 4, 6, 0];
        let hexad_chords = hexad.len() - 1;

        // Triangle: 3-6-9 → 0-indexed 2,5,8
        let tri: &[usize] = &[2, 5, 8, 2];
        let tri_chords = tri.len() - 1;

        let total_chords = hexad_chords + tri_chords;
        let reveal = (ctx.eased * total_chords as f32).round() as usize;

        // Draw hexad chords
        for i in 0..hexad_chords.min(reveal) {
            let (ax, ay) = verts[hexad[i]];
            let (bx, by) = verts[hexad[i + 1]];
            line(grid, ax, ay, bx, by);
        }

        // Draw triangle chords
        let tri_drawn = reveal.saturating_sub(hexad_chords);
        for i in 0..tri_chords.min(tri_drawn) {
            let (ax, ay) = verts[tri[i]];
            let (bx, by) = verts[tri[i + 1]];
            line(grid, ax, ay, bx, by);
        }

        // Vertex dots
        let vert_reveal = reveal.min(9);
        for (vx, vy) in verts.iter().take(vert_reveal) {
            draw::dot_i(grid, *vx, *vy);
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Rangoli / Kolam
// ─────────────────────────────────────────────────────────────────────────────

struct Rangoli;
impl ProgressStyle for Rangoli {
    fn name(&self) -> &str {
        "rangoli"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Rangoli kolam: a grid of pulli dots with looping curves drawn around \
         them in the South Indian kolam tradition — loops spiral outward from \
         the center dot as progress rises, time rotates the outer loops"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r_max = fit_radius(dw, dh);
        let rot = ctx.time * 0.07;

        // Grid of pulli dots arranged in concentric diamond rings.
        // Each ring k has 4k dots at distance k*step from center.
        let step = (r_max / 4.0).max(2.0);
        let n_rings = ((r_max / step) as usize).max(1).min(4);
        let total_dots = 1 + (1..=n_rings).map(|k| 4 * k).sum::<usize>();
        let reveal = (ctx.eased * (total_dots + n_rings * 4) as f32).round() as usize;

        let mut dots_drawn = 0usize;
        let mut loops_drawn = 0usize;

        // Center dot
        if reveal > 0 {
            draw::dot_i(grid, cx.round() as i32, cy.round() as i32);
            dots_drawn += 1;
        }

        // Loop around center dot (small circle)
        if reveal > 1 {
            let lrot = rot;
            arc(
                grid,
                cx + step * 0.5 * lrot.cos(),
                cy - step * 0.5 * lrot.sin(),
                step * 0.45,
                0.0,
                2.0 * PI,
            );
            loops_drawn += 1;
        }

        for ring in 1..=n_rings {
            let ring_dots = 4 * ring;
            for k in 0..ring_dots {
                if dots_drawn >= reveal {
                    break;
                }
                let angle = rot + 2.0 * PI * k as f32 / ring_dots as f32;
                let dr = step * ring as f32;
                let dx = cx + dr * angle.cos();
                let dy = cy - dr * angle.sin();
                draw::dot_i(grid, dx.round() as i32, dy.round() as i32);
                dots_drawn += 1;

                // Loop around this dot: figure-eight loops encircling adjacent pairs
                if dots_drawn + loops_drawn < reveal {
                    let next_a = rot + 2.0 * PI * (k + 1) as f32 / ring_dots as f32;
                    let nx = cx + dr * next_a.cos();
                    let ny = cy - dr * next_a.sin();
                    let mx = (dx + nx) / 2.0;
                    let my = (dy + ny) / 2.0;
                    let loop_r = ((dx - nx).hypot(dy - ny) * 0.35).max(1.0);
                    // Two arcs forming a loop around the midpoint
                    arc(grid, mx, my, loop_r, angle + PI * 0.2, angle + PI * 1.8);
                    loops_drawn += 1;
                }
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Chartres Labyrinth
// ─────────────────────────────────────────────────────────────────────────────

struct ChartresLabyrinth;
impl ProgressStyle for ChartresLabyrinth {
    fn name(&self) -> &str {
        "chartres-labyrinth"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Chartres labyrinth: the classical 11-circuit circular labyrinth path \
         winding inward — concentric arcs separated by quarter-turn gaps trace \
         the unicursal path as progress slowly fills each circuit"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r_max = fit_radius(dw, dh);

        // 11 circuits + center: 12 concentric arcs
        let n_circuits = 11usize;
        let reveal = (ctx.eased * (n_circuits + 1) as f32).ceil() as usize;

        // Slow rotation of the whole labyrinth with time
        let rot = ctx.time * 0.05;

        // Each circuit is a pair of arcs: the left half and the right half,
        // connected by cross-passages at the top (entrance) and at ±90° turns.
        // We simplify: each circuit = one near-full arc with a gap for the path entrance.
        for circuit in 0..reveal.min(n_circuits) {
            let frac = (n_circuits - circuit) as f32 / n_circuits as f32;
            let r = r_max * frac;

            // Gap angle: alternates left/right per circuit in the Chartres pattern
            let gap_side = if circuit % 2 == 0 { 1.0_f32 } else { -1.0_f32 };
            let gap_center = rot + gap_side * PI / 2.0;
            let gap_half = PI * 0.12; // gap width in radians

            // Arc sweeping almost full circle, skipping the gap
            let a_start = gap_center + gap_half;
            let a_end = gap_center + 2.0 * PI - gap_half;
            arc(grid, cx, cy, r, a_start, a_end);

            // Cross-passage: a short radial line at the gap
            if circuit + 1 < reveal {
                let inner_r = r_max * (n_circuits - circuit - 1) as f32 / n_circuits as f32;
                let (x0, y0) = polar(cx, cy, inner_r, gap_center + gap_half);
                let (x1, y1) = polar(cx, cy, r, gap_center + gap_half);
                line(grid, x0, y0, x1, y1);
            }
        }

        // Center (goal)
        if reveal > n_circuits {
            circle(grid, cx, cy, r_max / n_circuits as f32 * 0.5);
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Mandala Tessellation
// ─────────────────────────────────────────────────────────────────────────────

struct MandalaTessellation;
impl ProgressStyle for MandalaTessellation {
    fn name(&self) -> &str {
        "mandala-tessellation"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Mandala tessellation: 6-fold rotational tiling of kite-and-dart units \
         growing outward from the center — each tile is a rhombus drawn with \
         Bresenham lines, revealing ring by ring as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r_max = fit_radius(dw, dh);
        let rot = ctx.time * 0.09;

        let n_fold = 6usize;
        let n_rings = 4usize;
        let total = n_fold * n_rings;
        let reveal = (ctx.eased * total as f32).ceil() as usize;

        let tile_r = r_max / n_rings as f32;

        let mut drawn = 0usize;
        'outer: for ring in 1..=n_rings {
            let tiles_per_ring = n_fold * ring; // grows with ring
            let inner_r = tile_r * (ring - 1) as f32;
            let outer_r = tile_r * ring as f32;

            for k in 0..tiles_per_ring {
                if drawn >= reveal {
                    break 'outer;
                }
                let a0 = rot + 2.0 * PI * k as f32 / tiles_per_ring as f32;
                let a1 = rot + 2.0 * PI * (k + 1) as f32 / tiles_per_ring as f32;
                let am = (a0 + a1) / 2.0;

                // Rhombus: four corners
                let (ax, ay) = polar(cx, cy, inner_r, a0);
                let (bx, by) = polar(cx, cy, outer_r, a0);
                let (ex, ey) = polar(cx, cy, outer_r, a1);
                let (fx, fy) = polar(cx, cy, inner_r, a1);
                let (mx, my) = polar(cx, cy, (inner_r + outer_r) / 2.0, am);

                // Draw the kite as two triangles sharing the midpoint apex
                line(grid, ax, ay, mx, my);
                line(grid, mx, my, ex, ey);
                line(grid, ex, ey, fx, fy);
                line(grid, fx, fy, ax, ay);
                // Internal decorative diagonal
                line(grid, bx, by, fx, fy);

                drawn += 1;
            }
        }

        // Center hub
        draw::dot_i(grid, cx.round() as i32, cy.round() as i32);

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Dharma Wheel
// ─────────────────────────────────────────────────────────────────────────────

struct DharmaWheel;
impl ProgressStyle for DharmaWheel {
    fn name(&self) -> &str {
        "dharma-wheel"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Dharma wheel (Dharmachakra): 8 spokes radiating from a central hub \
         to an outer rim, with decorative felloe arcs between spoke tips — \
         the wheel spins continuously with time while spokes reveal with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r = fit_radius(dw, dh);
        let rot = ctx.time * 0.40; // continuous spin

        let n_spokes = 8usize;
        let hub_r = (r * 0.15).max(1.0);
        let rim_r = r;
        let felloe_r = r * 0.12; // arc radius between spoke tips

        // Outer rim
        circle(grid, cx, cy, rim_r);
        // Inner rim (double rim = traditional look)
        circle(grid, cx, cy, rim_r * 0.88);
        // Hub
        circle(grid, cx, cy, hub_r);
        circle(grid, cx, cy, hub_r * 0.5);

        let spoke_reveal = (ctx.eased * n_spokes as f32).ceil() as usize;
        for k in 0..spoke_reveal.min(n_spokes) {
            let a = rot + PI * k as f32 / n_spokes as f32; // 8 spokes = every 22.5°
            let (x0, y0) = polar(cx, cy, hub_r, a);
            let (x1, y1) = polar(cx, cy, rim_r * 0.88, a);
            line(grid, x0, y0, x1, y1);
            // Opposite spoke (each of 8 half-spokes creates a full diameter)
            let (x2, y2) = polar(cx, cy, hub_r, a + PI);
            let (x3, y3) = polar(cx, cy, rim_r * 0.88, a + PI);
            line(grid, x2, y2, x3, y3);
        }

        // Felloe arcs between spoke tips (decorative cusps)
        if ctx.eased > 0.5 {
            for k in 0..n_spokes {
                let a0 = rot + PI * k as f32 / n_spokes as f32;
                let a1 = rot + PI * (k + 1) as f32 / n_spokes as f32;
                let (tx0, ty0) = polar(cx, cy, rim_r * 0.88, a0);
                let (tx1, ty1) = polar(cx, cy, rim_r * 0.88, a1);
                let mx = (tx0 + tx1) as f32 / 2.0;
                let my = (ty0 + ty1) as f32 / 2.0;
                circle(grid, mx, my, felloe_r);
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Vesica Rosette
// ─────────────────────────────────────────────────────────────────────────────

struct VesicaRosette;
impl ProgressStyle for VesicaRosette {
    fn name(&self) -> &str {
        "vesica-rosette"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Vesica rosette: a six-petal rosette built from six overlapping vesica \
         piscis arcs — each arc is a circle of the same radius passing through \
         the center, petals bloom one by one with progress as the rosette rotates"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r = fit_radius(dw, dh) * 0.55; // vesica circles have radius = r_outer
        let rot = ctx.time * 0.14;

        let n_petals = 6usize;
        let reveal = (ctx.eased * (n_petals + 2) as f32).round() as usize;

        // Outer enclosing circle
        if reveal >= n_petals + 1 {
            circle(grid, cx, cy, r * 1.00);
        }

        // Center bindu
        draw::dot_i(grid, cx.round() as i32, cy.round() as i32);

        // Each of the 6 vesica circles is offset from center by r in one direction.
        // A vesica piscis is the intersection region of two equal circles, each
        // passing through the other's center.  We draw only the arc that lies within
        // the central enclosing circle.
        for k in 0..reveal.min(n_petals) {
            let a = rot + 2.0 * PI * k as f32 / n_petals as f32;
            let ocx = cx + r * a.cos();
            let ocy = cy - r * a.sin();
            // The arc of this circle that passes through the center region spans
            // approximately ±60° around the direction back toward the center.
            let back = a + PI;
            arc(grid, ocx, ocy, r, back - PI / 3.0, back + PI / 3.0);

            // Second arc on the same circle: forward-facing lobe
            // (gives the petal shape when two adjacent circles overlap)
            let fwd = a;
            arc(grid, ocx, ocy, r, fwd - PI / 3.0, fwd + PI / 3.0);
        }

        // Inner hexagon at the vertices of the petal intersections
        if reveal >= n_petals {
            for k in 0..n_petals {
                let a0 = rot + 2.0 * PI * k as f32 / n_petals as f32;
                let a1 = rot + 2.0 * PI * (k + 1) as f32 / n_petals as f32;
                let (x0, y0) = polar(cx, cy, r, a0);
                let (x1, y1) = polar(cx, cy, r, a1);
                line(grid, x0, y0, x1, y1);
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 11. Star of David / Hexagram
// ─────────────────────────────────────────────────────────────────────────────

struct StarOfDavid;
impl ProgressStyle for StarOfDavid {
    fn name(&self) -> &str {
        "star-of-david"
    }
    fn theme(&self) -> &str {
        "yantra"
    }
    fn describe(&self) -> &str {
        "Star of David hexagram: two interlocking equilateral triangles nested \
         inside concentric circles — the up-triangle and down-triangle appear \
         in sequence with progress while three harmonic circles spin with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let r = fit_radius(dw, dh);
        let rot = ctx.time * 0.12;

        let reveal = ctx.eased;

        // Three concentric circles (outer, mid, inner) spinning with time
        if reveal > 0.0 {
            circle(grid, cx, cy, r);
        }
        if reveal > 0.25 {
            circle(grid, cx, cy, r * 0.70);
        }
        if reveal > 0.50 {
            circle(grid, cx, cy, r * 0.35);
        }

        // Upward triangle (Star of David: apex at top, Δ)
        if reveal > 0.15 {
            let t_frac = ((reveal - 0.15) / 0.35).min(1.0);
            let tr = r * 0.85;
            let apex_a = PI / 2.0 + rot;
            let left_a = PI / 2.0 + 2.0 * PI / 3.0 + rot;
            let right_a = PI / 2.0 - 2.0 * PI / 3.0 + rot;
            let (ax, ay) = polar(cx, cy, tr, apex_a);
            let (lx, ly) = polar(cx, cy, tr, left_a);
            let (rx, ry) = polar(cx, cy, tr, right_a);

            // Reveal side by side progressively
            let sides = (t_frac * 3.0).ceil() as usize;
            if sides >= 1 {
                line(grid, ax, ay, lx, ly);
            }
            if sides >= 2 {
                line(grid, lx, ly, rx, ry);
            }
            if sides >= 3 {
                line(grid, rx, ry, ax, ay);
            }
        }

        // Downward triangle (inverted, ∇)
        if reveal > 0.50 {
            let t_frac = ((reveal - 0.50) / 0.35).min(1.0);
            let tr = r * 0.85;
            let apex_a = -PI / 2.0 + rot;
            let left_a = -PI / 2.0 + 2.0 * PI / 3.0 + rot;
            let right_a = -PI / 2.0 - 2.0 * PI / 3.0 + rot;
            let (ax, ay) = polar(cx, cy, tr, apex_a);
            let (lx, ly) = polar(cx, cy, tr, left_a);
            let (rx, ry) = polar(cx, cy, tr, right_a);

            let sides = (t_frac * 3.0).ceil() as usize;
            if sides >= 1 {
                line(grid, ax, ay, lx, ly);
            }
            if sides >= 2 {
                line(grid, lx, ly, rx, ry);
            }
            if sides >= 3 {
                line(grid, rx, ry, ax, ay);
            }
        }

        // Central hexagon (intersection of the two triangles)
        if reveal > 0.85 {
            let hex_r = r * 0.49;
            for k in 0..6usize {
                let a0 = rot + PI / 6.0 + 2.0 * PI * k as f32 / 6.0;
                let a1 = rot + PI / 6.0 + 2.0 * PI * (k + 1) as f32 / 6.0;
                let (x0, y0) = polar(cx, cy, hex_r, a0);
                let (x1, y1) = polar(cx, cy, hex_r, a1);
                line(grid, x0, y0, x1, y1);
            }
        }

        // Center bindu
        draw::dot_i(grid, cx.round() as i32, cy.round() as i32);

        Ok(())
    }
}
