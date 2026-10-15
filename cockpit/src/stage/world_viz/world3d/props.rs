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
        "fire" => mat::FLAME,
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
    // Sculpted tongues keep their authored shapes. Map their local volume
    // onto the flame ramp once, without a glazing lattice or live shading work.
    let flames: Vec<V3> = mesh
        .tris
        .iter()
        .filter(|t| t.mat == mat::FLAME)
        .flat_map(|t| t.v)
        .collect();
    if !flames.is_empty() {
        let bounds = |axis: fn(V3) -> f32| {
            (
                flames
                    .iter()
                    .copied()
                    .map(axis)
                    .fold(f32::INFINITY, f32::min),
                flames
                    .iter()
                    .copied()
                    .map(axis)
                    .fold(f32::NEG_INFINITY, f32::max),
            )
        };
        let (x0, x1) = bounds(|p| p.x);
        let (y0, y1) = bounds(|p| p.y);
        let (z0, z1) = bounds(|p| p.z);
        for tri in mesh.tris.iter_mut().filter(|t| t.mat == mat::FLAME) {
            tri.uv = tri.v.map(|p| {
                let u = if tri.normal.x.abs() > tri.normal.y.abs() {
                    (p.y - y0) / (y1 - y0).max(0.001)
                } else {
                    (p.x - x0) / (x1 - x0).max(0.001)
                };
                [u, (p.z - z0) / (z1 - z0).max(0.001)]
            });
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

/// Finish the three placed sculptures once, before their cached copies are
/// transformed. A small curvature-aware relaxation rounds decimator kinks;
/// feet, extrema, sharp horns and deep creases stay pinned. No face is added.
fn sculpt(mut mesh: Mesh, name: &str) -> Mesh {
    use std::collections::BTreeMap;
    let key = |p: V3| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
    let mut corners: BTreeMap<[u32; 3], (V3, V3, u32, Vec<V3>)> = BTreeMap::new();
    let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
    let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
    for tri in &mesh.tris {
        for i in 0..3 {
            let p = tri.v[i];
            lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
            let entry = corners
                .entry(key(p))
                .or_insert((V3::ZERO, V3::ZERO, 0, Vec::new()));
            entry.0 += tri.v[(i + 1) % 3] + tri.v[(i + 2) % 3];
            entry.1 += tri.normal;
            entry.2 += 2;
            entry.3.push(tri.normal);
        }
    }
    for tri in &mut mesh.tris {
        for p in &mut tri.v {
            let (neighbors, normals, n, faces) = &corners[&key(*p)];
            let normal = normals.normalize();
            let extremum = p.z <= 0.01
                || p.x == lo.x
                || p.x == hi.x
                || p.y == lo.y
                || p.y == hi.y
                || p.z == hi.z;
            if !extremum && faces.iter().all(|f| f.dot(normal) > 0.55) {
                *p = p.lerp(*neighbors / *n as f32, 0.12);
            }
        }
    }
    // Average only faces within a rounded plane. Sharp material seams retain
    // their facing, while the skull and armour lose the paper-facet glare.
    let source = mesh.tris.clone();
    for tri in &mut mesh.tris {
        let geometric = (tri.v[1] - tri.v[0]).cross(tri.v[2] - tri.v[0]).normalize();
        let mut normal = geometric;
        for adjacent in &source {
            if adjacent.mat == tri.mat
                && adjacent.normal.dot(geometric) > 0.55
                && adjacent.v.iter().any(|p| tri.v.contains(p))
            {
                normal += adjacent.normal * 0.40;
            }
        }
        tri.normal = normal.normalize();
        tri.mat = match tri.mat {
            mat::STONE => mat::SCULPTED_STONE,
            mat::PARCHMENT if name == "dragon_skull" => mat::BONE,
            m => m,
        };
    }
    mesh
}

/// A bundled prop by name, parsed once.
pub(crate) fn prop(name: &str) -> Option<Arc<Mesh>> {
    static CACHE: OnceLock<Vec<(&'static str, Arc<Mesh>)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        PROPS
            .iter()
            .filter_map(|(name, text)| Some((*name, Arc::new(sculpt(parse(text).ok()?, name)))))
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
