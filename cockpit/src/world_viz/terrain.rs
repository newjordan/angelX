//! Shared road geometry for the retained ride's terrain sampler.

/// A half-tile-wide cross through the tile centre defines the carved road.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn on_road_band(wx: f32, wy: f32) -> bool {
    (wy.fract() - 0.5).abs() <= 0.25 || (wx.fract() - 0.5).abs() <= 0.25
}
