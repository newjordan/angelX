//! 3D scenes on the GPU (docs/GLASS_CORE.md, step 2.1): `angel_stage::scene`
//! drawn with a mesh pipeline, a depth buffer and a material that is unlit or
//! lit by one fixed light, into any
//! colour view or into a picture the layer system then shows anywhere. A
//! material may cast its picture from a projector instead of laying it on
//! by uv (a painted plate thrown back onto its blocks, docs/WORLD-3D.md).
//!
//! One draw is one range of one mesh with one material. Opaque draws go
//! first, in node order, writing depth. Blended draws (straight or
//! premultiplied alpha) follow, farthest node centre first, testing depth
//! without writing it. A cube with a picture on each face is six draws of
//! one mesh.
//!
//! Built-in meshes (quad, cube, plane, spheres) are made once, on first use.
//! A host's own go up with [`Core::upload_mesh`] under a budget, as pictures
//! do. Pictures a material names are the ones [`Core::upload`] keeps, and
//! drawing them marks them used, so eviction spares them.

use std::collections::HashMap;
use std::num::NonZeroU64;

use angel_stage::scene::{
    AlphaMode, Look, Mat4, Material, MeshData, MeshError, MeshRef, Role, SPHERE_SEGMENTS, Sampling, Scene,
    TextureSource, mat_finite, mat_point,
};
use angel_stage::primitives::{FENCE_POSTS, RING_SEGMENTS, STAIR_STEPS};
use bytemuck::{Pod, Zeroable};

use crate::look::{LookState, MASK_FORMAT};
use crate::picture::eviction_order;
use crate::stage::MAX_TARGET_PX;
use crate::{Core, FastMap};

/// The depth buffer's format.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Uploaded meshes kept at most, in bytes of vertices and indices; the least
/// recently drawn go first.
const MESH_BUDGET: usize = 128 << 20;

/// The most uploaded meshes kept at once, whatever their size.
const MESH_COUNT: usize = 4096;

/// The depth buffers kept: one per size drawn into lately (a window, a
/// stage's props and the scene targets). A resize makes the new size's and lets the old one go.
const DEPTH_TARGETS: usize = 4;

/// The most draws one scene makes.
const MAX_DRAWS: usize = 65_536;

/// One vertex as the GPU has it: position, normal, uv, colour.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeshVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
}

/// What one draw hands the shader (`mesh.wgsl`'s `Draw`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DrawUniform {
    model: Mat4,
    view_proj: Mat4,
    tint: [f32; 4],
    flags: [u32; 4],
    /// x: emissive; yzw: a projected material's patch's rectangle in its
    /// picture (`Patch::rect`): x, y, and `width << 8 | height`.
    light: [f32; 4],
    /// The projector a projected material casts its picture from (world to
    /// clip), `flags.w` 1; zero for one that is not.
    projector: Mat4,
    /// A projected material's patch (`Material::patch`, group 2's picture):
    /// where its corner lies in the world, and a pixel's size there; all 0
    /// for none (or one whose picture is not on the GPU).
    patch: [f32; 4],
}

const DRAW_BYTES: u64 = std::mem::size_of::<DrawUniform>() as u64;

/// A mesh on the GPU.
pub(crate) struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    vertex_count: u32,
    index_count: u32,
    /// Ranges of the indices (`MeshData::groups`).
    groups: Vec<(u32, u32)>,
    /// The middle of its box, where a blended draw is sorted from.
    centre: [f32; 3],
    bytes: usize,
    last_used: u64,
}

