//! A binary glTF reader for what a rigged, blendshaped head needs: the node
//! tree, a skin, triangle meshes with joints and weights, and morph targets
//! (dense or sparse, as Blender writes shape keys), with their names. It
//! reads nothing else (no animations, cameras or textures), and refuses
//! what it cannot read rather than guessing.

use angel_stage::scene::Mat4;
use serde_json::Value;

/// One node: its place relative to its parent, what hangs from it.
#[derive(Clone, Debug)]
pub struct GNode {
    pub name: String,
    pub translation: [f32; 3],
    /// `[x, y, z, w]`.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub children: Vec<usize>,
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
}

/// A skin: its joints (node indices) and their inverse bind matrices.
#[derive(Clone, Debug)]
pub struct GSkin {
    pub joints: Vec<usize>,
    pub inverse_bind: Vec<Mat4>,
}

/// A morph target: how far each vertex it moves goes, and how its normal
/// turns. Only the vertices it moves are listed.
#[derive(Clone, Debug, Default)]
pub struct GTarget {
    pub name: String,
    /// `(vertex, position delta, normal delta)`.
    pub deltas: Vec<(u32, [f32; 3], [f32; 3])>,
}

/// A triangle list and what is laid on its vertices.
#[derive(Clone, Debug, Default)]
pub struct GPrimitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub targets: Vec<GTarget>,
    pub material: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct GMesh {
    pub name: String,
    pub primitives: Vec<GPrimitive>,
}

/// A whole file: its nodes, skins, meshes, material names and the scene's
/// root nodes, and its embedded images (as encoded, PNG or JPEG) with the
/// image each material's base colour is.
#[derive(Clone, Debug)]
pub struct Gltf {
    pub nodes: Vec<GNode>,
    pub skins: Vec<GSkin>,
    pub meshes: Vec<GMesh>,
    pub materials: Vec<String>,
    pub roots: Vec<usize>,
    pub images: Vec<Vec<u8>>,
    pub material_images: Vec<Option<usize>>,
}

