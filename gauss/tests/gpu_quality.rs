//! Real shader regressions: image quality is tested as pixels, not shader text.
use angel_glass::{PictureProducer, wgpu};
use gauss_glass::{
    gpu::{Camera, Quality, Renderer, Stats},
    import::{Cloud, Gaussian, SH_C0},
};
use std::sync::{Arc, Mutex};

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}
impl Gpu {
    fn new() -> Self {
        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("GPU quality verification requires an adapter; do not silently skip");
        println!("Quality test adapter: {}", adapter.get_info().name);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .unwrap();
        Self { device, queue }
    }
    fn render(&self, cloud: Cloud, camera: Camera, size: (u32, u32)) -> Vec<u8> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("quality-pixel-test"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let stats = Arc::new(Mutex::new(Stats::default()));
        let mut renderer =
            Renderer::new(Arc::new(cloud), Arc::new(Mutex::new(camera)), stats.clone());
        renderer.draw(
            &self.device,
            &self.queue,
            &texture.create_view(&Default::default()),
            wgpu::TextureFormat::Rgba8Unorm,
            size,
        );
        let error = stats.lock().unwrap().error.clone();
        assert!(error.is_none(), "{error:?}");
        let stride = (size.0 * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: stride as u64 * size.1 as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = buffer.slice(..).get_mapped_range().unwrap();
        let mut out = Vec::with_capacity((size.0 * size.1 * 4) as usize);
        for row in mapped.chunks(stride as usize) {
            out.extend_from_slice(&row[..size.0 as usize * 4]);
        }
        drop(mapped);
        buffer.unmap();
        out
    }
}
fn splat(z: f32, scale: f32, color: f32) -> Gaussian {
    let mut sh = [[0.; 3]; 16];
    sh[0] = [(color - 0.5) / SH_C0; 3];
    Gaussian {
        position: [0., 0., z],
        scale: [scale; 3],
        rotation: [1., 0., 0., 0.],
        opacity: 0.5,
        sh,
    }
}
fn cloud(gaussians: Vec<Gaussian>) -> Cloud {
    Cloud {
        gaussians,
        degree: 0,
        source: "quality-fixture".into(),
        centre: [0.; 3],
        radius: 1.,
    }
}
fn camera() -> Camera {
    Camera {
        yaw: 0.,
        pitch: 0.,
        ..Camera::default()
    }
}
#[test]
fn tiny_splats_do_not_gain_energy_from_pixel_filter() {
    let gpu = Gpu::new();
    let pixels = gpu.render(cloud(vec![splat(0., 0.0025, 1.)]), camera(), (128, 128));
    let background = pixels[0] as i32;
    let energy: i32 = pixels
        .chunks_exact(4)
        .map(|p| (p[0] as i32 - background).max(0))
        .sum();
    println!("tiny splat red energy above background = {energy}");
    assert!(
        (3..35).contains(&energy),
        "Pixel filtering inflated or erased tiny splat: energy={energy}"
    );
}
#[test]
fn bright_layers_are_not_clamped_before_transparency_compositing() {
    let gpu = Gpu::new();
    let mut c = camera();
    c.exposure = 3.;
    let pixels = gpu.render(
        cloud(vec![splat(0.1, 0.25, 1.), splat(-0.1, 0.25, 0.)]),
        c,
        (128, 128),
    );
    let centre = &pixels[((64 * 128 + 64) * 4)..][..4];
    println!("bright behind dark translucent splat = {centre:?}");
    assert!(
        centre[0] > 245,
        "Radiance clipped before alpha compositing: {centre:?}"
    );
}

#[test]
fn ultra_is_an_exact_two_by_two_resolve_not_a_sharpening_filter() {
    let gpu = Gpu::new();
    let fixture = || cloud(vec![splat(0.1, 0.025, 1.), splat(-0.1, 0.032, 0.15)]);
    let mut c = camera();
    c.pan = [0.0037, -0.0071];
    let high = gpu.render(fixture(), c, (256, 256));
    c.quality = Quality::Ultra;
    let ultra = gpu.render(fixture(), c, (128, 128));
    let mut max_error = 0i32;
    for y in 0..128usize {
        for x in 0..128usize {
            for ch in 0..3 {
                let mut sum = 0u32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        sum += high[(((y * 2 + dy) * 256 + x * 2 + dx) * 4) + ch] as u32;
                    }
                }
                let want = (sum as f64 / 4.).round() as i32;
                max_error = max_error.max((want - ultra[(y * 128 + x) * 4 + ch] as i32).abs());
            }
        }
    }
    println!("2x2 resolve maximum 8-bit error = {max_error}");
    // Single quantization after resolve vs four independent quantizations.
    assert!(
        max_error <= 1,
        "Ultra differs from explicit four-sample integration"
    );
}

