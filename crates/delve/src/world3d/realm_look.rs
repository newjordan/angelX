//! The realm's look for a view (docs/REALM-MONOLITH.md, "The direction"): one
//! clean, nearly monotone palette a biome, dark to light, the biomes nearest
//! the view as zones with wide feathers (walking over a border slides the
//! whole palette from one to the next, never a seam), a low sun with long
//! shadows, the water's ramp, embers where lamps light (light = activity),
//! and the knight kept in sight through whatever stands before it. The
//! window and a page across a wire make the same look of the same view.

use angel_stage::look::{Dither, Lamp, Look, Reveal, Shadow, Sun, Zone};
use angel_stage::set::SetLamp;

use super::chunks::CHUNK;
use crate::world::biome::{self, Biome};
use crate::world::map::{CHUNKS_X, CHUNKS_Y};

pub const CASTLE_TOWN: [[u8; 3]; 8] = [
    [0x0c, 0x0e, 0x14],
    [0x14, 0x17, 0x22],
    [0x1d, 0x20, 0x32],
    [0x26, 0x2a, 0x45],
    [0x32, 0x37, 0x5d],
    [0x42, 0x47, 0x75],
    [0x56, 0x5a, 0x8f],
    [0x7d, 0x7f, 0xa6],
];
pub const GREENWOOD: [[u8; 3]; 8] = [
    [0x0d, 0x13, 0x10],
    [0x15, 0x21, 0x1b],
    [0x1f, 0x30, 0x27],
    [0x28, 0x43, 0x36],
    [0x36, 0x59, 0x49],
    [0x47, 0x70, 0x5e],
    [0x5b, 0x8a, 0x77],
    [0x81, 0xa2, 0x95],
];
pub const HIGHLANDS: [[u8; 3]; 8] = [
    [0x0e, 0x11, 0x12],
    [0x16, 0x1c, 0x20],
    [0x20, 0x29, 0x2f],
    [0x2b, 0x37, 0x40],
    [0x39, 0x49, 0x56],
    [0x4b, 0x5c, 0x6c],
    [0x5f, 0x73, 0x86],
    [0x84, 0x91, 0x9f],
];
pub const WASTES: [[u8; 3]; 8] = [
    [0x12, 0x0f, 0x0e],
    [0x1f, 0x18, 0x17],
    [0x2e, 0x24, 0x21],
    [0x40, 0x31, 0x2b],
    [0x55, 0x42, 0x3a],
    [0x6b, 0x55, 0x4c],
    [0x85, 0x6c, 0x60],
    [0x9e, 0x8e, 0x85],
];
pub const COAST: [[u8; 3]; 8] = [
    [0x0d, 0x13, 0x13],
    [0x16, 0x20, 0x20],
    [0x1f, 0x2f, 0x30],
    [0x2a, 0x3f, 0x41],
    [0x38, 0x54, 0x57],
    [0x49, 0x69, 0x6e],
    [0x5d, 0x81, 0x88],
    [0x83, 0x9b, 0xa0],
];
pub const FEN: [[u8; 3]; 8] = [
    [0x12, 0x13, 0x0d],
    [0x1f, 0x20, 0x16],
    [0x2d, 0x2f, 0x20],
    [0x3d, 0x41, 0x2a],
    [0x51, 0x57, 0x38],
    [0x66, 0x6d, 0x4a],
    [0x7e, 0x87, 0x5e],
    [0x98, 0xa0, 0x83],
];
pub const TILT_YARDS: [[u8; 3]; 8] = [
    [0x13, 0x10, 0x0d],
    [0x20, 0x1c, 0x16],
    [0x30, 0x2a, 0x1f],
    [0x42, 0x39, 0x29],
    [0x58, 0x4d, 0x37],
    [0x6f, 0x64, 0x48],
    [0x89, 0x7d, 0x5c],
    [0xa1, 0x99, 0x82],
];
pub const FAIRGROUND: [[u8; 3]; 8] = [
    [0x12, 0x0e, 0x12],
    [0x1f, 0x17, 0x1f],
    [0x2e, 0x21, 0x2d],
    [0x3f, 0x2c, 0x3e],
    [0x54, 0x3b, 0x52],
    [0x6a, 0x4d, 0x67],
    [0x84, 0x61, 0x7f],
    [0x9d, 0x86, 0x99],
];
pub const WIND_COUNTRY: [[u8; 3]; 8] = [
    [0x0f, 0x12, 0x0e],
    [0x19, 0x1e, 0x18],
    [0x24, 0x2d, 0x22],
    [0x30, 0x3d, 0x2e],
    [0x40, 0x52, 0x3d],
    [0x52, 0x68, 0x4f],
    [0x67, 0x80, 0x65],
    [0x89, 0x9b, 0x88],
];
pub const LONGBOW: [[u8; 3]; 8] = [
    [0x11, 0x13, 0x0d],
    [0x1c, 0x20, 0x16],
    [0x29, 0x30, 0x1f],
    [0x37, 0x41, 0x2a],
    [0x48, 0x57, 0x38],
    [0x5b, 0x6e, 0x49],
    [0x71, 0x88, 0x5d],
    [0x90, 0xa0, 0x83],
];
pub const MARCH: [[u8; 3]; 8] = [
    [0x12, 0x0e, 0x0f],
    [0x1f, 0x17, 0x19],
    [0x2e, 0x21, 0x23],
    [0x40, 0x2b, 0x2f],
    [0x55, 0x3a, 0x3e],
    [0x6b, 0x4c, 0x50],
    [0x85, 0x60, 0x64],
    [0x9e, 0x85, 0x87],
];