impl Gltf {
    /// The node named `name`.
    pub fn node(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    /// Each node's parent.
    pub fn parents(&self) -> Vec<Option<usize>> {
        let mut out = vec![None; self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate() {
            for &c in &node.children {
                if let Some(slot) = out.get_mut(c) {
                    *slot = Some(i);
                }
            }
        }
        out
    }
}

/// Read a `.glb`.
pub fn read(bytes: &[u8]) -> Result<Gltf, String> {
    let (json, bin) = chunks(bytes)?;
    let file = File { json: &json, bin };
    let nodes = arr(&json, "nodes")
        .iter()
        .map(|n| {
            let f3 = |key: &str, default: [f32; 3]| floats::<3>(n.get(key)).unwrap_or(default);
            GNode {
                name: n["name"].as_str().unwrap_or("").to_string(),
                translation: f3("translation", [0.0; 3]),
                rotation: floats::<4>(n.get("rotation")).unwrap_or([0.0, 0.0, 0.0, 1.0]),
                scale: f3("scale", [1.0; 3]),
                children: n["children"].as_array().map_or_else(Vec::new, |c| {
                    c.iter()
                        .filter_map(|v| v.as_u64())
                        .map(|v| v as usize)
                        .collect()
                }),
                mesh: n["mesh"].as_u64().map(|v| v as usize),
                skin: n["skin"].as_u64().map(|v| v as usize),
            }
        })
        .collect::<Vec<_>>();
    if nodes
        .iter()
        .any(|n| n.children.iter().any(|&c| c >= nodes.len()))
    {
        return Err("a node's child is not in the file".into());
    }
    let skins = arr(&json, "skins")
        .iter()
        .map(|s| {
            let joints: Vec<usize> = s["joints"]
                .as_array()
                .ok_or("a skin with no joints")?
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as usize)
                .collect();
            if joints.iter().any(|&j| j >= nodes.len()) {
                return Err("a skin's joint is not a node".to_string());
            }
            let inverse_bind = match s["inverseBindMatrices"].as_u64() {
                Some(a) => {
                    let (flat, n) = file.accessor(a as usize)?;
                    if n != 16 || flat.len() != joints.len() * 16 {
                        return Err("a skin's inverse bind matrices are not one per joint".into());
                    }
                    flat.chunks_exact(16)
                        .map(|m| std::array::from_fn(|c| std::array::from_fn(|r| m[c * 4 + r])))
                        .collect()
                }
                None => vec![angel_stage::scene::IDENTITY_MATRIX; joints.len()],
            };
            Ok(GSkin {
                joints,
                inverse_bind,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let meshes = arr(&json, "meshes")
        .iter()
        .map(|m| {
            let names: Vec<String> =
                m["extras"]["targetNames"]
                    .as_array()
                    .map_or_else(Vec::new, |a| {
                        a.iter()
                            .map(|v| v.as_str().unwrap_or("").to_string())
                            .collect()
                    });
            let primitives = m["primitives"]
                .as_array()
                .ok_or("a mesh with no primitives")?
                .iter()
                .map(|p| file.primitive(p, &names))
                .collect::<Result<Vec<_>, String>>()?;
            Ok(GMesh {
                name: m["name"].as_str().unwrap_or("").to_string(),
                primitives,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let materials = arr(&json, "materials")
        .iter()
        .map(|m| m["name"].as_str().unwrap_or("").to_string())
        .collect();
    let images = arr(&json, "images")
        .iter()
        .map(|i| match i["bufferView"].as_u64() {
            Some(v) => file.view(v).map(|(bytes, _)| bytes.to_vec()),
            None => Ok(Vec::new()),
        })
        .collect::<Result<Vec<_>, String>>()?;
    let material_images = arr(&json, "materials")
        .iter()
        .map(|m| {
            let texture = m["pbrMetallicRoughness"]["baseColorTexture"]["index"].as_u64()?;
            let image = json["textures"][texture as usize]["source"].as_u64()? as usize;
            (image < images.len()).then_some(image)
        })
        .collect();
    let scene = json["scene"].as_u64().unwrap_or(0) as usize;
    let roots = json["scenes"][scene]["nodes"]
        .as_array()
        .map_or_else(Vec::new, |r| {
            r.iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as usize)
                .filter(|&v| v < nodes.len())
                .collect()
        });
    Ok(Gltf {
        nodes,
        skins,
        meshes,
        materials,
        roots,
        images,
        material_images,
    })
}

/// The JSON and binary chunks.
fn chunks(bytes: &[u8]) -> Result<(Value, &[u8]), String> {
    let u32_at = |at: usize| -> Result<u32, String> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "the file is cut short".to_string())
    };
    if bytes.get(..4) != Some(b"glTF") {
        return Err("not a binary glTF".into());
    }
    if u32_at(4)? != 2 {
        return Err("not glTF 2".into());
    }
    let json_len = u32_at(12)? as usize;
    if u32_at(16)? != 0x4E4F_534A {
        return Err("the first chunk is not JSON".into());
    }
    let json_bytes = bytes
        .get(20..20 + json_len)
        .ok_or("the JSON chunk is cut short")?;
    let json: Value =
        serde_json::from_slice(json_bytes).map_err(|e| format!("the JSON chunk: {e}"))?;
    let at = 20 + json_len;
    let bin = if at + 8 <= bytes.len() && u32_at(at + 4)? == 0x004E_4942 {
        let len = u32_at(at)? as usize;
        bytes
            .get(at + 8..at + 8 + len)
            .ok_or("the binary chunk is cut short")?
    } else {
        &[]
    };
    Ok((json, bin))
}

fn arr<'a>(json: &'a Value, key: &str) -> &'a [Value] {
    json[key].as_array().map_or(&[], Vec::as_slice)
}

fn floats<const N: usize>(v: Option<&Value>) -> Option<[f32; N]> {
    let a = v?.as_array()?;
    (a.len() == N).then(|| std::array::from_fn(|i| a[i].as_f64().unwrap_or(0.0) as f32))
}

struct File<'a> {
    json: &'a Value,
    bin: &'a [u8],
}

/// The size of one component of a glTF component type.
fn component_size(kind: u64) -> Option<usize> {
    match kind {
        5120 | 5121 => Some(1),
        5122 | 5123 => Some(2),
        5125 | 5126 => Some(4),
        _ => None,
    }
}

/// One component at `at`, as a number (normalized integers to 0..1 or
/// -1..1 when `normalized`).
fn component(bytes: &[u8], at: usize, kind: u64, normalized: bool) -> Option<f32> {
    let b = bytes.get(at..at + component_size(kind)?)?;
    Some(match kind {
        5126 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
        5123 => {
            let v = u16::from_le_bytes([b[0], b[1]]) as f32;
            if normalized { v / 65535.0 } else { v }
        }
        5122 => {
            let v = i16::from_le_bytes([b[0], b[1]]) as f32;
            if normalized {
                (v / 32767.0).max(-1.0)
            } else {
                v
            }
        }
        5121 => {
            let v = b[0] as f32;
            if normalized { v / 255.0 } else { v }
        }
        5120 => {
            let v = b[0] as i8 as f32;
            if normalized { (v / 127.0).max(-1.0) } else { v }
        }
        _ => return None,
    })
}

impl File<'_> {
    /// A buffer view's bytes and its stride (0: packed).
    fn view(&self, index: u64) -> Result<(&[u8], usize), String> {
        let v = &self.json["bufferViews"][index as usize];
        if v["buffer"].as_u64().unwrap_or(0) != 0 {
            return Err("a buffer other than the file's own".into());
        }
        let start = v["byteOffset"].as_u64().unwrap_or(0) as usize;
        let len = v["byteLength"]
            .as_u64()
            .ok_or("a buffer view with no length")? as usize;
        let bytes = self
            .bin
            .get(start..start + len)
            .ok_or("a buffer view past the binary chunk")?;
        Ok((bytes, v["byteStride"].as_u64().unwrap_or(0) as usize))
    }

    /// An accessor as flat numbers and its components per element, sparse
    /// values laid over the dense ones (or over zeros).
    fn accessor(&self, index: usize) -> Result<(Vec<f32>, usize), String> {
        let a = &self.json["accessors"][index];
        let n = match a["type"].as_str() {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            Some("VEC4") => 4,
            Some("MAT4") => 16,
            _ => {
                return Err(format!(
                    "accessor {index}: a type this reader does not take"
                ));
            }
        };
        let kind = a["componentType"]
            .as_u64()
            .ok_or("an accessor with no component type")?;
        let size = component_size(kind).ok_or("an unknown component type")?;
        let normalized = a["normalized"].as_bool().unwrap_or(false);
        let count = a["count"].as_u64().ok_or("an accessor with no count")? as usize;
        if count > 1 << 24 {
            return Err("an accessor too long to be a head".into());
        }
        let mut out = vec![0.0f32; count * n];
        if let Some(view) = a["bufferView"].as_u64() {
            let (bytes, stride) = self.view(view)?;
            let offset = a["byteOffset"].as_u64().unwrap_or(0) as usize;
            let stride = if stride == 0 { size * n } else { stride };
            for i in 0..count {
                for c in 0..n {
                    out[i * n + c] =
                        component(bytes, offset + i * stride + c * size, kind, normalized)
                            .ok_or("an accessor past its view")?;
                }
            }
        }
        if let Some(sparse) = a.get("sparse") {
            let k = sparse["count"]
                .as_u64()
                .ok_or("a sparse accessor with no count")? as usize;
            let ikind = sparse["indices"]["componentType"]
                .as_u64()
                .ok_or("sparse indices with no type")?;
            let isize = component_size(ikind).ok_or("sparse indices of an unknown type")?;
            let (ibytes, _) = self.view(
                sparse["indices"]["bufferView"]
                    .as_u64()
                    .ok_or("sparse indices with no view")?,
            )?;
            let ioff = sparse["indices"]["byteOffset"].as_u64().unwrap_or(0) as usize;
            let (vbytes, _) = self.view(
                sparse["values"]["bufferView"]
                    .as_u64()
                    .ok_or("sparse values with no view")?,
            )?;
            let voff = sparse["values"]["byteOffset"].as_u64().unwrap_or(0) as usize;
            for j in 0..k {
                let i = component(ibytes, ioff + j * isize, ikind, false)
                    .ok_or("sparse indices past their view")? as usize;
                if i >= count {
                    return Err("a sparse index past its accessor".into());
                }
                for c in 0..n {
                    out[i * n + c] = component(vbytes, voff + (j * n + c) * size, kind, normalized)
                        .ok_or("sparse values past their view")?;
                }
            }
        }
        if out.iter().any(|v| !v.is_finite()) {
            return Err(format!(
                "accessor {index} holds a number that is not finite"
            ));
        }
        Ok((out, n))
    }

    fn vecs<const N: usize>(
        &self,
        index: Option<u64>,
        count: usize,
    ) -> Result<Vec<[f32; N]>, String> {
        let Some(index) = index else {
            return Ok(Vec::new());
        };
        let (flat, n) = self.accessor(index as usize)?;
        if n != N || flat.len() != count * N {
            return Err("an attribute that is not one per vertex".into());
        }
        Ok(flat
            .chunks_exact(N)
            .map(|c| std::array::from_fn(|i| c[i]))
            .collect())
    }

    fn primitive(&self, p: &Value, names: &[String]) -> Result<GPrimitive, String> {
        if p["mode"].as_u64().unwrap_or(4) != 4 {
            return Err("a primitive that is not triangles".into());
        }
        let attrs = &p["attributes"];
        let pos = attrs["POSITION"]
            .as_u64()
            .ok_or("a primitive with no positions")?;
        let positions = {
            let (flat, n) = self.accessor(pos as usize)?;
            if n != 3 {
                return Err("positions that are not 3D".into());
            }
            flat.chunks_exact(3)
                .map(|c| [c[0], c[1], c[2]])
                .collect::<Vec<_>>()
        };
        let count = positions.len();
        let normals = self.vecs::<3>(attrs["NORMAL"].as_u64(), count)?;
        let uvs = self.vecs::<2>(attrs["TEXCOORD_0"].as_u64(), count)?;
        let joints = self
            .vecs::<4>(attrs["JOINTS_0"].as_u64(), count)?
            .into_iter()
            .map(|j| j.map(|v| v as u16))
            .collect();
        let weights = self.vecs::<4>(attrs["WEIGHTS_0"].as_u64(), count)?;
        let indices = match p["indices"].as_u64() {
            Some(i) => {
                let (flat, n) = self.accessor(i as usize)?;
                if n != 1 {
                    return Err("indices that are not scalars".into());
                }
                flat.into_iter().map(|v| v as u32).collect::<Vec<_>>()
            }
            None => (0..count as u32).collect(),
        };
        if indices.len() % 3 != 0 || indices.iter().any(|&i| i as usize >= count) {
            return Err("indices that are not whole triangles of this primitive".into());
        }
        let mut targets = Vec::new();
        for (t, target) in p["targets"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .enumerate()
        {
            let dp = self.vecs::<3>(target["POSITION"].as_u64(), count)?;
            let dn = self.vecs::<3>(target["NORMAL"].as_u64(), count)?;
            let mut deltas = Vec::new();
            for v in 0..count {
                let d = dp.get(v).copied().unwrap_or([0.0; 3]);
                let n = dn.get(v).copied().unwrap_or([0.0; 3]);
                if d.iter().chain(&n).any(|x| x.abs() > 1e-9) {
                    deltas.push((v as u32, d, n));
                }
            }
            targets.push(GTarget {
                name: names
                    .get(t)
                    .cloned()
                    .unwrap_or_else(|| format!("target{t}")),
                deltas,
            });
        }
        Ok(GPrimitive {
            positions,
            normals,
            uvs,
            joints,
            weights,
            indices,
            targets,
            material: p["material"].as_u64().map(|v| v as usize),
        })
    }
}
