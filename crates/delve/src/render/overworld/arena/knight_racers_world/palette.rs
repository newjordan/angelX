//! The race's painted materials: a bounded bank laid over the shared one.
//! Every pod material keeps the look it had; the land gets strata, ash,
//! masonry and sky, each on a material the pods do not wear.
#![allow(dead_code)] // a palette: a light kept for a place that has not yet asked for it is not a mistake
use super::noise::{fbm, hash, value};
use super::tex;
use crate::render::world3d::mesh::mat;
use crate::render::world3d::raster::Painted;
use std::sync::OnceLock;

// Pod materials (not repainted here): see `knight_racers.rs`.
pub const IRON: u8 = mat::STONE_DARK;
pub const GUN: u8 = mat::HORSE_GREY;
pub const STEEL: u8 = mat::ARMOR;

// The land.
pub const ROAD_A: u8 = mat::PATH; // flagstone, light
pub const ROAD_B: u8 = mat::STONE; // flagstone, dark
pub const SAND: u8 = mat::HAY; // dune sand
pub const MESA: u8 = mat::ROCK; // banded red-ochre rock
pub const BASALT: u8 = mat::STABLE_WALL; // forge canyon: black banded rock
pub const ASH: u8 = mat::STABLE_FLOOR; // forge ash flats
pub const CAVE: u8 = mat::SCULPTED_STONE; // the deep road's blue-black rock
pub const MOOR: u8 = mat::GRASS; // castle moor
pub const MASONRY: u8 = mat::STABLE_ROOF; // castle ashlar
pub const SLATE: u8 = mat::ROOF; // roofs
pub const TIMBER: u8 = mat::STABLE_TIMBER; // beams, stands
pub const PLATE: u8 = mat::FLOOR; // foundry plate
pub const PLATE_B: u8 = mat::TACK_LEATHER;
pub const DEEP_A: u8 = mat::PARCHMENT; // the deep road's flags
pub const DEEP_B: u8 = mat::BOOKS;
pub const CLOTH: u8 = mat::CLOTH_BLUE; // banners
pub const CLOTH_R: u8 = mat::BANNER; // second banner colour (pods wear it too: kept as is)
pub const SKY: u8 = mat::WINDOW; // the dome and what hangs in it (emissive)
pub const GLOW: u8 = mat::FIRE; // lamps and windows
pub const LAVA: u8 = mat::EMBER;
pub const LAVA_HOT: u8 = mat::FLAME;
pub const CRYSTAL: u8 = mat::CRYSTAL;
pub const WATER: u8 = mat::WATER;

// The Foundry Run's own surfaces. Ids from 40 up are plain lit materials the raster has no
// table of its own for: they exist only as painted tables, so each course may have its own.
pub const EF_ROAD_A: u8 = 40; // basalt plate
pub const EF_ROAD_B: u8 = 41;
pub const EF_ROAD_HOT: u8 = 42; // plate near the lava: the glow lies on it
pub const EF_RAIL: u8 = 43; // barrier face
pub const EF_IRON: u8 = 44; // posts, gantries
pub const EF_BRICK: u8 = 45; // foundry brick
pub const EF_WALL: u8 = 46; // canyon wall, lit from the foot
pub const EF_ASH: u8 = 47; // cinder ground
pub const EF_CRUST: u8 = 48; // cooled slag
pub const EF_KERB: u8 = 49; // black kerb
pub const EF_BARS: u8 = 50; // hazard bars (a gold and black diagonal)
pub const EF_SOOT: u8 = 51; // chimneys, black with a sheen
// The Glassway's: polished dark stone, a rail of cave rock, a wall that glows at the foot with the crystals' light.
pub const GL_ROAD_A: u8 = 52;
pub const GL_ROAD_B: u8 = 53;
pub const GL_RAIL: u8 = 54;
pub const GL_WALL: u8 = 55;
pub const GL_FLOOR: u8 = 56;
pub const GL_BLOCK: u8 = 57;
pub const GL_KERB: u8 = 58;
/// The luminous mist the Glassway's drops are filled with, a little under the road: the light under them.
pub const GL_MIST: u8 = 59;
// Moatwalk's: cobble and flag in night blue, parapet, castle ashlar, moor, slate roof, dark timber and iron.
pub const MW_ROAD_A: u8 = 60;
pub const MW_ROAD_B: u8 = 61;
pub const MW_PARAPET: u8 = 62;
pub const MW_WALL: u8 = 63;
pub const MW_GROUND: u8 = 64;
pub const MW_ROOF: u8 = 65;
pub const MW_TIMBER: u8 = 66;
pub const MW_IRON: u8 = 67;
pub const MW_KERB: u8 = 68;
pub const MW_BANNER: u8 = 69;
pub const MW_BANNER_B: u8 = 70;

