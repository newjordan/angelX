//! Test floor for the architecture generators.
//!
//! Four things are pinned here, and they are the four things a renderer can be
//! wrecked by: (1) triangle budgets, (2) finite unit normals and defined
//! materials, (3) determinism — same seed, byte-identical triangle stream —
//! and (4) documented footprints, so a camera can be staged against a doc
//! comment instead of against a guess.

use super::super::math::{V3, v3};
use super::super::mesh::{Mesh, mat};
use super::prim;
use super::rng::Rng;
use super::*;

/// Every material id the shader knows about. A tri tagged with anything else
/// would land in the palette's default bucket and quietly render wrong.
const ALL_MATS: [u8; 17] = [
    mat::MOONLIGHT,
    mat::EMBER,
    mat::FIRE,
    mat::STONE,
    mat::STONE_DARK,
    mat::ROOF,
    mat::WOOD,
    mat::WINDOW,
    mat::DOOR,
    mat::GRASS,
    mat::PATH,
    mat::WATER,
    mat::FOLIAGE,
    mat::TRUNK,
    mat::BANNER,
    mat::BOOKS,
    mat::FLOOR,
];

/// Exact byte image of a triangle stream — the determinism probe.
fn digest(m: &Mesh) -> Vec<u32> {
    let mut out = Vec::with_capacity(m.tris.len() * 16);
    for t in &m.tris {
        for p in t.v {
            out.push(p.x.to_bits());
            out.push(p.y.to_bits());
            out.push(p.z.to_bits());
        }
        for uv in t.uv {
            out.push(uv[0].to_bits());
            out.push(uv[1].to_bits());
        }
        out.push(t.mat as u32);
        out.push(t.normal.x.to_bits());
        out.push(t.normal.y.to_bits());
        out.push(t.normal.z.to_bits());
    }
    out
}

/// Budget + hygiene sweep. `min_z` is the floor for non-WATER geometry.
fn check_with_floor(name: &str, m: &Mesh, max_tris: usize, min_z: f32) {
    assert!(!m.tris.is_empty(), "{name}: emitted no triangles");
    assert!(
        m.tris.len() <= max_tris,
        "{name}: {} tris blows the {max_tris} budget",
        m.tris.len()
    );
    for (i, t) in m.tris.iter().enumerate() {
        for p in t.v {
            assert!(
                p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
                "{name} tri {i}: non-finite vertex {p:?}"
            );
        }
        for uv in t.uv {
            assert!(
                uv[0].is_finite() && uv[1].is_finite(),
                "{name} tri {i}: non-finite uv {uv:?}"
            );
        }
        let n = t.normal;
        assert!(
            n.x.is_finite() && n.y.is_finite() && n.z.is_finite(),
            "{name} tri {i}: non-finite normal {n:?}"
        );
        let len = n.length();
        assert!(
            (len - 1.0).abs() < 1e-3,
            "{name} tri {i}: normal length {len} is not unit"
        );
        assert!(
            ALL_MATS.contains(&t.mat),
            "{name} tri {i}: material {} is not a mesh::mat constant",
            t.mat
        );
        if t.mat != mat::WATER {
            for p in t.v {
                assert!(
                    p.z >= min_z,
                    "{name} tri {i}: z {} below the {min_z} floor with mat {}",
                    p.z,
                    t.mat
                );
            }
        }
    }
}

fn check(name: &str, m: &Mesh, max_tris: usize) {
    check_with_floor(name, m, max_tris, -0.01);
}

fn assert_bbox(name: &str, m: &Mesh, x: (f32, f32), y: (f32, f32), z: (f32, f32)) {
    let (lo, hi) = prim::bbox(m).unwrap_or_else(|| panic!("{name}: empty mesh"));
    assert!(
        lo.x >= x.0 && hi.x <= x.1,
        "{name}: x span {:.2}..{:.2} escapes documented {x:?}",
        lo.x,
        hi.x
    );
    assert!(
        lo.y >= y.0 && hi.y <= y.1,
        "{name}: y span {:.2}..{:.2} escapes documented {y:?}",
        lo.y,
        hi.y
    );
    assert!(
        hi.z >= z.0 && hi.z <= z.1,
        "{name}: height {:.2} outside documented {z:?}",
        hi.z
    );
}

