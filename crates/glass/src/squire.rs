//! The squire: a micro pixel knight who brings the showcase in. He walks
//! up to the folded panel, winds up and kicks its corner, and the panel
//! springs open from the blow; then he hops onto its roof and sits there,
//! legs over the edge, riding along wherever it is dragged. When it
//! closes he stands and walks it off.
//!
//! An original figure, twelve by sixteen pixels, drawn here as text and
//! shown at a whole multiple of its pixels so it stays crisp. Everything
//! he does is a function of time since the panel opened or closed, the
//! same way the window's motes are: nothing is kept.

use crate::graphics::Picture;
use crate::layer::{LAYER_PICTURES, Layer, LayerKind, Placement};

pub(crate) const SHEET: u32 = LAYER_PICTURES + 0xA4;
/// The retinue's sheets: a formation's other houses, one each.
pub(crate) const RETINUE_SHEETS: u32 = LAYER_PICTURES + 0xB0;
pub(crate) const MOST_RETINUE: usize = 3;

const W: usize = 16;
const H: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Frame {
    Stand,
    StepA,
    StepB,
    Windup,
    Kick,
    Sit,
    Hop,
    /// Sitting, shins kicked forward.
    SitSwing,
    /// Sitting, an arm up in greeting.
    SitWave,
}

const FRAMES: [Frame; 9] = [
    Frame::Stand,
    Frame::StepA,
    Frame::StepB,
    Frame::Windup,
    Frame::Kick,
    Frame::Sit,
    Frame::Hop,
    Frame::SitSwing,
    Frame::SitWave,
];

/// Facing right. `p` the plume and `g` the emblem (the one warm
/// accent), `o` outline, `h` `s` `m` `d` steel bright to dark, `v` the
/// visor's slits, `c` the tabard, `b` boots, `.` nothing.
const TOP: [&str; 15] = [
    "......ppp.......",
    "....ppppp.......",
    "...pp..p........",
    ".....oooooo.....",
    "....ohhsssso....",
    "....ohsssvvo....",
    "....ohssssso....",
    "....ohssvvvo....",
    ".....ommmmo.....",
    "....oommmmmoo...",
    "...ohhmcccmmmo..",
    "..ohhmccgccmmdo.",
    "..ohmmcgggcmmdo.",
    "..ohm.ccgcc.mdo.",
    "..oo..ccccc..oo.",
];

fn legs(frame: Frame) -> [&'static str; 5] {
    match frame {
        Frame::Stand => [
            ".....occccco....",
            ".....omm.mmo....",
            ".....omm.mmo....",
            ".....omm.mmo....",
            ".....obbbobbb...",
        ],
        Frame::StepA => [
            ".....occccco....",
            "....omm...mmo...",
            "...omm.....mmo..",
            "..omm.......mmo.",
            ".obbb.......obbb",
        ],
        Frame::StepB => [
            ".....occccco....",
            "......ommmo.....",
            "......ommmo.....",
            "......ommmo.....",
            "......obbbbb....",
        ],
        Frame::Windup => [
            ".....occccco....",
            "...mmo..omm.....",
            ".obb....omm.....",
            "........omm.....",
            "........obbbb...",
        ],
        Frame::Kick => [
            ".....occcccommmm",
            ".....omm....obbb",
            ".....omm........",
            ".....omm........",
            ".....obbbb......",
        ],
        Frame::SitSwing => [
            ".....occcccmmmmo",
            "..............mo",
            "..............ob",
            "................",
            "................",
        ],
        Frame::Sit | Frame::SitWave => [
            ".....occcccmmmo.",
            "............omo.",
            "............omo.",
            "............omo.",
            "............obbb",
        ],
        Frame::Hop => [
            ".....occccco....",
            "....omm...mmo...",
            "....obb...obb...",
            "................",
            "................",
        ],
    }
}

/// A house's colours, as its knights wear them: the field on plume and
/// tabard, the charge on the emblem (docs/CASTLES.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Livery {
    pub field: [u8; 3],
    pub shadow: [u8; 3],
    pub charge: [u8; 3],
}

