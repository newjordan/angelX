//! The race's painted materials: a bounded bank laid over the shared one.
//! Every pod material keeps the look it had; the land gets strata, ash,
//! masonry and sky, each on a material the pods do not wear.
#![allow(dead_code)]
use super::noise::{fbm, hash, value};
use super::tex;
use crate::render::world3d::mesh::mat;
use crate::render::world3d::raster::Painted;
use std::sync::OnceLock;

pub const IRON: u8 = mat::STONE_DARK;
pub const GUN: u8 = mat::HORSE_GREY;
pub const STEEL: u8 = mat::ARMOR;

pub const ROAD_A: u8 = mat::PATH;
pub const ROAD_B: u8 = mat::STONE;
pub const SAND: u8 = mat::HAY;
pub const MESA: u8 = mat::ROCK;
pub const BASALT: u8 = mat::STABLE_WALL;
pub const ASH: u8 = mat::STABLE_FLOOR;
pub const CAVE: u8 = mat::SCULPTED_STONE;
pub const MOOR: u8 = mat::GRASS;
pub const MASONRY: u8 = mat::STABLE_ROOF;
pub const SLATE: u8 = mat::ROOF;
pub const TIMBER: u8 = mat::STABLE_TIMBER;
pub const PLATE: u8 = mat::FLOOR;
pub const PLATE_B: u8 = mat::TACK_LEATHER;
pub const DEEP_A: u8 = mat::PARCHMENT;
pub const DEEP_B: u8 = mat::BOOKS;
pub const CLOTH: u8 = mat::CLOTH_BLUE;
pub const CLOTH_R: u8 = mat::BANNER;
pub const SKY: u8 = mat::WINDOW;
pub const GLOW: u8 = mat::FIRE;
pub const LAVA: u8 = mat::EMBER;
pub const LAVA_HOT: u8 = mat::FLAME;
pub const CRYSTAL: u8 = mat::CRYSTAL;
pub const WATER: u8 = mat::WATER;

pub const EF_ROAD_A: u8 = 40;
pub const EF_ROAD_B: u8 = 41;
pub const EF_ROAD_HOT: u8 = 42;
pub const EF_RAIL: u8 = 43;
pub const EF_IRON: u8 = 44;
pub const EF_BRICK: u8 = 45;
pub const EF_WALL: u8 = 46;
pub const EF_ASH: u8 = 47;
pub const EF_CRUST: u8 = 48;
pub const EF_KERB: u8 = 49;
pub const EF_BARS: u8 = 50;
pub const EF_SOOT: u8 = 51;
pub const GL_ROAD_A: u8 = 52;
pub const GL_ROAD_B: u8 = 53;
pub const GL_RAIL: u8 = 54;
pub const GL_WALL: u8 = 55;
pub const GL_FLOOR: u8 = 56;
pub const GL_BLOCK: u8 = 57;
pub const GL_KERB: u8 = 58;
pub const GL_MIST: u8 = 59;
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
pub const WA_ROAD_A: u8 = 71;
pub const WA_ROAD_B: u8 = 72;
pub const WA_RAIL: u8 = 73;
pub const WA_CAP: u8 = 74;
pub const WA_POST: u8 = 75;
pub const WA_ROCK: u8 = 76;
pub const WA_GROUND: u8 = 77;
pub const WA_KERB: u8 = 78;
pub const WA_BONE: u8 = 79;
pub const WA_WASH: u8 = 80;
pub const WA_BANNER: u8 = 81;
pub const WA_BANNER_B: u8 = 82;
pub const WA_PLATE: u8 = 83;
pub const WA_ASHLAR: u8 = 84;
pub const WA_SLATE: u8 = 85;
pub const WA_TIMBER: u8 = 86;
pub const WA_BARS: u8 = 87;

pub const fn flat(kind: u8) -> u8 { 128 + kind }
pub fn flat_kind(mat: u8) -> Option<usize> { (mat >= 128).then(|| (mat - 128) as usize) }
pub const CRYSTAL_BODY: u8 = flat(4);
pub const CRYSTAL_TIP: u8 = flat(5);
pub const LANTERN: u8 = flat(6);
pub const LANTERN_POOL: u8 = flat(7);
pub const SHAFT: u8 = flat(8);
pub const SHAFT_POOL: u8 = flat(9);
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
pub const MOON_HALO_FAR: u8 = flat(48);
pub const HILL_FAR: u8 = flat(49);
pub const KEEP_LIT: u8 = flat(50);
pub const PENNANT: u8 = flat(51);
pub const MIST_FAR: u8 = flat(52);
pub const WA_LANTERN: u8 = flat(56);
pub const WA_LANTERN_DIM: u8 = flat(57);
pub const WA_POOL: u8 = flat(58);
pub const WA_FAR: u8 = flat(59);
pub const WA_FAR_KIND: usize = 59;
pub const WA_MOON: u8 = flat(60);
pub const WA_HALO_IN: u8 = flat(61);
pub const WA_HALO_OUT: u8 = flat(62);
pub const WA_CLOUD: u8 = flat(63);
pub const WASH_SKY: usize = 96;
pub const WASH_FAR: usize = 112;
pub const WASH_RINGS: [f32; 17] = [-0.16, 0.0, 0.02, 0.045, 0.07, 0.10, 0.14, 0.19, 0.25, 0.32, 0.41, 0.52, 0.66, 0.84, 1.05, 1.3, 1.5];

