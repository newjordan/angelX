//! glTF 2.0 for glass's own dialect: a `.glb`, or a `.gltf` whose buffers
//! ride inside it as `data:` URIs, read into what a scene draws — meshes,
//! materials, the node tree and its animation clips — under caps checked
//! before anything is allocated (docs/GLASS_DIALECT.md, `asset`).
//!
//! The `gltf` crate gives the JSON its types and nothing more. The GLB
//! container, every index, every accessor's bytes and the node tree are
//! read and checked here, by hand, so a truncated, lying or hostile file
//! is an error reply and never a panic or an allocation past a cap.
//!
//! What a file holds that the lit path cannot draw is kept for later:
//! metallic and roughness shade the colour (a smooth metal shows mostly
//! the reflections this path does not draw, so it is darkened), the other
//! textures are remembered by index, and morph-target weights are not
//! played. Skins are read whole (joints, inverse bind matrices, each
//! vertex's `JOINTS_0` and `WEIGHTS_0`) and bend their meshes as the
//! joints move.

use std::borrow::Cow;
use std::sync::Arc;

use angel_glass::scene::{
    AlphaMode, Mat4, Material, MeshData, Node, Sampling, Skin, Transform, IDENTITY_MATRIX, MAX_JOINTS, MAX_MESH_INDICES,
    MAX_MESH_VERTICES,
};
use gltf::json;
use json::validation::Checked;

use super::glass_scene::WireError;

/// The file, whole, as glass takes it (the APC file medium's own cap).
pub const ASSET_BYTES: usize = 64 << 20;
/// Objects and arrays in the JSON, counted before it is parsed, so a file
/// of tiny objects cannot grow many times its size in memory.
pub const JSON_STRUCTURES: usize = 1 << 18;
pub const ASSET_NODES: usize = 4_096;
pub const ASSET_MESHES: usize = 1_024;
pub const ASSET_PRIMITIVES: usize = 4_096;
/// Accessors, buffer views, textures and samplers, each.
pub const ASSET_ACCESSORS: usize = 16_384;
pub const ASSET_BUFFERS: usize = 64;
pub const ASSET_MATERIALS: usize = 1_024;
/// Pictures decoded for one asset, and their RGBA bytes together.
pub const ASSET_IMAGES: usize = 32;
pub const ASSET_TEXTURE_BYTES: usize = 64 << 20;
/// A texture's side, in pixels: the dialect's own picture cap.
pub const IMAGE_SIDE: u32 = 4_096;
/// Vertices and triangle-list indices across one asset (a primitive also
/// stays within the window's own per-mesh caps).
pub const ASSET_VERTICES: usize = 1 << 20;
pub const ASSET_INDICES: usize = 3 << 20;
/// How deep the node tree may go.
pub const ASSET_DEPTH: usize = 128;
pub const ASSET_CLIPS: usize = 64;
/// Channels across every clip, and keyframe values across every channel.
pub const ASSET_CHANNELS: usize = 4_096;
pub const ASSET_KEYS: usize = 1 << 20;
/// What a vertex and an index cost on the GPU, for the asset budget; a
/// skinned vertex costs its joints and weights besides.
pub const VERTEX_BYTES: usize = 48;
pub const INDEX_BYTES: usize = 4;
pub const SKIN_BYTES: usize = 24;
/// Skins in one asset, and joints in one skin (the window's own cap: one
/// draw's palette).
pub const ASSET_SKINS: usize = 64;
pub const ASSET_JOINTS: usize = MAX_JOINTS;

fn err(code: &'static str, message: impl Into<String>) -> WireError {
    WireError::new(code, message)
}

/// One node of the file: where it sits under its parent, and what it draws.
#[derive(Clone, Debug)]
pub struct GNode {
    pub transform: Transform,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
    pub mesh: Option<usize>,
    /// The skin that bends its mesh.
    pub skin: Option<usize>,
}

/// One skin of the file: its joints (nodes of the file) and their inverse
/// bind matrices, one each (the identity where the file gives none).
#[derive(Clone, Debug)]
pub struct GSkin {
    pub joints: Vec<usize>,
    pub inverse_bind: Arc<[Mat4]>,
}

/// One drawable primitive: its triangles, its material, and the mesh id
/// the window knows it by (given by the dialect before it goes up).
#[derive(Clone, Debug)]
pub struct Prim {
    pub mesh: Arc<MeshData>,
    pub material: usize,
    pub id: u32,
}

/// A material as the lit path draws it, the picture its colour texture
/// was decoded into (by index into [`Loaded::images`]), and the file's own
/// numbers, kept for a physically based path later.
#[derive(Clone, Debug)]
pub struct GMaterial {
    pub material: Material,
    pub image: Option<usize>,
    pub pbr: Pbr,
}

/// glTF's metallic-roughness material as the file gave it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pbr {
    pub base: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    /// Texture index and its uv set, for each map the file names.
    pub base_texture: Option<(usize, u32)>,
    pub metallic_roughness_texture: Option<(usize, u32)>,
    pub normal_texture: Option<(usize, u32)>,
    pub occlusion_texture: Option<(usize, u32)>,
    pub emissive_texture: Option<(usize, u32)>,
    pub alpha_cutoff: Option<f32>,
    /// `MASK` in the file; drawn blended until the lit path tests alpha.
    pub masked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Translation,
    Rotation,
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    Step,
    Linear,
    CubicSpline,
}

/// One animated property of one node.
#[derive(Clone, Debug)]
pub struct Channel {
    pub node: usize,
    pub path: Path,
    pub interp: Interp,
    /// Keyframe times, seconds, never decreasing.
    pub times: Vec<f32>,
    /// One value per time (three for a cubic spline: in-tangent, value,
    /// out-tangent), a vector padded with 0 or a quaternion.
    pub values: Vec<[f32; 4]>,
}

/// One animation of the file.
#[derive(Clone, Debug)]
pub struct Clip {
    pub channels: Vec<Channel>,
    /// The last keyframe time, seconds.
    pub duration: f32,
}

/// What was in the file that is not drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skipped {
    /// Points and lines.
    pub primitives: usize,
    /// Pictures named by an external URI.
    pub images: usize,
    /// Morph-target weight channels.
    pub channels: usize,
}

/// A whole file, read.
#[derive(Clone, Debug)]
pub struct Asset {
    pub nodes: Vec<GNode>,
    /// Each scene's root nodes. A file with no scenes has one, of every
    /// node without a parent.
    pub scenes: Vec<Vec<usize>>,
    /// The scene the file names as its own, else the first.
    pub scene: usize,
    /// Each mesh's primitives, by index into `prims`.
    pub meshes: Vec<Vec<usize>>,
    pub prims: Vec<Prim>,
    pub materials: Vec<GMaterial>,
    pub skins: Vec<GSkin>,
    pub clips: Vec<Clip>,
    pub skipped: Skipped,
    pub vertices: usize,
    pub triangles: usize,
    /// What its meshes cost on the GPU.
    pub bytes: usize,
}

/// A decoded colour texture.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// An asset and the pictures its materials name.
#[derive(Debug)]
pub struct Loaded {
    pub asset: Asset,
    pub images: Vec<Image>,
}

/// Read a `.glb` or a `.gltf` (with its buffers inside it).
pub fn load(bytes: &[u8]) -> Result<Loaded, WireError> {
    if bytes.len() > ASSET_BYTES {
        return Err(err("EFBIG", format!("an asset is capped at {ASSET_BYTES} bytes")));
    }
    let (json_bytes, bin) = if bytes.starts_with(b"glTF") { glb(bytes)? } else { (bytes, None) };
    let json_bytes = json_bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(json_bytes);
    if json_bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
        return Err(err("EBADF", "the asset is neither a glb nor glTF JSON"));
    }
    structures(json_bytes)?;
    let root: json::Root = serde_json::from_slice(json_bytes).map_err(|e| {
        err("EBADF", format!("the glTF JSON does not parse (line {}, column {})", e.line(), e.column()))
    })?;
    Reader::new(&root, bin)?.read()
}