/// The raised arm of [`Frame::SitWave`]: `(column, row, ink)` laid over
/// the figure, the resting arm cleared first.
const WAVE: [(usize, usize, u8); 22] = [
    (13, 11, b'.'), (14, 11, b'.'), (13, 12, b'.'), (14, 12, b'.'),
    (13, 13, b'.'), (14, 13, b'.'), (13, 14, b'.'), (14, 14, b'.'),
    (12, 5, b'o'), (13, 5, b'h'), (14, 5, b'o'),
    (12, 6, b'o'), (13, 6, b'm'), (14, 6, b'o'),
    (12, 7, b'o'), (13, 7, b'm'), (14, 7, b'o'),
    (12, 8, b'o'), (13, 8, b'm'), (14, 8, b'o'),
    (13, 9, b'm'), (14, 9, b'o'),
];

/// The figure for `frame`, row by row.
fn figure(frame: Frame) -> Vec<Vec<u8>> {
    let mut rows: Vec<Vec<u8>> = TOP.iter().copied().chain(legs(frame)).map(|r| r.as_bytes().to_vec()).collect();
    if frame == Frame::SitWave {
        for (x, y, c) in WAVE {
            rows[y][x] = c;
        }
    }
    rows
}

fn ink(c: u8, livery: Option<&Livery>) -> Option<[u8; 4]> {
    let rgba = |[r, g, b]: [u8; 3]| [r, g, b, 255];
    if let Some(l) = livery {
        match c {
            b'p' => return Some(rgba(l.field)),
            b'c' => return Some(rgba(l.shadow)),
            b'g' => return Some(rgba(l.charge)),
            _ => {}
        }
    }
    Some(match c {
        b'p' | b'g' => [196, 134, 92, 255],
        b'h' => [196, 202, 214, 255],
        b'o' => [16, 18, 24, 255],
        b's' => [158, 166, 180, 255],
        b'm' => [104, 112, 128, 255],
        b'd' => [64, 70, 84, 255],
        b'v' => [8, 8, 12, 255],
        b'c' => [52, 64, 104, 255],
        b'b' => [44, 38, 42, 255],
        _ => return None,
    })
}

/// Every frame in a row, facing right; below, the same row turned to
/// face left. In `livery` when a house serves; the Keep's own colours
/// (indigo, the warm accent) when none does.
pub(crate) fn sheet(livery: Option<&Livery>, id: u32) -> Picture {
    let (w, h) = (W * FRAMES.len(), H * 2);
    let mut rgba = vec![0u8; w * h * 4];
    for (i, &frame) in FRAMES.iter().enumerate() {
        for (y, row) in figure(frame).iter().enumerate() {
            for (x, &c) in row.iter().enumerate().take(W) {
                let Some(px) = ink(c, livery) else { continue };
                for (sy, sx) in [(y, i * W + x), (H + y, i * W + (W - 1 - x))] {
                    let o = (sy * w + sx) * 4;
                    rgba[o..o + 4].copy_from_slice(&px);
                }
            }
        }
    }
    Picture {
        id,
        width: w as u32,
        height: h as u32,
        rgba,
        cols: 0,
        rows: 0,
    }
}

/// Where he is and what he is doing: his top-left in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pose {
    pub at: [f32; 2],
    pub frame: Frame,
    pub left: bool,
}

/// The entrance, in seconds since the panel opened: walking in, the
/// wind-up, the kick (the panel pops at [`IMPACT`]), the hop to the roof.
pub(crate) const WALK_END: f32 = 0.42;
const WINDUP_END: f32 = 0.52;
pub(crate) const IMPACT: f32 = 0.55;
const KICK_END: f32 = 0.66;
pub(crate) const INTRO_END: f32 = 0.98;

/// His size on screen: a whole number of window pixels per sprite pixel.
pub(crate) fn pixel(scale: f32) -> f32 {
    (3.0 * scale).round().max(2.0)
}

pub(crate) fn size(px: f32) -> [f32; 2] {
    [W as f32 * px, H as f32 * px]
}

/// Where he kicks from: his toe (the last column) on the panel's
/// bottom-left corner.
fn kick_spot(rect: [f32; 4], px: f32) -> [f32; 2] {
    let [x, y, _, h] = rect;
    [x - (W as f32 - 1.0) * px, y + h - H as f32 * px]
}

/// His seat on the panel's roof, `seat_x` from its left: the seat row
/// (the hips, 15) on the top edge, legs over the front.
pub(crate) fn seat(rect: [f32; 4], seat_x: f32, px: f32) -> [f32; 2] {
    [rect[0] + seat_x, rect[1] - 16.0 * px]
}

pub(crate) fn entrance(t: f32, rect: [f32; 4], seat_x: f32, px: f32, scale: f32) -> Pose {
    follow(t, rect, seat_x, px, scale, 0)
}

