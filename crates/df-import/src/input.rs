//! Reading the export: `kingdom.json`, the `tiles/zNNN.txt` grids and
//! `prospect.txt`.

use crate::text;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// One z-level of the map: the four grids of `tiles/zNNN.txt`, row-major.
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub shape: Vec<u8>,
    pub mat: Vec<u8>,
    pub liquid: Vec<u8>,
    pub rock: Vec<u8>,
}

impl Grid {
    fn at(&self, layer: &[u8], x: i32, y: i32) -> Option<u8> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return None;
        }
        Some(layer[y as usize * self.w + x as usize])
    }
    pub fn shape(&self, x: i32, y: i32) -> u8 {
        self.at(&self.shape, x, y).unwrap_or(b'?')
    }
    pub fn mat(&self, x: i32, y: i32) -> u8 {
        self.at(&self.mat, x, y).unwrap_or(b'?')
    }
    pub fn liquid(&self, x: i32, y: i32) -> u8 {
        self.at(&self.liquid, x, y).unwrap_or(b' ')
    }
    pub fn rock(&self, x: i32, y: i32) -> u8 {
        self.at(&self.rock, x, y).unwrap_or(b' ')
    }
}

pub fn parse_grid(bytes: &[u8]) -> Result<(i32, Grid), String> {
    let lines: Vec<&[u8]> = bytes
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .collect();
    let head = String::from_utf8_lossy(lines.first().copied().unwrap_or(b""));
    let parts: Vec<&str> = head.split_whitespace().collect();
    if parts.len() != 3 || parts[0] != "Z" {
        return Err(format!("bad tile header {head:?}"));
    }
    let z: i32 = parts[1].parse().map_err(|e| format!("bad z {head:?}: {e}"))?;
    let (w, h) = parts[2].split_once('x').ok_or_else(|| format!("bad size {head:?}"))?;
    let w: usize = w.parse().map_err(|e| format!("bad width {head:?}: {e}"))?;
    let h: usize = h.parse().map_err(|e| format!("bad height {head:?}: {e}"))?;
    let mut layers = Vec::new();
    let mut i = 1;
    for name in ["SHAPE", "MAT", "LIQUID", "ROCK"] {
        if lines.get(i).copied() != Some(name.as_bytes()) {
            return Err(format!("z{z}: expected section {name} at line {}", i + 1));
        }
        i += 1;
        let mut buf = Vec::with_capacity(w * h);
        for r in 0..h {
            let row = lines.get(i + r).copied().unwrap_or(b"");
            for c in 0..w {
                buf.push(*row.get(c).unwrap_or(&b' '));
            }
        }
        i += h;
        layers.push(buf);
    }
    let rock = layers.pop().unwrap();
    let liquid = layers.pop().unwrap();
    let mat = layers.pop().unwrap();
    let shape = layers.pop().unwrap();
    Ok((z, Grid { w, h, shape, mat, liquid, rock }))
}

/// A room's tile mask: which tiles of its rectangle belong to it.
pub struct Extents {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub rows: Vec<Vec<u8>>,
}

/// One entry of `kingdom.json` -> `buildings`.
pub struct Building {
    pub id: i64,
    pub kind: String,
    pub subtype: Option<String>,
    pub name: String,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub z: i32,
    pub material: Option<String>,
    pub item: Option<String>,
    pub quality: Option<i64>,
    pub location_id: Option<i64>,
    pub owner: Option<i64>,
    pub unfinished: bool,
    pub artifact: bool,
    pub extents: Option<Extents>,
}

impl Building {
    pub fn is(&self, kind: &str) -> bool {
        self.kind == kind
    }
    pub fn sub(&self) -> &str {
        self.subtype.as_deref().unwrap_or("")
    }
    /// Every map tile the building stands on, in row order.
    pub fn cells(&self) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        if let Some(e) = &self.extents {
            for (yy, row) in e.rows.iter().enumerate() {
                for (xx, &c) in row.iter().enumerate() {
                    if c == b'1' {
                        out.push((e.x + xx as i32, e.y + yy as i32));
                    }
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
        for y in self.y1..=self.y2 {
            for x in self.x1..=self.x2 {
                out.push((x, y));
            }
        }
        out
    }
}

pub fn str_of(v: &Value, k: &str) -> Option<String> {
    match v.get(k)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn int_of(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)))
}

pub fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(|x| x.as_array()).map(|a| a.as_slice()).unwrap_or(&[])
}

