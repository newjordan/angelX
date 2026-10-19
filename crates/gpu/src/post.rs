//! The stage's post pass (`post.wgsl`): what burns and flies blooms softly,
//! the corners fall into black, a film's grain lies in the middle tones and
//! the last step to eight bits is dithered; a big hit splits the colours a
//! few pixels and lifts the light. The Delve's look holds: black stays
//! black (the bloom has a threshold, the vignette and the rise multiply,
//! the grain and the dither leave black alone), the light is the light's
//! own colour, never a white blowout, and the pixel art is read texel for
//! texel, never filtered.
//!
//! It runs over a stage's world, before the screen's own marks (the HUD,
//! the bands) are laid over it: they stay as crisp and as dark as drawn.
//! Every number is fixed, picked by looking at the Delve's busiest fights;
//! a renderer that cannot afford it turns it off (`Core::set_post`).

use crate::Core;
use crate::frame::Frame;

/// What counts as light: the part of a colour's brightest channel over
/// this. The Delve's walls, floors and figures sit under it; its fires,
/// shots and magic over.
pub const BLOOM_THRESHOLD: f32 = 0.55;
/// How softly the threshold comes in, either side of it.
pub const BLOOM_KNEE: f32 = 0.2;
/// What a white counts for against a colour of the same brightness: a
/// knight's pale steel is not a flame.
pub const BLOOM_WHITE: f32 = 0.5;
/// How much of the stage's own light (its additive layers: fires' pools,
/// shots' glow, motes) blooms of itself, and the light at which what is
/// bright around it blooms fully (none where there is no light at all).
pub const BLOOM_LIGHT: f32 = 0.12;
pub const BLOOM_LIT: f32 = 0.18;
/// How much of the bloom is laid over the stage.
pub const BLOOM_STRENGTH: f32 = 0.6;
/// Each smaller level's weight as it is added back up the chain.
pub const BLOOM_SPREAD: f32 = 0.85;
/// How dark the corners go (0 none, 1 black).
pub const VIGNETTE: f32 = 0.22;
/// The canvas's grain in the middle tones: still, a stage pixel each.
pub const GRAIN: f32 = 0.022;
/// How far the colours split at a full impact, in stage-texture pixels at
/// the corners (half that halfway out, none in the middle).
pub const ABERRATION_PX: f32 = 3.0;
/// How much the light rises at a full impact.
pub const IMPACT_RISE: f32 = 0.22;

/// The bloom's levels: the stage halved, then halved again three times.
const LEVELS: usize = 4;
/// The passes: bright, three down, three up, the composite.
const PASSES: usize = 8;
/// Bytes of one pass's uniforms (`Post` in post.wgsl), and the stride they
/// are kept at in one buffer.
const POST_BYTES: u64 = 64;
const STRIDE: u64 = 256;

/// The uniforms of one pass.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostUniform {
    pub texel: [f32; 4],
    pub bloom: [f32; 4],
    pub look: [f32; 4],
    pub hit: [f32; 4],
}

/// The eight passes' uniforms for a stage texture of `size`, `grid` of its
/// pixels to one of the stage's own, at `impact` (0..1), in the order they
/// run.
pub fn uniforms(size: (u32, u32), grid: f32, impact: f32) -> [PostUniform; PASSES] {
    let grid = if grid.is_finite() { grid.max(1.0) } else { 1.0 };
    let impact = if impact.is_finite() { impact.clamp(0.0, 1.0) } else { 0.0 };
    let levels = level_sizes(size);
    let texel = |(w, h): (u32, u32), (tw, th): (u32, u32)| [1.0 / w.max(1) as f32, 1.0 / h.max(1) as f32, tw as f32, th as f32];
    let bloom = |weight: f32| [BLOOM_THRESHOLD, BLOOM_KNEE, BLOOM_WHITE, weight];
    let look = [VIGNETTE, GRAIN, impact, grid];
    let hit = [ABERRATION_PX, IMPACT_RISE, BLOOM_LIGHT, BLOOM_LIT];
    let pass = |source: (u32, u32), target: (u32, u32), weight: f32| PostUniform { texel: texel(source, target), bloom: bloom(weight), look, hit };
    [
        pass(size, levels[0], 0.0),
        pass(levels[0], levels[1], 0.0),
        pass(levels[1], levels[2], 0.0),
        pass(levels[2], levels[3], 0.0),
        pass(levels[3], levels[2], BLOOM_SPREAD),
        pass(levels[2], levels[1], BLOOM_SPREAD),
        pass(levels[1], levels[0], BLOOM_SPREAD),
        pass(size, size, BLOOM_STRENGTH),
    ]
}

/// The bloom's levels for a stage texture of `size`: half, a quarter, an
/// eighth and a sixteenth of it, never under a pixel.
pub fn level_sizes(size: (u32, u32)) -> [(u32, u32); LEVELS] {
    std::array::from_fn(|k| ((size.0 >> (k + 1)).max(1), (size.1 >> (k + 1)).max(1)))
}

