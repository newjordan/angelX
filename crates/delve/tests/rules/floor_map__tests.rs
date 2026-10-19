use super::*;
use fortune::Mode;
use keyring::Door;
use std::collections::BTreeMap;

/// A run on floor `depth` of the Crypt, the party at the entrance, its
/// gates made.
pub(super) fn on_floor(depth: u32, seed: u64) -> Run {
    let mut run = Run::new(seed, 17, None);
    let pack = Pack::at(depth, Pack::Crypt);
    run.dungeon = layout::floor_for(Mode::LongWayDown, depth, pack, &mut run.rng);
    run.populate_boss_gates();
    run.enter(0, None);
    run
}

fn ids(map: &FloorMap) -> Vec<usize> {
    map.nodes.iter().map(|n| n.id).collect()
}

fn doorways(run: &Run, room: usize) -> Vec<usize> {
    (0..4)
        .filter_map(|d| run.dungeon.neighbour(room, d))
        .collect()
}

/// Every room visited: the whole floor known (but for a vault unfound).
fn visit_all(run: &mut Run) {
    for room in &mut run.dungeon.rooms {
        room.visited = true;
    }
}

#[test]
fn the_party_stands_in_a_visited_room_and_the_doorways_show_what_lies_beyond() {
    for seed in 1..=12 {
        for depth in 1..=4 {
            let run = on_floor(depth, seed);
            let map = run.floor_map(Some(1));
            let beyond = doorways(&run, 0);
            assert!(!beyond.is_empty());
            let home = map.node(0).expect("the room the party stands in");
            assert!(home.state.visited(), "seed {seed} depth {depth}");
            for id in &beyond {
                assert_eq!(
                    map.node(*id).map(|n| n.state),
                    Some(NodeState::Seen),
                    "seed {seed} depth {depth} room {id}"
                );
            }
            // Nothing else: a room not through a doorway of a visited room is
            // not on the map at all.
            assert_eq!(
                map.nodes.len(),
                1 + beyond.len(),
                "seed {seed} depth {depth}"
            );
            assert_eq!(map.here, Some(0));
        }
    }
}

#[test]
fn a_room_seen_is_an_outline_that_hides_what_it_is() {
    let mut run = on_floor(2, 5);
    // The exit room with a treasure, a guardian, whatever: seen from the
    // doorway it is only a place.
    for room in doorways(&run, 0) {
        let node = run.floor_map(Some(1)).node(room).cloned().unwrap();
        assert_eq!(node.state, NodeState::Seen);
        assert_eq!(node.kind, NodeKind::Unknown);
        assert!(node.icons.is_empty());
    }
    // Stood in, it is known.
    let next = doorways(&run, 0)[0];
    run.dungeon.rooms[next].visited = true;
    let node = run.floor_map(Some(1)).node(next).cloned().unwrap();
    assert!(node.state.visited());
    assert_ne!(node.kind, NodeKind::Unknown);
}

#[test]
fn rooms_beyond_the_seen_rooms_stay_hidden_until_one_is_visited() {
    let mut run = on_floor(1, 3);
    let near = doorways(&run, 0);
    // A room two doorways away and not beside the start.
    let far = (0..run.dungeon.rooms.len())
        .find(|i| *i != 0 && !near.contains(i))
        .expect("a floor is more than a room and its neighbours");
    assert!(run.floor_map(Some(1)).node(far).is_none());
    // Visiting a neighbour reveals the doorways of that one.
    let step = near[0];
    run.dungeon.rooms[step].visited = true;
    let map = run.floor_map(Some(1));
    for id in doorways(&run, step) {
        assert!(
            map.node(id).is_some(),
            "room {id} is through a doorway of room {step}"
        );
    }
    // Every node on the map is the party's, seen from it, or through one
    // doorway of a visited room, never further.
    for node in &map.nodes {
        if node.state == NodeState::Seen {
            assert!(
                map.nodes
                    .iter()
                    .any(|v| v.state.visited() && doorways(&run, v.id).contains(&node.id)),
                "room {} is seen through no doorway",
                node.id
            );
        }
    }
}

