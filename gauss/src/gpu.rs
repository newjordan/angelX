//! GPU-resident Gaussian renderer for a native Glass picture target.
//! Geometry/SH upload once; camera changes an exact float-depth radix-sorted index buffer.
use crate::import::Cloud;
use angel_glass::{PictureProducer, wgpu};
use bytemuck::{Pod, Zeroable};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
use wgpu::util::DeviceExt;

/// Reference retains the old direct 8-bit path for controlled A/B comparisons.
/// Balanced conserves filtered splat energy and composites in RGBA16Float.
/// Ultra also integrates a 2x2 high-resolution sample grid per output pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    Reference,
    #[default]
    Balanced,
    Ultra,
}
impl Quality {
    pub fn next(self) -> Self {
        match self {
            Self::Reference => Self::Balanced,
            Self::Balanced => Self::Ultra,
            Self::Ultra => Self::Reference,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Reference => "REFERENCE",
            Self::Balanced => "BALANCED / FP16 AA",
            Self::Ultra => "ULTRA / FP16 2X",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "reference" => Some(Self::Reference),
            "balanced" => Some(Self::Balanced),
            "ultra" => Some(Self::Ultra),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub pan: [f32; 2],
    pub zoom: f32,
    pub exposure: f32,
    pub viewport: [u32; 2],
    pub quality: Quality,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.3,
            pitch: -0.15,
            roll: 0.,
            pan: [0.; 2],
            zoom: 1.,
            exposure: 0.,
            viewport: [1200, 900],
            quality: Quality::default(),
        }
    }
}
impl Camera {
    pub fn rows(self) -> [[f32; 4]; 3] {
        let (s, c) = self.yaw.sin_cos();
        let (t, u) = self.pitch.sin_cos();
        let x = [c, 0., s, 0.];
        let y = [t * s, u, -t * c, 0.];
        let (sr, cr) = self.roll.sin_cos();
        [
            std::array::from_fn(|i| cr * x[i] - sr * y[i]),
            std::array::from_fn(|i| sr * x[i] + cr * y[i]),
            [-u * s, t, u * c, 0.],
        ]
    }
    pub fn drag(&mut self, dx: f32, dy: f32) {
        self.yaw = (self.yaw + dx * 0.006).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + dy * 0.006).clamp(-1.50, 1.50);
    }
    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        let scale =
            (3.2 / self.zoom) / (self.viewport[0].min(self.viewport[1]).max(1) as f32 * 1.2);
        self.pan[0] = (self.pan[0] - dx * scale).clamp(-100., 100.);
        self.pan[1] = (self.pan[1] + dy * scale).clamp(-100., 100.);
    }
    pub fn zoom_by(&mut self, steps: f32) {
        self.zoom = (self.zoom * (steps * 0.08).exp()).clamp(0.1, 40.);
    }
}
#[derive(Default, Clone, Debug)]
pub struct Stats {
    pub frames: u64,
    pub count: usize,
    pub resident_bytes: usize,
    pub sort_ms: f64,
    pub submit_ms: f64,
    pub error: Option<String>,
    /// Intermediate targets only; source buffers are counted separately.
    pub target_bytes: u64,
    pub render_size: [u32; 2],
    pub sample_scale: u32,
    /// Count full source uploads; changing quality/resolution must not increase it.
    pub asset_uploads: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Geometry {
    position: [f32; 4],
    axis0: [f32; 4],
    axis1: [f32; 4],
    axis2: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    rows: [[f32; 4]; 3],
    params: [f32; 4],
    options: [f32; 4],
    eye: [f32; 4],
    sh: [u32; 4],
}
struct Resources {
    pipeline: wgpu::RenderPipeline,
    reference_pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    order_buffer: wgpu::Buffer,
    positions: Vec<[f32; 3]>,
    order: Vec<u32>,
    keys: Vec<u32>,
    scratch: Vec<u32>,
    counts: Vec<u32>,
    last_rotation: Option<[u32; 3]>,
    output_format: wgpu::TextureFormat,
    resolve: Option<Resolve>,
}
pub struct Renderer {
    cloud: Arc<Cloud>,
    camera: Arc<Mutex<Camera>>,
    stats: Arc<Mutex<Stats>>,
    resources: Option<Resources>,
    failed: bool,
}
impl Renderer {
    pub fn new(cloud: Arc<Cloud>, camera: Arc<Mutex<Camera>>, stats: Arc<Mutex<Stats>>) -> Self {
        Self {
            cloud,
            camera,
            stats,
            resources: None,
            failed: false,
        }
    }
    fn initialize(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> Result<Resources, String> {
        let cloud = &self.cloud;
        let count = cloud.gaussians.len();
        let coefficients = (cloud.degree as usize + 1).pow(2);
        let geometry_bytes = count * std::mem::size_of::<Geometry>();
        let sh_bytes = count * coefficients * 12;
        let split = (count * coefficients * 3).div_ceil(2);
        let limit = device.limits().max_storage_buffer_binding_size as usize;
        if geometry_bytes > limit || split * 4 > limit {
            return Err(format!(
                "Asset exceeds this GPU's {limit} byte storage binding limit (geometry={geometry_bytes}, SH={sh_bytes}); no silent truncation"
            ));
        }
        let mut geometry = Vec::with_capacity(count);
        let mut sh = Vec::with_capacity(count * coefficients * 3);
        let inv = 1. / cloud.radius;
        for g in &cloud.gaussians {
            let p = std::array::from_fn::<_, 3, _>(|i| {
                (g.position[i] - cloud.centre[i]) * inv * if i == 0 { 1. } else { -1. }
            });
            let axes = g
                .axes()
                .map(|axis| [axis[0] * inv, -axis[1] * inv, -axis[2] * inv, 0.]);
            if !p.iter().chain(axes.iter().flatten()).all(|v| v.is_finite()) {
                return Err("Asset overflows normalized GPU covariance factors".into());
            }
            geometry.push(Geometry {
                position: [p[0], p[1], p[2], g.opacity],
                axis0: axes[0],
                axis1: axes[1],
                axis2: axes[2],
            });
            for c in &g.sh[..coefficients] {
                sh.extend_from_slice(c);
            }
        }
        let buffer = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let geometry_buffer = buffer(
            "gauss-resident-geometry",
            bytemuck::cast_slice(&geometry),
            wgpu::BufferUsages::STORAGE,
        );
        let sh_buffer = buffer(
            "gauss-resident-spherical-harmonics",
            bytemuck::cast_slice(&sh[..split]),
            wgpu::BufferUsages::STORAGE,
        );
        let sh_buffer_second = buffer(
            "gauss-resident-sh-second",
            bytemuck::cast_slice(&sh[split..]),
            wgpu::BufferUsages::STORAGE,
        );
        let order: Vec<u32> = (0..count as u32).collect();
        let order_buffer = buffer(
            "gauss-depth-order",
            bytemuck::cast_slice(&order),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let uniform = buffer(
            "gauss-camera",
            bytemuck::bytes_of(&Uniform::zeroed()),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gauss-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gauss-bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: geometry_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: order_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: sh_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: sh_buffer_second.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gauss-covariance-sh"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gaussian.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gauss-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let create_pipeline = |target_format| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("gauss-pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline = create_pipeline(wgpu::TextureFormat::Rgba16Float);
        let reference_pipeline = create_pipeline(format);
        {
            let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
            stats.count = count;
            stats.asset_uploads += 1;
            stats.resident_bytes =
                geometry_bytes + sh_bytes + count * 4 + std::mem::size_of::<Uniform>();
        }
        Ok(Resources {
            pipeline,
            reference_pipeline,
            bind,
            uniform,
            order_buffer,
            positions: geometry
                .iter()
                .map(|g| [g.position[0], g.position[1], g.position[2]])
                .collect(),
            order,
            keys: vec![0; count],
            scratch: vec![0; count],
            counts: vec![0; 65536],
            last_rotation: None,
            output_format: format,
            resolve: None,
        })
    }
}
impl PictureProducer for Renderer {
    fn size(&self) -> Option<(u32, u32)> {
        let c = self.camera.lock().unwrap_or_else(|e| e.into_inner());
        Some((c.viewport[0], c.viewport[1]))
    }
    fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) {
        let camera = *self.camera.lock().unwrap_or_else(|e| e.into_inner());
        // Quality changes reuse geometry, SH and both pipelines. Only an output
        // format change requires rebuilding bindings/pipelines for this device.
        if self
            .resources
            .as_ref()
            .is_some_and(|r| r.output_format != format)
        {
            self.resources = None;
            self.failed = false;
        }
        if self.resources.is_none() && !self.failed {
            match self.initialize(device, format) {
                Ok(r) => self.resources = Some(r),
                Err(e) => {
                    eprintln!("GAUSS_GPU_ERROR {e}");
                    self.stats.lock().unwrap_or_else(|e| e.into_inner()).error = Some(e);
                    self.failed = true;
                }
            }
        }
        let start = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gauss-frame"),
        });
        let mut sort_ms = 0.;
        let mut render_size = size;
        let mut sample_scale = 1;
        if camera.quality == Quality::Reference {
            if let Some(r) = &mut self.resources {
                r.resolve = None;
            }
        } else {
            let desired = if camera.quality == Quality::Ultra {
                2
            } else {
                1
            };
            // Bound extra target memory to 128 MiB. Keep a uniform integer sample grid
            // so box resolve remains exact even at a device/budget fallback.
            if desired == 2
                && size.0.saturating_mul(2) <= device.limits().max_texture_dimension_2d
                && size.1.saturating_mul(2) <= device.limits().max_texture_dimension_2d
                && u64::from(size.0) * u64::from(size.1) * 32 <= 128 * 1024 * 1024
            {
                sample_scale = 2;
            }
            render_size = (size.0 * sample_scale, size.1 * sample_scale);
            if let Some(r) = &mut self.resources {
                if r.resolve
                    .as_ref()
                    .is_none_or(|target| target.size != render_size)
                {
                    r.resolve = Some(Resolve::new(device, format, render_size));
                }
            }
        }
        if let Some(r) = &mut self.resources {
            let rotation = [
                camera.yaw.to_bits(),
                camera.pitch.to_bits(),
                camera.roll.to_bits(),
            ];
            let rows = camera.rows();
            if r.last_rotation != Some(rotation) {
                let before = Instant::now();
                sort_indices(
                    &r.positions,
                    rows[2],
                    &mut r.keys,
                    &mut r.counts,
                    &mut r.order,
                    &mut r.scratch,
                );
                queue.write_buffer(&r.order_buffer, 0, bytemuck::cast_slice(&r.order));
                r.last_rotation = Some(rotation);
                sort_ms = before.elapsed().as_secs_f64() * 1000.;
            }
            let u = Uniform {
                rows,
                params: [
                    render_size.0 as f32,
                    render_size.1 as f32,
                    3.2 / camera.zoom,
                    1.20,
                ],
                options: [
                    (self.cloud.degree + 1).pow(2) as f32,
                    camera.exposure,
                    if camera.quality == Quality::Reference {
                        0.
                    } else {
                        1.
                    },
                    0.,
                ],
                eye: [camera.pan[0], camera.pan[1], 0., 0.],
                sh: [
                    (self.cloud.degree + 1).pow(2),
                    (self.cloud.gaussians.len() * (self.cloud.degree as usize + 1).pow(2) * 3)
                        .div_ceil(2) as u32,
                    0,
                    0,
                ],
            };
            queue.write_buffer(&r.uniform, 0, bytemuck::bytes_of(&u));
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gauss-alpha-over"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self
                        .resources
                        .as_ref()
                        .and_then(|r| r.resolve.as_ref())
                        .map_or(view, |target| &target.view),
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.009,
                            g: 0.012,
                            b: 0.019,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(r) = &self.resources {
                pass.set_pipeline(if camera.quality == Quality::Reference {
                    &r.reference_pipeline
                } else {
                    &r.pipeline
                });
                pass.set_bind_group(0, &r.bind, &[]);
                pass.draw(0..6, 0..r.order.len() as u32);
            }
        }
        if let Some(target) = self.resources.as_ref().and_then(|r| r.resolve.as_ref()) {
            target.draw(&mut encoder, view);
        }
        queue.submit([encoder.finish()]);
        let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        stats.frames += 1;
        stats.render_size = [render_size.0, render_size.1];
        stats.sample_scale = sample_scale;
        stats.target_bytes = if camera.quality == Quality::Reference {
            0
        } else {
            u64::from(render_size.0) * u64::from(render_size.1) * 8
        };
        stats.submit_ms = start.elapsed().as_secs_f64() * 1000.;
        if sort_ms > 0. {
            stats.sort_ms = sort_ms;
        }
    }
}
/// Float accumulation is essential: an 8-bit attachment clamps SH radiance at
/// *each layer*, changing bright objects seen through translucent foregrounds.
struct Resolve {
    size: (u32, u32),
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}
impl Resolve {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, size: (u32, u32)) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gauss-fp16-accumulation"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gauss-resolve-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gauss-resolve-bind"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gauss-resolve-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("resolve.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gauss-resolve-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // Fragment output derivatives are exactly one output pixel. textureDimensions
        // and the output size are linked via UV derivatives, so no extra mutable uniform.
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gauss-resolve"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            size,
            _texture: texture,
            view,
            bind,
            pipeline,
        }
    }
    fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gauss-fp16-resolve"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