/// The GLB container: a 12-byte header, a JSON chunk, then perhaps a BIN
/// chunk; other chunks are passed over. Every length is checked against
/// what is there.
fn glb(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>), WireError> {
    let word = |at: usize| -> Option<u32> {
        bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let (Some(version), Some(length)) = (word(4), word(8)) else {
        return Err(err("EBADF", "the glb is truncated in its header"));
    };
    if version != 2 {
        return Err(err("EINVAL", "the glb's version must be 2"));
    }
    let length = length as usize;
    if length < 12 || length > bytes.len() {
        return Err(err("EBADF", "the glb's length is not the data it holds (truncated?)"));
    }
    let mut at = 12usize;
    let mut json_chunk = None;
    let mut bin = None;
    while at < length {
        let (Some(chunk_len), Some(kind)) = (word(at), word(at + 4)) else {
            return Err(err("EBADF", "a glb chunk header is truncated"));
        };
        let start = at + 8;
        let end = start.checked_add(chunk_len as usize).filter(|end| *end <= length);
        let Some(end) = end else {
            return Err(err("EBADF", "a glb chunk's length runs past the file"));
        };
        let data = &bytes[start..end];
        match kind {
            0x4E4F_534A if json_chunk.is_none() => json_chunk = Some(data),
            0x004E_4942 if json_chunk.is_some() && bin.is_none() => bin = Some(data),
            0x4E4F_534A | 0x004E_4942 => return Err(err("EBADF", "the glb's chunks are out of order")),
            _ => {}
        }
        if json_chunk.is_none() {
            return Err(err("EBADF", "the glb's first chunk must be JSON"));
        }
        // Chunks start on four-byte boundaries.
        at = end.saturating_add(3) & !3;
    }
    let json_chunk = json_chunk.ok_or_else(|| err("EBADF", "the glb has no JSON chunk"))?;
    Ok((json_chunk, bin))
}

/// Count objects and arrays outside strings, refusing past the cap.
fn structures(json: &[u8]) -> Result<(), WireError> {
    let (mut count, mut in_string, mut escaped) = (0usize, false, false);
    for &b in json {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
        } else if b == b'"' {
            in_string = true;
        } else if b == b'{' || b == b'[' {
            count += 1;
            if count > JSON_STRUCTURES {
                return Err(err("EFBIG", format!("the glTF JSON is capped at {JSON_STRUCTURES} objects and arrays")));
            }
        }
    }
    Ok(())
}

/// The bytes of a `data:` URI with base64 data.
fn data_uri(uri: &str) -> Option<Vec<u8>> {
    let rest = uri.strip_prefix("data:")?;
    let (head, data) = rest.split_once(',')?;
    if !head.ends_with(";base64") {
        return None;
    }
    super::base64(data.as_bytes())
}

fn component(c: &Checked<json::accessor::GenericComponentType>) -> Option<json::accessor::ComponentType> {
    match c {
        Checked::Valid(json::accessor::GenericComponentType(c)) => Some(*c),
        Checked::Invalid => None,
    }
}

fn component_size(c: json::accessor::ComponentType) -> usize {
    use json::accessor::ComponentType as C;
    match c {
        C::I8 | C::U8 => 1,
        C::I16 | C::U16 => 2,
        C::U32 | C::F32 => 4,
    }
}

/// One component as a number, normalized when the accessor says so.
fn float_of(c: json::accessor::ComponentType, normalized: bool, b: &[u8]) -> f32 {
    use json::accessor::ComponentType as C;
    let Some(b) = b.get(..component_size(c)) else { return 0.0 };
    match c {
        C::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        C::U8 => {
            let v = f32::from(b[0]);
            if normalized { v / 255.0 } else { v }
        }
        C::I8 => {
            let v = f32::from(b[0] as i8);
            if normalized { (v / 127.0).max(-1.0) } else { v }
        }
        C::U16 => {
            let v = f32::from(u16::from_le_bytes([b[0], b[1]]));
            if normalized { v / 65_535.0 } else { v }
        }
        C::I16 => {
            let v = f32::from(i16::from_le_bytes([b[0], b[1]]));
            if normalized { (v / 32_767.0).max(-1.0) } else { v }
        }
        C::U32 => {
            let v = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32;
            if normalized { v / u32::MAX as f32 } else { v }
        }
    }
}

/// One component as an unsigned integer (an index).
fn uint_of(c: json::accessor::ComponentType, b: &[u8]) -> u32 {
    use json::accessor::ComponentType as C;
    let Some(b) = b.get(..component_size(c)) else { return u32::MAX };
    match c {
        C::U8 => u32::from(b[0]),
        C::U16 => u32::from(u16::from_le_bytes([b[0], b[1]])),
        C::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        _ => u32::MAX,
    }
}

/// The file's JSON and its buffers' bytes, read with every index checked.
struct Reader<'a> {
    root: &'a json::Root,
    buffers: Vec<Cow<'a, [u8]>>,
}