/// Flat lights: materials from 128 up stand for a colour of the sky table's specials, drawn
/// unshaded and held through fog like a window. `flat(k)` is special `k`.
pub const fn flat(kind: u8) -> u8 { 128 + kind }
pub fn flat_kind(mat: u8) -> Option<usize> { (mat >= 128).then(|| (mat - 128) as usize) }
/// A crystal's body and its tip, a lantern's flame and its glow on the stone, a shaft of day.
pub const CRYSTAL_BODY: u8 = flat(4);
pub const CRYSTAL_TIP: u8 = flat(5);
pub const LANTERN: u8 = flat(6);
pub const LANTERN_POOL: u8 = flat(7);
pub const SHAFT: u8 = flat(8);
pub const SHAFT_POOL: u8 = flat(9);
// The Foundry Run's lights: a lava in four steps, sparks, windows, seams.
pub const EMBER_BRIGHT: u8 = flat(12);
pub const EMBER_DIM: u8 = flat(13);
pub const SPARK: u8 = flat(14);
pub const LAVA_MID: u8 = flat(15);
pub const LAVA_BRIGHT: u8 = flat(16);
pub const LAVA_CRUST: u8 = flat(17);
pub const WINDOW_HOT: u8 = flat(18);
pub const WINDOW_DIM: u8 = flat(19);
pub const SEAM: u8 = flat(20);
pub const ASH_FLAKE: u8 = flat(21);
pub const SMOKE: u8 = flat(22);
pub const HORIZON_GLOW: u8 = flat(23);
pub const HAZARD_GOLD: u8 = flat(24);
pub const HAZARD_BLACK: u8 = flat(25);
pub const BEACON: u8 = flat(26);
pub const FLAME_LICK: u8 = flat(27);
// The Glassway's and Moatwalk's lights.
pub const CYAN_GLOW: u8 = flat(28);
pub const CYAN_DIM: u8 = flat(29);
pub const VIOLET_GLOW: u8 = flat(30);
pub const VIOLET_DIM: u8 = flat(31);
pub const CYAN_POOL: u8 = flat(32);
pub const VIOLET_POOL: u8 = flat(33);
pub const MOTE: u8 = flat(34);
pub const SKYLINE: u8 = flat(35);
pub const MOON_HALO: u8 = flat(36);
pub const TORCH: u8 = flat(37);
pub const TORCH_DIM: u8 = flat(38);
pub const LANTERN_GLINT: u8 = flat(39);
pub const MOON_GLINT: u8 = flat(40);
pub const SILL: u8 = flat(41);
pub const SPRAY: u8 = flat(42);
pub const VIOLET_MOTE: u8 = flat(43);
pub const NIGHT_BLUE: u8 = flat(44);
pub const CYAN_FACET: u8 = flat(45);
pub const WARM_POOL: u8 = flat(47);
pub const STAR: u8 = flat(0);
pub const MOON: u8 = flat(1);
pub const VIOLET_FACET: u8 = flat(46);
// Moatwalk's night: the moon's far halo, the far hills, the keep's lit stone, a pennant.
pub const MOON_HALO_FAR: u8 = flat(48);
pub const HILL_FAR: u8 = flat(49);
pub const KEEP_LIT: u8 = flat(50);
pub const PENNANT: u8 = flat(51);
pub const MIST_FAR: u8 = flat(52);

