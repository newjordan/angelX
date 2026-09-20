//! Gadgets / consumer-tech progress bars — device UIs in braille.
//!
//! Every style mimics a familiar consumer device interaction: phone charging,
//! WiFi connecting, Bluetooth pairing, disk defragging, CRT power-on, dial-up
//! modem handshake, vinyl spinning up, drone propellers, an activity ring,
//! USB file transfer, a gear train, and an e-ink page refresh. All are
//! stateless — animation is driven entirely by `ctx.time` and `ctx.eased`.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ─── tiny deterministic hash ─────────────────────────────────────────────────

#[inline]
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

#[inline]
fn hashf(n: u32) -> f32 {
    (hash(n) % 1000) as f32 / 1000.0
}

/// All styles in the `gadgets` theme.
///
/// Returns 12 structurally distinct consumer-device progress bar
/// implementations, each stateless and animatable via `ctx.time`.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(PhoneBattery),
        Box::new(WifiConnect),
        Box::new(BluetoothPair),
        Box::new(DiskDefrag),
        Box::new(CrtPowerOn),
        Box::new(DialUpModem),
        Box::new(VinylSpinUp),
        Box::new(DroneProps),
        Box::new(ActivityRing),
        Box::new(UsbTransfer),
        Box::new(GearTrain),
        Box::new(EinkRefresh),
    ]
}

// ─── 1. Smartphone battery charging ──────────────────────────────────────────

