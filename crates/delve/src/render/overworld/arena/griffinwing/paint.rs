//! The canyon's painted surfaces: the walls' rock, stone and bone, the road
//! and the mist sea below, laid over the rasteriser's shared material table
//! for this scene only (`raster::Painted`, handed to `render_meshes_painted`;
//! every other 3D drawer in the game runs the shared table untouched). The
//! tiles are the `griffinwing.canyon-tex` slot; the stage chooses how many
//! units a tile spans and how much of its light the canyon keeps before the
//! haze (a stage setting: the imported pixels are the painting's own). Lava
//! and the lit windows stay on their emissive materials, in the code.
use super::bank;
use crate::render::world3d::mesh::mat;
use crate::render::world3d::raster::Painted;
use std::sync::OnceLock;

const KEEP: f32 = 0.9;
const DARK: f32 = 0.72;

fn table(name: &str, across: usize, keep: f32) -> Option<Vec<[u8; 3]>> {
    let tile = &bank::pic("canyon-tex", name, 0)?.img;
    let side = Painted::SIDE;
    let mut out = Vec::with_capacity(side * side);
    for y in 0..side {
        for x in 0..side {
            let c = tile
                .get((x * across / 4 % 64) as i32, (y * across / 4 % 64) as i32)
                .unwrap_or([0, 0, 0]);
            out.push(c.map(|v| (f32::from(v) * keep).round() as u8));
        }
    }
    Some(out)
}

fn make(stage: u8) -> Option<Painted> {
    let mut p = Painted::new();
    let _ = canyon(&mut p, stage);
    p.set(mat::STABLE_ROOF, super::skin::scales());
    p.set(mat::STABLE_WALL, super::skin::plates());
    p.set(mat::STABLE_TIMBER, super::skin::horn());
    p.set(mat::STABLE_FLOOR, super::skin::belly());
    p.set(mat::TACK_LEATHER, super::skin::web());
    p.set(mat::HAY, super::skin::slab());
    Some(p)
}

fn canyon(p: &mut Painted, stage: u8) -> Option<()> {
    p.set(mat::ROCK, table("rock", 2, KEEP)?);
    p.set(mat::STONE, table("stone", 2, KEEP)?);
    p.set(mat::STONE_DARK, table("stone", 2, KEEP * DARK)?);
    p.set(mat::WATER, table("mist", 2, KEEP)?);
    if stage == 1 {
        p.set(mat::STONE, table("causeway", 1, KEEP)?);
        p.set(mat::PATH, table("stone", 2, KEEP)?);
    }
    if stage == 2 {
        p.set(mat::BONE, table("bone", 4, KEEP)?);
    }
    Some(())
}

pub(super) fn painted(stage: u8) -> Option<&'static Painted> {
    static BEATS: OnceLock<[Option<Painted>; 3]> = OnceLock::new();
    BEATS.get_or_init(|| [make(0), make(1), make(2)])[usize::from(stage.min(2))].as_ref()
}
