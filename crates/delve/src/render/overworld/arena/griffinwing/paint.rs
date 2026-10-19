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

/// The share of a painted tile's light the canyon keeps before the haze. The
/// painted stone is dim already: at seven tenths (what a lit test frame would
/// suit) the gorge's mean value falls under the floor the canyon-lifting pass
/// set (`the_canyon_reads_at_every_beat_and_is_not_a_black_wall`), so the
/// stage takes a tenth off and no more.
const KEEP: f32 = 0.9;
/// The share of the stone's light the dark stone keeps (the code's dark stone
/// was about three fifths of its stone).
const DARK: f32 = 0.72;

/// A tile of the slot, the table's 8 units wide holding it `across` times (a
/// tile is `8 / across` units), its colours scaled to `keep` of their light.
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

/// The surfaces of beat `stage`, when the slot is painted whole: the gorge's
/// and the roost's rock and stone, the causeway's brick for its walls and the
/// stone for its road, the roost's bone, the mist below.
fn make(stage: u8) -> Option<Painted> {
    let mut p = Painted::new();
    // Rock and stone: a tile is 4 units.
    p.set(mat::ROCK, table("rock", 2, KEEP)?);
    p.set(mat::STONE, table("stone", 2, KEEP)?);
    p.set(mat::STONE_DARK, table("stone", 2, KEEP * DARK)?);
    // The cloud sea far below, in every beat: a tile is 4 units.
    p.set(mat::WATER, table("mist", 2, KEEP)?);
    if stage == 1 {
        // Brick courses with the windows set in them: a tile is 8 units.
        p.set(mat::STONE, table("causeway", 1, KEEP)?);
        p.set(mat::PATH, table("stone", 2, KEEP)?);
    }
    if stage == 2 {
        // Ribs a half unit wide: a tile is 2 units.
        p.set(mat::BONE, table("bone", 4, KEEP)?);
    }
    Some(p)
}

/// The painted surfaces for beat `stage` (0 to 2), or `None` to run the shared
/// material table.
pub(super) fn painted(stage: u8) -> Option<&'static Painted> {
    static BEATS: OnceLock<[Option<Painted>; 3]> = OnceLock::new();
    BEATS.get_or_init(|| [make(0), make(1), make(2)])[usize::from(stage.min(2))].as_ref()
}