/// The sky table (`SKY` material, read through `uv` at 32 texels to a unit):
/// rows 0..192 are the skies' gradients, side by side in 64-column bands, one
/// per `Look`; rows 192..224 are the far land's baked colours in 8-by-8 blocks
/// (a row of blocks per `Look`, a block per brightness and haze); rows
/// 224..256 are flat specials. Blocks survive every coarser copy of the table.
pub const SKY_GRADIENT_ROWS: usize = 192;
pub const SKY_COLUMNS: usize = 64;
const FAR_ROW0: usize = 192;
const SPECIAL_ROW0: usize = 224;
/// Haze steps of the far land, as a mix toward the horizon.
const HAZE: [f32; 4] = [0.4, 0.68, 0.88, 0.97];
/// The far land per `Look`: shadowed face, lit face.
const FAR: [[Rgb; 2]; 4] = [
    [[26, 14, 20], [200, 112, 60]],
    [[7, 4, 5], [196, 86, 34]],
    [[12, 18, 38], [84, 116, 170]],
    [[4, 8, 14], [82, 108, 108]],
];
/// What the far land fades toward: the night's haze, warmed by the look's horizon.
const NIGHT_HAZE: Rgb = [5, 7, 13];
/// The raster's own night haze, which the places not yet given a haze of their own keep.
const NIGHT_HAZE_NEAR: Rgb = [9, 13, 24];
const HORIZON: [Rgb; 4] = [[190, 104, 64], [170, 64, 26], [40, 66, 110], [110, 130, 150]];

/// The haze each place fades into, near and far: the raster's fog takes the near world to it
/// and the far land's baked colours meet it, so the two never part.
pub fn haze_of(look: super::route::Look) -> Rgb {
    use super::route::Look as L;
    match look { L::Forge | L::Ash => [62, 24, 13], L::Deep => [26, 46, 68], L::Castle => [16, 28, 50], L::Dune => NIGHT_HAZE_NEAR }
}

fn centre(block_col: usize, block_row: usize) -> [f32; 2] { [(block_col * 8 + 4) as f32 / 32., (block_row * 8 + 4) as f32 / 32.] }

/// The far land's baked colour for a face of brightness step `bright` (0..8)
/// at haze step `haze` (0..4), in the palette of `look`'s row.
pub fn far_uv(look: usize, bright: usize, haze: usize) -> [f32; 2] { centre(bright.min(7) * 4 + haze.min(3), FAR_ROW0 / 8 + look) }

/// A flat special: 0 star, 1 moon, 2 cloud shadow, 3 lantern glow.
pub fn special_uv(kind: usize) -> [f32; 2] { let (c, r) = special_block(kind); centre(c, r) }

/// The block of the sky table (column, row, in blocks of 8) that holds special `kind`: 32 to a
/// row, four rows. The world's own lights are kinds 0 to 47; the pods' lights sit above them.
pub fn special_block(kind: usize) -> (usize, usize) { (kind % 32, SPECIAL_ROW0 / 8 + (kind / 32).min(3)) }

/// Where on the sky table a view ray at `elev` radians sits, for sky `sky`.
pub fn sky_uv(sky: usize, elev: f32, az: f32) -> [f32; 2] {
    let t = if elev <= 0. { 1. } else { 1. - (elev / 1.25).clamp(0., 1.).powf(0.7) };
    let row = t * (SKY_GRADIENT_ROWS - 1) as f32;
    let col = sky as f32 * SKY_COLUMNS as f32 + (az.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU) * (SKY_COLUMNS - 2) as f32 + 1.;
    [col / 32., (row + 0.5) / 32.]
}

type Rgb = [u8; 3];

/// The flat lights, by kind (see `flat`): 32 colours, each read from a block of the sky table.
const SPECIALS: [Rgb; 128] = [
    [236, 238, 252], [214, 222, 238], [14, 16, 30], [176, 76, 28],
    // Crystal: a body of deep teal, a tip of cool light; a lantern's flame and its glow on stone; a shaft of day, its pool.
    [58, 134, 164], [150, 222, 238], [226, 150, 62], [128, 100, 70],
    [150, 196, 220], [84, 128, 158], [50, 92, 130], [150, 92, 54],
    // The Foundry Run: embers, sparks, lava in steps, windows, seams, ash, smoke, the horizon, hazard bars, a beacon.
    [255, 176, 78], [196, 84, 32], [255, 236, 170], [196, 72, 24],
    [240, 118, 36], [124, 38, 18], [250, 176, 70], [168, 88, 36],
    [240, 124, 42], [104, 78, 68], [30, 18, 16], [208, 96, 40],
    [238, 190, 66], [20, 15, 14], [255, 70, 40], [255, 206, 110],
    // The Glassway: crystal light in cyan and violet, bright and dim, and the pools it lays on stone.
    [96, 228, 238], [44, 124, 156], [158, 104, 240], [96, 62, 176],
    // 32..: pools, a mote, the skyline's black, a moon's halo, torches, the moat's glints, spray, a lit sill.
    [30, 74, 98], [56, 40, 98], [120, 210, 226], [12, 18, 34],
    [70, 90, 128], [255, 196, 96], [228, 130, 52], [206, 144, 74],
    [196, 220, 244], [226, 196, 128], [150, 190, 214], [120, 80, 200],
    [40, 52, 82], [66, 150, 190], [170, 120, 255], [74, 56, 44],
    [34, 50, 88], [22, 36, 66], [92, 106, 150], [176, 46, 50], [58, 160, 214], [24, 96, 150], [126, 90, 226], [160, 232, 248],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
];