impl<'a> Reader<'a> {
    fn new(root: &'a json::Root, bin: Option<&'a [u8]>) -> Result<Self, WireError> {
        if !root.asset.version.starts_with("2.") && root.asset.version != "2" {
            return Err(err("EINVAL", "the glTF version must be 2.x"));
        }
        let unsupported = root.extensions_required.iter().filter(|e| e.as_str() != "KHR_mesh_quantization").count();
        if unsupported > 0 {
            return Err(err("EINVAL", format!("the asset requires {unsupported} extension(s) glass does not take")));
        }
        let counts: [(&str, usize, usize); 11] = [
            ("nodes", root.nodes.len(), ASSET_NODES),
            ("meshes", root.meshes.len(), ASSET_MESHES),
            ("accessors", root.accessors.len(), ASSET_ACCESSORS),
            ("buffer views", root.buffer_views.len(), ASSET_ACCESSORS),
            ("buffers", root.buffers.len(), ASSET_BUFFERS),
            ("materials", root.materials.len(), ASSET_MATERIALS),
            ("textures", root.textures.len(), ASSET_ACCESSORS),
            ("samplers", root.samplers.len(), ASSET_ACCESSORS),
            ("images", root.images.len(), ASSET_ACCESSORS),
            ("animations", root.animations.len(), ASSET_CLIPS),
            ("skins", root.skins.len(), ASSET_SKINS),
        ];
        for (name, count, cap) in counts {
            if count > cap {
                return Err(err("EFBIG", format!("an asset is capped at {cap} {name}; this one has {count}")));
            }
        }
        if root.scenes.len() > ASSET_NODES {
            return Err(err("EFBIG", format!("an asset is capped at {ASSET_NODES} scenes")));
        }
        let mut buffers = Vec::with_capacity(root.buffers.len());
        for (i, buffer) in root.buffers.iter().enumerate() {
            let data: Cow<'a, [u8]> = match &buffer.uri {
                None => match bin {
                    Some(bin) if i == 0 => Cow::Borrowed(bin),
                    _ => return Err(err("EINVAL", format!("buffer {i} has no data"))),
                },
                Some(uri) => Cow::Owned(data_uri(uri).ok_or_else(|| {
                    err("EINVAL", format!("buffer {i} is not a base64 data: uri; glass reads no other file"))
                })?),
            };
            let want = buffer.byte_length.0;
            if (data.len() as u64) < want {
                return Err(err("EBADF", format!("buffer {i} is shorter than its byteLength")));
            }
            let data = match data {
                Cow::Borrowed(b) => Cow::Borrowed(&b[..want as usize]),
                Cow::Owned(mut v) => {
                    v.truncate(want as usize);
                    Cow::Owned(v)
                }
            };
            buffers.push(data);
        }
        Ok(Self { root, buffers })
    }

    /// A buffer view's bytes and its stride.
    fn view(&self, index: usize) -> Result<(&[u8], Option<usize>), WireError> {
        let view = self
            .root
            .buffer_views
            .get(index)
            .ok_or_else(|| err("EINVAL", format!("buffer view {index} is out of range")))?;
        let buffer = self
            .buffers
            .get(view.buffer.value())
            .ok_or_else(|| err("EINVAL", format!("buffer view {index} names a buffer out of range")))?;
        let offset = view.byte_offset.map_or(0, |o| o.0);
        let end = offset.checked_add(view.byte_length.0).filter(|end| *end <= buffer.len() as u64);
        let Some(end) = end else {
            return Err(err("EBADF", format!("buffer view {index} runs past its buffer")));
        };
        Ok((&buffer[offset as usize..end as usize], view.byte_stride.map(|s| s.0)))
    }

    /// How many items accessor `index` says it holds.
    fn count(&self, index: usize, what: &str) -> Result<u64, WireError> {
        self.root
            .accessors
            .get(index)
            .map(|a| a.count.0)
            .ok_or_else(|| err("EINVAL", format!("{what} accessor {index} is out of range")))
    }

    /// Accessor `index`'s items, `width` components each, converted by
    /// `conv`; at most `cap` items, refused before anything is allocated.
    fn items<T: Copy + Default>(
        &self,
        index: usize,
        cap: usize,
        what: &str,
        conv: impl Fn(json::accessor::ComponentType, bool, &[u8]) -> T,
    ) -> Result<(usize, Vec<T>), WireError> {
        self.read_items(index, cap, what, conv, false)
    }

    /// Accessor `index`'s 4x4 float matrices (inverse bind matrices),
    /// column-major as glTF stores them; at most `cap`.
    fn mat4s(&self, index: usize, cap: usize, what: &str) -> Result<Vec<Mat4>, WireError> {
        let a = self
            .root
            .accessors
            .get(index)
            .ok_or_else(|| err("EINVAL", format!("{what} accessor {index} is out of range")))?;
        if !matches!(a.type_, Checked::Valid(json::accessor::Type::Mat4))
            || component(&a.component_type) != Some(json::accessor::ComponentType::F32)
        {
            return Err(err("EINVAL", format!("{what} accessor {index} is not 4x4 float matrices")));
        }
        let (_, values) = self.read_items(index, cap, what, float_of, true)?;
        if values.iter().any(|v| !v.is_finite()) {
            return Err(err("EINVAL", format!("{what} accessor {index} holds a number that is not finite")));
        }
        Ok(values.chunks_exact(16).map(|m| std::array::from_fn(|c| [m[c * 4], m[c * 4 + 1], m[c * 4 + 2], m[c * 4 + 3]])).collect())
    }

    /// `items`, and with `matrix` a 4x4 matrix accessor's sixteen numbers
    /// an item (only a float one: its columns need no padding).
    fn read_items<T: Copy + Default>(
        &self,
        index: usize,
        cap: usize,
        what: &str,
        conv: impl Fn(json::accessor::ComponentType, bool, &[u8]) -> T,
        matrix: bool,
    ) -> Result<(usize, Vec<T>), WireError> {
        let a = self
            .root
            .accessors
            .get(index)
            .ok_or_else(|| err("EINVAL", format!("{what} accessor {index} is out of range")))?;
        if a.count.0 > cap as u64 {
            return Err(err("EFBIG", format!("{what} accessor {index} holds {} items; the cap is {cap}", a.count.0)));
        }
        let count = a.count.0 as usize;
        let width = match a.type_ {
            Checked::Valid(json::accessor::Type::Scalar) => 1,
            Checked::Valid(json::accessor::Type::Vec2) => 2,
            Checked::Valid(json::accessor::Type::Vec3) => 3,
            Checked::Valid(json::accessor::Type::Vec4) => 4,
            Checked::Valid(json::accessor::Type::Mat4) if matrix => 16,
            Checked::Valid(_) => return Err(err("EINVAL", format!("{what} accessor {index} is a matrix"))),
            Checked::Invalid => return Err(err("EINVAL", format!("{what} accessor {index} has no valid type"))),
        };
        let c = component(&a.component_type)
            .ok_or_else(|| err("EINVAL", format!("{what} accessor {index} has no valid component type")))?;
        let size = component_size(c);
        let item = width * size;
        let mut out = vec![T::default(); count * width];
        if let Some(view) = a.buffer_view {
            let (bytes, stride) = self.view(view.value())?;
            let stride = stride.unwrap_or(item);
            if stride < item {
                return Err(err("EINVAL", format!("{what} accessor {index}'s stride is shorter than an item")));
            }
            let offset = a.byte_offset.map_or(0, |o| o.0);
            if count > 0 {
                let need = (stride as u64)
                    .checked_mul(count as u64 - 1)
                    .and_then(|n| n.checked_add(offset))
                    .and_then(|n| n.checked_add(item as u64));
                if need.is_none_or(|need| need > bytes.len() as u64) {
                    return Err(err("EBADF", format!("{what} accessor {index} runs past its buffer view")));
                }
            }
            let offset = offset as usize;
            for i in 0..count {
                let at = offset + i * stride;
                for k in 0..width {
                    out[i * width + k] = conv(c, a.normalized, &bytes[at + k * size..]);
                }
            }
        }
        if let Some(sparse) = &a.sparse {
            let n = sparse.count.0;
            if n > count as u64 {
                return Err(err("EINVAL", format!("{what} accessor {index} has more sparse items than items")));
            }
            let n = n as usize;
            let ic = match sparse.indices.component_type {
                Checked::Valid(json::accessor::IndexComponentType(c)) => c,
                Checked::Invalid => {
                    return Err(err("EINVAL", format!("{what} accessor {index}'s sparse indices have no valid type")))
                }
            };
            let isize_ = component_size(ic);
            let (ibytes, _) = self.view(sparse.indices.buffer_view.value())?;
            let (vbytes, _) = self.view(sparse.values.buffer_view.value())?;
            let ioff = sparse.indices.byte_offset.0;
            let voff = sparse.values.byte_offset.0;
            let fits = |off: u64, each: usize, bytes: &[u8]| {
                (each as u64).checked_mul(n as u64).and_then(|x| x.checked_add(off)).is_some_and(|end| end <= bytes.len() as u64)
            };
            if !fits(ioff, isize_, ibytes) || !fits(voff, item, vbytes) {
                return Err(err("EBADF", format!("{what} accessor {index}'s sparse data runs past its view")));
            }
            for k in 0..n {
                let at = uint_of(ic, &ibytes[ioff as usize + k * isize_..]) as usize;
                if at >= count {
                    return Err(err("EINVAL", format!("{what} accessor {index} has a sparse index out of range")));
                }
                for j in 0..width {
                    out[at * width + j] = conv(c, a.normalized, &vbytes[voff as usize + k * item + j * size..]);
                }
            }
        }
        Ok((width, out))
    }

    /// Floats, every one finite.
    fn floats(&self, index: usize, cap: usize, what: &str) -> Result<(usize, Vec<f32>), WireError> {
        let (width, values) = self.items(index, cap, what, float_of)?;
        if values.iter().any(|v| !v.is_finite()) {
            return Err(err("EINVAL", format!("{what} accessor {index} holds a number that is not finite")));
        }
        Ok((width, values))
    }

    fn read(self) -> Result<Loaded, WireError> {
        let root = self.root;
        let (nodes, scenes, scene) = self.tree()?;
        let mut images = Vec::new();
        let mut decoded: Vec<Option<usize>> = vec![None; root.images.len()];
        let mut texture_bytes = 0usize;
        let mut skipped = Skipped::default();
        let mut materials = Vec::with_capacity(root.materials.len() + 1);
        for (i, m) in root.materials.iter().enumerate() {
            materials.push(self.material(i, m, &mut images, &mut decoded, &mut texture_bytes, &mut skipped)?);
        }
        // Primitives without a material take glTF's default one.
        let default_material = materials.len();
        materials.push(GMaterial { material: lit_material(&Pbr::default_material(), Sampling::Linear, false), image: None, pbr: Pbr::default_material() });
        let mut meshes = Vec::with_capacity(root.meshes.len());
        let mut prims = Vec::new();
        let (mut vertices, mut indices, mut triangles) = (0usize, 0usize, 0usize);
        for (mi, mesh) in root.meshes.iter().enumerate() {
            let mut list = Vec::with_capacity(mesh.primitives.len());
            for (pi, p) in mesh.primitives.iter().enumerate() {
                if prims.len() >= ASSET_PRIMITIVES {
                    return Err(err("EFBIG", format!("an asset is capped at {ASSET_PRIMITIVES} primitives")));
                }
                let material = match p.material {
                    Some(m) if m.value() < root.materials.len() => m.value(),
                    Some(m) => return Err(err("EINVAL", format!("mesh {mi} primitive {pi} names material {}, out of range", m.value()))),
                    None => default_material,
                };
                let uv_set = materials[material].pbr.base_texture.map_or(0, |(_, set)| set);
                let Some(data) = self.primitive(mi, pi, p, uv_set, &mut vertices, &mut indices)? else {
                    skipped.primitives += 1;
                    continue;
                };
                triangles += data.indices.len() / 3;
                list.push(prims.len());
                prims.push(Prim { mesh: Arc::new(data), material, id: 0 });
            }
            meshes.push(list);
        }
        for (i, node) in root.nodes.iter().enumerate() {
            if let Some(m) = node.mesh
                && m.value() >= meshes.len()
            {
                return Err(err("EINVAL", format!("node {i} names mesh {}, out of range", m.value())));
            }
        }
        let skins = self.skins(nodes.len())?;
        // A skinned node's mesh names no joint its skin does not have.
        for (i, node) in nodes.iter().enumerate() {
            let (Some(k), Some(m)) = (node.skin, node.mesh) else { continue };
            let joints = skins[k].joints.len();
            for &p in meshes.get(m).map_or(&[][..], Vec::as_slice) {
                if let Some(j) = prims[p].mesh.joints.iter().flatten().copied().find(|&j| usize::from(j) >= joints) {
                    return Err(err("EINVAL", format!("node {i}'s mesh {m} names joint {j}, past skin {k}'s {joints} joints")));
                }
            }
        }
        let clips = self.clips(nodes.len(), &mut skipped)?;
        let skinned: usize = prims.iter().map(|p| p.mesh.joints.len()).sum();
        let bytes = vertices * VERTEX_BYTES + indices * INDEX_BYTES + skinned * SKIN_BYTES;
        let asset = Asset { nodes, scenes, scene, meshes, prims, materials, skins, clips, skipped, vertices, triangles, bytes };
        Ok(Loaded { asset, images })
    }

    /// Every skin: at most [`ASSET_JOINTS`] joints, each a node that is
    /// there, and an inverse bind matrix for each (the identity when the
    /// file gives none).
    fn skins(&self, nodes: usize) -> Result<Vec<GSkin>, WireError> {
        let mut skins = Vec::with_capacity(self.root.skins.len());
        for (k, skin) in self.root.skins.iter().enumerate() {
            let n = skin.joints.len();
            if n > ASSET_JOINTS {
                return Err(err("EFBIG", format!("skin {k} has {n} joints; the cap is {ASSET_JOINTS}")));
            }
            if n == 0 {
                return Err(err("EINVAL", format!("skin {k} has no joints")));
            }
            let mut joints = Vec::with_capacity(n);
            for j in &skin.joints {
                let j = j.value();
                if j >= nodes {
                    return Err(err("EINVAL", format!("skin {k} names joint node {j}, out of range")));
                }
                joints.push(j);
            }
            let inverse_bind: Arc<[Mat4]> = match skin.inverse_bind_matrices {
                None => vec![IDENTITY_MATRIX; n].into(),
                Some(a) => {
                    let what = "inverseBindMatrices";
                    if self.count(a.value(), what)? < n as u64 {
                        return Err(err("EINVAL", format!("skin {k}'s inverse bind matrices are fewer than its {n} joints")));
                    }
                    let mut m = self.mat4s(a.value(), ASSET_JOINTS.max(n), what)?;
                    m.truncate(n);
                    m.into()
                }
            };
            skins.push(GSkin { joints, inverse_bind });
        }
        Ok(skins)
    }

    /// The node tree: transforms, parents (each node at most one, no
    /// cycle, at most [`ASSET_DEPTH`] deep), and each scene's roots.
    #[allow(clippy::type_complexity)]
    fn tree(&self) -> Result<(Vec<GNode>, Vec<Vec<usize>>, usize), WireError> {
        let root = self.root;
        let n = root.nodes.len();
        let mut nodes = Vec::with_capacity(n);
        for (i, node) in root.nodes.iter().enumerate() {
            let transform = match node.matrix {
                Some(m) => decompose(&m).ok_or_else(|| err("EINVAL", format!("node {i}'s matrix is not a finite transform")))?,
                None => {
                    let translation = node.translation.unwrap_or([0.0; 3]);
                    let rotation = node.rotation.map_or([0.0, 0.0, 0.0, 1.0], |q| q.0);
                    let scale = node.scale.unwrap_or([1.0; 3]);
                    if translation.iter().chain(&rotation).chain(&scale).any(|v| !v.is_finite()) {
                        return Err(err("EINVAL", format!("node {i}'s transform is not finite")));
                    }
                    Transform { translation, rotation: angel_glass::scene::quat_normalize(rotation), scale }
                }
            };
            let mut children = Vec::new();
            for c in node.children.iter().flatten() {
                let c = c.value();
                if c >= n {
                    return Err(err("EINVAL", format!("node {i} names child {c}, out of range")));
                }
                children.push(c);
            }
            let skin = match node.skin {
                Some(k) if k.value() < root.skins.len() => Some(k.value()),
                Some(k) => return Err(err("EINVAL", format!("node {i} names skin {}, out of range", k.value()))),
                None => None,
            };
            nodes.push(GNode { transform, children, parent: None, mesh: node.mesh.map(|m| m.value()), skin });
        }
        for i in 0..n {
            for k in 0..nodes[i].children.len() {
                let c = nodes[i].children[k];
                if c == i {
                    return Err(err("EINVAL", format!("node {i} is its own child: the node tree cycles")));
                }
                if nodes[c].parent.is_some() {
                    return Err(err("EINVAL", format!("node {c} has two parents")));
                }
                nodes[c].parent = Some(i);
            }
        }
        // Depth by walking up, remembered, so the whole tree costs one
        // walk; a walk longer than the node count is a cycle.
        let mut depth: Vec<Option<usize>> = vec![None; n];
        let mut chain = Vec::new();
        for start in 0..n {
            chain.clear();
            let mut at = start;
            let base = loop {
                if let Some(d) = depth[at] {
                    break d;
                }
                chain.push(at);
                if chain.len() > n {
                    return Err(err("EINVAL", format!("node {start}'s parents cycle")));
                }
                match nodes[at].parent {
                    Some(p) => at = p,
                    None => break 0,
                }
            };
            let mut d = base;
            for &i in chain.iter().rev() {
                d += 1;
                if d > ASSET_DEPTH {
                    return Err(err("EINVAL", format!("the node tree is deeper than {ASSET_DEPTH}")));
                }
                depth[i] = Some(d);
            }
        }
        let mut scenes = Vec::with_capacity(root.scenes.len().max(1));
        for (si, s) in root.scenes.iter().enumerate() {
            let mut roots = Vec::with_capacity(s.nodes.len());
            for r in &s.nodes {
                let r = r.value();
                if r >= n {
                    return Err(err("EINVAL", format!("scene {si} names node {r}, out of range")));
                }
                if nodes[r].parent.is_some() {
                    return Err(err("EINVAL", format!("scene {si} names node {r}, which is not a root")));
                }
                if roots.contains(&r) {
                    return Err(err("EINVAL", format!("scene {si} names node {r} twice")));
                }
                roots.push(r);
            }
            scenes.push(roots);
        }
        if scenes.is_empty() {
            scenes.push((0..n).filter(|i| nodes[*i].parent.is_none()).collect());
        }
        let scene = match root.scene {
            Some(s) if s.value() < scenes.len() => s.value(),
            Some(s) => return Err(err("EINVAL", format!("the default scene {} is out of range", s.value()))),
            None => 0,
        };
        Ok((nodes, scenes, scene))
    }

    #[allow(clippy::too_many_arguments)]
    fn material(
        &self,
        index: usize,
        m: &json::Material,
        images: &mut Vec<Image>,
        decoded: &mut [Option<usize>],
        texture_bytes: &mut usize,
        skipped: &mut Skipped,
    ) -> Result<GMaterial, WireError> {
        let root = self.root;
        let pbr_json = &m.pbr_metallic_roughness;
        let texture_of = |info: Option<(usize, u32)>| -> Result<Option<(usize, u32)>, WireError> {
            match info {
                Some((t, set)) if t < root.textures.len() => Ok(Some((t, set))),
                Some((t, _)) => Err(err("EINVAL", format!("material {index} names texture {t}, out of range"))),
                None => Ok(None),
            }
        };
        let finite = |v: &[f32]| v.iter().all(|x| x.is_finite());
        let base = pbr_json.base_color_factor.0;
        let emissive = m.emissive_factor.0;
        let (metallic, roughness) = (pbr_json.metallic_factor.0, pbr_json.roughness_factor.0);
        if !finite(&base) || !finite(&emissive) || !metallic.is_finite() || !roughness.is_finite() {
            return Err(err("EINVAL", format!("material {index} has a factor that is not finite")));
        }
        let (alpha_blend, masked) = match m.alpha_mode {
            Checked::Valid(json::material::AlphaMode::Opaque) => (false, false),
            Checked::Valid(json::material::AlphaMode::Blend) => (true, false),
            Checked::Valid(json::material::AlphaMode::Mask) => (true, true),
            Checked::Invalid => return Err(err("EINVAL", format!("material {index} has no valid alphaMode"))),
        };
        let pbr = Pbr {
            base: base.map(|v| v.clamp(0.0, 1.0)),
            metallic: metallic.clamp(0.0, 1.0),
            roughness: roughness.clamp(0.0, 1.0),
            emissive: emissive.map(|v| v.clamp(0.0, 1.0)),
            base_texture: texture_of(pbr_json.base_color_texture.as_ref().map(|t| (t.index.value(), t.tex_coord)))?,
            metallic_roughness_texture: texture_of(
                pbr_json.metallic_roughness_texture.as_ref().map(|t| (t.index.value(), t.tex_coord)),
            )?,
            normal_texture: texture_of(m.normal_texture.as_ref().map(|t| (t.index.value(), t.tex_coord)))?,
            occlusion_texture: texture_of(m.occlusion_texture.as_ref().map(|t| (t.index.value(), t.tex_coord)))?,
            emissive_texture: texture_of(m.emissive_texture.as_ref().map(|t| (t.index.value(), t.tex_coord)))?,
            alpha_cutoff: m.alpha_cutoff.map(|c| c.0),
            masked,
        };
        let mut sampling = Sampling::Linear;
        let mut image = None;
        if let Some((t, _)) = pbr.base_texture {
            let texture = &root.textures[t];
            let source = texture.source.value();
            if source >= root.images.len() {
                return Err(err("EINVAL", format!("texture {t} names image {source}, out of range")));
            }
            if let Some(s) = texture.sampler {
                let sampler = root
                    .samplers
                    .get(s.value())
                    .ok_or_else(|| err("EINVAL", format!("texture {t} names sampler {}, out of range", s.value())))?;
                if matches!(sampler.mag_filter, Some(Checked::Valid(json::texture::MagFilter::Nearest))) {
                    sampling = Sampling::Nearest;
                }
            }
            image = match decoded[source] {
                Some(at) => Some(at),
                None => self.image(source, images, texture_bytes, skipped)?.inspect(|at| decoded[source] = Some(*at)),
            };
        }
        let material = lit_material(&pbr, sampling, alpha_blend);
        let material = Material { double_sided: m.double_sided, ..material };
        Ok(GMaterial { material, image, pbr })
    }

    /// Decode image `index` into `images`; `None` when it is named by an
    /// external URI, which glass does not read.
    fn image(
        &self,
        index: usize,
        images: &mut Vec<Image>,
        texture_bytes: &mut usize,
        skipped: &mut Skipped,
    ) -> Result<Option<usize>, WireError> {
        let image = &self.root.images[index];
        let hint = match image.mime_type.as_ref().map(|m| m.0.as_str()) {
            Some("image/png") => Some("png"),
            Some("image/jpeg") => Some("jpeg"),
            _ => None,
        };
        let owned;
        let bytes: &[u8] = if let Some(view) = image.buffer_view {
            self.view(view.value())?.0
        } else if let Some(uri) = &image.uri {
            match data_uri(uri) {
                Some(data) => {
                    owned = data;
                    &owned
                }
                None => {
                    skipped.images += 1;
                    return Ok(None);
                }
            }
        } else {
            return Err(err("EINVAL", format!("image {index} has neither a buffer view nor a uri")));
        };
        if images.len() >= ASSET_IMAGES {
            return Err(err("EFBIG", format!("an asset is capped at {ASSET_IMAGES} textures")));
        }
        let (width, height, rgba) = super::decode_image_hinted(bytes, hint)
            .ok_or_else(|| err("EBADF", format!("image {index} could not be decoded")))?;
        if width > IMAGE_SIDE || height > IMAGE_SIDE {
            return Err(err("EFBIG", format!("image {index} is past {IMAGE_SIDE} pixels a side")));
        }
        *texture_bytes += rgba.len();
        if *texture_bytes > ASSET_TEXTURE_BYTES {
            return Err(err("EFBIG", format!("an asset's textures are capped at {ASSET_TEXTURE_BYTES} bytes")));
        }
        images.push(Image { width, height, rgba });
        Ok(Some(images.len() - 1))
    }

    /// One primitive's triangles, or `None` for points and lines.
    fn primitive(
        &self,
        mi: usize,
        pi: usize,
        p: &json::mesh::Primitive,
        uv_set: u32,
        vertices: &mut usize,
        indices_total: &mut usize,
    ) -> Result<Option<MeshData>, WireError> {
        use json::mesh::{Mode, Semantic};
        let mode = match p.mode {
            Checked::Valid(mode) => mode,
            Checked::Invalid => return Err(err("EINVAL", format!("mesh {mi} primitive {pi} has no valid mode"))),
        };
        if matches!(mode, Mode::Points | Mode::Lines | Mode::LineLoop | Mode::LineStrip) {
            return Ok(None);
        }
        let attribute = |s: Semantic| p.attributes.get(&Checked::Valid(s)).map(|a| a.value());
        let position = attribute(Semantic::Positions)
            .ok_or_else(|| err("EINVAL", format!("mesh {mi} primitive {pi} has no POSITION")))?;
        let n = self.count(position, "POSITION")?;
        if n > MAX_MESH_VERTICES as u64 || *vertices as u64 + n > ASSET_VERTICES as u64 {
            return Err(err("EFBIG", format!("mesh {mi} primitive {pi} has {n} vertices: past the cap")));
        }
        let (width, flat) = self.floats(position, MAX_MESH_VERTICES, "POSITION")?;
        if width != 3 {
            return Err(err("EINVAL", format!("mesh {mi} primitive {pi}'s POSITION is not three numbers")));
        }
        let n = flat.len() / 3;
        let positions: Vec<[f32; 3]> = flat.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        let same = |what: &str, index: usize, want: &[usize]| -> Result<Vec<f32>, WireError> {
            let (w, values) = self.floats(index, MAX_MESH_VERTICES, what)?;
            if !want.contains(&w) || values.len() / w != n {
                return Err(err("EINVAL", format!("mesh {mi} primitive {pi}'s {what} does not match its POSITION")));
            }
            Ok(values)
        };
        let normals: Vec<[f32; 3]> = match attribute(Semantic::Normals) {
            Some(index) => same("NORMAL", index, &[3])?.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
            None => Vec::new(),
        };
        let uvs: Vec<[f32; 2]> = match attribute(Semantic::TexCoords(uv_set)) {
            Some(index) => same("TEXCOORD", index, &[2])?.chunks_exact(2).map(|c| [c[0], c[1]]).collect(),
            None => Vec::new(),
        };
        // A skinned primitive's four joints and weights a vertex: both, or
        // neither.
        let (joints, weights): (Vec<[u16; 4]>, Vec<[f32; 4]>) = match (attribute(Semantic::Joints(0)), attribute(Semantic::Weights(0))) {
            (None, None) => (Vec::new(), Vec::new()),
            (Some(j), Some(w)) => {
                let ja = self.root.accessors.get(j).ok_or_else(|| err("EINVAL", format!("JOINTS_0 accessor {j} is out of range")))?;
                if !matches!(component(&ja.component_type), Some(json::accessor::ComponentType::U8 | json::accessor::ComponentType::U16)) {
                    return Err(err("EINVAL", format!("mesh {mi} primitive {pi}'s JOINTS_0 are not unsigned bytes or shorts")));
                }
                let (jw, jv) = self.items(j, MAX_MESH_VERTICES, "JOINTS_0", |c, _, b| uint_of(c, b))?;
                if jw != 4 || jv.len() / 4 != n {
                    return Err(err("EINVAL", format!("mesh {mi} primitive {pi}'s JOINTS_0 does not match its POSITION")));
                }
                let wv = same("WEIGHTS_0", w, &[4])?;
                if wv.iter().any(|v| *v < 0.0) {
                    return Err(err("EINVAL", format!("mesh {mi} primitive {pi} has a negative weight")));
                }
                let joints = jv.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]].map(|v| v.min(u32::from(u16::MAX)) as u16)).collect();
                let weights = wv.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
                (joints, weights)
            }
            (Some(_), None) => return Err(err("EINVAL", format!("mesh {mi} primitive {pi} has JOINTS_0 without WEIGHTS_0"))),
            (None, Some(_)) => return Err(err("EINVAL", format!("mesh {mi} primitive {pi} has WEIGHTS_0 without JOINTS_0"))),
        };
        let colors: Vec<[f32; 4]> = match attribute(Semantic::Colors(0)) {
            Some(index) => {
                let (w, values) = self.floats(index, MAX_MESH_VERTICES, "COLOR_0")?;
                if !(w == 3 || w == 4) || values.len() / w != n {
                    return Err(err("EINVAL", format!("mesh {mi} primitive {pi}'s COLOR_0 does not match its POSITION")));
                }
                values
                    .chunks_exact(w)
                    .map(|c| [c[0], c[1], c[2], if w == 4 { c[3] } else { 1.0 }].map(|v| v.clamp(0.0, 1.0)))
                    .collect()
            }
            None => Vec::new(),
        };
        let raw: Vec<u32> = match p.indices {
            Some(index) => {
                let index = index.value();
                let a = self.root.accessors.get(index).ok_or_else(|| err("EINVAL", format!("index accessor {index} is out of range")))?;
                let ok = matches!(
                    component(&a.component_type),
                    Some(json::accessor::ComponentType::U8 | json::accessor::ComponentType::U16 | json::accessor::ComponentType::U32)
                );
                if !ok {
                    return Err(err("EINVAL", format!("mesh {mi} primitive {pi}'s indices are not unsigned integers")));
                }
                let (w, values) = self.items(index, MAX_MESH_INDICES, "index", |c, _, b| uint_of(c, b))?;
                if w != 1 {
                    return Err(err("EINVAL", format!("mesh {mi} primitive {pi}'s indices are not scalars")));
                }
                values
            }
            None => (0..n as u32).collect(),
        };
        if let Some(bad) = raw.iter().find(|&&i| i as usize >= n) {
            return Err(err("EINVAL", format!("mesh {mi} primitive {pi} has index {bad}, out of range of its {n} vertices")));
        }
        let mut list = Vec::with_capacity(raw.len());
        match mode {
            Mode::Triangles => {
                if !raw.len().is_multiple_of(3) {
                    return Err(err("EINVAL", format!("mesh {mi} primitive {pi}'s indices are not whole triangles")));
                }
                list = raw;
            }
            Mode::TriangleStrip => {
                for i in 0..raw.len().saturating_sub(2) {
                    let (a, b) = if i % 2 == 0 { (raw[i + 1], raw[i + 2]) } else { (raw[i + 2], raw[i + 1]) };
                    list.extend([raw[i], a, b]);
                }
            }
            _ => {
                for i in 1..raw.len().saturating_sub(1) {
                    list.extend([raw[i], raw[i + 1], raw[0]]);
                }
            }
        }
        if list.is_empty() {
            return Ok(None);
        }
        if list.len() > MAX_MESH_INDICES || *indices_total + list.len() > ASSET_INDICES {
            return Err(err("EFBIG", format!("mesh {mi} primitive {pi} has {} indices: past the cap", list.len())));
        }
        let mut mesh = MeshData { positions, normals, uvs, colors, indices: list, groups: Vec::new(), joints, weights };
        if mesh.normals.is_empty() {
            mesh = flat_normals(&mesh).ok_or_else(|| {
                err("EFBIG", format!("mesh {mi} primitive {pi} needs flat normals past the vertex cap"))
            })?;
            if *vertices + mesh.positions.len() > ASSET_VERTICES {
                return Err(err("EFBIG", format!("an asset is capped at {ASSET_VERTICES} vertices")));
            }
        }
        mesh.validate().map_err(|e| err("EINVAL", format!("mesh {mi} primitive {pi}: {e}")))?;
        *vertices += mesh.positions.len();
        *indices_total += mesh.indices.len();
        Ok(Some(mesh))
    }

    /// Every clip's channels, checked: each names a node and a sampler that
    /// are there, with times that never go back.
    fn clips(&self, nodes: usize, skipped: &mut Skipped) -> Result<Vec<Clip>, WireError> {
        use json::animation::{Interpolation, Property};
        let mut clips = Vec::with_capacity(self.root.animations.len());
        let (mut channels_total, mut keys) = (0usize, 0usize);
        for (ai, anim) in self.root.animations.iter().enumerate() {
            let mut channels = Vec::with_capacity(anim.channels.len().min(ASSET_CHANNELS));
            let mut duration = 0.0f32;
            for (ci, ch) in anim.channels.iter().enumerate() {
                channels_total += 1;
                if channels_total > ASSET_CHANNELS {
                    return Err(err("EFBIG", format!("an asset is capped at {ASSET_CHANNELS} animation channels")));
                }
                let sampler = anim
                    .samplers
                    .get(ch.sampler.value())
                    .ok_or_else(|| err("EINVAL", format!("animation {ai} channel {ci} names sampler {}, out of range", ch.sampler.value())))?;
                let node = ch.target.node.value();
                if node >= nodes {
                    return Err(err("EINVAL", format!("animation {ai} channel {ci} names node {node}, out of range")));
                }
                let path = match ch.target.path {
                    Checked::Valid(Property::Translation) => Path::Translation,
                    Checked::Valid(Property::Rotation) => Path::Rotation,
                    Checked::Valid(Property::Scale) => Path::Scale,
                    Checked::Valid(Property::MorphTargetWeights) => {
                        skipped.channels += 1;
                        continue;
                    }
                    Checked::Invalid => return Err(err("EINVAL", format!("animation {ai} channel {ci} has no valid path"))),
                };
                let interp = match sampler.interpolation {
                    Checked::Valid(Interpolation::Step) => Interp::Step,
                    Checked::Valid(Interpolation::Linear) => Interp::Linear,
                    Checked::Valid(Interpolation::CubicSpline) => Interp::CubicSpline,
                    Checked::Invalid => return Err(err("EINVAL", format!("animation {ai} channel {ci} has no valid interpolation"))),
                };
                let input = sampler.input.value();
                let output = sampler.output.value();
                let room = ASSET_KEYS.saturating_sub(keys);
                let wanted = self.count(input, "animation input")?.saturating_add(self.count(output, "animation output")?);
                if wanted > room as u64 {
                    return Err(err("EFBIG", format!("an asset is capped at {ASSET_KEYS} keyframe values")));
                }
                let (w, times) = self.floats(input, room, "animation input")?;
                if w != 1 || times.is_empty() {
                    return Err(err("EINVAL", format!("animation {ai} channel {ci}'s times are not one or more numbers")));
                }
                if times[0] < 0.0 || times.windows(2).any(|t| t[1] < t[0]) {
                    return Err(err("EINVAL", format!("animation {ai} channel {ci}'s times go back")));
                }
                let (w, flat) = self.floats(output, room, "animation output")?;
                let want_w = if path == Path::Rotation { 4 } else { 3 };
                let per = if interp == Interp::CubicSpline { 3 } else { 1 };
                if w != want_w || flat.len() / w != times.len() * per {
                    return Err(err("EINVAL", format!("animation {ai} channel {ci}'s values do not match its times")));
                }
                keys += times.len() + flat.len() / w;
                let values: Vec<[f32; 4]> =
                    flat.chunks_exact(w).map(|c| if w == 4 { [c[0], c[1], c[2], c[3]] } else { [c[0], c[1], c[2], 0.0] }).collect();
                duration = duration.max(*times.last().unwrap_or(&0.0));
                channels.push(Channel { node, path, interp, times, values });
            }
            clips.push(Clip { channels, duration });
        }
        Ok(clips)
    }
}

