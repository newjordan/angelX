//! The world's look on the GPU (docs/GLASS_CORE.md, "The world's look"): a
//! scene with an `angel_stage::look::Look` is drawn at the look's internal
//! size, then a post pass puts every pixel on its ramp.
//!
//! The scene's own draw (`scene.rs`) goes into two targets at once through the
//! mesh pipeline's `fs_look` entry: a colour texture, and an R8 mask in which
//! an accent surface is 1. That is multiple render targets, which WebGPU and
//! WebGL2 (through wgpu) both have, with no extra pass. The fog is by
//! distance from the eye, in the same fragment shader. Then `look.wgsl`, a
//! fullscreen pass, reads both with `textureLoad`: it quantizes a pixel by
//! its luminance and the Bayer threshold at its internal place, keeps accents
//! as the nearest accent, and writes the result into the scene's output at
//! the output's size by nearest, which is 1:1 for a scene's own picture (the
//! layer that shows it then scales it up nearest, as it does any picture
//! shown bigger than it is) and a whole-pixel enlargement when the output is
//! a bigger view.
//!
//! The maths is `Look::quantize` in angel-stage, line for line; the uniform
//! below is what the shader is handed of it.
//!
//! A look that lights its scene itself (`Look::lights`: a sun, lamps, zones)
//! is placed on its ramps in `fs_look` instead, as each surface is drawn:
//! there the pixel's world point is known, so its zone's palette, its
//! shadow and its lamps' warmth can choose its ramp. The mask then says 0.5
//! ("done") and the post pass only enlarges it. The sun's shadows come from
//! a depth map drawn from the light just before the scene (`shadow_pass`).

use std::num::NonZeroU64;

use angel_stage::look::{Look, unit_rgb};
use bytemuck::{Pod, Zeroable};

/// The mask's format: one 8-bit channel is 1 where an accent was drawn.
pub(crate) const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// Internal target sets kept: one per size drawn lately.
const TARGETS_KEPT: usize = 2;

/// What `look.wgsl`'s `Look` is: the ramp with each entry's luminance, the
/// accents, the counts, the sizes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(crate) struct LookUniform {
    /// rgb 0..1, and the entry's luminance (never falling) in w.
    pub ramp: [[f32; 4]; 16],
    pub accent: [[f32; 4]; 4],
    /// Ramp entries, accents, Bayer bits per coordinate, 0.
    pub counts: [u32; 4],
    /// The internal size, then the output's.
    pub sizes: [f32; 4],
}

/// The shadow map's side, in texels.
pub(crate) const SHADOW_TEXELS: u32 = 2048;

/// The shadow map's format.
pub(crate) const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// What mesh.wgsl's `SceneLook` is: the eye, the fog, the fog's colour, and
/// for a look that lights its scene (`Look::lights`) its sun, shadow, ramps,
/// lamps and zones.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(crate) struct SceneLookUniform {
    /// xyz: the eye; w: 1 when the look lights the scene.
    pub eye: [f32; 4],
    /// Start, end, 1 when there is fog, the clock in seconds.
    pub fog: [f32; 4],
    /// rgb: the fog's colour; w: which ramp entry it is.
    pub fog_color: [f32; 4],
    /// xyz: toward the sun (unit), w: its strength.
    pub sun: [f32; 4],
    /// x: the ambient, y: one shadow texel's side in uv, z: 1 with a water
    /// ramp, w: the height the sky's light rises over (0: none).
    pub sun_more: [f32; 4],
    /// World to the shadow map's clip space.
    pub shadow: [[f32; 4]; 4],
    /// Ramp entries, zones, Bayer bits per coordinate, accents.
    pub counts: [u32; 4],
    /// 1 with a warm ramp, 1 with shadows, lamps, 1 with a sun.
    pub flags: [u32; 4],
    /// The ramp, rgb 0..1.
    pub ramp: [[f32; 4]; 16],
    /// The warm ramp, rgb 0..1.
    pub warm: [[f32; 4]; 16],
    pub accent: [[f32; 4]; 4],
    /// Each lamp in two: xyz where, w its reach; then x its strength.
    pub lamps: [[f32; 4]; 32],
    /// Each zone in two: x and z of its middle, its radius, its feather;
    /// then its box's half width and depth.
    pub zones: [[f32; 4]; 8],
    /// Each zone's ramp, 16 entries a zone.
    pub zone_ramps: [[f32; 4]; 64],
    /// The water's ramp, rgb 0..1.
    pub water: [[f32; 4]; 16],
    /// Each point kept in sight: xyz, w the opening's radius (0: none).
    pub reveal: [[f32; 4]; 4],
    /// x: world units in a metre.
    pub units: [f32; 4],
}