fn table(f: impl Fn(f32, f32) -> Rgb) -> Vec<Rgb> {
    (0..256 * 256).map(|i| f((i % 256) as f32 / 32., (i / 256) as f32 / 32.)).collect()
}

fn add(a: Rgb, b: Rgb, k: f32) -> Rgb { tex::add(a, b, k) }
fn mulc(c: Rgb, k: f32) -> Rgb { c.map(|v| (v as f32 * k).clamp(0., 255.) as u8) }
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb { [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8) }

/// Horizontal strata by `v`, irregular in thickness; `u` adds a fine grain.
fn strata(bands: &[Rgb], seed: u32, v_scale: f32) -> Vec<Rgb> {
    table(|u, v| {
        // `v` wraps every 8 units; a periodic value keeps the seam closed.
        let k = (v * v_scale).floor() as i32;
        let band = (hash(k.rem_euclid((8. * v_scale) as i32), 0, seed) * bands.len() as f32) as usize % bands.len();
        let edge = ((v * v_scale).fract() - 0.5).abs();
        let grain = value(u * 3., v * 9., seed) - 0.5;
        mulc(bands[band], 0.97 + 0.08 * grain + if edge > 0.47 { -0.07 } else { 0. })
    })
}

/// Coursed blocks, `w` by `h` world units.
fn blocks(base: Rgb, w: f32, h: f32, seed: u32) -> Vec<Rgb> {
    table(|u, v| {
        let row = (v / h).floor();
        let off = if (row as i32) % 2 == 0 { 0. } else { w * 0.5 };
        let col = ((u + off) / w).floor();
        let (fu, fv) = (((u + off) / w).fract(), (v / h).fract());
        let joint = fu < 0.05 || fv < 0.07;
        let tone = 0.82 + 0.3 * hash(col as i32, row as i32, seed);
        mulc(base, if joint { 0.55 } else { tone })
    })
}

/// Molten rock: a deep red crust, slow orange swells and thin bright cracks. The 256-texel
/// table covers eight world units a side, so every wave runs a whole number of times to tile
/// without a seam. The emissive gain (about 1.7) lifts it to a glow, so the table sits low.
fn lava() -> Vec<Rgb> {
    use std::f32::consts::TAU;
    table(|u, v| {
        let (a, b) = (u / 8. * TAU, v / 8. * TAU);
        // Warped waves, then the crack where two of them cross zero.
        let w1 = (2. * a + 1.6 * (b * 1.).sin()).sin();
        let w2 = (3. * b + 1.3 * (a * 2.).sin() + 1.).sin();
        let swell = 0.5 + 0.25 * (w1 + w2);
        let crack = (1. - (w1 * w2).abs() * 5.).clamp(0., 1.);
        let grain = value(u * 4., v * 4., 211) - 0.5;
        let base = mix([86, 22, 12], [138, 56, 20], swell.clamp(0., 1.));
        let hot = mix(base, [150, 98, 38], crack * 0.8);
        mulc(hot, 1. + 0.08 * grain)
    })
}

/// Moat water under night: a cold blue swell, with the moon's light caught on thin ripples.
/// Whole waves to the table's eight units, so it tiles.
fn water() -> Vec<Rgb> {
    use std::f32::consts::TAU;
    table(|u, v| {
        let (a, b) = (u / 8. * TAU, v / 8. * TAU);
        let swell = 0.5 + 0.25 * ((2. * a + 1.1 * (3. * b).sin()).sin() + (3. * b + 0.9 * (2. * a).sin()).sin());
        let ripple = ((6. * b + 2.2 * (4. * a).sin()).sin() * 0.5 + 0.5).powi(14);
        let base = mix([26, 46, 84], [44, 76, 120], swell.clamp(0., 1.));
        mix(base, [210, 232, 252], ripple * 0.9)
    })
}