#[test]
fn a_room_a_knight_has_stood_in_is_visited_even_if_never_marked() {
    let mut run = on_floor(1, 9);
    let beyond = doorways(&run, 0)[0];
    // A knight waiting outside, in a room the floor's flags forgot.
    run.dungeon.rooms[beyond].visited = false;
    run.join_seat(2, "Wren");
    let spot = (6.0, 6.0);
    run.exits.outside.insert(
        2,
        exits::Entry {
            at: beyond,
            party: BTreeMap::from([(1, spot), (2, spot)]),
        },
    );
    let map = run.floor_map(Some(1));
    assert!(map.node(beyond).unwrap().state.visited());
}

#[test]
fn a_secret_vault_is_not_on_the_map_until_its_wall_is_blown() {
    let (mut run, secret) = (1..60)
        .flat_map(|seed| (1..=4).map(move |depth| (depth, seed)))
        .map(|(depth, seed)| on_floor(depth, seed))
        .find_map(|run| run.dungeon.secret.map(|s| (run, s)))
        .expect("some floor hides a vault");
    visit_all(&mut run);
    // Even a vault the floor's flags call visited is no part of the map.
    run.dungeon.rooms[secret.vault].visited = true;
    let map = run.floor_map(Some(1));
    assert!(
        map.node(secret.vault).is_none(),
        "an unfound vault is hidden"
    );
    assert!(
        map.links
            .iter()
            .all(|l| l.a != secret.vault && l.b != secret.vault),
        "and no doorway leads to it"
    );
    assert!(map.node(secret.host).is_some());
    // Blown, it is a doorway of a visited room.
    run.dungeon.rooms[secret.vault].visited = false;
    assert!(run.dungeon.open_secret());
    let map = run.floor_map(Some(1));
    assert_eq!(
        map.node(secret.vault).map(|n| n.state),
        Some(NodeState::Seen)
    );
    assert!(
        map.links
            .iter()
            .any(|l| (l.a, l.b) == (secret.host.min(secret.vault), secret.host.max(secret.vault)))
    );
    // Entered, it is marked a vault.
    run.dungeon.rooms[secret.vault].visited = true;
    let node = run.floor_map(Some(1)).node(secret.vault).cloned().unwrap();
    assert_eq!(node.kind, NodeKind::Vault);
    assert!(node.icons.contains(&Icon::Vault));
}

#[test]
fn the_guardian_and_the_sanctuary_are_marked_once_visited() {
    for seed in 1..=8 {
        let mut run = on_floor(2, seed);
        let gates = run.boss_gates.clone().expect("floor two has gates");
        let map = run.floor_map(Some(1));
        // Floor two opens in a sanctuary.
        assert!(map.node(0).unwrap().icons.contains(&Icon::Sanctuary));
        assert_eq!(map.node(0).unwrap().kind, NodeKind::Sanctuary);
        // The guardian's room, unvisited, is not marked.
        if let Some(node) = map.node(gates.guardian) {
            assert!(node.icons.is_empty(), "a room not stood in shows no icons");
        }
        visit_all(&mut run);
        let map = run.floor_map(Some(1));
        let guardian = map.node(gates.guardian).expect("every room known");
        assert!(guardian.icons.contains(&Icon::Guardian) == (gates.guardian_boss.is_some()));
        assert_eq!(guardian.kind, NodeKind::Stairs);
        assert!(guardian.icons.contains(&Icon::Stairs));
        if gates.guardian_boss.is_some() {
            assert_eq!(guardian.icons[0], Icon::Guardian, "the skull comes first");
        }
        // Once it falls, the skull goes and the stairs stay.
        run.boss_gates.as_mut().unwrap().guardian_defeated = true;
        let after = run
            .floor_map(Some(1))
            .node(gates.guardian)
            .cloned()
            .unwrap();
        assert!(!after.icons.contains(&Icon::Guardian));
        assert!(after.icons.contains(&Icon::Stairs));
    }
}