fn assert_has(name: &str, m: &Mesh, wanted: &[u8]) {
    for &w in wanted {
        assert!(
            m.tris.iter().any(|t| t.mat == w),
            "{name}: expected some geometry tagged mat {w}"
        );
    }
}

/// Every builder under one seed: budget, normals, materials, ground floor.
#[test]
fn generators_are_budgeted_and_hygienic() {
    let s = 0xC0FF_EE01_u64;
    check("keep", &keep(s), 600);
    check("round_tower", &round_tower(5.0, 1.5, s), 250);
    check("rookery_dressing", &rookery_dressing(5.6, 1.6, s), 160);
    check("curtain_wall(8)", &curtain_wall(8.0, s), 150);
    check("curtain_wall(24)", &curtain_wall(24.0, s), 150);
    check("curtain_wall(2)", &curtain_wall(2.0, s), 150);
    check("gatehouse", &gatehouse(s), 450);
    check("forge", &forge(s), 260);
    check("observatory", &observatory(s), 260);
    check("library", &library(s), 800);
    check("library_interior", &library_interior(s), 600);
    check("cottage", &cottage(s), 120);
    check("chapel", &chapel(s), 220);
    check("market_stall", &market_stall(s), 140);
    check("well", &well(), 120);
    check("bridge(10)", &bridge(10.0), 200);
    check("bridge(4)", &bridge(4.0), 200);
    check("terrain_patch", &terrain_patch(40.0, 40.0, s), 800);
    check("tree", &tree(s), 60);
}

/// The measured triangle counts behind the table in `arch.rs`'s module doc.
/// The doc quotes real numbers, so there has to be a one-command way to
/// re-measure them after a generator changes.
///
/// `cargo test --bin angel print_the_triangle_budget -- --ignored --nocapture`
#[test]
#[ignore = "manual: prints the measured triangle counts for the arch.rs doc table"]
fn print_the_triangle_budget() {
    let s = 42u64;
    let rows: [(&str, Mesh); 16] = [
        ("keep", keep(s)),
        ("round_tower", round_tower(5.6, 1.6, s)),
        ("rookery_dressing", rookery_dressing(5.6, 1.6, s)),
        ("curtain_wall(22)", curtain_wall(22.0, s)),
        ("gatehouse", gatehouse(s)),
        ("forge", forge(s)),
        ("observatory", observatory(s)),
        ("library", library(s)),
        ("library_interior", library_interior(s)),
        ("cottage", cottage(s)),
        ("chapel", chapel(s)),
        ("market_stall", market_stall(s)),
        ("well", well()),
        ("bridge(9)", bridge(9.0)),
        ("tree", tree(s)),
        ("court_scene", court_scene(0x4341_5354_4C45_3344)),
    ];
    for (name, mesh) in rows {
        let (lo, hi) = prim::bbox(&mesh).expect("bbox");
        eprintln!(
            "{name:<18} {:>5} tris   x {:>6.2}..{:<6.2} y {:>6.2}..{:<6.2} top {:.2}",
            mesh.tris.len(),
            lo.x,
            hi.x,
            lo.y,
            hi.y,
            hi.z
        );
    }
}

/// The same sweep across a spread of seeds — jitter must never push a builder
/// over budget or produce a degenerate face.
#[test]
fn generators_hold_across_seeds() {
    for s in 0..24u64 {
        check("keep", &keep(s), 600);
        check("round_tower", &round_tower(4.0, 1.2, s), 250);
        check("rookery_dressing", &rookery_dressing(4.0, 1.2, s), 160);
        check("curtain_wall", &curtain_wall(14.0, s), 150);
        check("gatehouse", &gatehouse(s), 450);
        check("forge", &forge(s), 260);
        check("observatory", &observatory(s), 260);
        check("library", &library(s), 800);
        check("library_interior", &library_interior(s), 600);
        check("cottage", &cottage(s), 120);
        check("chapel", &chapel(s), 220);
        check("market_stall", &market_stall(s), 140);
        check("tree", &tree(s), 60);
        check("terrain_patch", &terrain_patch(24.0, 24.0, s), 800);
    }
}

