//! The floor pickups' own models: the art team's item drawings made 3D
//! (TRELLIS.2) and cut down for the floor by `scripts/delve/item_meshes.py`,
//! one mesh each, coloured per vertex. A pickup whose drawing has an
//! approved model is drawn as it, dropped onto the floor
//! (`pickup_meshes::plop`); one without keeps its card (`pickup_cards`).
//!
//! The pack is read once. Each mesh is centred on its box, its longest side
//! 1, `y` up and `+z` to the camera, as the stage's world is; the game sizes
//! it. Its id is what the props name (`MeshRef::Uploaded`) and what the
//! stage sends once (`StageMaker`).

use std::sync::OnceLock;

use angel_stage::scene::MeshData;

const PACK: &[u8] = include_bytes!("../../../../assets/items/items.mesh");

pub const FIRST_ID: u32 = 0x4954_0000;

pub struct Model {
    pub name: String,
    pub mesh: MeshData,
    pub half: [f32; 3],
}

fn models() -> &'static [Model] {
    static MODELS: OnceLock<Vec<Model>> = OnceLock::new();
    MODELS.get_or_init(|| read(PACK).unwrap_or_default())
}

pub fn read(bytes: &[u8]) -> Option<Vec<Model>> {
    let mut r = Reader { bytes, at: 0 };
    let mut take = |n: usize| r.take(n);
    if take(4)? != b"AIM1" {
        return None;
    }
    let u32_of = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let count = u32_of(take(4)?) as usize;
    let mut out = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        let len = take(1)?[0] as usize;
        let name = String::from_utf8(take(len)?.to_vec()).ok()?;
        let vertices = u32_of(take(4)?) as usize;
        let triangles = u32_of(take(4)?) as usize;
        let raw = take(vertices.checked_mul(14)?)?;
        let mut mesh = MeshData::default();
        for v in raw.chunks_exact(14) {
            let p = |k: usize| f32::from(i16::from_le_bytes([v[2 * k], v[2 * k + 1]])) / 32767.0 * 0.5;
            let n = |k: usize| f32::from(v[6 + k] as i8) / 127.0;
            mesh.positions.push([p(0), p(1), p(2)]);
            mesh.normals.push([n(0), n(1), n(2)]);
            mesh.colors.push([10, 11, 12, 13].map(|k| f32::from(v[k]) / 255.0));
        }
        let idx = take(triangles.checked_mul(6)?)?;
        mesh.indices = idx.chunks_exact(2).map(|c| u32::from(u16::from_le_bytes([c[0], c[1]]))).collect();
        mesh.validate().ok()?;
        let mut half = [0.0f32; 3];
        for k in 0..3 {
            let (lo, hi) = mesh.positions.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[k]), hi.max(p[k])));
            half[k] = (hi - lo) / 2.0;
        }
        out.push(Model { name, mesh, half });
    }
    Some(out)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
}

const APPROVED: [&str; 14] = [
    "sword", "axe", "spear", "staff", "hammer", "mace", "shield", "gem", "gear", "flask", "dagger", "gold", "pouch", "ore",
];

const FLASK: &str = "flask";

fn bottles() -> &'static [Model] {
    static BOTTLES: OnceLock<Vec<Model>> = OnceLock::new();
    BOTTLES.get_or_init(|| {
        let Some(flask) = models().iter().find(|m| m.name == FLASK) else { return Vec::new() };
        [("flask-red", [0.96, 0.24, 0.18]), ("flask-pale", [0.62, 0.76, 0.94])]
            .into_iter()
            .map(|(name, glass)| {
                let mut mesh = flask.mesh.clone();
                for c in &mut mesh.colors {
                    let l = (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]).clamp(0.0, 1.0);
                    let k = 0.55 + 1.6 * l;
                    *c = [(glass[0] * k).min(1.0), (glass[1] * k).min(1.0), (glass[2] * k).min(1.0), c[3]];
                }
                Model { name: name.into(), mesh, half: flask.half }
            })
            .collect()
    })
}

pub fn model(plate: u8, pale: bool) -> Option<(u32, &'static Model)> {
    let name = *super::pickup_cards::PLATES.get(usize::from(plate))?;
    if !APPROVED.contains(&name) {
        return None;
    }
    if name == FLASK {
        let k = usize::from(pale);
        return bottles().get(k).map(|m| (FIRST_ID + (models().len() + k) as u32, m));
    }
    let i = models().iter().position(|m| m.name == name)?;
    Some((FIRST_ID + i as u32, &models()[i]))
}

pub fn mesh(id: u32) -> Option<&'static MeshData> {
    let i = id.checked_sub(FIRST_ID)? as usize;
    models().get(i).or_else(|| bottles().get(i.checked_sub(models().len())?)).map(|m| &m.mesh)
}

pub fn size(plate: u8) -> f32 {
    match super::pickup_cards::PLATES.get(usize::from(plate)).copied().unwrap_or("") {
        "spear" | "staff" => 21.0,
        "sword" | "mace" => 18.0,
        "axe" | "hammer" => 16.0,
        "shield" | "dagger" => 12.0,
        "gold" => 11.0,
        _ => 10.0,
    }
}

pub fn lies_flat(plate: u8) -> bool {
    matches!(
        super::pickup_cards::PLATES.get(usize::from(plate)).copied(),
        Some("sword" | "axe" | "spear" | "staff" | "hammer" | "mace" | "dagger" | "shield" | "gear")
    )
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__item_pack_tests.rs"]
mod tests;
