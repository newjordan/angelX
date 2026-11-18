pub(super) const Z_WIDE: f32 = 1.0;
pub(super) const Z_CLOSE: f32 = 4.0;

pub(super) const SETTLE_TICKS: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CameraMode {
    Auto,
    Wide,
    Close,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Camera {
    pub(super) center: (f32, f32),
    pub(super) center_target: (f32, f32),
    pub(super) zoom: f32,
    pub(super) zoom_target: f32,
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

    pub(super) fn look_at(&mut self, tile: (usize, usize)) {
        self.center = (tile.0 as f32, tile.1 as f32);
        self.center_target = self.center;
    }

    pub(super) fn easing(&self) -> bool {
        (self.zoom - self.zoom_target).abs() >= 0.001
            || (self.center.0 - self.center_target.0).abs() >= 0.001
            || (self.center.1 - self.center_target.1).abs() >= 0.001
    }

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