/// A built-in mesh, by what makes it different.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum BuiltIn {
    Quad,
    Cube,
    Plane,
    /// Longitudes, already clamped.
    Sphere(u16),
    Gem,
    /// Sides, already clamped.
    Cone(u16),
    /// Sides, already clamped.
    Cylinder(u16),
    /// Sides, already clamped.
    Frustum(u16),
    Wedge,
    Gable,
    Hip,
    Pyramid,
    /// The half circle's sides, already clamped.
    Arch(u16),
    /// Steps, already clamped.
    Stairs(u16),
    Rock(u16),
    Blob(u16),
    /// Posts, already clamped.
    Fence(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MeshKey {
    BuiltIn(BuiltIn),
    Uploaded(u32),
}

fn mesh_key(mesh: MeshRef) -> MeshKey {
    match mesh {
        MeshRef::Quad => MeshKey::BuiltIn(BuiltIn::Quad),
        MeshRef::Cube => MeshKey::BuiltIn(BuiltIn::Cube),
        MeshRef::Plane => MeshKey::BuiltIn(BuiltIn::Plane),
        MeshRef::Sphere { segments } => MeshKey::BuiltIn(BuiltIn::Sphere(
            segments.clamp(*SPHERE_SEGMENTS.start(), *SPHERE_SEGMENTS.end()),
        )),
        MeshRef::Gem => MeshKey::BuiltIn(BuiltIn::Gem),
        MeshRef::Cone { segments } => MeshKey::BuiltIn(BuiltIn::Cone(
            segments.clamp(*SPHERE_SEGMENTS.start(), *SPHERE_SEGMENTS.end()),
        )),
        MeshRef::Cylinder { segments } => MeshKey::BuiltIn(BuiltIn::Cylinder(
            segments.clamp(*RING_SEGMENTS.start(), *RING_SEGMENTS.end()),
        )),
        MeshRef::Frustum { segments } => MeshKey::BuiltIn(BuiltIn::Frustum(
            segments.clamp(*RING_SEGMENTS.start(), *RING_SEGMENTS.end()),
        )),
        MeshRef::Wedge => MeshKey::BuiltIn(BuiltIn::Wedge),
        MeshRef::Gable => MeshKey::BuiltIn(BuiltIn::Gable),
        MeshRef::Hip => MeshKey::BuiltIn(BuiltIn::Hip),
        MeshRef::Pyramid => MeshKey::BuiltIn(BuiltIn::Pyramid),
        MeshRef::Arch { segments } => MeshKey::BuiltIn(BuiltIn::Arch(
            segments.clamp(*RING_SEGMENTS.start(), *RING_SEGMENTS.end()),
        )),
        MeshRef::Stairs { steps } => MeshKey::BuiltIn(BuiltIn::Stairs(
            steps.clamp(*STAIR_STEPS.start(), *STAIR_STEPS.end()),
        )),
        MeshRef::Rock { seed } => MeshKey::BuiltIn(BuiltIn::Rock(seed)),
        MeshRef::Blob { seed } => MeshKey::BuiltIn(BuiltIn::Blob(seed)),
        MeshRef::Fence { posts } => MeshKey::BuiltIn(BuiltIn::Fence(
            posts.clamp(*FENCE_POSTS.start(), *FENCE_POSTS.end()),
        )),
        MeshRef::Uploaded(id) => MeshKey::Uploaded(id),
    }
}

fn built_in_data(built_in: BuiltIn) -> MeshData {
    match built_in {
        BuiltIn::Quad => MeshData::quad(),
        BuiltIn::Cube => MeshData::cube(),
        BuiltIn::Plane => MeshData::plane(),
        BuiltIn::Sphere(segments) => MeshData::sphere(segments),
        BuiltIn::Gem => MeshData::gem(),
        BuiltIn::Cone(segments) => MeshData::cone(segments),
        BuiltIn::Cylinder(segments) => MeshData::cylinder(segments),
        BuiltIn::Frustum(segments) => MeshData::frustum(segments),
        BuiltIn::Wedge => MeshData::wedge(),
        BuiltIn::Gable => MeshData::gable(),
        BuiltIn::Hip => MeshData::hip(),
        BuiltIn::Pyramid => MeshData::pyramid(),
        BuiltIn::Arch(segments) => MeshData::arch(segments),
        BuiltIn::Stairs(steps) => MeshData::stairs(steps),
        BuiltIn::Rock(seed) => MeshData::rock(seed),
        BuiltIn::Blob(seed) => MeshData::blob(seed),
        BuiltIn::Fence(posts) => MeshData::fence(posts),
    }
}

/// A material's projector as the shader takes it: its matrix and `flags.w`.
/// One whose matrix is not finite is the zero matrix, which casts nothing
/// (every point has `w = 0`), so the surface is its tint and no NaN reaches
/// the GPU.
/// Where a world-to-clip matrix's lines of sight meet: the point its clip
/// x, y and w rows all take to 0 (`mesh.wgsl`'s `eye_of`). `None` for one
/// that looks along a direction (orthographic) or one that is not finite.
fn eye_of(m: &Mat4) -> Option<[f32; 3]> {
    let row = |r: usize| [m[0][r], m[1][r], m[2][r], m[3][r]];
    let (a, b, d) = (row(0), row(1), row(3));
    let det3 = |p: [f32; 3], q: [f32; 3], r: [f32; 3]| {
        p[0] * (q[1] * r[2] - q[2] * r[1]) - p[1] * (q[0] * r[2] - q[2] * r[0]) + p[2] * (q[0] * r[1] - q[1] * r[0])
    };
    let pick = |v: [f32; 4], skip: usize| -> [f32; 3] {
        let mut out = [0.0; 3];
        let mut k = 0;
        for (i, x) in v.iter().enumerate() {
            if i != skip {
                out[k] = *x;
                k += 1;
            }
        }
        out
    };
    let e: [f32; 4] = std::array::from_fn(|i| {
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        sign * det3(pick(a, i), pick(b, i), pick(d, i))
    });
    if !(e[3].is_finite() && e[3].abs() > 1e-12) {
        return None;
    }
    let eye = [e[0] / e[3], e[1] / e[3], e[2] / e[3]];
    eye.iter().all(|v| v.is_finite()).then_some(eye)
}

fn projector_of(material: &Material) -> (Mat4, u32) {
    match material.projector {
        Some(p) if mat_finite(&p.view_projection) => (p.view_projection, 1),
        Some(_) => ([[0.0; 4]; 4], 1),
        None => ([[0.0; 4]; 4], 0),
    }
}

/// A material's patch as the shader takes it: its corner in the world and
/// a pixel's size there, and its rectangle in its picture (x, y, `width <<
/// 8 | height`), when it has one that is cast from a projector and its
/// sizes are real ones; all 0 (the tint) otherwise.
fn patch_of(material: &Material) -> ([f32; 4], [f32; 3]) {
    match material.patch {
        Some(p)
            if material.projector.is_some()
                && p.texel.is_finite()
                && p.texel > 0.0
                && p.origin.iter().all(|v| v.is_finite())
                && (1..=255).contains(&p.rect[2])
                && (1..=255).contains(&p.rect[3]) =>
        {
            let [x, y, w, h] = p.rect.map(|v| v.min(1 << 16));
            ([p.origin[0], p.origin[1], p.origin[2], p.texel], [x as f32, y as f32, (w << 8 | h) as f32])
        }
        _ => ([0.0; 4], [0.0; 3]),
    }
}

/// How alpha is told to the shader (`mesh.wgsl`'s `flags.x`).
/// What a surface is to a look, as `mesh.wgsl` reads `flags.z`: 1 an
/// accent, 2 a figure, 3 water, 4 the sky, 5 an ember, 0 anything else.
/// The draw's `flags.z`: its role in the low byte (mesh.wgsl), and its
/// brand ceiling (`Material::ceiling`, docs/BRAND.md) in the next.
fn look_role(material: &Material) -> u32 {
    let role = if material.accent {
        1
    } else {
        match material.role {
            Role::Plain => 0,
            Role::Figure => 2,
            Role::Water => 3,
            Role::Sky => 4,
            Role::Ember => 5,
        }
    };
    role | u32::from(material.ceiling) << 8
}

fn alpha_flag(alpha: AlphaMode) -> u32 {
    match alpha {
        AlphaMode::Opaque => 0,
        AlphaMode::Blend => 1,
        AlphaMode::Premultiplied => 2,
    }
}

/// The bytes between two draws' uniforms: the draw's size up to the device's
/// alignment for a dynamic offset.
fn uniform_stride(align: u64) -> u64 {
    DRAW_BYTES.div_ceil(align.max(1)) * align.max(1)
}

/// What the mesh pipeline needs, made when a scene is first drawn so a host
/// that only draws layers pays nothing for it.
struct MeshState {
    shader: wgpu::ShaderModule,
    texture_layout: wgpu::BindGroupLayout,
    /// Indexed by `blended * 2 + double_sided`.
    pipelines: [wgpu::RenderPipeline; 4],
    /// White, for a material with no picture.
    white: wgpu::BindGroup,
    _white_texture: wgpu::Texture,
    uniforms: wgpu::Buffer,
    uniform_group: wgpu::BindGroup,
    uniform_layout: wgpu::BindGroupLayout,
    /// Draws the buffer has room for.
    capacity: usize,
    stride: u64,
}

impl MeshState {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view_format: wgpu::TextureFormat,
        sampler: &wgpu::Sampler,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glass-mesh-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glass-mesh-draw"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(DRAW_BYTES),
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glass-mesh-picture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        // Group 2 is a projected surface's patch, a picture as group 1 is.
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glass-mesh-layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        // The same blend as the quads: colours arrive premultiplied.
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = |blended: bool, double_sided: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("glass-mesh-pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4
                        ],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: (!double_sided).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    // Blended surfaces test against the depth but leave it.
                    depth_write_enabled: Some(!blended),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
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
            })
        };
        let pipelines = [
            pipeline(false, false),
            pipeline(false, true),
            pipeline(true, false),
            pipeline(true, true),
        ];
        let white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glass-mesh-white"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            white_texture.as_image_copy(),
            &[255; 4],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let white = texture_group(device, &texture_layout, &white_texture.create_view(&Default::default()), sampler);
        let stride = uniform_stride(u64::from(device.limits().min_uniform_buffer_offset_alignment));
        let capacity = 256;
        let (uniforms, uniform_group) = draw_buffer(device, &uniform_layout, capacity, stride);
        Self {
            shader,
            texture_layout,
            pipelines,
            white,
            _white_texture: white_texture,
            uniforms,
            uniform_group,
            uniform_layout,
            capacity,
            stride,
        }
    }
}

