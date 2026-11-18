//! The Stereopticon: video replay in a held frame over the terminal
//! (docs/RENDER_FOUNDATION.md, "Media" — the video slice).
//!
//! In the window, the artifacts pane's videos stop drawing braille into
//! their cells and play as glass instead: the decoder's RGBA (the same
//! off-thread worker, the same clock, caps and controls) is uploaded as
//! one picture a frame and framed as a plate leaning over the black
//! terminal — the Orrery's bracket chrome and lean, a rim of light, a
//! scan shimmer, a progress track with quarter ticks, and the screen's
//! own glow spilling onto the floor beneath. The pane's cells behind are
//! laid black, so the frame floats where the pane was. An ended replay
//! holds a beat and starts itself again; pauses and seeks keep working,
//! because the machinery underneath is the Scryglass's own.
//!
//! In a terminal every entry point returns false and the caller paints
//! cells as before. The frame's geometry is pure and shared with the
//! Orrery's (`ui::orrery`), so both panels lean the same way.

use crate::ui::scryglass::VideoPixels;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::Instant;

#[cfg(feature = "glass")]
const GROUP: u32 = 0x5354_5250;

#[cfg(feature = "glass")]
const SCREEN: u32 = angel_glass::LAYER_PICTURES + 0x57A7_0000;

#[cfg(feature = "glass")]
const CHROME: [f32; 4] = [0.35, 0.85, 1.0, 0.85];
#[cfg(feature = "glass")]
const PLATE: [f32; 4] = [0.012, 0.03, 0.055, 0.72];

pub(crate) fn decode_viewport(area: Rect, cell: (u16, u16)) -> (usize, usize) {
    if cell.0 == 0 || cell.1 == 0 {
        return (2, 2);
    }
    let w = (u32::from(area.width) * u32::from(cell.0)).div_ceil(2);
    let h = (u32::from(area.height) * u32::from(cell.1)).div_ceil(4);
    (
        w.clamp(1, 640) as usize,
        h.clamp(1, 180) as usize,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Played {
    Hidden,
    Shown,
    Restarting,
}

const REPLAY_HOLD: std::time::Duration = std::time::Duration::from_millis(1800);

#[derive(Default)]
struct Deck {
    uploaded: Option<(u64, u32, u32)>,
    ended_at: Option<Instant>,
}

static DECK: std::sync::LazyLock<std::sync::Mutex<Deck>> =
    std::sync::LazyLock::new(Default::default);

pub(crate) fn present(
    pixels: &VideoPixels,
    position: std::time::Duration,
    duration: Option<std::time::Duration>,
    paused: bool,
    anchor: Rect,
    buffer: &mut Buffer,
) -> Played {
    #[cfg(feature = "glass")]
    {
        let Some(host) = crate::ui::glass::host() else {
            return Played::Hidden;
        };
        let Some(cell) = crate::ui::glass::cell_pixels() else {
            return Played::Hidden;
        };
        let (w, h) = (pixels.rgba.width(), pixels.rgba.height());
        if w == 0 || h == 0 || anchor.width == 0 || anchor.height == 0 {
            return Played::Hidden;
        }
        {
            let mut deck = DECK.lock().unwrap();
            if deck.uploaded != Some((pixels.sequence, w, h)) {
                host.upload_picture(angel_glass::graphics::Picture {
                    id: SCREEN,
                    width: w,
                    height: h,
                    rgba: pixels.rgba.as_raw().clone(),
                    cols: 0,
                    rows: 0,
                });
                deck.uploaded = Some((pixels.sequence, w, h));
            }
        }
        let placed = crate::ui::present::fit_span(room_for_plate(anchor), cell, (w, h));
        let k = duration
            .filter(|d| !d.is_zero())
            .map(|d| (position.as_secs_f32() / d.as_secs_f32()).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let layers = frame_layers(placed, k, paused, position.as_secs_f32());
        crate::ui::glass::show_group(host, GROUP, layers);
        blank(buffer, anchor);
        let mut deck = DECK.lock().unwrap();
        if let Some(held) = deck.ended_at
            && held.elapsed() >= REPLAY_HOLD
        {
            deck.ended_at = None;
            return Played::Restarting;
        }
        Played::Shown
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = (pixels, position, duration, paused, anchor, buffer);
        Played::Hidden
    }
}

pub(crate) fn note_ended(ended: bool) {
    let mut deck = DECK.lock().unwrap();
    if ended {
        if deck.ended_at.is_none() {
            deck.ended_at = Some(Instant::now());
        }
    } else {
        deck.ended_at = None;
    }
}

pub(crate) fn warming(anchor: Rect, buffer: &mut Buffer, t: f32) -> bool {
    #[cfg(feature = "glass")]
    {
        let Some(host) = crate::ui::glass::host() else {
            return false;
        };
        if anchor.width < 4 || anchor.height < 3 {
            return false;
        }
        let plate = plate_rect(room_for_plate(anchor));
        let centre = centre_of(plate);
        let layers = warming_layers(plate, centre, t);
        crate::ui::glass::show_group(host, GROUP, layers);
        blank(buffer, anchor);
        true
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = (anchor, buffer, t);
        false
    }
}

fn blank(buffer: &mut Buffer, area: Rect) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_symbol(" ").set_bg(ratatui::style::Color::Rgb(0, 0, 0));
            }
        }
    }
}