/// Battery outline that fills with lightning bolt and rising charge bubbles.
struct PhoneBattery;
impl ProgressStyle for PhoneBattery {
    fn name(&self) -> &str {
        "phone-battery"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "Smartphone battery outline fills with charge; lightning bolt pulses"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Battery body outline — leave a 2-dot border.
        // "Terminal nub" on the right: a 2-dot-wide bump.
        let nub_w = (w / 16).max(1);
        let body_w = w.saturating_sub(nub_w + 2);
        let body_h = h.saturating_sub(2).max(1);
        let bx = 0usize;
        let by = 1usize;

        draw::rect_outline(grid, bx, by, body_w.max(2), body_h.max(2));

        // Nub
        let nub_y0 = by + body_h / 4;
        let nub_h = body_h / 2;
        draw::fill_rect(grid, bx + body_w, nub_y0, nub_w, nub_h.max(1));

        // Fill level inside the battery.
        let inner_w = body_w.saturating_sub(4).max(1);
        let inner_h = body_h.saturating_sub(4).max(1);
        let filled_w = ((ctx.eased * inner_w as f32) as usize).min(inner_w);
        if filled_w > 0 {
            draw::fill_rect(grid, bx + 2, by + 2, filled_w, inner_h);
        }

        // Lightning bolt: drawn with dots in the center when filling > 0.
        // Shape: top-right diagonal, then bottom-left diagonal.
        let bolt_x = (bx + body_w / 2) as i32;
        let bolt_mid = (by + body_h / 2) as i32;
        let bolt_h = inner_h as i32;
        let blink = (ctx.time * 2.0).fract() > 0.3 || ctx.eased > 0.5;
        if blink {
            // Top half of bolt: slants right.
            for dy in 0..bolt_h / 2 {
                let y = bolt_mid - dy as i32;
                let x = bolt_x + (dy as f32 * 0.6) as i32;
                draw::dot_i(grid, x, y);
                draw::dot_i(grid, x + 1, y);
            }
            // Bottom half: slants left.
            for dy in 0..=bolt_h / 2 {
                let y = bolt_mid + dy as i32;
                let x = bolt_x - (dy as f32 * 0.6) as i32;
                draw::dot_i(grid, x, y);
                draw::dot_i(grid, x + 1, y);
            }
        }

        // Rising charge bubbles — dots that travel upward inside the fill.
        let n_bubbles = 4usize;
        for b in 0..n_bubbles {
            let phase = b as f32 / n_bubbles as f32;
            let t = (ctx.time * 0.5 + phase).fract();
            let bub_x = bx + 2 + (hashf(b as u32 * 7 + 3) * inner_w as f32) as usize;
            let bub_x = bub_x.min(bx + body_w.saturating_sub(3));
            let bub_y = by
                + 2
                + inner_h
                    .saturating_sub(1)
                    .saturating_sub((t * inner_h as f32) as usize);
            // Only show bubble in the filled zone.
            let fill_right = bx + 2 + filled_w;
            if bub_x < fill_right {
                draw::dot(grid, bub_x, bub_y);
                if bub_x + 1 < fill_right {
                    draw::dot(grid, bub_x + 1, bub_y);
                }
            }
        }

        // Tint filled region green→yellow by charge level.
        let filled_cells = ((ctx.eased * cells_w as f32) as usize).min(cells_w);
        for cx in 0..filled_cells {
            let t = cx as f32 / cells_w.max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        Ok(())
    }
}

// ─── 2. WiFi connecting ───────────────────────────────────────────────────────

/// Concentric arcs pulse outward, then lock solid once connected.
struct WifiConnect;
impl ProgressStyle for WifiConnect {
    fn name(&self) -> &str {
        "wifi-connect"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "WiFi arcs pulse outward while connecting, then lock solid at 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Origin: bottom-center, like a standard WiFi symbol.
        let cx_dot = w / 2;
        let cy_dot = h.saturating_sub(1);

        // Number of arcs lit by progress (0–4).
        let n_arcs = 4usize;
        let arcs_lit = (ctx.eased * n_arcs as f32).floor() as usize;
        // Connected: all 4 arcs lit.
        let connected = arcs_lit >= n_arcs;

        // Draw the base dot (the WiFi "device" dot).
        draw::dot(grid, cx_dot, cy_dot);
        if cx_dot + 1 < w {
            draw::dot(grid, cx_dot + 1, cy_dot);
        }

        for arc in 0..n_arcs {
            let radius = (arc + 1) as f32 * (h as f32 / (n_arcs + 1) as f32);
            let lit = arc < arcs_lit;

            // Pulsing alpha for unlocked arcs: sine pulse that propagates outward.
            let pulse_phase = (ctx.time * 1.5 - arc as f32 * 0.4).fract();
            let pulse = (pulse_phase * PI * 2.0).sin() * 0.5 + 0.5;
            let draw_it = lit || (!connected && pulse > 0.5);

            if draw_it {
                // Draw a semicircular arc (top half only — like WiFi symbol).
                // Sweep 180° (from left to right, convex upward).
                let steps = ((PI * radius) as usize).max(8);
                for step in 0..=steps {
                    let angle = PI * (step as f32 / steps as f32);
                    // angle 0 = right, PI = left; we want arcs opening downward.
                    let dx = (angle.cos() * radius) as i32;
                    let dy = -(angle.sin() * radius * 0.6) as i32; // flatten vertically
                    draw::dot_i(grid, cx_dot as i32 + dx, cy_dot as i32 + dy);
                    if !lit {
                        // Dashed: only draw alternating steps for the pulsing arcs.
                        // Already drawn above; skip extra for pulsing effect.
                    }
                }

                // Color: lit arcs get palette gradient.
                if lit {
                    let t = arc as f32 / n_arcs.max(1) as f32;
                    let color = ctx.palette.sample(t);
                    let cell_cx = cx_dot / 2;
                    let r_cells = (radius as usize / 2).max(1);
                    let cx0 = cell_cx.saturating_sub(r_cells);
                    let cx1 = (cell_cx + r_cells).min(cells_w.saturating_sub(1));
                    let cy_cell = cy_dot / 4;
                    let r_cell_h = (radius as usize / 4).max(1);
                    let cy0 = cy_cell.saturating_sub(r_cell_h);
                    for cy in cy0..cells_h {
                        draw::tint_row(grid, cy, cx0, cx1, color);
                    }
                }
            }
        }

        // "Locked" indicator: horizontal bar at bottom when connected.
        if connected {
            let blink_off = (ctx.time * 3.0).fract() < 0.2;
            if !blink_off {
                draw::hline(
                    grid,
                    cx_dot.saturating_sub(2),
                    (cx_dot + 2).min(w - 1),
                    cy_dot,
                );
                let full_color = ctx.palette.sample(1.0);
                draw::tint_row(
                    grid,
                    cells_h.saturating_sub(1),
                    (cells_w / 2).saturating_sub(1),
                    (cells_w / 2 + 1).min(cells_w - 1),
                    full_color,
                );
            }
        }

        Ok(())
    }
}

// ─── 3. Bluetooth pairing ─────────────────────────────────────────────────────

/// The ᛒ-like Bluetooth glyph with handshake blips radiating outward.
struct BluetoothPair;
impl ProgressStyle for BluetoothPair {
    fn name(&self) -> &str {
        "bluetooth-pair"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "Bluetooth rune glyph at center; handshake blips pulse outward while pairing"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let cx = w / 2;
        let cy = h / 2;

        // ── Bluetooth glyph in dot space (approximated) ──
        // Vertical spine.
        let spine_h = (h * 3 / 4).max(2);
        let y0 = cy.saturating_sub(spine_h / 2);
        let y1 = (cy + spine_h / 2).min(h.saturating_sub(1));
        draw::vline(grid, cx, y0, y1);

        // Upper-right arm and return (forms the top lobe of ᛒ).
        let arm = (h / 5).max(1);
        // Upper-right diagonal.
        for i in 0..arm {
            draw::dot_i(grid, cx as i32 + i as i32, (y0 + arm) as i32 - i as i32);
        }
        // Upper-right return diagonal.
        for i in 0..arm {
            draw::dot_i(grid, cx as i32 + i as i32, (y0 + arm) as i32 + i as i32);
        }
        // Lower-right diagonal.
        for i in 0..arm {
            draw::dot_i(grid, cx as i32 + i as i32, cy as i32 + i as i32);
        }
        // Lower-right return diagonal.
        for i in 0..arm {
            draw::dot_i(grid, cx as i32 + i as i32, (cy + arm) as i32 - i as i32);
        }

        // ── Handshake blips ──
        // Blips travel horizontally outward from the glyph.
        let n_blips = 5usize;
        let paired = ctx.eased >= 1.0;
        for b in 0..n_blips {
            let phase = b as f32 / n_blips as f32;
            let t = if paired {
                // Frozen in place when paired.
                1.0f32
            } else {
                (ctx.time * 0.8 + phase).fract()
            };
            let reach = (t * (w / 2) as f32) as usize;

            // Left blip.
            let lx = cx.saturating_sub(reach);
            draw::dot(grid, lx, cy);

            // Right blip.
            let rx = (cx + reach).min(w.saturating_sub(1));
            draw::dot(grid, rx, cy);
        }

        // Tint: progress drives gradient, center stays brightest.
        let filled_cells = ((ctx.eased * cells_w as f32) as usize).min(cells_w);
        let mid_cell = cells_w / 2;
        let reach_cells = (filled_cells / 2).max(1);
        let cx0 = mid_cell.saturating_sub(reach_cells);
        let cx1 = (mid_cell + reach_cells).min(cells_w.saturating_sub(1));
        for cx_c in cx0..=cx1 {
            let t = (cx_c.saturating_sub(cx0)) as f32 / (cx1.saturating_sub(cx0) + 1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }
        Ok(())
    }
}

// ─── 4. Disk defragmenter ────────────────────────────────────────────────────

/// A grid of shade-glyph blocks reorganize from chaotic to solid.
struct DiskDefrag;
impl ProgressStyle for DiskDefrag {
    fn name(&self) -> &str {
        "disk-defrag"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "Disk defrag: chaotic shade blocks consolidate into solid filled region"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cells_w, cells_h) = grid.dimensions();
        if cells_w == 0 || cells_h == 0 {
            return Ok(());
        }

