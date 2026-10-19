//! Models for the showcase: an OBJ file read into one mesh, flat shaded,
//! centred and fitted to the turntable the window turns it on
//! (`angel_glass`'s showcase: a model's nodes hang from its root).
//!
//! The OBJ reader takes what modelling tools and the realm's own props
//! write: `v` (with optional vertex colours), `vn`, `f` with any of the
//! `a`, `a/b`, `a/b/c`, `a//c` forms (polygons fanned into triangles,
//! negative indices from the end) and `usemtl`, which splits the mesh
//! into parts. A part is coloured by its material's name: the realm's
//! material words (`stone`, `armor`, `cloth_gold` ...) have their tones;
//! any other name is steel. Files written in the realm's own convention
//! (its material words, z up) stand up the right way.

use angel_glass::scene::{Material, MeshData, MeshRef, Node};

/// The mesh id the showcase's model is uploaded under (one at a time).
pub(crate) const MESH: u32 = 0x5C0F_0001;
/// The most triangles a model may bring.
const MOST_TRIANGLES: usize = 400_000;

/// A model ready for the window: its mesh, the nodes that show it, and
/// the pictures its materials paint it with (`(id, width, height, rgba)`).
pub(crate) struct Model {
    pub(crate) mesh: MeshData,
    pub(crate) nodes: Vec<Node>,
    pub(crate) triangles: usize,
    pub(crate) textures: Vec<(u32, u32, u32, Vec<u8>)>,
}

/// The picture ids a model's textures go up under, and how many it may
/// bring; each is at most this many pixels a side.
const TEXTURES: u32 = angel_glass::LAYER_PICTURES + 0xA8;
const MOST_TEXTURES: usize = 4;
const TEXTURE_SIDE: u32 = 2048;

pub(crate) fn from_obj(text: &str) -> Result<Model, String> {
    let mut points: Vec<[f32; 3]> = Vec::new();
    let mut tints: Vec<Option<[f32; 3]>> = Vec::new();
    let mut tris: Vec<([usize; 3], usize)> = Vec::new();
    let mut parts: Vec<String> = vec![String::new()];
    let mut part = 0usize;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => {
                let n: Vec<f32> = words.filter_map(|w| w.parse().ok()).collect();
                if n.len() < 3 || !n[..3].iter().all(|v| v.is_finite()) {
                    return Err(format!("a vertex that is not three numbers: {line}"));
                }
                points.push([n[0], n[1], n[2]]);
                tints.push((n.len() >= 6).then(|| [n[3], n[4], n[5]]));
            }
            Some("usemtl") => {
                let name = words.next().unwrap_or("").to_string();
                part = match parts.iter().position(|p| *p == name) {
                    Some(i) => i,
                    None => {
                        parts.push(name);
                        parts.len() - 1
                    }
                };
            }
            Some("f") => {
                let corners: Vec<usize> = words
                    .map(|w| {
                        let raw: i64 = w.split('/').next().unwrap_or("").parse().map_err(|_| format!("a face corner that is not a number: {w}"))?;
                        let i = if raw < 0 { points.len() as i64 + raw } else { raw - 1 };
                        usize::try_from(i)
                            .ok()
                            .filter(|&i| i < points.len())
                            .ok_or_else(|| format!("a face naming a vertex that is not there: {w}"))
                    })
                    .collect::<Result<_, String>>()?;
                for k in 1..corners.len().saturating_sub(1) {
                    tris.push(([corners[0], corners[k], corners[k + 1]], part));
                }
                if tris.len() > MOST_TRIANGLES {
                    return Err(format!("more than {MOST_TRIANGLES} triangles"));
                }
            }
            _ => {}
        }
    }
    if tris.is_empty() {
        return Err("no faces".into());
    }
    // The realm's props are written z up; everything else y up.
    let realm = parts.iter().any(|p| realm_tone(p).is_some());
    let points: Vec<[f32; 3]> = if realm {
        points.iter().map(|&[x, y, z]| [x, z, -y]).collect()
    } else {
        points
    };
    // Centred on its middle, fitted to a sphere a little inside the unit.
    let (lo, hi) = points.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
        ([lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])], [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])])
    });
    let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let reach = points
        .iter()
        .map(|p| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2) + (p[2] - centre[2]).powi(2)).sqrt())
        .fold(0.0f32, f32::max)
        .max(1e-6);
    let k = 0.95 / reach;
    let at = |i: usize| {
        let p = points[i];
        [(p[0] - centre[0]) * k, (p[1] - centre[1]) * k, (p[2] - centre[2]) * k]
    };
    // Flat shaded: each triangle its own corners and its face's normal,
    // laid out part by part so each part is one group.
    let mut mesh = MeshData {
        positions: Vec::with_capacity(tris.len() * 3),
        normals: Vec::with_capacity(tris.len() * 3),
        uvs: Vec::with_capacity(tris.len() * 3),
        colors: Vec::with_capacity(tris.len() * 3),
        indices: Vec::with_capacity(tris.len() * 3),
        groups: Vec::new(),
    };
    let mut materials = Vec::new();
    for (index, name) in parts.iter().enumerate() {
        let first = mesh.indices.len() as u32;
        for (corners, _) in tris.iter().filter(|(_, p)| *p == index) {
            let [a, b, c] = corners.map(at);
            let n = normal(a, b, c);
            for (corner, p) in corners.iter().zip([a, b, c]) {
                mesh.indices.push(mesh.positions.len() as u32);
                mesh.positions.push(p);
                mesh.normals.push(n);
                mesh.uvs.push([0.0, 0.0]);
                let [r, g, b] = tints[*corner].unwrap_or([1.0, 1.0, 1.0]);
                mesh.colors.push([r, g, b, 1.0]);
            }
        }
        let count = mesh.indices.len() as u32 - first;
        if count > 0 {
            mesh.groups.push((first, count));
            let (tone, glow) = realm_tone(name).unwrap_or((STEEL, 0.0));
            materials.push(lit(tone, glow));
        }
    }
    let mut node = Node::new(2, MeshRef::Uploaded(MESH), lit(STEEL, 0.0));
    node.parent = Some(angel_glass::MODEL_ROOT);
    node.face_materials = materials;
    Ok(Model { mesh, nodes: vec![node], triangles: tris.len(), textures: Vec::new() })
}