/// Explicit local artifact generation, not part of the default test run.
/// Uses the exact production shader/pipelines, deterministic camera, no UI layout changes.
#[test]
#[ignore = "manual quality comparison: requires local downloaded assets"]
fn export_same_camera_quality_comparison() {
    let gpu = Gpu::new();
    let asset = std::env::var("GAUSS_QUALITY_ASSET").unwrap_or_else(|_| "assets/nike.splat".into());
    let stem = std::path::Path::new(&asset)
        .file_stem()
        .unwrap()
        .to_string_lossy();
    let directory = std::path::Path::new("captures/quality").join(stem.as_ref());
    std::fs::create_dir_all(&directory).unwrap();
    let source = Cloud::load(&asset).unwrap();
    std::fs::write(
        directory.join("source.txt"),
        format!(
            "asset={asset}\ncount={}\nsh_degree={}\nprojection=factored\n",
            source.gaussians.len(),
            source.degree
        ),
    )
    .unwrap();
    for (name, zoom) in [("overview", 1.), ("detail", 2.4)] {
        // A 4x-per-axis reference is for measurement, not a runtime mode.
        // Box-downsample in integer arithmetic after the high-resolution capture.
        let high = gpu.render(
            source.clone(),
            Camera {
                zoom,
                ..Camera::default()
            },
            (4000, 3040),
        );
        let mut integrated = vec![0u8; 1000 * 760 * 4];
        for y in 0..760usize {
            for x in 0..1000usize {
                for ch in 0..4 {
                    let mut sum = 0u32;
                    for dy in 0..4 {
                        for dx in 0..4 {
                            sum += high[(((y * 4 + dy) * 4000 + x * 4 + dx) * 4) + ch] as u32;
                        }
                    }
                    integrated[(y * 1000 + x) * 4 + ch] = ((sum + 8) / 16) as u8;
                }
            }
        }
        image::save_buffer(
            directory.join(format!("{name}-sample4x.png")),
            &integrated,
            1000,
            760,
            image::ColorType::Rgba8,
        )
        .unwrap();
        for (mode, quality) in [
            ("reference", Quality::Reference),
            ("balanced", Quality::Balanced),
            ("ultra", Quality::Ultra),
        ] {
            let c = Camera {
                quality,
                zoom,
                ..Camera::default()
            };
            let pixels = gpu.render(source.clone(), c, (1000, 760));
            let path = directory.join(format!("{name}-{mode}.png"));
            image::save_buffer(&path, &pixels, 1000, 760, image::ColorType::Rgba8).unwrap();
            println!(
                "QUALITY_CAPTURE {} count={}",
                path.display(),
                source.gaussians.len()
            );
        }
    }
}