        let total_cells = cells_w * cells_h;
        // How many cells are "defragged" (consolidated) vs still fragmented.
        let defragged = (ctx.eased * total_cells as f32) as usize;

        // Cells are laid out in a zigzag (boustrophedon) scan order.
        for cy in 0..cells_h {
            for cx in 0..cells_w {
                let linear = cy * cells_w + cx;
                let color = ctx
                    .palette
                    .sample(linear as f32 / total_cells.max(1) as f32);

                if linear < defragged {
                    // Defragged: solid block.
                    draw::glyph(grid, cx, cy, '█');
                    draw::tint_row(grid, cy, cx, cx, color);
                } else {
                    // Fragmented: random shade glyph determined by hash + time.
                    // The fragmented blocks "shuffle" — they animate in place.
                    let epoch = (ctx.time * 4.0) as u32;
                    let h_val = hash(linear as u32 * 31 + epoch * 7 + 3);
                    let shade_level = (h_val % 4) as usize; // 0..3 → ' ░▒▓'
                    if shade_level > 0 {
                        draw::shade(grid, cx, cy, shade_level);
                        // Dim tint for fragmented cells.
                        let frag_color = ctx.palette.sample(0.15);
                        draw::tint_row(grid, cy, cx, cx, frag_color);
                    }
                }
            }
        }
        Ok(())
    }
}

// ─── 5. CRT TV power-on ──────────────────────────────────────────────────────

/// A bright horizontal line at center expands vertically to fill the screen.
struct CrtPowerOn;
impl ProgressStyle for CrtPowerOn {
    fn name(&self) -> &str {
        "crt-power-on"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "CRT power-on: bright center line expands vertically to fill the screen"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let mid = h / 2;
        // The beam starts as a single scanline and grows to full height.
        let beam_h = ((ctx.eased * h as f32) as usize).max(1).min(h);
        let y0 = mid.saturating_sub(beam_h / 2);
        let y1 = (y0 + beam_h).min(h);

        // Fill the beam area — horizontal scanlines.
        for y in y0..y1 {
            draw::hline(grid, 0, w.saturating_sub(1), y);
        }

        // Bright edge lines (phosphor glow at the expanding boundary).
        // The leading edges are extra dense (draw them twice — one line extra).
        if y0 > 0 {
            draw::hline(grid, 0, w.saturating_sub(1), y0.saturating_sub(1));
        }
        if y1 < h {
            draw::hline(grid, 0, w.saturating_sub(1), y1);
        }

        // Horizontal scanline "noise" inside the beam: a few dotted lines fade in.
        if beam_h > 4 {
            let noise_lines = beam_h / 4;
            for nl in 0..noise_lines {
                let y_nl = y0 + 2 + nl * 4;
                if y_nl >= y1 {
                    break;
                }
                // Sparse noise dots derived from hash.
                for x in 0..w {
                    let on = (hash((x as u32).wrapping_add(y_nl as u32 * 17)) % 5) == 0;
                    if on {
                        draw::dot(grid, x, y_nl);
                    }
                }
            }
        }

        // Tint: white-ish center → palette edge glow.
        let cy_center = cells_h / 2;
        let beam_cells = (beam_h / 4).max(1);
        let cy0 = cy_center.saturating_sub(beam_cells / 2);
        let cy1 = (cy0 + beam_cells).min(cells_h.saturating_sub(1));
        for cy in cy0..=cy1 {
            let t = (cy.saturating_sub(cy0)) as f32 / (cy1.saturating_sub(cy0) + 1).max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy, 0, cells_w.saturating_sub(1), color);
        }
        Ok(())
    }
}