/// A terrain grid is capped at 20 × 20 cells, so no patch size can blow the
/// budget however large it is asked to be.
#[test]
fn terrain_grid_is_capped() {
    for &(w, d) in &[(4.0_f32, 4.0_f32), (40.0, 40.0), (400.0, 400.0)] {
        let m = terrain_patch(w, d, 3);
        check("terrain_patch", &m, 800);
        assert!(m.tris.len() >= 8, "terrain {w}x{d}: too coarse to shade");
    }
}

/// Rivers cut below z = 0 and are tagged WATER; nothing else ever is.
#[test]
fn only_water_dips_below_ground() {
    let opts = TerrainOpts {
        cell: 2.0,
        relief: 0.34,
        path: Some(Strip::along_y(0.0, 1.2)),
        river: Some(Strip::along_x(-4.0, 2.0)),
        water_depth: 0.4,
    };
    let m = terrain_patch_with(30.0, 30.0, 11, &opts);
    check("terrain_patch_with(river)", &m, 800);
    assert_has(
        "terrain_patch_with",
        &m,
        &[mat::GRASS, mat::PATH, mat::WATER],
    );
    let (lo, _) = prim::bbox(&m).expect("terrain bbox");
    assert!(lo.z < -0.01, "the river never cut a bed: min z {}", lo.z);
    assert_eq!(
        Axis::X,
        opts.river.map(|r| r.axis).unwrap(),
        "river strip axis"
    );
}

/// Footprints and heights match the doc comments — camera staging depends on
/// these numbers being true.
#[test]
fn footprints_match_docs() {
    let s = 42u64;
    assert_bbox("keep", &keep(s), (-3.85, 3.85), (-3.85, 3.85), (5.4, 6.3));
    assert_bbox(
        "round_tower",
        &round_tower(5.0, 1.5, s),
        (-2.05, 2.05),
        (-2.05, 2.05),
        (7.6, 9.1), // +1.10 more when the seed grants a banner mast
    );
    assert_bbox(
        "curtain_wall",
        &curtain_wall(12.0, s),
        (-6.05, 6.05),
        (-0.70, 0.70),
        (3.5, 4.0),
    );
    assert_bbox(
        "gatehouse",
        &gatehouse(s),
        (-4.10, 4.10),
        (-1.80, 1.80),
        (7.0, 7.8),
    );
    assert_bbox("forge", &forge(s), (-2.20, 3.50), (-2.15, 2.15), (4.1, 4.7));
    assert_bbox(
        "observatory",
        &observatory(s),
        (-2.65, 2.65),
        (-2.65, 2.65),
        // The sighting tube, not the dome, is the highest thing on it.
        (7.35, 7.85),
    );
    // The dressing has to land *on* its tower, never outside its eave.
    for &(h, rad) in &[(5.6_f32, 1.6_f32), (4.0, 1.2)] {
        // The rim birds hang a little past the eave on purpose; nothing may
        // reach past the tower's own banner mast.
        let reach = rad * 1.34 + 0.30;
        assert_bbox(
            "rookery_dressing",
            &rookery_dressing(h, rad, s),
            (-reach, reach),
            (-reach, reach),
            (h + 0.16, h + 1.5 * rad + 1.10),
        );
    }
    assert_bbox(
        "library",
        &library(s),
        (-6.55, 6.55),
        (-5.05, 3.75),
        (9.2, 10.5),
    );
    assert_bbox(
        "library_interior",
        &library_interior(s),
        (-5.75, 5.75),
        (-2.75, 2.75),
        (6.7, 6.9),
    );
    assert_bbox(
        "cottage",
        &cottage(s),
        (-2.50, 2.50),
        (-1.45, 1.45),
        (3.1, 4.1),
    );
    assert_bbox(
        "chapel",
        &chapel(s),
        (-3.75, 3.75),
        (-1.85, 1.85),
        (6.6, 7.2),
    );
    assert_bbox(
        "market_stall",
        &market_stall(s),
        (-1.45, 1.45),
        (-1.05, 1.05),
        (2.2, 2.4),
    );
    assert_bbox("well", &well(), (-1.00, 1.00), (-0.80, 0.80), (2.4, 2.6));
    assert_bbox(
        "bridge(10)",
        &bridge(10.0),
        (-5.75, 5.75),
        (-1.35, 1.35),
        (2.2, 2.5),
    );
    assert_bbox(
        "terrain_patch",
        &terrain_patch(20.0, 12.0, s),
        (-10.05, 10.05),
        (-6.05, 6.05),
        (0.0, 0.40),
    );
    for s in 0..12u64 {
        assert_bbox("tree", &tree(s), (-1.35, 1.35), (-1.35, 1.35), (2.9, 4.3));
    }
}