/// Exact float-depth order, far-to-near (ascending camera-space z). Two
/// stable 16-bit radix passes retain thin layers even beside distant floaters.
/// No allocations or comparison sort during camera interaction.
fn sort_indices(
    positions: &[[f32; 3]],
    row: [f32; 4],
    keys: &mut [u32],
    counts: &mut [u32],
    order: &mut [u32],
    scratch: &mut [u32],
) {
    for (p, key) in positions.iter().zip(keys.iter_mut()) {
        let bits = (p[0] * row[0] + p[1] * row[1] + p[2] * row[2]).to_bits();
        *key = if bits & 0x8000_0000 != 0 {
            !bits
        } else {
            bits ^ 0x8000_0000
        };
    }
    for shift in [0, 16] {
        counts.fill(0);
        for &key in keys.iter() {
            counts[((key >> shift) & 65535) as usize] += 1;
        }
        let mut sum = 0;
        for n in counts.iter_mut() {
            let count = *n;
            *n = sum;
            sum += count;
        }
        for i in 0..keys.len() {
            let id = if shift == 0 { i } else { scratch[i] as usize };
            let bucket = ((keys[id] >> shift) & 65535) as usize;
            if shift == 0 {
                scratch[counts[bucket] as usize] = id as u32;
            } else {
                order[counts[bucket] as usize] = id as u32;
            }
            counts[bucket] += 1;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counting_order_is_far_to_near_and_stable() {
        let p = [[0., 0., 2.], [0., 0., -1.], [0., 0., 2.], [0., 0., 0.]];
        let mut order = [0; 4];
        sort_indices(
            &p,
            [0., 0., 1., 0.],
            &mut [0; 4],
            &mut vec![0; 65536],
            &mut order,
            &mut [0; 4],
        );
        assert_eq!(order, [1, 3, 0, 2]);
        let p = [
            [0., 0., 1.000001],
            [0., 0., 1.],
            [0., 0., 1.],
            [0., 0., 10000.],
        ];
        sort_indices(
            &p,
            [0., 0., 1., 0.],
            &mut [0; 4],
            &mut vec![0; 65536],
            &mut order,
            &mut [0; 4],
        );
        assert_eq!(order, [1, 2, 0, 3]);
    }
    #[test]
    fn camera_rows_are_orthonormal() {
        let r = Camera {
            yaw: 1.17,
            pitch: -0.83,
            roll: 0.64,
            ..Camera::default()
        }
        .rows();
        for i in 0..3 {
            for j in 0..3 {
                let dot = (0..3).map(|k| r[i][k] * r[j][k]).sum::<f32>();
                assert!((dot - if i == j { 1. } else { 0. }).abs() < 1e-6);
            }
        }
    }
}
