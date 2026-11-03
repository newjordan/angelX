//! Thrown things in the air (`Lifted`): a hob's bomb, a Broodmarrow's
//! impling, a critter's glob. A window that draws them itself is told where
//! they are; a terminal's frame draws them in the picture.

use super::*;

/// Thrown things in the air on a seat's whole screen (`lifted` below the
/// band).
pub fn seat_lifted(run: &Run, w: i32, h: i32, focus: Option<u32>, alpha: f32) -> Vec<Lifted> {
    let band = band_rows(h);
    let mut out = lifted(run, w, view_rows(h), focus, alpha);
    for thing in &mut out {
        thing.y += band as f32;
    }
    out
}

/// Thrown things the window draws itself (crates/glass): bombs, implings
/// and globs in the air leave the picture (their landing rings stay) and are
/// reported by `lifted`, so the window can lift them toward the camera.
static LIFTED_OUTSIDE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Draw thrown things outside the picture (true) or in it (false, the
/// default: a terminal shows them in the frame).
pub fn draw_lifted_outside(on: bool) {
    LIFTED_OUTSIDE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn lifted_outside() -> bool {
    LIFTED_OUTSIDE.load(std::sync::atomic::Ordering::Relaxed)
}

/// What is in the air.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LiftedKind {
    /// A hob's bomb.
    Bomb,
    /// A Broodmarrow's impling.
    Impling,
    /// A critter's glob.
    Glob,
}

/// One thrown thing in the air, for a window that draws it: where its
/// shadow is (the point under it on the ground, in the frame's pixels), how
/// far along its flight is (0 thrown, 1 landing), and which picture of it
/// shows now (`lifted_sprite`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lifted {
    pub kind: LiftedKind,
    pub x: f32,
    pub y: f32,
    pub k: f32,
    pub variant: u8,
}

/// Everything in the air on a `w` × `h` frame, as `frame_px` places it.
/// `alpha` is how far the drawing is between this tick and the next, so a
/// flight moves at the frame rate, not the 30 Hz tick.
pub fn lifted(run: &Run, w: i32, h: i32, focus: Option<u32>, alpha: f32) -> Vec<Lifted> {
    use crate::rules::critters::GLOB_FALL;
    use crate::rules::foes::{FLING_FALL, LOB_FALL};
    let tick = run.tick as u32;
    let mut out = Vec::new();
    let mut add = |kind, from: (f32, f32), to: (f32, f32), fall: u32, total: u32, variant| {
        let k = (1.0 - (fall as f32 - alpha.clamp(0.0, 1.0)) / total as f32).clamp(0.0, 1.0);
        let gx = from.0 + (to.0 - from.0) * k;
        let gy = from.1 + (to.1 - from.1) * k;
        let (x, y) = frame_px(run, w, h, focus, gx, gy);
        out.push(Lifted {
            kind,
            x: x as f32,
            y: y as f32,
            k,
            variant,
        });
    };
    for lob in &run.lobs {
        let fuse = u8::from(!(tick / 2).is_multiple_of(2));
        add(LiftedKind::Bomb, lob.from, (lob.x, lob.y), lob.fall, LOB_FALL, fuse);
    }
    for fling in &run.flings {
        let flap = u8::from(!(tick / 3).is_multiple_of(2));
        add(LiftedKind::Impling, fling.from, (fling.x, fling.y), fling.fall, FLING_FALL, flap);
    }
    for glob in &run.critters.globs {
        add(LiftedKind::Glob, glob.from, (glob.x, glob.y), glob.fall, GLOB_FALL, 0);
    }
    out
}

/// A thrown thing's picture at native size (one native pixel per pixel),
/// as the frame would have drawn it.
pub fn lifted_sprite(kind: LiftedKind, variant: u8) -> Img {
    match kind {
        LiftedKind::Bomb => sprites::bomb(u32::from(variant) * 2),
        LiftedKind::Impling => sprites::flung_impling(u32::from(variant) * 3),
        LiftedKind::Glob => sprites::glob(),
    }
}