impl Pbr {
    /// glTF's default material: white, fully metallic and rough.
    fn default_material() -> Self {
        Pbr { base: [1.0; 4], metallic: 1.0, roughness: 1.0, ..Pbr::default() }
    }
}

/// A metallic-roughness material on the lit path: a smooth metal is mostly
/// reflection, which this path does not draw, so its colour is darkened by
/// up to half; the emissive colour shines through `emissive` and is mixed
/// into the tint by as much.
pub fn lit_material(pbr: &Pbr, sampling: Sampling, blended: bool) -> Material {
    let shine = pbr.metallic * (1.0 - pbr.roughness);
    let dim = 1.0 - 0.5 * shine;
    let glow = pbr.emissive.iter().copied().fold(0.0f32, f32::max).clamp(0.0, 1.0);
    let mut tint = [pbr.base[0] * dim, pbr.base[1] * dim, pbr.base[2] * dim, pbr.base[3]];
    for (k, t) in tint.iter_mut().take(3).enumerate() {
        *t = *t * (1.0 - glow) + pbr.emissive[k] * glow;
    }
    Material {
        tint: tint.map(|v| v.clamp(0.0, 1.0)),
        sampling,
        alpha: if blended { AlphaMode::Blend } else { AlphaMode::Opaque },
        lit: true,
        emissive: glow,
        ..Material::default()
    }
}