/// Materials that must be present, because the shader keys real behaviour off
/// them (WINDOW is emissive, BOOKS is shelf fill, WATER is reflective).
#[test]
fn semantic_materials_are_tagged() {
    let s = 5u64;
    assert_has(
        "keep",
        &keep(s),
        &[
            mat::STONE,
            mat::STONE_DARK,
            mat::WINDOW,
            mat::DOOR,
            mat::ROOF,
        ],
    );
    assert_has(
        "round_tower",
        &round_tower(6.0, 1.4, 3),
        &[mat::STONE, mat::ROOF, mat::WINDOW],
    );
    assert_has(
        "rookery_dressing",
        &rookery_dressing(5.6, 1.6, s),
        &[mat::STONE, mat::STONE_DARK],
    );
    // The forge *is* its fire: without EMBER the Smithy vantage is a shed.
    assert_has(
        "forge",
        &forge(s),
        &[
            mat::STONE,
            mat::STONE_DARK,
            mat::ROOF,
            mat::WOOD,
            mat::EMBER,
        ],
    );
    assert_has(
        "observatory",
        &observatory(s),
        &[
            mat::STONE,
            mat::STONE_DARK,
            mat::ROOF,
            mat::WINDOW,
            mat::DOOR,
            mat::WOOD,
        ],
    );
    assert_has(
        "curtain_wall",
        &curtain_wall(10.0, s),
        &[mat::STONE, mat::STONE_DARK],
    );
    assert_has(
        "gatehouse",
        &gatehouse(s),
        &[mat::STONE, mat::DOOR, mat::PATH, mat::ROOF],
    );
    assert_has(
        "library",
        &library(s),
        &[mat::STONE, mat::ROOF, mat::WINDOW, mat::DOOR],
    );
    assert_has(
        "library_interior",
        &library_interior(s),
        &[
            mat::FLOOR,
            mat::BOOKS,
            mat::WOOD,
            mat::WINDOW,
            // The hall is lit by candle and hearth, and both are live flame:
            // the hero interior breathes with the other seven or it is the one
            // dead room in the realm.
            mat::FIRE,
            mat::DOOR,
        ],
    );
    assert_has("cottage", &cottage(s), &[mat::ROOF, mat::DOOR, mat::WINDOW]);
    assert_has("chapel", &chapel(s), &[mat::ROOF, mat::WINDOW, mat::DOOR]);
    assert_has("well", &well(), &[mat::STONE, mat::WATER, mat::WOOD]);
    assert_has("bridge", &bridge(9.0), &[mat::STONE, mat::PATH]);
    assert_has(
        "terrain_patch",
        &terrain_patch(20.0, 20.0, s),
        &[mat::GRASS],
    );

    // Trees cover both kinds across seeds, and every seed grows both parts.
    for s in 0..10u64 {
        assert_has("tree", &tree(s), &[mat::TRUNK, mat::FOLIAGE]);
    }
    // Some tower seed must fly a banner.
    assert!(
        (0..16u64).any(|s| round_tower(5.0, 1.3, s)
            .tris
            .iter()
            .any(|t| t.mat == mat::BANNER)),
        "no tower seed in 0..16 raised a banner"
    );
}

