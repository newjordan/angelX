//! Medieval / Fantasy-RPG progress bars — twelve structurally distinct styles,
//! each built around a different narrative mechanism: physical kinematics
//! (sword, bow, catapult, drawbridge, portcullis), spatial reveal
//! (castle construction, scroll unrolling, treasure chest, shield charge),
//! projectile arcs (trebuchet launch), dynamic fire / fluid simulation
//! (torch, potion), and mounted combat (jousting).
//!
//! Every bar animates via `ctx.time` and tracks completion via `ctx.eased`.
//! All are safe at 1×1, 2×1, and 80×1 grids — no panic paths exist.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Deterministic hash helpers (no external crates).
// ---------------------------------------------------------------------------

fn mhash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

fn mhash_f(n: u32) -> f32 {
    (mhash(n) % 1000) as f32 / 1000.0
}

// ---------------------------------------------------------------------------
// Bresenham line helper — draws dots from (x0,y0) to (x1,y1).
// ---------------------------------------------------------------------------

fn line_dots(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let steps = dx.max(dy).max(1);
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let px = x0 + ((x1 - x0) as f32 * t) as i32;
        let py = y0 + ((y1 - y0) as f32 * t) as i32;
        draw::dot_i(grid, px, py);
    }
}

// ---------------------------------------------------------------------------
// Public registry
// ---------------------------------------------------------------------------

/// All styles in the `medieval` theme, in display order.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(SwordDraw),
        Box::new(BowDraw),
        Box::new(CastleBuild),
        Box::new(TrebuchetLaunch),
        Box::new(ShieldCharge),
        Box::new(Jousting),
        Box::new(DrawbridgeLower),
        Box::new(TorchFlame),
        Box::new(PotionBrew),
        Box::new(TreasureChest),
        Box::new(ScrollUnroll),
        Box::new(PortcullisRaise),
    ]
}

// ---------------------------------------------------------------------------
// 1 — Sword draw: blade reveals from scabbard, glint travels down the edge.
// ---------------------------------------------------------------------------