impl MeshState {
    /// The pipelines for a scene with a look: the same vertices and shader
    /// module, `fs_look` writing the colour and the accent mask, no blending
    /// (a covered pixel is replaced), depth written whatever the material's
    /// alpha. Indexed by `double_sided`. `scene_layout` is group 3.
    fn look_pipelines(
        &self,
        device: &wgpu::Device,
        view_format: wgpu::TextureFormat,
        scene_layout: &wgpu::BindGroupLayout,
    ) -> [wgpu::RenderPipeline; 2] {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glass-look-mesh-layout"),
            bind_group_layouts: &[
                Some(&self.uniform_layout),
                Some(&self.texture_layout),
                Some(&self.texture_layout),
                Some(scene_layout),
            ],
            immediate_size: 0,
        });
        let pipeline = |double_sided: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("glass-look-mesh-pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4
                        ],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: (!double_sided).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_look"),
                    compilation_options: Default::default(),
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: view_format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                        Some(wgpu::ColorTargetState {
                            format: MASK_FORMAT,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                    ],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        [pipeline(false), pipeline(true)]
    }

    /// The pipelines that draw a lit scene's depth at its output's size for
    /// what stands among it: depth only, through the camera, what the look's
    /// reveal opens discarded (`fs_reveal_depth`). Indexed by
    /// `double_sided`; `scene_layout` is group 3.
    fn reveal_depth_pipelines(&self, device: &wgpu::Device, scene_layout: &wgpu::BindGroupLayout) -> [wgpu::RenderPipeline; 2] {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glass-reveal-depth-layout"),
            bind_group_layouts: &[Some(&self.uniform_layout), None, None, Some(scene_layout)],
            immediate_size: 0,
        });
        let pipeline = |double_sided: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("glass-reveal-depth-pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4
                        ],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: (!double_sided).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_reveal_depth"),
                    compilation_options: Default::default(),
                    targets: &[],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        [pipeline(false), pipeline(true)]
    }

    /// The pipelines that draw a sun's shadow map: depth only, from the
    /// light (`vs_shadow`), pushed back a little by the slope so a lit
    /// surface does not shadow itself. Indexed by `double_sided`.
    /// `shadow_layout` is group 3.
    fn shadow_pipelines(&self, device: &wgpu::Device, shadow_layout: &wgpu::BindGroupLayout) -> [wgpu::RenderPipeline; 2] {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glass-shadow-layout"),
            bind_group_layouts: &[Some(&self.uniform_layout), None, None, Some(shadow_layout)],
            immediate_size: 0,
        });
        let pipeline = |double_sided: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("glass-shadow-pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_shadow"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4
                        ],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: (!double_sided).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::look::SHADOW_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.5, clamp: 0.0 },
                }),
                multisample: Default::default(),
                fragment: None,
                multiview_mask: None,
                cache: None,
            })
        };
        [pipeline(false), pipeline(true)]
    }
}

fn texture_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("glass-mesh-picture"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// The buffer the draws' uniforms are written to, and the group that binds
/// one draw's slice of it at a dynamic offset.
fn draw_buffer(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    capacity: usize,
    stride: u64,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glass-mesh-draws"),
        size: capacity as u64 * stride,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("glass-mesh-draw"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: NonZeroU64::new(DRAW_BYTES),
            }),
        }],
    });
    (buffer, group)
}

/// A depth buffer of one size.
struct DepthTarget {
    size: (u32, u32),
    view: wgpu::TextureView,
    last_used: u64,
}