/// Same seed → byte-identical triangle stream. This is the world3d
/// determinism law; a frame cache keyed on scene state depends on it.
#[test]
fn generators_are_deterministic() {
    macro_rules! same {
        ($name:literal, $e:expr_2021) => {
            assert_eq!(digest(&$e), digest(&$e), "{} is not deterministic", $name);
        };
    }
    same!("keep", keep(7));
    same!("round_tower", round_tower(5.0, 1.5, 7));
    same!("curtain_wall", curtain_wall(13.0, 7));
    same!("gatehouse", gatehouse(7));
    same!("rookery_dressing", rookery_dressing(5.6, 1.6, 7));
    same!("forge", forge(7));
    same!("observatory", observatory(7));
    same!("library", library(7));
    same!("library_interior", library_interior(7));
    same!("cottage", cottage(7));
    same!("chapel", chapel(7));
    same!("market_stall", market_stall(7));
    same!("well", well());
    same!("bridge", bridge(9.0));
    same!("terrain_patch", terrain_patch(30.0, 30.0, 7));
    same!("tree", tree(7));
    same!("court_scene", court_scene(7));
    same!("hamlet_scene", hamlet_scene(7));
}

/// Different seeds must actually produce different geometry — a builder that
/// ignores its seed would silently make every cottage in a row a clone.
#[test]
fn seeds_change_the_geometry() {
    fn variety(name: &str, f: impl Fn(u64) -> Mesh) {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        for s in 0..8u64 {
            let d = digest(&f(s));
            if !seen.contains(&d) {
                seen.push(d);
            }
        }
        assert!(
            seen.len() >= 2,
            "{name}: 8 seeds produced only {} distinct mesh(es)",
            seen.len()
        );
    }
    variety("keep", keep);
    variety("round_tower", |s| round_tower(5.0, 1.5, s));
    variety("curtain_wall", |s| curtain_wall(13.0, s));
    variety("gatehouse", gatehouse);
    variety("rookery_dressing", |s| rookery_dressing(5.6, 1.6, s));
    variety("forge", forge);
    variety("observatory", observatory);
    variety("library", library);
    variety("library_interior", library_interior);
    variety("cottage", cottage);
    variety("chapel", chapel);
    variety("market_stall", market_stall);
    variety("terrain_patch", |s| terrain_patch(30.0, 30.0, s));
    variety("tree", tree);
    variety("court_scene", court_scene);
}

/// The composed vista: one call, a whole scene, inside the ~5k whole-scene
/// budget with room left for the renderer agent's props.
#[test]
fn court_scene_is_a_staged_vista() {
    for s in [0u64, 1, 7, 99] {
        let m = court_scene(s);
        // Buildings are sunk 0.12 into displaced ground and the bridge stands
        // in the river bed, so the non-water floor sits below zero here.
        check_with_floor("court_scene", &m, 5000, -0.60);
        assert!(
            m.tris.len() > 2500,
            "court_scene(seed {s}): only {} tris — something failed to place",
            m.tris.len()
        );
        assert_has(
            "court_scene",
            &m,
            &[
                mat::GRASS,
                mat::PATH,
                mat::WATER,
                mat::STONE,
                mat::ROOF,
                mat::WINDOW,
                mat::DOOR,
                mat::FOLIAGE,
                mat::TRUNK,
                mat::EMBER,
            ],
        );
        let (lo, hi) = scene_bounds(&m).expect("scene bounds");
        assert!(
            lo.x >= -29.0 && hi.x <= 29.0 && lo.y >= -29.0 && hi.y <= 29.0,
            "court scene escapes its 56x56 patch: {lo:?}..{hi:?}"
        );
        assert!(
            hi.z > 8.0 && hi.z < 12.0,
            "court scene skyline height {} is wrong for a library lantern",
            hi.z
        );
    }
}

