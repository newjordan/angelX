//! The squire: a micro pixel knight who brings the showcase in. He walks
//! up to the folded panel, winds up and kicks its corner, and the panel
//! springs open from the blow; then he hops onto its roof and sits there,
//! legs over the edge, riding along wherever it is dragged. When it
//! closes he stands and walks it off.
//!
//! Motion, placement, and drawing support for showcase knights. Authored
//! pixels and clip timing arrive through the live showcase resource.

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

impl Frame {
    pub(crate) fn from_index(index: u8) -> Self {
        FRAMES.get(index as usize).copied().unwrap_or(Frame::Sit)
    }
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

/// Authored squire art, sitting clips and optional card-state tracks.
/// The showcase retains its entrance/exit machinery; card actor policy is
/// prepared from the live module and sampled against the existing card clock.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize, serde::Deserialize))]
pub struct ShowcaseSquireClip {
    pub palette: Vec<[u8; 3]>,
    pub frames: Vec<Vec<u8>>,
    pub idle: Vec<ClipStep>,
    pub wave: Vec<ClipStep>,
    /// Version-two module-owned card actor choreography. Legacy packs retain
    /// their authored idle/wave as a neutral card fallback.
    #[cfg_attr(feature = "remote", serde(default, skip_serializing_if = "Option::is_none"))]
    pub card: Option<crate::CardMotion>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize, serde::Deserialize))]
pub struct ClipStep {
    pub frame: u8,
    pub millis: u16,
}

/// Exact prepared resource identity. Revision numbers are scoped to a resource
/// and can repeat across providers or generations.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize, serde::Deserialize))]
pub struct ShowcaseClipIdentity {
    pub module: String,
    pub name: String,
    pub generation: u64,
    pub revision: u64,
    pub digest: String,
}

impl ShowcaseSquireClip {
    const MAX_PACK_BYTES: usize = 1024 * 1024;