#[test]
fn quality_switching_and_resize_reuse_asset_buffers_and_report_target_cost() {
    let gpu = Gpu::new();
    let camera = Arc::new(Mutex::new(camera()));
    let stats = Arc::new(Mutex::new(Stats::default()));
    let mut renderer = Renderer::new(
        Arc::new(cloud(vec![splat(0., 0.1, 1.)])),
        camera.clone(),
        stats.clone(),
    );
    for (quality, size, scale) in [
        (Quality::Balanced, (64, 48), 1),
        (Quality::Ultra, (64, 48), 2),
        (Quality::Ultra, (32, 24), 2),
        (Quality::Reference, (32, 24), 1),
        (Quality::Balanced, (64, 48), 1),
    ] {
        camera.lock().unwrap().quality = quality;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("quality-switch-test"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        renderer.draw(
            &gpu.device,
            &gpu.queue,
            &texture.create_view(&Default::default()),
            wgpu::TextureFormat::Rgba8Unorm,
            size,
        );
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .unwrap();
        let s = stats.lock().unwrap();
        assert_eq!(
            s.asset_uploads, 1,
            "Quality switching reuploaded source buffers"
        );
        assert_eq!(s.sample_scale, scale);
        assert_eq!(s.render_size, [size.0 * scale, size.1 * scale]);
        assert_eq!(
            s.target_bytes,
            if quality == Quality::Reference {
                0
            } else {
                u64::from(size.0) * u64::from(size.1) * u64::from(scale * scale) * 8
            }
        );
        assert!(s.error.is_none(), "{:?}", s.error);
    }
}

/// Independent f64 matrix oracle for non-centred, fully 3D Gaussians. This
/// constructs R S² R^T and J C J^T directly; it does not call production axes,
/// covariance, camera-row, sorting, SH or ellipse-eigenvector code.
fn perspective_oracle(g: &Gaussian, c: Camera, size: u32) -> Vec<u8> {
    let (sy, cy) = (c.yaw as f64).sin_cos();
    let (sp, cp) = (c.pitch as f64).sin_cos();
    let (sr, cr) = (c.roll as f64).sin_cos();
    let x = [cy, 0., sy];
    let y = [sp * sy, cp, -sp * cy];
    let rows: [[f64; 3]; 3] = [
        std::array::from_fn(|i| cr * x[i] - sr * y[i]),
        std::array::from_fn(|i| sr * x[i] + cr * y[i]),
        [-cp * sy, sp, cp * cy],
    ];
    let dot = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    let position = [
        g.position[0] as f64,
        -g.position[1] as f64,
        -g.position[2] as f64,
    ];
    let rotated = rows.map(|row| dot(row, position));
    let p = [
        rotated[0] - c.pan[0] as f64,
        rotated[1] - c.pan[1] as f64,
        rotated[2],
    ];
    let distance = 3.2 / c.zoom as f64 - p[2];
    assert!(distance > 0.05);
    let f = size as f64 * 1.2;
    let gx = [f / distance, 0., f * p[0] / distance.powi(2)];
    let gy = [0., -f / distance, -f * p[1] / distance.powi(2)];
    let invrot = |v: [f64; 3]| {
        std::array::from_fn::<_, 3, _>(|i| {
            rows[0][i] * v[0] + rows[1][i] * v[1] + rows[2][i] * v[2]
        })
    };
    // Undo the viewer's Y/Z convention to project the source covariance.
    let j = [invrot(gx), invrot(gy)].map(|v| [v[0], -v[1], -v[2]]);
    let [w, x, y, z] = g.rotation.map(f64::from);
    let rotation = [
        [
            1. - 2. * (y * y + z * z),
            2. * (x * y - w * z),
            2. * (x * z + w * y),
        ],
        [
            2. * (x * y + w * z),
            1. - 2. * (x * x + z * z),
            2. * (y * z - w * x),
        ],
        [
            2. * (x * z - w * y),
            2. * (y * z + w * x),
            1. - 2. * (x * x + y * y),
        ],
    ];
    let covariance: [[f64; 3]; 3] = std::array::from_fn(|i| {
        std::array::from_fn(|k| {
            (0..3)
                .map(|n| rotation[i][n] * rotation[k][n] * (g.scale[n] as f64).powi(2))
                .sum()
        })
    });
    let project = |a: [f64; 3], b: [f64; 3]| {
        (0..3)
            .flat_map(|i| (0..3).map(move |k| a[i] * covariance[i][k] * b[k]))
            .sum::<f64>()
    };
    let a = project(j[0], j[0]);
    let b = project(j[0], j[1]);
    let d = project(j[1], j[1]);
    let det_raw = (a * d - b * b).max(0.);
    let det = (a + 0.3) * (d + 0.3) - b * b;
    let opacity = g.opacity as f64 * (det_raw / det).sqrt();
    let centre = [
        size as f64 * 0.5 + f * p[0] / distance,
        size as f64 * 0.5 - f * p[1] / distance,
    ];
    let mut out = vec![0; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 + 0.5 - centre[0];
            let dy = y as f64 + 0.5 - centre[1];
            let r2 = ((d + 0.3) * dx * dx - 2. * b * dx * dy + (a + 0.3) * dy * dy) / det;
            let alpha = (opacity * (-0.5 * r2).exp()).min(0.99);
            let alpha = if alpha < 1. / 255. { 0. } else { alpha };
            let at = ((y * size + x) * 4) as usize;
            for (ch, bg) in [0.009, 0.012, 0.019].into_iter().enumerate() {
                out[at + ch] = ((alpha + (1. - alpha) * bg) * 255.).round() as u8;
            }
            out[at + 3] = 255;
        }
    }
    out
}
#[test]
fn fully_3d_anisotropic_projection_matches_independent_matrix_oracle() {
    let gpu = Gpu::new();
    let mut g = splat(0.18, 1., 1.);
    g.position = [0.07, -0.04, 0.18];
    g.scale = [0.12, 0.018, 0.0009];
    g.rotation = [0.72, -0.21, 0.33, 0.49];
    g.validate(0).unwrap();
    for (yaw, pitch, roll) in [(0.2, -0.35, 0.17), (-0.9, 0.6, -0.4), (1.3, -0.7, 0.9)] {
        let c = Camera {
            yaw,
            pitch,
            roll,
            pan: [0.03, -0.02],
            zoom: 1.2,
            ..camera()
        };
        let expected = perspective_oracle(&g, c, 192);
        let actual = gpu.render(cloud(vec![g.clone()]), c, (192, 192));
        let max_error = actual
            .iter()
            .zip(&expected)
            .map(|(&a, &b)| a.abs_diff(b))
            .max()
            .unwrap();
        let rms = (actual
            .iter()
            .zip(&expected)
            .map(|(&a, &b)| (a as f64 - b as f64).powi(2))
            .sum::<f64>()
            / actual.len() as f64)
            .sqrt();
        println!(
            "3D_PROJECTION yaw={yaw} pitch={pitch} roll={roll} maximum={max_error}/255 RMS={rms:.5}/255"
        );
        assert!(
            max_error <= 1,
            "GPU projection differs from independent J C J^T oracle"
        );
    }
}