/// The approach corridor. The travel sweep runs the east verge of the road
/// (`world3d::ROAD_NEAR/BANK/FAR`), and the ride's camera flies it at eye
/// height — so anything built within a few tiles of that polyline is a wall in
/// the face for a third of the journey, which is the one composition the world
/// vista law forbids outright. This is the guard that keeps the hamlet off the
/// road: it failed at the old spacing (1.7 tiles) and is why the cottages moved.
#[test]
fn the_approach_corridor_stays_clear_of_the_sweep() {
    // The two straight runs `world3d::road_pose` interpolates.
    const SWEEP: [((f32, f32), (f32, f32)); 2] =
        [((0.6, -14.4), (4.3, -19.4)), ((4.3, -19.4), (4.6, -26.5))];
    const FLOOR: f32 = 3.0;

    let m = court_scene(0x4341_5354_4C45_3344);
    for (a, b) in SWEEP {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        for tri in &m.tris {
            // Ground planes are what the camera *stands on*; only built mass
            // counts as something it can walk into.
            if matches!(tri.mat, mat::GRASS | mat::PATH | mat::WATER | mat::FLOOR) {
                continue;
            }
            for p in tri.v {
                // The bridge is the road. A parapet at the camera's elbow
                // while it crosses the river is the ride working, not a wall
                // in the face. (Footprint: `bridge(9.0)` at (0, -22) along y.)
                if p.x.abs() <= 2.2 && p.y <= -16.0 {
                    continue;
                }
                // Only mass the sweep goes *past* counts. The gatehouse the
                // road runs into is the destination, and a destination is
                // allowed to fill the frame — that is what arriving looks
                // like. It projects off the near end of the run, so the
                // strict-interior test excludes it without a special case.
                let along = ((p.x - a.0) * dx + (p.y - a.1) * dy) / (len * len);
                if !(0.02..0.98).contains(&along) {
                    continue;
                }
                let (nx, ny) = (a.0 + dx * along, a.1 + dy * along);
                let lateral = ((p.x - nx).powi(2) + (p.y - ny).powi(2)).sqrt();
                assert!(
                    lateral >= FLOOR,
                    "the sweep passes {lateral:.2} tiles abeam built mass at ({:.1}, {:.1}) — the corridor must stay {FLOOR} wide",
                    p.x,
                    p.y
                );
            }
        }
    }
}

/// The forge is only a forge because of its fire, and the fire has to be
/// *inside the shed*, at working height, facing the mouth — an EMBER quad on
/// the roof or behind the back wall would light nothing.
#[test]
fn the_forge_is_about_its_fire() {
    for s in 0..8u64 {
        let m = forge(s);
        let fire: Vec<_> = m.tris.iter().filter(|t| t.mat == mat::EMBER).collect();
        assert!(
            fire.len() >= 8,
            "forge(seed {s}): only {} ember faces — the mouth will not read",
            fire.len()
        );
        for t in &fire {
            for p in t.v {
                assert!(
                    p.x > -2.2 && p.x < 2.2 && p.y > -1.9 && p.y < 1.8,
                    "forge(seed {s}): ember at {p:?} is outside the shed"
                );
                assert!(
                    (0.25..3.0).contains(&p.z),
                    "forge(seed {s}): ember at {p:?} is not at working height"
                );
            }
        }
    }
}

#[test]
fn hamlet_scene_is_cheap() {
    let m = hamlet_scene(3);
    check_with_floor("hamlet_scene", &m, 1600, -0.40);
    assert_has(
        "hamlet_scene",
        &m,
        &[mat::GRASS, mat::PATH, mat::ROOF, mat::DOOR],
    );
}