#[test]
fn the_dragons_lair_and_the_unknown_s_threshold_keep_a_guardian() {
    for depth in [FLOORS, DEEPEST] {
        let mut run = on_floor(depth, 4);
        visit_all(&mut run);
        let gates = run.boss_gates.clone().unwrap();
        let node = run
            .floor_map(Some(1))
            .node(gates.guardian)
            .cloned()
            .unwrap();
        assert_eq!(node.kind, NodeKind::Lair);
        assert_eq!(node.icons.first(), Some(&Icon::Guardian));
    }
}

#[test]
fn route_chiefs_are_marked_only_where_the_party_has_been() {
    let mut run = on_floor(2, 6);
    let chief = run.boss_gates.as_ref().unwrap().leaders[0].room;
    assert!(
        run.floor_map(Some(1))
            .node(chief)
            .is_none_or(|n| !n.icons.contains(&Icon::Chief))
    );
    run.dungeon.rooms[chief].visited = true;
    assert!(
        run.floor_map(Some(1))
            .node(chief)
            .unwrap()
            .icons
            .contains(&Icon::Chief)
    );
    // Slain, it is marked no more.
    run.boss_gates.as_mut().unwrap().leaders[0].defeated = true;
    assert!(
        !run.floor_map(Some(1))
            .node(chief)
            .unwrap()
            .icons
            .contains(&Icon::Chief)
    );
}

#[test]
fn the_boss_door_is_locked_until_the_key_opens_it() {
    let mut run = on_floor(2, 2);
    visit_all(&mut run);
    let guardian = run.boss_gates.as_ref().unwrap().guardian;
    assert!(run.boss_door_shut());
    let map = run.floor_map(Some(1));
    let into_guardian: Vec<&Link> = map
        .links
        .iter()
        .filter(|l| l.a == guardian || l.b == guardian)
        .collect();
    assert!(!into_guardian.is_empty());
    assert!(
        into_guardian.iter().all(|l| l.locked),
        "every way into the guardian's room is chained"
    );
    assert!(
        map.links
            .iter()
            .filter(|l| l.a != guardian && l.b != guardian)
            .all(|l| !l.locked)
    );
    run.works.key.door = Door::Open;
    assert!(run.floor_map(Some(1)).links.iter().all(|l| !l.locked));
}

#[test]
fn the_boss_key_is_marked_where_it_lies_and_on_the_knight_who_carries_it() {
    let mut run = on_floor(2, 7);
    visit_all(&mut run);
    let room = run.boss_gates.as_ref().unwrap().leaders[0].room;
    run.works.key.hold = keyring::Hold::Lying {
        room,
        x: 10.0,
        y: 10.0,
    };
    let map = run.floor_map(Some(1));
    assert!(map.node(room).unwrap().icons.contains(&Icon::Key));
    assert!(map.party.iter().all(|m| !m.key));
    run.works.key.hold = keyring::Hold::Carried {
        knight: 1,
        x: 10.0,
        y: 10.0,
    };
    let map = run.floor_map(Some(1));
    assert!(map.nodes.iter().all(|n| !n.icons.contains(&Icon::Key)));
    assert!(map.own().unwrap().key);
}

#[test]
fn the_peddlers_camp_and_the_treasure_are_marked() {
    for seed in 1..=10 {
        let mut run = on_floor(1, seed);
        visit_all(&mut run);
        let map = run.floor_map(Some(1));
        let treasure = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Treasure)
            .expect("a treasure room");
        assert!(map.node(treasure).unwrap().icons.contains(&Icon::Treasure));
        if let Some(camp) = run.merchant_camp() {
            assert!(map.node(camp.room).unwrap().icons.contains(&Icon::Merchant));
        }
    }
}

#[test]
fn the_side_halls_and_nooks_carry_their_own_icons() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 1..=40 {
        for depth in 1..=5 {
            let mut run = on_floor(depth, seed);
            nooks::add_nook(&mut run.dungeon);
            visit_all(&mut run);
            if let Some(secret) = run.dungeon.secret {
                run.dungeon.rooms[secret.vault].visited = false;
            }
            let map = run.floor_map(Some(1));
            for node in &map.nodes {
                let kind = run.dungeon.rooms[node.id].kind;
                let want = match kind {
                    RoomKind::Pit => Some(Icon::Pit),
                    RoomKind::Wanted => Some(Icon::Wanted),
                    RoomKind::Ledge => Some(Icon::Ledge),
                    k if k.nook() => Some(Icon::Nook),
                    _ => None,
                };
                if let Some(icon) = want {
                    assert!(node.icons.contains(&icon), "{kind:?} on floor {depth}");
                    seen.insert(icon);
                }
            }
        }
    }
    for icon in [Icon::Pit, Icon::Wanted, Icon::Ledge, Icon::Nook] {
        assert!(seen.contains(&icon), "{icon:?} never came up");
    }
}