pub const SKY_GRADIENT_ROWS: usize = 192;
pub const SKY_COLUMNS: usize = 64;
const FAR_ROW0: usize = 192;
const SPECIAL_ROW0: usize = 224;
const HAZE: [f32; 4] = [0.4, 0.68, 0.88, 0.97];
const FAR: [[Rgb; 2]; 4] = [
    [[26, 14, 20], [200, 112, 60]],
    [[7, 4, 5], [196, 86, 34]],
    [[12, 18, 38], [84, 116, 170]],
    [[4, 8, 14], [82, 108, 108]],
];
const NIGHT_HAZE: Rgb = [5, 7, 13];
const NIGHT_HAZE_NEAR: Rgb = [9, 13, 24];
const HORIZON: [Rgb; 4] = [[190, 104, 64], [170, 64, 26], [40, 66, 110], [110, 130, 150]];
const WASH_HAZE: Rgb = [38, 60, 96];
const WASH_HORIZON: Rgb = [146, 178, 212];
const WASH_FAR_FACES: [Rgb; 2] = [[5, 9, 20], [122, 152, 204]];

pub fn haze_of(look: super::route::Look) -> Rgb {
    use super::route::Look as L;
    match look { L::Forge | L::Ash => [62, 24, 13], L::Deep => [26, 46, 68], L::Castle => [16, 28, 50], L::Wash => WASH_HAZE, L::Dune => NIGHT_HAZE_NEAR }
}

fn centre(block_col: usize, block_row: usize) -> [f32; 2] { [(block_col * 8 + 4) as f32 / 32., (block_row * 8 + 4) as f32 / 32.] }

pub fn far_uv(look: usize, bright: usize, haze: usize) -> [f32; 2] {
    if look >= 4 { return special_uv(WASH_FAR + (bright.min(7) / 2) * 4 + haze.min(3)); }
    centre(bright.min(7) * 4 + haze.min(3), FAR_ROW0 / 8 + look)
}

pub fn special_uv(kind: usize) -> [f32; 2] { let (c, r) = special_block(kind); centre(c, r) }

pub fn special_block(kind: usize) -> (usize, usize) { (kind % 32, SPECIAL_ROW0 / 8 + (kind / 32).min(3)) }

pub fn sky_uv(sky: usize, elev: f32, az: f32) -> [f32; 2] {
    let t = if elev <= 0. { 1. } else { 1. - (elev / 1.25).clamp(0., 1.).powf(0.7) };
    let row = t * (SKY_GRADIENT_ROWS - 1) as f32;
    let col = sky as f32 * SKY_COLUMNS as f32 + (az.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU) * (SKY_COLUMNS - 2) as f32 + 1.;
    [col / 32., (row + 0.5) / 32.]
}

type Rgb = [u8; 3];

const SPECIALS: [Rgb; 128] = [
    [236, 238, 252], [214, 222, 238], [14, 16, 30], [176, 76, 28],
    [58, 134, 164], [150, 222, 238], [226, 150, 62], [128, 100, 70],
    [150, 196, 220], [84, 128, 158], [50, 92, 130], [150, 92, 54],
    [255, 176, 78], [196, 84, 32], [255, 236, 170], [196, 72, 24],
    [240, 118, 36], [124, 38, 18], [250, 176, 70], [168, 88, 36],
    [240, 124, 42], [104, 78, 68], [30, 18, 16], [208, 96, 40],
    [238, 190, 66], [20, 15, 14], [255, 70, 40], [255, 206, 110],
    [96, 228, 238], [44, 124, 156], [158, 104, 240], [96, 62, 176],
    [30, 74, 98], [56, 40, 98], [120, 210, 226], [12, 18, 34],
    [70, 90, 128], [255, 196, 96], [228, 130, 52], [206, 144, 74],
    [196, 220, 244], [226, 196, 128], [150, 190, 214], [120, 80, 200],
    [40, 52, 82], [66, 150, 190], [170, 120, 255], [74, 56, 44],
    [34, 50, 88], [22, 36, 66], [92, 106, 150], [176, 46, 50], [58, 160, 214], [24, 96, 150], [126, 90, 226], [160, 232, 248],
    [196, 232, 255], [92, 140, 200], [118, 156, 212], [74, 108, 156], [236, 242, 255], [150, 204, 240], [84, 134, 188], [178, 202, 232],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
];

