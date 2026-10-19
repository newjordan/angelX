//! The realm, lit: every chunk's set reads whole, its ground meets its
//! neighbours' with no seam, the walked ground is the floor and nothing
//! stands over it, every place and castle stands on its footprint, and the
//! look of any view is one the renderer takes.

use super::*;
use crate::world3d::realm_look::{self, BIOMES, View};
use angel_stage::set::Set;

const LAMPS_A_VIEW_TEST: usize = realm_look::LAMPS_A_VIEW;

fn every_chunk() -> impl Iterator<Item = (i32, i32)> {
    let (gw, gh) = super::super::kingdom::GRID;
    (0..gh).flat_map(move |cz| (0..gw).map(move |cx| (cx, cz)))
}

fn parsed(cx: i32, cz: i32) -> Set {
    let index = super::super::kingdom::index(cx, cz).expect("a chunk");
    Set::parse(&set_text(index)).unwrap_or_else(|r| panic!("chunk {cx},{cz}: {:?}", r.iter().take(3).map(ToString::to_string).collect::<Vec<_>>()))
}

#[test]
fn every_realm_chunk_reads_whole_with_its_ground() {
    for (cx, cz) in every_chunk() {
        let set = parsed(cx, cz);
        assert_eq!(set.id, chunk_id(cx, cz));
        let g = set.ground.as_ref().unwrap_or_else(|| panic!("chunk {cx},{cz} has a ground"));
        assert_eq!((g.cols, g.rows, g.step), ((CHUNK.0 * 2) as usize, (CHUNK.1 * 2) as usize, 8.0));
        assert_eq!(g.at, [(cx * CHUNK.0) as f32 * TILE, (cz * CHUNK.1) as f32 * TILE]);
        assert!(set.lamps.len() <= LAMPS_A_CHUNK, "chunk {cx},{cz}: {} lamps", set.lamps.len());
        assert!(g.heights.iter().all(|h| (-17.0..=120.0).contains(h)), "chunk {cx},{cz}: heights in range");
    }
}

#[test]
fn neighbours_share_their_edges_heights_and_tones() {
    for (cx, cz) in every_chunk() {
        let a = parsed(cx, cz);
        let ga = a.ground.as_ref().unwrap();
        if let Some(index) = super::super::kingdom::index(cx + 1, cz) {
            let b = Set::parse(&set_text(index)).unwrap();
            let gb = b.ground.as_ref().unwrap();
            for j in 0..=ga.rows {
                let (ka, kb) = (j * (ga.cols + 1) + ga.cols, j * (gb.cols + 1));
                assert_eq!((ga.heights[ka], ga.tones[ka]), (gb.heights[kb], gb.tones[kb]), "{cx},{cz} east edge row {j}");
            }
        }
        if let Some(index) = super::super::kingdom::index(cx, cz + 1) {
            let b = Set::parse(&set_text(index)).unwrap();
            let gb = b.ground.as_ref().unwrap();
            for i in 0..=ga.cols {
                let (ka, kb) = (ga.rows * (ga.cols + 1) + i, i);
                assert_eq!((ga.heights[ka], ga.tones[ka]), (gb.heights[kb], gb.tones[kb]), "{cx},{cz} south edge col {i}");
            }
        }
    }
}

#[test]
fn the_walked_ground_is_the_floor_and_nothing_stands_over_it() {
    let land = super::super::kingdom::land();
    let terrain = realm_ground::terrain();
    for (cx, cz) in every_chunk() {
        let set = parsed(cx, cz);
        let (c, r, w, h) = land.chunk_rect(cx, cz);
        for y in r..r + h {
            for x in c..c + w {
                if !land.walkable(x, y) || land.cover(x, y) == Cover::Bridge {
                    continue;
                }
                let (px, pz) = (x as f32 * TILE + 8.0, y as f32 * TILE + 8.0);
                assert_eq!(terrain.height_at(px, pz), 0.0, "tile {x},{y} is walked: the floor");
                for s in &set.shapes {
                    // What lies on the floor, and the boot-high dressing a
                    // knight walks through.
                    if s.at[1] + s.size[1] <= blockout::LOW {
                        continue;
                    }
                    let [a, b, cc, d] = crate::world3d::fit::footprint(s);
                    let slender = (cc - a).min(d - b) <= 2.5 && (cc - a).max(d - b) <= 7.0;
                    // Inset: a crown or a slab may spill a pixel or two over
                    // its tile's edge, never over a tile's middle.
                    let inside = |px: f32, pz: f32| px > a + 3.0 && px < cc - 3.0 && pz > b + 3.0 && pz < d - 3.0;
                    let samples = [(px, pz), (px - 4.0, pz - 4.0), (px + 4.0, pz + 4.0), (px - 4.0, pz + 4.0), (px + 4.0, pz - 4.0)];
                    assert!(
                        slender || s.at[1] >= 30.0 || !samples.iter().any(|&(sx, sz)| inside(sx, sz)),
                        "chunk {cx},{cz}: line {} stands over walked tile {x},{y}",
                        s.line
                    );
                }
            }
        }
    }
}

