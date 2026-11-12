//! The Works' 3D kit: every plan a piece in every state, inside its
//! footprint, moving when it works, cheap enough for a hundred on a screen.

use super::*;
use crate::render::overworld::works_kit::{self as flat, lines};

fn states() -> Vec<State> {
    let mut v: Vec<State> = (0..STEPS).map(State::Building).collect();
    v.extend([State::Idle, State::Working, State::Starved(Load::Timber), State::BackedUp(Load::Stone)]);
    v
}

#[test]
fn every_flat_plan_has_a_piece_in_every_state() {
    for id in flat::plans() {
        for state in states() {
            for frame in 0..state.frames() {
                let piece = solids(id, state, frame).unwrap_or_else(|| panic!("{id} {state:?}"));
                assert!(piece.solids.len() >= 4, "{id} {state:?} has {} solids", piece.solids.len());
            }
        }
    }
    assert!(solids("no-such-plan", State::Idle, 0).is_none());
}

#[test]
fn a_piece_stays_over_its_footprint_and_off_the_ground_below() {
    let mut faults = Vec::new();
    for id in flat::plans() {
        let (w, d) = extent(id).unwrap();
        for state in states() {
            for frame in 0..state.frames() {
                for s in &solids(id, state, frame).unwrap().solids {
                    let [x0, z0, x1, z1] = s.footprint();
                    // Smoke drifts and sails sweep a little past the edge; a building does not.
                    let slack = if s.kind == Kind::Plaster && matches!(s.shape, Shape::Sphere) { 8.0 } else { 3.0 };
                    if !(x0 >= -slack && z0 >= -slack && x1 <= w + slack && z1 <= d + slack) {
                        faults.push(format!("{id} {state:?}: {:?} {:?} outside {w} x {d}", s.shape, s.at));
                    }
                    if s.bottom() < -0.01 {
                        faults.push(format!("{id} {state:?} below ground: {:?} {:?}", s.shape, s.at));
                    }
                    assert!(s.size.iter().all(|v| *v >= 0.0 && v.is_finite()), "{id} {s:?}");
                }
            }
        }
    }
    faults.dedup();
    assert!(faults.is_empty(), "{} faults:\n{}", faults.len(), { let mut ids: Vec<String> = faults.iter().map(|f| f.split(':').next().unwrap_or("").to_string()).collect(); ids.dedup(); ids.join("\n") });
}

#[test]
fn a_working_piece_moves_loops_and_differs_from_idle() {
    for id in flat::plans() {
        let frames: Vec<_> = (0..FRAMES).map(|f| solids(id, State::Working, f).unwrap()).collect();
        let same = |a: &Piece, b: &Piece| a.solids == b.solids;
        assert!(frames.iter().any(|f| !same(f, &frames[0])), "{id} does not move when it works");
        assert!(same(&solids(id, State::Working, FRAMES).unwrap(), &frames[0]), "{id} does not loop");
        assert!(!same(&solids(id, State::Idle, 0).unwrap(), &frames[0]), "{id} looks the same working and idle");
    }
}

#[test]
fn states_read_differently_and_raising_climbs() {
    for id in flat::plans() {
        let idle = solids(id, State::Idle, 0).unwrap();
        let starved = solids(id, State::Starved(Load::Lime), 0).unwrap();
        let backed = solids(id, State::BackedUp(Load::Lime), 0).unwrap();
        assert!(idle.solids != starved.solids && idle.solids != backed.solids && starved.solids != backed.solids, "{id}");
        let tops: Vec<f32> = (0..STEPS)
            .map(|s| solids(id, State::Building(s), 0).unwrap().solids.iter().map(|x| x.top()).fold(0.0, f32::max))
            .collect();
        assert!(tops.windows(2).all(|w| w[0] <= w[1] + 14.0), "{id} {tops:?}");
        assert!(tops[1] < tops[3] + 1.0 && tops[0] < 12.0, "{id} {tops:?}");
    }
}

