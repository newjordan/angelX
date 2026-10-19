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
fn an_arm_names_the_biome_down_its_road_or_the_place_it_reaches() {
    let names: Vec<&str> = biome::Biome::ALL.iter().map(|b| b.name()).collect();
    let geo = kingdom::geography(&[]);
    let mut biomes_named = std::collections::BTreeSet::new();
    for p in posts() {
        for (_, arm) in &p.arms {
            assert!(
                names.contains(&arm.as_str()) || arm == "The Delve" || geo.sites.iter().any(|s| &s.name == arm),
                "{arm} is neither a biome nor a place"
            );
            if names.contains(&arm.as_str()) {
                biomes_named.insert(arm.clone());
            }
        }
    }
    // Every biome is signposted from somewhere.
    assert_eq!(biomes_named.len(), names.len(), "signposted: {biomes_named:?}");
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
