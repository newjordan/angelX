//! Still-only display transform. Zoom is relative to Fit, not a claimed native 1:1.
//! Integer keys keep worker admission/cache equality deterministic; floating point
//! is confined to finite, bounded transform arithmetic, never supplied by input.
use ratatui::layout::Rect;

const UNIT: i64 = 1_000_000;
const FIT: u32 = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct View {
    pub zoom: u32,
    pub x: i64,
    pub y: i64,
}
impl Default for View {
    fn default() -> Self {
        Self {
            zoom: FIT,
            x: UNIT / 2,
            y: UNIT / 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Action {
    ZoomIn,
    ZoomOut,
    Left,
    Right,
    Up,
    Down,
    Fit,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Transform {
    pub scale: f64,
    pub center: (f64, f64),
    pub pixels: (u32, u32),
    pub source: (u32, u32),
}
impl View {
    pub fn transform(self, source: (u32, u32), pixels: (u32, u32)) -> Transform {
        let (w, h) = (source.0.max(1) as f64, source.1.max(1) as f64);
        let scale = (pixels.0.max(1) as f64 / w).min(pixels.1.max(1) as f64 / h)
            * self.zoom.clamp(FIT, FIT * 64) as f64
            / FIT as f64;
        Transform {
            scale,
            center: (
                self.x as f64 / UNIT as f64 * w,
                self.y as f64 / UNIT as f64 * h,
            ),
            pixels,
            source,
        }
    }
    fn clamp(&mut self, source: (u32, u32), pixels: (u32, u32)) {
        self.zoom = self.zoom.clamp(FIT, FIT * 64);
        let t = self.transform(source, pixels);
        for (center, size, screen) in [
            (&mut self.x, source.0, pixels.0),
            (&mut self.y, source.1, pixels.1),
        ] {
            let half = (screen as f64 / t.scale / size.max(1) as f64 / 2.0).min(0.5);
            let lo = (half * UNIT as f64).ceil() as i64;
            *center = (*center).clamp(lo, UNIT - lo);
        }
    }
    pub fn zoom_at(
        &mut self,
        up: bool,
        pointer: (f64, f64),
        source: (u32, u32),
        pixels: (u32, u32),
    ) {
        let before = self.transform(source, pixels);
        let anchor = before.source_at(pointer);
        self.zoom = if up {
            (self.zoom * 5 / 4).min(FIT * 64)
        } else {
            (self.zoom * 4 / 5).max(FIT)
        };
        let after = self.transform(source, pixels);
        self.x = ((anchor.0 - (pointer.0 - pixels.0 as f64 / 2.0) / after.scale)
            / source.0.max(1) as f64
            * UNIT as f64)
            .round() as i64;
        self.y = ((anchor.1 - (pointer.1 - pixels.1 as f64 / 2.0) / after.scale)
            / source.1.max(1) as f64
            * UNIT as f64)
            .round() as i64;
        self.clamp(source, pixels);
    }
    /// Positive motion moves the image with the hand (camera center goes left/up).
    pub fn pan(&mut self, dx: f64, dy: f64, source: (u32, u32), pixels: (u32, u32)) {
        let t = self.transform(source, pixels);
        self.x -= (dx / t.scale / source.0.max(1) as f64 * UNIT as f64).round() as i64;
        self.y -= (dy / t.scale / source.1.max(1) as f64 * UNIT as f64).round() as i64;
        self.clamp(source, pixels);
    }
}
impl Transform {
    pub fn source_at(self, point: (f64, f64)) -> (f64, f64) {
        (
            self.center.0 + (point.0 - self.pixels.0 as f64 / 2.0) / self.scale,
            self.center.1 + (point.1 - self.pixels.1 as f64 / 2.0) / self.scale,
        )
    }
    /// Borrow the source; allocate only the RGBA viewport (<=4MP at admission).
    /// Downscale integrates each source pixel's area coverage, preserving thin
    /// strokes without aliasing. Magnification uses fractional bilinear samples.
    /// No crop copy, weight table, or source-width × output-height float image:
    /// scratch is constant (four f64 channel sums), including at extreme zoom.
    pub fn raster(self, source: &image::DynamicImage) -> image::DynamicImage {
        image::DynamicImage::ImageRgba8(self.sample(source))
    }

    fn sample(
        self,
        source: &impl image::GenericImageView<Pixel = image::Rgba<u8>>,
    ) -> image::RgbaImage {
        let (pw, ph) = self.pixels;
        let mut canvas = image::RgbaImage::new(pw, ph);
        for (px, py, pixel) in canvas.enumerate_pixels_mut() {
            if self.scale >= 1.0 {
                let p = self.source_at((px as f64 + 0.5, py as f64 + 0.5));
                if p.0 >= 0.0
                    && p.1 >= 0.0
                    && p.0 < self.source.0 as f64
                    && p.1 < self.source.1 as f64
                {
                    *pixel = image::imageops::interpolate_bilinear(
                        source,
                        (p.0 - 0.5).clamp(0.0, (self.source.0 - 1) as f64) as f32,
                        (p.1 - 0.5).clamp(0.0, (self.source.1 - 1) as f64) as f32,
                    )
                    .unwrap();
                }
                continue;
            }
            let top = self.source_at((px as f64, py as f64));
            let bottom = self.source_at((px as f64 + 1.0, py as f64 + 1.0));
            let left = top.0.max(0.0);
            let top_y = top.1.max(0.0);
            let right = bottom.0.min(self.source.0 as f64);
            let bottom_y = bottom.1.min(self.source.1 as f64);
            let mut sum = [0.0; 4];
            for y in top_y.floor() as u32..bottom_y.ceil().max(0.0) as u32 {
                let wy = (bottom_y.min(y as f64 + 1.0) - top_y.max(y as f64)).max(0.0);
                for x in left.floor() as u32..right.ceil().max(0.0) as u32 {
                    let weight = wy * (right.min(x as f64 + 1.0) - left.max(x as f64)).max(0.0);
                    let rgba = source.get_pixel(x, y).0;
                    let alpha = rgba[3] as f64 * weight;
                    for c in 0..3 {
                        sum[c] += rgba[c] as f64 * alpha;
                    }
                    sum[3] += alpha;
                }
            }
            if sum[3] > 0.0 {
                for c in 0..3 {
                    pixel[c] = (sum[c] / sum[3]).round().clamp(0.0, 255.0) as u8;
                }
                pixel[3] = (sum[3] * self.scale * self.scale).round().clamp(0.0, 255.0) as u8;
            }
        }
        canvas
    }
}

#[derive(Default)]
pub(crate) struct Inspector {
    pub view: View,
    pub source: Option<(u32, u32)>,
    pub scene: Option<Rect>,
    /// Input authority belongs to the last painted SAME identity/geometry.
    /// Desired view may advance while that protocol is still on screen.
    pub painted: Option<View>,
    pub viewport: Option<Rect>,
    pub pixels: (u32, u32),
    pub loading: bool,
    pub drag: Option<(u16, u16)>,
}
impl Inspector {
    pub fn geometry(&mut self, scene: Rect, pixels: (u32, u32)) {
        if self.scene != Some(scene) || self.pixels != pixels {
            self.viewport = None;
            self.painted = None;
            self.drag = None;
            self.scene = Some(scene);
            self.pixels = pixels;
            if let Some(source) = self.source {
                self.view.clamp(source, pixels);
            }
        }
    }
    pub fn pointer(&self, x: u16, y: u16) -> (f64, f64) {
        let r = self.scene.unwrap_or_default();
        (
            (x as f64 + 0.5 - r.x as f64) * self.pixels.0 as f64 / r.width.max(1) as f64,
            (y as f64 + 0.5 - r.y as f64) * self.pixels.1 as f64 / r.height.max(1) as f64,
        )
    }
    pub fn zoom(&mut self, up: bool, point: (f64, f64)) {
        if let Some(source) = self.source {
            self.view.zoom_at(up, point, source, self.pixels);
            self.loading = true;
        }
    }
    pub fn pan(&mut self, dx: f64, dy: f64) {
        if let Some(source) = self.source {
            self.view.pan(dx, dy, source, self.pixels);
            self.loading = true;
        }
    }
    pub fn action(&mut self, action: Action) {
        match action {
            Action::ZoomIn | Action::ZoomOut => self.zoom(
                matches!(action, Action::ZoomIn),
                (self.pixels.0 as f64 / 2.0, self.pixels.1 as f64 / 2.0),
            ),
            Action::Fit => {
                self.view = View::default();
                self.drag = None;
                self.loading = true;
            }
            Action::Left => self.pan(self.pixels.0 as f64 / 8.0, 0.0),
            Action::Right => self.pan(-(self.pixels.0 as f64) / 8.0, 0.0),
            Action::Up => self.pan(0.0, self.pixels.1 as f64 / 8.0),
            Action::Down => self.pan(0.0, -(self.pixels.1 as f64) / 8.0),
        }
    }
    pub fn percent(&self) -> u32 {
        self.view.zoom * 100 / FIT
    }
    pub fn status_label(&self) -> String {
        let scale = if self.view.zoom == FIT {
            "Fit 100%".to_string()
        } else {
            format!("{}% inspect (of Fit)", self.percent())
        };
        if self.loading {
            format!("{scale} · updating")
        } else {
            scale
        }
    }
    pub fn semantic(&self) -> serde_json::Value {
        serde_json::json!({"mode": if self.view.zoom == FIT {"fit"} else {"inspect"},
            "zoom_percent": self.percent(), "pan": [self.view.x as f64 / UNIT as f64, self.view.y as f64 / UNIT as f64],
            "source_dimensions": self.source, "viewport": self.viewport.map(|r| [r.x, r.y, r.width, r.height]),
            "scene": self.scene.map(|r| [r.x, r.y, r.width, r.height]),
            "viewport_pixels": self.pixels, "loading": self.loading, "dragging": self.drag.is_some(),
            "status": if self.loading {if self.viewport.is_some() {"updating"} else {"loading"}} else if self.viewport.is_some() {"ready"} else {"empty"},
            "zoom_basis": "relative_to_fit",
            "painted": self.painted.map(|v| serde_json::json!({"zoom_percent": v.zoom * 100 / FIT,
                "pan": [v.x as f64 / UNIT as f64, v.y as f64 / UNIT as f64]}))})
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/still_inspector__tests.rs"]
mod tests;
