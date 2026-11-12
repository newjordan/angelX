//! The hillside works in 3D: what the look dyes of the lines laid on the ground.

use super::*;
use crate::world::hillside::Way4;

fn road(tiles: &[(i16, i16)]) -> Vec<Way4> {
    tiles.iter().map(|&tile| Way4 { tile, links: 0 }).collect()
}

#[test]
fn a_path_is_split_into_its_straight_runs_corners_in_both() {
    let path = [(0, 0), (1, 0), (2, 0), (2, 1), (2, 2), (3, 2)];
    assert_eq!(runs(&path), vec![[0, 0, 2, 0], [2, 0, 2, 2], [2, 2, 3, 2]]);
    assert_eq!(runs(&[(5, 5), (6, 5)]), vec![[5, 5, 6, 5]]);
    assert!(runs(&[(1, 1)]).is_empty());
}

#[test]
fn the_roads_about_the_keep_are_runs_and_the_far_ones_are_left_plain() {
    let keep = [119, 67, 146, 94];
    // A road from the gate south, a street across it, and a road far off.
    let mut tiles: Vec<(i16, i16)> = (95..=105).map(|y| (133, y)).collect();
    tiles.extend((136..=142).map(|x| (x, 100)));
    tiles.extend((20..=30).map(|x| (x, 20)));
    tiles.push((150, 110));
    let runs = road_runs(&road(&tiles), keep);
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert_eq!(runs[0], [133, 95, 133, 105], "the longest first");
    assert_eq!(runs[1], [136, 100, 142, 100]);
    // Two tiles are not a run; a cap on how many.
    assert!(road_runs(&road(&[(130, 96), (131, 96)]), keep).is_empty());
    let many: Vec<(i16, i16)> = (0..20).flat_map(|k| (125..=131).map(move |x| (x, 96 + k))).collect();
    assert!(road_runs(&road(&many), keep).len() <= ROAD_RUNS);
}

// ------------------------------------------------------------- baking ----

use crate::kingdom::{Command, catalog};
use crate::rules::Run;
use angel_stage::set::Kind;

fn works() -> Run {
    let c = catalog();
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(c, 7);
    let k = &mut run.home.industry;
    for r in c.resources.keys() {
        k.stock.insert(r.clone(), 600);
    }
    k.researched = c.all_techs().map(|(k, _)| k.clone()).collect();
    for (plan, tile) in [("sawpit", (143, 100)), ("sawpit", (150, 100)), ("well", (143, 106)), ("cottage", (150, 106)), ("woodsman-camp", (157, 100)), ("carters-yard", (143, 112))] {
        k.apply(c, Command::Build { plan: plan.into(), tile }).unwrap_or_else(|e| panic!("{plan}: {e}"));
        let id = k.at(tile).expect("it stands");
        k.buildings.get_mut(&id).expect("it stands").construction = 0;
        let _ = k.apply(c, Command::Connect(id));
    }
    for _ in 0..8 {
        run.step(&std::collections::BTreeMap::new());
    }
    run
}

const WHOLE: [f32; 4] = [0.0, 0.0, 4000.0, 3000.0];

#[test]
fn a_works_is_a_few_meshes_not_a_node_a_block() {
    let run = works();
    let baked = of(&run, WHOLE, None).expect("the hillside");
    let nodes = legacy_nodes(&run, WHOLE);
    assert!(nodes.len() > 300, "{} blocks", nodes.len());
    // A mesh a building (two where something shines in it), a few for the roads, none a block.
    assert!(baked.parts.len() * 8 < nodes.len(), "{} meshes for {} blocks", baked.parts.len(), nodes.len());
    for p in &baked.parts {
        assert_eq!(p.baked.mesh.validate(), Ok(()), "{:?}", p.baked.role);
        let n = p.baked.mesh.positions.len();
        assert!(p.baked.mesh.normals.len() == n && p.baked.mesh.colors.len() == n && p.baked.mesh.uvs.len() == n);
        assert!(p.baked.mesh.normals.iter().all(|v| ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() - 1.0).abs() < 1e-3), "unit normals");
    }
    let mut ids: Vec<u32> = baked.parts.iter().map(|p| p.baked.id).collect();
    ids.sort_unstable();
    ids.dedup();
    // Two sawpits of one size share a mesh: fewer meshes than parts.
    assert!(ids.len() < baked.parts.len(), "{} ids for {} parts", ids.len(), baked.parts.len());
}

