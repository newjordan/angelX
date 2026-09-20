//! Architecture / construction progress bars.
//!
//! Twelve structurally distinct styles, each animating a different building
//! form or construction activity:
//!
//! - `skyscraper`       — floors stack floor-by-floor, crane swings with time
//! - `suspension-bridge`— towers rise, catenary cable spans, suspenders drop
//! - `gothic-arch`      — pointed arches spring upward, rose window blooms
//! - `brick-wall`       — running-bond masonry laid course by course, trowel sweeps
//! - `pyramid`          — stepped stone courses narrow toward the apex
//! - `scaffolding`      — horizontal planks + vertical poles erect around a form
//! - `geodesic-dome`    — triangulated hemisphere wireframe assembles by arc
//! - `spiral-staircase` — perspective helix climbs with progress
//! - `roman-aqueduct`   — round arches march across from left to right
//! - `classical-column` — fluted shaft grows, capital blooms at the top
//! - `blueprint`        — a drafting pen sweeps lines onto graph paper
//! - `tower-crane`      — mast rises, jib extends, load swings on a cable

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ─────────────────────────────────────────────────────────────────────────────
// Registry
// ─────────────────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — sandstone into slate. Applied to styles that draw monochrome.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(214, 184, 142);
const TINT_END: Color = Color::rgb(122, 144, 176);

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

/// All styles in the `architecture` theme.
///
/// Returns twelve `Box<dyn ProgressStyle>` values, each implementing a
/// structurally distinct architecture/construction scene in braille dot space.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Skyscraper),
        Box::new(Tinted(SuspensionBridge)),
        Box::new(Tinted(GothicArch)),
        Box::new(BrickWall),
        Box::new(Pyramid),
        Box::new(Tinted(Scaffolding)),
        Box::new(Tinted(GeodesicDome)),
        Box::new(Tinted(SpiralStaircase)),
        Box::new(Tinted(RomanAqueduct)),
        Box::new(Tinted(ClassicalColumn)),
        Box::new(Blueprint),
        Box::new(TowerCrane),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Bresenham-style line between two dot-space points using `draw::dot_i`.