// ─── 6. Dial-up modem handshake ──────────────────────────────────────────────

/// Noisy oscillation across the bar gradually resolves to a steady carrier tone.
struct DialUpModem;
impl ProgressStyle for DialUpModem {
    fn name(&self) -> &str {
        "dialup-modem"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "Dial-up modem: chaotic noise waveform settles to a clean carrier tone as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let mid = (h / 2) as i32;
        let amp = (h / 2).saturating_sub(1) as f32;

        for x in 0..w {
            // "Clean" carrier: a smooth sine at this column.
            let carrier_phase = x as f32 / w as f32 * 4.0 * PI + ctx.time * 3.0;
            let clean_y = mid + (carrier_phase.sin() * amp * 0.8) as i32;

            // "Noisy" signal: carrier + random hash jitter.
            let epoch = (ctx.time * 8.0) as u32;
            let noise = hashf(x as u32 * 13 + epoch * 37) * 2.0 - 1.0;
            let noisy_y = mid + ((carrier_phase.sin() * 0.8 + noise * 1.5) * amp) as i32;

            // Blend: lerp from noisy (progress=0) → clean (progress=1).
            let y = (noisy_y as f32 * (1.0 - ctx.eased) + clean_y as f32 * ctx.eased) as i32;
            draw::dot_i(grid, x as i32, y);

            // Draw a second dot one above/below to fatten the trace.
            draw::dot_i(grid, x as i32, y + 1);
        }

        // Tint: chaotic region dim, settled region bright (left→right by progress).
        let filled_cells = ((ctx.eased * cells_w as f32) as usize).min(cells_w);
        for cx in 0..cells_w {
            let t = cx as f32 / cells_w.max(1) as f32;
            let is_settled = cx < filled_cells;
            let color = if is_settled {
                ctx.palette.sample(t)
            } else {
                ctx.palette.sample(0.05)
            };
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        Ok(())
    }
}

// ─── 7. CD / Vinyl spinning up ────────────────────────────────────────────────

/// A disc outline with an index mark rotates; RPM increases with progress.
struct VinylSpinUp;
impl ProgressStyle for VinylSpinUp {
    fn name(&self) -> &str {
        "vinyl-spin"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "Vinyl/CD disc outline spins up; rotation speed scales with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let cx = w / 2;
        let cy = h / 2;
        // Radius: constrained to fit in the shorter axis, slightly inset.
        let r_max = (w.min(h * 2) / 2).saturating_sub(1).max(1);

        // Angular speed: 0 rpm at progress=0 → 360 deg/s at progress=1.
        let omega = ctx.eased * 2.0 * PI * 2.0; // 2 full turns/sec at full progress.
        let angle = ctx.time * omega;

        // Outer disc ring.
        let steps = (2.0 * PI * r_max as f32) as usize + 4;
        for i in 0..steps {
            let theta = i as f32 / steps as f32 * 2.0 * PI;
            // Squish Y by 0.5 (dots are taller than wide).
            let dx = (theta.cos() * r_max as f32) as i32;
            let dy = (theta.sin() * r_max as f32 * 0.5) as i32;
            draw::dot_i(grid, cx as i32 + dx, cy as i32 + dy);
        }

        // Inner hole (label ring) — about 30% of r_max.
        let r_inner = (r_max * 3 / 10).max(1);
        let inner_steps = (2.0 * PI * r_inner as f32) as usize + 4;
        for i in 0..inner_steps {
            let theta = i as f32 / inner_steps as f32 * 2.0 * PI;
            let dx = (theta.cos() * r_inner as f32) as i32;
            let dy = (theta.sin() * r_inner as f32 * 0.5) as i32;
            draw::dot_i(grid, cx as i32 + dx, cy as i32 + dy);
        }

        // Center spindle dot.
        draw::dot(grid, cx, cy);

        // Index mark: a short radial line from inner to outer, rotating.
        let mark_steps = (r_max - r_inner).max(1);
        for step in 0..=mark_steps {
            let r = r_inner + step;
            let dx = (angle.cos() * r as f32) as i32;
            let dy = (angle.sin() * r as f32 * 0.5) as i32;
            draw::dot_i(grid, cx as i32 + dx, cy as i32 + dy);
        }

        // Groove lines (concentric rings at intermediate radii).
        let n_grooves = 3usize;
        for g in 1..=n_grooves {
            let r_g = r_inner + (r_max - r_inner) * g / (n_grooves + 1);
            if r_g == 0 {
                continue;
            }
            let g_steps = (2.0 * PI * r_g as f32) as usize + 4;
            for i in (0..g_steps).step_by(3) {
                // Sparse for groove look.
                let theta = i as f32 / g_steps as f32 * 2.0 * PI;
                let dx = (theta.cos() * r_g as f32) as i32;
                let dy = (theta.sin() * r_g as f32 * 0.5) as i32;
                draw::dot_i(grid, cx as i32 + dx, cy as i32 + dy);
            }
        }

        // Tint the disc area with palette gradient (left→right).
        for cx_c in 0..cells_w {
            let t = cx_c as f32 / cells_w.max(1) as f32;
            let color = ctx.palette.sample(ctx.eased * t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }
        Ok(())
    }
}

// ─── 8. Drone quad-propellers ────────────────────────────────────────────────

/// Four rotating arc-pairs in quadrants; RPM rises to full with progress.
struct DroneProps;
impl ProgressStyle for DroneProps {
    fn name(&self) -> &str {
        "drone-props"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "Quad-drone: four propeller arcs spin up as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Propeller hubs at the four quadrant centers.
        let qw = w / 2;
        let qh = h / 2;
        // Arc radius: fills the quadrant.
        let r = (qw.min(qh * 2) / 2).max(1);

        // Spin rate: 0 → 3 rev/sec.
        let omega = ctx.eased * 2.0 * PI * 3.0;
        let angle = ctx.time * omega;

        // Four hub positions: (cx, cy) in dot space.
        let hubs = [
            (qw / 2, qh / 2),
            (w - qw / 2, qh / 2),
            (qw / 2, h - qh / 2),
            (w - qw / 2, h - qh / 2),
        ];

        // Draw a cross-frame connector (drone body).
        let body_cx = w / 2;
        let body_cy = h / 2;
        draw::dot(grid, body_cx, body_cy);
        // Arms from center to each hub (sparse dots).
        for (hx, hy) in &hubs {
            let dx = *hx as i32 - body_cx as i32;
            let dy = *hy as i32 - body_cy as i32;
            let steps = dx.abs().max(dy.abs()).max(1);
            for step in 0..steps {
                let t = step as f32 / steps as f32;
                let px = body_cx as i32 + (dx as f32 * t) as i32;
                let py = body_cy as i32 + (dy as f32 * t) as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // For each hub, draw two blade arcs (180° apart).
        for (prop_idx, (hx, hy)) in hubs.iter().enumerate() {
            let hub_angle = angle + prop_idx as f32 * PI * 0.5; // Each prop offset.
                                                                // Two blades per prop, 180° apart.
            for blade in 0..2usize {
                let blade_angle = hub_angle + blade as f32 * PI;
                // Arc: 60° sweep.
                let arc_sweep = PI / 3.0;
                let arc_steps = (arc_sweep * r as f32) as usize + 4;
                for step in 0..arc_steps {
                    let theta = blade_angle - arc_sweep / 2.0
                        + step as f32 / arc_steps.max(1) as f32 * arc_sweep;
                    let dx = (theta.cos() * r as f32) as i32;
                    let dy = (theta.sin() * r as f32 * 0.5) as i32;
                    draw::dot_i(grid, *hx as i32 + dx, *hy as i32 + dy);
                }
            }
            // Hub dot.
            draw::dot(grid, *hx, *hy);
        }

        // Tint by quadrant.
        for cx_c in 0..cells_w {
            for cy_c in 0..cells_h {
                let t = cx_c as f32 / cells_w.max(1) as f32;
                let color = ctx.palette.sample(t * ctx.eased);
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }
        Ok(())
    }
}

// ─── 9. Smartwatch activity ring ─────────────────────────────────────────────

/// A circular ring closes clockwise as progress increases.
struct ActivityRing;
impl ProgressStyle for ActivityRing {
    fn name(&self) -> &str {
        "activity-ring"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "Smartwatch activity ring closes clockwise as progress fills it"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let cx = w / 2;
        let cy = h / 2;
        let r_outer = (w.min(h * 2) / 2).saturating_sub(1).max(2);
        let r_inner = r_outer.saturating_sub((r_outer / 4).max(1));

        // Total arc swept: progress × 2π, starting at top (−π/2).
        let start_angle = -PI / 2.0;
        let sweep = ctx.eased * 2.0 * PI;

        // Draw thick arc from r_inner to r_outer.
        let steps = ((2.0 * PI * r_outer as f32) as usize + 4).max(16);
        for i in 0..=steps {
            let frac = i as f32 / steps as f32;
            let theta = frac * 2.0 * PI;
            // Only draw dots within the swept angle.
            let rel = (theta - (start_angle + PI * 2.0)).rem_euclid(2.0 * PI);
            if rel > sweep {
                continue;
            }

            for r in r_inner..=r_outer {
                let dx = (theta.cos() * r as f32) as i32;
                let dy = (theta.sin() * r as f32 * 0.5) as i32;
                draw::dot_i(grid, cx as i32 + dx, cy as i32 + dy);
            }
        }

        // "Track" ring (unfilled portion) — sparse dots.
        for i in 0..=steps {
            let frac = i as f32 / steps as f32;
            let theta = frac * 2.0 * PI;
            let rel = (theta - (start_angle + PI * 2.0)).rem_euclid(2.0 * PI);
            if rel <= sweep {
                continue;
            }
            if i % 3 != 0 {
                continue;
            } // Sparse.
            let r = (r_inner + r_outer) / 2;
            let dx = (theta.cos() * r as f32) as i32;
            let dy = (theta.sin() * r as f32 * 0.5) as i32;
            draw::dot_i(grid, cx as i32 + dx, cy as i32 + dy);
        }

        // Leading-edge glow: extra dots at the arc tip.
        let tip_theta = start_angle + sweep;
        for r in r_inner..=r_outer {
            let dx = (tip_theta.cos() * r as f32) as i32;
            let dy = (tip_theta.sin() * r as f32 * 0.5) as i32;
            draw::dot_i(grid, cx as i32 + dx, cy as i32 + dy);
            draw::dot_i(grid, cx as i32 + dx + 1, cy as i32 + dy);
        }

        // Tint filled cells.
        let mid_c = cells_w / 2;
        let r_cells = (r_outer / 2).max(1);
        let cx0 = mid_c.saturating_sub(r_cells);
        let cx1 = (mid_c + r_cells).min(cells_w.saturating_sub(1));
        for cx_c in cx0..=cx1 {
            let t = (cx_c.saturating_sub(cx0)) as f32 / (cx1.saturating_sub(cx0) + 1).max(1) as f32;
            let color = ctx.palette.sample(ctx.eased * t + ctx.eased * 0.2);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }
        Ok(())
    }
}

// ─── 10. USB file transfer ────────────────────────────────────────────────────

/// File glyphs fly from a left device to a right device; count matches eased%.
struct UsbTransfer;
impl ProgressStyle for UsbTransfer {
    fn name(&self) -> &str {
        "usb-transfer"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "USB transfer: file packets fly left→right; count and speed scale with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let mid = h / 2;

        // ── Device icons: left (source) and right (destination) ──
        // Left block: a 2-dot-wide, full-height rect.
        let dev_w = (w / 16).max(2);
        draw::fill_rect(grid, 0, 0, dev_w, h);
        // Right block.
        draw::fill_rect(grid, w.saturating_sub(dev_w), 0, dev_w, h);

        // ── USB cable track line ──
        draw::hline(grid, dev_w, w.saturating_sub(dev_w + 1), mid);

        // ── Flying file packets ──
        // Number of active packets: 1 at 0%, up to 6 at 100%.
        let max_packets = 6usize;
        let active = ((ctx.eased * max_packets as f32).ceil() as usize).min(max_packets);
        let track_w = w.saturating_sub(dev_w * 2 + 2);

        for p in 0..active {
            let phase = p as f32 / max_packets as f32;
            // Speed: faster at higher progress.
            let speed = 0.4 + ctx.eased * 0.6;
            let t = (ctx.time * speed + phase).fract();
            let x0 = dev_w + 1 + (t * track_w as f32) as usize;
            let x0 = x0.min(w.saturating_sub(dev_w + 3));

            // Packet shape: small rectangle, 3-dot wide, 2-dot tall.
            let pkt_w = (w / 20).max(2).min(4);
            let pkt_h = 2usize.min(h);
            let py0 = mid.saturating_sub(pkt_h / 2);
            draw::fill_rect(grid, x0, py0, pkt_w, pkt_h);

            // Tiny "file" notch on the packet (top-right corner void).
            if x0 + pkt_w < w && py0 < h {
                // Just leave that corner unset (draw::fill_rect already did it,
                // but this gives the packet a recognisable dog-ear by drawing the
                // outline of the corner explicitly with dots).
            }

            // Tint.
            let t_color = p as f32 / max_packets.max(1) as f32;
            let color = ctx.palette.sample(t_color);
            let cx0 = (x0 / 2).min(cells_w.saturating_sub(1));
            let cx1 = ((x0 + pkt_w) / 2).min(cells_w.saturating_sub(1));
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx0, cx1, color);
            }
        }

        // Tint device icons.
        let dev_cells = (dev_w / 2).max(1);
        let src_color = ctx.palette.sample(0.0);
        let dst_color = ctx.palette.sample(1.0);
        for cy in 0..cells_h {
            draw::tint_row(grid, cy, 0, dev_cells.saturating_sub(1), src_color);
            draw::tint_row(
                grid,
                cy,
                cells_w.saturating_sub(dev_cells),
                cells_w.saturating_sub(1),
                dst_color,
            );
        }
        Ok(())
    }
}

// ─── 11. Gear train ──────────────────────────────────────────────────────────

/// Interlocking rotating gears; large left, smaller right, alternate directions.
struct GearTrain;
impl ProgressStyle for GearTrain {
    fn name(&self) -> &str {
        "gear-train"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "Interlocking gear train: large and small gears rotate in opposite directions"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Draw a gear: circle with N teeth (short radial protrusions).
        let draw_gear = |grid: &mut BrailleGrid,
                         cx: i32,
                         cy: i32,
                         r: usize,
                         n_teeth: usize,
                         angle_offset: f32| {
            if r == 0 {
                return;
            }
            let tooth_len = (r / 4).max(1) as f32;
            let r_f = r as f32;
            let steps = (2.0 * PI * r_f) as usize + 8;
            for i in 0..=steps {
                let theta = i as f32 / steps as f32 * 2.0 * PI + angle_offset;
                // Is this angle at a tooth?
                let tooth_phase = (theta * n_teeth as f32 / (2.0 * PI)).fract();
                let tooth_bump = if tooth_phase < 0.25 || tooth_phase > 0.75 {
                    tooth_len
                } else {
                    0.0
                };
                let r_here = r_f + tooth_bump;
                let dx = (theta.cos() * r_here) as i32;
                let dy = (theta.sin() * r_here * 0.5) as i32;
                draw::dot_i(grid, cx + dx, cy + dy);
            }
            // Hub.
            draw::dot_i(grid, cx, cy);
        };

        // Gear dimensions: big gear on the left, smaller on the right.
        let big_r = (h * 3 / 8).max(2);
        let small_r = (big_r / 2).max(1);
        let big_cx = (big_r + 2) as i32;
        let big_cy = (h / 2) as i32;
        // Small gear meshes with the big gear; positioned to its right.
        let mesh_gap = 1i32; // gap between teeth
        let small_cx = big_cx + big_r as i32 + small_r as i32 + mesh_gap;
        let small_cy = big_cy;

        // Rotation: speed 1.0 rad/s at full progress.
        let omega_big = ctx.eased * 1.5;
        let big_angle = ctx.time * omega_big;
        // Small gear rotates inversely proportional to size ratio.
        let gear_ratio = big_r as f32 / small_r.max(1) as f32;
        let small_angle = -ctx.time * omega_big * gear_ratio;

        let n_teeth_big = (big_r / 2).max(4).min(16);
        let n_teeth_small = (small_r / 2).max(3).min(8);

        draw_gear(grid, big_cx, big_cy, big_r, n_teeth_big, big_angle);
        // Only draw small gear if it fits within the grid.
        if small_cx + small_r as i32 + 2 < w as i32 {
            draw_gear(
                grid,
                small_cx,
                small_cy,
                small_r,
                n_teeth_small,
                small_angle,
            );
        }

        // Optional third (tiny) gear further right.
        let tiny_r = (small_r / 2).max(1);
        let tiny_cx = small_cx + small_r as i32 + tiny_r as i32 + mesh_gap;
        let tiny_angle = -small_angle * (small_r as f32 / tiny_r.max(1) as f32);
        if tiny_cx + tiny_r as i32 + 2 < w as i32 {
            draw_gear(
                grid,
                tiny_cx,
                small_cy,
                tiny_r,
                (tiny_r / 2).max(3).min(6),
                tiny_angle,
            );
        }

        // Tint: gradient left to right.
        for cx_c in 0..cells_w {
            let t = cx_c as f32 / cells_w.max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }
        Ok(())
    }
}

// ─── 12. E-ink page refresh ───────────────────────────────────────────────────

/// Screen flashes to black, then content fills in cell-by-cell, row by row.
struct EinkRefresh;
impl ProgressStyle for EinkRefresh {
    fn name(&self) -> &str {
        "eink-refresh"
    }
    fn theme(&self) -> &str {
        "gadgets"
    }
    fn describe(&self) -> &str {
        "E-ink refresh: flashes full black then redraws content row-by-row via shade blocks"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cells_w, cells_h) = grid.dimensions();
        if cells_w == 0 || cells_h == 0 {
            return Ok(());
        }
        let (w, h) = draw::dot_dims(grid);