fn parse_building(v: &Value) -> Option<Building> {
    let extents = v.get("extents").and_then(|e| {
        Some(Extents {
            x: int_of(e, "x")? as i32,
            y: int_of(e, "y")? as i32,
            w: int_of(e, "w")? as i32,
            h: int_of(e, "h")? as i32,
            rows: arr(e, "rows")
                .iter()
                .filter_map(|r| r.as_str().map(|s| s.as_bytes().to_vec()))
                .collect(),
        })
    });
    Some(Building {
        id: int_of(v, "id")?,
        kind: str_of(v, "type")?,
        subtype: str_of(v, "subtype"),
        name: str_of(v, "name").unwrap_or_default(),
        x1: int_of(v, "x1")? as i32,
        y1: int_of(v, "y1")? as i32,
        x2: int_of(v, "x2")? as i32,
        y2: int_of(v, "y2")? as i32,
        z: int_of(v, "z")? as i32,
        material: str_of(v, "material"),
        item: str_of(v, "item").map(|s| text::clean_item(&s)),
        quality: int_of(v, "quality"),
        location_id: int_of(v, "location_id"),
        owner: int_of(v, "owner"),
        unfinished: v.get("unfinished").and_then(|x| x.as_bool()).unwrap_or(false),
        artifact: v.get("artifact").and_then(|x| x.as_bool()).unwrap_or(false),
        extents,
    })
}

/// DFHack's `prospect all`: section name to (material token, tile count).
pub type Prospect = BTreeMap<String, Vec<(String, u64)>>;

pub fn parse_prospect(text: &str) -> Prospect {
    let mut out: Prospect = BTreeMap::new();
    let mut section = String::new();
    for line in text.lines() {
        if !line.starts_with(' ') && line.trim_end().ends_with(':') {
            section = line.trim_end().trim_end_matches(':').to_string();
            continue;
        }
        if section.is_empty() || line.trim_start().starts_with(">>>") {
            continue;
        }
        if let Some((name, rest)) = line.split_once(" : ") {
            let count = rest.split_whitespace().next().and_then(|n| n.parse().ok());
            if let Some(count) = count {
                out.entry(section.clone()).or_default().push((name.trim().to_string(), count));
            }
        }
    }
    out
}

pub struct Export {
    pub root: Value,
    pub grids: BTreeMap<i32, Grid>,
    pub buildings: Vec<Building>,
    pub prospect: Prospect,
    pub map_w: i32,
    pub map_h: i32,
}

impl Export {
    pub fn shape(&self, z: i32, x: i32, y: i32) -> u8 {
        self.grids.get(&z).map(|g| g.shape(x, y)).unwrap_or(b'?')
    }
    pub fn mat(&self, z: i32, x: i32, y: i32) -> u8 {
        self.grids.get(&z).map(|g| g.mat(x, y)).unwrap_or(b'?')
    }
    pub fn liquid(&self, z: i32, x: i32, y: i32) -> u8 {
        self.grids.get(&z).map(|g| g.liquid(x, y)).unwrap_or(b' ')
    }
    pub fn rock(&self, z: i32, x: i32, y: i32) -> u8 {
        self.grids.get(&z).map(|g| g.rock(x, y)).unwrap_or(b' ')
    }
}

pub fn load(dir: &Path) -> Result<Export, String> {
    let kj = dir.join("kingdom.json");
    let bytes = fs::read(&kj).map_err(|e| format!("{}: {e}", kj.display()))?;
    let mut root: Value = serde_json::from_str(&text::decode_cp437(&bytes))
        .map_err(|e| format!("{}: {e}", kj.display()))?;
    text::capitalize_value(&mut root);
    let mut grids = BTreeMap::new();
    let tiles_dir = dir.join("tiles");
    let mut names: Vec<_> = fs::read_dir(&tiles_dir)
        .map_err(|e| format!("{}: {e}", tiles_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    names.sort();
    let (mut map_w, mut map_h) = (0, 0);
    for p in names {
        let b = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let (z, g) = parse_grid(&b).map_err(|e| format!("{}: {e}", p.display()))?;
        map_w = g.w as i32;
        map_h = g.h as i32;
        grids.insert(z, g);
    }
    if grids.is_empty() {
        return Err(format!("{}: no zNNN.txt tile grids", tiles_dir.display()));
    }
    let buildings = arr(&root, "buildings").iter().filter_map(parse_building).collect();
    let prospect = fs::read(dir.join("prospect.txt"))
        .map(|b| parse_prospect(&text::decode_cp437(&b)))
        .unwrap_or_default();
    Ok(Export { root, grids, buildings, prospect, map_w, map_h })
}
