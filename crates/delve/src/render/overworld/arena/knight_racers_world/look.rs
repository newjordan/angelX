//! What each place is made of: materials for the road, its barriers and the land, the roles
//! every set piece draws from, and how the place is dressed. A `Look` is one course's own
//! (see `route::Look`), so the style is the place.
use super::palette::*;
use super::route::Look;
use crate::render::world3d::mesh::mat;

pub struct Style {
    pub road: [u8; 2],
    pub kerb: [u8; 2],
    pub rail: u8,
    pub rail_cap: u8,
    pub post: u8,
    pub lamp: u8,
    /// Lamps every this many metres (0: none).
    pub lamp_every: f32,
    pub ground: u8,
    pub rock: u8,
    pub liquid: Option<u8>,
    // The roles the set pieces draw from, so a place's gates and stands wear its own materials.
    pub plate: u8,
    pub plate_b: u8,
    pub masonry: u8,
    pub slate: u8,
    pub timber: u8,
    pub cloth: u8,
    pub cloth_b: u8,
    pub glow: u8,
    /// The light a seam, a lane line or a boost strip is made of.
    pub seam: u8,
    /// The hot end of the place's fire: boost strips, kerb studs, the top of a flame.
    pub hot: u8,
    /// Hazard bars: what a block that hurts is dressed in.
    pub warn: u8,
    /// A crowd's colours, for a grandstand.
    pub crowd: [u8; 6],
    /// The dim light a lane line is made of.
    pub lane: u8,
    /// The road beside a drop (a lake, a chasm): the plate that takes the glow of it.
    pub road_hot: u8,
    /// Whether the road is laid in long plates (not a checker) and wears the marks below.
    pub plated: bool,
    /// Metres between the seams across the road (0: none), and whether lane lines run down it.
    pub seam_every: f32,
    pub lanes: bool,
    /// The beat of what stands beside the road: 0 none, 1 pylons of fire, 2 crystal clusters, 3 braziers and banners.
    pub beat: u8,
    /// The pool of a lamp's light laid on the stones.
    pub pool: u8,
    /// Rock whose face is lit from its foot (its `v` is height above the road, not above the sea).
    pub wall_glow: bool,
    /// Metres along the road to one unit of the road table's `u`, when the road wears a surface laid in
    /// its own frame (see `roadtex`): its table goes `road[0]`, and `road[1]` where the land falls away.
    /// 0 for a road tiled over the world as before.
    pub road_along: f32,
}

const OLD_CROWD: [u8; 6] = [CLOTH, CLOTH_R, GLOW, STEEL, mat::BONE, LAVA];
const EF_CROWD: [u8; 6] = [EF_BRICK, EF_IRON, EF_ROAD_HOT, EF_BRICK, WINDOW_DIM, EF_RAIL];
const GL_CROWD: [u8; 6] = [GL_BLOCK, GL_RAIL, GL_ROAD_B, GL_BLOCK, CYAN_DIM, GL_RAIL];
const MW_CROWD: [u8; 6] = [MW_BANNER, MW_BANNER_B, MW_TIMBER, MW_WALL, WINDOW_HOT, MW_ROOF];

pub fn style(look: Look) -> Style {
    match look {
        Look::Dune => Style { road: [ROAD_A, ROAD_B], kerb: [STEEL, CLOTH_R], rail: ROAD_B, rail_cap: STEEL, post: TIMBER, lamp: GLOW, lamp_every: 0., ground: SAND, rock: MESA, liquid: None,
            plate: PLATE, plate_b: PLATE_B, masonry: MASONRY, slate: SLATE, timber: TIMBER, cloth: CLOTH, cloth_b: CLOTH_R, glow: GLOW, seam: GLOW, hot: LAVA_HOT, warn: CLOTH_R, crowd: OLD_CROWD, lane: GLOW,
            road_hot: ROAD_B, plated: false, seam_every: 0., lanes: false, beat: 0, pool: LANTERN_POOL, wall_glow: false, road_along: 0. },
        Look::Forge | Look::Ash => Style { road: [EF_ROAD_A, EF_ROAD_HOT], kerb: [LAVA_HOT, EF_KERB], rail: EF_RAIL, rail_cap: SEAM, post: EF_IRON, lamp: WINDOW_HOT, lamp_every: 16., ground: EF_ASH, rock: EF_WALL,
            liquid: if look == Look::Forge { Some(LAVA) } else { None },
            plate: EF_IRON, plate_b: EF_RAIL, masonry: EF_BRICK, slate: EF_SOOT, timber: EF_IRON, cloth: EF_BRICK, cloth_b: EF_BARS, glow: WINDOW_HOT, seam: SEAM, hot: LAVA_HOT, warn: EF_BARS, crowd: EF_CROWD, lane: EMBER_DIM,
            road_hot: EF_ROAD_HOT, plated: true, seam_every: 14., lanes: true, beat: 1, pool: LANTERN_POOL, wall_glow: true, road_along: super::roadtex::EMBER_ALONG },
        Look::Deep => Style { road: [GL_ROAD_A, GL_ROAD_B], kerb: [CYAN_DIM, GL_KERB], rail: GL_RAIL, rail_cap: CYAN_DIM, post: GL_RAIL, lamp: LANTERN, lamp_every: 16., ground: GL_FLOOR, rock: GL_WALL, liquid: Some(GL_MIST),
            plate: GL_RAIL, plate_b: GL_RAIL, masonry: GL_BLOCK, slate: GL_RAIL, timber: TIMBER, cloth: CLOTH, cloth_b: CLOTH_R, glow: LANTERN, seam: CYAN_GLOW, hot: LAVA_HOT, warn: CLOTH_R, crowd: GL_CROWD, lane: CYAN_DIM,
            road_hot: GL_ROAD_B, plated: true, seam_every: 12., lanes: true, beat: 2, pool: CYAN_POOL, wall_glow: true, road_along: super::roadtex_glass::GLASS_ALONG },
        Look::Castle => Style { road: [MW_ROAD_A, MW_ROAD_B], kerb: [MW_KERB, MW_PARAPET], rail: MW_PARAPET, rail_cap: MW_WALL, post: MW_IRON, lamp: TORCH, lamp_every: 16., ground: MW_GROUND, rock: CAVE, liquid: Some(WATER),
            plate: MW_IRON, plate_b: MW_IRON, masonry: MW_WALL, slate: MW_ROOF, timber: MW_TIMBER, cloth: MW_BANNER_B, cloth_b: MW_BANNER, glow: WINDOW_HOT, seam: TORCH_DIM, hot: LAVA_HOT, warn: MW_BANNER, crowd: MW_CROWD, lane: TORCH_DIM,
            road_hot: MW_ROAD_B, plated: true, seam_every: 0., lanes: false, beat: 3, pool: WARM_POOL, wall_glow: false, road_along: super::roadtex_moat::MOAT_ALONG },
    }
}