#[test]
fn working_fires_light_the_ground_and_idle_ones_do_not() {
    let mut lit = 0;
    for id in flat::plans() {
        if solids(id, State::Working, 3).unwrap().solids.iter().any(|s| s.kind == Kind::Fire) {
            lit += 1;
            assert!(!solids(id, State::Working, 3).unwrap().lamps.is_empty(), "{id} burns with no light");
            assert!(solids(id, State::Idle, 0).unwrap().solids.iter().all(|s| s.kind != Kind::Fire), "{id} burns when idle");
        }
    }
    assert!(lit >= 12, "only {lit} plans burn");
}

#[test]
fn standing_a_piece_keeps_it_on_its_footprint_whichever_way_it_faces() {
    let piece = solids("sawpit", State::Idle, 0).unwrap();
    for face in [Face::South, Face::East, Face::North, Face::West] {
        let foot = [160.0, 96.0, 160.0 + 48.0, 96.0 + 32.0];
        let (fw, fd) = (48.0f32, 32.0f32);
        let (w, d) = if matches!(face, Face::South | Face::North) { (fw, fd) } else { (fd, fw) };
        let at = stand(&piece, [foot[0], foot[1], foot[0] + w, foot[1] + d], face);
        for s in &at.solids {
            let [x0, z0, x1, z1] = s.footprint();
            assert!(x0 >= foot[0] - 3.0 && x1 <= foot[0] + w + 3.0 && z0 >= foot[1] - 3.0 && z1 <= foot[1] + d + 3.0, "{face:?}: {s:?}");
        }
    }
}

#[test]
fn lines_and_carriers_build() {
    for kind in LineKind::ALL {
        for links in 0..16u8 {
            for post in [false, true] {
                let v = line_solids(kind, links, post, 2);
                assert!(!v.is_empty(), "{kind:?} {links}");
                for s in v.iter() {
                    let [x0, z0, x1, z1] = s.footprint();
                    assert!(x0 >= -3.0 && z0 >= -3.0 && x1 <= 19.0 && z1 <= 19.0, "{kind:?} {links}: {s:?}");
                }
            }
        }
        for dir in [Dir::East, Dir::West, Dir::North, Dir::South] {
            for load in std::iter::once(None).chain(Load::ALL.iter().map(|l| Some(*l))) {
                for f in [0, 3] {
                    assert!(!vehicle_solids(kind, dir, load, f).is_empty(), "{kind:?} {dir:?} {load:?}");
                }
            }
        }
    }
    let _ = lines::TILE_W;
}

#[test]
fn every_load_is_its_own_heap() {
    let mut seen: Vec<(Load, Vec<Solid>)> = Vec::new();
    for load in Load::ALL {
        let v = load_solids(load, 0.0, 0.0, 14.0, 0.0, 8.0, 1);
        assert!(!v.is_empty(), "{load:?}");
        for (other, o) in &seen {
            assert!(*o != v, "{load:?} and {other:?} are the same heap");
        }
        seen.push((load, v));
    }
}

#[test]
fn a_hundred_pieces_are_cheap() {
    use std::time::Instant;
    // Made once: the first sight of each (plan, state, frame) costs; every later one is a lookup.
    let ids: Vec<&str> = flat::plans().collect();
    let started = Instant::now();
    let mut count = 0;
    for k in 0..100 {
        let id = ids[k % ids.len()];
        let piece = solids(id, State::Working, (k % 8) as u32).unwrap();
        count += piece.solids.len();
    }
    let first = started.elapsed();
    let started = Instant::now();
    for k in 0..100 {
        let id = ids[k % ids.len()];
        let piece = solids(id, State::Working, (k % 8) as u32).unwrap();
        count += piece.solids.len();
    }
    let again = started.elapsed();
    assert!(count > 0);
    println!("works_kit 3D: 100 pieces first sight {first:?}, again {again:?}, {} solids a piece on average", count / 200);
    assert!(again.as_millis() < 20, "{again:?}");
}


