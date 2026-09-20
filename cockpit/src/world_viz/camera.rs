/// Cells per tile at the two camera altitudes: `Z_WIDE` is the travel view
/// (1 tile = 1 terminal cell), `Z_CLOSE` the micro camera (4 cells per tile).
/// The auto state machine eases `zoom` between them.
pub(super) const Z_WIDE: f32 = 1.0;
pub(super) const Z_CLOSE: f32 = 4.0;

/// Ticks the knight must sit arrived at a landmark before the auto camera
/// punches in — hysteresis, so rapid tool-target flapping keeps the wide view.
pub(super) const SETTLE_TICKS: u32 = 8;

/// How the camera altitude is chosen. `Auto` runs the settle state machine;
/// `Wide`/`Close` pin it (the `/world zoom` override).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CameraMode {
    Auto,
    Wide,
    Close,
}

/// A viewport onto the island — which tiles are visible and where the top-left
/// world tile sits, plus the altitude (`zoom`) the state machine eases it to.
/// The wide travel view is rendered avatar-relative and integer (byte identity
/// with the pre-camera renderer); `center`/`zoom` drive only the zoomed view.
#[derive(Clone, Copy, Debug)]
pub(super) struct Camera {
    /// Close-camera focus in fractional tile coords, eased toward the framed
    /// midpoint. The wide view ignores this and follows the integer avatar.
    pub(super) center: (f32, f32),
    /// Where `center` is easing to.
    pub(super) center_target: (f32, f32),
    /// Cells per tile, eased toward `zoom_target` (`Z_WIDE`..=`Z_CLOSE`).
    pub(super) zoom: f32,
    pub(super) zoom_target: f32,
    /// Auto state machine vs. a pinned override.
    pub(super) mode: CameraMode,
}

impl Camera {
    pub(super) const fn wide() -> Self {
        Camera {
            center: (0.0, 0.0),
            center_target: (0.0, 0.0),
            zoom: Z_WIDE,
            zoom_target: Z_WIDE,
            mode: CameraMode::Auto,
        }
    }

    /// Snap the camera focus to a tile (initial placement; no easing).
    pub(super) fn look_at(&mut self, tile: (usize, usize)) {
        self.center = (tile.0 as f32, tile.1 as f32);
        self.center_target = self.center;
    }

    /// True while any altitude/focus ease is still in flight. Keeps the event
    /// loop on the fast tick — and, critically, returns false once settled so
    /// the loop can idle (invariant 5).
    pub(super) fn easing(&self) -> bool {
        (self.zoom - self.zoom_target).abs() >= 0.001
            || (self.center.0 - self.center_target.0).abs() >= 0.001
            || (self.center.1 - self.center_target.1).abs() >= 0.001
    }

    /// Cycle the manual override Auto → Wide → Close → Auto; returns the new
    /// mode's label for the status title.
    pub(super) fn cycle_mode(&mut self) -> &'static str {
        self.mode = match self.mode {
            CameraMode::Auto => CameraMode::Wide,
            CameraMode::Wide => CameraMode::Close,
            CameraMode::Close => CameraMode::Auto,
        };
        match self.mode {
            CameraMode::Auto => "auto",
            CameraMode::Wide => "wide",
            CameraMode::Close => "close",
        }
    }
}
