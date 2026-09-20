//! Transit / Transportation progress bars — the **transit** theme.
//!
//! Twelve structurally distinct bars built around trains, planes, boats, and
//! mass-transit systems.  Every style uses a different spatial layout, temporal
//! mechanic, and geometric primitive — color never separates two styles here.
//!
//! # Style catalogue
//!
//! | name | mechanic |
//! |---|---|
//! | `steam-train`       | Locomotive + cars slide right; wheels rotate + smoke puffs rise |
//! | `airplane-takeoff`  | Plane taxis along a runway then climbs on a curved ascent |
//! | `sailboat-tacking`  | Hull + sail zigzag across the bar; heading flips at walls |
//! | `subway-map`        | Station dots along a route line; train marker advances between them |
//! | `hot-air-balloon`   | Envelope + basket rises; flame bursts pulse from basket |
//! | `ferris-wheel`      | Cabins orbit a rimmed wheel that spins driven by time |
//! | `cable-car`         | Gondola climbs a diagonal cable; support towers mark intervals |
//! | `escalator`         | Steps slide diagonally upward in a loop driven by time |
//! | `rocket-gantry`     | Countdown fills the frame; gantry arms retract then liftoff plume |
//! | `ferry-crossing`    | Ferry hull crosses with a V-shaped wake spreading behind |
//! | `bicycle`           | Two spinning wheels advance; pedal crank rotates between them |
//! | `helicopter`        | Rotor blades spin above a fuselage; body rises as progress grows |

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Bresenham line in dot-space, bounds-safe via `draw::dot_i`.
fn line(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let steps = dx.max(dy).max(1);
    for i in 0..=steps {
        let px = x0 + (x1 - x0) * i / steps;
        let py = y0 + (y1 - y0) * i / steps;
        draw::dot_i(grid, px, py);
    }
}

/// Draw a small filled circle (disk) in dot-space at (cx,cy) with radius r.
fn disk(grid: &mut BrailleGrid, cx: i32, cy: i32, r: i32) {
    let r = r.max(0);
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r + r {
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }
    }
}