fn wash_sky_at(e: f32) -> Rgb {
    const STOPS: [(f32, Rgb); 9] = [
        (0.00, [176, 204, 230]), (0.03, [146, 182, 218]), (0.07, [106, 148, 198]), (0.12, [66, 102, 160]), (0.19, [34, 58, 112]),
        (0.28, [16, 30, 72]), (0.42, [8, 15, 40]), (0.75, [4, 8, 24]), (1.30, [2, 4, 14]),
    ];
    let e = e.max(0.);
    for w in STOPS.windows(2) {
        if e <= w[1].0 { return mix(w[0].1, w[1].1, ((e - w[0].0) / (w[1].0 - w[0].0)).clamp(0., 1.)); }
    }
    STOPS[STOPS.len() - 1].1
}

fn wash_special(kind: usize) -> Rgb {
    if kind < WASH_FAR {
        let k = (kind - WASH_SKY).min(WASH_RINGS.len() - 2);
        wash_sky_at((WASH_RINGS[k] + WASH_RINGS[k + 1]) * 0.5)
    } else {
        let (bright, haze) = ((kind - WASH_FAR) / 4, (kind - WASH_FAR) % 4);
        let face = mix(WASH_FAR_FACES[0], WASH_FAR_FACES[1], bright as f32 / 3.);
        mix(face, mix(WASH_HAZE, WASH_HORIZON, 0.22), HAZE[haze])
    }
}

fn table(f: impl Fn(f32, f32) -> Rgb) -> Vec<Rgb> {
    (0..256 * 256).map(|i| f((i % 256) as f32 / 32., (i / 256) as f32 / 32.)).collect()
}

fn add(a: Rgb, b: Rgb, k: f32) -> Rgb { tex::add(a, b, k) }
fn mulc(c: Rgb, k: f32) -> Rgb { c.map(|v| (v as f32 * k).clamp(0., 255.) as u8) }
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb { [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8) }

fn strata(bands: &[Rgb], seed: u32, v_scale: f32) -> Vec<Rgb> {
    table(|u, v| {
        let k = (v * v_scale).floor() as i32;
        let band = (hash(k.rem_euclid((8. * v_scale) as i32), 0, seed) * bands.len() as f32) as usize % bands.len();
        let edge = ((v * v_scale).fract() - 0.5).abs();
        let grain = value(u * 3., v * 9., seed) - 0.5;
        mulc(bands[band], 0.97 + 0.08 * grain + if edge > 0.47 { -0.07 } else { 0. })
    })
}

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

