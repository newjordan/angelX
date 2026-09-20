//! Wildlife-themed progress bars — ten visually distinct creatures rendered
//! in braille dots and block glyphs.
//!
//! Each style uses a fundamentally different drawing algorithm:
//! - `galloping-horse`    — running silhouette with cycling leg phases
//! - `butterfly`          — figure advancing with wings opening/closing
//! - `spider-web`         — radial + spiral web threads appear as progress fills
//! - `beehive`            — hexagonal comb cells fill one by one
//! - `peacock-fan`        — tail feathers fan out radially with eased spread
//! - `leaping-frog`       — parabolic arcs in two-phase jumps
//! - `octopus`            — central blob with undulating sine tentacles
//! - `owl`                — fixed head with blinking eyes and slow head-turn
//! - `murmuration`        — many tiny dots flowing in a starling-like wave
//! - `elephant`           — walking silhouette with swinging trunk

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Theme tint — forest canopy green. Applied to styles that draw monochrome.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(144, 222, 104);
const TINT_END: Color = Color::rgb(44, 144, 92);

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

/// All styles in the `wildlife` theme.
///
/// Returns one boxed [`ProgressStyle`] per creature variant. Every style is
/// stateless — all animation comes from `ctx.time` and `ctx.eased`.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(GallopingHorse),
        Box::new(Butterfly),
        Box::new(Tinted(SpiderWeb)),
        Box::new(Beehive),
        Box::new(PeacockFan),
        Box::new(Tinted(LeapingFrog)),
        Box::new(Octopus),
        Box::new(Tinted(Owl)),
        Box::new(Murmuration),
        Box::new(Elephant),
    ]
}

// ── Galloping Horse ──────────────────────────────────────────────────────────

