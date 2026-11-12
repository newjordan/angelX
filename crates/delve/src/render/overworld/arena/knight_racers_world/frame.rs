//! The road's local frame at one distance: where a point `lat` metres left of
//! the centreline and `up` above the road lands in the world.
use super::route::Route;
use crate::render::world3d::math::{V3, v3};

#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub s: f32,
    pub x: f32,
    pub y: f32,
    pub sn: f32,
    pub cs: f32,
    pub h: f32,
    /// Road roll, as the height gained per metre to the right.
    pub tilt: f32,
    pub wl: f32,
    pub wr: f32,
}

/// The main line's frame at `s`.
pub fn row(r: &dyn Route, s: f32) -> Row { row_on(r, 0, s) }

/// The frame of road `line` (0 the main line, n the n-th fork) at main-line distance `s`.
pub fn row_on(r: &dyn Route, line: u8, s: f32) -> Row {
    let f = r.frame(line, s);
    let (sn, cs) = f.th.sin_cos();
    Row { s, x: f.x, y: f.y, sn, cs, h: f.h, tilt: f.bank.tan(), wl: f.wl, wr: f.wr }
}

impl Row {
    pub fn pt(&self, lat: f32, up: f32) -> V3 {
        v3(self.x - lat * self.sn, self.y + lat * self.cs, self.h + up - lat * self.tilt)
    }
    /// The half-width on the side `sd` (+1 left, -1 right).
    pub fn half(&self, sd: f32) -> f32 { if sd > 0. { self.wl } else { self.wr } }
}