/// The post pass's uniform for a look (already normalized), drawn at
/// `internal` pixels into an output of `output` pixels.
pub(crate) fn look_uniform(look: &Look, internal: (u32, u32), output: (u32, u32)) -> LookUniform {
    let mut uniform = LookUniform::zeroed();
    let levels = look.ramp_levels();
    for (i, (&c, &level)) in look.ramp.iter().zip(&levels).enumerate().take(16) {
        let [r, g, b] = unit_rgb(c);
        uniform.ramp[i] = [r, g, b, level];
    }
    for (i, &c) in look.accent.iter().enumerate().take(4) {
        let [r, g, b] = unit_rgb(c);
        uniform.accent[i] = [r, g, b, 0.0];
    }
    uniform.counts = [look.ramp.len().min(16) as u32, look.accent.len().min(4) as u32, look.dither.bits(), 0];
    uniform.sizes = [internal.0 as f32, internal.1 as f32, output.0 as f32, output.1 as f32];
    uniform
}

/// The mesh pipeline's per-scene uniform for a look and an eye.
pub(crate) fn scene_look_uniform(look: &Look, eye: [f32; 3]) -> SceneLookUniform {
    let mut uniform = SceneLookUniform::zeroed();
    let lights = look.lights();
    uniform.eye = [eye[0], eye[1], eye[2], if lights { 1.0 } else { 0.0 }];
    uniform.fog[3] = look.time;
    if let (Some(fog), Some(rgb)) = (look.fog, look.fog_rgb()) {
        uniform.fog = [fog.start, fog.end, 1.0, look.time];
        uniform.fog_color = [rgb[0], rgb[1], rgb[2], fog.color as f32];
    }
    if !lights {
        return uniform;
    }
    for (i, r) in look.reveal.iter().chain(&look.reveals).enumerate().take(4) {
        uniform.reveal[i] = [r.at[0], r.at[1], r.at[2], r.radius];
    }
    uniform.units = [look.metre, 0.0, 0.0, 0.0];
    let rgb4 = |c: [u8; 3]| {
        let [r, g, b] = unit_rgb(c);
        [r, g, b, 0.0]
    };
    for (i, &c) in look.ramp.iter().enumerate().take(16) {
        uniform.ramp[i] = rgb4(c);
    }
    if let Some(warm) = &look.warm {
        for (i, &c) in warm.iter().enumerate().take(16) {
            uniform.warm[i] = rgb4(c);
        }
    }
    if let Some(water) = &look.water {
        for (i, &c) in water.iter().enumerate().take(16) {
            uniform.water[i] = rgb4(c);
        }
        uniform.sun_more[2] = 1.0;
    }
    for (i, &c) in look.accent.iter().enumerate().take(4) {
        uniform.accent[i] = rgb4(c);
    }
    for (i, lamp) in look.lamps.iter().enumerate().take(16) {
        uniform.lamps[2 * i] = [lamp.at[0], lamp.at[1], lamp.at[2], lamp.reach];
        uniform.lamps[2 * i + 1] = [lamp.strength, 0.0, 0.0, 0.0];
    }
    for (i, zone) in look.zones.iter().enumerate().take(4) {
        uniform.zones[2 * i] = [zone.at[0], zone.at[1], zone.radius, zone.feather];
        uniform.zones[2 * i + 1] = [zone.half[0], zone.half[1], 0.0, 0.0];
        for (k, &c) in zone.ramp.iter().enumerate().take(16) {
            uniform.zone_ramps[i * 16 + k] = rgb4(c);
        }
    }
    let shadow = look.sun.and_then(|sun| sun.shadow_matrix(SHADOW_TEXELS));
    if let Some(sun) = look.sun {
        let t = sun.toward_unit();
        uniform.sun = [t[0], t[1], t[2], sun.strength];
        uniform.sun_more[0] = sun.ambient;
        uniform.sun_more[1] = 1.0 / SHADOW_TEXELS as f32;
        uniform.sun_more[3] = sun.rise;
    }
    if let Some(m) = shadow {
        uniform.shadow = m;
    }
    uniform.counts = [
        look.ramp.len().min(16) as u32,
        look.zones.len().min(4) as u32,
        look.dither.bits(),
        look.accent.len().min(4) as u32,
    ];
    uniform.flags = [
        u32::from(look.warm.is_some()),
        u32::from(shadow.is_some()),
        look.lamps.len().min(16) as u32,
        u32::from(look.sun.is_some()),
    ];
    uniform
}