fn plain(base: Rgb, seed: u32, amount: f32) -> Vec<Rgb> {
    table(|u, v| mulc(base, 1. + amount * (fbm(u * 2.2, v * 2.2, seed, 3) - 0.5) * 2.))
}

/// The sky: a column of gradient per `Look` (`u` in whole units picks the sky),
/// altitude down `v`; the last rows hold flat specials in 8-by-8 blocks so even
/// the coarsest copy keeps them.
fn sky_table() -> Vec<Rgb> {
    // Skies, zenith to horizon, as (altitude fraction, colour). Dark indigo, one warm band.
    const SKIES: [&[(f32, Rgb)]; 4] = [
        // Dune: dusk, a low ochre glow (stops sit on the realm's inks, so the bands are clean)
        &[(0.0, [25, 41, 52]), (0.45, [47, 74, 82]), (0.64, [69, 96, 112]), (0.76, [104, 70, 41]), (0.9, [169, 118, 57]), (1.0, [216, 169, 94])],
        // Forge: smoke over furnace light. Dark red zenith, a lit underside of smoke, a hot rim.
        &[(0.0, [10, 5, 6]), (0.28, [22, 10, 9]), (0.52, [44, 18, 12]), (0.70, [84, 34, 16]), (0.84, [170, 74, 26]), (0.94, [230, 130, 50]), (1.0, [252, 184, 90])],
        // Deep road: black, a faint blue glow from below
        &[(0.0, [6, 10, 20]), (0.5, [12, 22, 38]), (0.8, [22, 44, 70]), (1.0, [48, 92, 124])],
        // Castle: night. Near black over head, a deep blue, and the moon's silver where the sky meets the land.
        &[(0.0, [8, 3, 24]), (0.45, [22, 10, 58]), (0.6, [46, 16, 100]), (0.72, [52, 22, 112]), (0.8, [38, 36, 112]), (0.88, [40, 70, 124]), (0.95, [92, 128, 170]), (1.0, [150, 178, 212])],
    ];

    table(|u, v| {
        let (col, row) = ((u * 32.) as usize, (v * 32.) as usize);
        if row >= SPECIAL_ROW0 { return SPECIALS[((row - SPECIAL_ROW0) / 8).min(3) * 32 + (col / 8).min(31)]; }
        if row >= FAR_ROW0 {
            let look = (row - FAR_ROW0) / 8;
            let (bright, haze) = ((col / 8) / 4, (col / 8) % 4);
            let [dark, lit] = FAR[look.min(3)];
            let face = mix(dark, lit, bright as f32 / 7.);
            let far_haze = match look { 1 => mix(haze_of(super::route::Look::Forge), HORIZON[1], 0.35), 2 => mix(haze_of(super::route::Look::Deep), HORIZON[2], 0.2), 3 => mix(haze_of(super::route::Look::Castle), HORIZON[3], 0.25), _ => mix(NIGHT_HAZE, HORIZON[look.min(3)], 0.06) };
            return mix(face, far_haze, HAZE[haze]);
        }
        let sky = (col / SKY_COLUMNS).min(3);
        let t = row as f32 / (SKY_GRADIENT_ROWS - 1) as f32;
        let stops = SKIES[sky];
        let mut c = stops[stops.len() - 1].1;
        for w in stops.windows(2) {
            if t <= w[1].0 {
                c = mix(w[0].1, w[1].1, ((t - w[0].0) / (w[1].0 - w[0].0)).clamp(0., 1.));
                break;
            }
        }
        if sky == 1 {
            // Smoke over the forge: plumes that go black overhead, and thin out and catch the
            // furnace light near the horizon, where the glow swells and fades round the compass.
            let az = ((col % SKY_COLUMNS).max(1) - 1) as f32 / (SKY_COLUMNS - 2) as f32 * std::f32::consts::TAU;
            let big = tex::ring_noise(az, t * 2.6, 41, 1.7);
            let fine = tex::ring_noise(az, t * 8., 47, 4.2);
            let dens = (big * 1.5 + fine * 0.5 - 0.62).clamp(0., 1.);
            let thin = ((t - 0.74) / 0.26).clamp(0., 1.);
            let swell = 0.65 + 0.7 * tex::ring_noise(az, 0.3, 55, 1.3);
            let lit = mix(c, [236, 118, 44], thin * thin * 0.55 * (1. - dens * 0.6) * swell);
            return mulc(lit, 1. - dens * (0.7 - 0.45 * thin));
        }
        if sky == 3 {
            // Night cloud in long streaks, silver along the edge the moon lights.
            let az = ((col % SKY_COLUMNS).max(1) - 1) as f32 / (SKY_COLUMNS - 2) as f32 * std::f32::consts::TAU;
            let big = tex::ring_noise(az, t * 3., 63, 2.1);
            let dens = (big * 1.5 - 0.62).clamp(0., 1.);
            let rim = (dens * (1. - dens) * 4.).clamp(0., 1.);
            return add(mulc(c, 1. - 0.28 * dens), [70, 92, 130], rim * 0.3 * (0.4 + t));
        }
        // Streaks of cloud, dim.
        let cloud = fbm(col as f32 * 0.09, row as f32 * 0.35, 5 + sky as u32, 3);
        mulc(c, 0.92 + 0.2 * (cloud - 0.5) * 2. * (0.3 + t))
    })
}