#[test]
fn absent_seats_are_left_off_and_away_knights_are_dimmed() {
    let mut run = on_floor(1, 11);
    run.join_seat(2, "Wren");
    run.join_seat(3, "Bors");
    run.join_seat(4, "Pell");
    run.set_absent(2, true);
    run.set_away(3, true);
    run.players.get_mut(&4).unwrap().hp = 0;
    let map = run.floor_map(Some(1));
    let knights: Vec<u32> = map.party.iter().map(|m| m.knight).collect();
    assert_eq!(knights, vec![1, 3, 4], "an absent seat is nobody's knight");
    let by = |id: u32| map.party.iter().find(|m| m.knight == id).unwrap();
    assert!(!by(1).away && !by(1).fallen);
    assert!(by(3).away, "away is dimmed");
    assert!(by(4).fallen);
}

#[test]
fn the_seat_s_own_marker_is_marked_and_no_seat_marks_none() {
    let mut run = on_floor(1, 12);
    run.join_seat(2, "Wren");
    run.join_seat(3, "Bors");
    let map = run.floor_map(Some(3));
    let own: Vec<u32> = map
        .party
        .iter()
        .filter(|m| m.own)
        .map(|m| m.knight)
        .collect();
    assert_eq!(own, vec![3]);
    assert!(run.floor_map(None).party.iter().all(|m| !m.own));
    assert_eq!(run.floor_map(None).here, Some(run.at));
}

#[test]
fn a_marker_stands_inside_its_node_where_the_knight_stands_in_the_room() {
    let mut run = on_floor(1, 13);
    run.join_seat(2, "Wren");
    let (w, h) = (run.room().width(), run.room().height());
    run.players.get_mut(&1).unwrap().x = w * 0.25;
    run.players.get_mut(&1).unwrap().y = h * 0.75;
    run.players.get_mut(&2).unwrap().x = w * 5.0; // out of the room, or nearly
    run.players.get_mut(&2).unwrap().y = -3.0;
    let map = run.floor_map(Some(1));
    let node = map.node(run.at).unwrap();
    for marker in &map.party {
        let r = node.rect;
        assert!(
            (r.x..=r.x + r.w).contains(&marker.at.0) && (r.y..=r.y + r.h).contains(&marker.at.1),
            "knight {} at {:?} outside {:?}",
            marker.knight,
            marker.at,
            r
        );
    }
    let one = map.party.iter().find(|m| m.knight == 1).unwrap();
    assert!((one.at.0 - (node.rect.x + node.rect.w * 0.25)).abs() < 1e-4);
    assert!((one.at.1 - (node.rect.y + node.rect.h * 0.75)).abs() < 1e-4);
}

#[test]
fn a_knight_waiting_outside_is_marked_in_the_room_they_wait_in() {
    let mut run = on_floor(1, 14);
    run.join_seat(2, "Wren");
    let outer = doorways(&run, 0)[0];
    // The party plays on in a hall off the floor; the knight waits at the
    // entry room.
    run.dungeon.rooms[outer].visited = true;
    run.at = outer;
    run.exits.outside.insert(
        2,
        exits::Entry {
            at: 0,
            party: BTreeMap::from([(1, (8.0, 8.0)), (2, (9.0, 9.0))]),
        },
    );
    run.players.get_mut(&2).unwrap().x = 9.0;
    run.players.get_mut(&2).unwrap().y = 9.0;
    let map = run.floor_map(Some(1));
    assert_eq!(
        map.party.iter().find(|m| m.knight == 1).unwrap().node,
        outer
    );
    assert_eq!(map.party.iter().find(|m| m.knight == 2).unwrap().node, 0);
    // A party member's own view leaves the one outside out of its players;
    // the exits still place them.
    let view = run.view_for_seat(1).expect("a view with a knight outside");
    assert!(!view.players.contains_key(&2));
    let map = view.floor_map(Some(1));
    assert_eq!(
        map.party.iter().find(|m| m.knight == 2).map(|m| m.node),
        Some(0)
    );
}