struct GallopingHorse;
impl ProgressStyle for GallopingHorse {
    fn name(&self) -> &str {
        "galloping-horse"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Horse galloping rightward — body advances with progress, four legs cycle through a gait"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = h.saturating_sub(1);
        let mid = h / 2;

        // Head of horse advances with progress.
        let head_x = (ctx.eased * w as f32) as usize;
        let head_x = head_x.min(w.saturating_sub(1));

        // Gait phase: four-beat gallop cycle driven by time.
        let gait = ctx.time * 8.0;

        // Draw hoofprints (dust trail) every 12 dots behind the horse.
        let step = 12usize;
        let mut tx = 0usize;
        while tx + step < head_x {
            let mark_x = tx + step / 2;
            draw::dot(grid, mark_x.min(w - 1), base);
            draw::dot(grid, (mark_x + 1).min(w - 1), base);
            tx += step;
        }

        // --- Horse body silhouette centered at head_x - 4 ---
        let bx = head_x.saturating_sub(4);

        // Body: a horizontal blob (3 dots wide, 2 tall at mid).
        for dx in 0..4usize {
            let px = bx.saturating_sub(dx);
            if px < w {
                draw::dot(grid, px, mid);
                if mid + 1 < h {
                    draw::dot(grid, px, mid + 1);
                }
            }
        }

        // Neck & head — above and ahead of body.
        let neck_x = (bx + 1).min(w.saturating_sub(1));
        draw::dot_i(grid, neck_x as i32 + 1, mid as i32 - 1);
        draw::dot_i(grid, neck_x as i32 + 2, mid as i32 - 2);
        draw::dot_i(grid, neck_x as i32 + 3, mid as i32 - 2); // muzzle
                                                              // Mane dot.
        draw::dot_i(grid, neck_x as i32 + 1, mid as i32 - 2);

        // Tail — behind the body.
        let tail_x = bx.saturating_sub(4) as i32;
        let tail_wave = ((gait * 0.5).sin() * 1.5).round() as i32;
        draw::dot_i(grid, tail_x, mid as i32 + tail_wave);
        draw::dot_i(grid, tail_x - 1, mid as i32 + tail_wave + 1);

        // Four legs: two pairs, each alternating up/down.
        // Leg offsets from bx: front pair at bx+1/bx+2, rear at bx-1/bx-2.
        let leg_positions: [i32; 4] = [bx as i32 + 2, bx as i32 + 1, bx as i32 - 1, bx as i32 - 2];
        // Four-beat gait: each leg offset by quarter cycle.
        let quarter = PI / 2.0;
        for (i, &lx) in leg_positions.iter().enumerate() {
            let phase = gait + i as f32 * quarter;
            // Hoof height: 0 = fully raised, 1 = on ground.
            let lift = (phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            let foot_y = mid as i32 + 1 + (lift * (base as f32 - mid as f32 - 1.0)).round() as i32;
            let knee_y = mid as i32 + 1;
            // Upper leg.
            draw::dot_i(grid, lx, knee_y);
            // Lower leg + hoof.
            draw::dot_i(grid, lx, foot_y);
            if foot_y + 1 <= base as i32 {
                draw::dot_i(grid, lx, foot_y + 1); // hoof ground touch
            }
        }

        // Palette tint: sweep up to head.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            for cx in 0..filled_cells.min(cells_w) {
                let t = if filled_cells <= 1 {
                    0.5
                } else {
                    cx as f32 / (filled_cells - 1) as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── Butterfly ────────────────────────────────────────────────────────────────

struct Butterfly;
impl ProgressStyle for Butterfly {
    fn name(&self) -> &str {
        "butterfly"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Butterfly advancing with wings that open and close on a time-driven flap cycle"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = (h / 2) as i32;

        // Body advances with progress.
        let body_x = (ctx.eased * w as f32) as usize;
        let body_x = body_x.min(w.saturating_sub(1)) as i32;

        // Wing flap: cos so wings are fully open at flap=1, closed at flap=0.
        let flap_t = (ctx.time * 6.0).cos().abs(); // 0..1, bounces
        let wing_span = ((w / 4) as f32 * flap_t).round() as i32;
        let wing_h = ((h / 2) as f32 * flap_t).round() as i32;

        // Body: three dots vertically.
        draw::dot_i(grid, body_x, mid - 1);
        draw::dot_i(grid, body_x, mid);
        draw::dot_i(grid, body_x, mid + 1);

        // Upper wings: fan of dots spanning left and right above mid.
        for s in 1..=wing_span.max(1) {
            let s_frac = if wing_span <= 1 {
                1.0
            } else {
                s as f32 / wing_span as f32
            };
            let wy = mid - (s_frac * wing_h as f32).round() as i32;
            draw::dot_i(grid, body_x - s, wy);
            draw::dot_i(grid, body_x + s, wy);
            // Wing edge — outermost dot.
            if s == wing_span {
                draw::dot_i(grid, body_x - s, wy + 1);
                draw::dot_i(grid, body_x + s, wy + 1);
            }
        }

        // Lower wings: smaller, below mid.
        let lower_span = (wing_span * 2 / 3).max(0);
        let lower_h = (wing_h / 2).max(0);
        for s in 1..=lower_span.max(1) {
            let s_frac = if lower_span <= 1 {
                1.0
            } else {
                s as f32 / lower_span as f32
            };
            let wy = mid + 1 + (s_frac * lower_h as f32).round() as i32;
            draw::dot_i(grid, body_x - s, wy);
            draw::dot_i(grid, body_x + s, wy);
        }

        // Antennae (always visible, slight wave).
        let ant_wave = ((ctx.time * 4.0).sin() * 0.5).round() as i32;
        draw::dot_i(grid, body_x - 1, mid - 2 + ant_wave);
        draw::dot_i(grid, body_x + 1, mid - 2 - ant_wave);
        draw::dot_i(grid, body_x - 2, mid - 3 + ant_wave);
        draw::dot_i(grid, body_x + 2, mid - 3 - ant_wave);

        // Tint only the wing cells with palette color.
        let (cells_w, cells_h) = grid.dimensions();
        let center_cx = (body_x.max(0) as usize / 2).min(cells_w.saturating_sub(1));
        let wing_cells = (wing_span.max(0) as usize / 2 + 1).min(cells_w);
        let cx0 = center_cx.saturating_sub(wing_cells);
        let cx1 = (center_cx + wing_cells).min(cells_w.saturating_sub(1));
        for cy in 0..cells_h {
            for cx in cx0..=cx1 {
                let t = if cx1 == cx0 {
                    0.5
                } else {
                    (cx - cx0) as f32 / (cx1 - cx0) as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── Spider Web ───────────────────────────────────────────────────────────────

struct SpiderWeb;
impl ProgressStyle for SpiderWeb {
    fn name(&self) -> &str {
        "spider-web"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Spider weaving a radial web — spoke threads appear first, then concentric spiral rings fill with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let max_r = (w.min(h * 2) / 2).max(1) as f32;

        // Number of radial spokes.
        let n_spokes = 8usize;
        // Spokes appear one by one in the first 40% of progress.
        let spoke_progress = (ctx.eased * n_spokes as f32 / 0.4).min(n_spokes as f32);
        let full_spokes = spoke_progress as usize;

        // Draw radial spokes.
        for s in 0..full_spokes.min(n_spokes) {
            let angle = s as f32 * (2.0 * PI / n_spokes as f32);
            let r_max = max_r as i32;
            for r in 0..=r_max {
                let dx = (angle.cos() * r as f32).round() as i32;
                let dy = (angle.sin() * r as f32).round() as i32;
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Concentric ring threads fill in the remaining 60% of progress.
        let n_rings = 5usize;
        let ring_progress = ((ctx.eased - 0.4) / 0.6 * n_rings as f32).max(0.0);
        let full_rings = ring_progress as usize;
        let partial_ring_frac = ring_progress.fract();

        for ring in 0..full_rings.min(n_rings) {
            let r = (max_r * (ring + 1) as f32 / n_rings as f32).round() as i32;
            // Full ring: draw 64 points around circumference.
            for step in 0..64usize {
                let angle = step as f32 * (2.0 * PI / 64.0);
                let dx = (angle.cos() * r as f32).round() as i32;
                let dy = (angle.sin() * r as f32 * 0.5).round() as i32; // squish vertically
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Partial ring for the currently-being-woven ring.
        if full_rings < n_rings {
            let r = (max_r * (full_rings + 1) as f32 / n_rings as f32).round() as i32;
            let steps = (partial_ring_frac * 64.0) as usize;
            for step in 0..steps {
                let angle = step as f32 * (2.0 * PI / 64.0);
                let dx = (angle.cos() * r as f32).round() as i32;
                let dy = (angle.sin() * r as f32 * 0.5).round() as i32;
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Spider: a small dot near the center, moving outward on one spoke with time.
        let spider_spoke_angle = (ctx.time * 0.8).rem_euclid(2.0 * PI);
        let spider_r = (max_r * 0.5 * (0.5 + 0.5 * (ctx.time * 0.7).sin())) as i32;
        let sx = cx + (spider_spoke_angle.cos() * spider_r as f32).round() as i32;
        let sy = cy + (spider_spoke_angle.sin() * spider_r as f32 * 0.5).round() as i32;
        draw::dot_i(grid, sx, sy);
        draw::dot_i(grid, sx + 1, sy);
        draw::dot_i(grid, sx, sy + 1);
        Ok(())
    }
}

// ── Beehive ──────────────────────────────────────────────────────────────────

struct Beehive;
impl ProgressStyle for Beehive {
    fn name(&self) -> &str {
        "beehive"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Honeycomb cells filling one by one — each hexagonal cell appears as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Hexagonal cell size in dots: width=6, height=4 (pointy-top hex approximation).
        let cell_w = 6usize;
        let cell_h = 4usize;
        let cols = (w / cell_w).max(1);
        let rows = (h / cell_h).max(1);
        let total_cells = cols * rows;

        // How many cells are lit?
        let lit = (ctx.eased * total_cells as f32).round() as usize;

        for idx in 0..lit.min(total_cells) {
            let col = idx % cols;
            let row = idx / cols;
            // Offset odd rows by half cell width for hex stagger.
            let x_off = if row % 2 == 1 { cell_w / 2 } else { 0 };
            let x0 = col * cell_w + x_off;
            let y0 = row * cell_h;

            // Draw a simple hex outline (6-sided) in dot space.
            // Points: use flat-top hex with half-width horizontal edges.
            let hw = (cell_w / 2) as i32;
            let hh = (cell_h / 2) as i32;
            let cx = (x0 + cell_w / 2) as i32;
            let cy = (y0 + cell_h / 2) as i32;

            // Top & bottom horizontal edges.
            for dx in -hw + 1..hw {
                draw::dot_i(grid, cx + dx, cy - hh);
                draw::dot_i(grid, cx + dx, cy + hh);
            }
            // Left & right diagonal sides.
            for dy in -hh..=hh {
                let frac = dy.abs() as f32 / hh.max(1) as f32;
                let x_indent = (frac * 1.0).round() as i32;
                draw::dot_i(grid, cx - hw + x_indent, cy + dy);
                draw::dot_i(grid, cx + hw - x_indent, cy + dy);
            }

            // Fill interior with progress-gated shimmer.
            let fill_frac = ((idx + 1) as f32 / total_cells.max(1) as f32).min(1.0);
            let pulse = (ctx.time * 4.0 + fill_frac * PI * 2.0).sin() * 0.3 + 0.7;
            if pulse > 0.6 {
                // Solid fill.
                for dy in -hh + 1..hh {
                    let indent = (dy.abs() as f32 / hh.max(1) as f32).round() as i32;
                    for dx in -hw + 1 + indent..hw - indent {
                        draw::dot_i(grid, cx + dx, cy + dy);
                    }
                }
            }
        }

        // Palette tint: left to right across the bar.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cx = (ctx.eased * cells_w as f32) as usize;
        for cy_c in 0..cells_h {
            for cx_c in 0..filled_cx.min(cells_w) {
                let t = if filled_cx <= 1 {
                    0.5
                } else {
                    cx_c as f32 / (filled_cx - 1) as f32
                };
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── Peacock Fan ──────────────────────────────────────────────────────────────

struct PeacockFan;
impl ProgressStyle for PeacockFan {
    fn name(&self) -> &str {
        "peacock-fan"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Peacock fanning its tail — radial feathers spread from a pivot point as progress increases"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Pivot at bottom-center.
        let px = (w / 2) as i32;
        let py = (h - 1) as i32;
        let max_feather_len = ((h as f32 * 0.9) as i32).max(1);

        // Fan arc: spans from angle_min to angle_max (in radians from vertical).
        // Progress controls how wide the fan is spread.
        let half_fan = ctx.eased * PI * 0.7; // up to 126° each side
        let n_feathers = 12usize;

        for f in 0..n_feathers {
            let t = if n_feathers <= 1 {
                0.5
            } else {
                f as f32 / (n_feathers - 1) as f32
            };
            // Angle from straight-up (-PI/2), distributed across the fan.
            let angle = -PI / 2.0 + (t - 0.5) * 2.0 * half_fan;

            // Feather only appears once enough progress to include it.
            if t > ctx.eased + 0.05 {
                continue;
            }

            // Feather length: outermost feathers slightly shorter.
            let edge = 1.0 - (2.0 * t - 1.0).abs() * 0.25;
            let flen = (max_feather_len as f32 * edge) as i32;

            // Draw feather shaft.
            for r in 0..=flen {
                let dx = (angle.cos() * r as f32 * 1.5).round() as i32; // stretch horizontally
                let dy = (angle.sin() * r as f32).round() as i32;
                draw::dot_i(grid, px + dx, py + dy);
            }

            // Eye spot at tip: oscillates color/dot with time.
            let eye_r = flen;
            let edx = (angle.cos() * eye_r as f32 * 1.5).round() as i32;
            let edy = (angle.sin() * eye_r as f32).round() as i32;
            let pulse = ((ctx.time * 3.0 + t * PI * 2.0).sin() * 0.5 + 0.5) > 0.4;
            if pulse {
                draw::dot_i(grid, px + edx + 1, py + edy);
                draw::dot_i(grid, px + edx - 1, py + edy);
                draw::dot_i(grid, px + edx, py + edy - 1);
            }
        }

        // Peacock body: small vertical column at pivot.
        for dy in 0..=(h / 4) as i32 {
            draw::dot_i(grid, px, py - dy);
        }
        // Head.
        draw::dot_i(grid, px, py - (h / 4) as i32 - 1);
        draw::dot_i(grid, px + 1, py - (h / 4) as i32 - 2); // crest

        // Palette tint across full bar width.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = if cells_w <= 1 {
                    0.5
                } else {
                    cx_c as f32 / (cells_w - 1) as f32
                };
                let color = ctx.palette.sample(t * ctx.eased);
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }
        Ok(())
    }
}

// ── Leaping Frog ─────────────────────────────────────────────────────────────

struct LeapingFrog;
impl ProgressStyle for LeapingFrog {
    fn name(&self) -> &str {
        "leaping-frog"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Frog leaping in two-phase parabolic arcs — each jump leaves a ripple splash on landing"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = (h - 1) as i32;

        // Number of lily-pad stops (jump targets).
        let n_jumps = 4usize;
        let jump_w = w / n_jumps.max(1);

        // Which jump are we in?
        let jump_idx_f = ctx.eased * n_jumps as f32;
        let current_jump = (jump_idx_f as usize).min(n_jumps.saturating_sub(1));
        let jump_frac = jump_idx_f.fract();

        // Draw lily pads at each destination.
        for j in 0..=current_jump {
            let pad_x = (j * jump_w + jump_w / 2) as i32;
            // Lily pad: small oval on base line.
            draw::dot_i(grid, pad_x - 1, base);
            draw::dot_i(grid, pad_x, base);
            draw::dot_i(grid, pad_x + 1, base);
            draw::dot_i(grid, pad_x, base - 1);

            // Splash ripple on landing: fades over time.
            if j < current_jump {
                // Already landed — static rings.
                draw::dot_i(grid, pad_x - 2, base);
                draw::dot_i(grid, pad_x + 2, base);
                draw::dot_i(grid, pad_x, base - 2);
            }
        }

        // Frog arc position during current jump.
        let start_x = (current_jump * jump_w) as i32;
        let end_x = ((current_jump + 1) * jump_w) as i32;
        let frog_x = start_x + ((jump_frac * (end_x - start_x) as f32) as i32);

        // Parabolic height: peak at jump_frac = 0.5.
        let peak_h = (h as f32 * 0.65) as i32;
        let arc_h = (4.0 * peak_h as f32 * jump_frac * (1.0 - jump_frac)) as i32;
        let frog_y = base - arc_h;
        let frog_y = frog_y.max(0);

        // Frog body.
        draw::dot_i(grid, frog_x, frog_y);
        draw::dot_i(grid, frog_x + 1, frog_y);
        draw::dot_i(grid, frog_x, frog_y + 1);
        draw::dot_i(grid, frog_x + 1, frog_y + 1);

        // Eyes on top.
        draw::dot_i(grid, frog_x - 1, frog_y - 1);
        draw::dot_i(grid, frog_x + 2, frog_y - 1);

        // Legs: extend outward during jump, tucked when crouching.
        let leg_ext = (arc_h as f32 / peak_h.max(1) as f32 * 2.0).min(2.0) as i32;
        // Front legs.
        draw::dot_i(grid, frog_x - 1 - leg_ext, frog_y + 1);
        draw::dot_i(grid, frog_x + 2 + leg_ext, frog_y + 1);
        // Rear legs.
        draw::dot_i(grid, frog_x - 1, frog_y + 2 + leg_ext);
        draw::dot_i(grid, frog_x + 2, frog_y + 2 + leg_ext);

        Ok(())
    }
}

// ── Octopus ───────────────────────────────────────────────────────────────────

struct Octopus;
impl ProgressStyle for Octopus {
    fn name(&self) -> &str {
        "octopus"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Octopus with eight undulating tentacles — the filled region reveals more arms as progress grows"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Mantle (body) centered horizontally at progress position.
        let cx = (ctx.eased * w as f32) as i32;
        let cx = cx.clamp(0, w as i32 - 1);
        let cy = (h / 3) as i32;

        // Mantle: oval blob.
        let mw = ((w / 8).max(2) as i32).min(6);
        let mh = ((h / 4).max(1) as i32).min(4);
        for dy in -mh..=mh {
            let x_extent =
                (mw as f32 * (1.0 - (dy as f32 / mh.max(1) as f32).powi(2)).sqrt()).round() as i32;
            for dx in -x_extent..=x_extent {
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Eyes.
        draw::dot_i(grid, cx - mw / 2, cy - mh / 2);
        draw::dot_i(grid, cx + mw / 2, cy - mh / 2);

        // Eight tentacles: angle them downward, sine-undulate with time.
        let n_arms = 8usize;
        // Number of arms visible = scaled with progress.
        let visible_arms = (ctx.eased * n_arms as f32).round() as usize;
        let arm_len = ((h as f32 * 0.7) as usize).max(2);

        for a in 0..visible_arms.min(n_arms) {
            // Spread arms evenly across the bottom semicircle.
            let t = if n_arms <= 1 {
                0.5
            } else {
                a as f32 / (n_arms - 1) as f32
            };
            let base_angle = PI * t; // 0 → PI (left to right below mantle)
            let arm_phase = ctx.time * 2.5 + a as f32 * 0.8;

            // Draw arm segment by segment, applying sine wave lateral offset.
            let arm_x0 = cx + ((t * 2.0 - 1.0) * mw as f32).round() as i32;
            let arm_y0 = cy + mh;

            for step in 0..arm_len {
                let frac = step as f32 / arm_len.max(1) as f32;
                // Direction angle: fans outward then curves back down.
                let dir_angle = base_angle + (0.5 - t).abs() * PI * 0.3 * frac;
                let lateral = (arm_phase + frac * PI * 2.0).sin() * frac * 2.0;
                let dx = (dir_angle.cos() * frac * mw as f32 * 1.5 + lateral).round() as i32;
                let dy =
                    (frac * arm_len as f32 * 0.8 + (1.0 - frac) * dir_angle.sin()).round() as i32;
                draw::dot_i(grid, arm_x0 + dx, arm_y0 + dy);
                // Sucker: every 3 steps, a dot perpendicular.
                if step % 3 == 0 {
                    draw::dot_i(grid, arm_x0 + dx + 1, arm_y0 + dy);
                }
            }
        }

        // Palette tint.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cx = (ctx.eased * cells_w as f32) as usize;
        for cy_c in 0..cells_h {
            for cx_c in 0..filled_cx.min(cells_w) {
                let t = if filled_cx <= 1 {
                    0.5
                } else {
                    cx_c as f32 / (filled_cx - 1) as f32
                };
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── Owl ──────────────────────────────────────────────────────────────────────

struct Owl;
impl ProgressStyle for Owl {
    fn name(&self) -> &str {
        "owl"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Owl with blinking eyes and a slow head-turn — tracks progress by rotating to face rightward"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Owl body centered at progress position.
        let body_cx = (ctx.eased * w as f32) as i32;
        let body_cx = body_cx.clamp(0, w as i32 - 1);
        let body_cy = (h / 2) as i32;

        // Body: tall oval.
        let bw = ((w / 10).max(1) as i32).min(4);
        let bh = ((h / 2).max(2) as i32).min(6);
        for dy in -bh..=bh {
            let x_ext =
                (bw as f32 * (1.0 - (dy as f32 / bh.max(1) as f32).powi(2)).sqrt()).round() as i32;
            for dx in -x_ext..=x_ext {
                draw::dot_i(grid, body_cx + dx, body_cy + dy);
            }
        }

        // Head: slightly smaller oval above body.
        let head_cy = body_cy - bh - 1;
        let hw = (bw - 1).max(1);
        let hh = (bh / 2).max(1);
        for dy in -hh..=hh {
            let x_ext =
                (hw as f32 * (1.0 - (dy as f32 / hh.max(1) as f32).powi(2)).sqrt()).round() as i32;
            for dx in -x_ext..=x_ext {
                draw::dot_i(grid, body_cx + dx, head_cy + dy);
            }
        }

        // Ear tufts.
        draw::dot_i(grid, body_cx - hw, head_cy - hh - 1);
        draw::dot_i(grid, body_cx + hw, head_cy - hh - 1);

        // Blink: eyes appear for most of the time, close briefly every ~3s.
        let blink_cycle = (ctx.time * 1.2).fract();
        let eyes_open = blink_cycle > 0.08; // closed 8% of cycle

        // Head-turn: gaze direction follows progress — look right as bar fills.
        let gaze_x_off = ((ctx.eased * 2.0 - 1.0) * hw as f32 * 0.5).round() as i32;

        if eyes_open {
            // Eyes: two dots with offset based on gaze.
            draw::dot_i(grid, body_cx - hw / 2 + gaze_x_off, head_cy);
            draw::dot_i(grid, body_cx + hw / 2 + gaze_x_off, head_cy);
            // Pupil highlight.
            draw::dot_i(grid, body_cx - hw / 2 + gaze_x_off + 1, head_cy - 1);
            draw::dot_i(grid, body_cx + hw / 2 + gaze_x_off + 1, head_cy - 1);
        } else {
            // Closed eyes: a horizontal line.
            draw::dot_i(grid, body_cx - hw / 2, head_cy);
            draw::dot_i(grid, body_cx - hw / 2 + 1, head_cy);
            draw::dot_i(grid, body_cx + hw / 2 - 1, head_cy);
            draw::dot_i(grid, body_cx + hw / 2, head_cy);
        }

        // Beak.
        draw::dot_i(grid, body_cx + gaze_x_off, head_cy + 1);

        // Wings: outstretched slightly when progress > 0.5.
        if ctx.eased > 0.5 {
            let wing_ext = ((ctx.eased - 0.5) * 2.0 * bw as f32 * 2.0).round() as i32;
            draw::dot_i(grid, body_cx - bw - wing_ext, body_cy);
            draw::dot_i(grid, body_cx + bw + wing_ext, body_cy);
            draw::dot_i(grid, body_cx - bw - wing_ext + 1, body_cy + 1);
            draw::dot_i(grid, body_cx + bw + wing_ext - 1, body_cy + 1);
        }

        // Talons at base.
        draw::dot_i(grid, body_cx - 1, body_cy + bh + 1);
        draw::dot_i(grid, body_cx, body_cy + bh + 1);
        draw::dot_i(grid, body_cx + 1, body_cy + bh + 1);

        Ok(())
    }
}

// ── Murmuration ───────────────────────────────────────────────────────────────

struct Murmuration;
impl ProgressStyle for Murmuration {
    fn name(&self) -> &str {
        "murmuration"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Starling murmuration — a dense wave of dots that rolls across the bar in flowing sine sheets"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Front of the flock advances with progress.
        let front_x = (ctx.eased * w as f32) as usize;
        let flock_depth = (w / 2).max(4);

        // Flock: a cloud of dots with per-dot sine offsets.
        // We generate a deterministic but chaotic-looking distribution.
        let n_birds = ((front_x / 3).max(1)).min(60);

        for b in 0..n_birds {
            // Each bird has a unique phase and speed.
            let seed = b as f32;
            let x_offset = (seed * 7.3 + ctx.time * (1.5 + (seed * 0.13).fract() * 2.0))
                .rem_euclid(flock_depth as f32);
            let bx = front_x.saturating_sub(x_offset as usize);
            if bx >= w {
                continue;
            }

            // Y: three-layer sine (simulates turbulent flocking).
            let y_phase = seed * 1.7 + ctx.time * (2.0 + (seed * 0.07).fract());
            let y_frac = 0.5 + 0.35 * (y_phase).sin() + 0.15 * (y_phase * 2.3 + seed * 0.9).sin();
            let by = (y_frac * (h - 1) as f32).round() as usize;
            let by = by.min(h - 1);

            draw::dot(grid, bx, by);

            // Wing tip dots (brief v-shape).
            let wing_beat = (ctx.time * 6.0 + seed * 0.5).sin();
            if wing_beat > 0.2 {
                draw::dot_i(grid, bx as i32 - 1, by as i32 - 1);
                draw::dot_i(grid, bx as i32 + 1, by as i32 - 1);
            }
        }

        // Faint trailing density: shade the track with palette.
        let (cells_w, cells_h) = grid.dimensions();
        let front_cell = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            for cx in 0..front_cell.min(cells_w) {
                let t = if front_cell <= 1 {
                    0.5
                } else {
                    cx as f32 / (front_cell - 1) as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── Elephant ─────────────────────────────────────────────────────────────────

struct Elephant;
impl ProgressStyle for Elephant {
    fn name(&self) -> &str {
        "elephant"
    }
    fn theme(&self) -> &str {
        "wildlife"
    }
    fn describe(&self) -> &str {
        "Elephant walking rightward — four legs cycle in a slow plod, trunk swings with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let base = (h - 1) as i32;
        // Elephant is larger — body is proportionally big.
        let body_h = ((h * 2 / 3).max(2) as i32).min(8);
        let body_w = ((w / 6).max(3) as i32).min(10);

        // Position: leading edge (trunk tip) at progress × width.
        let trunk_tip_x = (ctx.eased * w as f32) as i32;
        let trunk_tip_x = trunk_tip_x.clamp(0, w as i32 - 1);

        // Body center is behind the trunk.
        let body_cx = (trunk_tip_x - body_w / 2 - 2).max(0);
        let body_top = base - body_h;

        // Body rectangle (filled solid so it's clearly an elephant silhouette).
        for dy in 0..=body_h {
            let x_w = if dy == 0 || dy == body_h {
                body_w * 2 / 3
            } else {
                body_w
            };
            for dx in -x_w..=x_w {
                draw::dot_i(grid, body_cx + dx, body_top + dy);
            }
        }

        // Head: forward of body, rounded.
        let head_cx = body_cx + body_w + 1;
        let head_r = (body_h / 3).max(1);
        for dy in -head_r..=head_r {
            let x_ext = (head_r as f32 * (1.0 - (dy as f32 / head_r.max(1) as f32).powi(2)).sqrt())
                .round() as i32;
            for dx in -x_ext..=x_ext {
                draw::dot_i(grid, head_cx + dx, base - body_h / 2 + dy);
            }
        }

        // Eye.
        draw::dot_i(grid, head_cx + head_r / 2, base - body_h / 2 - head_r / 2);

        // Ear: large floppy disc behind and above head.
        let ear_cx = head_cx - head_r;
        let ear_cy = base - body_h / 2 - head_r / 2;
        for dy in -head_r - 1..=head_r {
            let x_ext = ((head_r + 1) as f32
                * (1.0 - (dy as f32 / (head_r + 1).max(1) as f32).powi(2)).sqrt())
            .round() as i32;
            draw::dot_i(grid, ear_cx - x_ext / 2, ear_cy + dy);
        }

        // Trunk: hangs from head, swings side-to-side with time.
        let trunk_swing = (ctx.time * 1.8).sin();
        let trunk_len = (body_h / 2 + 2).max(2) as i32;
        let trunk_base_x = head_cx + head_r;
        let trunk_base_y = base - body_h / 2 + head_r / 2;
        for step in 0..=trunk_len {
            let frac = step as f32 / trunk_len.max(1) as f32;
            let swing_x = (trunk_swing * frac * 2.0 * head_r as f32).round() as i32;
            let tx = trunk_base_x + swing_x;
            let ty = trunk_base_y + step;
            draw::dot_i(grid, tx, ty);
        }
        // Trunk tip curl.
        let tip_x = trunk_base_x + (trunk_swing * 2.0 * head_r as f32).round() as i32;
        let tip_y = trunk_base_y + trunk_len;
        draw::dot_i(grid, tip_x + 1, tip_y - 1);
        draw::dot_i(grid, tip_x + 1, tip_y);

        // Tail: short stub at back.
        let tail_x = body_cx - body_w;
        let tail_wave = ((ctx.time * 2.5).sin() * 1.0).round() as i32;
        draw::dot_i(grid, tail_x - 1, base - body_h + tail_wave);
        draw::dot_i(grid, tail_x - 2, base - body_h + tail_wave + 1);

        // Four legs: plodding walk cycle.
        let plod = ctx.time * 2.5;
        let half = PI;
        let leg_positions: [i32; 4] = [
            body_cx + body_w / 2,
            body_cx + body_w / 4,
            body_cx - body_w / 4,
            body_cx - body_w / 2,
        ];
        let leg_len = (base - (base - body_h + body_h / 3)).max(1);
        for (i, &lx) in leg_positions.iter().enumerate() {
            let phase = plod + i as f32 * half / 2.0;
            // Lift = 0 (ground) to 1 (raised).
            let lift = (phase.sin().max(0.0) * 0.6) as i32;
            let leg_top = base - body_h + body_h / 3;
            let leg_bot = base - lift;
            draw::dot_i(grid, lx, leg_top);
            draw::dot_i(grid, lx, (leg_top + leg_len / 2).min(leg_bot));
            draw::dot_i(grid, lx, leg_bot.max(leg_top));
            // Hoof: wider at bottom.
            draw::dot_i(grid, lx - 1, leg_bot.max(leg_top));
            draw::dot_i(grid, lx + 1, leg_bot.max(leg_top));
        }

        // Palette tint behind the elephant.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cx = (ctx.eased * cells_w as f32) as usize;
        for cy_c in 0..cells_h {
            for cx_c in 0..filled_cx.min(cells_w) {
                let t = if filled_cx <= 1 {
                    0.5
                } else {
                    cx_c as f32 / (filled_cx - 1) as f32
                };
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}