/// The same triangles, each with its own three vertices and its face's
/// normal (glTF's rule for a primitive without normals); `None` past the
/// window's vertex cap.
fn flat_normals(mesh: &MeshData) -> Option<MeshData> {
    let count = mesh.indices.len();
    if count > MAX_MESH_VERTICES {
        return None;
    }
    let mut out = MeshData {
        positions: Vec::with_capacity(count),
        normals: Vec::with_capacity(count),
        uvs: Vec::with_capacity(if mesh.uvs.is_empty() { 0 } else { count }),
        colors: Vec::with_capacity(if mesh.colors.is_empty() { 0 } else { count }),
        indices: (0..count as u32).collect(),
        groups: Vec::new(),
        joints: Vec::with_capacity(if mesh.joints.is_empty() { 0 } else { count }),
        weights: Vec::with_capacity(if mesh.weights.is_empty() { 0 } else { count }),
    };
    for tri in mesh.indices.chunks_exact(3) {
        let p = [tri[0], tri[1], tri[2]].map(|i| mesh.positions.get(i as usize).copied().unwrap_or([0.0; 3]));
        let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let v = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
        let c = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let len = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
        let normal = if len.is_finite() && len > 1e-20 { [c[0] / len, c[1] / len, c[2] / len] } else { [0.0, 0.0, 1.0] };
        for &i in tri {
            let i = i as usize;
            out.positions.push(mesh.positions.get(i).copied().unwrap_or([0.0; 3]));
            out.normals.push(normal);
            if !mesh.uvs.is_empty() {
                out.uvs.push(mesh.uvs.get(i).copied().unwrap_or([0.0; 2]));
            }
            if !mesh.colors.is_empty() {
                out.colors.push(mesh.colors.get(i).copied().unwrap_or([1.0; 4]));
            }
            if !mesh.joints.is_empty() {
                out.joints.push(mesh.joints.get(i).copied().unwrap_or([0; 4]));
                out.weights.push(mesh.weights.get(i).copied().unwrap_or([0.0; 4]));
            }
        }
    }
    Some(out)
}

