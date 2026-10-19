//! The renderer core: the GPU half of angelX's compositor, with no window,
//! no fonts on disk and no clock of its own (docs/GLASS_CORE.md).
//!
//! The host makes the wgpu `Device` and `Queue` (and, for a window, the
//! surface) and hands them over; [`Core`] draws one pipeline of instanced
//! quads into whatever texture it is given: solid rectangles, masks and
//! colour images from a packed atlas, and whole pictures. Around that it
//! keeps the pictures the host uploads (under a budget), the stages the
//! host sets (a game's layers, drawn into their own pictures and slid in
//! when the scene changes), the background image, and the layer compiler
//! that turns `angel_stage` layers into instances.
//!
//! Beside the quads is a mesh pipeline with a depth buffer: [`Core::draw_scene`]
//! draws an `angel_stage::scene::Scene` (nodes with transforms, meshes and
//! unlit materials under a camera) into any colour view, and
//! [`Core::set_scene_target`] draws one into a picture every frame, which a
//! layer then places like any other. Meshes are built in or uploaded
//! ([`Core::upload_mesh`]); a material's pictures are the ones
//! [`Core::upload`] keeps.
//!
//! What the core does not do, so that the browser can link it too: open a
//! window, find a font, read a file or a clock, or start a thread. Wherever
//! it needs the time (a slide, a breath, a flame, drifting motes) the host
//! passes it in, as seconds since a start of its own choosing. It names no
//! wgpu backend either: the window's crate turns on Vulkan and Metal, and
//! the `web` feature turns on the browser's.

use std::collections::HashMap;

mod atlas;
mod draw;
mod frame;
mod hash;
mod layers;
mod look;
mod picture;
mod post;
mod scene;
mod stage;
mod wallpaper;

pub use atlas::Entry;
pub use frame::{COLOR, Draw, Frame, Instance, MASK, MESH_TRIANGLE, PARCHMENT, PICTURE, PICTURE_PREMULTIPLIED, PICTURE_STRAIGHT, PRIMITIVE};
pub use hash::{FastMap, FxHasher};
pub use picture::{Picture, PictureInfo, premultiply_in_place, well_formed};
pub use post::{ABERRATION_PX, BLOOM_KNEE, BLOOM_STRENGTH, BLOOM_THRESHOLD, BLOOM_WHITE, GRAIN, IMPACT_RISE, PostUniform, VIGNETTE};
pub use stage::{MAX_TARGET_PX, Stage, scaled_layers, target_size};
pub use wallpaper::{Layout, Wallpaper};

use atlas::{Atlas, AtlasHealth};
use draw::{bind, instance_buffer};
use picture::GpuPicture;
use wallpaper::GpuWallpaper;

/// The renderer: a device and queue the host made, the pipeline that draws
/// every frame, and everything it keeps between frames.
pub struct Core {
    device: wgpu::Device,
    queue: wgpu::Queue,
    view_format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    nearest: wgpu::Sampler,
    linear: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    pictures: FastMap<u32, GpuPicture>,
    picture_bytes: usize,
    /// ANGEL_GLASS_GPU_PREMULTIPLY: uploaded pictures stay straight in
    /// storage and the shader premultiplies (`PICTURE_STRAIGHT`). It
    /// spares the producer thread the CPU scan; off by default.
    gpu_premultiply: bool,
    /// Stages drawn into their pictures every frame, by picture id.
    stages: HashMap<u32, Stage>,
    /// The scene each stage last drew, and the slides under way (when one
    /// began, on the host's clock, and the way the new scene comes in).
    stage_scenes: HashMap<u32, u64>,
    slides: HashMap<u32, (f64, [f32; 2])>,
    frame_number: u64,
    /// Meshes, depth buffers and scene targets (`scene.rs`).
    three: scene::SceneState,
    wallpaper: Option<GpuWallpaper>,
    /// The soft shadow under lifted layers, once drawn into the atlas.
    shadow_entry: Option<Entry>,
    /// A light's falloff, once drawn into the atlas.
    light_entry: Option<Entry>,
    /// When the host's clock first showed a breathing layer: a breath
    /// starts from nothing there.
    breath_origin: Option<f64>,
    text_gamma: f32,
    globals: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: usize,
    atlas: Atlas,
    atlas_health: AtlasHealth,
    /// A finished frame's buffers, kept for the next frame to refill.
    spare_frame: Frame,
    /// Emptied instance buffers kept for the next build.
    spare_vecs: Vec<Vec<Instance>>,
    /// A post-processed stage's screen marks, drawn over its world after
    /// the post pass: the frame's buffers kept as `spare_frame`'s are.
    spare_screen: Frame,
    /// And its lights alone, which the post pass blooms around.
    spare_lights: Frame,
    /// The stages' post pass (`post.rs`), made when first used, and whether
    /// it runs.
    post: Option<post::PostState>,
    post_on: bool,
}