#[test]
fn what_is_baked_fills_the_box_the_blocks_filled() {
    let run = works();
    let baked = of(&run, WHOLE, None).expect("the hillside");
    let nodes = legacy_nodes(&run, WHOLE);
    // The box round every block, the way a node places it, against the box round every mesh.
    let extent = |lo: &mut [f32; 3], hi: &mut [f32; 3], p: [f32; 3]| {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    };
    let (mut nlo, mut nhi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for n in &nodes {
        if let Some(unit) = n.mesh.and_then(|m| m.generate()) {
            for v in &unit.positions {
                let s = [v[0] * n.transform.scale[0], v[1] * n.transform.scale[1], v[2] * n.transform.scale[2]];
                let r = angel_stage::scene::quat_rotate(n.transform.rotation, s);
                extent(&mut nlo, &mut nhi, [r[0] + n.transform.translation[0], r[1] + n.transform.translation[1], r[2] + n.transform.translation[2]]);
            }
        }
    }
    let (mut blo, mut bhi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &baked.parts {
        let (lo, hi) = p.bounds();
        extent(&mut blo, &mut bhi, lo);
        extent(&mut blo, &mut bhi, hi);
    }
    for k in 0..3 {
        assert!((nlo[k] - blo[k]).abs() < 0.5 && (nhi[k] - bhi[k]).abs() < 0.5, "axis {k}: blocks {}..{}, meshes {}..{}", nlo[k], nhi[k], blo[k], bhi[k]);
    }
}

#[test]
fn a_mesh_is_kept_until_what_is_in_it_changes() {
    let run = works();
    let a = of(&run, WHOLE, None).expect("the hillside");
    let b = of(&run, WHOLE, None).expect("the hillside");
    let ids = |h: &HillNodes| h.parts.iter().map(|p| p.baked.id).collect::<Vec<_>>();
    assert_eq!(ids(&a), ids(&b), "the same frame again makes nothing");
    // The view moving over the same works makes nothing either: a chunk's roads are the chunk's.
    let c = of(&run, [100.0, 100.0, 3900.0, 2900.0], None).expect("the hillside");
    assert_eq!(ids(&a).len(), ids(&c).len());
    assert_eq!(ids(&a), ids(&c));
    // A road laid in a chunk is a new mesh for that chunk, the rest are the ones they were.
    let mut run = run;
    let k = &mut run.home.industry;
    let before: std::collections::BTreeSet<u32> = ids(&a).into_iter().collect();
    let laid = [(146, 108), (160, 118), (165, 100), (170, 108), (175, 112)].into_iter().find(|t| k.apply(catalog(), Command::Road(*t)).is_ok());
    assert!(laid.is_some(), "a road went down somewhere");
    let d = of(&run, WHOLE, None).expect("the hillside");
    let after: std::collections::BTreeSet<u32> = ids(&d).into_iter().collect();
    let fresh = after.difference(&before).count();
    assert!(fresh >= 1 && fresh <= 2, "{fresh} new meshes for one tile of road");
    assert!(after.intersection(&before).count() >= before.len() / 2, "the rest are kept");
}

#[test]
fn a_barge_lies_in_the_timber_ramp_and_a_carrier_far_off_has_fewer_blocks() {
    let near = vehicle(LineKind::Cart, flat::lines::Dir::East, Some(flat::Load::Ashlar), 0, true);
    let far = vehicle(LineKind::Cart, flat::lines::Dir::East, Some(flat::Load::Ashlar), 0, false);
    let tris = |v: &[Arc<Baked>]| v.iter().map(|b| b.mesh.indices.len() / 3).sum::<usize>();
    assert!(tris(&far) < tris(&near), "{} far against {} near triangles", tris(&far), tris(&near));
    assert_eq!(crate::world3d::works_look::material_of("barge-hull"), crate::world3d::works_look::Material::Timber);
}

#[test]
fn a_block_that_shines_and_water_are_meshes_of_their_own() {
    let solids = [
        Solid::new(super::super::blockout::Shape::Box, Kind::Stone, [8.0, 0.0, 8.0], [6.0, 6.0, 6.0]),
        Solid::new(super::super::blockout::Shape::Box, Kind::Fire, [8.0, 6.0, 8.0], [2.0, 2.0, 2.0]),
        Solid::new(super::super::blockout::Shape::Box, Kind::Water, [20.0, 0.0, 8.0], [8.0, 0.4, 8.0]),
    ];
    let baked = hill_mesh::bake(&solids, [100.0, 0.0, 200.0], |t| t);
    let roles: Vec<_> = baked.iter().map(|b| b.role).collect();
    assert_eq!(roles, vec![hill_mesh::PartRole::Plain, hill_mesh::PartRole::Water, hill_mesh::PartRole::Accent]);
    // The stone's tone is its vertices' colour; the fire's is its own, to the pixel.
    let tone = Kind::Stone.tone();
    assert!(baked[0].mesh.colors.iter().all(|c| (c[0] - tone).abs() < 1e-6 && c[3] == 1.0));
    let [r, g, b] = Kind::Fire.capture();
    assert!(baked[2].mesh.colors.iter().all(|c| (c[0] - f32::from(r) / 255.0).abs() < 1e-6 && (c[1] - f32::from(g) / 255.0).abs() < 1e-6 && (c[2] - f32::from(b) / 255.0).abs() < 1e-6));
    // Shifted where it was told: the stone stands 6 wide about x 108.
    let (lo, hi) = baked[0].bounds;
    assert!((lo[0] - 105.0).abs() < 1e-3 && (hi[0] - 111.0).abs() < 1e-3 && (lo[2] - 205.0).abs() < 1e-3 && (hi[2] - 211.0).abs() < 1e-3, "{lo:?} {hi:?}");
    assert!(hi[1] > 5.9 && hi[1] < 6.1);
}

#[test]
fn the_rail_network_lies_as_track_and_its_wagons_run_on_it() {
    use crate::kingdom::{Act, Stop};
    let c = catalog();
    let mut run = works();
    let plain = of(&run, WHOLE, None).expect("the hillside").parts.len();
    let k = &mut run.home.industry;
    for tile in [(180, 108), (196, 108)] {
        k.apply(c, Command::Build { plan: "depot".into(), tile }).expect("a depot");
        let id = k.at(tile).expect("it stands");
        k.buildings.get_mut(&id).expect("it stands").construction = 0;
    }
    for x in 183..=195 {
        k.apply(c, Command::Rail((x, 108))).unwrap_or_else(|e| panic!("rail at {x}: {e}"));
    }
    k.apply(c, Command::Bay { tile: (183, 108), name: Some("West".into()) }).expect("a bay");
    k.apply(c, Command::Bay { tile: (195, 108), name: Some("East".into()) }).expect("a bay");
    let stops = vec![Stop { bay: "West".into(), act: Act::Load, good: None }, Stop { bay: "East".into(), act: Act::Unload, good: None }];
    k.apply(c, Command::Route { name: None, stops, wagons: 1 }).expect("a route");
    run.step(&std::collections::BTreeMap::new());
    let hv = crate::world::hillside::of(&run).expect("a view");
    assert!(hv.rails.len() >= 13 && hv.wagons.len() == 1, "the view has the track and the wagon");
    let laid = of(&run, WHOLE, None).expect("the hillside");
    assert!(laid.parts.len() > plain + 1, "the track and the wagon are meshes: {} against {plain}", laid.parts.len());
    let (lo, hi) = laid.parts.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
        let (a, b) = p.bounds();
        (std::array::from_fn(|k| lo[k].min(a[k])), std::array::from_fn(|k| hi[k].max(b[k])))
    });
    assert!(lo[0] <= 183.0 * 16.0 && hi[0] >= 195.0 * 16.0, "the track spans its tiles: {lo:?} {hi:?}");
}