/// A column-major 4x4 matrix as translation, rotation and scale. A matrix
/// with shear has none exactly; the rotation is then the one its columns
/// come nearest to. `None` when it is not finite.
pub fn decompose(m: &[f32; 16]) -> Option<Transform> {
    if m.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let col = |c: usize| [m[c * 4], m[c * 4 + 1], m[c * 4 + 2]];
    let (x, y, z) = (col(0), col(1), col(2));
    let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let det = {
        let c = cross(y, z);
        x[0] * c[0] + x[1] * c[1] + x[2] * c[2]
    };
    let mut scale = [len(x), len(y), len(z)];
    if det < 0.0 {
        scale[0] = -scale[0];
    }
    let unit = |v: [f32; 3], s: f32| if s.abs() > 1e-20 { [v[0] / s, v[1] / s, v[2] / s] } else { [0.0; 3] };
    let (ux, uy, uz) = (unit(x, scale[0]), unit(y, scale[1]), unit(z, scale[2]));
    let rotation = if scale.iter().all(|s| s.abs() > 1e-20) {
        quat_from_columns(ux, uy, uz)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    };
    Some(Transform { translation: [m[12], m[13], m[14]], rotation, scale })
}

/// The unit quaternion of a rotation given by its three columns.
fn quat_from_columns(x: [f32; 3], y: [f32; 3], z: [f32; 3]) -> [f32; 4] {
    let (m00, m01, m02) = (x[0], y[0], z[0]);
    let (m10, m11, m12) = (x[1], y[1], z[1]);
    let (m20, m21, m22) = (x[2], y[2], z[2]);
    let trace = m00 + m11 + m22;
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s]
    };
    angel_glass::scene::quat_normalize(q)
}