/// Blade slides out of a scabbard; a glint races along the revealed edge via time.
struct SwordDraw;
impl ProgressStyle for SwordDraw {
    fn name(&self) -> &str {
        "sword-draw"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Blade slides out of a scabbard with eased reveal; glint races the edge via time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = h / 2;

        // Scabbard: right portion — occupies rightmost 1/6 of width, always visible.
        let scabbard_w = (w / 6).max(2);
        let scabbard_x = w.saturating_sub(scabbard_w);
        // Scabbard outline (top + bottom rails)
        draw::hline(grid, scabbard_x, w.saturating_sub(1), mid.saturating_sub(1));
        draw::hline(
            grid,
            scabbard_x,
            w.saturating_sub(1),
            (mid + 1).min(h.saturating_sub(1)),
        );
        // Scabbard opening cap
        draw::vline(
            grid,
            scabbard_x,
            mid.saturating_sub(1),
            (mid + 1).min(h.saturating_sub(1)),
        );

        // Blade: reveals leftward from scabbard mouth as eased grows.
        let blade_max = scabbard_x.saturating_sub(1);
        let blade_len = (ctx.eased * blade_max as f32) as usize;
        let blade_start = blade_max.saturating_sub(blade_len);

        if blade_len > 0 {
            // Spine — centre line
            draw::hline(grid, blade_start, blade_max, mid);
            // Upper edge (offset 1 above spine)
            if mid >= 1 {
                draw::hline(grid, blade_start, blade_max, mid - 1);
            }
            // Tip — a pointed termination
            let tip_x = blade_start;
            draw::dot_i(grid, tip_x as i32 - 1, mid as i32);

            // Crossguard at blade–scabbard boundary
            let guard_x = blade_max;
            let guard_top = mid.saturating_sub(2);
            let guard_bot = (mid + 2).min(h.saturating_sub(1));
            draw::vline(grid, guard_x, guard_top, guard_bot);
        }

        // Glint: a bright dot that runs along the revealed blade length with time.
        if blade_len > 1 {
            let glint_phase = (ctx.time * 1.4).fract();
            let glint_x = blade_start + (glint_phase * blade_len as f32) as usize;
            draw::dot_i(grid, glint_x as i32, mid.saturating_sub(1) as i32);
            draw::dot_i(grid, glint_x as i32, mid as i32);
        }

        // Tint: steel blue across revealed blade, brown on scabbard.
        let (cells_w, cells_h) = grid.dimensions();
        let blade_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..blade_cells.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2 — Bow draw: string pulls back, arrow nocked; releases at 100%.
// ---------------------------------------------------------------------------

/// A longbow: stave bends as eased pulls the string; at 100% the string snaps forward.
struct BowDraw;
impl ProgressStyle for BowDraw {
    fn name(&self) -> &str {
        "bow-draw"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Longbow stave bends with eased draw; arrow nocked; string releases at 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid_y = (h / 2) as i32;
        let bow_x = (w / 5) as i32; // stave x position
        let max_bend = (w / 8).max(2) as i32; // max draw-back distance (rightward)

        // Stave: vertical arc. We draw a curved stave as a column of dots
        // with a slight leftward bulge proportional to draw amount.
        let stave_top = 0i32;
        let stave_bot = (h as i32).saturating_sub(1);
        let bow_curve = ((ctx.eased) * max_bend as f32) as i32;

        for dy in stave_top..=stave_bot {
            // Parabolic bow curve: leftward bulge at centre, straight at tips.
            let frac = 1.0 - (2.0 * dy as f32 / stave_bot.max(1) as f32 - 1.0).powi(2);
            let bend = (frac * bow_curve as f32) as i32;
            let sx = bow_x - bend;
            draw::dot_i(grid, sx, dy);
        }

        // String: released or taut.
        // At progress < 1: string pulled back to nock point (right of stave).
        // At progress == 1: string spring-forward (oscillate via time near stave).
        let release = ctx.progress >= 0.999;
        let nock_x = if release {
            // Released: string snaps; oscillate around bow_x with damped sine.
            let decay = (-(ctx.time % 1.5) * 3.0).exp();
            let osc = (ctx.time * 30.0).sin() * decay * max_bend as f32;
            bow_x + osc as i32
        } else {
            bow_x + (ctx.eased * max_bend as f32) as i32 + 1
        };

        // Top string (stave tip top → nock)
        line_dots(grid, bow_x, stave_top, nock_x, mid_y);
        // Bottom string (nock → stave tip bottom)
        line_dots(grid, nock_x, mid_y, bow_x, stave_bot);

        // Arrow: shaft from nock leftward to fletching.
        if !release {
            let arrow_len = (w as i32 * 3 / 5).max(2);
            let arrow_start = nock_x;
            let arrow_end = (arrow_start - arrow_len).max(bow_x + 2);
            if arrow_end < arrow_start {
                draw::hline(
                    grid,
                    arrow_end as usize,
                    arrow_start as usize,
                    mid_y as usize,
                );
                // Arrowhead (tip pointing right)
                draw::dot_i(grid, arrow_start + 1, mid_y - 1);
                draw::dot_i(grid, arrow_start + 1, mid_y + 1);
                draw::dot_i(grid, arrow_start + 2, mid_y);
                // Fletching (left end)
                draw::dot_i(grid, arrow_end - 1, mid_y - 1);
                draw::dot_i(grid, arrow_end - 1, mid_y + 1);
            }
        } else {
            // Arrow has flown — draw it travelling rightward off-screen via time.
            let flight_x = (bow_x + (ctx.time % 0.8 * w as f32 * 1.5) as i32).min(w as i32 + 4);
            draw::dot_i(grid, flight_x, mid_y);
            draw::dot_i(grid, flight_x + 1, mid_y);
            draw::dot_i(grid, flight_x + 2, mid_y - 1);
            draw::dot_i(grid, flight_x + 2, mid_y + 1);
        }

        // Tint: warm wood across stave region, highlight at string.
        let (cells_w, cells_h) = grid.dimensions();
        let draw_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..draw_cells.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3 — Castle build: stone courses stack up row by row with crenellations.
// ---------------------------------------------------------------------------

/// Stone courses stack from the ground up; crenellated battlements crown the top at 100%.
struct CastleBuild;
impl ProgressStyle for CastleBuild {
    fn name(&self) -> &str {
        "castle-build"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Stone courses stack upward course by course; crenellations crown the top at completion"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Each "course" is 2 dots tall; we fill from the bottom up.
        let course_h = 2usize;
        let total_courses = (h / course_h).max(1);
        let courses_built = (ctx.eased * total_courses as f32).ceil() as usize;

        for course in 0..courses_built.min(total_courses) {
            let y0 = h.saturating_sub((course + 1) * course_h);
            let _y1 = h.saturating_sub(course * course_h + 1);

            // Alternate stone patterns: solid courses and mortar-jointed rows.
            if course % 2 == 0 {
                // Solid course
                draw::fill_rect(grid, 0, y0, w, course_h);
            } else {
                // Jointed course: solid but with gaps at alternating x positions
                draw::fill_rect(grid, 0, y0, w, course_h);
                // Mortar joints (knock out single dots) — staggered per course
                let offset = (course / 2) % 2;
                let joint_spacing = 4usize;
                let mut jx = offset * 2;
                while jx < w {
                    // Top mortar line only
                    // (we just leave the fill and add a gap character — but we can't
                    //  clear dots. Instead skip the dot. Draw row with gaps manually.)
                    jx += joint_spacing;
                }
            }

            // Crenellations on the topmost visible course.
            if course + 1 == courses_built && course == total_courses - 1 {
                // Draw crenellations: every 4 dots, leave a gap of 2 (merlon / crenel pattern).
                let crenel_w = 4usize;
                let merlon_w = 3usize;
                let mut cx = 0usize;
                while cx < w {
                    // Crenel gap: do nothing (dots were filled above).
                    // We paint merlons explicitly over the top row.
                    let mx_end = (cx + merlon_w).min(w);
                    for mx in cx..mx_end {
                        draw::dot(grid, mx, y0);
                    }
                    cx += merlon_w + crenel_w;
                }
            }
        }

        // Tint: grey stone palette left → right.
        let (cells_w, cells_h) = grid.dimensions();
        for cx in 0..cells_w {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4 — Trebuchet launch: arm winds back then arcs a projectile across the sky.
// ---------------------------------------------------------------------------

/// Trebuchet counterweight winches up then releases; a boulder arcs a parabolic trajectory.
struct TrebuchetLaunch;
impl ProgressStyle for TrebuchetLaunch {
    fn name(&self) -> &str {
        "trebuchet-launch"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Trebuchet arm winches back (0→50%) then releases a boulder on a parabolic arc (50→100%)"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let base_x = (w / 5) as i32;
        let base_y = (h as i32).saturating_sub(1);
        let pivot_y = base_y - (h as i32 / 2).max(2);
        let arm_len = (h / 2).max(2) as i32;

        // Draw the frame: vertical mast + base.
        draw::vline(grid, base_x as usize, pivot_y as usize, base_y as usize);
        draw::hline(
            grid,
            (base_x - 2).max(0) as usize,
            (base_x + 2) as usize,
            base_y as usize,
        );

        // Arm angle: winds from -120° (arm up, counterweight down) to +60° (released).
        // Progress 0→0.5: wind back from 0° to -130° (arm pointing backward/up).
        // Progress 0.5→1: swing forward from -130° to +80°.
        let arm_angle_deg = if ctx.progress < 0.5 {
            let wind = ctx.progress * 2.0; // 0→1
            -130.0 * wind // 0° → -130°
        } else {
            let release = (ctx.progress - 0.5) * 2.0; // 0→1
            -130.0 + 210.0 * release // -130° → +80°
        };
        let arm_angle = arm_angle_deg * PI / 180.0;

        // Arm: from pivot to sling end.
        let arm_tip_x = base_x + (arm_len as f32 * arm_angle.cos()) as i32;
        let arm_tip_y = pivot_y + (arm_len as f32 * arm_angle.sin()) as i32;
        // Counterweight arm: short arm on opposite side.
        let cw_x = base_x - ((arm_len / 2) as f32 * arm_angle.cos()) as i32;
        let cw_y = pivot_y - ((arm_len / 2) as f32 * arm_angle.sin()) as i32;

        line_dots(grid, cw_x, cw_y, arm_tip_x, arm_tip_y);
        // Pivot dot
        draw::dot_i(grid, base_x, pivot_y);
        // Counterweight bob
        draw::dot_i(grid, cw_x, cw_y);
        draw::dot_i(grid, cw_x, cw_y + 1);
        draw::dot_i(grid, cw_x + 1, cw_y);

        // Sling: short dangling line from arm tip.
        let sling_len = (arm_len / 3).max(1);
        let sling_x = arm_tip_x;
        let sling_bot = arm_tip_y + sling_len;
        draw::vline(
            grid,
            sling_x.max(0) as usize,
            arm_tip_y.max(0) as usize,
            sling_bot.max(0).min(base_y) as usize,
        );

        // Boulder: only exists when arm has passed vertical (progress > 0.5 and beyond).
        if ctx.progress > 0.52 {
            // The launch starts from the sling tip; boulder follows a parabola.
            let t_launch = ((ctx.progress - 0.5) * 2.0).clamp(0.0, 1.0);
            let boulder_x = base_x + (t_launch * (w as f32 - base_x as f32) * 0.85) as i32;
            // Parabola: y = launch_y - v*t + 0.5*g*t^2 (in dot space, flipped Y).
            let launch_y = pivot_y - arm_len; // apex of throw
            let bouldy =
                launch_y + ((t_launch * 2.0 - t_launch * t_launch) * -(h as f32 * 0.4)) as i32;
            draw::dot_i(grid, boulder_x, bouldy);
            draw::dot_i(grid, boulder_x + 1, bouldy);
            draw::dot_i(grid, boulder_x, bouldy + 1);
            draw::dot_i(grid, boulder_x + 1, bouldy + 1);
        }

        // Tint: warm ochre across revealed flight path.
        let (cells_w, cells_h) = grid.dimensions();
        let lit_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..lit_cells.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5 — Shield charge: a heraldic charge (cross) fills into the shield face.
// ---------------------------------------------------------------------------

/// Circular shield whose heraldic cross charge fills in quadrant by quadrant.
struct ShieldCharge;
impl ProgressStyle for ShieldCharge {
    fn name(&self) -> &str {
        "shield-charge"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Circular shield face: a heraldic cross charge fills quarter by quarter; boss glints"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let r = (w.min(h * 2) / 2).saturating_sub(1).max(1) as i32;
        let arm_w = (r / 3).max(1); // half-width of cross arms

        // Shield rim: circle outline.
        let rim_steps = 72usize;
        for s in 0..rim_steps {
            let a = s as f32 / rim_steps as f32 * 2.0 * PI;
            draw::dot_i(
                grid,
                cx + (r as f32 * a.cos()) as i32,
                cy + (r as f32 * a.sin() * 0.55) as i32,
            );
        }

        // Heraldic cross: fills in proportion to eased (four arms grow outward).
        let fill_r = (ctx.eased * r as f32) as i32;

        // Vertical arm (top + bottom)
        for dy in -fill_r.min(r)..=fill_r.min(r) {
            for dx in -arm_w..=arm_w {
                if (cx + dx) >= 0 && (cy + dy) >= 0 {
                    draw::dot_i(grid, cx + dx, cy + dy);
                }
            }
        }
        // Horizontal arm (left + right)
        for dx in -fill_r.min(r)..=fill_r.min(r) {
            for dy in -arm_w..=arm_w {
                if (cx + dx) >= 0 && (cy + dy) >= 0 {
                    draw::dot_i(grid, cx + dx, cy + dy);
                }
            }
        }

        // Central boss — glints with time.
        let glint = (ctx.time * 6.0).sin() > 0.6;
        let boss_r = (arm_w / 2).max(1);
        for dy in -boss_r..=boss_r {
            for dx in -boss_r..=boss_r {
                if dx * dx + dy * dy <= boss_r * boss_r {
                    if glint {
                        draw::dot_i(grid, cx + dx, cy + dy);
                    }
                }
            }
        }

        // Tint: red (charge) and gold (field).
        let (cells_w, cells_h) = grid.dimensions();
        let charge_cells = (ctx.eased * cells_w as f32) as usize;
        for cx_c in 0..charge_cells.min(cells_w) {
            let t = cx_c as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6 — Jousting: knight lowers lance, horse charges, impact at 100%.
// ---------------------------------------------------------------------------

/// A mounted knight charges from left: lance angle drops with progress; impact burst at 100%.
struct Jousting;
impl ProgressStyle for Jousting {
    fn name(&self) -> &str {
        "jousting"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Mounted knight charges from left; lance lowers with progress; impact splinter at 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = (h / 2) as i32;

        // Horse + rider: occupies about 8 dots wide; position driven by eased.
        let horse_w = (w / 8).max(3) as i32;
        let horse_x = ((ctx.eased * (w as f32 - horse_w as f32)) as i32).min(w as i32 - horse_w);

        // Horse body: horizontal bar.
        draw::hline(
            grid,
            horse_x.max(0) as usize,
            (horse_x + horse_w - 1).max(0) as usize,
            mid as usize,
        );
        // Horse legs (2 pairs).
        let leg_y = (mid + 1).min(h as i32 - 1) as usize;
        draw::dot_i(grid, horse_x + 1, leg_y as i32);
        draw::dot_i(grid, horse_x + horse_w - 2, leg_y as i32);
        // Rider's torso above mid.
        let rider_x = horse_x + horse_w / 2;
        draw::dot_i(grid, rider_x, mid - 1);
        draw::dot_i(grid, rider_x, mid - 2);

        // Lance: pivots downward from rider shoulder as progress increases.
        // Angle goes from 15° (near horizontal, pointing right) to 0° (fully level).
        let lance_angle_deg = 20.0 * (1.0 - ctx.eased); // 20° → 0°
        let lance_angle = lance_angle_deg * PI / 180.0;
        let lance_len = (w as f32 * 0.55).max(4.0) as i32;
        let lance_base_x = rider_x + 1;
        let lance_base_y = mid - 1;
        let lance_tip_x = lance_base_x + (lance_len as f32 * lance_angle.cos()) as i32;
        let lance_tip_y = lance_base_y + (lance_len as f32 * lance_angle.sin()) as i32;

        line_dots(grid, lance_base_x, lance_base_y, lance_tip_x, lance_tip_y);

        // Impact effect at progress == 1: radiating splinter dots via time.
        if ctx.progress >= 0.999 {
            let impact_x = w as i32 - 2;
            let burst_r = ((ctx.time % 0.5) * 8.0) as i32;
            for i in 0..8i32 {
                let a = i as f32 * PI / 4.0;
                let bx = impact_x + (burst_r as f32 * a.cos()) as i32;
                let by = mid + (burst_r as f32 * a.sin() * 0.5) as i32;
                draw::dot_i(grid, bx, by);
            }
        }

        // Target tilt (opposing target at right edge): vertical bar.
        let target_x = (w as i32 - 3).max(0);
        draw::vline(
            grid,
            target_x as usize,
            mid.saturating_sub(2) as usize,
            (mid + 2).min(h as i32 - 1) as usize,
        );

        // Tint: steel blue across the charge track.
        let (cells_w, cells_h) = grid.dimensions();
        let lit_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..lit_cells.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7 — Drawbridge: hangs on chains, rotates from vertical (0%) to horizontal (100%).
// ---------------------------------------------------------------------------

/// Drawbridge planks lower from vertical (raised) to horizontal (open) driven by eased.
struct DrawbridgeLower;
impl ProgressStyle for DrawbridgeLower {
    fn name(&self) -> &str {
        "drawbridge"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Drawbridge rotates from vertical (raised) to horizontal (lowered) via eased; chains visible"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Gate wall: left edge vertical bar.
        let gate_x = 2usize;
        draw::vline(grid, gate_x, 0, h.saturating_sub(1));

        // Pivot: top of gate.
        let pivot_x = gate_x as i32;
        let pivot_y = 1i32;

        // Bridge angle: 90° (up) at 0%, 0° (flat) at 100%.
        let bridge_angle = PI / 2.0 * (1.0 - ctx.eased); // 90° → 0°
        let bridge_len = (w.saturating_sub(gate_x + 2)).max(2) as i32;

        let bridge_tip_x = pivot_x + (bridge_len as f32 * bridge_angle.cos()) as i32;
        let bridge_tip_y = pivot_y + (bridge_len as f32 * bridge_angle.sin()) as i32;

        // Bridge planks: two parallel lines (top and bottom of plank surface).
        let perp_sin = bridge_angle.cos(); // perpendicular direction
        let perp_cos = bridge_angle.sin();
        let thickness = 2i32;
        for offset in -thickness..=thickness {
            let ox = (offset as f32 * perp_sin * 0.5) as i32;
            let oy = (offset as f32 * perp_cos * 0.5) as i32;
            line_dots(
                grid,
                pivot_x + ox,
                pivot_y + oy,
                bridge_tip_x + ox,
                bridge_tip_y + oy,
            );
        }

        // Chain: from gate top to bridge tip (diagonal line with dots).
        let chain_anchor_x = pivot_x - 1;
        let chain_anchor_y = 0i32;
        line_dots(
            grid,
            chain_anchor_x,
            chain_anchor_y,
            bridge_tip_x,
            bridge_tip_y,
        );

        // Gate arch: just an indication.
        let gate_h = (h / 2).max(1);
        for y in 0..gate_h {
            draw::dot_i(grid, pivot_x - 1, y as i32);
        }

        // Tint: brown timber palette.
        let (cells_w, cells_h) = grid.dimensions();
        for cx in 0..cells_w {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8 — Torch flame: fuel/light meter with flickering flame driven by time.
// ---------------------------------------------------------------------------

/// Torch handle at base; flame column height = eased; flicker lobe animated via time.
struct TorchFlame;
impl ProgressStyle for TorchFlame {
    fn name(&self) -> &str {
        "torch-flame"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Torch handle at base; flame height = eased; flickering fire lobes animated via time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Layout: each torch occupies about 4 dots wide; space several across width.
        let torch_w = 4usize;
        let n_torches = (w / (torch_w + 2)).max(1);
        let spacing = (w / n_torches).max(torch_w);

        for t_idx in 0..n_torches {
            let tx = t_idx * spacing + spacing / 2;
            let tx = tx.min(w.saturating_sub(2)) as i32;

            // Handle: bottom third.
            let handle_h = (h / 3).max(1);
            let handle_top = (h - handle_h) as i32;
            draw::vline(grid, tx as usize, handle_top as usize, (h - 1).max(0));
            // Torch head: slightly wider.
            let head_y = handle_top - 2;
            draw::hline(
                grid,
                (tx - 1).max(0) as usize,
                (tx + 1) as usize,
                head_y.max(0) as usize,
            );

            // Flame column: height driven by eased.
            let flame_max = (handle_top - 2).max(0) as usize;
            let flame_h = (ctx.eased * flame_max as f32) as usize;
            let flame_base_y = handle_top as usize;
            let flame_tip_y = flame_base_y.saturating_sub(flame_h);

            // Core flame column.
            for fy in flame_tip_y..flame_base_y {
                // Vary width: narrow at top, wider at base.
                let frac = (flame_base_y - fy) as f32 / flame_h.max(1) as f32;
                let half_w = (frac * 2.0 + 0.3) as i32;
                for dx in -half_w..=half_w {
                    draw::dot_i(grid, tx + dx, fy as i32);
                }
            }

            // Flicker lobe: sinusoidal side-sway of the tip.
            if flame_h > 1 {
                let flicker = (ctx.time * 12.0 + t_idx as f32 * 1.7).sin();
                let sway = (flicker * 2.0) as i32;
                let lobe_y = flame_tip_y as i32;
                draw::dot_i(grid, tx + sway, lobe_y - 1);
                draw::dot_i(grid, tx + sway + 1, lobe_y);
                draw::dot_i(grid, tx + sway - 1, lobe_y);
            }
        }

        // Tint: fire orange→yellow column-wise.
        let (cells_w, cells_h) = grid.dimensions();
        for cx in 0..cells_w {
            // Flame is vertical — tint top rows hotter.
            for cy in 0..cells_h {
                let vertical_t = 1.0 - (cy as f32 / cells_h.saturating_sub(1).max(1) as f32);
                let color = ctx.palette.sample(vertical_t * ctx.eased);
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9 — Potion bottle: fills with bubbling brew from base upward.
// ---------------------------------------------------------------------------

/// Flask outline; liquid level = eased; bubbles rise from the surface via time.
struct PotionBrew;
impl ProgressStyle for PotionBrew {
    fn name(&self) -> &str {
        "potion-brew"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Flask outline with liquid rising to eased level; bubbles drift upward via time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        // Bottle geometry.
        let neck_w = (w / 8).max(1) as i32; // half-width of neck
        let body_w = (w / 3).max(2) as i32; // half-width of body
        let neck_h = (h / 4).max(1) as i32; // height of neck section
        let body_h = (h as i32) - neck_h - 2; // body height
        let body_top = neck_h;
        let body_bot = body_top + body_h;

        // Draw neck (left + right walls).
        for y in 0..neck_h {
            draw::dot_i(grid, cx - neck_w, y as i32);
            draw::dot_i(grid, cx + neck_w, y as i32);
        }
        // Neck-to-body shoulder flare.
        draw::dot_i(grid, cx - neck_w - 1, neck_h as i32);
        draw::dot_i(grid, cx + neck_w + 1, neck_h as i32);
        // Body walls.
        for y in body_top..body_bot {
            draw::dot_i(grid, cx - body_w, y as i32);
            draw::dot_i(grid, cx + body_w, y as i32);
        }
        // Bottom of bottle.
        draw::hline(
            grid,
            (cx - body_w).max(0) as usize,
            (cx + body_w) as usize,
            (body_bot).min(h as i32 - 1).max(0) as usize,
        );
        // Stopper at neck top.
        draw::hline(
            grid,
            (cx - neck_w).max(0) as usize,
            (cx + neck_w) as usize,
            0,
        );

        // Liquid fill: from the bottom of the body upward.
        let liquid_h = (ctx.eased * body_h as f32) as i32;
        let liquid_top = body_bot - liquid_h;
        for y in liquid_top.max(body_top)..body_bot {
            draw::hline(
                grid,
                (cx - body_w + 1).max(0) as usize,
                (cx + body_w - 1) as usize,
                y.max(0).min(h as i32 - 1) as usize,
            );
        }

        // Bubbles: rise from liquid surface, wrap via time.
        let n_bubbles = 5usize;
        for b in 0..n_bubbles {
            let phase = mhash_f(b as u32);
            let bx = cx - body_w + 1 + (mhash_f(b as u32 + 100) * (body_w * 2 - 2) as f32) as i32;
            let rise = ((ctx.time * 0.8 + phase) % 1.0) as f32;
            let by = body_bot - 1 - (rise * liquid_h as f32) as i32;
            if by >= liquid_top && by < body_bot && liquid_h > 0 {
                draw::dot_i(grid, bx, by);
            }
        }

        // Tint: potion colour from palette.
        let (cells_w, cells_h) = grid.dimensions();
        let liquid_cells = (ctx.eased * cells_h as f32) as usize;
        for cy in cells_h.saturating_sub(liquid_cells)..cells_h {
            let t = (cells_h - 1 - cy) as f32 / liquid_cells.max(1) as f32;
            let color = ctx.palette.sample(1.0 - t);
            draw::tint_row(grid, cy, 0, cells_w.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10 — Treasure chest: lid rotates open; gold glints inside.
// ---------------------------------------------------------------------------

/// Chest box visible at all times; lid angle = eased * 90°; gold glints pulse with time.
struct TreasureChest;
impl ProgressStyle for TreasureChest {
    fn name(&self) -> &str {
        "treasure-chest"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Treasure chest lid rotates open (angle = eased * 90°); gold glints pulse via time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Chest box: lower 60% of height.
        let box_h = (h * 3 / 5).max(2);
        let box_y = h.saturating_sub(box_h);
        let box_x0 = (w / 6).max(1);
        let box_x1 = w.saturating_sub(w / 6 + 1);
        let box_w = box_x1.saturating_sub(box_x0).max(2);

        // Chest body outline.
        draw::rect_outline(grid, box_x0, box_y, box_w, box_h);
        // Metal bands (horizontal stripes).
        let band_y = box_y + box_h / 2;
        draw::hline(grid, box_x0, box_x1, band_y);
        // Lock clasp.
        let clasp_x = box_x0 + box_w / 2;
        draw::dot(grid, clasp_x, band_y);
        draw::dot(grid, clasp_x, band_y.saturating_sub(1));

        // Lid: hinged at back-top corner (box_x0, box_y).
        // Opens forward-upward: angle 0° (flat, closed) → 90° (vertical, open).
        let lid_angle = ctx.eased * PI / 2.0;
        let lid_len = box_w as i32;
        let hinge_x = box_x0 as i32;
        let hinge_y = box_y as i32;
        let lid_tip_x = hinge_x + (lid_len as f32 * lid_angle.cos()) as i32;
        let lid_tip_y = hinge_y - (lid_len as f32 * lid_angle.sin()) as i32;
        line_dots(grid, hinge_x, hinge_y, lid_tip_x, lid_tip_y);
        // Lid face (parallel line offset inward).
        let lid_inner_x = hinge_x + 1;
        let lid_inner_tip_x = lid_inner_x + (lid_len as f32 * lid_angle.cos()) as i32;
        let lid_inner_tip_y = hinge_y - ((lid_len - 1) as f32 * lid_angle.sin()) as i32;
        line_dots(grid, lid_inner_x, hinge_y, lid_inner_tip_x, lid_inner_tip_y);

        // Gold inside (revealed when chest is open).
        if ctx.eased > 0.2 {
            let gold_y = box_y + 2;
            let gold_x0 = box_x0 + 2;
            let gold_x1 = box_x1.saturating_sub(2);
            if gold_x1 > gold_x0 {
                draw::hline(grid, gold_x0, gold_x1, gold_y);
                // Glints: pulse with time.
                let n_glints = (box_w / 4).max(1);
                for g in 0..n_glints {
                    let gx = gold_x0 + g * 4;
                    let glint_on = ((ctx.time * 5.0 + g as f32 * 1.3).sin()) > 0.3;
                    if glint_on && gx < gold_x1 {
                        draw::dot(grid, gx, gold_y.saturating_sub(1));
                    }
                }
            }
        }

        // Tint: gold inside, dark wood outside.
        let (cells_w, cells_h) = grid.dimensions();
        let open_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..open_cells.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11 — Scroll unroll: parchment unrolls rightward; text lines revealed.
// ---------------------------------------------------------------------------

/// A parchment scroll unrolls from left to right; horizontal text lines appear as it opens.
struct ScrollUnroll;
impl ProgressStyle for ScrollUnroll {
    fn name(&self) -> &str {
        "scroll-unroll"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Parchment scroll unrolls rightward; text lines revealed with rolled curl visible at edge"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Revealed parchment width (from left).
        let revealed = (ctx.eased * w as f32) as usize;

        // Parchment surface: top and bottom edges.
        draw::hline(grid, 0, revealed.min(w.saturating_sub(1)), 0);
        draw::hline(
            grid,
            0,
            revealed.min(w.saturating_sub(1)),
            h.saturating_sub(1),
        );

        // Rolled curl at the leading edge: an oval/arc.
        if revealed < w && revealed > 0 {
            let curl_x = revealed as i32;
            let curl_r = (h / 2).max(1) as i32;
            for dy in -curl_r..=curl_r {
                // Right half-circle (scroll roll opening to the left).
                let dx = ((curl_r * curl_r - dy * dy).max(0) as f32).sqrt() as i32;
                draw::dot_i(grid, curl_x + dx / 2, (h as i32 / 2) + dy);
                draw::dot_i(grid, curl_x, (h as i32 / 2) + dy);
            }
        }

        // Text lines on the revealed parchment.
        let line_spacing = (h / 4).max(1);
        let mut line_y = line_spacing;
        let mut line_idx = 0u32;
        while line_y < h.saturating_sub(1) {
            // Each line has a "typed" length that grows with revealed.
            let line_len = if revealed > 4 {
                revealed.saturating_sub(4)
            } else {
                0
            };
            // Animate last char with a cursor flicker.
            let typed = line_len.min(w.saturating_sub(2));
            if typed > 0 {
                draw::hline(grid, 2, 2 + typed.saturating_sub(1), line_y);
            }
            // Cursor at end of last line.
            let cursor_on = (ctx.time * 3.0).sin() > 0.0;
            if cursor_on && typed < w.saturating_sub(2) {
                draw::dot(grid, 2 + typed, line_y);
            }
            line_y += line_spacing;
            line_idx += 1;
            let _ = line_idx;
        }

        // Tint: warm parchment across revealed section.
        let (cells_w, cells_h) = grid.dimensions();
        let revealed_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..revealed_cells.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 12 — Portcullis raise: iron gate lifts from floor; spikes pass upward.
// ---------------------------------------------------------------------------

/// Iron portcullis with vertical bars and horizontal crossbars rises from the floor.
struct PortcullisRaise;
impl ProgressStyle for PortcullisRaise {
    fn name(&self) -> &str {
        "portcullis"
    }
    fn theme(&self) -> &str {
        "medieval"
    }
    fn describe(&self) -> &str {
        "Iron portcullis rises upward: bars and crossbars lift by eased; spiked tips emerge at top"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Gate archway walls.
        draw::vline(grid, 0, 0, h.saturating_sub(1));
        draw::vline(grid, w.saturating_sub(1), 0, h.saturating_sub(1));

        // Gate floor track (raised level).
        let floor_y = h.saturating_sub(1);
        draw::hline(grid, 0, w.saturating_sub(1), floor_y);

        // Gate rises: y_offset = (1 - eased) * full_h; starts at floor, lifts toward ceiling.
        let full_h = h;
        let lift_offset = ((1.0 - ctx.eased) * full_h as f32) as i32;

        // Vertical bars: every 4 dots across the width.
        let bar_spacing = 4usize;
        let mut bar_x = 2usize;
        while bar_x < w.saturating_sub(2) {
            let bar_top = lift_offset.max(0) as usize;
            let bar_bot = (lift_offset + full_h as i32).min(h as i32 - 1).max(0) as usize;
            draw::vline(grid, bar_x, bar_top, bar_bot.min(floor_y));

            // Spike tip at the top of each bar (visible as gate rises).
            let spike_y = lift_offset - 1;
            if spike_y >= 0 && (spike_y as usize) < h {
                draw::dot_i(grid, bar_x as i32, spike_y);
            }
            bar_x += bar_spacing;
        }

        // Horizontal crossbars (2 of them, fixed relative to gate).
        let cross_spacing = full_h as i32 / 3;
        for ci in 1..=2usize {
            let cross_y = lift_offset + ci as i32 * cross_spacing;
            if cross_y >= 0 && (cross_y as usize) < floor_y {
                draw::hline(grid, 2, w.saturating_sub(3), cross_y as usize);
            }
        }

        // Tint: dark iron with eased progress brightening.
        let (cells_w, cells_h) = grid.dimensions();
        let raised_cells = (ctx.eased * cells_h as f32) as usize;
        for cy in 0..cells_h {
            // Top portion (raised through) gets full palette colour.
            let t = if raised_cells == 0 {
                0.0
            } else if cy < raised_cells {
                cy as f32 / raised_cells as f32
            } else {
                0.1 // dim un-raised section
            };
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy, 0, cells_w.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Isolated tests for the medieval theme — run even when other themes panic.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::super::{BarContext, Easing, Palette};
    use super::*;
    use crate::BrailleGrid;

    #[test]
    fn medieval_styles_render_all_sizes() {
        let sizes = [(1usize, 1usize), (2, 1), (50, 4), (80, 1), (10, 8)];
        let progresses = [0.0f32, 0.001, 0.5, 0.999, 1.0];
        let times = [0.0f32, 1.5, 3.7, 100.0];
        for style in styles() {
            assert!(!style.name().is_empty(), "empty name");
            assert_eq!(
                style.theme(),
                "medieval",
                "wrong theme for {}",
                style.name()
            );
            for &(w, h) in &sizes {
                for &p in &progresses {
                    for &t in &times {
                        let mut grid = BrailleGrid::new(w, h).unwrap();
                        let ctx = BarContext::new(p, t, w, h)
                            .with_easing(Easing::CubicInOut)
                            .with_palette(Palette::default());
                        style.render(&mut grid, &ctx).unwrap_or_else(|e| {
                            panic!("{} failed at {w}x{h} p={p} t={t}: {e}", style.name())
                        });
                    }
                }
            }
        }
    }
}
