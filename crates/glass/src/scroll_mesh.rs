//! Bounded, exported TRELLIS roller geometry; no model runtime in the UI.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}
#[derive(Debug)]
pub(crate) struct Mesh {
    pub vertices: Vec<Vertex>,
    pub triangles: Vec<[u32; 3]>,
}
impl Mesh {
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 16 || &bytes[..8] != b"AXSM0001" {
            return Err("invalid mesh header");
        }
        let word =
            |offset: usize| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let nv = word(8) as usize;
        let nt = word(12) as usize;
        if !(3..=36_000).contains(&nv) || !(1..=8_000).contains(&nt) {
            return Err("mesh exceeds runtime bounds");
        }
        if bytes.len() != 16 + nv * 32 + nt * 12 {
            return Err("mesh byte count mismatch");
        }
        let mut vertices = Vec::with_capacity(nv);
        for i in 0..nv {
            let mut v = [0.0; 8];
            for (j, value) in v.iter_mut().enumerate() {
                *value = f32::from_bits(word(16 + i * 32 + j * 4));
            }
            if !v.iter().all(|v| v.is_finite())
                || v[..3].iter().any(|v| v.abs() > 8.0)
                || v[3..6].iter().any(|v| v.abs() > 1.01)
                || v[6..].iter().any(|v| !(0.0..=1.0).contains(v))
            {
                return Err("invalid mesh vertex");
            }
            vertices.push(Vertex {
                position: [v[0], v[1], v[2]],
                normal: [v[3], v[4], v[5]],
                uv: [v[6], v[7]],
            });
        }
        let mut triangles = Vec::with_capacity(nt);
        for i in 0..nt {
            let offset = 16 + nv * 32 + i * 12;
            let t = [word(offset), word(offset + 4), word(offset + 8)];
            if t.iter().any(|&v| v as usize >= nv) || t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                return Err("invalid mesh triangle");
            }
            triangles.push(t);
        }
        Ok(Self {
            vertices,
            triangles,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut out = b"AXSM0001".to_vec();
        out.extend(3u32.to_le_bytes());
        out.extend(1u32.to_le_bytes());
        for p in [[-1.0f32, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]] {
            for v in p.into_iter().chain([0.0, 0.0, 1.0, 0.0, 0.0]) {
                out.extend(v.to_le_bytes());
            }
        }
        for i in [0u32, 1, 2] {
            out.extend(i.to_le_bytes());
        }
        out
    }
    #[test]
    fn embedded_trellis_export_is_bounded_and_textured() {
        let (mesh, w, h, pixels) =
            asset().expect("generated asset must decode, not silently use fallback");
        assert_eq!((mesh.vertices.len(), mesh.triangles.len()), (6487, 7815));
        assert_eq!((w, h, pixels.len()), (1024, 1024, 1024 * 1024 * 4));
        assert!(
            pixels
                .chunks_exact(4)
                .all(|p| p[..3].iter().all(|c| *c <= p[3]))
        );
        for phase in [0.0, 1.0, 10.0, -10.0] {
            assert!(mesh.vertices.iter().all(|v| {
                let p = v.project(phase, [500.0, 200.0], 400.0, 20.0);
                p.pixel.iter().all(|v| v.is_finite()) && p.depth.is_finite()
            }));
        }
    }
    #[test]
    fn axle_rotation_moves_geometry_and_normals_not_texture() {
        let v = Vertex {
            position: [0.4, 1.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.3, 0.8],
        };
        let front = v.project(0.0, [50.0, 100.0], 40.0, 10.0);
        let turned = v.project(std::f32::consts::FRAC_PI_2, [50.0, 100.0], 40.0, 10.0);
        assert_eq!(front.pixel, [66.0, 90.0]);
        assert!((turned.pixel[1] - 100.0).abs() < 0.001);
        assert!((turned.depth - 1.0).abs() < 0.001);
        assert!(turned.light > front.light);
        assert_eq!(v.uv, [0.3, 0.8]);
        for phase in [-1000.0f32, -1.0, 0.0, 30.0, 1000.0] {
            let p = v.project(phase, [50.0, 100.0], 40.0, 10.0);
            assert!(
                p.pixel.iter().all(|v| v.is_finite()) && p.depth.is_finite() && p.light.is_finite()
            );
        }
    }
    #[test]
    fn trellis_mesh_accepts_bounded_textured_geometry_and_rejects_corruption() {
        let bytes = fixture();
        let mesh = Mesh::parse(&bytes).expect("valid textured triangle");
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.triangles, [[0, 1, 2]]);
        for n in [0, 7, 15, bytes.len() - 1] {
            assert!(Mesh::parse(&bytes[..n]).is_err());
        }
        let mut bad = bytes.clone();
        bad[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Mesh::parse(&bad).is_err());
        let mut bad = bytes.clone();
        let end = bad.len();
        bad[end - 4..].copy_from_slice(&3u32.to_le_bytes());
        assert!(Mesh::parse(&bad).is_err());
        let mut bad = bytes.clone();
        bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Mesh::parse(&bad).is_err());
        let mut bad = bytes.clone();
        bad.push(0);
        assert!(Mesh::parse(&bad).is_err());
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Projected {
    pub pixel: [f32; 2],
    pub depth: f32,
    pub light: f32,
}
impl Vertex {
    /// Rigid X-axis rotation from actual paper travel/radius. Projection is
    /// orthographic like the paper's near-flat rest pose; geometry and normals
    /// rotate together, so texture and lighting do not slide over a flat plate.
    pub fn project(self, phase: f32, center: [f32; 2], half_width: f32, radius: f32) -> Projected {
        let (s, c) = phase.sin_cos();
        let [x, y, z] = self.position;
        let yy = y * c - z * s;
        let zz = y * s + z * c;
        let [nx, ny, nz] = self.normal;
        let ny = ny * c - nz * s;
        let nz = self.normal[1] * s + nz * c;
        let light = (0.38 + 0.70 * (-nx * 0.30 + ny * 0.55 + nz * 0.78).max(0.0))
            + 0.16 * nz.max(0.0).powi(24);
        Projected {
            pixel: [center[0] + x * half_width, center[1] - yy * radius],
            depth: zz,
            light,
        }
    }
}

// Offline, bounded TRELLIS export: the UI does not load the generative model.
// Keep the texture premultiplied to match the compositor's blending contract.
pub(crate) fn asset() -> Option<(Mesh, u32, u32, Vec<u8>)> {
    const MESH: &[u8] =
        include_bytes!("../../../cockpit/assets/onboarding/setup/trellis/roller.axsm");
    const TEXTURE: &[u8] =
        include_bytes!("../../../cockpit/assets/onboarding/setup/trellis/roller-basecolor.png");
    let mesh = Mesh::parse(MESH).ok()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(TEXTURE));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    if info.width != 1024 || info.height != 1024 {
        return None;
    }
    let size = reader.output_buffer_size()?;
    if size > 1024 * 1024 * 4 {
        return None;
    }
    let mut data = vec![0; size];
    let frame = reader.next_frame(&mut data).ok()?;
    if frame.color_type != png::ColorType::Rgba {
        return None;
    }
    data.truncate(frame.buffer_size());
    for pixel in data.chunks_exact_mut(4) {
        for c in 0..3 {
            pixel[c] = ((u16::from(pixel[c]) * u16::from(pixel[3]) + 127) / 255) as u8;
        }
    }
    Some((mesh, frame.width, frame.height, data))
}