/// Independent f64 oracle for a centred planar Gaussian: no shader code or
/// eigendecomposition is reused. Integrate the filtered ellipse at pixel centres.
fn needle_oracle(size: u32, angle: f64, shift: f64) -> Vec<u8> {
    let f = size as f64 * 1.2 / 3.2;
    let major = (2. * f).powi(2);
    let minor = (0.0002 * f).powi(2);
    let (s, c) = angle.sin_cos();
    let a = major * c * c + minor * s * s + 0.3;
    let b = (major - minor) * s * c;
    let d = major * s * s + minor * c * c + 0.3;
    let det = (major + 0.3) * (minor + 0.3);
    let opacity = 0.5 * (major * minor / det).sqrt();
    let mut out = vec![0; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 + 0.5 - size as f64 * 0.5 + shift;
            let dy = y as f64 + 0.5 - size as f64 * 0.5;
            let r2 = (d * dx * dx - 2. * b * dx * dy + a * dy * dy) / det;
            let alpha = opacity * (-0.5 * r2).exp();
            let alpha = if alpha < 1. / 255. {
                0.
            } else {
                alpha.min(0.99)
            };
            let at = ((y * size + x) * 4) as usize;
            for (channel, background) in [0.009, 0.012, 0.019].iter().enumerate() {
                out[at + channel] = ((alpha + (1. - alpha) * background) * 255.).round() as u8;
            }
            out[at + 3] = 255;
        }
    }
    out
}
fn needle(angle: f64) -> Cloud {
    let mut g = splat(0., 1., 1.);
    g.scale = [2., 0.0002, 0.0002];
    g.rotation = [
        (angle * 0.5).cos() as f32,
        0.,
        0.,
        (angle * 0.5).sin() as f32,
    ];
    cloud(vec![g])
}
#[test]
fn rotated_thin_splats_match_double_precision_projection_in_subpixel_motion() {
    let gpu = Gpu::new();
    let size = 256;
    for angle in [
        0.,
        0.00001,
        0.01,
        0.3,
        std::f64::consts::FRAC_PI_4,
        1.3,
        std::f64::consts::FRAC_PI_2,
    ] {
        for step in 0..5 {
            let shift = step as f64 * 0.2;
            let mut c = camera();
            c.pan[0] = (shift * 3.2 / (size as f64 * 1.2)) as f32;
            let actual = gpu.render(needle(angle), c, (size, size));
            let expected = needle_oracle(size, angle, shift);
            let want_energy: i64 = expected
                .chunks_exact(4)
                .map(|p| (p[0] as i64 - 2).max(0))
                .sum();
            let got_energy: i64 = actual
                .chunks_exact(4)
                .map(|p| (p[0] as i64 - 2).max(0))
                .sum();
            let max_error = actual
                .iter()
                .zip(&expected)
                .map(|(&a, &b)| a.abs_diff(b))
                .max()
                .unwrap();
            println!(
                "needle angle={angle:.4} shift={shift:.1}px energy GPU={got_energy} f64={want_energy} max_error={max_error}"
            );
            assert!(want_energy > 500, "Oracle fixture must have visible energy");
            assert!(
                (got_energy - want_energy).abs() * 10 < want_energy,
                "Thin Gaussian lost area or brightness: GPU={got_energy} independent f64={want_energy}"
            );
            assert!(
                max_error <= 1,
                "Thin ellipse deviated from independent projected Gaussian by {max_error}/255"
            );
        }
    }
}