/// Crenellation teeth have to survive the Bayer screen: a tooth narrower than
/// roughly half a tile disappears at vista range.
#[test]
fn merlon_teeth_are_dot_legible() {
    let mut m = Mesh::new();
    let n = prim::merlons(
        &mut m,
        v3(-4.0, 0.0, 3.0),
        v3(4.0, 0.0, 3.0),
        v3(0.0, 1.0, 0.0),
        0.8,
        0.36,
        1.30,
        14,
        mat::STONE,
    );
    assert_eq!(n, 6, "8 tiles at 1.30 pitch should give 6 teeth");
    assert_eq!(m.tris.len(), n * 8, "each tooth is 4 quads (no inner face)");
    let width = (8.0 / n as f32) * 0.58;
    assert!(
        width > 0.45,
        "merlon width {width} is too fine for the dot grid"
    );

    // The cap keeps a very long run inside budget without shrinking teeth.
    let mut long = Mesh::new();
    let n2 = prim::merlons(
        &mut long,
        v3(-30.0, 0.0, 3.0),
        v3(30.0, 0.0, 3.0),
        v3(0.0, 1.0, 0.0),
        0.8,
        0.36,
        1.30,
        14,
        mat::STONE,
    );
    assert_eq!(n2, 14, "tooth count must clamp to max_teeth");
}

/// The raven has to *be* a bird at braille scale, and the three features that
/// make it one are measurable: the head stands above the back, the beak reaches
/// past the breast, and the tail lifts clear of the perch. Pin all three, plus
/// the documented footprint the tower dressing places against.
#[test]
fn the_raven_reads_as_a_bird() {
    let mut m = Mesh::new();
    prim::bird_profile(
        &mut m,
        v3(0.0, 0.0, 2.0),
        v3(0.0, 1.0, 0.0),
        1.0,
        mat::STONE,
    );
    assert_eq!(m.tris.len(), 9, "the profile is nine triangles");
    let (lo, hi) = prim::bbox(&m).expect("bbox");
    assert!(
        (hi.y - 0.62).abs() < 1e-4 && (lo.y + 0.62).abs() < 1e-4,
        "profile spans ±0.62 s across: {:.3}..{:.3}",
        lo.y,
        hi.y
    );
    assert!(
        (lo.z - 2.0).abs() < 1e-4 && (hi.z - 2.66).abs() < 1e-4,
        "profile stands 0..0.66 s off its perch: {:.3}..{:.3}",
        lo.z,
        hi.z
    );

    // Sample the silhouette's height at three places along the body. The head
    // must out-top the back, and the tail must sit below both — a shape that
    // only ever descends is a paper dart, which is what the first pass drew.
    let top_at = |y: f32| {
        m.tris
            .iter()
            .flat_map(|t| t.v)
            .filter(|p| (p.y - y).abs() < 0.09)
            .fold(f32::NEG_INFINITY, |acc, p| acc.max(p.z))
    };
    let (head, nape, back, tail) = (top_at(-0.26), top_at(-0.16), top_at(0.24), top_at(0.62));
    assert!(head > nape, "no neck: head {head:.3} nape {nape:.3}");
    assert!(head > back, "no head: head {head:.3} back {back:.3}");
    assert!(tail < back, "no tail drop: tail {tail:.3} back {back:.3}");

    // The cross plane is a lump, not a second profile — twelve triangles all
    // in, and nothing sticking out sideways past the body.
    let mut crossed = Mesh::new();
    prim::raven(&mut crossed, V3::ZERO, v3(1.0, 0.0, 0.0), 1.0, mat::STONE);
    assert_eq!(crossed.tris.len(), 12);
    let (lo, hi) = prim::bbox(&crossed).expect("bbox");
    assert!(
        lo.y >= -0.31 && hi.y <= 0.31,
        "the bulk plane must stay inside the body: {:.3}..{:.3}",
        lo.y,
        hi.y
    );

    // A zero axis must drop the bird rather than emit NaN normals.
    let mut degenerate = Mesh::new();
    prim::bird_profile(&mut degenerate, V3::ZERO, V3::ZERO, 1.0, mat::STONE);
    prim::bird_profile(
        &mut degenerate,
        V3::ZERO,
        v3(1.0, 0.0, 0.0),
        0.0,
        mat::STONE,
    );
    assert!(
        degenerate.tris.is_empty(),
        "degenerate ravens must be dropped"
    );
}