/// A squire of the retinue, `place` places behind the lead (0 is the
/// lead himself): he sets off a little later, lines up behind, and where
/// the lead kicks he jumps for joy; then he hops up to his own seat.
pub(crate) fn follow(t: f32, rect: [f32; 4], seat_x: f32, px: f32, scale: f32, place: usize) -> Pose {
    let t = t - 0.12 * place as f32;
    let mut kick = kick_spot(rect, px);
    kick[0] -= place as f32 * (W as f32 - 3.0) * px;
    if place > 0 && (WALK_END..KICK_END).contains(&t) {
        let cheer = ((t - WALK_END) / 0.1) as u32 % 2 == 1;
        let lift = if cheer { 3.0 * px } else { 0.0 };
        return Pose {
            at: [kick[0], kick[1] - lift],
            frame: if cheer { Frame::Hop } else { Frame::Stand },
            left: false,
        };
    }
    if t < WALK_END {
        let k = t / WALK_END;
        let from = (kick[0] - 120.0 * scale).max(-(W as f32) * px);
        let k = k.max(0.0);
        let step = if ((t / 0.07) as u32) % 2 == 0 { Frame::StepA } else { Frame::StepB };
        return Pose {
            at: [from + (kick[0] - from) * k, kick[1]],
            frame: step,
            left: false,
        };
    }
    if t < WINDUP_END {
        return Pose { at: kick, frame: Frame::Windup, left: false };
    }
    if t < KICK_END {
        return Pose { at: kick, frame: Frame::Kick, left: false };
    }
    let roof = seat(rect, seat_x, px);
    if t < INTRO_END {
        // An arc up to the roof, higher than the roof by a little.
        let k = (t - KICK_END) / (INTRO_END - KICK_END);
        let peak = roof[1].min(kick[1]) - 40.0 * scale;
        let x = kick[0] + (roof[0] - kick[0]) * k;
        // A parabola through kick (k=0), peak and roof (k=1).
        let a = kick[1];
        let c = roof[1];
        let y = (1.0 - k) * (1.0 - k) * a + 2.0 * (1.0 - k) * k * (2.0 * peak - (a + c) / 2.0) + k * k * c;
        return Pose { at: [x, y], frame: Frame::Hop, left: roof[0] < kick[0] };
    }
    Pose { at: roof, frame: Frame::Sit, left: false }
}

/// The exit, in seconds since the panel began to close, over `rect` as
/// it slides: up on his feet on the roof, walking it away.
pub(crate) fn exit(t: f32, rect: [f32; 4], seat_x: f32, px: f32) -> Pose {
    let [x, y, _, _] = rect;
    let frame = if t < 0.06 {
        Frame::Stand
    } else if ((t / 0.07) as u32) % 2 == 0 {
        Frame::StepA
    } else {
        Frame::StepB
    };
    Pose {
        at: [x + seat_x, y - H as f32 * px],
        frame,
        left: false,
    }
}

/// On his feet on the roof, `seat_x` from its left.
pub(crate) fn roof_stand(rect: [f32; 4], seat_x: f32, px: f32) -> [f32; 2] {
    [rect[0] + seat_x, rect[1] - H as f32 * px]
}

/// How long a wave lasts, and each arm's swing in it.
pub(crate) const WAVE_SECONDS: f32 = 1.0;
const WAVE_BEAT: f32 = 0.16;
/// Now and then he swings his legs: a few kicks every so often.
const SWING_EVERY: f32 = 4.5;
const SWING_BEAT: f32 = 0.22;
const SWINGS: f32 = 4.0;

/// Sitting, `t` seconds after he sat down, `waved` seconds after a
/// greeting (if one is going): the frame, and how long until it may
/// change. Nothing changes between swings, so the window sleeps.
pub(crate) fn idle(t: f32, waved: Option<f32>) -> (Frame, f32) {
    if let Some(w) = waved.filter(|w| (0.0..WAVE_SECONDS).contains(w)) {
        let beat = (w / WAVE_BEAT) as u32;
        let frame = if beat % 2 == 0 { Frame::SitWave } else { Frame::Sit };
        return (frame, WAVE_BEAT - w % WAVE_BEAT);
    }
    let into = t.max(0.0) % SWING_EVERY;
    let start = SWING_EVERY - SWINGS * SWING_BEAT;
    if into < start {
        return (Frame::Sit, start - into);
    }
    let beat = ((into - start) / SWING_BEAT) as u32;
    let frame = if beat % 2 == 0 { Frame::SitSwing } else { Frame::Sit };
    (frame, SWING_BEAT - (into - start) % SWING_BEAT)
}