pub const WATER: [[u8; 3]; 8] = [
    [0x0b, 0x12, 0x16],
    [0x12, 0x1e, 0x25],
    [0x1a, 0x2b, 0x35],
    [0x24, 0x3a, 0x46],
    [0x31, 0x4c, 0x59],
    [0x42, 0x61, 0x6d],
    [0x5b, 0x7c, 0x85],
    [0x82, 0x9f, 0xa3],
];

pub const EMBER: [[u8; 3]; 8] = [
    [0x17, 0x10, 0x0e],
    [0x29, 0x19, 0x12],
    [0x41, 0x25, 0x17],
    [0x5d, 0x33, 0x1d],
    [0x7f, 0x45, 0x23],
    [0xa3, 0x5e, 0x2b],
    [0xc6, 0x83, 0x3f],
    [0xe6, 0xb2, 0x66],
];

pub const GLINT: [u8; 3] = [0xd9, 0xd6, 0xb8];
pub const WINDOW: [u8; 3] = [0xee, 0xae, 0x55];
pub const HILLSIDE: [[u8; 3]; 8] = [
    [0x0e, 0x13, 0x0c],
    [0x18, 0x22, 0x14],
    [0x24, 0x33, 0x1d],
    [0x31, 0x46, 0x27],
    [0x41, 0x5c, 0x33],
    [0x55, 0x73, 0x42],
    [0x6e, 0x8c, 0x56],
    [0x90, 0xa4, 0x75],
];
pub const FIRE: [u8; 3] = [0xe2, 0x7a, 0x34];

pub fn palette(b: Biome) -> [[u8; 3]; 8] {
    match b {
        Biome::CastleTown => CASTLE_TOWN,
        Biome::Greenwood => GREENWOOD,
        Biome::Highlands => HIGHLANDS,
        Biome::Wastes => WASTES,
        Biome::Coast => COAST,
        Biome::Fen => FEN,
        Biome::TiltYards => TILT_YARDS,
        Biome::Fairground => FAIRGROUND,
        Biome::WindCountry => WIND_COUNTRY,
        Biome::Longbow => LONGBOW,
        Biome::March if crate::world::plan::tabula() => HILLSIDE,
        Biome::March => MARCH,
    }
}

pub const BIOMES: [Biome; 11] = [
    Biome::CastleTown,
    Biome::Greenwood,
    Biome::Highlands,
    Biome::Wastes,
    Biome::Coast,
    Biome::Fen,
    Biome::TiltYards,
    Biome::Fairground,
    Biome::WindCountry,
    Biome::Longbow,
    Biome::March,
];

pub const FEATHER: f32 = 224.0;
pub const ROUND: f32 = 24.0;
pub const METRE: f32 = 11.0;
pub const SUN: [f32; 3] = [-0.66, 0.46, 0.46];
pub const SHADOW_REACH: f32 = 960.0;
pub const LAMPS_A_VIEW: usize = 8;
pub const REVEAL: f32 = 30.0;
pub const REVEAL_ON_STAGE: bool = true;
pub const REVEAL_AT: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub knight: Option<[f32; 2]>,
    pub others: [Option<[f32; 2]>; 3],
    pub time: f32,
}