// ------------------------------------------------------------- sampling ----

impl Channel {
    /// The value at `t` seconds: held before the first key and after the
    /// last, stepped, mixed (rotations along the shorter arc) or a cubic
    /// Hermite spline between keys, as the channel's sampler says.
    pub fn sample(&self, t: f32) -> [f32; 4] {
        let n = self.times.len();
        let key = |k: usize| -> [f32; 4] {
            let at = if self.interp == Interp::CubicSpline { k * 3 + 1 } else { k };
            self.values.get(at).copied().unwrap_or([0.0, 0.0, 0.0, 1.0])
        };
        if n == 0 {
            return [0.0, 0.0, 0.0, 1.0];
        }
        if t.is_nan() || t <= self.times[0] {
            return key(0);
        }
        if t >= self.times[n - 1] {
            return key(n - 1);
        }
        let next = self.times.partition_point(|x| *x <= t).clamp(1, n - 1);
        let k = next - 1;
        let span = self.times[next] - self.times[k];
        if span.is_nan() || span <= 0.0 {
            return key(next);
        }
        let u = (t - self.times[k]) / span;
        let rotation = self.path == Path::Rotation;
        match self.interp {
            Interp::Step => key(k),
            Interp::Linear => {
                let (a, b) = (key(k), key(next));
                if rotation {
                    super::glass_scene::slerp(a, b, u)
                } else {
                    [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u, a[2] + (b[2] - a[2]) * u, 0.0]
                }
            }
            Interp::CubicSpline => {
                let get = |i: usize| self.values.get(i).copied().unwrap_or([0.0; 4]);
                let (v0, b0) = (get(k * 3 + 1), get(k * 3 + 2));
                let (a1, v1) = (get(next * 3), get(next * 3 + 1));
                let (u2, u3) = (u * u, u * u * u);
                let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
                let h10 = u3 - 2.0 * u2 + u;
                let h01 = -2.0 * u3 + 3.0 * u2;
                let h11 = u3 - u2;
                let mut out = [0.0f32; 4];
                for c in 0..4 {
                    out[c] = h00 * v0[c] + h10 * span * b0[c] + h01 * v1[c] + h11 * span * a1[c];
                }
                if rotation {
                    angel_glass::scene::quat_normalize(out)
                } else {
                    out[3] = 0.0;
                    out
                }
            }
        }
    }
}

/// Which clip plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipSel {
    One(usize),
    All,
}

/// A clip playing on one instance of an asset. Its position is kept as a
/// time and the clock it was true at; the clock is taken at the first tick
/// after a change, so a late view of the clock never makes it jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    pub clip: ClipSel,
    pub speed: f32,
    pub looping: bool,
    pub paused: bool,
    /// Seconds into the clip, unwrapped, at `since`.
    pub at: f64,
    /// The clock `at` was true at; `None` until the next tick.
    pub since: Option<u64>,
    /// The clip time last written, so a still pose is not written again.
    pub shown: Option<f32>,
}

impl Playback {
    pub fn new(clip: ClipSel) -> Self {
        Self { clip, speed: 1.0, looping: true, paused: false, at: 0.0, since: None, shown: None }
    }

    /// Seconds into the clip, unwrapped, at `clock`.
    pub fn position(&self, clock: u64) -> f64 {
        match self.since {
            Some(since) if !self.paused => self.at + clock.saturating_sub(since) as f64 / 1000.0 * f64::from(self.speed),
            _ => self.at,
        }
    }

