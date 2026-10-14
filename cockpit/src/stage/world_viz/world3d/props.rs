//! Authored props: meshes generated offline (Hunyuan 3D), reduced to a couple
//! of thousand faces and tagged with the shared semantic materials, so they
//! light and fog like everything the architecture generators build.
//!
//! The asset format is a small OBJ subset: `v x y z` in tile units with z up
//! and the base on z = 0, `usemtl <material>` naming one of [`material`]'s
//! words, and triangle `f a b c` lines. Provenance for each prop is in
//! `cockpit/assets/realm/props/README.md`.

use std::sync::{Arc, OnceLock};

use super::math::{V3, v3};
use super::mesh::{Mesh, mat};

/// The props a scene places, by name. Props no scene uses yet live as files in
/// `assets/realm/props/library/`, checked by the tests but not compiled in.
pub(crate) const PROPS: &[(&str, &str)] = &[
    (
        "dragon_skull",
        include_str!("../../../../assets/realm/props/dragon_skull.obj"),
    ),
    (
        "knight_statue",
        include_str!("../../../../assets/realm/props/knight_statue.obj"),
    ),
    (
        "siege_perilous",
        include_str!("../../../../assets/realm/props/siege_perilous.obj"),
    ),
];

/// The material id an asset's `usemtl` word stands for.
pub(crate) fn material(word: &str) -> Option<u8> {
    Some(match word {
        "stone" => mat::STONE,
        "stone_dark" => mat::STONE_DARK,
        "roof" => mat::ROOF,
        "wood" => mat::WOOD,
        "trunk" => mat::TRUNK,
        "door" => mat::DOOR,
        "ember" => mat::EMBER,
        "fire" => mat::FIRE,
        "grass" => mat::GRASS,
        "foliage" => mat::FOLIAGE,
        "banner" => mat::BANNER,
        "brass" => mat::BRASS,
        "parchment" => mat::PARCHMENT,
        "crystal" => mat::CRYSTAL,
        "rock" => mat::ROCK,
        "armor" => mat::ARMOR,
        "cloth_gold" => mat::CLOTH_GOLD,
        "cloth_blue" => mat::CLOTH_BLUE,
        "leather" => mat::TACK_LEATHER,
        "water" => mat::WATER,
        "path" => mat::PATH,
        _ => return None,
    })
}

/// Parse a prop. Unknown lines are skipped; a face naming a vertex that does
/// not exist, or a material word that does not exist, is an error.
pub(crate) fn parse(text: &str) -> Result<Mesh, String> {
    let mut points: Vec<V3> = Vec::new();
    let mut mesh = Mesh::new();
    let mut current = mat::STONE;
    for (n, line) in text.lines().enumerate() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => {
                let xyz: Vec<f32> = words.filter_map(|w| w.parse().ok()).collect();
                let [x, y, z] = xyz[..] else {
                    return Err(format!("line {}: a vertex needs three numbers", n + 1));
                };
                points.push(v3(x, y, z));
            }
            Some("usemtl") => {
                let word = words.next().unwrap_or("");
                current = material(word)
                    .ok_or_else(|| format!("line {}: no material called {word:?}", n + 1))?;
            }
            Some("f") => {
                let index: Vec<usize> = words
                    .filter_map(|w| w.split('/').next()?.parse::<usize>().ok())
                    .collect();
                if index.len() < 3 {
                    return Err(format!("line {}: a face needs three vertices", n + 1));
                }
                let corner = |i: usize| {
                    i.checked_sub(1)
                        .and_then(|i| points.get(i).copied())
                        .ok_or_else(|| format!("line {}: no vertex {i}", n + 1))
                };
                let first = corner(index[0])?;
                for pair in index[1..].windows(2) {
                    let tri = [first, corner(pair[0])?, corner(pair[1])?];
                    // A sliver a decimator left behind has no facing to shade.
                    if (tri[1] - tri[0]).cross(tri[2] - tri[0]).length() < 1e-6 {
                        continue;
                    }
                    mesh.push_tri(tri, surface_uv(tri), current);
                }
            }
            _ => {}
        }
    }
    Ok(mesh)
}

/// Surface coordinates for a face of a sculpted mesh: projected onto the axis
/// plane it faces most, at the generators' scale of about one per tile, so
/// ashlar and grain patterns land on it the way they land on a wall.
fn surface_uv(tri: [V3; 3]) -> [[f32; 2]; 3] {
    let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
    let (ax, ay, az) = (normal.x.abs(), normal.y.abs(), normal.z.abs());
    tri.map(|p| {
        if az >= ax && az >= ay {
            [p.x, p.y]
        } else if ax >= ay {
            [p.y, p.z]
        } else {
            [p.x, p.z]
        }
    })
}

/// A bundled prop by name, parsed once.
pub(crate) fn prop(name: &str) -> Option<Arc<Mesh>> {
    static CACHE: OnceLock<Vec<(&'static str, Arc<Mesh>)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        PROPS
            .iter()
            .filter_map(|(name, text)| Some((*name, Arc::new(parse(text).ok()?))))
            .collect()
    });
    cache
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, mesh)| Arc::clone(mesh))
}

/// A copy of a bundled prop turned `yaw` radians and standing at `at`.
pub(crate) fn placed(name: &str, at: V3, yaw: f32, scale: f32) -> Option<Mesh> {
    let mesh = prop(name)?;
    Some(
        Mesh { tris: mesh.tris.clone() }
            .scaled(scale)
            .rotated_z(yaw)
            .translated(at),
    )
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/world3d__props__tests.rs"]
mod tests;