#[test]
fn every_place_and_castle_stands_on_its_footprint() {
    let land = super::super::kingdom::land();
    let tall = |set: &Set, foot: blockout::Rect, least: f32| {
        let [x0, z0, x1, z1] = blockout::px(foot);
        set.shapes.iter().any(|s| {
            let [a, b, c, d] = crate::world3d::fit::footprint(s);
            s.at[1] + s.size[1] >= least && a >= x0 - 1.0 && c <= x1 + 1.0 && b >= z0 - 1.0 && d <= z1 + 1.0
        })
    };
    for b in &land.builds {
        if in_castle(b) {
            continue;
        }
        let (cx, cz) = land.owner(b);
        let set = parsed(cx, cz);
        assert!(tall(&set, b.foot, 8.0), "{} ({:?}) stands on {:?}", b.name, b.kit, b.foot);
    }
    for site in castles() {
        let set = parsed(owner(site.foot).0, owner(site.foot).1);
        assert!(tall(&set, site.foot, 120.0), "{} stands great on {:?}", site.name, site.foot);
    }
    assert_eq!(castles().len(), crate::world::houses::ids().count() + 2, "every house's castle, the Keep and the Dragon Keep");
}

#[test]
fn each_biome_has_a_dark_monotone_palette_of_its_own() {
    use angel_stage::look::{luminance, unit_rgb};
    let mut firsts = Vec::new();
    for b in BIOMES {
        let ramp = realm_look::palette(b);
        let levels: Vec<f32> = ramp.iter().map(|&c| luminance(unit_rgb(c))).collect();
        assert!(levels.windows(2).all(|w| w[0] < w[1]), "{b:?}: dark to light {levels:?}");
        assert!(levels[0] < 0.08 && levels[7] < 0.62, "{b:?}: dark {levels:?}");
        // Nearly monotone: no channel far from the entry's grey.
        for c in ramp {
            let grey = (u16::from(c[0]) + u16::from(c[1]) + u16::from(c[2])) / 3;
            assert!(c.iter().all(|&v| (i32::from(v) - i32::from(grey)).abs() < 40), "{b:?}: {c:?}");
        }
        firsts.push(ramp[4]);
    }
    for (i, a) in firsts.iter().enumerate() {
        for b in &firsts[i + 1..] {
            assert_ne!(a, b, "each biome its own");
        }
    }
}

#[test]
fn a_views_look_is_valid_and_slides_its_palette_over_a_border() {
    let size = [384.0, 224.0];
    for (cx, cz) in every_chunk() {
        let [x, z] = centre(cx, cz);
        let view = View { origin: [x - size[0] / 2.0, z - size[1] / 2.0], size, knight: Some([x, z + 20.0]), others: [Some([x + 30.0, z]), None, None], time: 3.0 };
        let index = super::super::kingdom::index(cx, cz).unwrap();
        let lamps = parsed(cx, cz).lamps;
        let look = realm_look::look(&view, &lamps);
        assert_eq!(look.validate(), Ok(()), "chunk {cx},{cz}");
        assert!(look.lights() && look.zones.len() <= 4 && look.lamps.len() <= LAMPS_A_VIEW_TEST);
        assert_eq!((look.reveal.is_some(), look.reveals.len()), if realm_look::REVEAL_ON_STAGE { (true, 1) } else { (false, 0) });
        assert_eq!(look.ramp, realm_look::palette(crate::world::biome::chunk(cx, cz)).to_vec());
        let _ = index;
    }
    // Over a border (Castle Town into the Tilt-Yards, chunks 4,3 and 5,3):
    // the same mix of the two either side of the flip of the look's own ramp.
    let border = 5.0 * (CHUNK.0 as f32 * TILE);
    let z = 3.5 * (CHUNK.1 as f32 * TILE);
    let weight = |look: &angel_stage::look::Look, ramp: [[u8; 3]; 8], at: [f32; 2]| -> f32 {
        // The share of `ramp` at stage point `at`: 1 where it is the look's
        // own and no zone covers it, else the zones' feathers.
        let mut w = if look.ramp == ramp.to_vec() { 1.0 } else { 0.0 };
        for zone in &look.zones {
            let d = (zone.distance(at[0], at[1]) - zone.radius).max(0.0);
            let k = 1.0 - angel_stage::look::smoothstep(0.0, zone.feather, d);
            let mine = if zone.ramp == ramp.to_vec() { 1.0 } else { 0.0 };
            w = w * (1.0 - k) + mine * k;
        }
        w
    };
    let town = realm_look::palette(crate::world::biome::Biome::CastleTown);
    for dx in [-40.0f32, -8.0, 8.0, 40.0] {
        let p = [border + dx, z];
        let mut shares = Vec::new();
        for from in [border - 30.0, border + 30.0] {
            let view = View { origin: [from - size[0] / 2.0, z - size[1] / 2.0], size, knight: None, others: [None; 3], time: 0.0 };
            let look = realm_look::look(&view, &[]);
            shares.push(weight(&look, town, [p[0] - view.origin[0], p[1] - view.origin[1]]));
        }
        assert!((shares[0] - shares[1]).abs() < 0.05, "at {dx} from the border: {shares:?}");
    }
}

#[test]
#[ignore = "writes every chunk's realm set to the folder REALM_SETS names"]
fn write_realm_sets() {
    let dir = std::path::PathBuf::from(std::env::var("REALM_SETS").unwrap_or_else(|_| "/tmp/realm-sets".into()));
    std::fs::create_dir_all(&dir).unwrap();
    for (cx, cz) in every_chunk() {
        let index = super::super::kingdom::index(cx, cz).unwrap();
        std::fs::write(dir.join(format!("{}.set", chunk_id(cx, cz))), &*set_text(index)).unwrap();
    }
}