#[test]
fn subpixel_translation_preserves_small_splat_energy() {
    let gpu = Gpu::new();
    for quality in [Quality::Reference, Quality::Balanced, Quality::Ultra] {
        let mut values = Vec::new();
        for frame in 0..17 {
            let shift = frame as f32 / 16.;
            let c = Camera {
                quality,
                pan: [shift * 3.2 / (128. * 1.2), 0.],
                ..camera()
            };
            let pixels = gpu.render(cloud(vec![splat(0., 0.006, 1.)]), c, (128, 128));
            let energy: i64 = pixels
                .chunks_exact(4)
                .map(|p| (p[0] as i64 - 2).max(0))
                .sum();
            values.push(energy);
        }
        let mean = values.iter().sum::<i64>() as f64 / values.len() as f64;
        let swing = (*values.iter().max().unwrap() - *values.iter().min().unwrap()) as f64 / mean;
        println!(
            "MOTION_ENERGY {} frames={values:?} mean={mean:.2} relative_peak_to_peak={swing:.4}",
            quality.label()
        );
        if quality == Quality::Ultra {
            assert!(
                swing < 0.1,
                "Ultra loses energy through subpixel movement: {swing:.4}"
            );
        }
    }
}

#[test]
#[ignore = "manual deterministic motion preview; requires downloaded shoe asset"]
fn export_quality_motion_preview() {
    let gpu = Gpu::new();
    let root = std::path::Path::new("captures/stability/raw");
    std::fs::create_dir_all(root).unwrap();
    let source = Cloud::load("assets/nike.splat").unwrap();
    for frame in 0..144 {
        let t = ((frame as f32 - 12.) / 95.).clamp(0., 1.);
        let ease = t * t * t * (t * (t * 6. - 15.) + 10.);
        for (name, quality) in [("balanced", Quality::Balanced), ("ultra", Quality::Ultra)] {
            let c = Camera {
                quality,
                yaw: 0.3 + ease * 0.32,
                pitch: -0.15 + ease * 0.04,
                zoom: 1. + ease * 0.9,
                ..Camera::default()
            };
            let image = gpu.render(source.clone(), c, (720, 540));
            image::save_buffer(
                root.join(format!("{name}-{frame:04}.png")),
                &image,
                720,
                540,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
    println!(
        "MOTION_EXPORT_OK frames=144 fps=24 source_count={} raw_size=720x540 modes=balanced,ultra",
        source.gaussians.len()
    );
}