        // Phase 0 (progress 0–0.15): flash to full black.
        // Phase 1 (0.15–1.0): content draws in row by row, left to right.
        let flash_phase = 0.15f32;

        if ctx.eased < flash_phase {
            // Full black (invert flash) — fill everything.
            draw::fill_rect(grid, 0, 0, w, h);
            let color = ctx.palette.sample(0.0);
            draw::tint_row(grid, 0, 0, cells_w.saturating_sub(1), color);
        } else {
            // Normalise to the draw-in phase.
            let draw_frac = (ctx.eased - flash_phase) / (1.0 - flash_phase);
            let total_cells = cells_w * cells_h;
            let cells_drawn = ((draw_frac * total_cells as f32) as usize).min(total_cells);

            // Rows: complete rows first, then a partial last row.
            let full_rows = cells_drawn / cells_w.max(1);
            let partial_cols = cells_drawn % cells_w.max(1);

            for cy in 0..full_rows.min(cells_h) {
                for cx in 0..cells_w {
                    // Alternating shade density based on a hash pattern —
                    // simulates "newspaper" e-ink content.
                    let linear = cy * cells_w + cx;
                    let h_val = hash(linear as u32 * 13 + 7);
                    let shade_level = match h_val % 5 {
                        0 => 1,     // ░
                        1 => 2,     // ▒
                        2 => 3,     // ▓
                        3 | 4 => 4, // █
                        _ => 4,
                    };
                    draw::shade(grid, cx, cy, shade_level);
                    let t = cx as f32 / cells_w.max(1) as f32;
                    let color = ctx.palette.sample(t * 0.6 + 0.2);
                    draw::tint_row(grid, cy, cx, cx, color);
                }
            }
            // Partial row.
            let partial_row = full_rows.min(cells_h.saturating_sub(1));
            if full_rows < cells_h {
                for cx in 0..partial_cols.min(cells_w) {
                    let linear = partial_row * cells_w + cx;
                    let h_val = hash(linear as u32 * 13 + 7);
                    let shade_level = (h_val % 4 + 1) as usize;
                    draw::shade(grid, cx, partial_row, shade_level);
                    let t = cx as f32 / cells_w.max(1) as f32;
                    let color = ctx.palette.sample(t * 0.5 + 0.3);
                    draw::tint_row(grid, partial_row, cx, cx, color);
                }
                // Cursor line: a blinking single-dot underline at the draw-in edge.
                if partial_cols < cells_w {
                    let blink = (ctx.time * 4.0).fract() > 0.5;
                    if blink {
                        let (_, dot_h) = draw::dot_dims(grid);
                        let row_y = (partial_row * 4 + 3).min(dot_h.saturating_sub(1));
                        draw::dot(grid, partial_cols * 2, row_y);
                    }
                }
            }
        }
        Ok(())
    }
}