#[test]
fn the_runs_of_a_set_of_tiles_are_its_straight_stretches_the_longest_first() {
    use std::collections::BTreeSet;
    let set: BTreeSet<(i16, i16)> = [(0, 0), (1, 0), (2, 0), (3, 0), (5, 5), (2, 1), (2, 2)].into_iter().collect();
    let runs = tile_runs(&set, 10);
    assert_eq!(runs[0], [0, 0, 3, 0], "the longest row first");
    assert!(runs.contains(&[2, 0, 2, 2]), "and the column that crosses it");
    assert!(runs.contains(&[5, 5, 5, 5]), "a lone tile is a run of its own");
    assert_eq!(tile_runs(&set, 1).len(), 1);
}

#[test]
fn a_goods_place_on_a_tile_hides_what_has_not_come_in_and_what_a_house_covers() {
    use crate::kingdom::Piece;
    let belt = |piece| hill::Belt { tile: (0, 0), piece, dir: 1, slots: [None; 3], across: [None; 3], moved: 4, crewed: false, stuck: 0 };
    // On a plain roller the lead good is a third of the way along, and one that has not come in yet is not drawn.
    let (x, y, z) = good_place(&belt(Piece::Roller), 1, 2, 0.0, true).expect("the lead slot");
    assert!(x > 10.0 && (z - 8.0).abs() < 0.1 && y > 3.0);
    assert!(good_place(&belt(Piece::Roller), 1, 0, 1.0, true).is_none(), "a good still in the tile behind");
    assert!(good_place(&belt(Piece::Splitter), 1, 1, 0.0, true).is_none(), "a house hides its middle");
    // A trestle's raised line carries its goods higher than the one under it, and a chain lower than a roller.
    let (_, high, _) = good_place(&belt(Piece::Trestle), 1, 2, 0.0, true).unwrap();
    let (_, low, _) = good_place(&belt(Piece::Trestle), 2, 2, 0.0, false).unwrap();
    let (_, chain, _) = good_place(&belt(Piece::Chain), 1, 2, 0.0, true).unwrap();
    assert!(high > low && low > chain);
}

#[test]
fn a_rail_tiles_switch_bay_and_throw_are_one_number_and_back() {
    let bay = hill::RailBay { name: "Quarry".into(), side: 3, held: true };
    let r = hill::RailTile { tile: (0, 0), links: 15, switch: true, throw: Some(2), bay: Some(bay) };
    let e = rail_extra(&r);
    assert_ne!(e, 0);
    assert_eq!(e & 1, 1, "a switch");
    assert_eq!((e >> 1) & 7, 3, "thrown south (the way plus one)");
    assert_eq!((e >> 5) & 3, 3, "a bay on the west side");
    assert_eq!((e >> 7) & 1, 1, "worked by a wagon");
    assert!(!rail_solids(e).is_empty());
    let plain = hill::RailTile { tile: (0, 0), links: 5, switch: false, throw: None, bay: None };
    assert_eq!(rail_extra(&plain), 0, "plain track adds nothing");
}