/// A binary glTF (`.glb`), as Hunyuan and most tools write it: every mesh
/// of its default scene placed by its nodes, smooth normals as given (made
/// flat where it has none), vertex colours, and each part's base colour,
/// from its factor and its texture (embedded PNG or JPEG).
pub(crate) fn from_glb(bytes: &[u8]) -> Result<Model, String> {
    let u32_at = |at: usize| -> Result<u32, String> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "the file ends early".to_string())
    };
    if bytes.get(..4) != Some(b"glTF") {
        return Err("not a binary glTF".into());
    }
    let json_len = u32_at(12)? as usize;
    if u32_at(16)? != 0x4E4F_534A {
        return Err("the first chunk is not JSON".into());
    }
    let json: serde_json::Value = serde_json::from_slice(bytes.get(20..20 + json_len).ok_or("the JSON chunk is cut short")?)
        .map_err(|e| format!("its JSON: {e}"))?;
    let bin_at = 20 + json_len;
    let bin: &[u8] = if bin_at + 8 <= bytes.len() && u32_at(bin_at + 4)? == 0x004E_4942 {
        let len = u32_at(bin_at)? as usize;
        bytes.get(bin_at + 8..(bin_at + 8).saturating_add(len)).ok_or("the binary chunk is cut short")?
    } else {
        &[]
    };
    let gltf = Gltf { json: &json, bin };
    // The scene's nodes, each with its place in the world.
    let scene = json["scene"].as_u64().unwrap_or(0) as usize;
    let roots: Vec<usize> = json["scenes"][scene]["nodes"]
        .as_array()
        .map(|a| a.iter().filter_map(|n| n.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_else(|| (0..json["nodes"].as_array().map_or(0, |a| a.len())).collect());
    let mut placed: Vec<(usize, [[f32; 4]; 4])> = Vec::new();
    let mut stack: Vec<(usize, [[f32; 4]; 4], u32)> = roots.into_iter().map(|n| (n, IDENTITY, 0)).collect();
    while let Some((n, parent, depth)) = stack.pop() {
        let node = &json["nodes"][n];
        if node.is_null() || depth > 32 {
            continue;
        }
        let world = mul(parent, local(node));
        if let Some(mesh) = node["mesh"].as_u64() {
            placed.push((mesh as usize, world));
        }
        for child in node["children"].as_array().into_iter().flatten() {
            if let Some(c) = child.as_u64() {
                stack.push((c as usize, world, depth + 1));
            }
        }
    }
    // Every primitive's triangles, part by part.
    let mut mesh = MeshData {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        colors: Vec::new(),
        indices: Vec::new(),
        groups: Vec::new(),
    };
    let mut materials = Vec::new();
    let mut textures: Vec<(usize, u32)> = Vec::new();
    for (m, world) in placed {
        for prim in json["meshes"][m]["primitives"].as_array().into_iter().flatten() {
            if prim["mode"].as_u64().unwrap_or(4) != 4 {
                continue; // only triangle lists
            }
            let attrs = &prim["attributes"];
            let positions = gltf.floats(attrs["POSITION"].as_u64().ok_or("a part with no positions")? as usize, 3)?;
            let count = positions.len() / 3;
            let normals = attrs["NORMAL"].as_u64().map(|a| gltf.floats(a as usize, 3)).transpose()?;
            let uvs = attrs["TEXCOORD_0"].as_u64().map(|a| gltf.floats(a as usize, 2)).transpose()?;
            let colors = attrs["COLOR_0"].as_u64().map(|a| gltf.colors(a as usize)).transpose()?;
            let indices: Vec<u32> = match prim["indices"].as_u64() {
                Some(a) => gltf.indices(a as usize)?,
                None => (0..count as u32).collect(),
            };
            if indices.iter().any(|&i| i as usize >= count) {
                return Err("a triangle names a corner that is not there".into());
            }
            if mesh.indices.len() + indices.len() > MOST_TRIANGLES * 3 {
                return Err(format!("more than {MOST_TRIANGLES} triangles"));
            }
            let base = mesh.positions.len() as u32;
            let first = mesh.indices.len() as u32;
            for i in 0..count {
                let p = [positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]];
                mesh.positions.push(apply(world, p, 1.0));
                let n = normals.as_ref().map(|n| [n[i * 3], n[i * 3 + 1], n[i * 3 + 2]]).unwrap_or([0.0, 0.0, 0.0]);
                mesh.normals.push(unit(apply(world, n, 0.0)));
                mesh.uvs.push(uvs.as_ref().map(|uv| [uv[i * 2], uv[i * 2 + 1]]).unwrap_or([0.0, 0.0]));
                mesh.colors.push(colors.as_ref().map(|c| c[i]).unwrap_or([1.0; 4]));
            }
            mesh.indices.extend(indices.iter().map(|i| base + i));
            if normals.is_none() {
                // No normals given: each triangle's own.
                for tri in mesh.indices[first as usize..].chunks(3).map(|t| [t[0], t[1], t[2]]).collect::<Vec<_>>() {
                    let [a, b, c] = tri.map(|i| mesh.positions[i as usize]);
                    let n = normal(a, b, c);
                    for i in tri {
                        mesh.normals[i as usize] = n;
                    }
                }
            }
            mesh.groups.push((first, mesh.indices.len() as u32 - first));
            // Its material: the base colour, and its texture if it has one.
            let material = &json["materials"][prim["material"].as_u64().unwrap_or(u64::MAX) as usize];
            let pbr = &material["pbrMetallicRoughness"];
            let factor: Vec<f32> = pbr["baseColorFactor"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_f64().map(|v| v as f32)).collect())
                .unwrap_or_default();
            let tint = if factor.len() == 4 { [factor[0], factor[1], factor[2], factor[3]] } else { [1.0; 4] };
            let image = pbr["baseColorTexture"]["index"]
                .as_u64()
                .and_then(|t| json["textures"][t as usize]["source"].as_u64())
                .map(|i| i as usize);
            let picture = image.and_then(|image| {
                if let Some(&(_, id)) = textures.iter().find(|(i, _)| *i == image) {
                    return Some(id);
                }
                (textures.len() < MOST_TEXTURES).then(|| {
                    let id = TEXTURES + textures.len() as u32;
                    textures.push((image, id));
                    id
                })
            });
            let mut material = match picture {
                Some(id) => Material::picture(id),
                None => Material::color(tint),
            };
            material.tint = tint;
            material.lit = true;
            // A painted texture carries its own shading; the key light only
            // models it, so a dark texture still reads.
            if picture.is_some() {
                material.emissive = 0.3;
            }
            material.double_sided = true;
            materials.push(material);
        }
    }
    if mesh.indices.is_empty() {
        return Err("no triangles in its scene".into());
    }
    fit(&mut mesh.positions);
    // The textures, decoded and kept within the window's means.
    let mut pictures = Vec::new();
    for (image, id) in textures {
        let view = json["images"][image]["bufferView"].as_u64().ok_or("a texture that is not embedded")? as usize;
        let data = gltf.view(view)?;
        let decoded = image::load_from_memory(data).map_err(|e| format!("a texture: {e}"))?;
        let decoded = if decoded.width().max(decoded.height()) > TEXTURE_SIDE {
            decoded.resize(TEXTURE_SIDE, TEXTURE_SIDE, image::imageops::FilterType::Triangle)
        } else {
            decoded
        };
        let rgba = decoded.into_rgba8();
        pictures.push((id, rgba.width(), rgba.height(), rgba.into_raw()));
    }
    let triangles = mesh.indices.len() / 3;
    let mut node = Node::new(2, MeshRef::Uploaded(MESH), lit(STEEL, 0.0));
    node.parent = Some(angel_glass::MODEL_ROOT);
    node.face_materials = materials;
    Ok(Model { mesh, nodes: vec![node], triangles, textures: pictures })
}