impl Core {
    /// A renderer on `device` and `queue`, drawing into textures of
    /// `view_format`. `text_gamma` is the text coverage curve (1 = as
    /// rasterized).
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        view_format: wgpu::TextureFormat,
        text_gamma: f32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glass-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("glass.wgsl").into()),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glass-bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glass-layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glass-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Uint32,
                        4 => Float32x2, 5 => Float32, 6 => Float32x4, 7 => Float32x4
                    ],
                })],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: view_format,
                    blend: Some(wgpu::BlendState {
                        color: premultiplied,
                        alpha: premultiplied,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glass-globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atlas = Atlas::new(&device);
        let sampler = |filter: wgpu::FilterMode| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("glass-picture-sampler"),
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let nearest = sampler(wgpu::FilterMode::Nearest);
        let linear = sampler(wgpu::FilterMode::Linear);
        let bind_group = bind(&device, &bind_layout, &globals, &atlas.view, &nearest);
        let capacity = 16 * 1024;
        let instances = instance_buffer(&device, capacity);
        Self {
            device,
            queue,
            view_format,
            pipeline,
            bind_layout,
            nearest,
            linear,
            bind_group,
            pictures: FastMap::default(),
            picture_bytes: 0,
            gpu_premultiply: std::env::var("ANGEL_GLASS_GPU_PREMULTIPLY").is_ok_and(|v| v == "1"),
            stages: HashMap::new(),
            stage_scenes: HashMap::new(),
            slides: HashMap::new(),
            frame_number: 0,
            three: scene::SceneState::default(),
            wallpaper: None,
            shadow_entry: None,
            light_entry: None,
            breath_origin: None,
            text_gamma,
            globals,
            instances,
            capacity,
            atlas,
            atlas_health: AtlasHealth::default(),
            spare_frame: Frame::default(),
            spare_vecs: Vec::new(),
            spare_screen: Frame::default(),
            spare_lights: Frame::default(),
            post: None,
            post_on: post::on_by_default(),
        }
    }

    /// Start a frame: pictures drawn in it are the ones eviction spares.
    pub fn begin_frame(&mut self) {
        self.frame_number += 1;
    }

    /// Whether a stage's slide is under way, or one of its layers breathes,
    /// flickers or drifts: it needs frames though nothing else changes.
    pub fn stages_animating(&self) -> bool {
        !self.slides.is_empty()
            || self.stages.values().any(|s| s.layers.iter().any(angel_stage::Layer::animating))
    }

    /// A finished frame's buffers, for the next frame to refill. Give them
    /// back with [`Core::keep_frame`].
    pub fn take_frame(&mut self) -> Frame {
        std::mem::take(&mut self.spare_frame)
    }

    /// Keep a frame's buffers for the next one.
    pub fn keep_frame(&mut self, frame: Frame) {
        self.spare_frame = frame;
    }

    /// The emptied instance buffers kept for the next build, for it to pop
    /// its working buffers from. Give them back with [`Core::keep_vecs`].
    pub fn take_vecs(&mut self) -> Vec<Vec<Instance>> {
        std::mem::take(&mut self.spare_vecs)
    }

    /// Keep emptied instance buffers for the next build.
    pub fn keep_vecs(&mut self, vecs: Vec<Vec<Instance>>) {
        self.spare_vecs = vecs;
    }
}
