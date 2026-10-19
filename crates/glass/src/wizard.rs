//! The wizard: the showcase's herald for charts. The pixel cousin of the
//! dotted wizard who walks the intro's ascent and the research plot's
//! frontier: hooked hat, grey beard, travelling cloak, a staff with a warm
//! stone. He walks up beside the folded panel, raises his staff, and a
//! spark crosses to it; the panel unfolds from its middle and the chart
//! is swept in under a drift of motes while he holds the spell. Then he
//! leans on his staff at the panel's side. When it closes he raises the
//! staff again and it folds away into a point.
//!
//! An original figure, sixteen by twenty-four pixels, drawn as text like
//! the squire and shown at whole pixels; every pose is a function of time.

use crate::graphics::Picture;
use crate::layer::{LAYER_PICTURES, Layer, LayerKind, Particles, Placement};

pub(crate) const SHEET: u32 = LAYER_PICTURES + 0xA5;

const W: usize = 16;
const H: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Frame {
    Stand,
    StepA,
    StepB,
    Cast,
}

const FRAMES: [Frame; 4] = [Frame::Stand, Frame::StepA, Frame::StepB, Frame::Cast];

/// Facing right. `h` the hat, `r` its band and `g` the staff's stone (the
/// one warm accent), `f` face and hand, `w` beard, `k` `l` the cloak and
/// its lit fold, `t` the staff, `o` outline, `b` boots, `.` nothing.
const BODY: [&str; 22] = [
    "..oo............",
    ".ohho......ogo..",
    "..ohho.....ogo..",
    "...ohhoo....t...",
    "....ohhho...t...",
    "....ohhhho..t...",
    "...ohhhhhho.t...",
    ".oorrrrrrrrot...",
    "oohhhhhhhhhot...",
    ".....offfo..t...",
    ".....offofo.t...",
    "....owwwwo.ft...",
    "...okwwwwwkkt...",
    "..okkwwwwkkkt...",
    "..oklkwwwkkkt...",
    "..oklkkwkkkkt...",
    "..oklkkkkkkkt...",
    ".oklkkkkkkkkt...",
    ".oklkkkkkkkkt...",
    ".oklkkkkkkkkt...",
    "oklkkkkkkkkkt...",
    "okkkkkkkkkkot...",
];

fn feet(frame: Frame) -> [&'static str; 2] {
    match frame {
        Frame::StepA => ["...obb...ob.t...", "..obb....obbt..."],
        Frame::StepB => [".oo..obbob..t...", ".....obbob..t..."],
        Frame::Stand | Frame::Cast => [".oo.obb.ob..t...", "....obb.obb.t..."],
    }
}

/// The staff raised toward what he faces: `(column, row, ink)`, the
/// upright staff cleared first.
const RAISED: [(usize, usize, u8); 12] = [
    (12, 10, b't'),
    (12, 9, b't'),
    (13, 8, b't'),
    (13, 7, b't'),
    (14, 6, b't'),
    (14, 5, b't'),
    (15, 4, b't'),
    (14, 2, b'o'),
    (15, 1, b'o'),
    (15, 2, b'g'),
    (15, 3, b'g'),
    (14, 3, b'o'),
];

fn figure(frame: Frame) -> Vec<Vec<u8>> {
    let mut rows: Vec<Vec<u8>> = BODY
        .iter()
        .copied()
        .chain(feet(frame))
        .map(|r| r.as_bytes().to_vec())
        .collect();
    if frame == Frame::Cast {
        for row in rows.iter_mut() {
            for c in row[11..].iter_mut() {
                if matches!(*c, b't' | b'g') {
                    *c = b'.';
                }
            }
        }
        // The stone's setting goes with the staff.
        for y in 1..3 {
            for x in 11..14 {
                rows[y][x] = b'.';
            }
        }
        for (x, y, c) in RAISED {
            rows[y][x] = c;
        }
    }
    rows
}

fn ink(c: u8) -> Option<[u8; 4]> {
    Some(match c {
        b'o' => [16, 18, 24, 255],
        b'h' => [44, 50, 92, 255],
        b'r' | b'g' => [196, 134, 92, 255],
        b'f' => [176, 150, 128, 255],
        b'w' => [190, 192, 198, 255],
        b'k' => [58, 62, 92, 255],
        b'l' => [84, 90, 128, 255],
        b't' => [110, 82, 58, 255],
        b'b' => [44, 38, 42, 255],
        _ => return None,
    })
}

