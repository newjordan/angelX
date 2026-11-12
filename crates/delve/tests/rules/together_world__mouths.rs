// The ways down from the kingdom: each listed, on the map, reached by road,
// with a halt near it.

#[test]
fn every_entrance_that_goes_down_is_a_way_down_the_kingdom_marks() {
    for e in ENTRANCES.iter().filter(|e| matches!(e.to, Dest::Delve(_)) || e.id == "gate") {
        assert!(MOUTHS.iter().any(|m| m.entrance == e.id), "{} goes down but is no mouth", e.id);
    }
    for m in &MOUTHS {
        let e = ENTRANCES.iter().find(|e| e.id == m.entrance).unwrap_or_else(|| panic!("no entrance {}", m.entrance));
        assert_eq!(e.room, m.room, "{}", m.name);
        match e.to {
            Dest::Delve(pack) => assert_eq!(pack, m.below, "{}", m.name),
            Dest::Room(RoomKind::Home, _) => assert_eq!(m.below, Pack::Crypt, "{}", m.name),
            other => panic!("{} does not go down: {other:?}", m.name),
        }
    }
}

#[test]
fn every_way_down_stands_on_the_map_by_a_road_from_the_keep_with_a_halt_near_and_no_two_share_a_door() {
    use crate::world::{kingdom as land, map, waystations};
    let geo = land::geography(&[]);
    let keep = land::keep_stand();
    let n = (map::MAP_W * map::MAP_H) as usize;
    let idx = |(x, y): (i32, i32)| (y * map::MAP_W + x) as usize;
    let mut seen = vec![false; n];
    let mut queue = std::collections::VecDeque::from([keep]);
    seen[idx(keep)] = true;
    while let Some((x, y)) = queue.pop_front() {
        for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if land::is_road(&geo, next) && !seen[idx(next)] {
                seen[idx(next)] = true;
                queue.push_back(next);
            }
        }
    }
    let mut stands = Vec::new();
    for m in &MOUTHS {
        let site = geo.sites.iter().find(|s| s.room == m.room).unwrap_or_else(|| panic!("{} has no door on the map", m.name));
        assert!(seen[idx(site.stand)], "no road from the Keep to {}'s door at {:?}", m.name, site.stand);
        let near = waystations::placed().iter().map(|p| (p.keeper.0 - site.stand.0).abs().max((p.keeper.1 - site.stand.1).abs())).min().unwrap();
        assert!(near <= 20, "{}'s nearest halt is {near} tiles off", m.name);
        stands.push(site.stand);
    }
    for (i, a) in stands.iter().enumerate() {
        for b in &stands[i + 1..] {
            assert!((a.0 - b.0).abs().max((a.1 - b.1).abs()) >= 12, "{a:?} and {b:?} stand together");
        }
    }
}
