//! The debug layer of the shots: the guide lines the motion is drawn along,
//! laid over the picture so the paths can be judged before the effect is. A
//! comet's arc into the beacon, the Wyrm's neck and coil, the arc each part
//! takes when it breaks away, the spit's line from the mouth to each mark, the
//! sweep's path in from the walls and the roar's column are each a parametric
//! guide first (`finish.rs`, `wyrm.rs`), and the effect rides it. This layer
//! is off: only a test that asks for it (`set`) turns it on, and no frame of
//! the game draws it.
use super::cam::Cam;
use super::layout::{H, W};
use crate::render::overworld::ink::Img;
use crate::render::world3d::math::V3;
use crate::render::world3d::raster;
use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(false);

/// Turn the guide layer on or off (for the shots).
pub fn set(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// Whether the guide layer is drawn.
pub(super) fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// A dotted line along `pts` (screen dots), with a bigger dot at each end.
pub(super) fn dotted(im: &mut Img, pts: &[(f32, f32)], ink: char) {
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as i32).max(1);
        for i in 0..n {
            if i % 3 == 2 {
                continue;
            }
            let t = i as f32 / n as f32;
            im.put((a.0 + (b.0 - a.0) * t).round() as i32, (a.1 + (b.1 - a.1) * t).round() as i32, ink);
        }
    }
    if let (Some(&a), Some(&z)) = (pts.first(), pts.last()) {
        for (x, y) in [a, z] {
            im.rect(x.round() as i32 - 1, y.round() as i32 - 1, 3, 3, ink);
        }
    }
}

/// The guides of the Wyrm, over `im`, as `cam` sees them.
pub(super) fn paint(im: &mut Img, cam: &Cam, s: &super::Session) {
    for g in super::wyrm::guides(s) {
        let pts: Vec<(f32, f32)> = g
            .pts
            .iter()
            .filter_map(|&p| project(cam, p))
            .collect();
        dotted(im, &pts, g.ink);
    }
}

fn project(cam: &Cam, p: V3) -> Option<(f32, f32)> {
    let (x, y, d) = raster::project_point(&cam.view(), W as usize, H as usize, cam.point(p))?;
    (d > 0.5 && (-200.0..584.0).contains(&x) && (-200.0..424.0).contains(&y)).then_some((x, y))
}