fn line_dots(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        draw::dot_i(grid, x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

/// Draw a quarter-circle arc (quadrant qx,qy in {-1,+1}) centred at (cx,cy),
/// radius r in dot units, stepped every `step` degrees.
fn arc_quarter(grid: &mut BrailleGrid, cx: f32, cy: f32, r: f32, angle_start: f32, angle_end: f32) {
    if r < 1.0 {
        return;
    }
    let steps = ((r * 2.0) as usize).max(8);
    let mut prev: Option<(i32, i32)> = None;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let angle = angle_start + (angle_end - angle_start) * t;
        let x = (cx + r * angle.cos()).round() as i32;
        let y = (cy - r * angle.sin()).round() as i32; // screen y flipped
        if let Some((px, py)) = prev {
            line_dots(grid, px, py, x, y);
        }
        prev = Some((x, y));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Skyscraper
// ─────────────────────────────────────────────────────────────────────────────

/// Floors stack from the ground up as progress rises; a crane jib swings with time.
struct Skyscraper;
impl ProgressStyle for Skyscraper {
    fn name(&self) -> &str {
        "skyscraper"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Skyscraper rises floor by floor; rooftop crane jib swings with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Building occupies center third of the width.
        let bld_w = (dw / 3).max(2);
        let bld_x0 = (dw / 2).saturating_sub(bld_w / 2);
        let bld_x1 = (bld_x0 + bld_w).min(dw - 1);

        // Number of floors to draw.
        let max_floors = dh.saturating_sub(2).max(1);
        let floors = (ctx.eased * max_floors as f32).round() as usize;

        // Draw floors from bottom up.
        for f in 0..floors {
            let y = dh.saturating_sub(1 + f);
            draw::hline(grid, bld_x0, bld_x1, y);
            // Side walls every other floor line.
            if f % 2 == 0 {
                draw::dot(grid, bld_x0, y);
                draw::dot(grid, bld_x1, y);
            }
        }

        // Foundation baseline.
        draw::hline(grid, 0, dw.saturating_sub(1), dh.saturating_sub(1));

        // Crane: vertical mast above the building top.
        if floors >= max_floors.saturating_sub(1) || floors > 0 {
            let mast_top_y = dh.saturating_sub(floors + 2).max(0) as i32;
            let mast_x = (bld_x1 as i32).min(dw as i32 - 1);
            let building_top_y = dh.saturating_sub(floors + 1).max(0) as i32;
            // Mast (vertical post).
            for y in mast_top_y..=building_top_y {
                draw::dot_i(grid, mast_x, y);
            }
            // Jib swings: angle oscillates with time.
            let jib_len = ((dw as i32 - mast_x).max(2)).min(dw as i32 / 3);
            let swing = (ctx.time * 0.7).sin() * 0.3; // radians, small arc
            let jib_angle = PI * 0.5 + swing; // mostly horizontal left
            let jib_x1 = mast_x - (jib_len as f32 * jib_angle.cos()).round() as i32;
            let jib_y1 = mast_top_y + (jib_len as f32 * jib_angle.sin() * 0.25).round() as i32;
            line_dots(grid, mast_x, mast_top_y, jib_x1, jib_y1);
            // Counter-jib goes right.
            let cj_x = mast_x + (jib_len as i32 / 3);
            line_dots(grid, mast_x, mast_top_y, cj_x, mast_top_y);
            // Hook cable hangs from jib tip.
            let cable_len = (dh as i32 / 4).max(1);
            let load_y = (jib_y1 + cable_len).min(dh as i32 - 1);
            line_dots(grid, jib_x1, jib_y1, jib_x1, load_y);
            // Load block at cable bottom.
            draw::dot_i(grid, jib_x1 - 1, load_y);
            draw::dot_i(grid, jib_x1 + 1, load_y);
        }

        // Tint floors.
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx2 in 0..filled_cells.min(cw) {
            let t = if cw <= 1 { 0.5 } else { cx2 as f32 / cw as f32 };
            let color = ctx.palette.sample(t);
            for cy2 in 0..ch {
                draw::tint_row(grid, cy2, cx2, cx2, color);
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Suspension Bridge
// ─────────────────────────────────────────────────────────────────────────────

/// Two towers rise, a catenary main cable spans between them, suspender cables drop.
struct SuspensionBridge;
impl ProgressStyle for SuspensionBridge {
    fn name(&self) -> &str {
        "suspension-bridge"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Twin towers rise, catenary cable spans, vertical suspenders hang down"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 4 || dh < 2 {
            return Ok(());
        }

        let road_y = dh.saturating_sub(2);
        let tower_h = ((dh as f32 * 0.75) as usize).max(2);
        let tower_top_y = road_y.saturating_sub(tower_h);

        // Towers appear when progress passes 0.15 and 0.85.
        let left_tower_x = dw / 5;
        let right_tower_x = dw - dw / 5;

        // Roadway deck.
        draw::hline(grid, 0, dw.saturating_sub(1), road_y);

        // Left tower.
        if ctx.eased > 0.05 {
            let p = ((ctx.eased - 0.05) / 0.3).min(1.0);
            let h = (p * tower_h as f32).round() as usize;
            let ty0 = road_y.saturating_sub(h);
            draw::vline(grid, left_tower_x, ty0, road_y);
            draw::vline(grid, left_tower_x + 1, ty0, road_y);
            // Tower top crossbar.
            if h >= tower_h {
                draw::hline(
                    grid,
                    left_tower_x.saturating_sub(1),
                    left_tower_x + 2,
                    tower_top_y,
                );
            }
        }

        // Right tower.
        if ctx.eased > 0.5 {
            let p = ((ctx.eased - 0.5) / 0.3).min(1.0);
            let h = (p * tower_h as f32).round() as usize;
            let ty0 = road_y.saturating_sub(h);
            draw::vline(grid, right_tower_x, ty0, road_y);
            draw::vline(grid, right_tower_x + 1, ty0, road_y);
            if h >= tower_h {
                draw::hline(
                    grid,
                    right_tower_x.saturating_sub(1),
                    right_tower_x + 2,
                    tower_top_y,
                );
            }
        }

        // Main catenary cable: drawn when both towers are substantially up.
        if ctx.eased > 0.7 {
            let p = ((ctx.eased - 0.7) / 0.2).min(1.0);
            let sag = (tower_h as f32 * 0.4).max(1.0); // catenary dip at midspan
            let x0 = left_tower_x as f32;
            let x1 = right_tower_x as f32;
            let span_dots = ((x1 - x0) * p).max(1.0) as usize;
            for i in 0..=span_dots {
                let t = i as f32 / span_dots.max(1) as f32;
                let cx = x0 + t * (x1 - x0) * p;
                // Catenary shape: y = sag * cosh(k*(t-0.5)) / cosh(0.5k)
                // Approximated with parabola for simplicity.
                let cy_offset = sag * (4.0 * t * (1.0 - t));
                let cy = tower_top_y as f32 + cy_offset;
                draw::dot_i(grid, cx.round() as i32, cy.round() as i32);

                // Suspender every ~6 dots.
                if i % 6 == 0 {
                    let sx = cx.round() as i32;
                    let sy_top = cy.round() as i32;
                    let sy_bot = road_y as i32;
                    for sy in sy_top..=sy_bot {
                        draw::dot_i(grid, sx, sy);
                    }
                }
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Gothic Arch / Cathedral
// ─────────────────────────────────────────────────────────────────────────────

/// Pointed Gothic arches spring upward; a rose window blooms at the top.
struct GothicArch;
impl ProgressStyle for GothicArch {
    fn name(&self) -> &str {
        "gothic-arch"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Pointed Gothic arches spring upward; rose window blooms at the apex"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 2 || dh < 2 {
            return Ok(());
        }

        let base_y = dh.saturating_sub(1) as i32;
        // How many arches to draw based on width.
        let arch_count = ((dw / 10).max(1)).min(5);
        let arch_slot_w = dw / arch_count;

        for a in 0..arch_count {
            let ax_center = (a * arch_slot_w + arch_slot_w / 2) as f32;
            let arch_w_half = (arch_slot_w / 2).saturating_sub(1) as f32;
            let arch_top_y = (dh as f32 * (1.0 - ctx.eased) + 1.0) as i32;

            // Gothic pointed arch: two circular arcs meeting at a point.
            // Left arc: center offset right, arc from base-left to apex.
            // Right arc: center offset left, arc from base-right to apex.
            let foot_l = ax_center - arch_w_half;
            let foot_r = ax_center + arch_w_half;
            let apex_y = arch_top_y.max(0);

            // Arc radius roughly equals the arch width.
            let r = arch_w_half.max(1.0);

            // Left half-arch: sweep from foot_l up-right to apex.
            let steps = (r * 4.0) as usize + 4;
            let mut prev: Option<(i32, i32)> = None;
            for i in 0..=steps {
                let t = i as f32 / steps.max(1) as f32;
                // Interpolate from foot left to apex.
                let px = foot_l + t * (ax_center - foot_l);
                // Pointed arch Y: use sine for the springy curve.
                let raw_y = base_y as f32 - (t.sin() * (base_y - apex_y).abs() as f32 * t.sqrt());
                let py = raw_y.round() as i32;
                if let Some((ppx, ppy)) = prev {
                    line_dots(grid, ppx, ppy, px.round() as i32, py);
                }
                prev = Some((px.round() as i32, py));
            }
            // Right half-arch.
            prev = None;
            for i in 0..=steps {
                let t = i as f32 / steps.max(1) as f32;
                let px = foot_r - t * (foot_r - ax_center);
                let raw_y = base_y as f32 - (t.sin() * (base_y - apex_y).abs() as f32 * t.sqrt());
                let py = raw_y.round() as i32;
                if let Some((ppx, ppy)) = prev {
                    line_dots(grid, ppx, ppy, px.round() as i32, py);
                }
                prev = Some((px.round() as i32, py));
            }

            // Column shafts on left and right feet.
            draw::dot_i(grid, foot_l as i32, base_y);
            draw::dot_i(grid, foot_r as i32, base_y);
        }

        // Rose window: a small circle at the top center when near complete.
        if ctx.eased > 0.7 && dw >= 6 {
            let rose_p = ((ctx.eased - 0.7) / 0.3).min(1.0);
            let cx = (dw / 2) as f32;
            let cy = (dh as f32 * 0.15).max(2.0);
            let r = (dw as f32 * 0.07 * rose_p).max(1.0);
            // Outer ring.
            let spokes = 8usize;
            for i in 0..spokes {
                let angle = 2.0 * PI * i as f32 / spokes as f32 + ctx.time * 0.3;
                let x = (cx + r * angle.cos()).round() as i32;
                let y = (cy + r * angle.sin()).round() as i32;
                draw::dot_i(grid, x, y);
                // Spoke.
                line_dots(grid, cx.round() as i32, cy.round() as i32, x, y);
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Brick Wall
// ─────────────────────────────────────────────────────────────────────────────

/// Running-bond masonry laid course by course; trowel sweeps along each course.
struct BrickWall;
impl ProgressStyle for BrickWall {
    fn name(&self) -> &str {
        "brick-wall"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Running-bond brick courses laid row by row; trowel sweeps each course"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Each cell row is one brick course. Courses fill bottom-up.
        let total_courses = ch;
        let courses_done_f = ctx.eased * total_courses as f32;
        let courses_done = courses_done_f.floor() as usize;
        // Fractional course in progress.
        let partial_frac = courses_done_f.fract();

        // Brick width in cells: ~4 wide, 1 tall (one row = one brick height).
        let brick_w = (cw / 4).max(1);

        for course in 0..courses_done.min(total_courses) {
            let cell_y = total_courses.saturating_sub(1 + course);
            // Running bond: odd courses offset by half brick.
            let offset = if course % 2 == 1 { brick_w / 2 } else { 0 };

            let mut x = 0usize;
            while x < cw {
                // Mortar gap is just the cell boundary — use block glyphs.
                let bx = x;
                let inner = brick_w.saturating_sub(1);
                for bxi in 0..inner {
                    if bx + bxi < cw {
                        draw::glyph(grid, (bx + bxi + offset) % cw, cell_y, '█');
                    }
                }
                x += brick_w;
            }

            // Mortar line (top of each course): thin hline at top dot of cell.
            // The glyph rows cover it; mark with shade for mortar texture.
            if course > 0 {
                let mortar_y = total_courses.saturating_sub(1 + course);
                // Place ░ for mortar joints between bricks (at the vertical seam dots).
                let mut mx = offset;
                while mx < cw {
                    if mx > 0 {
                        draw::glyph(grid, mx % cw, mortar_y, '▏');
                    }
                    mx += brick_w;
                }
            }
        }

        // Partial course: trowel sweeps.
        if courses_done < total_courses {
            let cell_y = total_courses.saturating_sub(1 + courses_done);
            let trowel_x = (partial_frac * cw as f32) as usize;
            for bxi in 0..trowel_x.min(cw) {
                draw::shade(grid, bxi, cell_y, 2); // ▒ for freshly-laid course
            }
            // Trowel leading edge marker.
            if trowel_x < cw {
                draw::glyph(grid, trowel_x, cell_y, '▌');
            }
        }

        // Tint: warm brick orange-red gradient.
        let brick_start = crate::Color::rgb(200, 80, 40);
        let brick_end = crate::Color::rgb(230, 140, 80);
        for cx2 in 0..cw {
            let t = if cw <= 1 {
                0.5
            } else {
                cx2 as f32 / (cw - 1) as f32
            };
            let color = {
                let r =
                    (brick_start.r as f32 + (brick_end.r as f32 - brick_start.r as f32) * t) as u8;
                let g =
                    (brick_start.g as f32 + (brick_end.g as f32 - brick_start.g as f32) * t) as u8;
                let b =
                    (brick_start.b as f32 + (brick_end.b as f32 - brick_start.b as f32) * t) as u8;
                crate::Color::rgb(r, g, b)
            };
            let max_course_y = total_courses.saturating_sub(courses_done.min(total_courses));
            for cy2 in max_course_y..ch {
                draw::tint_row(grid, cy2, cx2, cx2, color);
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Pyramid
// ─────────────────────────────────────────────────────────────────────────────

/// Stepped stone courses narrow upward; the pyramid grows from base to apex.
struct Pyramid;
impl ProgressStyle for Pyramid {
    fn name(&self) -> &str {
        "pyramid"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Pyramid rises course by course; each step narrower than the last"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 2 || dh < 1 {
            return Ok(());
        }

        let max_courses = dh;
        let courses = (ctx.eased * max_courses as f32).ceil() as usize;
        let cx = dw / 2;

        for c in 0..courses.min(max_courses) {
            let y = dh.saturating_sub(1 + c);
            // Width tapers linearly: full at base, 1 dot at apex.
            let half_w = ((max_courses - c) * (dw / 2)) / max_courses.max(1);
            let x0 = cx.saturating_sub(half_w);
            let x1 = (cx + half_w).min(dw.saturating_sub(1));

            // Course line (step tread).
            draw::hline(grid, x0, x1, y);
            // Side walls (step risers).
            if c > 0 {
                draw::dot(grid, x0, y);
                draw::dot(grid, x1, y);
            }
        }

        // Tint from base (dark) to apex (bright).
        let (cw, ch) = grid.dimensions();
        for cy2 in 0..ch {
            let t = cy2 as f32 / ch.max(1) as f32;
            let color = ctx.palette.sample(1.0 - t);
            for cx2 in 0..cw {
                draw::tint_row(grid, cy2, cx2, cx2, color);
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Scaffolding
// ─────────────────────────────────────────────────────────────────────────────

/// Vertical poles and horizontal planks erect around a central building form.
struct Scaffolding;
impl ProgressStyle for Scaffolding {
    fn name(&self) -> &str {
        "scaffolding"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Scaffold poles and planks erect around a building; cross-braces appear last"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 4 || dh < 2 {
            return Ok(());
        }

        // Central building outline (always present, faint — just the rect).
        let bld_x0 = dw / 4;
        let bld_x1 = dw - dw / 4;
        let bld_y0 = dh / 4;
        let bld_y1 = dh.saturating_sub(1);
        draw::rect_outline(
            grid,
            bld_x0,
            bld_y0,
            bld_x1.saturating_sub(bld_x0),
            bld_y1.saturating_sub(bld_y0),
        );

        // Scaffold poles: progress reveals them left-to-right, inside-out.
        // Two poles at left and right of scaffold.
        let pole_left = bld_x0.saturating_sub(2);
        let pole_right = (bld_x1 + 2).min(dw.saturating_sub(1));

        // Pole heights grow with progress (bottom up).
        let pole_top_y = (dh as f32 * (1.0 - ctx.eased)).round() as usize;

        if ctx.eased > 0.0 {
            draw::vline(grid, pole_left, pole_top_y, dh.saturating_sub(1));
            draw::vline(grid, pole_right, pole_top_y, dh.saturating_sub(1));
        }

        // Horizontal planks: appear at intervals as progress advances.
        let plank_spacing = (dh / 4).max(2);
        let plank_count = dh / plank_spacing;
        let planks_shown = (ctx.eased * plank_count as f32).round() as usize;
        for p in 0..planks_shown.min(plank_count) {
            let py = dh.saturating_sub(1 + p * plank_spacing);
            draw::hline(grid, pole_left, pole_right, py);
        }

        // Cross-braces appear in the last 30% of progress.
        if ctx.eased > 0.7 {
            let brace_p = (ctx.eased - 0.7) / 0.3;
            let brace_top = (dh as f32 * (1.0 - brace_p)).round() as i32;
            // Left brace: ╲ from pole top-left to mid-right.
            let mid_x = ((pole_left + pole_right) / 2) as i32;
            let bot_y = dh as i32 - 1;
            line_dots(grid, pole_left as i32, brace_top, mid_x, bot_y);
            line_dots(grid, pole_right as i32, brace_top, mid_x, bot_y);
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Geodesic Dome
// ─────────────────────────────────────────────────────────────────────────────

/// Triangulated hemisphere wireframe assembles arc by arc as progress rises.
struct GeodesicDome;
impl ProgressStyle for GeodesicDome {
    fn name(&self) -> &str {
        "geodesic-dome"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Geodesic dome hemisphere assembles; triangulated arcs appear with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 4 || dh < 2 {
            return Ok(());
        }

        let cx = dw as f32 / 2.0;
        let cy = dh as f32 - 1.0; // dome sits on the baseline
        let r = ((dw as f32 / 2.0) - 1.0).min((dh as f32) - 1.0).max(1.0);

        // Baseline.
        draw::hline(grid, 0, dw.saturating_sub(1), dh.saturating_sub(1));

        // Main hemisphere arc (always drawn first).
        arc_quarter(grid, cx, cy, r, 0.0, PI);

        // Latitude rings: horizontal arcs at fractional heights.
        let lat_count = 4usize;
        let lats_shown = (ctx.eased * lat_count as f32).round() as usize;
        for li in 0..lats_shown.min(lat_count) {
            let frac = (li + 1) as f32 / (lat_count + 1) as f32;
            let lat_y_offset = r * frac; // screen offset from baseline upward
            let lat_r = (r * r - lat_y_offset * lat_y_offset).sqrt().max(0.5);
            let lat_cy = cy - lat_y_offset;
            // Draw the full horizontal arc at this latitude.
            arc_quarter(grid, cx, lat_cy, lat_r, 0.0, PI);
        }

        // Longitude meridian lines: spokes from base to apex.
        let meridian_count = 8usize;
        let meridians_shown = (ctx.eased * meridian_count as f32).round() as usize;
        for mi in 0..meridians_shown.min(meridian_count) {
            let angle = PI * mi as f32 / meridian_count as f32;
            let x0 = (cx + r * angle.cos()).round() as i32;
            let y0 = cy.round() as i32;
            let apex_x = cx.round() as i32;
            let apex_y = (cy - r).round() as i32;
            line_dots(grid, x0, y0, apex_x, apex_y);
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Spiral Staircase
// ─────────────────────────────────────────────────────────────────────────────

/// A perspective helix climbs with progress; treads project at diminishing depth.
struct SpiralStaircase;
impl ProgressStyle for SpiralStaircase {
    fn name(&self) -> &str {
        "spiral-staircase"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Perspective spiral staircase climbs; treads shrink with height"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 4 || dh < 4 {
            return Ok(());
        }

        let cx = dw as f32 / 2.0;
        // Number of turns in the helix.
        let turns = 2.0f32;
        let total_steps = (turns * 12.0) as usize;
        let steps_shown = (ctx.eased * total_steps as f32).round() as usize;

        // Animate rotation with time.
        let rot = ctx.time * 0.4;

        for s in 0..steps_shown.min(total_steps) {
            let t = s as f32 / total_steps as f32;
            let angle = 2.0 * PI * turns * t + rot;
            // Vertical position: top at t=1, bottom at t=0.
            let y_dot = (dh as f32 * (1.0 - t * 0.9)) as i32;
            // Horizontal radius shrinks slightly with height (perspective).
            let radius = (dw as f32 * 0.35) * (1.0 - t * 0.3);
            let x_dot = (cx + radius * angle.cos()).round() as i32;

            // Tread: short horizontal line at this step's position.
            let tread_len = (radius * 0.6).round() as i32;
            for tx in -tread_len..=tread_len {
                draw::dot_i(grid, x_dot + tx, y_dot);
            }
            // Riser dot.
            draw::dot_i(grid, x_dot, y_dot);
            draw::dot_i(grid, x_dot, y_dot + 1);

            // Central newel post.
            draw::dot_i(grid, cx.round() as i32, y_dot);
        }

        // Central post full height.
        for y in 0..dh {
            draw::dot(grid, dw / 2, y);
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Roman Aqueduct
// ─────────────────────────────────────────────────────────────────────────────

/// Round arches march left to right; the water channel deck appears last.
struct RomanAqueduct;
impl ProgressStyle for RomanAqueduct {
    fn name(&self) -> &str {
        "roman-aqueduct"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Roman aqueduct arches march across; water channel appears at completion"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 4 || dh < 3 {
            return Ok(());
        }

        let base_y = dh.saturating_sub(1);
        let arch_h = (dh as f32 * 0.6).round() as usize;
        let arch_w = (dw / 5).max(4); // width of each arch opening in dots
        let pier_w = (arch_w / 4).max(1);
        let arch_count = dw / (arch_w + pier_w);

        // How many arches revealed.
        let arches_shown = (ctx.eased * arch_count as f32).ceil() as usize;

        let mut x = 0usize;
        for a in 0..arches_shown.min(arch_count) {
            let _ = a;
            // Pier on left.
            for py in (base_y.saturating_sub(arch_h))..=base_y {
                draw::dot(grid, x, py);
                if pier_w > 1 {
                    draw::dot(grid, x + pier_w.saturating_sub(1), py);
                }
            }
            x += pier_w;

            // Semicircular arch.
            let arch_cx = (x + arch_w / 2) as f32;
            let arch_cy = base_y.saturating_sub(arch_h / 2) as f32;
            let arch_r = (arch_h / 2).max(1) as f32;
            arc_quarter(grid, arch_cx, arch_cy, arch_r, 0.0, PI);

            x += arch_w;
        }

        // Rightmost pier.
        if arches_shown > 0 && x < dw {
            for py in (base_y.saturating_sub(arch_h))..=base_y {
                draw::dot(grid, x.min(dw.saturating_sub(1)), py);
            }
        }

        // Water channel deck on top when complete.
        if ctx.eased > 0.9 {
            let deck_y = base_y.saturating_sub(arch_h);
            draw::hline(grid, 0, dw.saturating_sub(1), deck_y);
            // Water shimmer (animated dots).
            let shimmer_x = ((ctx.time * 3.0).sin() * dw as f32 * 0.5 + dw as f32 * 0.5) as usize;
            draw::dot(
                grid,
                shimmer_x.min(dw.saturating_sub(1)),
                deck_y.saturating_sub(1),
            );
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Classical Column
// ─────────────────────────────────────────────────────────────────────────────

/// A fluted Doric column shaft grows upward; the capital blooms at the top.
struct ClassicalColumn;
impl ProgressStyle for ClassicalColumn {
    fn name(&self) -> &str {
        "classical-column"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Classical column shaft rises with flutes; capital and entablature bloom last"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 4 || dh < 2 {
            return Ok(());
        }

        let cx = dw / 2;
        let shaft_half = (dw / 6).max(1);
        let x0 = cx.saturating_sub(shaft_half);
        let x1 = (cx + shaft_half).min(dw.saturating_sub(1));
        let base_y = dh.saturating_sub(1);
        let shaft_h = ((dh as f32 * 0.75) as usize).max(2);
        let shaft_top_y = base_y.saturating_sub(shaft_h);

        // Base / stylobate (always present).
        draw::hline(grid, 0, dw.saturating_sub(1), base_y);
        draw::hline(
            grid,
            x0.saturating_sub(1),
            (x1 + 1).min(dw.saturating_sub(1)),
            base_y.saturating_sub(1),
        );

        // Shaft grows upward.
        let shaft_drawn = (ctx.eased * shaft_h as f32).round() as usize;
        let shaft_drawn_y0 = base_y.saturating_sub(shaft_drawn);

        // Shaft outline: two vertical sides.
        draw::vline(grid, x0, shaft_drawn_y0, base_y.saturating_sub(1));
        draw::vline(grid, x1, shaft_drawn_y0, base_y.saturating_sub(1));

        // Flutes: vertical lines inside the shaft.
        let flute_count = ((x1 - x0) / 2).max(1);
        for f in 0..flute_count {
            let fx = x0 + 1 + f * 2;
            if fx < x1 {
                // Flute is dotted every 2 to suggest engraving.
                let mut fy = shaft_drawn_y0;
                while fy <= base_y.saturating_sub(1) {
                    draw::dot(grid, fx, fy);
                    fy += 2;
                }
            }
        }

        // Capital: only appears in the top 25% of progress.
        if ctx.eased > 0.75 {
            let cap_p = (ctx.eased - 0.75) / 0.25;
            let capital_top = shaft_top_y.saturating_sub((dh as f32 * 0.15 * cap_p) as usize);
            // Echinus (curved moulding).
            let spread = (shaft_half as f32 * cap_p * 0.5) as usize;
            let cap_x0 = x0.saturating_sub(spread);
            let cap_x1 = (x1 + spread).min(dw.saturating_sub(1));
            draw::hline(grid, cap_x0, cap_x1, shaft_top_y);
            // Abacus (flat slab).
            draw::hline(
                grid,
                cap_x0.saturating_sub(1),
                (cap_x1 + 1).min(dw.saturating_sub(1)),
                capital_top,
            );
            // Entablature (beam).
            if cap_p > 0.8 {
                draw::hline(grid, 0, dw.saturating_sub(1), capital_top.saturating_sub(1));
                draw::hline(grid, 0, dw.saturating_sub(1), capital_top.saturating_sub(2));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 11. Blueprint
// ─────────────────────────────────────────────────────────────────────────────

/// A drafting pen sweeps technical lines across graph paper; grid appears first.
struct Blueprint;
impl ProgressStyle for Blueprint {
    fn name(&self) -> &str {
        "blueprint"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Blueprint drafted: graph grid first, then walls, windows, and dimensions"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 4 || dh < 4 {
            return Ok(());
        }

        // Phase 1 (0–0.2): faint grid lines.
        if ctx.eased > 0.0 {
            let grid_spacing = 4usize;
            let grid_p = (ctx.eased / 0.2).min(1.0);
            let grid_cols = ((dw / grid_spacing) as f32 * grid_p) as usize;
            let grid_rows = ((dh / grid_spacing) as f32 * grid_p) as usize;
            for gx in 0..=grid_cols {
                let x = gx * grid_spacing;
                // Every other dot to keep it light.
                let mut y = 0usize;
                while y < dh {
                    draw::dot(grid, x.min(dw.saturating_sub(1)), y);
                    y += 2;
                }
            }
            for gy in 0..=grid_rows {
                let y = gy * grid_spacing;
                let mut x = 0usize;
                while x < dw {
                    draw::dot(grid, x, y.min(dh.saturating_sub(1)));
                    x += 2;
                }
            }
        }

        // Phase 2 (0.2–0.6): floor plan walls sweep in (outer rectangle).
        if ctx.eased > 0.2 {
            let wall_p = ((ctx.eased - 0.2) / 0.4).min(1.0);
            let x0 = dw / 8;
            let x1 = dw - dw / 8;
            let y0 = dh / 8;
            let y1 = dh - dh / 8;
            let perim = 2 * ((x1 - x0) + (y1 - y0));
            let drawn = (wall_p * perim as f32) as usize;
            // Walk the perimeter: top, right, bottom, left.
            let top_len = x1 - x0;
            let right_len = y1 - y0;
            let bot_len = x1 - x0;
            let left_len = y1 - y0;
            let mut rem = drawn;
            // Top.
            let seg = rem.min(top_len);
            draw::hline(grid, x0, x0 + seg, y0);
            rem = rem.saturating_sub(seg);
            // Right.
            let seg = rem.min(right_len);
            draw::vline(grid, x1, y0, y0 + seg);
            rem = rem.saturating_sub(seg);
            // Bottom (reversed).
            let seg = rem.min(bot_len);
            draw::hline(grid, x1.saturating_sub(seg), x1, y1);
            rem = rem.saturating_sub(seg);
            // Left (reversed).
            let seg = rem.min(left_len);
            draw::vline(grid, x0, y1.saturating_sub(seg), y1);
        }

        // Phase 3 (0.6–0.85): interior walls and door opening.
        if ctx.eased > 0.6 {
            let int_p = ((ctx.eased - 0.6) / 0.25).min(1.0);
            let mid_x = dw / 2;
            let y0 = dh / 8;
            let y1 = dh - dh / 8;
            let wall_len = (y1 - y0).saturating_sub(4); // door gap
            let drawn = (int_p * wall_len as f32) as usize;
            draw::vline(grid, mid_x, y0, y0 + drawn);
        }

        // Phase 4 (0.85–1.0): dimension lines + pen cursor.
        if ctx.eased > 0.85 {
            let dim_p = (ctx.eased - 0.85) / 0.15;
            // Dimension line along the top.
            let y_dim = (dh / 8).saturating_sub(2).max(0);
            let x_end = (dim_p * dw as f32) as usize;
            draw::hline(grid, 0, x_end.min(dw.saturating_sub(1)), y_dim);
            // Arrow heads.
            draw::dot(grid, 0, y_dim);
            let tail = x_end.min(dw.saturating_sub(1));
            draw::dot(grid, tail, y_dim);
        }

        // Pen cursor: a moving dot at the current draw frontier.
        let pen_x = (ctx.eased * dw as f32).round() as usize;
        let pen_y = ((ctx.time * 1.5).sin() * dh as f32 * 0.1 + dh as f32 * 0.5) as usize;
        draw::dot(
            grid,
            pen_x.min(dw.saturating_sub(1)),
            pen_y.min(dh.saturating_sub(1)),
        );

        // Tint: blueprint blue.
        let (cw, ch) = grid.dimensions();
        let blue = crate::Color::rgb(30, 80, 200);
        let light = crate::Color::rgb(100, 160, 255);
        for cx2 in 0..cw {
            for cy2 in 0..ch {
                let t = cx2 as f32 / cw.max(1) as f32;
                let color = {
                    let r = (blue.r as f32 + (light.r as f32 - blue.r as f32) * t) as u8;
                    let g = (blue.g as f32 + (light.g as f32 - blue.g as f32) * t) as u8;
                    let b = (blue.b as f32 + (light.b as f32 - blue.b as f32) * t) as u8;
                    crate::Color::rgb(r, g, b)
                };
                draw::tint_row(grid, cy2, cx2, cx2, color);
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 12. Tower Crane
// ─────────────────────────────────────────────────────────────────────────────

/// Mast rises, horizontal jib extends, a load hangs from the trolley and swings.
struct TowerCrane;
impl ProgressStyle for TowerCrane {
    fn name(&self) -> &str {
        "tower-crane"
    }
    fn theme(&self) -> &str {
        "architecture"
    }
    fn describe(&self) -> &str {
        "Tower crane mast rises, jib extends, trolley rolls, load swings on a cable"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw < 4 || dh < 4 {
            return Ok(());
        }

        let base_y = dh.saturating_sub(1);
        // Mast anchored at 1/4 width.
        let mast_x = dw / 4;

        // Mast grows to full height at progress 0.4.
        let mast_p = (ctx.eased / 0.4).min(1.0);
        let full_mast_h = dh.saturating_sub(2);
        let mast_h = (mast_p * full_mast_h as f32).round() as usize;
        let mast_top = base_y.saturating_sub(mast_h);

        draw::vline(grid, mast_x, mast_top, base_y);
        // Mast cross-bracing pattern (diagonal dots).
        let brace_spacing = (dh / 4).max(2);
        let mut by = mast_top;
        while by + brace_spacing <= base_y {
            line_dots(
                grid,
                mast_x as i32 - 1,
                by as i32,
                mast_x as i32 + 1,
                (by + brace_spacing) as i32,
            );
            line_dots(
                grid,
                mast_x as i32 + 1,
                by as i32,
                mast_x as i32 - 1,
                (by + brace_spacing) as i32,
            );
            by += brace_spacing;
        }

        // Jib extends right from mast top at progress 0.3–0.7.
        if ctx.eased > 0.3 {
            let jib_p = ((ctx.eased - 0.3) / 0.4).min(1.0);
            let max_jib = dw.saturating_sub(mast_x + 2);
            let jib_len = (jib_p * max_jib as f32).round() as usize;
            let jib_tip = (mast_x + jib_len).min(dw.saturating_sub(1));
            draw::hline(grid, mast_x, jib_tip, mast_top);

            // Counter-jib extends left.
            let cj_len = (jib_len / 3).max(1);
            let cj_start = mast_x.saturating_sub(cj_len);
            draw::hline(grid, cj_start, mast_x, mast_top);

            // Stays: angled cables from mast top to jib tip and counter-jib end.
            if jib_len > 2 {
                let stay_y = (mast_top + 1).min(base_y);
                line_dots(
                    grid,
                    mast_x as i32,
                    stay_y as i32,
                    jib_tip as i32,
                    mast_top as i32,
                );
                line_dots(
                    grid,
                    mast_x as i32,
                    stay_y as i32,
                    cj_start as i32,
                    mast_top as i32,
                );
            }

            // Trolley rolls along jib with time.
            if ctx.eased > 0.6 && jib_len > 1 {
                let trolley_norm = ((ctx.time * 0.5).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                let trolley_x = mast_x + (trolley_norm * jib_len as f32) as usize;
                let trolley_x = trolley_x.min(jib_tip);
                // Trolley body.
                draw::dot(grid, trolley_x, mast_top);
                draw::dot(grid, trolley_x, mast_top.saturating_add(1));

                // Load cable hangs from trolley.
                let cable_len = ((dh / 3) as f32 * ctx.eased) as usize;
                let load_y = (mast_top + 2 + cable_len).min(base_y);
                draw::vline(grid, trolley_x, mast_top + 2, load_y);

                // Load block.
                let lx = trolley_x as i32;
                let ly = load_y as i32;
                draw::dot_i(grid, lx - 1, ly);
                draw::dot_i(grid, lx, ly);
                draw::dot_i(grid, lx + 1, ly);
                draw::dot_i(grid, lx - 1, ly + 1);
                draw::dot_i(grid, lx, ly + 1);
                draw::dot_i(grid, lx + 1, ly + 1);
            }
        }

        // Ground baseline.
        draw::hline(grid, 0, dw.saturating_sub(1), base_y);

        // Tint: steel-grey to orange for the crane.
        let (cw, ch) = grid.dimensions();
        let steel = crate::Color::rgb(120, 130, 145);
        let orange = crate::Color::rgb(255, 140, 0);
        for cy2 in 0..ch {
            let t = cy2 as f32 / ch.max(1) as f32;
            let color = {
                let r = (orange.r as f32 + (steel.r as f32 - orange.r as f32) * t) as u8;
                let g = (orange.g as f32 + (steel.g as f32 - orange.g as f32) * t) as u8;
                let b = (orange.b as f32 + (steel.b as f32 - orange.b as f32) * t) as u8;
                crate::Color::rgb(r, g, b)
            };
            for cx2 in 0..cw {
                draw::tint_row(grid, cy2, cx2, cx2, color);
            }
        }

        Ok(())
    }
}