#[test]
fn rails_lie_on_sleepers_on_ballast_and_a_line_ends_in_a_buffer() {
    use angel_stage::set::Kind;
    // A straight run east to west: ballast, sleepers across it, two rails along it.
    let straight = line_solids(LineKind::Tram, lines::E | lines::W, false, 0);
    let count = |v: &[Solid], k: Kind| v.iter().filter(|s| s.kind == k).count();
    assert_eq!(count(&straight, Kind::Sand), 3, "the bed: the middle and an arm each way");
    assert_eq!(count(&straight, Kind::Trunk), 4, "two sleepers an arm");
    assert_eq!(count(&straight, Kind::Road), 4, "a rail in each arm, two rails");
    // Rails stand over the sleepers, the sleepers over the ballast.
    let top = |k: Kind| straight.iter().filter(|s| s.kind == k).map(|s| s.top()).fold(0.0f32, f32::max);
    assert!(top(Kind::Road) > top(Kind::Trunk) && top(Kind::Trunk) > top(Kind::Sand));
    // Sleepers run across the line (their long side is z when the line runs along x).
    assert!(straight.iter().filter(|s| s.kind == Kind::Trunk).all(|s| s.size[2] > s.size[0]));
    // A dead end has its buffer; a through tile does not.
    let end = line_solids(LineKind::Tram, lines::W, false, 0);
    assert!(end.iter().any(|s| s.kind == Kind::Trunk && s.size[1] > 3.0), "a buffer post");
    assert!(!straight.iter().any(|s| s.kind == Kind::Trunk && s.size[1] > 3.0));
    // A corner mitres: both arms' rails reach into the middle.
    let corner = line_solids(LineKind::Tram, lines::E | lines::S, false, 0);
    assert!(corner.iter().filter(|s| s.kind == Kind::Road).count() >= 4);
}

#[test]
fn a_ropeway_cable_sags_between_towers_and_a_tower_stands_over_it() {
    use angel_stage::set::Kind;
    // The cable's mean height over a tile of the span.
    let cable_at = |span: u32| {
        let tops: Vec<f32> = line_solids_at(LineKind::Rope, lines::E | lines::W, false, span, 0).iter().filter(|s| s.kind == Kind::Road).map(|s| s.top()).collect();
        tops.iter().sum::<f32>() / tops.len() as f32
    };
    // From a tower the cable falls, is lowest mid-span and rises to the next.
    assert!(cable_at(0) > cable_at(1) && cable_at(1) > cable_at(2), "{} {} {}", cable_at(0), cable_at(1), cable_at(2));
    assert!(cable_at(3) > cable_at(2) && (cable_at(3) - cable_at(1)).abs() < 0.01, "a span is the same both ways");
    let tower = line_solids_at(LineKind::Rope, lines::E | lines::W, true, 0, 0);
    let high = tower.iter().map(|s| s.top()).fold(0.0f32, f32::max);
    assert!(high >= rails::CABLE - 1.0, "a tower reaches the cable: {high}");
    // A bucket hangs from the cable, clear of the ground.
    let bucket = vehicle_solids(LineKind::Rope, Dir::East, Some(Load::Ashlar), 0);
    let low = bucket.iter().map(|s| s.bottom()).fold(f32::MAX, f32::min);
    assert!(low > 12.0 && low < rails::CABLE, "{low}");
}