/// A colour and a mask texture of one internal size, and the post pass's group
/// that reads them.
struct Targets {
    size: (u32, u32),
    color: wgpu::TextureView,
    mask: wgpu::TextureView,
    group: wgpu::BindGroup,
    last_used: u64,
}

/// What the look pass keeps between frames, made when a scene with a look is
/// first drawn, so a host with none pays nothing for it.
pub(crate) struct LookState {
    /// Group 3 of the mesh pipeline: the scene's eye and fog, its lights,
    /// and the sun's shadow map with the sampler that compares against it.
    pub(crate) scene_layout: wgpu::BindGroupLayout,
    pub(crate) scene_group: wgpu::BindGroup,
    scene_buffer: wgpu::Buffer,
    /// Group 3 of the shadow pipeline: the same buffer, without the map it
    /// is drawing into.
    pub(crate) shadow_layout: wgpu::BindGroupLayout,
    pub(crate) shadow_group: wgpu::BindGroup,
    /// The shadow map: one texel until a sun with shadows asks for it.
    shadow_map: wgpu::TextureView,
    shadow_size: u32,
    shadow_sampler: wgpu::Sampler,
    /// The depth-only pipeline that draws the shadow map: indexed by
    /// `double_sided`.
    pub(crate) shadow: Option<[wgpu::RenderPipeline; 2]>,
    /// The depth-only pipeline that draws a lit scene's depth with its
    /// reveal's openings left out (`DepthUse::Revealed`).
    pub(crate) reveal_depth: Option<[wgpu::RenderPipeline; 2]>,
    post_layout: wgpu::BindGroupLayout,
    post: wgpu::RenderPipeline,
    post_buffer: wgpu::Buffer,
    /// The mesh pipeline for the internal targets: indexed by `double_sided`.
    pub(crate) mesh: Option<[wgpu::RenderPipeline; 2]>,
    targets: Vec<Targets>,
}