/// Every frame facing right; below, the same row facing left.
pub(crate) fn sheet() -> Picture {
    let (w, h) = (W * FRAMES.len(), H * 2);
    let mut rgba = vec![0u8; w * h * 4];
    for (i, &frame) in FRAMES.iter().enumerate() {
        for (y, row) in figure(frame).iter().enumerate() {
            for (x, &c) in row.iter().enumerate().take(W) {
                let Some(px) = ink(c) else { continue };
                for (sy, sx) in [(y, i * W + x), (H + y, i * W + (W - 1 - x))] {
                    let o = (sy * w + sx) * 4;
                    rgba[o..o + 4].copy_from_slice(&px);
                }
            }
        }
    }
    Picture {
        id: SHEET,
        width: w as u32,
        height: h as u32,
        rgba,
        cols: 0,
        rows: 0,
    }
}

/// His entrance, in seconds since the panel opened: the walk, the raised
/// staff, the spark's flight, the panel unfolding at [`IMPACT`], and the
/// chart swept in over [`SWEEP`] while he holds the spell.
const WALK_END: f32 = 0.5;
const CAST_AT: f32 = 0.55;
pub(crate) const IMPACT: f32 = 0.8;
pub(crate) const SWEEP: f32 = 0.9;
pub(crate) const INTRO_END: f32 = IMPACT + SWEEP;

pub(crate) fn pixel(scale: f32) -> f32 {
    (3.0 * scale).round().max(2.0)
}

pub(crate) fn size(px: f32) -> [f32; 2] {
    [W as f32 * px, H as f32 * px]
}

/// Which side of the panel he keeps: the right, unless the window ends
/// there.
fn on_left(rect: [f32; 4], window: (f32, f32), px: f32) -> bool {
    rect[0] + rect[2] + size(px)[0] + 8.0 > window.0
}

/// Where he stands beside the panel, feet on its bottom line, and whether
/// he faces right (standing on its left).
fn post(rect: [f32; 4], window: (f32, f32), px: f32, scale: f32) -> ([f32; 2], bool) {
    let [x, y, w, h] = rect;
    let [sw, sh] = size(px);
    if on_left(rect, window, px) {
        ([(x - sw - 6.0 * scale).max(0.0), y + h - sh], true)
    } else {
        ([x + w + 6.0 * scale, y + h - sh], false)
    }
}

/// The stone at the staff's tip in a pose at `at`.
fn stone(at: [f32; 2], frame: Frame, faces_right: bool, px: f32) -> [f32; 2] {
    let (col, row) = if frame == Frame::Cast { (15.5, 2.5) } else { (12.5, 1.5) };
    let col = if faces_right { col } else { W as f32 - col };
    [at[0] + col * px, at[1] + row * px]
}

/// His entrance's layers at `t`: himself, and the spark in flight.
pub(crate) fn entrance(
    t: f32,
    rect: [f32; 4],
    window: (f32, f32),
    px: f32,
    scale: f32,
    first_id: u64,
    pointing: Option<[f32; 2]>,
    casting: Option<f32>,
) -> Vec<Layer> {
    let (spot, faces_right) = post(rect, window, px, scale);
    let mut layers = Vec::new();
    let (at, frame) = if t < WALK_END {
        // In from the window's edge on his side.
        let k = t / WALK_END;
        let from = if faces_right { -size(px)[0] } else { window.0 };
        let from = if faces_right { from.max(spot[0] - 160.0 * scale) } else { from.min(spot[0] + 160.0 * scale) };
        let step = if ((t / 0.08) as u32) % 2 == 0 { Frame::StepA } else { Frame::StepB };
        ([from + (spot[0] - from) * k, spot[1]], step)
    } else if (CAST_AT..INTRO_END).contains(&t) || pointing.is_some() || casting.is_some() {
        (spot, Frame::Cast)
    } else {
        (spot, Frame::Stand)
    };
    layers.push(figure_layer(first_id, at, frame, faces_right, px, (t / 0.1).min(1.0)));
    // The spark: from the stone to the panel's middle.
    if (CAST_AT..IMPACT).contains(&t) {
        let k = (t - CAST_AT) / (IMPACT - CAST_AT);
        let from = stone(at, Frame::Cast, faces_right, px);
        let to = [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0];
        let lift = -60.0 * scale * (4.0 * k * (1.0 - k));
        let p = [from[0] + (to[0] - from[0]) * k, from[1] + (to[1] - from[1]) * k + lift];
        layers.push(glow(first_id + 1, p, 26.0 * scale, 0.9));
    }
    // The stone lit while he casts.
    if frame == Frame::Cast {
        let p = stone(at, frame, faces_right, px);
        layers.push(glow(first_id + 2, p, 14.0 * scale, 0.55));
    }
    // Idle magic: a puff of motes off the stone.
    if let Some(age) = casting.filter(|age| (0.0..PUFF_LIFE).contains(age)) {
        layers.push(puff(first_id + 4, stone(at, Frame::Cast, faces_right, px), age, scale));
    }
    // Pointing his staff at a mark of the chart: a light where it falls.
    if let Some(p) = pointing.filter(|_| t >= INTRO_END) {
        layers.push(glow(first_id + 3, p, 22.0 * scale, 0.7));
    }
    layers
}

