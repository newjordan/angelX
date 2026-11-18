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
    pub lamp_every: f32,
    pub ground: u8,
    pub rock: u8,
    pub liquid: Option<u8>,
    pub plate: u8,
    pub plate_b: u8,
    pub masonry: u8,
    pub slate: u8,
    pub timber: u8,
    pub cloth: u8,
    pub cloth_b: u8,
    pub glow: u8,
    pub seam: u8,
    pub hot: u8,
    pub warn: u8,
    pub crowd: [u8; 6],
    pub lane: u8,
    pub road_hot: u8,
    pub plated: bool,
    pub seam_every: f32,
    pub lanes: bool,
    pub beat: u8,
    pub pool: u8,
    pub wall_glow: bool,
    pub road_along: f32,
    pub chevron: [u8; 2],
    pub banner: [u8; 2],
}

const CHEVRON: [u8; 2] = [HAZARD_BLACK, HAZARD_GOLD];
const BANNER: [u8; 2] = [CLOTH, HAZARD_GOLD];
const WA_CROWD: [u8; 6] = [WA_BANNER, WA_BANNER_B, WA_TIMBER, WA_ASHLAR, WA_LANTERN_DIM, WA_SLATE];
const OLD_CROWD: [u8; 6] = [CLOTH, CLOTH_R, GLOW, STEEL, mat::BONE, LAVA];
const EF_CROWD: [u8; 6] = [EF_BRICK, EF_IRON, EF_ROAD_HOT, EF_BRICK, WINDOW_DIM, EF_RAIL];
const GL_CROWD: [u8; 6] = [GL_BLOCK, GL_RAIL, GL_ROAD_B, GL_BLOCK, CYAN_DIM, GL_RAIL];
const MW_CROWD: [u8; 6] = [MW_BANNER, MW_BANNER_B, MW_TIMBER, MW_WALL, WINDOW_HOT, MW_ROOF];

pub fn style(look: Look) -> Style {
    match look {
        Look::Dune => Style { road: [ROAD_A, ROAD_B], kerb: [STEEL, CLOTH_R], rail: ROAD_B, rail_cap: STEEL, post: TIMBER, lamp: GLOW, lamp_every: 0., ground: SAND, rock: MESA, liquid: None,
            plate: PLATE, plate_b: PLATE_B, masonry: MASONRY, slate: SLATE, timber: TIMBER, cloth: CLOTH, cloth_b: CLOTH_R, glow: GLOW, seam: GLOW, hot: LAVA_HOT, warn: CLOTH_R, crowd: OLD_CROWD, lane: GLOW,
            road_hot: ROAD_B, plated: false, seam_every: 0., lanes: false, beat: 0, pool: LANTERN_POOL, wall_glow: false, road_along: 0., chevron: CHEVRON, banner: BANNER },
        Look::Forge | Look::Ash => Style { road: [EF_ROAD_A, EF_ROAD_HOT], kerb: [LAVA_HOT, EF_KERB], rail: EF_RAIL, rail_cap: SEAM, post: EF_IRON, lamp: WINDOW_HOT, lamp_every: 16., ground: EF_ASH, rock: EF_WALL,
            liquid: if look == Look::Forge { Some(LAVA) } else { None },
            plate: EF_IRON, plate_b: EF_RAIL, masonry: EF_BRICK, slate: EF_SOOT, timber: EF_IRON, cloth: EF_BRICK, cloth_b: EF_BARS, glow: WINDOW_HOT, seam: SEAM, hot: LAVA_HOT, warn: EF_BARS, crowd: EF_CROWD, lane: EMBER_DIM,
            road_hot: EF_ROAD_HOT, plated: true, seam_every: 14., lanes: true, beat: 1, pool: LANTERN_POOL, wall_glow: true, road_along: super::roadtex::EMBER_ALONG, chevron: CHEVRON, banner: BANNER },
        Look::Deep => Style { road: [GL_ROAD_A, GL_ROAD_B], kerb: [CYAN_DIM, GL_KERB], rail: GL_RAIL, rail_cap: CYAN_DIM, post: GL_RAIL, lamp: LANTERN, lamp_every: 16., ground: GL_FLOOR, rock: GL_WALL, liquid: Some(GL_MIST),
            plate: GL_RAIL, plate_b: GL_RAIL, masonry: GL_BLOCK, slate: GL_RAIL, timber: TIMBER, cloth: CLOTH, cloth_b: CLOTH_R, glow: LANTERN, seam: CYAN_GLOW, hot: LAVA_HOT, warn: CLOTH_R, crowd: GL_CROWD, lane: CYAN_DIM,
            road_hot: GL_ROAD_B, plated: true, seam_every: 12., lanes: true, beat: 2, pool: CYAN_POOL, wall_glow: true, road_along: super::roadtex_glass::GLASS_ALONG, chevron: CHEVRON, banner: BANNER },
        Look::Castle => Style { road: [MW_ROAD_A, MW_ROAD_B], kerb: [MW_KERB, MW_PARAPET], rail: MW_PARAPET, rail_cap: MW_WALL, post: MW_IRON, lamp: TORCH, lamp_every: 16., ground: MW_GROUND, rock: CAVE, liquid: Some(WATER),
            plate: MW_IRON, plate_b: MW_IRON, masonry: MW_WALL, slate: MW_ROOF, timber: MW_TIMBER, cloth: MW_BANNER_B, cloth_b: MW_BANNER, glow: WINDOW_HOT, seam: TORCH_DIM, hot: LAVA_HOT, warn: MW_BANNER, crowd: MW_CROWD, lane: TORCH_DIM,
            road_hot: MW_ROAD_B, plated: true, seam_every: 0., lanes: false, beat: 3, pool: WARM_POOL, wall_glow: false, road_along: super::roadtex_moat::MOAT_ALONG, chevron: CHEVRON, banner: BANNER },
        Look::Wash => Style { road: [WA_ROAD_A, WA_ROAD_B], kerb: [WA_KERB, WA_RAIL], rail: WA_RAIL, rail_cap: WA_CAP, post: WA_POST, lamp: WA_LANTERN, lamp_every: 16., ground: WA_GROUND, rock: WA_ROCK, liquid: Some(WA_WASH),
            plate: WA_PLATE, plate_b: WA_POST, masonry: WA_ASHLAR, slate: WA_SLATE, timber: WA_TIMBER, cloth: WA_BANNER, cloth_b: WA_BANNER_B, glow: WA_LANTERN, seam: WA_LANTERN_DIM, hot: WA_LANTERN, warn: WA_BARS, crowd: WA_CROWD, lane: WA_LANTERN_DIM,
            road_hot: WA_ROAD_B, plated: true, seam_every: 0., lanes: false, beat: 4, pool: WA_POOL, wall_glow: false, road_along: super::roadtex_wash::WASH_ALONG, chevron: [WA_POST, WA_CAP], banner: [WA_BANNER_B, WA_BANNER] },
    }
}