#[test]
fn there_is_no_map_at_home_or_in_the_world_above() {
    let mut home = home::Home::default();
    home.levels.insert(home::Station::Wing, 1);
    let run = Run::at_home(41, 1, None, home, Default::default());
    assert!(run.floor_map(Some(1)).is_empty());
    assert_eq!(run.floor_map(Some(1)), FloorMap::default());
}

#[test]
fn links_join_nodes_that_are_on_the_map_and_run_edge_to_edge() {
    for seed in 1..=10 {
        let mut run = on_floor(3, seed);
        visit_all(&mut run);
        let map = run.floor_map(Some(1));
        assert!(!map.links.is_empty());
        let mut pairs = std::collections::BTreeSet::new();
        for link in &map.links {
            let (a, b) = (map.node(link.a).unwrap(), map.node(link.b).unwrap());
            assert!(pairs.insert((link.a, link.b)), "one link a doorway");
            assert!(link.a < link.b);
            let on = |r: &Rect, (x, y): (f32, f32)| {
                let edge =
                    |v: f32, lo: f32, hi: f32| (v - lo).abs() < 1e-4 || (v - hi).abs() < 1e-4;
                (edge(x, r.x, r.x + r.w) && (r.y..=r.y + r.h).contains(&y))
                    || (edge(y, r.y, r.y + r.h) && (r.x..=r.x + r.w).contains(&x))
            };
            assert!(
                on(&a.rect, link.from),
                "link starts on room {}'s edge",
                link.a
            );
            assert!(on(&b.rect, link.to), "link ends on room {}'s edge", link.b);
        }
        // The floor is one piece: every known room is joined.
        let mut reached = std::collections::BTreeSet::from([0usize]);
        loop {
            let before = reached.len();
            for l in &map.links {
                if reached.contains(&l.a) || reached.contains(&l.b) {
                    reached.insert(l.a);
                    reached.insert(l.b);
                }
            }
            if reached.len() == before {
                break;
            }
        }
        assert_eq!(reached.len(), map.nodes.len(), "seed {seed}");
    }
}

#[test]
fn the_same_floor_makes_the_same_map() {
    let mut run = on_floor(3, 21);
    run.join_seat(2, "Wren");
    assert_eq!(run.floor_map(Some(2)), run.floor_map(Some(2)));
    assert_eq!(ids(&run.floor_map(Some(1))), ids(&run.floor_map(Some(2))));
}

#[test]
fn every_floor_of_every_mode_maps_into_a_whole_map() {
    for mode in Mode::WHEEL {
        for depth in 1..=DEEPEST {
            for seed in [1, 2, 3] {
                let mut run = Run::new(seed, 17, None);
                run.mode = mode;
                run.dungeon =
                    layout::floor_for(mode, depth, Pack::at(depth, Pack::Crypt), &mut run.rng);
                run.populate_boss_gates();
                run.enter(0, None);
                run.join_seat(2, "Wren");
                visit_all(&mut run);
                let map = run.floor_map(Some(1));
                let found: std::collections::BTreeSet<usize> =
                    map.nodes.iter().map(|n| n.id).collect();
                assert_eq!(
                    found.len(),
                    map.nodes.len(),
                    "{mode:?} floor {depth}: a node once"
                );
                assert!(map.here.is_some_and(|h| found.contains(&h)));
                assert!(
                    map.links
                        .iter()
                        .all(|l| found.contains(&l.a) && found.contains(&l.b))
                );
                assert!(map.party.iter().all(|m| found.contains(&m.node)));
                assert_eq!(map.depth, depth);
                // Whatever the mode, a vault stays out of it until found.
                if let Some(secret) = run.dungeon.secret.filter(|s| !s.found) {
                    assert!(!found.contains(&secret.vault), "{mode:?} floor {depth}");
                }
            }
        }
    }
}
