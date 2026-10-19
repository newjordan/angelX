// Included by render.rs: the rigid TRELLIS rollers, drawn as textured
// triangles (kind 12) through the compositor's own instance buffer, with
// their texture held in angel-gpu's picture store like any other picture.

/// The rollers' texture, among glass's own layer pictures.
const ROLLER_PICTURE: u32 = crate::layer::LAYER_PICTURES + 0x90;

struct ScrollMesh {
    mesh: crate::scroll_mesh::Mesh,
    size: [f32; 2],
    /// Kept to hand over again if the picture budget let the texture go.
    rgba: Vec<u8>,
}

impl Renderer {
    fn load_scroll_mesh(&mut self) {
        // The generated export is embedded for relocatable installations. A
        // failed decode leaves the procedural rollers available.
        if self.scroll_mesh_attempted {
            return;
        }
        self.scroll_mesh_attempted = true;
        if std::env::var("ANGEL_SCROLL_MESH").as_deref() == Ok("0") {
            return;
        }
        let Some((mesh, width, height, rgba)) = crate::scroll_mesh::asset() else {
            return;
        };
        self.scroll_mesh = Some(ScrollMesh { mesh, size: [width as f32, height as f32], rgba });
    }

    fn scroll_mesh_instances(
        &mut self,
        page: &crate::scroll::Page,
        origin: (f32, f32),
    ) -> Option<Vec<Instance>> {
        if page.pose.open <= 0.00001 {
            return None;
        }
        self.load_scroll_mesh();
        let asset = self.scroll_mesh.as_ref()?;
        if !self.core.has_picture(ROLLER_PICTURE) {
            self.core.upload(graphics::Picture {
                id: ROLLER_PICTURE,
                width: asset.size[0] as u32,
                height: asset.size[1] as u32,
                rgba: asset.rgba.clone(),
                cols: 0,
                rows: 0,
            });
        }
        let cell = self.fonts.cell;
        let [x, y, w, h] = [
            origin.0 + page.area.x as f32 * cell.width as f32,
            origin.1 + page.area.y as f32 * cell.height as f32,
            page.area.width as f32 * cell.width as f32,
            page.area.height as f32 * cell.height as f32,
        ];
        let pose = page.pose;
        let radius = (crate::scroll::RADIUS * h).max(3.0);
        let top = y + crate::scroll::surface(pose, 0.0)[0] * h;
        let bottom = y + crate::scroll::surface(pose, crate::scroll::released(pose))[0] * h;
        let alpha = (pose.open * 12.0).min(1.0);
        let mut out = Vec::with_capacity(asset.mesh.triangles.len() * 2);
        for (center, phase) in [
            (top - radius * 0.35, -pose.roller_angle() * 0.4),
            (bottom + radius * 0.50, pose.roller_angle()),
        ] {
            let vertices: Vec<_> = asset
                .mesh
                .vertices
                .iter()
                .map(|v| v.project(phase, [x + w * 0.5, center], w * 0.5 + radius * 1.2, radius))
                .collect();
            let mut order: Vec<_> = asset
                .mesh
                .triangles
                .iter()
                .map(|t| {
                    let z = t.iter().map(|&i| vertices[i as usize].depth).sum::<f32>();
                    (z, t)
                })
                .collect();
            // Painter ordering is local to each opaque rigid roller; all of it
            // sits above the curved paper/text and below later modal overlays.
            order.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
            for (_, t) in order {
                let v = t.map(|i| vertices[i as usize]);
                let p = v.map(|v| v.pixel);
                let uv = t.map(|i| asset.mesh.vertices[i as usize].uv);
                let light = (v.iter().map(|v| v.light).sum::<f32>() / 3.0).clamp(0.25, 1.15);
                out.push(Instance {
                    rect: [p[0][0], p[0][1], p[1][0], p[1][1]],
                    pivot: p[2],
                    uv: [
                        uv[0][0] * asset.size[0],
                        (1.0 - uv[0][1]) * asset.size[1],
                        uv[1][0] * asset.size[0],
                        (1.0 - uv[1][1]) * asset.size[1],
                    ],
                    tilt: [
                        uv[2][0] * asset.size[0],
                        (1.0 - uv[2][1]) * asset.size[1],
                        0.0,
                        0.0,
                    ],
                    color: [light, light, light, alpha],
                    kind: angel_gpu::MESH_TRIANGLE,
                    ..Zeroable::zeroed()
                });
            }
        }
        Some(out)
    }
}