pub fn look(view: &View, lamps: &[SetLamp]) -> Look {
    let [ox, oz] = view.origin;
    let centre = [ox + view.size[0] / 2.0, oz + view.size[1] / 2.0];
    let (base, zones) = zones(centre);
    let mut look = Look::new(palette(base).to_vec());
    look.accent = vec![GLINT, WINDOW, FIRE];
    look.dither = Dither::Bayer4;
    look.metre = METRE;
    look.time = view.time;
    look.sun = Some(Sun {
        toward: SUN,
        strength: 1.15,
        ambient: 0.27,
        shadow: Some(Shadow { focus: [view.size[0] / 2.0, 0.0, view.size[1] / 2.0], reach: SHADOW_REACH }),
        rise: 180.0,
    });
    look.warm = Some(EMBER.to_vec());
    look.water = Some(WATER.to_vec());
    look.zones = zones
        .into_iter()
        .map(|(b, [x0, z0, x1, z1])| {
            let shrink = ROUND + FEATHER / 2.0;
            Zone {
                ramp: palette(b).to_vec(),
                at: [(x0 + x1) / 2.0 - ox, (z0 + z1) / 2.0 - oz],
                half: [((x1 - x0) / 2.0 - shrink).max(0.0), ((z1 - z0) / 2.0 - shrink).max(0.0)],
                radius: ROUND,
                feather: FEATHER,
            }
        })
        .collect();
    look.zones.extend(camp_zones(view));
    look.zones.truncate(BIOME_ZONES + CAMP_ZONES);
    let mut near: Vec<&SetLamp> = lamps.iter().collect();
    let away = |l: &SetLamp| (l.at[0] - centre[0]).hypot(l.at[2] - centre[1]);
    near.sort_by(|a, b| away(a).total_cmp(&away(b)));
    look.lamps = near
        .into_iter()
        .take(LAMPS_A_VIEW)
        .map(|l| {
            let k = (l.at[0] * 0.37 + l.at[2] * 0.11) % 7.0;
            let t = view.time;
            let flicker = 1.0 + 0.06 * (t * 7.3 + k).sin() + 0.04 * (t * 12.1 + 2.0 * k).sin();
            Lamp { at: [l.at[0] - ox, l.at[1], l.at[2] - oz], reach: l.reach, strength: l.strength * flicker }
        })
        .collect();
    if REVEAL_ON_STAGE {
        look.reveal = view.knight.map(|[x, z]| Reveal { at: [x - ox, REVEAL_AT, z - oz], radius: REVEAL });
        look.reveals = view.others.iter().flatten().map(|&[x, z]| Reveal { at: [x - ox, REVEAL_AT, z - oz], radius: REVEAL }).collect();
    }
    look
}

pub const TENT_FEATHER: f32 = 8.0;
pub const BANNER_FEATHER: f32 = 4.0;
pub const SEEN_BEYOND: f32 = 96.0;

pub const BIOME_ZONES: usize = 4;
pub const CAMP_ZONES: usize = 4;
pub const KEEP_ZONES: usize = 3;

const CLOTH_CHROMA: f32 = 0.45;

pub fn cloth_ramp(cloth: [u8; 3], base: &[[u8; 3]; 8]) -> [[u8; 3]; 8] {
    use angel_stage::brand::{ENV_CHROMA, chroma, luma};
    let u = cloth.map(|c| f32::from(c) / 255.0);
    let own = luma(cloth);
    let lean = u.map(|c| c - own);
    let spread = chroma(cloth);
    let ratio = (spread / own.max(0.04)).max(CLOTH_CHROMA);
    base.map(|entry| {
        let l = luma(entry);
        let mut k = if spread < 1e-3 { 0.0 } else { (ratio * l).min(ENV_CHROMA) / spread };
        for d in lean {
            if d > 1e-4 {
                k = k.min((1.0 - l) / d);
            } else if d < -1e-4 {
                k = k.min(l / -d);
            }
        }
        lean.map(|d| ((l + d * k) * 255.0).round().clamp(0.0, 255.0) as u8)
    })
}