fn lava() -> Vec<Rgb> {
    use std::f32::consts::TAU;
    table(|u, v| {
        let (a, b) = (u / 8. * TAU, v / 8. * TAU);
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

fn wash() -> Vec<Rgb> {
    use std::f32::consts::TAU;
    table(|u, v| {
        let (a, b) = (u / 8. * TAU, v / 8. * TAU);
        let swell = 0.5 + 0.25 * ((2. * a + 1.2 * (3. * b).sin()).sin() + (3. * b + 0.8 * (2. * a).sin()).sin());
        let film = (0.5 + 0.5 * (a + 1.6 * (2. * b).sin() + 0.7).sin() * (b - 2. * a).cos()).clamp(0., 1.);
        let ripple = ((7. * b + 2.4 * (4. * a).sin()).sin() * 0.5 + 0.5).powi(16);
        let base = mix([52, 84, 132], [92, 132, 184], swell.clamp(0., 1.));
        let milky = mix(base, [184, 212, 240], film * film * 0.65);
        mix(milky, [236, 246, 255], ripple * 0.85)
    })
}

fn bone() -> Vec<Rgb> {
    table(|u, v| {
        let grain = fbm(u * 3., v * 1.2, 861, 3);
        let line = ((v * 6.).sin() * 0.5 + 0.5).powi(6);
        mulc(mix([204, 216, 230], [238, 244, 252], grain), 0.92 + 0.1 * line)
    })
}

fn plain(base: Rgb, seed: u32, amount: f32) -> Vec<Rgb> {
    table(|u, v| mulc(base, 1. + amount * (fbm(u * 2.2, v * 2.2, seed, 3) - 0.5) * 2.))
}

fn sky_table() -> Vec<Rgb> {
    const SKIES: [&[(f32, Rgb)]; 4] = [
        &[(0.0, [25, 41, 52]), (0.45, [47, 74, 82]), (0.64, [69, 96, 112]), (0.76, [104, 70, 41]), (0.9, [169, 118, 57]), (1.0, [216, 169, 94])],
        &[(0.0, [10, 5, 6]), (0.28, [22, 10, 9]), (0.52, [44, 18, 12]), (0.70, [84, 34, 16]), (0.84, [170, 74, 26]), (0.94, [230, 130, 50]), (1.0, [252, 184, 90])],
        &[(0.0, [6, 10, 20]), (0.5, [12, 22, 38]), (0.8, [22, 44, 70]), (1.0, [48, 92, 124])],
        &[(0.0, [8, 3, 24]), (0.45, [22, 10, 58]), (0.6, [46, 16, 100]), (0.72, [52, 22, 112]), (0.8, [38, 36, 112]), (0.88, [40, 70, 124]), (0.95, [92, 128, 170]), (1.0, [150, 178, 212])],
    ];

    table(|u, v| {
        let (col, row) = ((u * 32.) as usize, (v * 32.) as usize);
        if row >= SPECIAL_ROW0 {
            let kind = ((row - SPECIAL_ROW0) / 8).min(3) * 32 + (col / 8).min(31);
            return if kind >= WASH_SKY { wash_special(kind) } else { SPECIALS[kind] };
        }
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
            let az = ((col % SKY_COLUMNS).max(1) - 1) as f32 / (SKY_COLUMNS - 2) as f32 * std::f32::consts::TAU;
            let big = tex::ring_noise(az, t * 3., 63, 2.1);
            let dens = (big * 1.5 - 0.62).clamp(0., 1.);
            let rim = (dens * (1. - dens) * 4.).clamp(0., 1.);
            return add(mulc(c, 1. - 0.28 * dens), [70, 92, 130], rim * 0.3 * (0.4 + t));
        }
        let cloud = fbm(col as f32 * 0.09, row as f32 * 0.35, 5 + sky as u32, 3);
        mulc(c, 0.92 + 0.2 * (cloud - 0.5) * 2. * (0.3 + t))
    })
}

pub fn painted() -> &'static Painted {
    static PALETTE: OnceLock<Painted> = OnceLock::new();
    PALETTE.get_or_init(|| {
        let mut p = Painted::new();
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
        p.set(BASALT, strata(&[[62, 38, 28], [48, 32, 28], [74, 40, 28], [38, 26, 24], [92, 52, 34]], 41, 1.0));
        p.set(ASH, plain([52, 36, 32], 51, 0.07));
        p.set(LAVA, tex::lava([66, 16, 8], [132, 40, 12], [198, 84, 24], 0., 0., 211));
        p.set(WATER, water());
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
        moonward(&mut p);
        p
    })
}

fn emberfall(p: &mut Painted) {
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
    p.set(GLOW, tex::lava([170, 78, 26], [222, 128, 44], [250, 188, 80], 0.3, 0.6, 411));
}

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

fn moonward(p: &mut Painted) {
    p.set(WA_ROAD_A, super::roadtex_wash::frost_road(false));
    p.set(WA_ROAD_B, super::roadtex_wash::frost_road(true));
    p.set(WA_RAIL, tex::brick([104, 124, 160], [28, 38, 64], 0.9, 0.3, 801));
    p.set(WA_CAP, tex::plates([176, 194, 220], [70, 86, 116], 0.8, 0.5, 811, 0.1, 0.2));
    p.set(WA_POST, tex::iron([40, 52, 76], [14, 20, 34], [140, 170, 210], 821));
    p.set(WA_ROCK, strata(&[[58, 76, 108], [46, 62, 92], [72, 90, 122], [40, 54, 82], [84, 100, 130]], 831, 0.8));
    p.set(WA_GROUND, tex::flecked([46, 64, 92], [176, 200, 230], 0.012, 841, 0.2));
    p.set(WA_KERB, tex::plates([156, 176, 206], [44, 58, 88], 0.5, 0.5, 851, 0.1, 0.2));
    p.set(WA_BONE, bone());
    p.set(WA_WASH, wash());
    p.set(WA_BANNER, tex::plates([170, 196, 230], [110, 138, 180], 0.25, 0.5, 871, 0.1, 0.2));
    p.set(WA_BANNER_B, tex::plates([30, 46, 112], [18, 28, 76], 0.25, 0.5, 881, 0.1, 0.2));
    p.set(WA_PLATE, tex::iron([52, 66, 96], [16, 22, 40], [150, 176, 212], 891));
    p.set(WA_ASHLAR, tex::brick([136, 154, 186], [40, 52, 80], 0.7, 0.35, 901));
    p.set(WA_SLATE, tex::brick([40, 52, 84], [16, 22, 38], 0.4, 0.2, 911));
    p.set(WA_TIMBER, strata(&[[104, 112, 128], [88, 98, 116], [118, 126, 142]], 921, 4.0));
    p.set(WA_BARS, tex::bars([196, 214, 236], [22, 32, 64], 0.5));
}