/// A glTF's JSON and its binary chunk, read by accessor.
struct Gltf<'a> {
    json: &'a serde_json::Value,
    bin: &'a [u8],
}

impl Gltf<'_> {
    fn view(&self, view: usize) -> Result<&[u8], String> {
        let v = &self.json["bufferViews"][view];
        if v["buffer"].as_u64().unwrap_or(0) != 0 {
            return Err("data outside the file".into());
        }
        let at = v["byteOffset"].as_u64().unwrap_or(0) as usize;
        let len = v["byteLength"].as_u64().ok_or("a view with no length")? as usize;
        let end = at.checked_add(len).ok_or("a view past the end of the data")?;
        self.bin.get(at..end).ok_or_else(|| "a view past the end of the data".into())
    }

    /// An accessor's elements as `(bytes, component type, count, width, stride)`.
    fn accessor(&self, a: usize, width: usize) -> Result<(&[u8], u64, usize, usize), String> {
        let acc = &self.json["accessors"][a];
        let view = acc["bufferView"].as_u64().ok_or("an accessor with no data")? as usize;
        let data = self.view(view)?;
        let at = acc["byteOffset"].as_u64().unwrap_or(0) as usize;
        let kind = acc["componentType"].as_u64().unwrap_or(0);
        let count = acc["count"].as_u64().unwrap_or(0) as usize;
        let size = match kind {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => return Err(format!("component type {kind}")),
        };
        let stride = self.json["bufferViews"][view]["byteStride"].as_u64().map_or(size * width, |s| s as usize);
        // Checked: a hostile count or stride must not wrap past the bound
        // (and then size an allocation from it).
        let need = if count == 0 {
            Some(0)
        } else {
            stride
                .checked_mul(count - 1)
                .and_then(|n| n.checked_add(at))
                .and_then(|n| n.checked_add(size * width))
        };
        if need.is_none_or(|need| need > data.len()) || stride < size * width && count > 1 {
            return Err("an accessor past the end of its view".into());
        }
        Ok((&data[at..], kind, count, stride))
    }

    fn floats(&self, a: usize, width: usize) -> Result<Vec<f32>, String> {
        let (data, kind, count, stride) = self.accessor(a, width)?;
        if kind != 5126 {
            return Err("positions, normals and uvs must be floats".into());
        }
        let mut out = Vec::with_capacity(count * width);
        for i in 0..count {
            for k in 0..width {
                let o = i * stride + k * 4;
                let v = f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
                out.push(if v.is_finite() { v } else { 0.0 });
            }
        }
        Ok(out)
    }

    fn colors(&self, a: usize) -> Result<Vec<[f32; 4]>, String> {
        let width = if self.json["accessors"][a]["type"] == "VEC3" { 3 } else { 4 };
        let (data, kind, count, stride) = self.accessor(a, width)?;
        let one = |i: usize, k: usize| -> f32 {
            let o = i * stride;
            match kind {
                5126 => f32::from_le_bytes([data[o + k * 4], data[o + k * 4 + 1], data[o + k * 4 + 2], data[o + k * 4 + 3]]),
                5121 => f32::from(data[o + k]) / 255.0,
                5123 => f32::from(u16::from_le_bytes([data[o + k * 2], data[o + k * 2 + 1]])) / 65535.0,
                _ => 1.0,
            }
        };
        Ok((0..count)
            .map(|i| [one(i, 0), one(i, 1), one(i, 2), if width == 4 { one(i, 3) } else { 1.0 }].map(|v| v.clamp(0.0, 1.0)))
            .collect())
    }

    fn indices(&self, a: usize) -> Result<Vec<u32>, String> {
        let (data, kind, count, stride) = self.accessor(a, 1)?;
        let stride = stride.max(1);
        Ok((0..count)
            .map(|i| {
                let o = i * stride;
                match kind {
                    5121 => u32::from(data[o]),
                    5123 => u32::from(u16::from_le_bytes([data[o], data[o + 1]])),
                    _ => u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]),
                }
            })
            .collect())
    }
}

