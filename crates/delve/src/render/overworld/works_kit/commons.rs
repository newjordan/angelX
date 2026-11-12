//! The common things: the well.

use super::{Ctx, paint};
use crate::render::overworld::material::ramp;

pub(super) fn well(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 5) as f32, 7.5, 4.5, seed);
    // The ring of stone and its black water.
    paint::stone(im, 2, h - 9, 12, 7, seed);
    im.ellipse(8.0, (h - 9) as f32, 6.0, 2.5, 'h');
    im.ellipse(8.0, (h - 9) as f32, 4.5, 1.7, 'q');
    // Two posts, a bar with the windlass, a little roof.
    paint::post(im, 2, 6, h - 8);
    paint::post(im, 12, 6, h - 8);
    paint::lean_roof(im, 0, 1, 16, 6, ramp::TIMBER, seed);
    let drop = if work { ((paint::turn(t).cos() + 1.0) * 3.0).round() as i32 } else { 5 };
    im.line(3, 9, 12, 9, 'O');
    paint::rope(im, 8, 9, 8, 11 + drop, 0.0, 't');
    paint::bucket(im, 6, 11 + drop, 'z');
    if work {
        im.put(12 + (t % 2) as i32, 8 + (t % 2) as i32 * 2, 'U');
    }
    let _ = w;
}