pub fn painted() -> &'static Painted {
    static PALETTE: OnceLock<Painted> = OnceLock::new();
    PALETTE.get_or_init(|| {
        let mut p = Painted::new();
        // The pods' own materials and the original sandstone, exactly as they were.
        for (material, base) in [
            (mat::BRASS, [174i32, 113, 73]),
            (mat::PATH, [139, 119, 93]),
            (mat::STONE, [118, 108, 96]),
            (IRON, [133, 151, 163]),
            (GUN, [89, 109, 127]),
            (STEEL, [199, 210, 218]),
        ] {
            let t = (0..256 * 256).map(|i| {
                let (x, y) = (i % 256, i / 256);
                let grain = ((x / 8 * 13 + y / 8 * 7) % 9) as i32 - 4;
                let seam = if material == mat::BRASS && y % 64 < 2 { -24 } else { 0 };
                base.map(|v| (v + grain + seam).clamp(0, 255) as u8)
            }).collect();
            p.set(material, t);
        }
        p.set(SAND, plain([176, 124, 74], 21, 0.06));
        p.set(MESA, strata(&[[158, 96, 62], [122, 74, 52], [94, 60, 46], [176, 118, 76], [108, 66, 50]], 31, 1.0));
        // Forge rock: dark, a little warm, so the lava and the furnace mouths are the one warm accent.
        p.set(BASALT, strata(&[[62, 38, 28], [48, 32, 28], [74, 40, 28], [38, 26, 24], [92, 52, 34]], 41, 1.0));
        p.set(ASH, plain([52, 36, 32], 51, 0.07));
        p.set(LAVA, tex::lava([66, 16, 8], [132, 40, 12], [198, 84, 24], 0., 0., 211));
        p.set(WATER, water());
        // The Deep Road's rock: blue-black, but with enough in it to take a lantern's light.
        p.set(CAVE, strata(&[[38, 54, 86], [32, 46, 78], [52, 70, 108], [26, 38, 66], [62, 60, 84]], 61, 0.8));
        p.set(MOOR, plain([30, 52, 40], 71, 0.07));
        p.set(DEEP_A, blocks([104, 130, 176], 2.0, 1.0, 141));
        p.set(DEEP_B, blocks([78, 100, 146], 2.0, 1.0, 151));
        p.set(MASONRY, blocks([122, 132, 150], 1.4, 0.7, 81));
        p.set(SLATE, blocks([60, 66, 92], 0.8, 0.5, 91));
        p.set(TIMBER, strata(&[[106, 74, 50], [86, 58, 40], [120, 84, 56]], 101, 4.0));
        p.set(PLATE, blocks([100, 108, 132], 2.0, 1.0, 111));
        p.set(PLATE_B, blocks([74, 80, 102], 2.0, 1.0, 121));
        p.set(CLOTH, plain([58, 92, 170], 131, 0.1));
        p.set(SKY, sky_table());
        emberfall(&mut p);
        glassway(&mut p);
        moatwalk(&mut p);
        p
    })
}