#[cfg(feature = "glass")]
fn room_for_plate(anchor: Rect) -> Rect {
    const LEFT: u16 = 1;
    const RIGHT: u16 = 2;
    const TOP: u16 = 2;
    const BOTTOM: u16 = 3;
    if anchor.width <= LEFT + RIGHT + 4 || anchor.height <= TOP + BOTTOM + 3 {
        return anchor;
    }
    Rect {
        x: anchor.x + LEFT,
        y: anchor.y + TOP,
        width: anchor.width - LEFT - RIGHT,
        height: anchor.height - TOP - BOTTOM,
    }
}

#[cfg(feature = "glass")]
fn plate_rect(placed: Rect) -> Rect {
    let grow_w = 0.9_f32.min(f32::from(placed.width) * 0.06);
    let grow_h = 1.1_f32.min(f32::from(placed.height) * 0.08);
    Rect {
        x: placed.x.saturating_sub(grow_w as u16),
        y: placed.y.saturating_sub((grow_h + 0.35) as u16),
        width: placed.width + (2.0 * grow_w) as u16,
        height: placed.height + (2.0 * grow_h + 0.9) as u16,
    }
}

#[cfg(feature = "glass")]
fn centre_of(rect: Rect) -> [f32; 2] {
    [
        f32::from(rect.x) + f32::from(rect.width) / 2.0,
        f32::from(rect.y) + f32::from(rect.height) / 2.0,
    ]
}

#[cfg(feature = "glass")]
fn leaned(id: u64, z: i32, rect: [f32; 4], kind: angel_glass::LayerKind, centre: [f32; 2]) -> angel_glass::Layer {
    let unit = crate::ui::glass::cell_pixels()
        .map_or((12.0, 25.0), |(w, h)| (f32::from(w), f32::from(h)));
    let (rect, angle) = crate::ui::orrery::tilted_rect_in(rect, centre, unit);
    let mut layer = angel_glass::Layer::new(id, z, angel_glass::Placement::Cells(rect), kind);
    layer.angle = angle;
    layer
}