#[test]
fn an_ox_walks_and_a_cart_wheel_turns() {
    // The ox's legs are not the same two steps running, and a wheel's marked block has moved.
    let (a, b) = (vehicle_solids(LineKind::Cart, Dir::East, None, 0), vehicle_solids(LineKind::Cart, Dir::East, None, 2));
    assert_ne!(*a, *b, "a step later the cart is not the same picture");
    assert_eq!(a.len(), b.len());
    // Four legs, two raised and two down at a time.
    let legs = |v: &[Solid]| v.iter().filter(|s| s.kind == angel_stage::set::Kind::Trunk && s.size[1] > 3.0 && s.at[0] > 18.0).map(|s| s.at[1]).collect::<Vec<_>>();
    let (la, lb) = (legs(&a), legs(&b));
    assert_eq!(la.len(), 4);
    assert_eq!(la.iter().filter(|&&y| y > 0.5).count(), 2);
    assert_eq!(lb.iter().filter(|&&y| y > 0.5).count(), 2);
    assert_ne!(la, lb, "the pairs swap");
    // Stone, lime, timber and ore on a cart are four different heaps.
    let heaps: Vec<_> = [Load::Stone, Load::Lime, Load::Timber, Load::Ore].iter().map(|l| vehicle_solids(LineKind::Cart, Dir::East, Some(*l), 0)).collect();
    for i in 0..4 {
        for j in i + 1..4 {
            assert_ne!(*heaps[i], *heaps[j]);
        }
    }
}

#[test]
fn a_carrier_seen_from_afar_is_a_few_blocks_of_the_same_silhouette() {
    for kind in LineKind::ALL {
        for load in [None, Some(Load::Ashlar)] {
            let (near, far) = (vehicle_solids_lod(kind, Dir::East, load, 0, true), vehicle_solids_lod(kind, Dir::East, load, 0, false));
            assert_eq!(*near, *vehicle_solids(kind, Dir::East, load, 0));
            assert!(far.len() * 2 <= near.len() + 8, "{kind:?} {load:?}: {} far of {} near", far.len(), near.len());
            let reach = |v: &[Solid]| v.iter().fold((f32::MAX, f32::MIN, 0.0f32), |(lo, hi, top), s| (lo.min(s.at[0] - s.size[0] / 2.0), hi.max(s.at[0] + s.size[0] / 2.0), top.max(s.top())));
            let (n, f) = (reach(&near), reach(&far));
            assert!((n.0 - f.0).abs() < 3.0 && (n.1 - f.1).abs() < 3.0, "{kind:?}: length {n:?} vs {f:?}");
            assert!((n.2 - f.2).abs() < 3.5, "{kind:?}: height {n:?} vs {f:?}");
        }
    }
}

// ------------------------------------------------- the line, the rails, the swape ----

#[test]
fn every_piece_of_rollerway_builds_every_way_and_keeps_to_its_tile() {
    use crate::kingdom::Piece;
    for piece in Piece::ALL {
        for dir in 0..4u8 {
            for phase in [None, Some(0), Some(3)] {
                let solids = belts::tile(piece, dir, phase, true);
                assert!(solids.len() >= 6, "{piece:?} {dir}: {} solids", solids.len());
                for s in solids.iter() {
                    let [x0, z0, x1, z1] = s.footprint();
                    assert!(x0 >= -0.5 && z0 >= -0.5 && x1 <= 16.5 && z1 <= 16.5, "{piece:?} facing {dir}: {s:?} is off its tile");
                    assert!(s.bottom() >= 0.0 && s.size.iter().all(|v| v.is_finite() && *v >= 0.0));
                }
            }
        }
    }
}

#[test]
fn rollers_turn_and_a_stood_line_stands_and_the_ways_differ() {
    use crate::kingdom::Piece;
    let still = belts::tile(Piece::Roller, 1, None, false);
    let steps: Vec<_> = (0..4u8).map(|p| belts::tile(Piece::Roller, 1, Some(p), false)).collect();
    assert!(steps.iter().any(|s| **s != *steps[0]), "the lugs ride round the logs");
    assert!(steps.iter().any(|s| **s != *still), "a moving tile differs from a stood one");
    // North and east are the same tile turned: not the same blocks.
    assert_ne!(*belts::tile(Piece::Roller, 0, None, false), *belts::tile(Piece::Roller, 1, None, false));
    // The hand-chain with its labourer is not the one with its stool, and he moves at work.
    assert_ne!(*belts::tile(Piece::Chain, 1, Some(0), true), *belts::tile(Piece::Chain, 1, Some(0), false));
    assert_ne!(*belts::tile(Piece::Chain, 1, Some(0), true), *belts::tile(Piece::Chain, 1, Some(2), true));
    // Splitter, merge and gate are three houses.
    let houses: Vec<_> = [Piece::Splitter, Piece::Merge, Piece::Gate].iter().map(|p| belts::tile(*p, 1, None, false)).collect();
    assert!(houses[0] != houses[1] && houses[1] != houses[2] && houses[0] != houses[2]);
}