/// Draw a circle outline at (cx,cy) with radius r.
fn circle(grid: &mut BrailleGrid, cx: i32, cy: i32, r: i32) {
    let r = r.max(1);
    let steps = (r * 7).max(12) as usize;
    let mut prev: Option<(i32, i32)> = None;
    for s in 0..=steps {
        let angle = s as f32 / steps as f32 * 2.0 * PI;
        let px = cx + (r as f32 * angle.cos()) as i32;
        let py = cy + (r as f32 * angle.sin()) as i32;
        draw::dot_i(grid, px, py);
        if let Some((ppx, ppy)) = prev {
            if (px - ppx).abs() + (py - ppy).abs() > 2 {
                line(grid, ppx, ppy, px, py);
            }
        }
        prev = Some((px, py));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Registry
// ─────────────────────────────────────────────────────────────────────────────

/// All styles in the `transit` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per transportation bar.  Every style is
/// geometrically distinct — they differ in spatial layout and temporal mechanic,
/// never merely in color.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(SteamTrain),
        Box::new(AirplaneTakeoff),
        Box::new(SailboatTacking),
        Box::new(SubwayMap),
        Box::new(HotAirBalloon),
        Box::new(FerrisWheel),
        Box::new(CableCar),
        Box::new(Escalator),
        Box::new(RocketGantry),
        Box::new(FerryCrossing),
        Box::new(Bicycle),
        Box::new(Helicopter),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Steam train
//    Locomotive + passenger cars advance rightward.  Progress = how far the
//    train has traveled across the bar.  Driving wheels rotate (circle arcs
//    spinning via time).  Smoke puffs rise from the stack: puffs grow and drift
//    upward over time.  Horizontal fill behind the train = distance covered.
// ─────────────────────────────────────────────────────────────────────────────

struct SteamTrain;
impl ProgressStyle for SteamTrain {
    fn name(&self) -> &str {
        "steam-train"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Steam locomotive chugs rightward: spinning wheels, rising smoke puffs, trailing passenger cars"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Ground line
        let ground_y = dh.saturating_sub(1);
        draw::hline(grid, 0, dw - 1, ground_y);

        // Wheel radius (scale with height)
        let wheel_r = ((dh as f32 * 0.18).max(2.0) as i32).max(2);
        let wheel_y = (ground_y as i32) - wheel_r;

        // Train head position: advances rightward with eased
        // Train width (locomotive + 2 cars) spans about 40% of the bar
        let train_span = ((dw as f32 * 0.40).max(8.0)) as i32;
        let loco_w = (train_span / 2).max(4);
        let car_w = (train_span / 4).max(3);

        // Lead edge of locomotive = eased * dw, clamped so loco doesn't exit right
        let head_x = ((ctx.eased * dw as f32) as i32)
            .min(dw as i32 - 1)
            .max(loco_w);

        // ── Locomotive body ──
        let loco_x0 = head_x - loco_w;
        let loco_top = (wheel_y - (dh as i32 / 3)).max(0);
        // Body rectangle
        for y in loco_top..wheel_y {
            draw::hline(
                grid,
                loco_x0.max(0) as usize,
                (head_x - 1).max(loco_x0).min(dw as i32 - 1) as usize,
                y as usize,
            );
        }
        // Cab (taller box at the rear half)
        let cab_x0 = loco_x0;
        let cab_top = (loco_top - dh as i32 / 5).max(0);
        for y in cab_top..loco_top {
            draw::hline(
                grid,
                cab_x0.max(0) as usize,
                (cab_x0 + loco_w / 2).min(dw as i32 - 1).max(0) as usize,
                y as usize,
            );
        }
        // Smokestack: a short vertical column at front top
        let stack_x = (head_x - loco_w / 6).max(0).min(dw as i32 - 1);
        let stack_top = loco_top - 1;
        draw::vline(
            grid,
            stack_x as usize,
            stack_top.max(0) as usize,
            loco_top as usize,
        );

        // ── Driving wheels (3 on the locomotive) ──
        let wheel_angle = ctx.time * 3.0 * PI;
        let loco_wheel_positions = [
            loco_x0 + loco_w / 5,
            loco_x0 + 2 * loco_w / 5,
            loco_x0 + 3 * loco_w / 5,
        ];
        for &wx in &loco_wheel_positions {
            if wx >= 0 && wx < dw as i32 {
                circle(grid, wx, wheel_y, wheel_r);
                // Spoke: a rotating line inside the wheel
                let spoke_ex = (wx as f32 + wheel_r as f32 * wheel_angle.cos()) as i32;
                let spoke_ey = (wheel_y as f32 + wheel_r as f32 * wheel_angle.sin()) as i32;
                line(grid, wx, wheel_y, spoke_ex, spoke_ey);
            }
        }

        // ── Passenger cars (2 cars trailing behind locomotive) ──
        let gap = (wheel_r).max(2);
        for car_idx in 0..2 {
            let car_x1 = loco_x0 - gap - car_idx as i32 * (car_w + gap);
            let car_x0 = car_x1 - car_w;
            if car_x1 < 0 {
                break;
            }
            let car_top = wheel_y - dh as i32 / 4;
            // Car body outline
            for y in car_top.max(0)..wheel_y {
                draw::hline(
                    grid,
                    car_x0.max(0) as usize,
                    car_x1.min(dw as i32 - 1).max(0) as usize,
                    y as usize,
                );
            }
            // Windows: two dots
            let win_y = (car_top + (wheel_y - car_top) / 3).max(0);
            draw::dot_i(grid, car_x0 + car_w / 3, win_y);
            draw::dot_i(grid, car_x0 + 2 * car_w / 3, win_y);
            // Car wheels (2 per car)
            for wi in [car_x0 + car_w / 4, car_x0 + 3 * car_w / 4] {
                if wi >= 0 && wi < dw as i32 {
                    circle(grid, wi, wheel_y, (wheel_r * 2 / 3).max(1));
                }
            }
        }

        // ── Smoke puffs above the stack ──
        let puff_count = 4usize;
        for p in 0..puff_count {
            let age = (ctx.time * 0.8 + p as f32 * 0.25).fract();
            let puff_x = stack_x + (age * 4.0) as i32;
            let puff_y = stack_top as i32 - (age * dh as f32 * 0.5) as i32;
            let puff_r = ((age * 3.0) as i32).max(1).min(wheel_r);
            if puff_y >= 0 {
                circle(grid, puff_x, puff_y, puff_r);
            }
        }

        // Track already drawn (ground_y).  Color: left of train = traveled.
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            draw::tint_row(grid, ch / 2, cx, cx, ctx.palette.sample(t));
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Airplane takeoff
//    Progress < 0.55: plane taxis along a runway (horizontal, on the ground).
//    Progress 0.55..1.0: plane lifts off and follows an ascending curve
//    (y = baseline - eased^2 * height).  The runway is the full-width baseline.
//    The plane is a simple swept silhouette (fuselage + wing + tail).
//    Dotted center-line markings scroll leftward as the plane moves.
// ─────────────────────────────────────────────────────────────────────────────

struct AirplaneTakeoff;
impl ProgressStyle for AirplaneTakeoff {
    fn name(&self) -> &str {
        "airplane-takeoff"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Plane taxis along a runway then climbs on an ascending curve; scrolling center-line markings"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let ground_y = dh.saturating_sub(2) as i32;

        // ── Runway ──
        draw::hline(grid, 0, dw - 1, ground_y as usize);
        // Dotted center line (scrolling left)
        let scroll = (ctx.time * 8.0) as usize % 8;
        let mut x = scroll;
        while x < dw {
            draw::dot(grid, x, ground_y as usize - (dh / 4).max(1));
            x += 8;
        }

        // ── Plane position ──
        let taxi_threshold = 0.55_f32;
        let plane_x = (ctx.eased * (dw as f32 - 4.0)).max(0.0) as i32;

        // Altitude: 0 while taxiing, then ramps up
        let altitude = if ctx.eased <= taxi_threshold {
            0.0_f32
        } else {
            let climb_frac = (ctx.eased - taxi_threshold) / (1.0 - taxi_threshold);
            climb_frac * climb_frac * (dh as f32 - 3.0)
        };
        let plane_y = (ground_y as f32 - altitude) as i32;

        // ── Plane silhouette ──
        // Fuselage: horizontal line of 6–10 dots
        let fuselage_len = ((dw as f32 * 0.12).max(6.0) as i32).max(6);
        let nose_x = plane_x;
        let tail_x = (plane_x - fuselage_len).max(0);
        draw::hline(
            grid,
            tail_x as usize,
            nose_x.min(dw as i32 - 1).max(0) as usize,
            plane_y.max(0).min(dh as i32 - 1) as usize,
        );

        // Wing: diagonal sweep downward from mid-fuselage
        let wing_root_x = plane_x - fuselage_len / 3;
        let wing_span = fuselage_len / 2;
        let wing_sweep = (dh as i32 / 3).max(1);
        if altitude < dh as f32 * 0.3 {
            // On ground / low altitude: wing is nearly horizontal (slight dihedral)
            line(
                grid,
                wing_root_x,
                plane_y,
                wing_root_x - wing_span,
                (plane_y + 1).min(dh as i32 - 1),
            );
        } else {
            // Climbing: wings sweep back more aggressively
            line(
                grid,
                wing_root_x,
                plane_y,
                wing_root_x - wing_span,
                (plane_y + wing_sweep / 2).min(dh as i32 - 1),
            );
        }

        // Tail fin: short vertical at the rear
        let tail_top_y = (plane_y - dh as i32 / 5).max(0);
        draw::vline(
            grid,
            tail_x.max(0) as usize,
            tail_top_y as usize,
            plane_y.max(0).min(dh as i32 - 1) as usize,
        );

        // Jet exhaust: a few trailing dots behind the tail when airborne
        if altitude > 2.0 {
            for ex in 1..=3 {
                let ex_x = tail_x - ex;
                if ex_x >= 0 {
                    draw::dot_i(grid, ex_x, plane_y);
                }
            }
        }

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Sailboat tacking
//    A sailboat (hull + triangular sail) crosses the bar left→right.
//    It zigzags vertically — tacking upwind — so the heading reverses each
//    time the boat hits the top or bottom boundary.
//    The hull is a short horizontal wedge; the sail is a right triangle.
//    Waves are simple sine undulations along the bottom.
// ─────────────────────────────────────────────────────────────────────────────

struct SailboatTacking;
impl ProgressStyle for SailboatTacking {
    fn name(&self) -> &str {
        "sailboat-tacking"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Sailboat hull and sail zigzag across the bar tacking upwind; sine-wave water below"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // ── Water (bottom quarter) ──
        let water_top = dh * 3 / 4;
        for x in 0..dw {
            let wave = (PI * x as f32 / (dw as f32 * 0.2) + ctx.time * 2.0).sin();
            let wy = (water_top as f32 + wave * (dh as f32 * 0.04)) as usize;
            draw::dot(grid, x, wy.min(dh - 1));
        }

        // ── Boat position (horizontal = eased * dw) ──
        let boat_x = ((ctx.eased * dw as f32) as i32).min(dw as i32 - 1).max(2);

        // Tacking: vertical position oscillates with a zigzag wave
        // The number of tacks grows with progress.
        let tack_freq = 1.5_f32; // tacks per full crossing
        let tack_phase = ctx.eased * tack_freq * 2.0 * PI;
        let tack_range = (water_top as f32 * 0.75).max(1.0);
        let boat_y_center = water_top as f32 / 2.0;
        let boat_y = (boat_y_center + tack_range * tack_phase.sin() * 0.5) as i32;
        let clamp_lo = 0_i32;
        let clamp_hi = (water_top as i32).max(0);
        let boat_y = boat_y.clamp(clamp_lo, clamp_hi);

        // ── Hull (a flat wedge: wider at stern, narrows to point at bow) ──
        let hull_len = ((dw as f32 * 0.10).max(5.0)) as i32;
        // Bow (front = right), stern (back = left)
        draw::dot_i(grid, boat_x, boat_y); // bow point
        for i in 1..hull_len {
            let half_w = (i as f32 / hull_len as f32 * 2.0) as i32;
            for dy in -half_w..=half_w {
                draw::dot_i(grid, boat_x - i, boat_y + dy);
            }
        }

        // ── Sail (right triangle: mast=vertical, boom=horizontal, hyp=diagonal) ──
        let mast_h = ((dh as f32 * 0.45).max(3.0)) as i32;
        let sail_base = hull_len / 2;
        let mast_x = boat_x - sail_base;
        // Mast (vertical)
        draw::vline(
            grid,
            mast_x.max(0) as usize,
            (boat_y - mast_h).max(0) as usize,
            (boat_y - 1).max(0) as usize,
        );
        // Boom (horizontal, at hull level)
        draw::hline(
            grid,
            mast_x.max(0) as usize,
            (boat_x - 1).max(0).min(dw as i32 - 1) as usize,
            boat_y as usize,
        );
        // Hypotenuse (from mast top to bow)
        line(grid, mast_x, (boat_y - mast_h).max(0), boat_x - 1, boat_y);

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Subway map
//    A horizontal line represents a transit route.  Station dots are placed at
//    equal intervals.  A "train" marker (filled disk) advances between stations
//    as eased grows.  Stations behind the train are shown as filled; ahead as
//    hollow circles.  The line itself is drawn as a thick rail.
// ─────────────────────────────────────────────────────────────────────────────

struct SubwayMap;
impl ProgressStyle for SubwayMap {
    fn name(&self) -> &str {
        "subway-map"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Subway route line: station dots fill as the train marker advances between stops"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let rail_y = dh / 2;

        // ── Rail (double line for visual weight) ──
        draw::hline(grid, 0, dw - 1, rail_y);
        if rail_y + 1 < dh {
            draw::hline(grid, 0, dw - 1, rail_y + 1);
        }

        // ── Stations ──
        let n_stations = ((dw / 8).max(3)).min(16);
        let station_r = (dh as i32 / 6).max(1);

        for s in 0..n_stations {
            let sx = (s * dw) / (n_stations - 1).max(1);
            let t_station = sx as f32 / dw as f32;
            if t_station <= ctx.eased {
                // Visited: filled disk
                disk(grid, sx as i32, rail_y as i32, station_r);
            } else {
                // Ahead: hollow circle
                circle(grid, sx as i32, rail_y as i32, station_r);
            }
        }

        // ── Train marker ──
        let train_x = (ctx.eased * dw as f32) as i32;
        let train_r = (station_r * 2).max(2);
        disk(grid, train_x, rail_y as i32, train_r);
        // Halo / outline for distinction
        circle(grid, train_x, rail_y as i32, train_r + 1);

        // Station labels: small tick marks above each station
        for s in 0..n_stations {
            let sx = (s * dw) / (n_stations - 1).max(1);
            let tick_top = rail_y.saturating_sub(station_r as usize + 2);
            let tick_bot = rail_y.saturating_sub(station_r as usize + 1);
            draw::vline(grid, sx.min(dw - 1), tick_top, tick_bot);
        }

        // Color: cells behind the train
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Hot-air balloon
//    Progress = how high the balloon has risen (bottom → top).
//    The envelope is an oval (wide top, narrowing to the neck).
//    The basket hangs below on two cords.
//    Flame bursts: tall vertical spike from the basket mouth, pulsing with time.
//    The balloon's horizontal position is centered; vertical position rises.
// ─────────────────────────────────────────────────────────────────────────────

struct HotAirBalloon;
impl ProgressStyle for HotAirBalloon {
    fn name(&self) -> &str {
        "hot-air-balloon"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Hot-air balloon rises from bottom to top; envelope oval, basket, pulsing flame bursts"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cx = (dw / 2) as i32;

        // Balloon rises: at eased=0 it sits at the bottom; at 1 it's at the top.
        let env_ry = ((dh as f32 * 0.30).max(2.0)) as i32;
        let env_rx = ((dw as f32 * 0.28).max(2.0)) as i32;
        let basket_h = ((dh as f32 * 0.10).max(1.0)) as i32;
        let basket_w = ((dw as f32 * 0.10).max(2.0)) as i32;
        let cord_len = ((dh as f32 * 0.05).max(1.0)) as i32;
        let total_h = env_ry * 2 + cord_len + basket_h;

        // Center y: starts at bottom-total_h/2, ends at top+total_h/2
        let bottom_cy = (dh as i32) - env_ry - 2;
        let top_cy = env_ry + 2;
        let balloon_cy = (bottom_cy as f32 + (top_cy - bottom_cy) as f32 * ctx.eased) as i32;
        let cy_lo = 0_i32;
        let cy_hi = (dh as i32 - 1).max(0);
        let balloon_cy = balloon_cy.clamp(cy_lo, cy_hi);

        // ── Envelope outline (ellipse) ──
        let env_steps = ((env_rx + env_ry) * 4).max(24) as usize;
        let mut prev: Option<(i32, i32)> = None;
        for s in 0..=env_steps {
            let angle = s as f32 / env_steps as f32 * 2.0 * PI;
            let px = cx + (env_rx as f32 * angle.cos()) as i32;
            let py = balloon_cy + (env_ry as f32 * angle.sin()) as i32;
            draw::dot_i(grid, px, py);
            if let Some((ppx, ppy)) = prev {
                if (px - ppx).abs() + (py - ppy).abs() > 2 {
                    line(grid, ppx, ppy, px, py);
                }
            }
            prev = Some((px, py));
        }
        // Vertical stripes on the envelope (2 stripes)
        for stripe_off in [-env_rx / 3, env_rx / 3] {
            draw::vline(
                grid,
                (cx + stripe_off).max(0) as usize,
                (balloon_cy - env_ry + 1).max(0) as usize,
                (balloon_cy + env_ry - 1).min(dh as i32 - 1).max(0) as usize,
            );
        }

        // ── Neck (bottom of envelope narrowing) ──
        let neck_y = balloon_cy + env_ry;
        let neck_w = basket_w / 2;
        draw::hline(
            grid,
            (cx - neck_w).max(0) as usize,
            (cx + neck_w).min(dw as i32 - 1).max(0) as usize,
            neck_y.max(0).min(dh as i32 - 1) as usize,
        );

        // ── Cords from neck to basket ──
        let basket_top_y = neck_y + cord_len;
        line(grid, cx - neck_w, neck_y, cx - basket_w, basket_top_y);
        line(grid, cx + neck_w, neck_y, cx + basket_w, basket_top_y);

        // ── Basket ──
        let basket_bot_y = basket_top_y + basket_h;
        for y in basket_top_y..=basket_bot_y.min(dh as i32 - 1) {
            draw::hline(
                grid,
                (cx - basket_w).max(0) as usize,
                (cx + basket_w).min(dw as i32 - 1).max(0) as usize,
                y.max(0) as usize,
            );
        }

        // ── Flame burst (from basket mouth = balloon_cy + env_ry - 1, going up) ──
        let flame_pulse = (ctx.time * 6.0).sin() * 0.5 + 0.5; // 0..1
        let flame_h = ((env_ry as f32 * 0.6 * flame_pulse) as i32).max(1);
        for fy in 0..flame_h {
            let fw = ((1.0 - fy as f32 / flame_h as f32) * neck_w as f32) as i32;
            let flame_y = neck_y - 1 - fy;
            if flame_y >= 0 {
                draw::hline(
                    grid,
                    (cx - fw).max(0) as usize,
                    (cx + fw).min(dw as i32 - 1).max(0) as usize,
                    flame_y as usize,
                );
            }
        }

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx_cell in 0..filled.min(cw) {
            let t = cx_cell as f32 / cw.max(1) as f32;
            for cy_cell in 0..ch {
                draw::tint_row(grid, cy_cell, cx_cell, cx_cell, ctx.palette.sample(t));
            }
        }

        let _ = total_h;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Ferris wheel
//    A large circle (rim) with 8 evenly-spaced cabins (small circles) orbiting
//    it.  The whole wheel rotates driven by time.  Two support struts anchor the
//    axle to the ground.  Progress controls how far the fill line covers the
//    lower half of the wheel (boarding/loading metaphor).
// ─────────────────────────────────────────────────────────────────────────────

struct FerrisWheel;
impl ProgressStyle for FerrisWheel {
    fn name(&self) -> &str {
        "ferris-wheel"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Ferris wheel spins continuously; 8 cabins orbit the rim; support struts anchor to ground"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cx = (dw / 2) as i32;
        let ground_y = dh.saturating_sub(2) as i32;

        // Wheel fits vertically
        let wheel_r = ((dh as f32 * 0.38).max(3.0) as i32).max(3);
        let axle_y = (ground_y - wheel_r).max(wheel_r);

        // ── Rim ──
        circle(grid, cx, axle_y, wheel_r);

        // ── Spokes (8 spokes rotating with time) ──
        let n_spokes = 8usize;
        for sp in 0..n_spokes {
            let angle = sp as f32 / n_spokes as f32 * 2.0 * PI + ctx.time * 0.8;
            let sx = (cx as f32 + wheel_r as f32 * angle.cos()) as i32;
            let sy = (axle_y as f32 + wheel_r as f32 * angle.sin()) as i32;
            line(grid, cx, axle_y, sx, sy);
        }

        // ── Hub (axle disk) ──
        disk(grid, cx, axle_y, (wheel_r / 6).max(1));

        // ── Cabins (small dots at spoke tips) ──
        let n_cabins = 8usize;
        let cabin_r = (wheel_r / 5).max(1);
        for c in 0..n_cabins {
            let angle = c as f32 / n_cabins as f32 * 2.0 * PI + ctx.time * 0.8;
            let cabin_cx = (cx as f32 + wheel_r as f32 * angle.cos()) as i32;
            let cabin_cy = (axle_y as f32 + wheel_r as f32 * angle.sin()) as i32;
            // Cabin hangs below its pivot point (gravity-aligned)
            let hang = cabin_r;
            disk(grid, cabin_cx, cabin_cy + hang, cabin_r);
        }

        // ── Support struts ──
        let strut_spread = (wheel_r as f32 * 0.7) as i32;
        line(grid, cx, axle_y, cx - strut_spread, ground_y);
        line(grid, cx, axle_y, cx + strut_spread, ground_y);
        draw::hline(
            grid,
            (cx - strut_spread).max(0) as usize,
            (cx + strut_spread).min(dw as i32 - 1) as usize,
            ground_y as usize,
        );

        // ── Progress indicator: arc of filled cabins ──
        let n_filled = (ctx.eased * n_cabins as f32).round() as usize;
        for c in 0..n_filled.min(n_cabins) {
            let angle = c as f32 / n_cabins as f32 * 2.0 * PI + ctx.time * 0.8;
            let cabin_cx = (cx as f32 + (wheel_r as f32 * 1.05) * angle.cos()) as i32;
            let cabin_cy = (axle_y as f32 + (wheel_r as f32 * 1.05) * angle.sin()) as i32;
            draw::dot_i(grid, cabin_cx, cabin_cy);
            draw::dot_i(grid, cabin_cx + 1, cabin_cy);
        }

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx_cell in 0..filled.min(cw) {
            let t = cx_cell as f32 / cw.max(1) as f32;
            for cy_cell in 0..ch {
                draw::tint_row(grid, cy_cell, cx_cell, cx_cell, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Cable car / gondola
//    A diagonal cable runs from bottom-left to top-right.  A gondola box
//    (rectangle with a wheel hanger above) slides along the cable as eased
//    grows.  Support towers divide the cable into spans.
// ─────────────────────────────────────────────────────────────────────────────

struct CableCar;
impl ProgressStyle for CableCar {
    fn name(&self) -> &str {
        "cable-car"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Gondola climbs a diagonal cable from valley to peak; support towers mark intervals"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cable_x0 = 0_i32;
        let cable_y0 = (dh as i32) - 1;
        let cable_x1 = (dw as i32) - 1;
        let cable_y1 = 0_i32;

        // ── Cable ──
        line(grid, cable_x0, cable_y0, cable_x1, cable_y1);

        // ── Support towers (3 towers at 25%, 50%, 75%) ──
        let n_towers = 3usize;
        for t in 1..=n_towers {
            let frac = t as f32 / (n_towers + 1) as f32;
            let tx = (cable_x0 as f32 + (cable_x1 - cable_x0) as f32 * frac) as i32;
            let ty = (cable_y0 as f32 + (cable_y1 - cable_y0) as f32 * frac) as i32;
            // Tower = vertical line from cable attachment to ground
            draw::vline(
                grid,
                tx.max(0) as usize,
                ty.max(0) as usize,
                (cable_y0).min(dh as i32 - 1) as usize,
            );
            // Tower cap
            draw::dot_i(grid, tx - 1, ty);
            draw::dot_i(grid, tx + 1, ty);
        }

        // ── Gondola position along cable ──
        let g_frac = ctx.eased;
        let g_x = (cable_x0 as f32 + (cable_x1 - cable_x0) as f32 * g_frac) as i32;
        let g_y_cab = (cable_y0 as f32 + (cable_y1 - cable_y0) as f32 * g_frac) as i32;

        let car_w = ((dw as f32 * 0.07).max(3.0)) as i32;
        let car_h = ((dh as f32 * 0.20).max(2.0)) as i32;
        let hanger = 2_i32;

        // Wheel hanger: connects gondola to cable
        draw::vline(
            grid,
            g_x.max(0) as usize,
            g_y_cab.max(0) as usize,
            (g_y_cab + hanger).min(dh as i32 - 1).max(0) as usize,
        );
        draw::dot_i(grid, g_x - 1, g_y_cab);
        draw::dot_i(grid, g_x + 1, g_y_cab);

        // Gondola body (rectangle)
        let body_top = g_y_cab + hanger;
        let body_bot = (body_top + car_h).min(dh as i32 - 1);
        for y in body_top..=body_bot {
            draw::hline(
                grid,
                (g_x - car_w).max(0) as usize,
                (g_x + car_w).min(dw as i32 - 1).max(0) as usize,
                y.max(0) as usize,
            );
        }

        // Window
        let win_y = (body_top + car_h / 3).min(body_bot);
        draw::dot_i(grid, g_x, win_y);

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Escalator
//    Diagonal steps tile the frame and scroll upward continuously, driven purely
//    by time.  Progress determines how wide the escalator band is (how many
//    steps fill from left to right = eased * width).  Each step is a small
//    right-triangle: horizontal tread + vertical riser.
// ─────────────────────────────────────────────────────────────────────────────

struct Escalator;
impl ProgressStyle for Escalator {
    fn name(&self) -> &str {
        "escalator"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Escalator steps scroll upward diagonally via time; progress widens the moving band"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Step size: tread width and riser height in dots
        let tread = ((dw as f32 / 8.0).max(3.0) as usize).max(3);
        let riser = ((dh as f32 / 4.0).max(2.0) as usize).max(2);

        // Scroll offset: steps move up (decreasing y) over time
        let scroll = ((ctx.time * riser as f32 * 1.5) as usize) % (riser + 1).max(1);

        // Active escalator width = eased fraction of dw
        let active_w = ((ctx.eased * dw as f32).round() as usize).min(dw);

        // Handrails: two diagonal lines bounding the escalator
        let hr_left_x0 = 0_i32;
        let hr_left_y0 = (dh as i32) - 1;
        let hr_left_x1 = active_w as i32;
        let hr_left_y1 = 0_i32;
        if active_w > 0 {
            line(grid, hr_left_x0, hr_left_y0, hr_left_x1, hr_left_y1);
        }

        // Draw step grid
        let total_steps_x = dw / tread + 2;
        let total_steps_y = dh / riser + 2;
        for row in 0..total_steps_y {
            for col in 0..total_steps_x {
                // Position of this step's top-left corner, with scroll
                let x = col * tread;
                // Each row is diagonally offset by col so steps nest on the staircase
                let base_y = (dh + riser).saturating_sub((row + 1) * riser);
                let y_raw = base_y as i32 + scroll as i32 - col as i32 * riser as i32 / 2;

                if x >= active_w {
                    continue;
                } // only draw within active band

                // Tread (horizontal)
                let tread_x1 = (x + tread).min(active_w) - 1;
                if y_raw >= 0 && (y_raw as usize) < dh {
                    draw::hline(grid, x.min(dw - 1), tread_x1.min(dw - 1), y_raw as usize);
                }
                // Riser (vertical)
                let riser_y0 = y_raw;
                let riser_y1 = y_raw + riser as i32;
                if x < dw {
                    for ry in riser_y0..=riser_y1 {
                        draw::dot_i(grid, x as i32, ry);
                    }
                }
            }
        }

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Rocket gantry launch
//    Focus: the launch pad, gantry structure, and countdown.
//    progress < 0.80: gantry arms extend inward from the sides, holding the rocket.
//    progress 0.80..1.0: arms retract; exhaust plume grows from the bottom.
//    The rocket is a tall narrow rectangle (body) + nose cone (triangle) + fins.
//    Liftoff smoke billows outward at the base via a widening half-disk.
// ─────────────────────────────────────────────────────────────────────────────

struct RocketGantry;
impl ProgressStyle for RocketGantry {
    fn name(&self) -> &str {
        "rocket-gantry"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Launch pad countdown: gantry arms hold then retract; liftoff plume erupts at the base"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cx = (dw / 2) as i32;
        let ground_y = (dh as i32) - 1;
        let rocket_w = ((dw as f32 * 0.08).max(3.0)) as i32;
        let rocket_h = ((dh as f32 * 0.55).max(4.0)) as i32;
        let nose_h = ((rocket_h as f32 * 0.25).max(2.0)) as i32;
        let fin_w = ((rocket_w as f32 * 0.8).max(2.0)) as i32;
        let fin_h = ((rocket_h as f32 * 0.15).max(2.0)) as i32;

        // Liftoff threshold
        let liftoff_t = 0.80_f32;
        // How far the rocket has risen above the pad (0 until liftoff, then grows)
        let rise = if ctx.eased < liftoff_t {
            0_i32
        } else {
            let frac = (ctx.eased - liftoff_t) / (1.0 - liftoff_t);
            (frac * frac * dh as f32 * 0.6) as i32
        };

        let rocket_base_y = ground_y - rise;
        let rocket_top_y = (rocket_base_y - rocket_h).max(0);

        // ── Rocket body ──
        for y in rocket_top_y..=rocket_base_y.min(ground_y) {
            draw::hline(
                grid,
                (cx - rocket_w).max(0) as usize,
                (cx + rocket_w).min(dw as i32 - 1).max(0) as usize,
                y.max(0).min(dh as i32 - 1) as usize,
            );
        }

        // ── Nose cone (triangle pointing up) ──
        for ny in 0..nose_h {
            let half_w = ((1.0 - ny as f32 / nose_h as f32) * rocket_w as f32) as i32;
            let y = rocket_top_y - nose_h + ny;
            if y >= 0 {
                draw::hline(
                    grid,
                    (cx - half_w).max(0) as usize,
                    (cx + half_w).min(dw as i32 - 1).max(0) as usize,
                    y as usize,
                );
            }
        }

        // ── Fins (at the base) ──
        let fin_y0 = rocket_base_y - fin_h;
        for fy in 0..fin_h {
            let flare = (fy as f32 / fin_h as f32 * fin_w as f32) as i32;
            let y = fin_y0 + fy;
            if y >= 0 && y <= ground_y {
                draw::dot_i(grid, cx - rocket_w - flare, y);
                draw::dot_i(grid, cx + rocket_w + flare, y);
            }
        }

        // ── Gantry structure ──
        let gantry_tower_x_l = cx - dw as i32 / 4;
        let gantry_tower_x_r = cx + dw as i32 / 4;
        // Towers (full height vertical bars)
        draw::vline(grid, gantry_tower_x_l.max(0) as usize, 0, ground_y as usize);
        draw::vline(
            grid,
            gantry_tower_x_r.max(0).min(dw as i32 - 1) as usize,
            0,
            ground_y as usize,
        );

        // Gantry arms: horizontal bars at 3 heights, retracting as we approach liftoff
        let arm_retract = if ctx.eased >= liftoff_t {
            let frac = (ctx.eased - liftoff_t) / (1.0 - liftoff_t);
            (frac * (gantry_tower_x_r - cx - rocket_w) as f32) as i32
        } else {
            0_i32
        };

        for arm_frac in [0.25_f32, 0.50, 0.75] {
            let arm_y = (ground_y as f32 * arm_frac) as i32;
            let arm_inner_l = cx - rocket_w - 1 + arm_retract;
            let arm_inner_r = cx + rocket_w + 1 - arm_retract;
            // Left arm: from left tower to rocket
            if arm_inner_l > gantry_tower_x_l {
                draw::hline(
                    grid,
                    gantry_tower_x_l.max(0) as usize,
                    arm_inner_l.max(0).min(dw as i32 - 1) as usize,
                    arm_y.max(0).min(dh as i32 - 1) as usize,
                );
            }
            // Right arm: from rocket to right tower
            if arm_inner_r < gantry_tower_x_r {
                draw::hline(
                    grid,
                    arm_inner_r.max(0) as usize,
                    gantry_tower_x_r.min(dw as i32 - 1).max(0) as usize,
                    arm_y.max(0).min(dh as i32 - 1) as usize,
                );
            }
        }

        // ── Launch plume (after liftoff) ──
        if ctx.eased >= liftoff_t {
            let plume_frac = (ctx.eased - liftoff_t) / (1.0 - liftoff_t);
            let plume_h = (plume_frac * dh as f32 * 0.5) as i32;
            let pulse = (ctx.time * 10.0).sin() * 0.3 + 0.7;
            for py in 0..plume_h {
                let spread =
                    ((py as f32 / plume_h.max(1) as f32) * rocket_w as f32 * 2.0 * pulse) as i32;
                let y = ground_y + py;
                if y < dh as i32 {
                    draw::hline(
                        grid,
                        (cx - rocket_w - spread).max(0) as usize,
                        (cx + rocket_w + spread).min(dw as i32 - 1).max(0) as usize,
                        y as usize,
                    );
                }
            }
        }

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx_cell in 0..filled.min(cw) {
            let t = cx_cell as f32 / cw.max(1) as f32;
            for cy_cell in 0..ch {
                draw::tint_row(grid, cy_cell, cx_cell, cx_cell, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Ferry crossing with wake
//     Ferry is a wide flat hull (a thick rectangle).  Progress = crossing fraction.
//     Behind the ferry, two diverging wake lines spread at ±angle.
//     The water surface is a horizontal baseline.  Wave crests dot the surface.
// ─────────────────────────────────────────────────────────────────────────────

struct FerryCrossing;
impl ProgressStyle for FerryCrossing {
    fn name(&self) -> &str {
        "ferry-crossing"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Ferry hull crosses the bar; a V-shaped wake fans out behind; wave crests dot the surface"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let water_y = dh / 2;

        // ── Water surface ──
        for x in 0..dw {
            let wave = (PI * x as f32 / (dw as f32 * 0.15) + ctx.time * 3.0).sin();
            let wy = (water_y as f32 + wave * (dh as f32 * 0.04)) as usize;
            draw::dot(grid, x, wy.min(dh - 1));
        }

        // ── Ferry position ──
        let ferry_w = ((dw as f32 * 0.18).max(6.0)) as i32;
        let ferry_h = ((dh as f32 * 0.20).max(2.0)) as i32;
        let bow_x = ((ctx.eased * dw as f32) as i32)
            .min(dw as i32 - 1)
            .max(ferry_w);
        let stern_x = bow_x - ferry_w;
        let hull_top = (water_y as i32) - ferry_h;

        // Hull: filled rectangle
        for y in hull_top.max(0)..=water_y as i32 {
            draw::hline(
                grid,
                stern_x.max(0) as usize,
                bow_x.min(dw as i32 - 1).max(0) as usize,
                y as usize,
            );
        }

        // Superstructure (box on top of hull)
        let super_w = ferry_w / 3;
        let super_h = (ferry_h / 2).max(1);
        let super_x0 = stern_x + ferry_w / 3;
        for y in (hull_top - super_h).max(0)..hull_top.max(0) {
            draw::hline(
                grid,
                super_x0.max(0) as usize,
                (super_x0 + super_w).min(dw as i32 - 1).max(0) as usize,
                y as usize,
            );
        }

        // Funnel / smokestack
        let funnel_x = super_x0 + super_w / 2;
        let funnel_top = (hull_top - super_h - ferry_h / 3).max(0);
        draw::vline(
            grid,
            funnel_x.max(0) as usize,
            funnel_top as usize,
            (hull_top - super_h).max(funnel_top).max(0) as usize,
        );

        // ── Wake: two lines diverging from stern ──
        let wake_len = ((ctx.eased * dw as f32) as i32).min(dw as i32);
        let wake_angle_dots = (dh as f32 * 0.25).max(1.0) as i32;
        let wake_y0 = water_y as i32;
        // Upper wake line
        line(
            grid,
            stern_x.max(0),
            wake_y0,
            (stern_x - wake_len).max(0),
            (wake_y0 - wake_angle_dots).max(0),
        );
        // Lower wake line
        line(
            grid,
            stern_x.max(0),
            wake_y0,
            (stern_x - wake_len).max(0),
            (wake_y0 + wake_angle_dots).min(dh as i32 - 1),
        );
        // Ripple dots along wake
        for w in (0..wake_len as usize).step_by(4.max(1)) {
            let phase = w as f32 / wake_len as f32;
            let spread = (phase * wake_angle_dots as f32) as i32;
            let wx = stern_x - w as i32;
            if wx >= 0 {
                draw::dot_i(grid, wx, wake_y0 - spread / 2);
                draw::dot_i(grid, wx, wake_y0 + spread / 2);
            }
        }

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 11. Bicycle
//     Two spoked wheels advance rightward.  A pedal crank between them rotates
//     with time.  Chain stays and seat tube connect the wheels.  Progress = how
//     far the bicycle has traveled.  Wheel rotation rate = time * constant.
// ─────────────────────────────────────────────────────────────────────────────

struct Bicycle;
impl ProgressStyle for Bicycle {
    fn name(&self) -> &str {
        "bicycle"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Two spoked wheels advance rightward; crank rotates between them; frame connects front to rear"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let ground_y = (dh as i32) - 1;
        let wheel_r = ((dh as f32 * 0.30).max(3.0) as i32).max(3);
        let axle_y = ground_y - wheel_r;

        // Wheel separation
        let wb = ((dw as f32 * 0.28).max(wheel_r as f32 * 2.0 + 2.0)) as i32;

        // Bicycle center x: follows eased
        let cx_lo = wb / 2;
        let cx_hi = (dw as i32 - wb / 2 - 1).max(cx_lo);
        let center_x =
            ((ctx.eased * (dw as f32 - wb as f32 / 2.0)) as i32 + wb / 2).clamp(cx_lo, cx_hi);
        let rear_x = center_x - wb / 2;
        let front_x = center_x + wb / 2;

        // Wheel spin angle
        let spin = ctx.time * 4.0;

        // ── Rear wheel ──
        circle(grid, rear_x, axle_y, wheel_r);
        let n_spokes = 6usize;
        for s in 0..n_spokes {
            let a = s as f32 / n_spokes as f32 * 2.0 * PI + spin;
            let sx = (rear_x as f32 + wheel_r as f32 * a.cos()) as i32;
            let sy = (axle_y as f32 + wheel_r as f32 * a.sin()) as i32;
            line(grid, rear_x, axle_y, sx, sy);
        }

        // ── Front wheel ──
        circle(grid, front_x, axle_y, wheel_r);
        for s in 0..n_spokes {
            let a = s as f32 / n_spokes as f32 * 2.0 * PI + spin;
            let sx = (front_x as f32 + wheel_r as f32 * a.cos()) as i32;
            let sy = (axle_y as f32 + wheel_r as f32 * a.sin()) as i32;
            line(grid, front_x, axle_y, sx, sy);
        }

        // ── Frame ──
        // Seat tube: rear axle up to saddle
        let saddle_y = axle_y - wheel_r * 2 / 3;
        let saddle_x = rear_x + wb / 6;
        draw::vline(
            grid,
            rear_x.max(0) as usize,
            axle_y.max(0) as usize,
            saddle_y.max(0).min(dh as i32 - 1) as usize,
        );
        // Saddle (horizontal)
        draw::hline(
            grid,
            (saddle_x - wheel_r / 3).max(0) as usize,
            (saddle_x + wheel_r / 3).min(dw as i32 - 1) as usize,
            saddle_y.max(0) as usize,
        );
        // Top tube: saddle to stem
        let stem_x = front_x - wb / 8;
        let stem_y = axle_y - wheel_r * 2 / 3;
        line(grid, saddle_x, saddle_y, stem_x, stem_y);
        // Down tube: saddle bottom to bottom bracket
        let bb_x = center_x;
        let bb_y = axle_y;
        line(grid, saddle_x, saddle_y, bb_x, bb_y);
        // Chain stay: bottom bracket to rear axle
        line(grid, bb_x, bb_y, rear_x, axle_y);
        // Fork: stem down to front axle
        line(grid, stem_x, stem_y, front_x, axle_y);
        // Handlebar (short horizontal above stem)
        let hb_y = stem_y - 1;
        draw::hline(
            grid,
            (stem_x - wheel_r / 4).max(0) as usize,
            (stem_x + wheel_r / 4).min(dw as i32 - 1) as usize,
            hb_y.max(0) as usize,
        );

        // ── Crank (at bottom bracket) ──
        let crank_len = (wheel_r / 2).max(2);
        let crank_a = spin * 1.5;
        let crank_ex = (bb_x as f32 + crank_len as f32 * crank_a.cos()) as i32;
        let crank_ey = (bb_y as f32 + crank_len as f32 * crank_a.sin()) as i32;
        line(grid, bb_x, bb_y, crank_ex, crank_ey);
        // Second crank arm (180° offset)
        let crank_bx = (bb_x as f32 - crank_len as f32 * crank_a.cos()) as i32;
        let crank_by = (bb_y as f32 - crank_len as f32 * crank_a.sin()) as i32;
        line(grid, bb_x, bb_y, crank_bx, crank_by);
        // Pedals
        draw::dot_i(grid, crank_ex - 1, crank_ey);
        draw::dot_i(grid, crank_ex, crank_ey);
        draw::dot_i(grid, crank_ex + 1, crank_ey);
        draw::dot_i(grid, crank_bx - 1, crank_by);
        draw::dot_i(grid, crank_bx, crank_by);
        draw::dot_i(grid, crank_bx + 1, crank_by);

        // ── Ground ──
        draw::hline(grid, 0, dw - 1, ground_y as usize);

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 12. Helicopter
//     A fuselage (horizontal elongated body) with a tail boom extending right.
//     Above the body: the main rotor — two blades that spin rapidly with time.
//     At the tail tip: the tail rotor (small spinning disk).
//     The whole helicopter rises as progress grows (y tracks eased).
// ─────────────────────────────────────────────────────────────────────────────

struct Helicopter;
impl ProgressStyle for Helicopter {
    fn name(&self) -> &str {
        "helicopter"
    }
    fn theme(&self) -> &str {
        "transit"
    }
    fn describe(&self) -> &str {
        "Helicopter fuselage rises as progress grows; main rotor blades spin; tail rotor whirls"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // ── Helicopter vertical position (rises with eased) ──
        let min_y = (dh as i32 / 4).max(2); // highest point (top quarter)
        let max_y = (dh as i32 * 3 / 4).max(min_y + 1); // lowest point (bottom quarter)
        let body_y = (max_y as f32 - (max_y - min_y) as f32 * ctx.eased) as i32;

        // Helicopter is centered horizontally
        let cx = (dw / 2) as i32;

        let body_w = ((dw as f32 * 0.25).max(6.0)) as i32;
        let body_h = ((dh as f32 * 0.12).max(2.0)) as i32;
        let tail_len = ((dw as f32 * 0.25).max(4.0)) as i32;

        // ── Fuselage (filled rectangle) ──
        let body_x0 = cx - body_w / 2;
        let body_x1 = cx + body_w / 2;
        for y in body_y..=(body_y + body_h).min(dh as i32 - 1) {
            draw::hline(
                grid,
                body_x0.max(0) as usize,
                body_x1.min(dw as i32 - 1).max(0) as usize,
                y.max(0) as usize,
            );
        }

        // ── Cockpit dome (small half-disk on the front / left) ──
        let dome_cx = body_x0 + body_w / 5;
        let dome_r = (body_h * 2 / 3).max(1);
        let dome_top = body_y - dome_r;
        for dy in 0..dome_r {
            let half_w = ((1.0 - dy as f32 / dome_r as f32) * dome_r as f32) as i32;
            let y = dome_top + dy;
            if y >= 0 {
                draw::hline(
                    grid,
                    (dome_cx - half_w).max(0) as usize,
                    (dome_cx + half_w).min(dw as i32 - 1).max(0) as usize,
                    y as usize,
                );
            }
        }

        // ── Tail boom (thin horizontal line extending right from fuselage) ──
        let boom_y = body_y + body_h / 2;
        let tail_tip = body_x1 + tail_len;
        draw::hline(
            grid,
            body_x1.max(0) as usize,
            tail_tip.min(dw as i32 - 1).max(0) as usize,
            boom_y.max(0).min(dh as i32 - 1) as usize,
        );
        // Tail fin (short vertical at the tip)
        let fin_h = (body_h * 2).max(2);
        draw::vline(
            grid,
            tail_tip.max(0).min(dw as i32 - 1) as usize,
            (boom_y - fin_h / 2).max(0) as usize,
            (boom_y + fin_h / 2).min(dh as i32 - 1) as usize,
        );

        // ── Main rotor (two blades, 90° apart, spinning fast) ──
        let rotor_y = body_y - 1;
        let rotor_r = ((dw as f32 * 0.38).max(4.0)) as i32;
        let rotor_spin = ctx.time * 8.0 * PI; // fast spin

        for blade_i in 0..2 {
            let angle = rotor_spin + blade_i as f32 * PI;
            let rx = (cx as f32 + rotor_r as f32 * angle.cos()) as i32;
            let ry = (rotor_y as f32 + rotor_r as f32 * angle.sin() * 0.2) as i32;
            line(grid, cx, rotor_y, rx, ry);
        }
        // Rotor hub
        disk(grid, cx, rotor_y, 1);

        // ── Tail rotor (small, vertical, at tail tip) ──
        let tr_r = (fin_h / 3).max(1);
        let tr_spin = ctx.time * 12.0 * PI;
        let tr_cx = tail_tip.min(dw as i32 - 2);
        let tr_cy = boom_y;
        for blade_i in 0..2 {
            let angle = tr_spin + blade_i as f32 * PI;
            let tx = (tr_cx as f32 + tr_r as f32 * angle.cos() * 0.2) as i32;
            let ty = (tr_cy as f32 + tr_r as f32 * angle.sin()) as i32;
            line(grid, tr_cx, tr_cy, tx, ty);
        }

        // ── Landing skids ──
        let skid_y = (body_y + body_h + 1).min(dh as i32 - 1);
        let skid_l0 = (body_x0 - body_w / 6).max(0) as usize;
        let skid_l1 = (body_x0 + body_w / 4).min(dw as i32 - 1).max(0) as usize;
        let skid_r0 = (body_x1 - body_w / 4).max(0) as usize;
        let skid_r1 = (body_x1 + body_w / 6).min(dw as i32 - 1).max(0) as usize;
        draw::hline(grid, skid_l0, skid_l1, skid_y.max(0) as usize);
        draw::hline(grid, skid_r0, skid_r1, skid_y.max(0) as usize);
        // Struts: short verticals connecting body bottom to skid
        draw::vline(
            grid,
            (body_x0 + body_w / 5).max(0) as usize,
            (body_y + body_h).max(0) as usize,
            skid_y.max(0) as usize,
        );
        draw::vline(
            grid,
            (body_x1 - body_w / 5).max(0) as usize,
            (body_y + body_h).max(0) as usize,
            skid_y.max(0) as usize,
        );

        // Color
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cx_cell in 0..filled.min(cw) {
            let t = cx_cell as f32 / cw.max(1) as f32;
            for cy_cell in 0..ch {
                draw::tint_row(grid, cy_cell, cx_cell, cx_cell, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}