const IDENTITY: [[f32; 4]; 4] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// A node's own transform, column-major: its matrix, or its translation,
/// rotation and scale.
fn local(node: &serde_json::Value) -> [[f32; 4]; 4] {
    let nums = |key: &str| -> Vec<f32> {
        node[key].as_array().map(|a| a.iter().filter_map(|v| v.as_f64().map(|v| v as f32)).collect()).unwrap_or_default()
    };
    let m = nums("matrix");
    if m.len() == 16 {
        return [[m[0], m[1], m[2], m[3]], [m[4], m[5], m[6], m[7]], [m[8], m[9], m[10], m[11]], [m[12], m[13], m[14], m[15]]];
    }
    let t = nums("translation");
    let r = nums("rotation");
    let s = nums("scale");
    let t = if t.len() == 3 { [t[0], t[1], t[2]] } else { [0.0; 3] };
    let [x, y, z, w] = if r.len() == 4 { [r[0], r[1], r[2], r[3]] } else { [0.0, 0.0, 0.0, 1.0] };
    let s = if s.len() == 3 { [s[0], s[1], s[2]] } else { [1.0; 3] };
    [
        [(1.0 - 2.0 * (y * y + z * z)) * s[0], 2.0 * (x * y + z * w) * s[0], 2.0 * (x * z - y * w) * s[0], 0.0],
        [2.0 * (x * y - z * w) * s[1], (1.0 - 2.0 * (x * x + z * z)) * s[1], 2.0 * (y * z + x * w) * s[1], 0.0],
        [2.0 * (x * z + y * w) * s[2], 2.0 * (y * z - x * w) * s[2], (1.0 - 2.0 * (x * x + y * y)) * s[2], 0.0],
        [t[0], t[1], t[2], 1.0],
    ]
}