pub fn camp_zones(view: &View) -> Vec<Zone> {
    use crate::render::overworld::camps;
    use crate::world::houses;
    if !crate::world::plan::tabula() {
        return Vec::new();
    }
    let [ox, oz] = view.origin;
    let centre = [ox + view.size[0] / 2.0, oz + view.size[1] / 2.0];
    let mut cloths: Vec<([f32; 4], [u8; 3], f32)> = Vec::new();
    for id in houses::ids() {
        let foot = super::blockout::px(houses::footprint(id));
        let cloth = camps::house_cloth(houses::get(id));
        let [tents, banner] = super::realm_camp::camp_cloth_boxes(foot);
        cloths.push((tents, cloth, TENT_FEATHER));
        cloths.push((banner, cloth, BANNER_FEATHER));
    }
    let mut near: Vec<(f32, Zone)> = cloths
        .into_iter()
        .filter(|(b, _, f)| {
            let m = SEEN_BEYOND + f;
            b[2] + m > ox && b[0] - m < ox + view.size[0] && b[3] + m > oz && b[1] - m < oz + view.size[1]
        })
        .map(|([x0, z0, x1, z1], cloth, feather)| {
            let away = ((x0 + x1) / 2.0 - centre[0]).hypot((z0 + z1) / 2.0 - centre[1]);
            let zone = Zone {
                ramp: cloth_ramp(cloth, &HILLSIDE).to_vec(),
                at: [(x0 + x1) / 2.0 - ox, (z0 + z1) / 2.0 - oz],
                half: [(x1 - x0) / 2.0, (z1 - z0) / 2.0],
                radius: 0.0,
                feather,
            };
            (away, zone)
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    near.truncate(CAMP_ZONES);
    near.reverse();
    near.into_iter().map(|(_, zone)| zone).collect()
}

pub fn zones(centre: [f32; 2]) -> (Biome, Vec<(Biome, [f32; 4])>) {
    let (cw, ch) = ((CHUNK.0 * 16) as f32, (CHUNK.1 * 16) as f32);
    let (ccx, ccz) = (((centre[0] / cw).floor() as i32).clamp(0, CHUNKS_X - 1), ((centre[1] / ch).floor() as i32).clamp(0, CHUNKS_Y - 1));
    let base = biome::chunk(ccx, ccz);
    let reach = 2;
    let inside = |cx: i32, cz: i32| (cx - ccx).abs() <= reach && (cz - ccz).abs() <= reach && (0..CHUNKS_X).contains(&cx) && (0..CHUNKS_Y).contains(&cz);
    let rect_px = |[c0, r0, c1, r1]: [i32; 4]| [c0 as f32 * cw, r0 as f32 * ch, (c1 + 1) as f32 * cw, (r1 + 1) as f32 * ch];
    let away = |[x0, z0, x1, z1]: [f32; 4]| {
        let dx = (x0 - centre[0]).max(centre[0] - x1).max(0.0);
        let dz = (z0 - centre[1]).max(centre[1] - z1).max(0.0);
        dx.hypot(dz)
    };
    let mut found: Vec<(f32, Biome, [f32; 4])> = Vec::new();
    for b in BIOMES {
        if b == base {
            continue;
        }
        let cells: Vec<(i32, i32)> = (ccz - reach..=ccz + reach)
            .flat_map(|cz| (ccx - reach..=ccx + reach).map(move |cx| (cx, cz)))
            .filter(|&(cx, cz)| inside(cx, cz) && biome::chunk(cx, cz) == b)
            .collect();
        let Some(&(sx, sz)) = cells.iter().min_by(|a, b| away(rect_px([a.0, a.1, a.0, a.1])).total_cmp(&away(rect_px([b.0, b.1, b.0, b.1])))) else {
            continue;
        };
        let mut r = [sx, sz, sx, sz];
        let all = |c0: i32, r0: i32, c1: i32, r1: i32| (r0..=r1).all(|z| (c0..=c1).all(|x| inside(x, z) && biome::chunk(x, z) == b));
        loop {
            let mut grown = false;
            for side in 0..4 {
                let [c0, r0, c1, r1] = r;
                let ok = match side {
                    0 => all(c0 - 1, r0, c0 - 1, r1),
                    1 => all(c1 + 1, r0, c1 + 1, r1),
                    2 => all(c0, r0 - 1, c1, r0 - 1),
                    _ => all(c0, r1 + 1, c1, r1 + 1),
                };
                if ok {
                    match side {
                        0 => r[0] -= 1,
                        1 => r[2] += 1,
                        2 => r[1] -= 1,
                        _ => r[3] += 1,
                    }
                    grown = true;
                }
            }
            if !grown {
                break;
            }
        }
        let px = rect_px(r);
        found.push((away(px), b, px));
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.truncate(BIOME_ZONES);
    found.reverse();
    (base, found.into_iter().map(|(_, b, r)| (b, r)).collect())
}
