//! A small glTF 2.0 writer: enough to make a `.glb`, or a `.gltf` with its
//! buffer inside it as a `data:` URI, in code and with no dependencies. The
//! sampler builds its model with it, and the intake's tests build their
//! files with it, sound and broken (`src/intake/gltf_tests.rs`).

#![allow(dead_code)]

/// A file being written: the binary buffer and each JSON array, as text.
#[derive(Clone, Debug, Default)]
pub struct Gltf {
    pub bin: Vec<u8>,
    pub views: Vec<String>,
    pub accessors: Vec<String>,
    pub meshes: Vec<String>,
    pub materials: Vec<String>,
    pub textures: Vec<String>,
    pub images: Vec<String>,
    pub samplers: Vec<String>,
    pub nodes: Vec<String>,
    pub scenes: Vec<String>,
    pub animations: Vec<String>,
    /// The scene the file names as its own.
    pub scene: Option<usize>,
    /// Fields added to the top-level object as they are (`"key":value`).
    pub extra: Vec<String>,
}

fn push(list: &mut Vec<String>, json: String) -> usize {
    list.push(json);
    list.len() - 1
}

fn numbers(values: &[f32]) -> String {
    values.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(",")
}

impl Gltf {
    /// Bytes into the buffer, four-aligned, as a buffer view.
    pub fn view(&mut self, bytes: &[u8]) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let offset = self.bin.len();
        self.bin.extend_from_slice(bytes);
        push(&mut self.views, format!(r#"{{"buffer":0,"byteOffset":{offset},"byteLength":{}}}"#, bytes.len()))
    }

    /// Floats as an accessor of `ty` (`SCALAR`, `VEC2`, `VEC3`, `VEC4`), with
    /// each component's min and max.
    pub fn floats(&mut self, values: &[f32], ty: &str) -> usize {
        let width = match ty {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            _ => 4,
        };
        let count = values.len() / width;
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let view = self.view(&bytes);
        let mut min = vec![f32::INFINITY; width];
        let mut max = vec![f32::NEG_INFINITY; width];
        for item in values.chunks(width) {
            for (k, v) in item.iter().enumerate() {
                min[k] = min[k].min(*v);
                max[k] = max[k].max(*v);
            }
        }
        if count == 0 {
            min = vec![0.0; width];
            max = vec![0.0; width];
        }
        push(
            &mut self.accessors,
            format!(
                r#"{{"bufferView":{view},"componentType":5126,"count":{count},"type":"{ty}","min":[{}],"max":[{}]}}"#,
                numbers(&min),
                numbers(&max)
            ),
        )
    }

    /// Triangle indices, 16-bit when they fit.
    pub fn indices(&mut self, values: &[u32]) -> usize {
        let small = values.iter().all(|v| *v < 65_536);
        let bytes: Vec<u8> = if small {
            values.iter().flat_map(|v| (*v as u16).to_le_bytes()).collect()
        } else {
            values.iter().flat_map(|v| v.to_le_bytes()).collect()
        };
        let view = self.view(&bytes);
        let component = if small { 5123 } else { 5125 };
        push(
            &mut self.accessors,
            format!(r#"{{"bufferView":{view},"componentType":{component},"count":{},"type":"SCALAR"}}"#, values.len()),
        )
    }

    /// An accessor as raw JSON, for files that lie.
    pub fn accessor(&mut self, json: String) -> usize {
        push(&mut self.accessors, json)
    }

    /// A mesh of one triangle-list primitive.
    pub fn mesh(&mut self, attributes: &[(&str, usize)], indices: Option<usize>, material: Option<usize>) -> usize {
        let prim = primitive(attributes, indices, material, None);
        push(&mut self.meshes, format!(r#"{{"primitives":[{prim}]}}"#))
    }

    /// A mesh as raw JSON.
    pub fn mesh_json(&mut self, json: String) -> usize {
        push(&mut self.meshes, json)
    }

    pub fn material(&mut self, json: String) -> usize {
        push(&mut self.materials, json)
    }

    /// A PNG as a texture, sampled nearest or linear.
    pub fn png_texture(&mut self, png: &[u8], nearest: bool) -> usize {
        let view = self.view(png);
        let image = push(&mut self.images, format!(r#"{{"bufferView":{view},"mimeType":"image/png"}}"#));
        let filter = if nearest { 9728 } else { 9729 };
        let sampler = push(&mut self.samplers, format!(r#"{{"magFilter":{filter},"minFilter":{filter}}}"#));
        push(&mut self.textures, format!(r#"{{"source":{image},"sampler":{sampler}}}"#))
    }

    pub fn node(&mut self, json: String) -> usize {
        push(&mut self.nodes, json)
    }

    pub fn add_scene(&mut self, roots: &[usize]) -> usize {
        let roots = roots.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(",");
        push(&mut self.scenes, format!(r#"{{"nodes":[{roots}]}}"#))
    }

    pub fn animation(&mut self, json: String) -> usize {
        push(&mut self.animations, json)
    }

    /// The JSON, its buffer at `uri` (`None`: the GLB's BIN chunk).
    pub fn json(&self, uri: Option<&str>) -> String {
        let mut fields = vec![r#""asset":{"version":"2.0","generator":"glass glb.rs"}"#.to_string()];
        let mut list = |name: &str, items: &[String]| {
            if !items.is_empty() {
                fields.push(format!(r#""{name}":[{}]"#, items.join(",")));
            }
        };
        list("bufferViews", &self.views);
        list("accessors", &self.accessors);
        list("meshes", &self.meshes);
        list("materials", &self.materials);
        list("textures", &self.textures);
        list("images", &self.images);
        list("samplers", &self.samplers);
        list("nodes", &self.nodes);
        list("scenes", &self.scenes);
        list("animations", &self.animations);
        if !self.bin.is_empty() {
            let buffer = match uri {
                Some(uri) => format!(r#"{{"byteLength":{},"uri":"{uri}"}}"#, self.bin.len()),
                None => format!(r#"{{"byteLength":{}}}"#, self.bin.len()),
            };
            fields.push(format!(r#""buffers":[{buffer}]"#));
        }
        if let Some(scene) = self.scene {
            fields.push(format!(r#""scene":{scene}"#));
        }
        fields.extend(self.extra.iter().cloned());
        format!("{{{}}}", fields.join(","))
    }

    /// The file as a `.glb`.
    pub fn glb(&self) -> Vec<u8> {
        container(self.json(None).as_bytes(), (!self.bin.is_empty()).then_some(self.bin.as_slice()))
    }

    /// The file as a `.gltf`, its buffer a base64 `data:` URI.
    pub fn gltf(&self) -> Vec<u8> {
        let uri = format!("data:application/octet-stream;base64,{}", base64(&self.bin));
        self.json(Some(&uri)).into_bytes()
    }
}

/// One primitive as JSON; `mode` defaults to triangles.
pub fn primitive(attributes: &[(&str, usize)], indices: Option<usize>, material: Option<usize>, mode: Option<u32>) -> String {
    let attrs = attributes.iter().map(|(name, a)| format!(r#""{name}":{a}"#)).collect::<Vec<_>>().join(",");
    let mut out = format!(r#"{{"attributes":{{{attrs}}}"#);
    if let Some(i) = indices {
        out.push_str(&format!(r#","indices":{i}"#));
    }
    if let Some(m) = material {
        out.push_str(&format!(r#","material":{m}"#));
    }
    if let Some(mode) = mode {
        out.push_str(&format!(r#","mode":{mode}"#));
    }
    out.push('}');
    out
}

/// A GLB container round a JSON chunk and perhaps a BIN chunk, each padded
/// to four bytes as the format wants.
pub fn container(json: &[u8], bin: Option<&[u8]>) -> Vec<u8> {
    let mut json = json.to_vec();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut body = Vec::new();
    body.extend_from_slice(&(json.len() as u32).to_le_bytes());
    body.extend_from_slice(b"JSON");
    body.extend_from_slice(&json);
    if let Some(bin) = bin {
        let mut bin = bin.to_vec();
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        body.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        body.extend_from_slice(b"BIN\0");
        body.extend_from_slice(&bin);
    }
    let mut out = Vec::with_capacity(12 + body.len());
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&((12 + body.len()) as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// Standard base64, padded.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// RGBA pixels as a PNG, its deflate stream stored (not compressed): a
/// valid file any decoder takes, made without a compressor.
pub fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for row in rgba.chunks(width as usize * 4).take(height as usize) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    if blocks.peek().is_none() {
        zlib.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(block) = blocks.next() {
        zlib.push(u8::from(blocks.peek().is_none()));
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in &raw {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = 0xffff_ffffu32;
    for byte in kind.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
}