pub(crate) fn layer(id: u64, sheet: u32, pose: Pose, px: f32, opacity: f32) -> Layer {
    let index = FRAMES.iter().position(|&f| f == pose.frame).unwrap_or(0) as f32;
    let row = if pose.left { H as f32 } else { 0.0 };
    let [w, h] = size(px);
    let mut layer = Layer::new(
        id,
        0,
        Placement::Pixels([pose.at[0].round(), pose.at[1].round(), w, h]),
        LayerKind::PicturePart(sheet, [index * W as f32, row, W as f32, H as f32]),
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
                assert_eq!(row.len(), W, "{frame:?}");
                assert!(row.iter().all(|&c| c == b'.' || ink(c, None).is_some()), "{frame:?}");
            }
        }
        let sheet = sheet(None, SHEET);
        assert_eq!(sheet.rgba.len() as u32, sheet.width * sheet.height * 4);
    }

    #[test]
    fn he_swings_his_legs_now_and_then_and_waves_when_asked() {
        let (frame, wait) = idle(0.5, None);
        assert_eq!(frame, Frame::Sit);
        assert!(wait > 1.0, "between swings the window can sleep");
        let swinging = (0..40).map(|i| idle(i as f32 * 0.05 + 3.6, None).0);
        assert!(swinging.clone().any(|f| f == Frame::SitSwing));
        let waving = (0..6).map(|i| idle(10.0, Some(i as f32 * 0.16 + 0.01)).0);
        assert!(waving.clone().any(|f| f == Frame::SitWave));
        assert_eq!(idle(0.5, Some(WAVE_SECONDS + 0.1)).0, Frame::Sit, "the wave ends");
    }

    #[test]
    fn a_house_dresses_him_in_its_colours() {
        let livery = Livery { field: [10, 20, 30], shadow: [5, 10, 15], charge: [200, 190, 100] };
        let plain = sheet(None, SHEET);
        let dressed = sheet(Some(&livery), SHEET);
        assert_ne!(plain.rgba, dressed.rgba);
        let has = |rgba: &[u8], c: [u8; 3]| rgba.chunks(4).any(|p| p[..3] == c && p[3] == 255);
        assert!(has(&dressed.rgba, livery.field) && has(&dressed.rgba, livery.charge));
        assert!(has(&dressed.rgba, livery.shadow));
    }

    #[test]
    fn the_left_row_is_the_right_row_turned() {
        let sheet = sheet(None, SHEET);
        let w = sheet.width as usize;
        let at = |x: usize, y: usize| &sheet.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
        for y in 0..H {
            for x in 0..W {
                assert_eq!(at(x, y), at(W - 1 - x, H + y));
            }
        }
    }

    #[test]
    fn he_kicks_the_corner_and_ends_on_the_roof() {
        let rect = [400.0, 200.0, 600.0, 400.0];
        let px = 3.0;
        let kick = entrance(IMPACT, rect, 300.0, px, 1.0);
        assert_eq!(kick.frame, Frame::Kick);
        // The toe (the last column) is on the corner's x, his feet on its bottom.
        assert_eq!(kick.at[0] + (W as f32 - 1.0) * px, rect[0]);
        assert_eq!(kick.at[1] + H as f32 * px, rect[1] + rect[3]);
        let sat = entrance(5.0, rect, 300.0, px, 1.0);
        assert_eq!(sat.frame, Frame::Sit);
        assert_eq!(sat.at, seat(rect, 300.0, px));
        let start = entrance(0.0, rect, 300.0, px, 1.0);
        assert!(start.at[0] < kick.at[0], "he walks in from the left");
        // The hop lands where he sits.
        let landing = entrance(INTRO_END - 1e-4, rect, 300.0, px, 1.0);
        // The retinue lines up behind and never kicks.
        let behind = follow(IMPACT + 0.12, rect, 250.0, px, 1.0, 1);
        assert!(behind.at[0] < kick.at[0] && behind.frame != Frame::Kick);
        assert_eq!(follow(9.0, rect, 250.0, px, 1.0, 1).at, seat(rect, 250.0, px));
        assert!((landing.at[1] - sat.at[1]).abs() < 1.0);
    }
}