/// A shadow map `side` texels square, drawn into and then read.
fn shadow_texture(device: &wgpu::Device, side: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("glass-look-shadow-map"),
            size: wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn scene_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    shadow_map: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("glass-look-scene"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(shadow_map) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

impl LookState {
    /// The shadow map at its full size, made the first time a sun with
    /// shadows is drawn (the scene's group then names it).
    pub(crate) fn shadow_map(&mut self, device: &wgpu::Device) -> wgpu::TextureView {
        if self.shadow_size != SHADOW_TEXELS {
            self.shadow_map = shadow_texture(device, SHADOW_TEXELS);
            self.shadow_size = SHADOW_TEXELS;
            self.scene_group =
                scene_group(device, &self.scene_layout, &self.scene_buffer, &self.shadow_map, &self.shadow_sampler);
        }
        self.shadow_map.clone()
    }

    pub(crate) fn new(device: &wgpu::Device, view_format: wgpu::TextureFormat) -> Self {
        let uniform = |size: u64, visibility: wgpu::ShaderStages| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(size),
            },
            count: None,
        };
        let scene_size = std::mem::size_of::<SceneLookUniform>() as u64;
        let post_size = std::mem::size_of::<LookUniform>() as u64;
        let both = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glass-look-scene"),
            entries: &[
                uniform(scene_size, both),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glass-look-shadow"),
            entries: &[uniform(scene_size, both)],
        });
        let buffer = |label: &str, size: u64| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let scene_buffer = buffer("glass-look-scene", scene_size);
        let post_buffer = buffer("glass-look-post", post_size);
        // Comparing a depth against the map, the four nearest texels
        // filtered: the hardware's own soft edge, one texel wide.
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glass-look-shadow"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_map = shadow_texture(device, 1);
        let scene_group = scene_group(device, &scene_layout, &scene_buffer, &shadow_map, &shadow_sampler);
        let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glass-look-shadow"),
            layout: &shadow_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: scene_buffer.as_entire_binding() }],
        });
        let texture_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let mut post_entries = vec![uniform(post_size, wgpu::ShaderStages::FRAGMENT)];
        post_entries.extend([texture_entry(1), texture_entry(2)]);
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glass-look-post"),
            entries: &post_entries,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glass-look-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("look.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glass-look-layout"),
            bind_group_layouts: &[Some(&post_layout)],
            immediate_size: 0,
        });
        // Opaque pixels replace; the clear pixels are transparent and add
        // nothing: the same blend as everything else the core draws.
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let post = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glass-look-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: view_format,
                    blend: Some(wgpu::BlendState { color: premultiplied, alpha: premultiplied }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            scene_layout,
            scene_group,
            scene_buffer,
            shadow_layout,
            shadow_group,
            shadow_map,
            shadow_size: 1,
            shadow_sampler,
            shadow: None,
            reveal_depth: None,
            post_layout,
            post,
            post_buffer,
            mesh: None,
            targets: Vec::new(),
        }
    }

    /// The internal colour and mask views of `size`, made when missing (the
    /// least recently used size goes when there are too many), with this
    /// scene's uniforms written ahead of the submission that uses them.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view_format: wgpu::TextureFormat,
        look: &Look,
        eye: [f32; 3],
        size: (u32, u32),
        output: (u32, u32),
        frame: u64,
    ) -> (wgpu::TextureView, wgpu::TextureView) {
        queue.write_buffer(
            &self.scene_buffer,
            0,
            bytemuck::bytes_of(&scene_look_uniform(look, eye)),
        );
        queue.write_buffer(
            &self.post_buffer,
            0,
            bytemuck::bytes_of(&look_uniform(look, size, output)),
        );
        if let Some(found) = self.targets.iter_mut().find(|t| t.size == size) {
            found.last_used = frame;
            return (found.color.clone(), found.mask.clone());
        }
        if self.targets.len() >= TARGETS_KEPT
            && let Some(oldest) = self
                .targets
                .iter()
                .enumerate()
                .min_by_key(|(_, t)| t.last_used)
                .map(|(i, _)| i)
        {
            self.targets.swap_remove(oldest);
        }
        let texture = |label: &str, format: wgpu::TextureFormat| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let color = texture("glass-look-colour", view_format);
        let mask = texture("glass-look-mask", MASK_FORMAT);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glass-look-post"),
            layout: &self.post_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.post_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&color) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&mask) },
            ],
        });
        self.targets.push(Targets { size, color: color.clone(), mask: mask.clone(), group, last_used: frame });
        (color, mask)
    }

    /// Record the post pass: the internal targets of `size` quantized into
    /// `view`. The view is cleared to nothing first when `clear` is set, and
    /// drawn over when it is not.
    pub(crate) fn post_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        size: (u32, u32),
        clear: bool,
        label: &str,
    ) {
        let Some(targets) = self.targets.iter().find(|t| t.size == size) else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: if clear { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.post);
        pass.set_bind_group(0, &targets.group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_stage::look::{Dither, Fog};

    fn look() -> Look {
        let mut look = Look::new(vec![[0x1e, 0x24, 0x31], [0x44, 0x4e, 0x6f], [0x8a, 0x93, 0xc7]]);
        look.accent = vec![[0xf0, 0xe0, 0x88]];
        look.fog = Some(Fog { color: 1, start: 4.0, end: 40.0 });
        look
    }

    #[test]
    fn the_uniforms_are_what_the_shaders_declare() {
        // look.wgsl: 16 + 4 vec4s, a vec4<u32>, a vec4<f32>.
        assert_eq!(std::mem::size_of::<LookUniform>(), 16 * 16 + 4 * 16 + 16 + 16);
        // mesh.wgsl: five vec4s, a matrix, two vec4<u32>s, then the ramps,
        // accents, lamps, zones, the zones' ramps, the water's and the reveal.
        assert_eq!(std::mem::size_of::<SceneLookUniform>(), (5 + 4 + 2 + 16 + 16 + 4 + 32 + 8 + 64 + 16 + 4 + 1) * 16);
        assert_eq!(std::mem::size_of::<LookUniform>() % 16, 0);
    }

    #[test]
    fn a_look_goes_to_the_shader_as_its_ramp_levels_accents_and_sizes() {
        let l = look();
        let u = look_uniform(&l, (960, 540), (1920, 1080));
        assert_eq!(u.counts, [3, 1, 2, 0]);
        assert_eq!(u.sizes, [960.0, 540.0, 1920.0, 1080.0]);
        // Each entry's colour, with its luminance from the reference.
        let levels = l.ramp_levels();
        for (i, level) in levels.iter().enumerate() {
            let [r, g, b] = unit_rgb(l.ramp[i]);
            assert_eq!(u.ramp[i], [r, g, b, *level]);
        }
        assert!(u.ramp[0][3] < u.ramp[1][3] && u.ramp[1][3] < u.ramp[2][3]);
        // The rest is empty; the accent is its colour.
        assert_eq!(u.ramp[3], [0.0; 4]);
        assert_eq!(u.accent[0][..3], unit_rgb([0xf0, 0xe0, 0x88]));
        assert_eq!(u.accent[1], [0.0; 4]);
        // No dither is no bits; the finer weave is three.
        let mut none = l.clone();
        none.dither = Dither::None;
        assert_eq!(look_uniform(&none, (8, 8), (8, 8)).counts[2], 0);
        none.dither = Dither::Bayer8;
        assert_eq!(look_uniform(&none, (8, 8), (8, 8)).counts[2], 3);
    }

    #[test]
    fn the_fog_goes_to_the_mesh_shader_as_its_colour_and_range() {
        let l = look();
        let u = scene_look_uniform(&l, [1.0, 2.0, 3.0]);
        assert_eq!(u.eye, [1.0, 2.0, 3.0, 0.0]);
        assert_eq!(u.fog, [4.0, 40.0, 1.0, 0.0]);
        assert_eq!(u.fog_color[3], 1.0);
        assert_eq!(u.fog_color[..3], unit_rgb([0x44, 0x4e, 0x6f]));
        let mut clear = l;
        clear.fog = None;
        assert_eq!(scene_look_uniform(&clear, [0.0; 3]).fog, [0.0; 4]);
    }

    #[test]
    fn a_look_that_lights_goes_to_the_mesh_shader_whole() {
        use angel_stage::look::{Lamp, Reveal, Shadow, Sun, Zone};
        let mut l = look();
        assert_eq!(scene_look_uniform(&l, [0.0; 3]).eye[3], 0.0, "a plain look is placed afterwards");
        l.sun = Some(Sun { toward: [0.0, 2.0, 0.0], strength: 0.8, ambient: 0.3, shadow: Some(Shadow { focus: [0.0; 3], reach: 50.0 }), rise: 30.0 });
        l.lamps = vec![Lamp { at: [1.0, 2.0, 3.0], reach: 9.0, strength: 0.7 }];
        l.warm = Some(vec![[10, 0, 0], [100, 50, 0], [250, 200, 100]]);
        l.water = Some(vec![[0, 10, 20], [0, 60, 90], [100, 200, 240]]);
        l.zones = vec![Zone { ramp: vec![[5, 5, 5], [60, 60, 60], [200, 200, 200]], at: [4.0, -4.0], half: [2.0, 3.0], radius: 1.0, feather: 5.0 }];
        l.reveal = Some(Reveal { at: [7.0, 8.0, 9.0], radius: 6.0 });
        let u = scene_look_uniform(&l, [1.0, 2.0, 3.0]);
        assert_eq!(u.eye, [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(u.counts, [3, 1, 2, 1]);
        assert_eq!(u.flags, [1, 1, 1, 1]);
        assert_eq!(u.sun, [0.0, 1.0, 0.0, 0.8]);
        assert_eq!(u.sun_more, [0.3, 1.0 / SHADOW_TEXELS as f32, 1.0, 30.0]);
        assert_ne!(u.shadow, [[0.0; 4]; 4]);
        assert_eq!(u.lamps[0], [1.0, 2.0, 3.0, 9.0]);
        assert_eq!(u.lamps[1][0], 0.7);
        assert_eq!(u.zones[0], [4.0, -4.0, 1.0, 5.0]);
        assert_eq!(u.zones[1][..2], [2.0, 3.0]);
        assert_eq!(u.zone_ramps[1][..3], unit_rgb([60, 60, 60]));
        assert_eq!(u.warm[2][..3], unit_rgb([250, 200, 100]));
        assert_eq!(u.water[1][..3], unit_rgb([0, 60, 90]));
        assert_eq!(u.reveal[0], [7.0, 8.0, 9.0, 6.0]);
        assert_eq!(u.reveal[1], [0.0; 4]);
    }
}
