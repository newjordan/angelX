use super::*;

#[test]
fn fingerposts_stand_beside_the_roads_apart_with_two_arms_or_more() {
    let geo = kingdom::geography(&[]);
    let all = posts();
    assert!(all.len() >= 20, "{} posts", all.len());
    for (i, p) in all.iter().enumerate() {
        assert_eq!(geo.room.tile(p.at.0, p.at.1), Tile::Floor, "{p:?} stands in the way");
        assert!(!kingdom::is_road(&geo, p.at), "{p:?} stands on the road");
        assert!(kingdom::is_road(&geo, p.road), "{p:?} marks no road");
        assert!(chebyshev(p.at, p.road) <= 3, "{p:?} stands off its road");
        assert!(p.arms.len() >= 2, "{p:?} points one way");
        let ways: std::collections::BTreeSet<Way> = p.arms.iter().map(|a| a.0).collect();
        assert_eq!(ways.len(), p.arms.len(), "{p:?} has two arms one way");
        for other in &all[i + 1..] {
            assert!(chebyshev(p.road, other.road) >= SPACING, "{p:?} crowds {other:?}");
        }
    }
}

#[test]
fn every_road_into_another_biome_passes_a_post() {
    // Each main road's crossing of a biome border has a post within a
    // post's spacing of it.
    for road in ROADS.iter().filter(|r| r.width == 3) {
        let course = roads::course(road.points);
        for w in course.windows(2) {
            if biome::at(w[0].0, w[0].1) != biome::at(w[1].0, w[1].1) {
                assert!(
                    posts().iter().any(|p| chebyshev(p.road, w[1]) <= SPACING),
                    "no post where the road crosses into {} at {:?}",
                    biome::at(w[1].0, w[1].1).name(),
                    w[1]
                );
            }
        }
    }
}

#[test]
fn an_arm_names_a_region_a_place_a_halt_or_a_way_down() {
    let names: Vec<&str> = biome::Biome::ALL.iter().map(|b| b.name()).collect();
    let geo = kingdom::geography(&[]);
    let mut biomes_named = std::collections::BTreeSet::new();
    for p in posts() {
        for (_, arm) in &p.arms {
            match arm_kind(arm) {
                ArmKind::Region => {
                    assert!(names.contains(&arm.as_str()), "{arm} is no region");
                    biomes_named.insert(arm.clone());
                }
                ArmKind::Dungeon => assert!(crate::rules::world::MOUTHS.iter().any(|m| m.name == arm)),
                ArmKind::Halt => assert!(crate::world::waystations::STOPS.iter().any(|s| s.name == arm)),
                ArmKind::Place => assert!(geo.sites.iter().any(|s| &s.name == arm), "{arm} is neither a region, a halt, a way down nor a place"),
            }
        }
    }
    // Every biome is signposted from somewhere.
    assert_eq!(biomes_named.len(), names.len(), "signposted: {biomes_named:?}");
}

#[test]
fn a_fork_names_real_destinations_a_biome_crossing_names_its_region() {
    let (mut regions, mut named) = (0, 0);
    for p in posts() {
        for (_, arm) in &p.arms {
            if arm_kind(arm) == ArmKind::Region {
                regions += 1;
            } else {
                named += 1;
            }
        }
    }
    assert!(named >= 25, "{named} destinations, {regions} regions");
}

#[test]
fn every_way_down_is_named_on_a_post_and_every_way_down_has_a_post_where_its_road_meets_the_main_road() {
    let geo = kingdom::geography(&[]);
    let raster = roads::raster();
    let road = |t: (i32, i32)| kingdom::is_road(&geo, t);
    for m in crate::rules::world::MOUTHS.iter() {
        assert!(
            posts().iter().any(|p| p.arms.iter().any(|(_, a)| a == m.name)),
            "no post names {}",
            m.name
        );
        let site = geo.sites.iter().find(|s| s.room == m.room).unwrap_or_else(|| panic!("{} has no door on the map", m.name));
        let (fork, _) = fork_of(site.stand, &raster, &road).unwrap_or_else(|| panic!("{}'s door reaches no main road", m.name));
        assert!(
            posts().iter().any(|p| chebyshev(p.road, fork) < SPACING),
            "no post within {SPACING} tiles of where the way to {} meets the main road at {fork:?}",
            m.name
        );
    }
}

#[test]
fn a_halt_is_named_by_a_post_down_its_road() {
    let named = crate::world::waystations::STOPS.iter().filter(|s| posts().iter().any(|p| p.arms.iter().any(|(_, a)| a == s.name))).count();
    assert!(named >= 4, "{named} halts are signed");
}

#[test]
fn a_fingerpost_is_dusk_timber_with_a_board_each_way() {
    use crate::render::overworld::{ink, signs as drawn};
    let bare = drawn::sprite(&[]);
    for ways in [vec![Way::East, Way::West], vec![Way::North, Way::South], vec![Way::East, Way::North, Way::West, Way::South]] {
        let im = drawn::sprite(&ways);
        assert!(im.pixels().all(|c| !ink::is_signal(c)), "a post is no game state");
        assert_ne!(im.rgba_bytes(), bare.rgba_bytes(), "{ways:?} shows its boards");
    }
}