fn mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for (c, col) in out.iter_mut().enumerate() {
        for (r, cell) in col.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

/// `p` through `m`; `w` 1 for a point, 0 for a direction.
fn apply(m: [[f32; 4]; 4], p: [f32; 3], w: f32) -> [f32; 3] {
    [0, 1, 2].map(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r] * w)
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1e-12 { [v[0] / len, v[1] / len, v[2] / len] } else { [0.0, 1.0, 0.0] }
}

/// Centre points on their middle and fit them in a sphere a little
/// inside the unit, the turntable's room.
fn fit(points: &mut [[f32; 3]]) {
    let (lo, hi) = points.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
        ([lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])], [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])])
    });
    let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let reach = points
        .iter()
        .map(|p| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2) + (p[2] - centre[2]).powi(2)).sqrt())
        .fold(0.0f32, f32::max)
        .max(1e-6);
    let k = 0.95 / reach;
    for p in points.iter_mut() {
        *p = [(p[0] - centre[0]) * k, (p[1] - centre[1]) * k, (p[2] - centre[2]) * k];
    }
}

const STEEL: [f32; 3] = [0.64, 0.66, 0.70];

fn lit(tone: [f32; 3], glow: f32) -> Material {
    let mut material = Material::lit([tone[0], tone[1], tone[2], 1.0], glow);
    material.double_sided = true;
    material
}

/// The realm's material words and the tones its props wear.
fn realm_tone(word: &str) -> Option<([f32; 3], f32)> {
    Some(match word {
        "stone" | "rock" => ([0.56, 0.57, 0.60], 0.0),
        "stone_dark" => ([0.36, 0.37, 0.41], 0.0),
        "roof" => ([0.30, 0.33, 0.42], 0.0),
        "wood" | "trunk" | "door" => ([0.46, 0.33, 0.23], 0.0),
        "leather" => ([0.40, 0.27, 0.19], 0.0),
        "ember" => ([0.80, 0.42, 0.20], 0.6),
        "fire" => ([0.95, 0.62, 0.30], 0.9),
        "grass" | "foliage" => ([0.30, 0.40, 0.30], 0.0),
        "banner" => ([0.55, 0.20, 0.16], 0.0),
        "brass" | "cloth_gold" => ([0.77, 0.58, 0.32], 0.0),
        "cloth_blue" => ([0.26, 0.31, 0.54], 0.0),
        "parchment" | "path" => ([0.76, 0.70, 0.56], 0.0),
        "crystal" => ([0.58, 0.70, 0.78], 0.3),
        "armor" => ([0.66, 0.70, 0.76], 0.0),
        "water" => ([0.24, 0.36, 0.48], 0.0),
        _ => return None,
    })
}