/// What a scene's draw does with depth.
#[derive(Clone, Copy)]
pub(crate) enum DepthUse<'a> {
    /// A depth buffer of the target's size, cleared, and dropped after.
    Fresh,
    /// This depth target, cleared, and kept after (a capture's depth map, a
    /// world's blocks for its figures to stand behind).
    Keep(&'a wgpu::TextureView),
    /// This depth target as an earlier draw left it: tested against, and
    /// kept (a world's figures, behind its blocks).
    Continue(&'a wgpu::TextureView),
    /// For a scene with a look that lights it: drawn at the look's own
    /// size with a depth of its own, then its opaque surfaces' depth drawn
    /// once more into this target, of the output's size, cleared, with what
    /// the look's reveal opens left out, so what stands among them later
    /// (a world's figures) shows where the reveal opened the way to it.
    Revealed(&'a wgpu::TextureView),
}

/// Everything the 3D side keeps between frames.
#[derive(Default)]
pub(crate) struct SceneState {
    meshes: FastMap<u32, GpuMesh>,
    mesh_bytes: usize,
    built_in: HashMap<BuiltIn, GpuMesh>,
    state: Option<MeshState>,
    depths: Vec<DepthTarget>,
    /// Scenes drawn into their own pictures every frame, by picture id.
    pub(crate) targets: HashMap<u32, (Scene, (u32, u32))>,
    /// The draws' uniforms as bytes, kept for the next scene to refill.
    scratch: Vec<u8>,
    /// What a scene with a look needs (`look.rs`), made at the first.
    look: Option<LookState>,
}

/// One draw: a range of a mesh, with a material, placed.
struct Item<'a> {
    key: MeshKey,
    /// The range of indices, or all of them.
    range: Option<(u32, u32)>,
    material: &'a Material,
    world: Mat4,
    /// How far the middle of it is in front of the camera (negative: view
    /// space looks down -Z), for ordering blended draws.
    view_z: f32,
    /// The picture of its patch (`Material::patch`), once it is known to be
    /// on the GPU with its group made.
    patch: Option<u32>,
}

impl Core {
    /// Put a mesh on the GPU under `id`, over the one that id has. Refused,
    /// with why, when it fails [`MeshData::validate`] (too big, malformed).
    /// Meshes are kept under a budget: past it, the least recently drawn go,
    /// never one drawn in the current frame.
    pub fn upload_mesh(&mut self, id: u32, mesh: &MeshData) -> Result<(), MeshError> {
        mesh.validate()?;
        // An animated mesh re-sends the same shape every frame: write over
        // the buffers it has.
        if let Some(existing) = self.three.meshes.get_mut(&id)
            && existing.vertex_count as usize == mesh.positions.len()
            && existing.index_count as usize == mesh.indices.len()
        {
            self.queue
                .write_buffer(&existing.vertices, 0, bytemuck::cast_slice(&vertices_of(mesh)));
            self.queue
                .write_buffer(&existing.indices, 0, bytemuck::cast_slice(&mesh.indices));
            existing.groups = mesh.groups.clone();
            existing.centre = mesh.centre();
            existing.last_used = self.frame_number;
            return Ok(());
        }
        let gpu = make_mesh(&self.device, &self.queue, mesh, self.frame_number);
        let bytes = gpu.bytes;
        if let Some(old) = self.three.meshes.insert(id, gpu) {
            self.three.mesh_bytes -= old.bytes;
        }
        self.three.mesh_bytes += bytes;
        self.evict_meshes();
        Ok(())
    }

    /// Whether uploaded mesh `id` is on the GPU.
    pub fn has_mesh(&self, id: u32) -> bool {
        self.three.meshes.contains_key(&id)
    }

    /// Drop uploaded mesh `id`.
    pub fn remove_mesh(&mut self, id: u32) {
        if let Some(old) = self.three.meshes.remove(&id) {
            self.three.mesh_bytes -= old.bytes;
        }
    }

    fn evict_meshes(&mut self) {
        if self.three.mesh_bytes <= MESH_BUDGET && self.three.meshes.len() <= MESH_COUNT {
            return;
        }
        let order = eviction_order(
            self.three.meshes.iter().map(|(&id, m)| (id, m.last_used, m.bytes)),
            self.three.mesh_bytes,
            MESH_BUDGET,
            MESH_COUNT,
            self.frame_number,
        );
        for id in order {
            self.remove_mesh(id);
        }
    }

    /// The mesh `key` names, when it is on the GPU.
    fn mesh_of(&self, key: MeshKey) -> Option<&GpuMesh> {
        match key {
            MeshKey::Uploaded(id) => self.three.meshes.get(&id),
            MeshKey::BuiltIn(built_in) => self.three.built_in.get(&built_in),
        }
    }

    /// The depth buffer for targets of `size`, made when missing. The least
    /// recently used size goes when there are too many.
    pub(crate) fn depth_view(&mut self, size: (u32, u32)) -> wgpu::TextureView {
        let frame = self.frame_number;
        if let Some(depth) = self.three.depths.iter_mut().find(|d| d.size == size) {
            depth.last_used = frame;
            return depth.view.clone();
        }
        if self.three.depths.len() >= DEPTH_TARGETS
            && let Some(oldest) = self
                .three
                .depths
                .iter()
                .enumerate()
                .min_by_key(|(_, d)| d.last_used)
                .map(|(i, _)| i)
        {
            self.three.depths.swap_remove(oldest);
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glass-depth"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.three.depths.push(DepthTarget {
            size,
            view: view.clone(),
            last_used: frame,
        });
        view
    }

    /// Draw `scene` into `view`, `size` pixels (the size of the texture
    /// behind it, which the depth buffer must match), in a submission of its
    /// own. The camera's `clear` is the colour the view is cleared to first
    /// (premultiplied for the target), or the view is drawn over when it has
    /// none; depth always starts fresh. The aspect ratio is the size's.
    ///
    /// A node whose mesh is an uploaded one that is not there, or whose
    /// material names a picture that is not (or not yet) on the GPU, draws
    /// nothing for that part; a camera that cannot project draws nothing at
    /// all, and nothing here panics or reaches the GPU as NaN. `label` names
    /// the commands in a GPU error.
    pub fn draw_scene(
        &mut self,
        view: &wgpu::TextureView,
        size: (u32, u32),
        scene: &Scene,
        label: &str,
    ) {
        self.draw_scene_into(view, size, scene, label, None);
    }

    /// `draw_scene`, leaving out any part that would sample picture `target`
    /// (the one `view` is of: a texture cannot be read while it is drawn
    /// into).
    pub(crate) fn draw_scene_into(
        &mut self,
        view: &wgpu::TextureView,
        size: (u32, u32),
        scene: &Scene,
        label: &str,
        target: Option<u32>,
    ) {
        self.draw_scene_with_depth(view, size, scene, label, target, DepthUse::Fresh);
    }

    /// The depth `scene` leaves when drawn at `size` (without its look, if
    /// it has one): one number a pixel, row by row from the top, 0 at the
    /// camera's near plane to 1 at its far one, and 1 where nothing was
    /// drawn. Empty when the GPU does not answer in time or `size` is not a
    /// target's. For a capture's depth map (docs/WORLD-3D.md).
    pub fn scene_depth(&mut self, scene: &Scene, size: (u32, u32)) -> Vec<f32> {
        let (width, height) = size;
        if width == 0 || height == 0 || width > MAX_TARGET_PX || height > MAX_TARGET_PX {
            return Vec::new();
        }
        let without_look;
        let scene = if scene.look.is_some() {
            without_look = Scene { look: None, ..scene.clone() };
            &without_look
        } else {
            scene
        };
        let extent = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = |label, format, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let colour = texture("glass-depth-colour", self.view_format, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let depth = texture(
            "glass-depth-kept",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth_view = depth.create_view(&Default::default());
        self.draw_scene_with_depth(
            &colour.create_view(&Default::default()),
            size,
            scene,
            "glass-scene-depth",
            None,
            DepthUse::Keep(&depth_view),
        );
        let row_bytes = (width * 4).div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glass-depth-readback"),
            size: u64::from(row_bytes * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &depth,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: Some(height),
                },
            },
            extent,
        );
        self.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let polled = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        });
        let mut out = Vec::with_capacity((width * height) as usize);
        if polled.is_ok() && matches!(rx.recv_timeout(std::time::Duration::from_secs(1)), Ok(Ok(()))) {
            if let Ok(mapped) = slice.get_mapped_range() {
                for row in mapped.chunks(row_bytes as usize).take(height as usize) {
                    out.extend(row[..(width * 4) as usize].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
                }
            }
            readback.unmap();
        }
        out
    }

    /// `draw_scene_into`, with depth as `depth` says (a depth target of
    /// `size` when it names one, which a look's scene is not drawn at: it
    /// takes a fresh one).
    pub(crate) fn draw_scene_with_depth(
        &mut self,
        view: &wgpu::TextureView,
        size: (u32, u32),
        scene: &Scene,
        label: &str,
        target: Option<u32>,
        depth: DepthUse<'_>,
    ) {
        let (width, height) = size;
        if width == 0 || height == 0 {
            return;
        }
        // A scene with a look is drawn at the look's internal size and then
        // quantized into the view (look.rs); `size` is from here on the size
        // it is drawn at, `output_size` the view's.
        let look = scene.look.as_ref().and_then(Look::normalized);
        let output_size = size;
        let size = look.as_ref().map_or(size, |l| l.internal_size(size));
        let (width, height) = size;
        if self.three.state.is_none() {
            self.three.state = Some(MeshState::new(&self.device, &self.queue, self.view_format, &self.nearest));
        }
        // The eye: where the projection's lines meet (a stage camera's
        // transform is the identity, its pinhole in its matrix), else where
        // the camera stands (one that looks along a direction).
        let eye_size = size;
        let look_views = look.as_ref().map(|look| {
            let eye = scene
                .camera
                .view_projection(eye_size.0.max(1) as f32 / eye_size.1.max(1) as f32)
                .and_then(|m| eye_of(&m))
                .unwrap_or(scene.camera.transform.translation);
            let state = self.three.state.as_ref().expect("made above");
            let device = &self.device;
            let view_format = self.view_format;
            let kept = self.three.look.get_or_insert_with(|| LookState::new(device, view_format));
            if kept.mesh.is_none() {
                kept.mesh = Some(state.look_pipelines(device, view_format, &kept.scene_layout));
            }
            if look.sun.is_some_and(|sun| sun.shadow.is_some()) && kept.shadow.is_none() {
                kept.shadow = Some(state.shadow_pipelines(device, &kept.shadow_layout));
            }
            if matches!(depth, DepthUse::Revealed(_)) && kept.reveal_depth.is_none() {
                kept.reveal_depth = Some(state.reveal_depth_pipelines(device, &kept.scene_layout));
            }
            kept.begin(device, &self.queue, view_format, look, eye, size, output_size, self.frame_number)
        });
        let frame = self.frame_number;
        let view_proj = scene.camera.view_projection(width as f32 / height as f32);
        let camera = scene.camera.view_matrix();

        // Which meshes the nodes want, made or marked used.
        let mut wanted: Vec<(usize, MeshKey, Mat4)> = Vec::new();
        if view_proj.is_some() {
            for drawable in scene.draw_list() {
                let Some(mesh) = scene.nodes[drawable.node].mesh else {
                    continue;
                };
                let key = mesh_key(mesh);
                match key {
                    MeshKey::BuiltIn(built_in) => {
                        let device = &self.device;
                        let queue = &self.queue;
                        self.three
                            .built_in
                            .entry(built_in)
                            .or_insert_with(|| make_mesh(device, queue, &built_in_data(built_in), frame));
                    }
                    MeshKey::Uploaded(id) => match self.three.meshes.get_mut(&id) {
                        Some(mesh) => mesh.last_used = frame,
                        None => continue,
                    },
                }
                wanted.push((drawable.node, key, drawable.world));
            }
        }

        // Each part of each node as a draw: a mesh with groups is one draw a
        // group, each with its own material; one without is a single draw.
        let mut items: Vec<Item<'_>> = Vec::new();
        for (node_index, key, world) in wanted {
            if items.len() >= MAX_DRAWS {
                break;
            }
            let Some(mesh) = self.mesh_of(key) else {
                continue;
            };
            let node = &scene.nodes[node_index];
            let view_z = mat_point(&camera, mat_point(&world, mesh.centre))[2];
            if mesh.groups.is_empty() {
                items.push(Item { key, range: None, material: &node.material, world, view_z, patch: None });
            } else {
                for (g, &range) in mesh.groups.iter().enumerate() {
                    let material = node.face_materials.get(g).unwrap_or(&node.material);
                    items.push(Item { key, range: Some(range), material, world, view_z, patch: None });
                }
            }
        }
        items.truncate(MAX_DRAWS);

        // The pictures they sample: marked used, bind groups made, and the
        // draws whose picture is not there dropped.
        let state = self.three.state.as_ref().expect("made above");
        let texture_layout = &state.texture_layout;
        let (device, nearest, linear, pictures) = (&self.device, &self.nearest, &self.linear, &mut self.pictures);
        items.retain_mut(|item| {
            // A projected surface's patch: marked used, its group made; one
            // that is not there leaves the surface its tint.
            if let Some(patch) = item.material.patch.filter(|p| item.material.projector.is_some() && Some(p.picture) != target)
                && let Some(picture) = pictures.get_mut(&patch.picture)
            {
                picture.last_used = frame;
                if picture.mesh_groups[0].is_none() {
                    picture.mesh_groups[0] = Some(texture_group(device, texture_layout, &picture.view, nearest));
                }
                item.patch = Some(patch.picture);
            }
            let TextureSource::Picture(id) = item.material.source else {
                return true;
            };
            if Some(id) == target {
                return false;
            }
            let Some(picture) = pictures.get_mut(&id) else {
                return false;
            };
            picture.last_used = frame;
            let way = usize::from(item.material.sampling == Sampling::Linear);
            if picture.mesh_groups[way].is_none() {
                let sampler = if way == 1 { linear } else { nearest };
                picture.mesh_groups[way] = Some(texture_group(device, texture_layout, &picture.view, sampler));
            }
            true
        });

        // Opaque first in node order, then blended, farthest first.
        let mut order: Vec<usize> = (0..items.len()).collect();
        order.sort_by(|&a, &b| {
            let (a, b) = (&items[a], &items[b]);
            let (ba, bb) = (a.material.alpha.blended(), b.material.alpha.blended());
            ba.cmp(&bb).then_with(|| {
                if ba {
                    // The more negative the view z, the farther.
                    a.view_z.total_cmp(&b.view_z)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
        });

        // The uniforms, one slice each.
        let stride = self.three.state.as_ref().map_or(256, |s| s.stride) as usize;
        let mut scratch = std::mem::take(&mut self.three.scratch);
        scratch.clear();
        scratch.resize(order.len() * stride, 0);
        if let Some(view_proj) = view_proj {
            for (slot, &i) in order.iter().enumerate() {
                let item = &items[i];
                let (projector, projected) = projector_of(item.material);
                let (patch, rect) = if item.patch.is_some() { patch_of(item.material) } else { ([0.0; 4], [0.0; 3]) };
                let uniform = DrawUniform {
                    model: item.world,
                    view_proj,
                    tint: item.material.tint_clamped(),
                    flags: [
                        alpha_flag(item.material.alpha),
                        u32::from(item.material.lit),
                        look_role(item.material),
                        projected,
                    ],
                    light: [item.material.emissive_clamped(), rect[0], rect[1], rect[2]],
                    projector,
                    patch,
                };
                scratch[slot * stride..slot * stride + DRAW_BYTES as usize]
                    .copy_from_slice(bytemuck::bytes_of(&uniform));
            }
        }
        {
            let state = self.three.state.as_mut().expect("made above");
            if order.len() > state.capacity {
                state.capacity = order.len().next_power_of_two();
                let (buffer, group) = draw_buffer(&self.device, &state.uniform_layout, state.capacity, state.stride);
                state.uniforms = buffer;
                state.uniform_group = group;
            }
            if !scratch.is_empty() {
                self.queue.write_buffer(&state.uniforms, 0, &scratch);
            }
        }
        self.three.scratch = scratch;

        let revealed = if let DepthUse::Revealed(target) = depth { Some(target.clone()) } else { None };
        let (depth, depth_load, depth_store) = match depth {
            DepthUse::Keep(kept) if look.is_none() => (kept.clone(), wgpu::LoadOp::Clear(1.0), wgpu::StoreOp::Store),
            DepthUse::Continue(kept) if look.is_none() => (kept.clone(), wgpu::LoadOp::Load, wgpu::StoreOp::Store),
            _ => (self.depth_view(size), wgpu::LoadOp::Clear(1.0), wgpu::StoreOp::Discard),
        };
        let load = match scene.camera.clear_premultiplied() {
            Some([r, g, b, a]) => wgpu::LoadOp::Clear(wgpu::Color {
                r: f64::from(r),
                g: f64::from(g),
                b: f64::from(b),
                a: f64::from(a),
            }),
            None => wgpu::LoadOp::Load,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        // A sun with shadows: its map first, from the light, of what is
        // opaque and plain (figures, water, accents and see-through things
        // cast nothing).
        let casts_shadow = view_proj.is_some() && look.as_ref().is_some_and(|l| l.sun.is_some_and(|s| s.shadow.is_some()));
        if casts_shadow && let Some(kept) = self.three.look.as_mut() {
            let map = kept.shadow_map(&self.device);
            let kept = self.three.look.as_ref().expect("made above");
            let state = self.three.state.as_ref().expect("made above");
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glass-shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &map,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(pipelines) = &kept.shadow {
                pass.set_bind_group(3, &kept.shadow_group, &[]);
                let mut last = usize::MAX;
                for (slot, &i) in order.iter().enumerate() {
                    let item = &items[i];
                    let casts = !item.material.alpha.blended()
                        && !item.material.accent
                        && item.material.role == Role::Plain
                        && item.material.projector.is_none();
                    if !casts {
                        continue;
                    }
                    let Some(mesh) = self.mesh_of(item.key) else {
                        continue;
                    };
                    let which = usize::from(item.material.double_sided);
                    if which != last {
                        pass.set_pipeline(&pipelines[which]);
                        last = which;
                    }
                    pass.set_bind_group(0, &state.uniform_group, &[(slot * stride) as u32]);
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    let (first, count) = item.range.unwrap_or((0, mesh.index_count));
                    pass.draw_indexed(first..first + count, 0, 0..1);
                }
            }
        }
        {
            // With a look the scene is drawn into the internal colour and
            // accent mask (always cleared, to the camera's clear or nothing),
            // not into the view.
            let (draw_view, draw_load) = match &look_views {
                Some((color, _)) => (
                    color,
                    match scene.camera.clear_premultiplied() {
                        Some(_) => load,
                        None => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    },
                ),
                None => (view, load),
            };
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: draw_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: draw_load,
                    store: wgpu::StoreOp::Store,
                },
            };
            let mask_attachment = look_views.as_ref().map(|(_, mask)| wgpu::RenderPassColorAttachment {
                view: mask,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            });
            let attachments = [Some(color_attachment), mask_attachment];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &attachments[..if look_views.is_some() { 2 } else { 1 }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations { load: depth_load, store: depth_store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let state = self.three.state.as_ref().expect("made above");
            let look_mesh = look_views.as_ref().and_then(|_| self.three.look.as_ref()?.mesh.as_ref());
            if let Some(kept) = self.three.look.as_ref().filter(|_| look_views.is_some()) {
                pass.set_bind_group(3, &kept.scene_group, &[]);
            }
            let mut last_pipeline = usize::MAX;
            // Group 2 is the same for most draws in a row (white, or one
            // kind's patch): set only when it changes.
            let mut last_patch: Option<&wgpu::BindGroup> = None;
            for (slot, &i) in order.iter().enumerate() {
                let item = &items[i];
                let Some(mesh) = self.mesh_of(item.key) else {
                    continue;
                };
                let group = match item.material.source {
                    TextureSource::None => &state.white,
                    TextureSource::Picture(id) => {
                        let way = usize::from(item.material.sampling == Sampling::Linear);
                        match self.pictures.get(&id).and_then(|p| p.mesh_groups[way].as_ref()) {
                            Some(group) => group,
                            None => continue,
                        }
                    }
                };
                let which = usize::from(item.material.alpha.blended()) * 2
                    + usize::from(item.material.double_sided);
                if which != last_pipeline {
                    match look_mesh {
                        Some(pipelines) => pass.set_pipeline(&pipelines[which % 2]),
                        None => pass.set_pipeline(&state.pipelines[which]),
                    }
                    last_pipeline = which;
                }
                let patch = item
                    .patch
                    .and_then(|id| self.pictures.get(&id)?.mesh_groups[0].as_ref())
                    .unwrap_or(&state.white);
                pass.set_bind_group(0, &state.uniform_group, &[(slot * stride) as u32]);
                pass.set_bind_group(1, group, &[]);
                if !last_patch.is_some_and(|last| std::ptr::eq(last, patch)) {
                    pass.set_bind_group(2, patch, &[]);
                    last_patch = Some(patch);
                }
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                let (first, count) = item.range.unwrap_or((0, mesh.index_count));
                pass.draw_indexed(first..first + count, 0, 0..1);
            }
        }
        if look.is_some()
            && let Some(kept) = &self.three.look
        {
            kept.post_pass(&mut encoder, view, size, scene.camera.clear.is_some(), label);
            // The depth what stands among these surfaces is tested against,
            // at the output's size, the reveal's openings left out.
            if let (Some(target), Some(pipelines)) = (revealed.as_ref(), kept.reveal_depth.as_ref()) {
                let state = self.three.state.as_ref().expect("made above");
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("glass-reveal-depth"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: target,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_bind_group(3, &kept.scene_group, &[]);
                let mut last = usize::MAX;
                for (slot, &i) in order.iter().enumerate() {
                    let item = &items[i];
                    if item.material.alpha.blended() || item.material.role == Role::Figure {
                        continue;
                    }
                    let Some(mesh) = self.mesh_of(item.key) else {
                        continue;
                    };
                    let which = usize::from(item.material.double_sided);
                    if which != last {
                        pass.set_pipeline(&pipelines[which]);
                        last = which;
                    }
                    pass.set_bind_group(0, &state.uniform_group, &[(slot * stride) as u32]);
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    let (first, count) = item.range.unwrap_or((0, mesh.index_count));
                    pass.draw_indexed(first..first + count, 0, 0..1);
                }
            }
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Show a scene in the picture `id`, `size` pixels: it is drawn into its
    /// own texture every frame by [`Core::render_scenes`], and any layer that
    /// names the picture places it, lifts it and turns it as it does any
    /// picture. Its clear colour is the picture's background (clear to
    /// nothing to float it over what is behind). `None` takes it down.
    ///
    /// The picture's id is shared with uploads and stages; the texture is
    /// premultiplied, which the layer compiler knows.
    pub fn set_scene_target(&mut self, id: u32, target: Option<(Scene, (u32, u32))>) {
        match target {
            Some(target) => {
                self.three.targets.insert(id, target);
            }
            None => {
                self.three.targets.remove(&id);
                if self.pictures.get(&id).is_some_and(|p| p.target) {
                    self.pictures.remove(&id);
                }
            }
        }
    }

    /// Draw every scene target into its texture, each in a submission of its
    /// own, before the frame that shows them.
    pub fn render_scenes(&mut self) {
        if self.three.targets.is_empty() {
            return;
        }
        let targets = std::mem::take(&mut self.three.targets);
        for (&id, (scene, size)) in &targets {
            let size = (size.0.min(MAX_TARGET_PX), size.1.min(MAX_TARGET_PX));
            if size.0 == 0 || size.1 == 0 {
                continue;
            }
            // A scene with a look is shown in a picture of its internal size:
            // the layer that places it enlarges it by nearest.
            let size = match scene.look.as_ref().and_then(Look::normalized) {
                Some(look) => look.internal_size(size),
                None => size,
            };
            self.target_texture(id, size, 0, 0, true);
            let Some(view) = self.pictures.get(&id).map(|p| p.view.clone()) else {
                continue;
            };
            self.draw_scene_into(&view, size, scene, "glass-scene", Some(id));
        }
        self.three.targets = targets;
    }
}

/// A mesh's vertices as the GPU reads them: an attribute left out is white,
/// +Z, or the picture's corner.
fn vertices_of(mesh: &MeshData) -> Vec<MeshVertex> {
    (0..mesh.positions.len())
        .map(|i| MeshVertex {
            position: mesh.positions[i],
            normal: mesh.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]),
            uv: mesh.uvs.get(i).copied().unwrap_or([0.0; 2]),
            color: mesh.colors.get(i).copied().unwrap_or([1.0; 4]),
        })
        .collect()
}

/// A valid mesh on the GPU.
fn make_mesh(device: &wgpu::Device, queue: &wgpu::Queue, mesh: &MeshData, frame: u64) -> GpuMesh {
    let vertices = vertices_of(mesh);
    let vertex_bytes = (vertices.len() * std::mem::size_of::<MeshVertex>()) as u64;
    let index_bytes = (mesh.indices.len() * 4) as u64;
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glass-mesh-vertices"),
        size: vertex_bytes,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glass-mesh-indices"),
        size: index_bytes,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&mesh.indices));
    GpuMesh {
        vertices: vertex_buffer,
        indices: index_buffer,
        vertex_count: vertices.len() as u32,
        index_count: mesh.indices.len() as u32,
        groups: mesh.groups.clone(),
        centre: mesh.centre(),
        bytes: (vertex_bytes + index_bytes) as usize,
        last_used: frame,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draws_uniforms_are_three_matrices_and_four_vectors() {
        // Two mat4, a tint, four flags, the light, the projector's mat4 and
        // the patch: what mesh.wgsl's Draw is. It fills the usual 256-byte
        // stride exactly.
        assert_eq!(DRAW_BYTES, 2 * 64 + 16 + 16 + 16 + 64 + 16);
        const { assert!(DRAW_BYTES <= 256) };
    }

    #[test]
    fn the_stride_is_the_devices_alignment() {
        assert_eq!(uniform_stride(256), 256);
        assert_eq!(uniform_stride(64), 256);
        assert_eq!(uniform_stride(32), 256);
        assert_eq!(uniform_stride(16), 256);
        assert_eq!(uniform_stride(0), 256);
    }

    #[test]
    fn a_projector_is_told_to_the_shader_and_a_bad_one_casts_nothing() {
        use angel_stage::scene::Projector;
        let m = [[1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0], [0.0; 4], [0.0, 0.0, 0.0, 1.0]];
        assert_eq!(projector_of(&Material::default()), ([[0.0; 4]; 4], 0));
        let cast = Material { projector: Some(Projector { view_projection: m }), ..Material::default() };
        assert_eq!(projector_of(&cast), (m, 1));
        let mut bad = m;
        bad[2][1] = f32::NAN;
        let bad = Material { projector: Some(Projector { view_projection: bad }), ..Material::default() };
        assert_eq!(projector_of(&bad), ([[0.0; 4]; 4], 1));
    }

    #[test]
    fn a_patch_is_told_to_the_shader_only_on_a_projected_surface() {
        use angel_stage::scene::{Patch, Projector};
        let projector = Some(Projector { view_projection: angel_stage::scene::IDENTITY_MATRIX });
        let none = ([0.0; 4], [0.0; 3]);
        let patch = Some(Patch { picture: 7, rect: [64, 128, 40, 9], origin: [1.0, 2.0, 3.0], texel: 0.5 });
        assert_eq!(patch_of(&Material { projector, patch, ..Material::default() }), ([1.0, 2.0, 3.0, 0.5], [64.0, 128.0, (40 * 256 + 9) as f32]));
        assert_eq!(patch_of(&Material { patch, ..Material::default() }), none, "not cast: no patch");
        assert_eq!(patch_of(&Material { projector, ..Material::default() }), none);
        let bad = Some(Patch { texel: f32::NAN, ..patch.unwrap() });
        assert_eq!(patch_of(&Material { projector, patch: bad, ..Material::default() }), none);
        let flat = Some(Patch { texel: 0.0, ..patch.unwrap() });
        assert_eq!(patch_of(&Material { projector, patch: flat, ..Material::default() }), none);
        let empty = Some(Patch { rect: [0, 0, 0, 4], ..patch.unwrap() });
        assert_eq!(patch_of(&Material { projector, patch: empty, ..Material::default() }), none);
    }

    #[test]
    fn the_shaders_face_shades_and_grazing_are_the_scenes() {
        let source = include_str!("mesh.wgsl");
        let [top, front, side] = angel_stage::scene::FACE_SHADES;
        for (name, v) in [("FACE_TOP", top), ("FACE_FRONT", front), ("FACE_SIDE", side), ("GRAZING", angel_stage::scene::GRAZING)] {
            assert!(source.contains(&format!("const {name}: f32 = {v:?};")), "{name}");
        }
    }

    #[test]
    fn built_in_meshes_are_told_apart_by_their_shape() {
        assert_eq!(mesh_key(MeshRef::Cube), MeshKey::BuiltIn(BuiltIn::Cube));
        assert_eq!(mesh_key(MeshRef::Uploaded(9)), MeshKey::Uploaded(9));
        // Spheres are clamped before they are keyed: one mesh for every size past the end.
        assert_eq!(mesh_key(MeshRef::Sphere { segments: 0 }), mesh_key(MeshRef::Sphere { segments: 3 }));
        assert_eq!(mesh_key(MeshRef::Sphere { segments: 9999 }), mesh_key(MeshRef::Sphere { segments: 128 }));
        assert_ne!(mesh_key(MeshRef::Sphere { segments: 8 }), mesh_key(MeshRef::Sphere { segments: 9 }));
        assert_eq!(mesh_key(MeshRef::Gem), MeshKey::BuiltIn(BuiltIn::Gem));
        assert_eq!(mesh_key(MeshRef::Cone { segments: 0 }), mesh_key(MeshRef::Cone { segments: 3 }));
        assert_ne!(mesh_key(MeshRef::Cone { segments: 6 }), mesh_key(MeshRef::Sphere { segments: 6 }));
    }

    #[test]
    fn the_shaders_light_is_the_scenes() {
        // One light for every scene: the shader spells the same numbers.
        let source = include_str!("mesh.wgsl");
        let [x, y, z] = angel_stage::scene::KEY_LIGHT;
        assert!(source.contains(&format!("vec3<f32>({x}, {y}, {z})")), "KEY_LIGHT");
        assert!(source.contains(&format!("const AMBIENT: f32 = {};", angel_stage::scene::AMBIENT_LIGHT)), "AMBIENT");
    }

    #[test]
    fn alpha_is_told_to_the_shader_as_its_mode() {
        assert_eq!(alpha_flag(AlphaMode::Opaque), 0);
        assert_eq!(alpha_flag(AlphaMode::Blend), 1);
        assert_eq!(alpha_flag(AlphaMode::Premultiplied), 2);
    }

    #[test]
    fn a_missing_attribute_takes_its_default() {
        let mesh = MeshData {
            positions: vec![[1.0, 2.0, 3.0]; 3],
            indices: vec![0, 1, 2],
            ..MeshData::default()
        };
        let v = vertices_of(&mesh);
        assert_eq!(v.len(), 3);
        assert_eq!((v[1].normal, v[1].uv, v[1].color), ([0.0, 0.0, 1.0], [0.0; 2], [1.0; 4]));
        assert_eq!(std::mem::size_of::<MeshVertex>(), 48);
    }

    #[test]
    fn the_eye_is_where_a_projections_lines_meet() {
        use angel_stage::scene::{Camera, Projection, Transform};
        // A perspective camera: the eye is where it stands.
        let camera = Camera {
            transform: Transform::from_translation([3.0, 7.0, -2.0]),
            projection: Projection::Perspective { fov_y: 0.8, near: 0.5, far: 100.0 },
            clear: None,
        };
        let eye = eye_of(&camera.view_projection(1.5).unwrap()).expect("an eye");
        for k in 0..3 {
            assert!((eye[k] - [3.0, 7.0, -2.0][k]).abs() < 1e-3, "{eye:?}");
        }
        // The stage camera: its transform is the identity, its pinhole on the
        // stage's middle line in front of the floor and above it.
        let stage = angel_stage::scene::stage_camera(0.3, (384, 224));
        let eye = eye_of(&stage.view_projection(384.0 / 224.0).unwrap()).expect("a pinhole");
        assert!((eye[0] - 192.0).abs() < 1e-2, "{eye:?}");
        assert!(eye[2] > 224.0, "toward the viewer past the stage: {eye:?}");
        // An orthographic camera looks along a direction: no eye.
        let ortho = Camera { projection: Projection::Orthographic { height: 4.0, near: 0.1, far: 10.0 }, ..camera };
        assert_eq!(eye_of(&ortho.view_projection(1.0).unwrap()), None);
    }
}