/// Whether the post pass runs unless the host says otherwise:
/// `ANGEL_GLASS_POST=0` turns it off where there is an environment (a
/// window); the browser's page says so itself.
pub fn on_by_default() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("ANGEL_GLASS_POST").map_or(true, |v| v != "0")
    }
    #[cfg(target_arch = "wasm32")]
    {
        true
    }
}

/// Pipelines and the uniforms' buffer, made the first time a stage is
/// post-processed; the textures, made for a size and kept while it holds.
pub(crate) struct PostState {
    layout: wgpu::BindGroupLayout,
    bright: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    targets: Option<Targets>,
}

struct Targets {
    size: (u32, u32),
    /// The stage's world, drawn here and read by the passes.
    scene: wgpu::TextureView,
    /// Its lights alone (`Frame::lights_into`), at the first level's size.
    light: wgpu::TextureView,
    levels: [wgpu::TextureView; LEVELS],
    /// Each pass's bindings, in the order they run.
    groups: [wgpu::BindGroup; PASSES],
}

impl PostState {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glass-post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glass-post-bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(POST_BYTES),
                    },
                    count: None,
                },
                texture(1),
                texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glass-post-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = |entry: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let bright = pipeline("fs_bright", None);
        let down = pipeline("fs_down", None);
        let up = pipeline("fs_up", Some(wgpu::BlendState { color: add, alpha: add }));
        let composite = pipeline("fs_composite", None);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glass-post-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glass-post-uniforms"),
            size: STRIDE * PASSES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { layout, bright, down, up, composite, sampler, uniforms, targets: None }
    }

    /// Let the textures go (no stage is drawn now); they are made again
    /// for the next.
    pub(crate) fn release(&mut self) {
        self.targets = None;
    }

    /// The textures for a stage of `size`, made when missing or another size.
    fn targets(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat, size: (u32, u32)) -> &Targets {
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            self.targets = None;
            let make = |(w, h): (u32, u32), label| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let scene = make(size, "glass-post-scene");
            let sizes = level_sizes(size);
            let levels: [wgpu::TextureView; LEVELS] = std::array::from_fn(|k| make(sizes[k], "glass-post-level"));
            let light = make(sizes[0], "glass-post-light");
            // Each pass reads its source, and a second texture it is not
            // drawing into (only the composite uses it).
            let reads: [(&wgpu::TextureView, &wgpu::TextureView); PASSES] = [
                (&scene, &light),
                (&levels[0], &levels[3]),
                (&levels[1], &levels[0]),
                (&levels[2], &levels[0]),
                (&levels[3], &levels[0]),
                (&levels[2], &levels[0]),
                (&levels[1], &levels[3]),
                (&scene, &levels[0]),
            ];
            let groups = std::array::from_fn(|k| {
                let (source, glow) = reads[k];
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("glass-post-group"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &self.uniforms,
                                offset: STRIDE * k as u64,
                                size: wgpu::BufferSize::new(POST_BYTES),
                            }),
                        },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(source) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(glow) },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    ],
                })
            });
            self.targets = Some(Targets { size, scene, light, levels, groups });
        }
        self.targets.as_ref().expect("made above")
    }
}

impl Core {
    /// Whether stages are post-processed (`post.rs`); on unless the host
    /// turned it off.
    pub fn post(&self) -> bool {
        self.post_on
    }

    /// Turn the stages' post pass on or off (a weak device, or the player's
    /// choice). Off, a stage is drawn straight into its texture, as before
    /// there was one, and the post pass's textures are let go.
    pub fn set_post(&mut self, on: bool) {
        self.post_on = on;
        if !on && let Some(post) = self.post.as_mut() {
            post.release();
        }
    }