#[cfg(feature = "glass")]
fn frame_layers(placed: Rect, k: f32, paused: bool, clock: f32) -> Vec<angel_glass::Layer> {
    use angel_glass::{Layer, LayerKind, Placement};
    let plate = plate_rect(placed);
    let centre = centre_of(plate);
    let (px, py, pw, ph) = (
        f32::from(plate.x),
        f32::from(plate.y),
        f32::from(plate.width),
        f32::from(plate.height),
    );
    let mut out: Vec<Layer> = Vec::with_capacity(24);

    out.push(Layer::new(
        0,
        0,
        Placement::Cells([px + 0.6, py + ph * 0.55, pw, ph * 0.5]),
        LayerKind::Solid([0.0, 0.0, 0.0, 0.18]),
    ));
    let mut spill = Layer::new(
        1,
        0,
        Placement::Cells([px + 1.2, py + ph - 0.4, pw - 2.4, ph * 0.45]),
        LayerKind::Glow([0.30, 0.55, 0.80, 0.10]),
    );
    spill.additive = true;
    out.push(spill);

    let mut plate_layer = leaned(2, 1, [px, py, pw, ph], LayerKind::Solid(PLATE), centre);
    plate_layer.breathe = 0.8;
    out.push(plate_layer);

    let mut screen = leaned(
        3,
        4,
        [
            f32::from(placed.x),
            f32::from(placed.y),
            f32::from(placed.width),
            f32::from(placed.height),
        ],
        LayerKind::Picture(SCREEN),
        centre,
    );
    screen.breathe = 0.5;
    out.push(screen);

    let (sx, sy, sw, sh) = (
        f32::from(placed.x),
        f32::from(placed.y),
        f32::from(placed.width),
        f32::from(placed.height),
    );
    let mut id = 10u64;
    for rim in [
        [sx - 0.24, sy - 0.24, sw + 0.48, 0.10],
        [sx - 0.24, sy + sh + 0.14, sw + 0.48, 0.10],
        [sx - 0.24, sy - 0.24, 0.10, sh + 0.48],
        [sx + sw + 0.14, sy - 0.24, 0.10, sh + 0.48],
    ] {
        let mut bar = leaned(id, 5, rim, LayerKind::Solid([CHROME[0], CHROME[1], CHROME[2], 0.55]), centre);
        bar.additive = true;
        bar.flicker = 0.06;
        out.push(bar);
        id += 1;
    }

    let arm_w = (pw * 0.14).min(3.6).max(1.2);
    let arm_h = (ph * 0.14).min(2.4).max(0.9);
    let mut id = 20u64;
    for corner in [
        [px - 0.30, py - 0.28],
        [px + pw + 0.30 - arm_w, py - 0.28],
        [px - 0.30, py + ph + 0.19 - arm_h],
        [px + pw + 0.30 - arm_w, py + ph + 0.19 - arm_h],
    ] {
        for arm in [
            [corner[0], corner[1], arm_w, 0.09],
            [corner[0], corner[1], 0.09, arm_h],
        ] {
            let mut bracket = leaned(id, 6, arm, LayerKind::Solid(CHROME), centre);
            bracket.flicker = 0.08;
            out.push(bracket);
            id += 1;
        }
    }

    let track = [sx + 0.5, sy + sh - 0.75, (sw - 1.0).max(1.0), 0.09];
    out.push(leaned(
        40,
        8,
        track,
        LayerKind::Solid([0.04, 0.08, 0.12, 0.85]),
        centre,
    ));
    let fill_w = (track[2] * k).max(0.05);
    let mut fill = leaned(
        41,
        9,
        [track[0], track[1], fill_w, track[3]],
        LayerKind::Solid([0.30, 0.90, 0.95, 0.8]),
        centre,
    );
    fill.additive = true;
    out.push(fill);
    for q in 1..4 {
        let at = track[0] + track[2] * (q as f32) / 4.0;
        let mut tick = leaned(
            44 + q as u64,
            9,
            [at - 0.08, track[1] - 0.42, 0.16, 0.32],
            LayerKind::Solid([CHROME[0], CHROME[1], CHROME[2], 0.85]),
            centre,
        );
        tick.additive = true;
        out.push(tick);
    }

    if paused {
        for (i, dx) in [-0.85f32, 0.35].into_iter().enumerate() {
            let mut bar = leaned(
                50 + i as u64,
                8,
                [sx + sw / 2.0 + dx, sy + sh / 2.0 - 1.1, 0.5, 2.2],
                LayerKind::Solid([0.85, 0.95, 1.0, 0.85]),
                centre,
            );
            bar.additive = true;
            out.push(bar);
        }
    }

    let sweep = (clock * 0.22).fract();
    let mut sweep = Layer::new(
        60,
        9,
        Placement::Cells([
            sx,
            sy + sweep * sh,
            sw,
            0.7_f32.min(sh * 0.12),
        ]),
        LayerKind::Solid([0.35, 0.75, 1.0, 0.08]),
    );
    sweep.additive = true;
    out.push(sweep);

    out
}

#[cfg(feature = "glass")]
fn warming_layers(plate: Rect, centre: [f32; 2], t: f32) -> Vec<angel_glass::Layer> {
    use angel_glass::{Layer, LayerKind, Placement};
    let (px, py, pw, ph) = (
        f32::from(plate.x),
        f32::from(plate.y),
        f32::from(plate.width),
        f32::from(plate.height),
    );
    let mut out = Vec::with_capacity(8);
    out.push(Layer::new(
        0,
        0,
        Placement::Cells([px + 0.6, py + ph * 0.55, pw, ph * 0.5]),
        LayerKind::Solid([0.0, 0.0, 0.0, 0.18]),
    ));
    out.push(leaned(2, 1, [px, py, pw, ph], LayerKind::Solid(PLATE), centre));
    let arm_w = (pw * 0.14).min(3.6).max(1.2);
    let arm_h = (ph * 0.14).min(2.4).max(0.9);
    let mut id = 20u64;
    for corner in [
        [px - 0.30, py - 0.28],
        [px + pw + 0.30 - arm_w, py - 0.28],
        [px - 0.30, py + ph + 0.19 - arm_h],
        [px + pw + 0.30 - arm_w, py + ph + 0.19 - arm_h],
    ] {
        for arm in [
            [corner[0], corner[1], arm_w, 0.09],
            [corner[0], corner[1], 0.09, arm_h],
        ] {
            let mut bracket = leaned(id, 6, arm, LayerKind::Solid(CHROME), centre);
            bracket.flicker = 0.08;
            out.push(bracket);
            id += 1;
        }
    }
    let sweep = (t * 0.5).fract();
    let mut sweep = Layer::new(
        60,
        9,
        Placement::Cells([px + 0.5, py + sweep * ph, pw - 1.0, 0.8_f32.min(ph * 0.12)]),
        LayerKind::Solid([0.35, 0.75, 1.0, 0.08]),
    );
    sweep.additive = true;
    out.push(sweep);
    out
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/stereopticon__tests.rs"]
mod tests;
