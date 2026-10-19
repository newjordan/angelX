// Foreground scroll presentation. Included by render.rs to share the atlas and
// instance buffers without exposing compositor internals as a public API.
impl Renderer {
    fn scroll_instances(
        &mut self,
        page: &crate::scroll::Page,
        origin: (f32, f32),
    ) -> Vec<Instance> {
        use crate::scroll::{RADIUS, released, surface};
        let cell = self.fonts.cell;
        let (cw, ch) = (cell.width as f32, cell.height as f32);
        let area = page.area;
        let (x, y, w, h) = (
            origin.0 + area.x as f32 * cw,
            origin.1 + area.y as f32 * ch,
            area.width as f32 * cw,
            area.height as f32 * ch,
        );
        let pose = page.pose;
        if pose.open <= 0.00001 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(4096);
        let visibility = (pose.open * 12.0).min(1.0);
        let length = released(pose);
        let top_y = y + surface(pose, 0.0)[0] * h;
        let bottom_y = y + surface(pose, length)[0] * h;
        // A restrained veil leaves the workspace visible; nested translucent
        // shadows soften in six bands instead of an opaque rectangular card.
        out.push(Instance::solid(
            0.0,
            0.0,
            self.config.width as f32,
            self.config.height as f32,
            [
                0.015 * visibility,
                0.017 * visibility,
                0.024 * visibility,
                0.22 * visibility,
            ],
        ));
        for band in (0..7).rev() {
            let spread = band as f32 * 3.2;
            out.push(Instance::solid(
                x + 9.0 - spread,
                top_y + 14.0 - spread,
                w + spread * 2.0,
                (bottom_y - top_y).max(4.0) + spread * 2.0,
                [0.0, 0.0, 0.0, 0.026 * visibility],
            ));
        }
        let bend = |mut instance: Instance| {
            instance.tilt = [x, y, w, h];
            instance.stand = [3.0, pose.open, pose.sway, 0.0];
            instance
        };
        // 192 geometric strips: arc-length coordinates, cylinder curvature,
        // perspective and lighting; no stretching a flat scroll plate.
        let slices = 192;
        for i in 0..slices {
            let a = length * i as f32 / slices as f32;
            let b = length * (i + 1) as f32 / slices as f32;
            let light = surface(pose, (a + b) * 0.5)[2];
            let mut paper = Instance::solid(
                x,
                y + a * h,
                w,
                (b - a) * h,
                [0.91 * light, 0.82 * light, 0.64 * light, visibility],
            );
            paper.kind = angel_gpu::PARCHMENT;
            paper.uv = [0.0, a, 1.0, b - a];
            out.push(bend(paper));
            // Fine inset rules, coloured like burnished ink; they roll with paper.
            for u in [0.027, 0.973] {
                out.push(bend(Instance::solid(
                    x + w * u,
                    y + a * h,
                    0.85,
                    (b - a) * h,
                    [
                        0.47 * visibility,
                        0.31 * visibility,
                        0.16 * visibility,
                        0.5 * visibility,
                    ],
                )));
            }
        }
        let mut text = Vec::new();
        for row in 0..area.height {
            let mut col = 0;
            while col < area.width {
                let cell = &page.buffer[(col, row)];
                let span = crate::backend::symbol_width(cell.symbol())
                    .clamp(1, 2)
                    .min(usize::from(area.width - col));
                let xx = x + col as f32 * cw;
                let yy = y + row as f32 * ch;
                let fg = self.rgb(cell.fg, [56, 36, 20]);
                if let Color::Rgb(_, _, _) = cell.bg {
                    let bg = self.rgb(cell.bg, [232, 209, 163]);
                    append_scroll_slices(
                        &mut text,
                        Instance::solid(
                            xx,
                            yy,
                            span as f32 * cw,
                            ch,
                            premultiply(bg, 0.92 * visibility),
                        ),
                        y,
                        h,
                        length,
                    );
                }
                if !cell.symbol().trim().is_empty() && !cell.modifier.contains(Modifier::HIDDEN) {
                    let mut style = 0;
                    if cell.modifier.contains(Modifier::BOLD) {
                        style |= BOLD;
                    }
                    if cell.modifier.contains(Modifier::ITALIC) {
                        style |= ITALIC;
                    }
                    if let Some(entry) = self.glyph(cell.symbol(), style, span as u8) {
                        let alpha = if cell.modifier.contains(Modifier::DIM) {
                            0.6
                        } else {
                            1.0
                        };
                        append_scroll_slices(
                            &mut text,
                            Instance {
                                rect: [xx + entry.left, yy + entry.top, entry.uv[2], entry.uv[3]],
                                uv: entry.uv,
                                color: premultiply(fg, alpha * visibility),
                                kind: if entry.colored { COLOR } else { MASK },
                                ..Zeroable::zeroed()
                            },
                            y,
                            h,
                            length,
                        );
                    }
                }
                col += span as u16;
            }
        }
        out.extend(text.into_iter().map(bend));
        self.load_scroll_mesh();
        if self.scroll_mesh.is_some() {
            return out;
        }
        // Both fallback rollers are actual cylinders: horizontal lit bands wrap with
        // their angle (paper travel / radius). A fixed top and weighted lower.
        let radius = (RADIUS * h).max(3.0);
        for (center, phase) in [
            (top_y - radius * 0.35, -pose.roller_angle() * 0.4),
            (bottom_y + radius * 0.50, pose.roller_angle()),
        ] {
            let shaft_x = x - radius * 0.5;
            let shaft_w = w + radius;
            for band in 0..32 {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * band as f32 / 32.0;
                let b =
                    -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * (band + 1) as f32 / 32.0;
                let mid = (a + b) * 0.5;
                let light = (0.43 + 0.55 * (mid + 0.48).cos()).max(0.18);
                let grain = 0.94 + 0.06 * (mid * 16.0 + phase).sin();
                out.push(Instance::solid(
                    shaft_x,
                    center + radius * a.sin(),
                    shaft_w,
                    radius * (b.sin() - a.sin()) + 0.15,
                    [
                        0.36 * light * grain * visibility,
                        0.18 * light * grain * visibility,
                        0.075 * light * grain * visibility,
                        visibility,
                    ],
                ));
            }
            // Turned brass collars/endcaps, elliptical silhouette and lit bands.
            for cx in [shaft_x - radius * 0.38, shaft_x + shaft_w + radius * 0.38] {
                for band in 0..28 {
                    let v = -1.0 + 2.0 * (band as f32 + 0.5) / 28.0;
                    let reach = (1.0 - v * v).max(0.0).sqrt() * radius * 0.70;
                    let light = (0.55 + 0.40 * (v + 0.35).cos()).clamp(0.2, 1.0);
                    out.push(Instance::solid(
                        cx - reach,
                        center + v * radius * 1.25,
                        reach * 2.0,
                        radius * 2.5 / 28.0 + 0.15,
                        [
                            0.63 * light * visibility,
                            0.43 * light * visibility,
                            0.18 * light * visibility,
                            visibility,
                        ],
                    ));
                }
                out.push(Instance::solid(
                    cx - radius * 0.08,
                    center - radius * 0.88,
                    radius * 0.10,
                    radius * 0.32,
                    [
                        0.93 * visibility,
                        0.76 * visibility,
                        0.42 * visibility,
                        visibility,
                    ],
                ));
            }
        }
        out
    }
}

/// Split glyphs/backgrounds into at most 2px-high strips so the very same
/// cylindrical projection curves their geometry, with atlas UVs kept exact.
fn append_scroll_slices(
    out: &mut Vec<Instance>,
    instance: Instance,
    top: f32,
    height: f32,
    released: f32,
) {
    let [x, y, w, h] = instance.rect;
    if h <= 0.0 {
        return;
    }
    let from = y.max(top);
    let end = (y + h).min(top + height * released);
    let mut yy = from;
    while yy < end {
        let to = (yy + 2.0).min(end);
        let a = (yy - y) / h;
        let b = (to - y) / h;
        out.push(Instance {
            rect: [x, yy, w, to - yy],
            uv: [
                instance.uv[0],
                instance.uv[1] + instance.uv[3] * a,
                instance.uv[2],
                instance.uv[3] * (b - a),
            ],
            ..instance
        });
        yy = to;
    }
}