fn normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 1e-12 { [n[0] / len, n[1] / len, n[2] / len] } else { [0.0, 1.0, 0.0] }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE: &str = "v 0 0 0\nv 2 0 0\nv 2 2 0\nv 0 2 0\nv 0 0 2\nv 2 0 2\nv 2 2 2\nv 0 2 2\n\
        usemtl a\nf 1 2 3 4\nf 5 8 7 6\nusemtl b\nf 1 5 6 2\nf 2/1/1 6/1/1 7/1/1 3/1/1\nf -8 -4 -1 -5\nf 4//1 3//1 7//1 8//1\n";

    #[test]
    fn a_cube_reads_fanned_centred_and_fitted() {
        let model = from_obj(CUBE).unwrap();
        assert_eq!(model.triangles, 12);
        assert_eq!(model.mesh.indices.len(), 36);
        assert_eq!(model.mesh.groups.len(), 2, "one group per material");
        let reach = model
            .mesh
            .positions
            .iter()
            .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
            .fold(0.0f32, f32::max);
        assert!((reach - 0.95).abs() < 1e-4, "fitted to the turntable's sphere");
        assert!(model.mesh.validate().is_ok());
        assert_eq!(model.nodes[0].parent, Some(angel_glass::MODEL_ROOT));
    }

    #[test]
    fn broken_files_say_why() {
        let why = |text: &str| from_obj(text).err().unwrap_or_default();
        assert!(why("v 0 0 0\nf 1 2 3\n").contains("not there"));
        assert!(!why("v 0 0\n").is_empty());
        assert!(why("# nothing\n").contains("no faces"));
    }

    /// A one-triangle GLB built by hand: a JSON chunk and a binary chunk.
    fn tiny_glb(rotated: bool) -> Vec<u8> {
        let mut bin = Vec::new();
        for v in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0, 0.0] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        for i in [0u16, 1, 2, 0] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        let rotation = if rotated { r#","rotation":[0.7071068,0,0,0.7071068]"# } else { "" };
        let mut json = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0{rotation}}}],
            "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"material":0}}]}}],
            "materials":[{{"pbrMetallicRoughness":{{"baseColorFactor":[0.5,0.25,0.1,1.0]}}}}],
            "buffers":[{{"byteLength":{}}}],
            "bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":6}}],
            "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}},{{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}]}}"#,
            bin.len()
        )
        .into_bytes();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&((12 + 8 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        out.extend_from_slice(&json);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
        out.extend_from_slice(&bin);
        out
    }

    #[test]
    fn a_glb_reads_its_triangle_its_colour_and_its_node() {
        let model = from_glb(&tiny_glb(false)).unwrap();
        assert_eq!(model.triangles, 1);
        assert_eq!(model.nodes[0].face_materials[0].tint, [0.5, 0.25, 0.1, 1.0]);
        assert!(model.mesh.validate().is_ok());
        // Rotated a quarter turn about x, the tall edge (y) lies along z.
        let model = from_glb(&tiny_glb(true)).unwrap();
        let span = |k: usize| {
            let v: Vec<f32> = model.mesh.positions.iter().map(|p| p[k]).collect();
            v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
        };
        assert!(span(2) > span(1), "the node's rotation is applied");
        assert!(from_glb(b"not a model").is_err());
        // A count that would wrap the bound is refused, not trusted.
        let mut hostile = String::from_utf8_lossy(&tiny_glb(false)).into_owned().into_bytes();
        let at = hostile.windows(9).position(|w| w == b"\"count\":3").unwrap();
        let huge = b"\"count\":9";
        hostile[at..at + 9].copy_from_slice(huge);
        assert!(from_glb(&hostile).is_err());
    }

    #[test]
    fn the_realms_props_stand_up() {
        // A thin post along z, in the realm's z-up words, ends up along y.
        let post = "usemtl stone\nv 0 0 0\nv 0.1 0 0\nv 0 0 5\nf 1 2 3\n";
        let model = from_obj(post).unwrap();
        let tall = model.mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max)
            - model.mesh.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        assert!(tall > 1.5);
    }
}