#[test]
fn each_good_on_the_line_is_its_own_crate_and_lies_the_way_it_goes() {
    let ashlar = belts::good(Load::Ashlar, true);
    assert_ne!(*ashlar, *belts::good(Load::Timber, true));
    assert_ne!(*ashlar, *belts::good(Load::Ashlar, false));
    for s in ashlar.iter() {
        assert!(s.bottom() >= 0.0);
    }
}

#[test]
fn a_switch_has_its_lever_thrown_toward_the_way_and_a_bay_its_platform_on_the_buildings_side() {
    let upright = belts::rail_extras(true, None, None);
    let east = belts::rail_extras(true, Some(1), None);
    let south = belts::rail_extras(true, Some(2), None);
    assert!(upright != east && east != south, "the lever and the blade lean to the way thrown");
    assert!(belts::rail_extras(false, None, None).is_empty(), "plain track has neither");
    let bay = |side, held| belts::rail_extras(false, None, Some((side, held, 5)));
    assert_ne!(bay(1, false), bay(3, false), "the platform is on the side of the building");
    assert_ne!(bay(1, false), bay(1, true), "the lantern is lit while a wagon works the bay");
    assert_ne!(belts::rail_extras(false, None, Some((1, false, 1))), belts::rail_extras(false, None, Some((1, false, 6))), "its board carries a mark of its name");
    assert!(bay(1, true).iter().any(|s| s.kind == Kind::Window), "a lit lantern shines");
}

#[test]
fn a_swape_turns_to_the_way_it_faces_and_its_arm_swings_at_work() {
    let rest = solids("swape", State::Idle, 0).unwrap();
    let swing: Vec<_> = (0..FRAMES).map(|f| solids("swape", State::Working, f).unwrap()).collect();
    assert!(swing.iter().any(|s| s.solids != swing[0].solids), "the arm moves");
    assert!(swing.iter().any(|s| s.solids != rest.solids), "working is not resting");
    let foot = [0.0, 0.0, 16.0, 16.0];
    let south = stand(&rest, foot, Face::South);
    let east = stand(&rest, foot, Face::East);
    assert_ne!(south.solids, east.solids, "facing a different way is a different place for the arm");
}

#[test]
fn the_buildings_the_first_kit_left_undrawn_are_drawn_at_the_size_the_sim_gives_them() {
    for (id, tiles) in [("hod-gang", (1, 1)), ("great-crane", (1, 1)), ("vineyard", (1, 1)), ("wine-press", (1, 1)), ("aqueduct", (7, 1)), ("swape", (1, 1))] {
        assert_eq!(flat::size(id), Some(tiles), "{id}");
        assert!(solids(id, State::Idle, 0).is_some(), "{id} has a 3D piece");
        assert!(flat::flat(id, State::Working, 3).is_some(), "{id} has a flat picture");
    }
    // A mule train is its own look, not a cart's.
    let mule = vehicle_solids(LineKind::Mule, Dir::East, Some(Load::Ore), 0);
    assert_ne!(*mule, *vehicle_solids(LineKind::Cart, Dir::East, Some(Load::Ore), 0));
    assert_ne!(*line_solids(LineKind::Mule, 10, false, 0), *line_solids(LineKind::Cart, 10, false, 0));
}