    /// A post-processed stage's frame into `target` (`size` pixels, which
    /// it covers whole), in one submission: its world into the pass's own
    /// texture, its lights alone (`Frame::lights_into`) at half that, the
    /// pass (`grid` of the target's pixels to a stage pixel, at `impact`)
    /// into `target`, and the screen's own marks over it.
    pub(crate) fn draw_post(&mut self, target: &wgpu::TextureView, size: (u32, u32), world: &Frame, screen: &Frame, grid: f32, impact: f32) {
        let mut lights = std::mem::take(&mut self.spare_lights);
        world.lights_into(&mut lights);
        self.write_frames(&[world, &lights, screen], size);
        let (device, format) = (&self.device, self.view_format);
        let post = self.post.get_or_insert_with(|| PostState::new(device, format));
        let values = uniforms(size, grid, impact);
        let mut bytes = [0u8; (STRIDE as usize) * PASSES];
        for (k, value) in values.iter().enumerate() {
            let at = k * STRIDE as usize;
            bytes[at..at + POST_BYTES as usize].copy_from_slice(bytemuck::bytes_of(value));
        }
        self.queue.write_buffer(&post.uniforms, 0, &bytes);
        post.targets(device, format, size);
        let post = self.post.as_ref().expect("made above");
        let targets = post.targets.as_ref().expect("made above");
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("glass-post") });
        let black = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        let pass = begin_pass;
        let (world_n, lights_n) = (world.instances.len() as u32, lights.instances.len() as u32);
        self.encode_at(&mut pass(&mut encoder, &targets.scene, black), world, 0);
        self.encode_at(&mut pass(&mut encoder, &targets.light, black), &lights, world_n);
        let steps: [(&wgpu::RenderPipeline, &wgpu::TextureView, wgpu::LoadOp<wgpu::Color>); PASSES] = [
            (&post.bright, &targets.levels[0], black),
            (&post.down, &targets.levels[1], black),
            (&post.down, &targets.levels[2], black),
            (&post.down, &targets.levels[3], black),
            (&post.up, &targets.levels[2], wgpu::LoadOp::Load),
            (&post.up, &targets.levels[1], wgpu::LoadOp::Load),
            (&post.up, &targets.levels[0], wgpu::LoadOp::Load),
            (&post.composite, target, black),
        ];
        for (k, (pipeline, view, load)) in steps.into_iter().enumerate() {
            let mut pass = pass(&mut encoder, view, load);
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &targets.groups[k], &[]);
            pass.draw(0..3, 0..1);
        }
        if !screen.instances.is_empty() {
            self.encode_at(&mut pass(&mut encoder, target, wgpu::LoadOp::Load), screen, world_n + lights_n);
        }
        self.queue.submit([encoder.finish()]);
        self.spare_lights = lights;
    }
}

/// A pass over all of `view`, loading it as `load` says.
fn begin_pass<'e>(encoder: &'e mut wgpu::CommandEncoder, view: &wgpu::TextureView, load: wgpu::LoadOp<wgpu::Color>) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("glass-post"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uniforms_are_the_shaders_size() {
        assert_eq!(std::mem::size_of::<PostUniform>() as u64, POST_BYTES);
        assert!(POST_BYTES <= STRIDE && STRIDE % 256 == 0, "each pass's slice at a legal offset");
    }

    #[test]
    fn the_levels_halve_and_never_vanish() {
        assert_eq!(level_sizes((1152, 672)), [(576, 336), (288, 168), (144, 84), (72, 42)]);
        assert_eq!(level_sizes((3, 1)), [(1, 1), (1, 1), (1, 1), (1, 1)]);
    }

    #[test]
    fn the_numbers_keep_black_black_and_light_its_own() {
        // A threshold the dark palette sits under, a knee inside it, and a
        // bloom that adds light rather than replacing the picture.
        assert!(BLOOM_THRESHOLD - BLOOM_KNEE > 0.3, "the walls and floors never bloom");
        assert!(BLOOM_THRESHOLD + BLOOM_KNEE < 1.0, "a flame does");
        assert!((0.0..1.0).contains(&BLOOM_WHITE), "white counts less than a colour");
        assert!(BLOOM_STRENGTH > 0.0 && BLOOM_STRENGTH <= 1.0, "screened over, at most once");
        assert!((0.0..0.5).contains(&VIGNETTE), "the corners darken, never go out");
        assert!(GRAIN < 0.05, "a grain, not a noise");
        assert!(ABERRATION_PX <= 4.0 && IMPACT_RISE < 0.5, "a hit is felt, not a blur");
    }

    #[test]
    fn each_pass_is_told_its_source_and_its_weight() {
        let u = uniforms((1152, 672), 3.0, 0.0);
        // The bright pass reads the stage, a texel of it.
        assert_eq!(u[0].texel, [1.0 / 1152.0, 1.0 / 672.0, 576.0, 336.0]);
        // Down passes weigh nothing; up passes spread; the composite lays
        // the bloom at its strength over the stage's own size.
        assert!(u[1..4].iter().all(|p| p.bloom[3] == 0.0));
        assert!(u[4..7].iter().all(|p| p.bloom[3] == BLOOM_SPREAD));
        assert_eq!(u[7].bloom[3], BLOOM_STRENGTH);
        assert_eq!(&u[7].texel[2..], &[1152.0, 672.0]);
        assert_eq!(u[7].look, [VIGNETTE, GRAIN, 0.0, 3.0]);
    }

    #[test]
    fn an_impact_is_held_to_one_and_a_bad_one_or_grid_is_none() {
        assert_eq!(uniforms((64, 64), 1.0, 5.0)[7].look[2], 1.0);
        assert_eq!(uniforms((64, 64), 1.0, -1.0)[7].look[2], 0.0);
        assert_eq!(uniforms((64, 64), 1.0, f32::NAN)[7].look[2], 0.0);
        // A grid under a pixel, or none, is a pixel.
        assert_eq!(uniforms((64, 64), 0.5, 0.0)[7].look[3], 1.0);
        assert_eq!(uniforms((64, 64), f32::INFINITY, 0.0)[7].look[3], 1.0);
    }
}