/// A light shaft has to have a *body*: two of its three faces must turn toward
/// a camera looking down the room's long axis, or the beam renders as a line.
#[test]
fn a_light_shaft_turns_a_face_toward_an_axial_camera() {
    let mut m = Mesh::new();
    prim::light_shaft(
        &mut m,
        0.0,
        (2.6, 3.0),
        (0.8, 0.05),
        0.4,
        1.3,
        mat::MOONLIGHT,
    );
    check("light_shaft", &m, 12);
    assert_eq!(m.tris.len(), 7, "three quads and a sill cap");
    // The room's axis is +x; count the faces with a real x component.
    let facing = m.tris.iter().filter(|t| t.normal.x.abs() > 0.25).count();
    assert!(
        facing >= 4,
        "only {facing} of {} faces turn off the wall plane",
        m.tris.len()
    );
    let (lo, hi) = prim::bbox(&m).expect("bbox");
    assert!(
        (hi.z - 3.0).abs() < 1e-4 && lo.z >= 0.0,
        "the shaft runs sill to floor: {:.3}..{:.3}",
        lo.z,
        hi.z
    );
}

/// Degenerate faces never reach the mesh — that is what guarantees the
/// unit-normal invariant the shader relies on.
#[test]
fn degenerate_faces_are_dropped() {
    let mut m = Mesh::new();
    prim::tri(
        &mut m,
        [v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0), v3(2.0, 0.0, 0.0)],
        [[0.0; 2]; 3],
        mat::STONE,
    );
    assert!(m.tris.is_empty(), "a collinear tri was emitted");

    prim::quad(
        &mut m,
        v3(0.0, 0.0, 0.0),
        v3(1.0, 0.0, 0.0),
        v3(1.0, 0.0, 0.0),
        v3(0.0, 0.0, 1.0),
        mat::STONE,
    );
    assert_eq!(m.tris.len(), 1, "only the collapsed half should be dropped");
    assert!((m.tris[0].normal.length() - 1.0).abs() < 1e-5);
}

/// The seeded stream is a pure function of the seed and stays in range.
#[test]
fn rng_is_pure_and_bounded() {
    let draw = |seed: u64| -> Vec<u32> {
        let mut r = Rng::new(seed);
        (0..32).map(|_| r.unit().to_bits()).collect()
    };
    assert_eq!(draw(7), draw(7), "same seed drifted");
    assert_ne!(draw(7), draw(8), "different seeds collided");

    let mut r = Rng::new(1234);
    for _ in 0..512 {
        let u = r.unit();
        assert!((0.0..1.0).contains(&u), "unit() escaped [0,1): {u}");
        let j = r.jitter(0.5);
        assert!((-0.5..=0.5).contains(&j), "jitter escaped range: {j}");
        assert!(r.below(7) < 7);
        let i = r.int_in(3, 6);
        assert!((3..=6).contains(&i));
    }
}

/// `V3` is `Copy`, so the bbox helper must not have been fooled by a
/// zero-length mesh, and empty meshes report `None` rather than panicking.
#[test]
fn bbox_handles_the_empty_mesh() {
    assert!(prim::bbox(&Mesh::new()).is_none());
    let mut m = Mesh::new();
    prim::boxed(
        &mut m,
        v3(-1.0, -2.0, 0.0),
        v3(1.0, 2.0, 3.0),
        mat::STONE,
        prim::F_ALL,
    );
    let (lo, hi) = prim::bbox(&m).expect("bbox");
    assert_eq!((lo.x, lo.y, lo.z), (-1.0, -2.0, 0.0));
    assert_eq!((hi.x, hi.y, hi.z), (1.0, 2.0, 3.0));
    assert_eq!(m.tris.len(), 12);
    let _: V3 = lo;
}