    /// Parse a bounded squire resource from a caller-owned module pack.
    /// Glass validates the generic contract but chooses neither a pack path
    /// nor a default authored clip; callers install the prepared lease.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > Self::MAX_PACK_BYTES {
            return Err("squire clip exceeds 1024 KiB".into());
        }
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        Self::from_json(&value)
    }

    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        fn steps(value: &serde_json::Value, key: &str, max_total_ms: u32) -> Result<Vec<ClipStep>, String> {
            let values = value
                .get(key)
                .and_then(serde_json::Value::as_array)
                .filter(|values| !values.is_empty() && values.len() <= 64)
                .ok_or_else(|| format!("squire clip `{key}` must have 1..64 steps"))?;
            let steps: Vec<ClipStep> = values
                .iter()
                .map(|step| {
                    let frame = step
                        .get("frame")
                        .and_then(serde_json::Value::as_u64)
                        .filter(|&frame| frame < 9)
                        .ok_or_else(|| format!("squire clip `{key}` frame must be 0..8"))?
                        as u8;
                    let millis = step
                        .get("millis")
                        .and_then(serde_json::Value::as_u64)
                        .filter(|&millis| (40..=10_000).contains(&millis))
                        .ok_or_else(|| format!("squire clip `{key}` duration must be 40..10000ms"))?
                        as u16;
                    Ok(ClipStep { frame, millis })
                })
                .collect::<Result<_, String>>()?;
            if steps.iter().map(|step| u32::from(step.millis)).sum::<u32>() > max_total_ms {
                return Err(format!(
                    "squire clip `{key}` exceeds its total duration limit"
                ));
            }
            Ok(steps)
        }

        let palette_values = value
            .get("palette")
            .and_then(serde_json::Value::as_array)
            .filter(|palette| palette.len() == 12)
            .ok_or("squire clip palette must contain twelve RGB colours")?;
        let palette = palette_values
            .iter()
            .map(|colour| {
                let channels = colour
                    .as_array()
                    .filter(|channels| channels.len() == 3)
                    .ok_or("squire clip palette entries must be RGB")?;
                let mut rgb = [0; 3];
                for (out, channel) in rgb.iter_mut().zip(channels) {
                    *out = channel
                        .as_u64()
                        .filter(|&channel| channel <= 255)
                        .ok_or("squire clip RGB channels must be bytes")? as u8;
                }
                Ok(rgb)
            })
            .collect::<Result<Vec<[u8; 3]>, String>>()?;
        let frame_values = value
            .get("frames")
            .and_then(serde_json::Value::as_array)
            .filter(|frames| frames.len() == 9)
            .ok_or("squire clip must contain nine showcase poses")?;
        let frames = frame_values
            .iter()
            .map(|frame| {
                let pixels = frame
                    .get("pixels")
                    .and_then(serde_json::Value::as_array)
                    .filter(|pixels| pixels.len() == 320)
                    .ok_or("each squire pose must contain 320 indexed pixels")?;
                pixels
                    .iter()
                    .map(|pixel| {
                        pixel
                            .as_u64()
                            .filter(|&index| index < 12)
                            .map(|index| index as u8)
                            .ok_or_else(|| "squire pixel indices must be 0..11".to_string())
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect::<Result<Vec<_>, String>>()?;
        let idle = steps(value, "idle", 60_000)?;
        let wave = steps(value, "wave", 10_000)?;
        let card = value
            .get("card")
            .map(crate::CardMotion::from_json)
            .transpose()?;
        let clip = ShowcaseSquireClip {
            palette,
            frames,
            idle,
            wave,
            card,
        };
        #[cfg(feature = "remote")]
        if clip.card.is_some() && clip.wire_value()?.to_string().len() > 64 * 1024 {
            return Err("prepared squire card exceeds the 64 KiB remote publication budget".into());
        }
        Ok(clip)
    }

    #[cfg(feature = "remote")]
    pub(crate) fn wire_value(&self) -> Result<serde_json::Value, String> {
        let mut value = serde_json::json!({"palette":self.palette,"frames":self.frames.iter().map(|pixels|serde_json::json!({"pixels":pixels})).collect::<Vec<_>>(),"idle":self.idle,"wave":self.wave});
        if let Some(card) = &self.card {
            value["card"] = serde_json::to_value(card).map_err(|error| error.to_string())?;
        }
        Ok(value)
    }

    pub(crate) fn frame_at(steps: &[ClipStep], elapsed: f32, looped: bool) -> (u8, f32) {
        let total: u32 = steps.iter().map(|step| u32::from(step.millis)).sum();
        if total == 0 {
            return (0, 1.0);
        }
        let mut at = (elapsed.max(0.0) * 1000.0) as u32;
        if looped {
            at %= total;
        } else if at >= total {
            return (steps.last().map_or(0, |step| step.frame), 0.0);
        }
        for step in steps {
            let duration = u32::from(step.millis);
            if at < duration {
                return (step.frame, (duration - at) as f32 / 1000.0);
            }
            at -= duration;
        }
        (steps.last().map_or(0, |step| step.frame), 0.0)
    }

    pub(crate) fn picture(&self, livery: Option<&Livery>, id: u32) -> Picture {
        let (w, h) = (W * FRAMES.len(), H * 2);
        let mut rgba = vec![0u8; w * h * 4];
        for i in 0..FRAMES.len() {
            let Some(frame) = self.frames.get(i) else { continue };
            for y in 0..H {
                for x in 0..W {
                    let Some(index) = frame.get(y * W + x).copied().map(usize::from) else { continue };
                    if index == 0 { continue; }
                    let rgb = match (index, livery) {
                        (9, Some(l)) => l.field,
                        (10, Some(l)) => l.shadow,
                        (11, Some(l)) => l.charge,
                        _ => self.palette.get(index).copied().unwrap_or([0; 3]),
                    };
                    let pixel = [rgb[0], rgb[1], rgb[2], 255];
                    for (sy, sx) in [(y, i * W + x), (H + y, i * W + (W - 1 - x))] {
                        let offset = (sy * w + sx) * 4;
                        rgba[offset..offset + 4].copy_from_slice(&pixel);
                    }
                }
            }
        }
        Picture { id, width: w as u32, height: h as u32, rgba, cols: 0, rows: 0 }
    }
}

/// A house's colours, as its knights wear them: the field on plume and
/// tabard, the charge on the emblem (docs/CASTLES.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize, serde::Deserialize))]
pub struct Livery {
    pub field: [u8; 3],
    pub shadow: [u8; 3],
    pub charge: [u8; 3],
}

/// A transparent sheet keeps layout and animation geometry stable while no
/// authored clip is available. Pixel art itself comes from the live resource.
pub(crate) fn placeholder_sheet(id: u32) -> Picture {
    let (w, h) = (W * FRAMES.len(), H * 2);
    Picture {
        id,
        width: w as u32,
        height: h as u32,
        rgba: vec![0; w * h * 4],
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
    fn caller_supplied_json_uses_the_generic_validated_clip_contract() {
        let pack = serde_json::json!({
            "palette": vec![[0, 0, 0]; 12],
            "frames": (0..9).map(|_| serde_json::json!({"pixels": vec![1; 320]})).collect::<Vec<_>>(),
            "idle": [{"frame": 5, "millis": 4500}],
            "wave": [{"frame": 8, "millis": 1000}],
        });
        let bytes = serde_json::to_vec(&pack).unwrap();
        let clip = ShowcaseSquireClip::from_json_bytes(&bytes).expect("valid caller-supplied pack");
        assert_eq!(clip.frames.len(), 9);
        assert!(clip.frames.iter().flatten().all(|&pixel| pixel == 1));
        assert!(ShowcaseSquireClip::from_json_bytes(b"{}").is_err());
        assert!(ShowcaseSquireClip::from_json_bytes(&vec![b' '; ShowcaseSquireClip::MAX_PACK_BYTES + 1]).is_err());
    }

    #[test]
    fn missing_authored_clip_is_a_transparent_layout_preserving_sheet() {
        let sheet = placeholder_sheet(SHEET);
        assert_eq!(sheet.rgba.len() as u32, sheet.width * sheet.height * 4);
        assert_eq!((sheet.width, sheet.height), ((W * FRAMES.len()) as u32, (H * 2) as u32));
        assert!(sheet.rgba.iter().all(|&channel| channel == 0));
    }

    #[test]
    fn he_kicks_the_corner_and_ends_on_the_roof() {
        let rect = [400.0, 200.0, 600.0, 400.0];
        let px = 3.0;
        let kick = follow(IMPACT, rect, 300.0, px, 1.0, 0);
        assert_eq!(kick.frame, Frame::Kick);
        // The toe (the last column) is on the corner's x, his feet on its bottom.
        assert_eq!(kick.at[0] + (W as f32 - 1.0) * px, rect[0]);
        assert_eq!(kick.at[1] + H as f32 * px, rect[1] + rect[3]);
        let sat = follow(5.0, rect, 300.0, px, 1.0, 0);
        assert_eq!(sat.frame, Frame::Sit);
        assert_eq!(sat.at, seat(rect, 300.0, px));
        let start = follow(0.0, rect, 300.0, px, 1.0, 0);
        assert!(start.at[0] < kick.at[0], "he walks in from the left");
        // The hop lands where he sits.
        let landing = follow(INTRO_END - 1e-4, rect, 300.0, px, 1.0, 0);
        // The retinue lines up behind and never kicks.
        let behind = follow(IMPACT + 0.12, rect, 250.0, px, 1.0, 1);
        assert!(behind.at[0] < kick.at[0] && behind.frame != Frame::Kick);
        assert_eq!(follow(9.0, rect, 250.0, px, 1.0, 1).at, seat(rect, 250.0, px));
        assert!((landing.at[1] - sat.at[1]).abs() < 1.0);
    }
}