/// His exit at `t` of `close`: the staff raised, the panel folding into
/// its middle, and a burst of motes and a flash where it goes.
pub(crate) fn exit(t: f32, close: f32, rect: [f32; 4], window: (f32, f32), px: f32, scale: f32, id: u64) -> Vec<Layer> {
    let (at, faces_right) = post(rect, window, px, scale);
    let fade = 1.0 - ((t / close - 0.5) / 0.5).clamp(0.0, 1.0);
    let mut layers = vec![figure_layer(id, at, Frame::Cast, faces_right, px, fade)];
    layers.push(glow(id + 1, stone(at, Frame::Cast, faces_right, px), 14.0 * scale, 0.55 * fade));
    let centre = [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0];
    let gone = (t / close - 0.55).max(0.0) / 0.45;
    if gone > 0.0 {
        layers.push(glow(id + 2, centre, 60.0 * scale, 0.8 * (1.0 - gone)));
        let mut burst = Layer::new(
            id + 3,
            0,
            Placement::Pixels([centre[0] - 6.0 * scale, centre[1] - 6.0 * scale, 12.0 * scale, 12.0 * scale]),
            LayerKind::Particles(Particles {
                color: [0.93, 0.78, 0.58, 0.95],
                count: 48,
                size: (2.0 * scale).round().max(2.0),
                life: close * 0.45,
                rise: 320.0 * scale,
                sway: 160.0 * scale,
                seed: 53,
                burst: Some(t - close * 0.55),
            }),
        );
        burst.opacity = 1.0;
        layers.push(burst);
    }
    layers
}

/// Idle, he lifts his staff now and then and a puff of motes rises off
/// the stone. `t` seconds after his entrance: how far into a lift he is
/// (if he is), and how long until the window must draw him again.
const IDLE_EVERY: f32 = 6.0;
const LIFT: f32 = 0.9;
const PUFF_LIFE: f32 = 0.8;

pub(crate) fn idle(t: f32) -> (Option<f32>, f32) {
    let into = t.max(0.0) % IDLE_EVERY;
    let start = IDLE_EVERY - LIFT;
    if into < start {
        (None, start - into)
    } else {
        // While the puff drifts, every display frame.
        (Some(into - start), 1.0 / 60.0)
    }
}

fn puff(id: u64, at: [f32; 2], age: f32, scale: f32) -> Layer {
    let mut layer = Layer::new(
        id,
        0,
        Placement::Pixels([at[0] - 4.0 * scale, at[1] - 4.0 * scale, 8.0 * scale, 8.0 * scale]),
        LayerKind::Particles(Particles {
            color: [0.93, 0.80, 0.62, 0.9],
            count: 14,
            size: (2.0 * scale).round().max(2.0),
            life: PUFF_LIFE,
            rise: 60.0 * scale,
            sway: -90.0 * scale,
            seed: 19,
            burst: Some(age),
        }),
    );
    layer.opacity = 1.0;
    layer
}