    /// Hold the position here and take the clock again at the next tick.
    pub fn rebase(&mut self, clock: u64) {
        self.at = self.position(clock);
        self.since = None;
    }

    /// The clip time at `clock`, and whether a clip that does not loop has
    /// run out.
    pub fn local(&self, clock: u64, duration: f32) -> (f32, bool) {
        let p = self.position(clock);
        let d = f64::from(duration.max(0.0));
        if d <= 0.0 {
            return (0.0, !self.looping);
        }
        if self.looping {
            (p.rem_euclid(d) as f32, false)
        } else {
            let ended = (self.speed >= 0.0 && p >= d) || (self.speed < 0.0 && p <= 0.0);
            (p.clamp(0.0, d) as f32, ended)
        }
    }

    /// Whether it needs frames: started, not paused, moving, not run out.
    pub fn moving(&self, clock: u64, duration: f32) -> bool {
        self.since.is_some() && !self.paused && self.speed != 0.0 && !self.local(clock, duration).1
    }
}

impl Asset {
    /// How long the clips `sel` names run, seconds.
    pub fn duration(&self, sel: ClipSel) -> f32 {
        match sel {
            ClipSel::One(i) => self.clips.get(i).map_or(0.0, |c| c.duration),
            ClipSel::All => self.clips.iter().map(|c| c.duration).fold(0.0, f32::max),
        }
    }
}

// --------------------------------------------------------- instancing ----

/// Node ids an asset's nodes take in a scene: the top bit set, above every
/// id a program may name (docs/GLASS_DIALECT.md), then the instance, then
/// the node or, with the next bit, a primitive's own node.
pub const EXPANDED: u64 = 1 << 63;
const PRIM_NODE: u64 = 1 << 62;

fn node_id(instance: u32, index: usize) -> u64 {
    EXPANDED | (u64::from(instance) << 32) | (index as u64 & 0xffff_ffff)
}

/// One asset hung from one node of a scene.
#[derive(Clone, Debug)]
pub struct Instance {
    /// The program's node it hangs from.
    pub host: u64,
    pub asset_id: u32,
    pub asset: Arc<Asset>,
    /// Each of the asset's nodes to its place among the scene's nodes.
    pub places: Vec<Option<u32>>,
    /// Each drawing node's place, and the material the file gave it.
    pub drawn: Vec<(u32, Material)>,
    pub play: Option<Playback>,
}

/// Hang scene `which` of `asset` from node `host`, appending its nodes to
/// `out`. `room` is how many more nodes the scene may take.
pub fn instantiate(
    asset: &Arc<Asset>,
    asset_id: u32,
    which: usize,
    host: u64,
    instance: u32,
    room: usize,
    out: &mut Vec<Node>,
) -> Result<Instance, WireError> {
    let roots = asset
        .scenes
        .get(which)
        .ok_or_else(|| err("EINVAL", format!("asset {asset_id} has no scene {which}")))?;
    let mut places = vec![None; asset.nodes.len()];
    let mut drawn = Vec::new();
    // Each skin as this instance's joints name it, made at its first mesh.
    let mut skins: Vec<Option<Skin>> = vec![None; asset.skins.len()];
    let mut skin_of = |k: Option<usize>, mesh: &MeshData| -> Option<Skin> {
        let k = k?;
        if mesh.joints.is_empty() {
            return None;
        }
        let file = asset.skins.get(k)?;
        let slot = skins.get_mut(k)?;
        Some(
            slot.get_or_insert_with(|| Skin {
                joints: file.joints.iter().map(|&j| node_id(instance, j)).collect::<Vec<u64>>().into(),
                inverse_bind: Arc::clone(&file.inverse_bind),
            })
            .clone(),
        )
    };
    let mut stack: Vec<(usize, u64)> = roots.iter().rev().map(|&r| (r, host)).collect();
    let mut prim_nodes = 0u64;
    let start = out.len();
    while let Some((gi, parent)) = stack.pop() {
        let Some(g) = asset.nodes.get(gi) else { continue };
        if places[gi].is_some() {
            continue;
        }
        let prims: &[usize] = g.mesh.and_then(|m| asset.meshes.get(m)).map_or(&[], |v| v.as_slice());
        let need = 1 + if prims.len() > 1 { prims.len() } else { 0 };
        if out.len() - start + need > room {
            return Err(err("EFBIG", format!("asset {asset_id} takes the scene past {} nodes", room + start)));
        }
        let id = node_id(instance, gi);
        let place = out.len() as u32;
        places[gi] = Some(place);
        let mut node = Node::empty(id);
        node.parent = Some(parent);
        node.transform = g.transform;
        if let [only] = prims
            && let Some(prim) = asset.prims.get(*only)
        {
            node.mesh = Some(angel_glass::scene::MeshRef::Uploaded(prim.id));
            node.material = asset.materials.get(prim.material).map_or_else(Material::default, |m| m.material);
            node.skin = skin_of(g.skin, &prim.mesh);
            drawn.push((place, node.material));
        }
        out.push(node);
        if prims.len() > 1 {
            for &p in prims {
                let Some(prim) = asset.prims.get(p) else { continue };
                let material = asset.materials.get(prim.material).map_or_else(Material::default, |m| m.material);
                let mut child = Node::new(
                    EXPANDED | PRIM_NODE | (u64::from(instance) << 32) | (prim_nodes & 0xffff_ffff),
                    angel_glass::scene::MeshRef::Uploaded(prim.id),
                    material,
                );
                prim_nodes += 1;
                child.parent = Some(id);
                child.skin = skin_of(g.skin, &prim.mesh);
                drawn.push((out.len() as u32, material));
                out.push(child);
            }
        }
        for &c in g.children.iter().rev() {
            stack.push((c, id));
        }
    }
    Ok(Instance { host, asset_id, asset: Arc::clone(asset), places, drawn, play: None })
}

impl Instance {
    /// Every node of the asset back in the pose the file gave it.
    pub fn rest(&self, nodes: &mut [Node]) {
        for (gi, place) in self.places.iter().enumerate() {
            if let (Some(place), Some(g)) = (place, self.asset.nodes.get(gi))
                && let Some(node) = nodes.get_mut(*place as usize)
            {
                node.transform = g.transform;
            }
        }
    }

    /// The clips `sel` names, posed at `t` seconds.
    pub fn pose(&self, nodes: &mut [Node], sel: ClipSel, t: f32) {
        let clips: &[Clip] = match sel {
            ClipSel::One(i) => self.asset.clips.get(i..=i).unwrap_or(&[]),
            ClipSel::All => &self.asset.clips,
        };
        for clip in clips {
            for ch in &clip.channels {
                let Some(Some(place)) = self.places.get(ch.node) else { continue };
                let Some(node) = nodes.get_mut(*place as usize) else { continue };
                let v = ch.sample(t);
                match ch.path {
                    Path::Translation => node.transform.translation = [v[0], v[1], v[2]],
                    Path::Rotation => node.transform.rotation = v,
                    Path::Scale => node.transform.scale = [v[0], v[1], v[2]],
                }
            }
        }
    }

    /// Every drawing node's material as the host node's own makes it: as
    /// the file made it, its tint times the host's colour and shining at
    /// least the host's emissive, liquid as the host is. A chrome host
    /// draws every one in its own material instead (its colour the
    /// chrome's tint, no texture), each keeping only its own sides.
    pub fn shade(&self, nodes: &mut [Node], host: &Material) {
        let [chrome, liquid, _] = host.finish_clamped();
        for (place, base) in &self.drawn {
            let Some(node) = nodes.get_mut(*place as usize) else { continue };
            if chrome > 0.0 {
                node.material = Material { double_sided: base.double_sided, glow: 0.0, ..*host };
                continue;
            }
            let mut m = *base;
            for (k, c) in host.tint.iter().enumerate() {
                m.tint[k] = (base.tint[k] * c).clamp(0.0, 1.0);
            }
            m.emissive = base.emissive.max(host.emissive.clamp(0.0, 1.0));
            m.liquid = liquid;
            if m.tint[3] < 1.0 && m.alpha == AlphaMode::Opaque {
                m.alpha = AlphaMode::Blend;
            }
            node.material = m;
        }
    }
}