/// The Foundry Run's own surfaces.
fn emberfall(p: &mut Painted) {
    // The road: warm basalt plates, joints near black. Two tones, laid in 8 m plates by `road`.
    p.set(EF_ROAD_A, super::roadtex::ember_road(false));
    p.set(EF_ROAD_B, tex::plates([23, 19, 18], [7, 5, 5], 2.0, 1.0, 311, 0.16, 0.3));
    p.set(EF_ROAD_HOT, super::roadtex::ember_road(true));
    p.set(EF_RAIL, tex::iron([44, 24, 16], [14, 7, 5], [200, 104, 48], 331));
    p.set(EF_IRON, tex::iron([36, 20, 14], [11, 6, 4], [180, 96, 46], 341));
    p.set(EF_BRICK, tex::brick([104, 52, 36], [30, 18, 16], 0.5, 0.25, 351));
    p.set(EF_WALL, tex::ember_wall(&[[34, 17, 11], [26, 13, 9], [42, 20, 12], [22, 11, 8], [50, 24, 14]], [255, 112, 38], [255, 176, 72], 361));
    p.set(EF_ASH, super::roadtex::cinder_ground());
    p.set(EF_CRUST, tex::crust([34, 24, 24], [190, 70, 26], 381));
    p.set(EF_KERB, tex::plates([30, 14, 9], [8, 4, 3], 0.5, 0.5, 391, 0.1, 0.2));
    p.set(EF_BARS, tex::bars([232, 186, 64], [26, 20, 18], 0.5));
    p.set(EF_SOOT, tex::iron([28, 16, 12], [10, 6, 5], [150, 84, 44], 401));
    // Fire: the lamp and window orange, hot at the heart. Pods' engines wear it too.
    p.set(GLOW, tex::lava([170, 78, 26], [222, 128, 44], [250, 188, 80], 0.3, 0.6, 411));
}

/// The Glassway's own surfaces: stone polished to a dark blue, rock that takes crystal light.
fn glassway(p: &mut Painted) {
    p.set(GL_ROAD_A, super::roadtex_glass::glass_road(false));
    p.set(GL_ROAD_B, super::roadtex_glass::glass_road(true));
    p.set(GL_RAIL, tex::iron([30, 46, 72], [10, 16, 28], [96, 190, 214], 521));
    p.set(GL_WALL, tex::ember_wall(&[[16, 24, 42], [12, 20, 36], [20, 30, 52], [10, 16, 32], [24, 36, 60]], [70, 190, 230], [190, 150, 255], 531));
    p.set(GL_FLOOR, tex::flecked([18, 28, 46], [120, 226, 244], 0.014, 541, 0.18));
    p.set(GL_BLOCK, tex::brick([46, 66, 100], [12, 18, 32], 0.5, 0.25, 551));
    p.set(GL_KERB, tex::plates([16, 24, 40], [6, 10, 18], 0.5, 0.5, 561, 0.1, 0.2));
    p.set(GL_MIST, super::roadtex_glass::mist());
}

/// Moatwalk by night: cool flag and cobble, a parapet of moonlit stone, a castle of ashlar.
fn moatwalk(p: &mut Painted) {
    p.set(MW_ROAD_A, super::roadtex_moat::cobble_road(false));
    p.set(MW_ROAD_B, super::roadtex_moat::cobble_road(true));
    p.set(MW_PARAPET, tex::brick([120, 134, 172], [30, 36, 56], 0.5, 0.25, 621));
    p.set(MW_WALL, tex::brick([104, 118, 156], [28, 34, 52], 0.7, 0.35, 631));
    p.set(MW_GROUND, tex::flecked([20, 34, 34], [90, 120, 120], 0.01, 641, 0.2));
    p.set(MW_ROOF, tex::brick([34, 42, 72], [16, 20, 34], 0.4, 0.2, 651));
    p.set(MW_TIMBER, tex::iron([62, 44, 34], [24, 16, 12], [150, 110, 70], 661));
    p.set(MW_IRON, tex::iron([36, 40, 54], [12, 14, 22], [110, 120, 150], 671));
    p.set(MW_KERB, tex::plates([36, 42, 62], [12, 14, 24], 0.5, 0.5, 681, 0.1, 0.2));
    p.set(MW_BANNER, tex::plates([150, 40, 44], [90, 24, 28], 0.25, 0.5, 691, 0.1, 0.2));
    p.set(MW_BANNER_B, tex::plates([48, 84, 176], [30, 56, 120], 0.25, 0.5, 701, 0.1, 0.2));
}