/// The motes along the sweep's leading edge, `edge` a column of the
/// content in window pixels from `top` to `bottom`.
pub(crate) fn sweep_motes(id: u64, edge: f32, top: f32, bottom: f32, scale: f32) -> Layer {
    let mut layer = Layer::new(
        id,
        0,
        Placement::Pixels([edge - 3.0 * scale, top, 6.0 * scale, (bottom - top).max(1.0)]),
        LayerKind::Particles(Particles {
            color: [0.93, 0.80, 0.62, 0.9],
            count: 40,
            size: (2.0 * scale).round().max(2.0),
            life: 0.5,
            rise: 18.0 * scale,
            sway: 5.0 * scale,
            seed: 31,
            burst: None,
        }),
    );
    layer.opacity = 1.0;
    layer
}

fn glow(id: u64, at: [f32; 2], radius: f32, strength: f32) -> Layer {
    let mut layer = Layer::new(
        id,
        0,
        Placement::Pixels([at[0] - radius, at[1] - radius, 2.0 * radius, 2.0 * radius]),
        LayerKind::Glow([0.86, 0.60, 0.40, 0.8]),
    );
    layer.additive = true;
    layer.opacity = strength;
    layer
}

fn figure_layer(id: u64, at: [f32; 2], frame: Frame, faces_right: bool, px: f32, opacity: f32) -> Layer {
    let index = FRAMES.iter().position(|&f| f == frame).unwrap_or(0) as f32;
    let row = if faces_right { 0.0 } else { H as f32 };
    let [w, h] = size(px);
    let mut layer = Layer::new(
        id,
        0,
        Placement::Pixels([at[0].round(), at[1].round(), w, h]),
        LayerKind::PicturePart(SHEET, [index * W as f32, row, W as f32, H as f32]),
    );
    layer.opacity = opacity;
    layer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_frame_is_drawn_to_size() {
        for &frame in &FRAMES {
            let rows = figure(frame);
            assert_eq!(rows.len(), H, "{frame:?}");
            for row in rows {
                assert_eq!(row.len(), W);
                assert!(row.iter().all(|&c| c == b'.' || ink(c).is_some()), "{frame:?}");
            }
        }
        let sheet = sheet();
        assert_eq!(sheet.rgba.len() as u32, sheet.width * sheet.height * 4);
    }

    #[test]
    fn he_stands_beside_the_panel_and_keeps_to_the_window() {
        let window = (1600.0, 900.0);
        let px = 3.0;
        let rect = [400.0, 200.0, 600.0, 400.0];
        let (at, faces_right) = post(rect, window, px, 1.0);
        assert!(!faces_right && at[0] >= rect[0] + rect[2], "right of the panel, facing it");
        assert_eq!(at[1] + size(px)[1], rect[1] + rect[3], "feet on its bottom line");
        let flush = [1000.0, 200.0, 600.0, 400.0];
        let (at, faces_right) = post(flush, window, px, 1.0);
        assert!(faces_right && at[0] + size(px)[0] <= flush[0], "the window ends there: he takes the left");
    }

    #[test]
    fn the_spark_flies_before_the_panel_opens() {
        let rect = [400.0, 200.0, 600.0, 400.0];
        let flying = entrance(0.7, rect, (1600.0, 900.0), 3.0, 1.0, 0, None, None);
        assert!(flying.iter().any(|l| matches!(l.kind, LayerKind::Glow(_))));
        let settled = entrance(5.0, rect, (1600.0, 900.0), 3.0, 1.0, 0, None, None);
        assert_eq!(settled.len(), 1, "after the spell, only himself");
        let pointing = entrance(5.0, rect, (1600.0, 900.0), 3.0, 1.0, 0, Some([700.0, 300.0]), None);
        assert!(pointing.len() > 1, "pointing, the staff is lit and so is the mark");
        let puffing = entrance(5.0, rect, (1600.0, 900.0), 3.0, 1.0, 0, None, Some(0.2));
        assert!(puffing.iter().any(|l| matches!(l.kind, LayerKind::Particles(_))));
    }

    #[test]
    fn idle_he_rests_most_of_the_time_and_lifts_his_staff_now_and_then() {
        let (lift, wait) = idle(1.0);
        assert!(lift.is_none() && wait > 3.0, "the window sleeps between lifts");
        let (lift, wait) = idle(IDLE_EVERY - 0.5);
        assert!(lift.is_some() && wait < 0.05);
    }
}
