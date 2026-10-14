use super::*;
use crate::render::overworld::{arena, kingdom as geography};
fn run() -> Run {
    let mut r = Run::at_home(
        41,
        1,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    r.walk_kingdom(Some(geography::keep_stand()));
    r
}
#[test]
fn together_kingdom_all_places_have_connected_doors() {
    let r = run();
    let geo = geography::geography(&r.kingdom.wishes);
    let start = geography::keep_stand();
    let mut seen = std::collections::BTreeSet::from([start]);
    let mut todo = vec![start];
    while let Some((x, y)) = todo.pop() {
        for p in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if geo.room.tile(p.0, p.1) == Tile::Floor && seen.insert(p) {
                todo.push(p);
            }
        }
    }
    for site in &geo.sites {
        assert!(
            seen.contains(&site.stand),
            "unreachable {} at {:?}",
            site.name,
            site.stand
        );
    }
    for (kind, _) in world::ROOMS {
        if geography::outdoor(kind) {
            assert!(geo.sites.iter().any(|s| s.room == kind), "missing {kind:?}");
        }
    }
    assert!(r.valid_snapshot());
}
#[test]
fn together_kingdom_walking_crosses_screens_without_entry_or_reset() {
    let mut r = run();
    let before = r.at;
    let rev = r.dungeon.world_revision;
    let room = r.room();
    let lane = (2..room.rows as i32 - 2)
        .find_map(|y| {
            (2..room.cols as i32 - 32)
                .find(|&x| (x..x + 32).all(|x| room.tile(x, y) == Tile::Floor))
                .map(|x| (x, y))
        })
        .unwrap();
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = ((lane.0 as f32 + 0.5) * 2.0, (lane.1 as f32 + 0.5) * 2.0);
    h.camera = Some((h.x, h.y));
    let start = h.x;
    let friend = r.players[&2].x;
    for _ in 0..140 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                move_x: 1,
                ..Default::default()
            },
        )]));
        assert_eq!(r.at, before);
    }
    assert!(r.players[&1].x > start + 48.0);
    assert_eq!(r.players[&2].x, friend);
    assert_eq!(r.dungeon.world_revision, rev);
}
#[test]
fn together_kingdom_camera_eases_and_seats_never_share_focus() {
    let mut r = run();
    r.players.get_mut(&1).unwrap().camera = Some((50.0, 50.0));
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (60.0, 51.0);
    let h = r.players.get_mut(&2).unwrap();
    (h.x, h.y) = (170.0, 190.0);
    h.camera = Some((170.0, 189.0));
    r.follow_kingdom_cameras();
    let cam = r.players[&1].camera.unwrap();
    assert!(cam.0 > 50.0 && cam.0 < 55.0);
    assert_eq!(cam.1, 50.0);
    assert_ne!(arena::camera(&r, Some(1)), arena::camera(&r, Some(2)));
    assert_eq!(arena::camera(&r, None), arena::camera(&r, Some(1)));
    let old = r.players[&2].camera;
    r.players.get_mut(&1).unwrap().x += 5.0;
    r.follow_kingdom_cameras();
    assert_eq!(old, r.players[&2].camera);
    for h in r.players.values_mut() {
        h.camera = Some((0.0, 0.0));
    }
    r.follow_kingdom_cameras();
    assert_eq!(arena::camera(&r, Some(1)), (0, 0));
}
#[test]
fn together_kingdom_e_enters_and_saved_visits_return_to_their_door() {
    for kind in [
        RoomKind::Gate,
        RoomKind::CollegeGrounds,
        RoomKind::Ranch,
        RoomKind::RealmHall,
    ] {
        let mut r = run();
        let geo = geography::geography(&r.kingdom.wishes);
        let (i, site) = geo
            .sites
            .iter()
            .enumerate()
            .find(|(_, s)| s.room == kind)
            .unwrap();
        for h in r.players.values_mut() {
            (h.x, h.y) = (
                (site.stand.0 as f32 + 0.5) * 2.0,
                (site.stand.1 as f32 + 0.7) * 2.0,
            );
        }
        r.step(&BTreeMap::new());
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
        assert_eq!(r.room().kind, kind, "site {i}");
        let back = r.kingdom.return_to.clone().unwrap();
        let mut loaded: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        assert!(loaded.valid_snapshot());
        loaded.leave_kingdom_place();
        assert_eq!(loaded.room().kind, RoomKind::Kingdom);
        assert!((loaded.players[&1].x - back.x).abs() < 2.0);
        assert!((loaded.players[&1].y - back.y).abs() < 0.01);
        loaded.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
        assert_eq!(
            loaded.room().kind,
            RoomKind::Kingdom,
            "held E does not bounce"
        );
    }
}
#[test]
fn together_kingdom_legacy_surface_save_migrates_without_changing_settlement() {
    let mut old = Run::at_home(7, 1, None, Default::default(), Default::default());
    old.come_to_the_gate();
    let below = world::below_the_world(&old.dungeon);
    let at = old.at;
    let x = old.players[&1].x;
    let mut data = serde_json::to_value(&old).unwrap();
    data.as_object_mut().unwrap().remove("kingdom");
    data["players"]["1"]
        .as_object_mut()
        .unwrap()
        .remove("camera");
    let mut restored: Run = serde_json::from_value(data).unwrap();
    restored.migrate_kingdom();
    assert_eq!(restored.at, at);
    assert_eq!(restored.players[&1].x, x);
    assert_eq!(
        serde_json::to_value(world::below_the_world(&restored.dungeon)).unwrap(),
        serde_json::to_value(below).unwrap()
    );
    assert!(restored.valid_snapshot());
    restored.leave_kingdom_place();
    assert_eq!(restored.room().kind, RoomKind::Kingdom);
}
#[test]
fn together_kingdom_mirror_carries_independent_cameras_and_return_marker() {
    let mut host = run();
    let pose = host.pose();
    host.players.get_mut(&1).unwrap().x += 1.0;
    host.follow_kingdom_cameras();
    host.step(&BTreeMap::new());
    let mut guest = host.wire_copy();
    assert_eq!(guest.players[&1].camera, host.players[&1].camera);
    assert_eq!(guest.players[&2].camera, host.players[&2].camera);
    assert!(host.drawn_with(&pose, 0.5, None, Run::valid_snapshot));
    host.enter_kingdom_place(0);
    guest = host.wire_copy();
    assert_eq!(
        guest.kingdom.return_to.as_ref().unwrap().name,
        host.kingdom.return_to.as_ref().unwrap().name
    );
}

#[test]
fn together_kingdom_every_visit_arrives_on_open_ground_and_returns_its_party() {
    let mut r = run();
    let geo = geography::geography(&r.kingdom.wishes);
    for (index, site) in geo.sites.iter().enumerate() {
        r.walk_kingdom(Some(site.stand));
        let party = r
            .players
            .iter()
            .map(|(&id, h)| (id, (h.x, h.y)))
            .collect::<BTreeMap<_, _>>();
        r.enter_kingdom_place(index);
        assert_eq!(r.room().kind, site.room);
        for h in r.players.values() {
            assert!(
                r.settlement_walkable_for_test(h.x, h.y),
                "blocked arrival {}",
                site.name
            );
        }
        r.leave_kingdom_place();
        for (&id, &(x, y)) in &party {
            assert_eq!(
                (r.players[&id].x, r.players[&id].y),
                (x, y),
                "return {}",
                site.name
            );
        }
    }
}

#[test]
fn together_kingdom_a_friend_can_enter_their_own_distant_door() {
    let mut r = run();
    let geo = geography::geography(&r.kingdom.wishes);
    for (id, kind) in [(1, RoomKind::Gate), (2, RoomKind::Ranch)] {
        let site = geo.sites.iter().find(|s| s.room == kind).unwrap();
        let h = r.players.get_mut(&id).unwrap();
        (h.x, h.y) = (
            (site.stand.0 as f32 + 0.5) * 2.0,
            (site.stand.1 as f32 + 0.7) * 2.0,
        );
    }
    let positions = r
        .players
        .iter()
        .map(|(&id, h)| (id, (h.x, h.y)))
        .collect::<BTreeMap<_, _>>();
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(
        2,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert_eq!(r.room().kind, RoomKind::Ranch);
    r.leave_kingdom_place();
    for (id, at) in positions {
        assert_eq!((r.players[&id].x, r.players[&id].y), at);
    }
}

#[test]
fn together_kingdom_legacy_interior_and_cellar_saves_find_the_surface() {
    for kind in [RoomKind::CollegeHall, RoomKind::KingsHall, RoomKind::Home] {
        let mut r = Run::at_home(7, 1, None, Default::default(), Default::default());
        let i = world::room_of(&r.dungeon, kind).unwrap();
        r.arrive(i, (12.0, 10.0));
        r.migrate_kingdom();
        assert!(r.valid_snapshot());
        if kind == RoomKind::Home {
            let gate = world::room_of(&r.dungeon, RoomKind::Gate).unwrap();
            r.arrive(gate, world::COURTYARD_ARRIVAL);
        }
        let root = if kind == RoomKind::CollegeHall {
            RoomKind::CollegeGrounds
        } else if kind == RoomKind::KingsHall {
            RoomKind::MineHead
        } else {
            RoomKind::Gate
        };
        assert_eq!(r.kingdom.return_to.as_ref().unwrap().root, root);
        r.leave_kingdom_place();
        assert_eq!(r.room().kind, RoomKind::Kingdom);
    }
}

#[test]
fn together_kingdom_built_wishes_keep_their_lots_and_open_real_halls() {
    use crate::realm::{Status, Wish};
    let mut r = run();
    r.kingdom.wishes = std::sync::Arc::new(vec![Wish {
        id: "shrine".into(),
        name: "Wayside Shrine".into(),
        words: String::new(),
        by: "Friend".into(),
        status: Status::Built,
        price: Default::default(),
        art: vec![
            "..GG..".into(),
            ".GHHG.".into(),
            ".GHHG.".into(),
            "GGGGGG".into(),
        ],
        near: "chapel".into(),
        paid: Vec::new(),
        raised: 1,
    }]);
    r.raise_kingdom();
    let geo = geography::geography(&r.kingdom.wishes);
    let lot = crate::render::overworld::wishes::lots(&r.kingdom.wishes)[0].unwrap();
    assert_eq!(r.room().tile(lot.tx, lot.ty), Tile::Block);
    let (index, site) = geo
        .sites
        .iter()
        .enumerate()
        .find(|(_, s)| s.name == "Wayside Shrine")
        .unwrap();
    assert_eq!(site.stand, (lot.tx + lot.tw / 2, lot.ty + lot.th));
    assert_eq!(r.room().tile(site.stand.0, site.stand.1), Tile::Floor);
    r.enter_kingdom_place(index);
    assert_eq!(r.room().kind, RoomKind::RealmHall);
    let saved: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
    assert!(saved.valid_snapshot());
    assert_eq!(saved.kingdom.wishes, r.kingdom.wishes);
    r.leave_kingdom_place();
    assert_eq!(r.room().kind, RoomKind::Kingdom);
}

#[test]
fn together_exits_kingdom_return_cancels_chariot() {
    let mut r = run();
    let geo = geography::geography(&r.kingdom.wishes);
    let site = geo
        .sites
        .iter()
        .position(|s| s.room == RoomKind::Hippodrome)
        .unwrap();
    r.enter_kingdom_place(site);
    r.board_chariot(1);
    r.leave_kingdom_place();
    assert!(
        r.chariot.is_none(),
        "departed driver still races and captures inputs on re-entry"
    );
}

#[test]
fn together_exits_kingdom_return_cancels_joust() {
    let mut r = run();
    let geo = geography::geography(&r.kingdom.wishes);
    let site = geo
        .sites
        .iter()
        .position(|s| s.room == RoomKind::Lists)
        .unwrap();
    r.enter_kingdom_place(site);
    r.mount_up(1);
    assert!(r.joust.is_some());
    r.leave_kingdom_place();
    assert!(
        r.joust.is_none(),
        "departed rider still mounted on re-entry"
    );
}

/// The whole kingdom as a newcomer meets it, one plate per place:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_kingdom_tour -- --ignored`.
/// Plates land in `<dir>/tour/` as PPM, named by order and place.
#[test]
#[ignore]
fn write_kingdom_tour() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("tour");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = arena::frame(run, 768, 448);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let settle = |run: &mut Run| {
        for _ in 0..20 {
            run.step(&BTreeMap::new());
        }
    };
    // A first floor down: where it starts, and each room kind on it.
    let mut floor = Run::new(41, 1, None);
    settle(&mut floor);
    save(&floor, "002-floor1-start");
    let kinds: Vec<(usize, RoomKind)> = floor.dungeon.rooms.iter().map(|r| r.kind).enumerate().collect();
    for (index, kind) in kinds.into_iter().take(10) {
        let mut f = Run::new(41, 1, None);
        f.enter_for_test(index);
        f.enemies.clear();
        settle(&mut f);
        save(&f, &format!("003-floor1-{index:02}-{kind:?}"));
    }
    // Tallow, just scratched behind the ears.
    let mut purr = Run::new(41, 1, None);
    purr.enemies.clear();
    let (hx, hy) = (purr.players[&1].x, purr.players[&1].y);
    purr.cat = Some(crate::rules::cat::Cat {
        x: hx + 1.4,
        y: hy,
        fetching: None,
        carrying: None,
        hiss: 0,
        hissed: 0,
        left: false,
        moving: false,
        fetched: 0,
        stuck: 0,
        petted: 0,
        pet_held: Default::default(),
    });
    purr.tick = 100;
    purr.cat.as_mut().unwrap().petted = 92;
    save(&purr, "004-tallow-purr");
    let atlas = geography::atlas(&[]);
    let mut out = format!("P6\n{} {}\n255\n", atlas.w, atlas.h).into_bytes();
    out.extend(atlas.rgb_bytes());
    std::fs::write(dir.join("000-atlas.ppm"), out).unwrap();
    let mut home = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    home.naked_start = true;
    settle(&mut home);
    save(&home, "000-undercroft");
    let mut r = run();
    settle(&mut r);
    save(&r, "001-kingdom-keep");
    let geo = geography::geography(&r.kingdom.wishes);
    for (i, (x, y)) in crate::rules::classes::stones(&geo) {
        let mut r = run();
        r.walk_kingdom(Some((x, y + 2)));
        settle(&mut r);
        save(&r, &format!("05{i}-class"));
    }
    for (i, (thing, (x, y))) in crate::rules::trinkets::things(&geo).into_iter().enumerate() {
        let mut r = run();
        r.walk_kingdom(Some((x, y + 1)));
        settle(&mut r);
        save(&r, &format!("06{i}-{thing:?}"));
        // And the moment it is used.
        r.kingdom.moments.used.insert(i as u8, r.tick.saturating_sub(8));
        use crate::rules::trinkets::Thing;
        match thing {
            Thing::Bench => r.players.get_mut(&1).unwrap().sitting = true,
            Thing::Stocks => {
                let h = r.players.get_mut(&1).unwrap();
                (h.x, h.y) = ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.6) * TILE_UNITS);
                h.stocked = true;
                h.splats = 7;
            }
            Thing::Goose => {
                r.kingdom.moments.goose = Some((1, r.tick));
                for _ in 0..8 {
                    r.step(&BTreeMap::new());
                }
            }
            _ => {}
        }
        save(&r, &format!("06{i}-{thing:?}-used"));
    }
    for (i, site) in geo.sites.iter().enumerate() {
        let mut r = run();
        r.walk_kingdom(Some(site.stand));
        settle(&mut r);
        let name: String = site.name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        save(&r, &format!("1{i:02}-site-{name}"));
    }
    // Inside every place the kingdom's doors open on.
    for (i, site) in geo.sites.iter().enumerate() {
        if matches!(site.room, RoomKind::RealmHall) {
            continue;
        }
        let mut r = run();
        r.enter_kingdom_place(i);
        settle(&mut r);
        save(&r, &format!("3{i:02}-inside-{:?}", site.room));
    }
    for (i, (kind, _)) in world::ROOMS.iter().enumerate() {
        if geography::outdoor(*kind) {
            continue;
        }
        let mut r = run();
        let Some(index) = world::room_of(&r.dungeon, *kind) else {
            continue;
        };
        r.enter_for_test(index);
        settle(&mut r);
        save(&r, &format!("2{i:02}-room-{kind:?}"));
    }
}

#[test]
fn every_countryside_place_stands_in_its_own_region() {
    let geo = geography::geography(&[]);
    for (kind, _) in world::ROOMS {
        if geography::outdoor(kind) {
            assert!(geo.sites.iter().any(|s| s.room == kind), "{kind:?} has no door on the map");
        }
    }
    for region in &geography::REGIONS {
        for &(kind, _) in region.places {
            let lot = geo.sites.iter().find(|s| s.room == kind).and_then(|s| s.lot).unwrap();
            assert!(region.holds(lot.tx, lot.ty), "{kind:?} was built outside {}", region.name);
        }
    }
    // Regions' names never sit on a building or a door.
    for (name, (x, y)) in geography::regions() {
        for site in &geo.sites {
            assert!(site.lot.is_none_or(|l| !l.contains(x, y)), "{name} is lettered on {}", site.name);
            assert_ne!(site.stand, (x, y), "{name} is lettered on {}'s door", site.name);
        }
    }
}

#[test]
fn the_kingdom_points_a_newcomer_to_the_delve_then_somewhere_new() {
    let mut r = run();
    assert_eq!(r.kingdom_goal(1).unwrap().name, "The Delve", "first, the Delve");
    // Back from a delve, a race already won (else the Griffin Eyrie is the
    // goal): the nearest place not yet stepped into.
    r.home.deepest = 2;
    r.home.countryside.race_won = true;
    let first = r.kingdom_goal(1).unwrap();
    assert_ne!(first.name, "The Delve");
    let geo = geography::geography(&r.kingdom.wishes);
    let index = geo.sites.iter().position(|s| s.stand == first.stand).unwrap();
    r.enter_kingdom_place(index);
    r.leave_kingdom_place();
    let next = r.kingdom_goal(1).unwrap();
    assert_ne!(next.stand, first.stand, "a place seen is not pointed at again");
    // Off the surface there is no goal marker.
    let gate = world::room_of(&r.dungeon, RoomKind::Gate).unwrap();
    r.enter_for_test(gate);
    assert!(r.kingdom_goal(1).is_none());
}

#[test]
fn a_knight_trots_twice_as_fast_along_a_kingdom_road() {
    let r = run();
    let geo = geography::geography(&r.kingdom.wishes);
    let (w, h) = (r.room().cols as i32, r.room().rows as i32);
    // A straight east-west stretch of road, and a stretch of open ground.
    let stretch = |road: bool| {
        (2..h - 2).flat_map(|y| (2..w - 12).map(move |x| (x, y))).find(|&(x, y)| {
            (0..10).all(|k| {
                r.room().tile(x + k, y) == Tile::Floor
                    && geography::is_road(&geo, (x + k, y)) == road
                    && r.room().tile(x + k, y - 1) == Tile::Floor
                    && r.room().tile(x + k, y + 1) == Tile::Floor
            })
        })
    };
    let walked = |(x, y): (i32, i32)| {
        let mut r = run();
        let hero = r.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS);
        let start = hero.x;
        // The pace a browser seat's picture is stamped with, to slide by.
        let stamped = r.ground_pace_of(1).unwrap() * 10.0 * DT;
        for _ in 0..10 {
            r.step(&BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]));
        }
        let walked = r.players[&1].x - start;
        assert!(
            (walked - stamped).abs() < 0.01,
            "stamped pace walks {stamped}, the step walked {walked}"
        );
        walked
    };
    let road = walked(stretch(true).expect("a road"));
    let field = walked(stretch(false).expect("open ground"));
    assert!(road > field * 1.8, "road {road} vs field {field}");
}

/// The kingdom's layout at a glance: the whole map, a tile chart of every
/// site, lot and road, and a walk shot of each region:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_kingdom_layout -- --ignored`.
#[test]
#[ignore]
fn write_kingdom_layout() {
    use crate::render::overworld::{MAP_H, MAP_W};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let geo = geography::geography(&[]);
    let mut atlas = (*geography::atlas(&[])).clone();
    geography::letter_regions(&mut atlas, (0, 0));
    let mut out = format!("P6\n{} {}\n255\n", atlas.w, atlas.h).into_bytes();
    out.extend(atlas.rgb_bytes());
    std::fs::write(dir.join("map.ppm"), out).unwrap();
    let mut chart = String::new();
    let mark = |i: usize| (if i < 26 { b'A' + i as u8 } else { b'a' + (i - 26) as u8 % 26 }) as char;
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            let site = geo.sites.iter().position(|s| s.stand == (x, y));
            let lot = geo.sites.iter().position(|s| s.lot.is_some_and(|l| l.contains(x, y)));
            chart.push(match (site, lot) {
                (Some(i), _) => mark(i),
                (_, Some(_)) => '#',
                _ if geography::is_road(&geo, (x, y)) => '+',
                _ => geography::terrain(x, y) as char,
            });
        }
        chart.push('\n');
    }
    for (i, s) in geo.sites.iter().enumerate() {
        chart.push_str(&format!("{} {:?} {:?} {} {:?}\n", mark(i), s.stand, s.room, s.name, s.lot.map(|l| (l.tx, l.ty))));
    }
    for (name, at) in geography::regions() {
        chart.push_str(&format!("region {name} {at:?}\n"));
    }
    std::fs::write(dir.join("chart.txt"), chart).unwrap();
    let save = |run: &Run, name: &str| {
        let img = arena::frame(run, 768, 448);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    for (i, (name, (x, y))) in geography::regions().into_iter().enumerate() {
        let mut r = run();
        // Stand among the region's places, or by its name if it has none.
        let doors: Vec<(i32, i32)> = geography::REGIONS[i]
            .places
            .iter()
            .filter_map(|&(kind, _)| geo.sites.iter().find(|s| s.room == kind).map(|s| s.stand))
            .collect();
        let n = doors.len().max(1) as i32;
        let at = if doors.is_empty() {
            (x, y - 3)
        } else {
            (doors.iter().map(|d| d.0).sum::<i32>() / n, doors.iter().map(|d| d.1).sum::<i32>() / n)
        };
        r.walk_kingdom(Some(at));
        for _ in 0..20 {
            r.step(&BTreeMap::new());
        }
        let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        save(&r, &format!("region-{i:02}-{name}"));
    }
    // Standing in the town's own places: each shows its own name once.
    for name in ["Keep", "Village", "Colosseum", "Artisan Quarter", "Knights Tournament", "Delve"] {
        let site = geo.sites.iter().find(|s| s.name == name).unwrap();
        let mut r = run();
        r.walk_kingdom(Some(site.stand));
        for _ in 0..20 {
            r.step(&BTreeMap::new());
        }
        let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        save(&r, &format!("place-{name}"));
    }
}

/// Knight `id` stood at the stand of the kingdom's place of kind `kind`.
fn stand_at(r: &mut Run, id: u32, kind: RoomKind) {
    let geo = geography::geography(&r.kingdom.wishes);
    let site = geo.sites.iter().find(|s| s.room == kind).unwrap();
    let h = r.players.get_mut(&id).unwrap();
    (h.x, h.y) = ((site.stand.0 as f32 + 0.5) * 2.0, (site.stand.1 as f32 + 0.7) * 2.0);
}

/// One tick with knight 1 doing `one` and knight 2 doing `two`.
fn tick(r: &mut Run, one: Option<Input>, two: Input) {
    let mut inputs = BTreeMap::from([(2, two)]);
    if let Some(one) = one {
        inputs.insert(1, one);
    }
    r.step(&inputs);
}

fn aim() -> Input {
    Input { aim_x: 1, ..Default::default() }
}

fn e() -> Input {
    Input { interact: true, ..Default::default() }
}

#[test]
fn together_kingdom_a_place_left_closes_behind_its_last_knight_not_an_away_one() {
    // The host's knight stands idle (its player at their code) while a
    // friend goes into the ranch, comes out, and tries the college.
    let mut r = run();
    for _ in 0..super::AWAY_TICKS {
        tick(&mut r, None, aim());
    }
    assert!(r.away(1) && !r.away(2));
    stand_at(&mut r, 2, RoomKind::Ranch);
    tick(&mut r, None, aim());
    tick(&mut r, None, e());
    assert_eq!(r.room().kind, RoomKind::Ranch);
    tick(&mut r, None, aim());
    tick(&mut r, None, Input { leave_activity: true, ..Default::default() });
    assert_eq!(r.room().kind, RoomKind::Kingdom, "the friend was the last one out who was playing");
    assert!(r.exits.outside.is_empty());
    stand_at(&mut r, 2, RoomKind::CollegeGrounds);
    tick(&mut r, None, aim());
    tick(&mut r, None, e());
    assert_eq!(r.room().kind, RoomKind::CollegeGrounds, "and the friend's doors open again");
}

#[test]
fn together_kingdom_a_knight_waiting_outside_is_told_where_the_party_is_until_it_comes_out() {
    let mut r = run();
    stand_at(&mut r, 2, RoomKind::Ranch);
    tick(&mut r, Some(aim()), aim());
    tick(&mut r, Some(aim()), e());
    assert_eq!(r.room().kind, RoomKind::Ranch);
    tick(&mut r, Some(aim()), aim());
    tick(&mut r, Some(aim()), Input { leave_activity: true, ..Default::default() });
    // The host plays on inside: the friend waits at the ranch's door.
    assert_eq!(r.room().kind, RoomKind::Ranch);
    assert!(r.exits.outside.contains_key(&2));
    let view = r.view_for_seat(2).unwrap();
    let waiting = view.kingdom.waiting.clone().expect("the friend's view says the party is inside");
    assert_eq!(waiting.place, "Horse Ranch");
    assert!(r.view_for_seat(1).is_none_or(|v| v.kingdom.waiting.is_none()));
    // The host goes back to their code: once they are away, the ranch
    // closes and the party is whole in the kingdom.
    for _ in 0..super::AWAY_TICKS {
        tick(&mut r, None, aim());
    }
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert!(r.exits.outside.is_empty());
    assert!(r.view_for_seat(2).is_none_or(|v| v.kingdom.waiting.is_none()));
}

#[test]
fn together_kingdom_one_knight_holding_e_holds_no_one_elses_door() {
    let mut r = run();
    stand_at(&mut r, 2, RoomKind::Ranch);
    // Knight 1 holds E away from every door, as a stuck key or pad would.
    tick(&mut r, Some(e()), aim());
    tick(&mut r, Some(e()), aim());
    tick(&mut r, Some(e()), e());
    assert_eq!(r.room().kind, RoomKind::Ranch);
}

#[test]
fn together_kingdom_e_held_through_a_door_opens_nothing_until_let_go() {
    let mut r = run();
    stand_at(&mut r, 2, RoomKind::Ranch);
    tick(&mut r, None, aim());
    tick(&mut r, None, e());
    assert_eq!(r.room().kind, RoomKind::Ranch);
    r.leave_kingdom_place();
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    stand_at(&mut r, 2, RoomKind::Ranch);
    // Still down from the press that went in: nothing.
    tick(&mut r, None, e());
    tick(&mut r, None, e());
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    tick(&mut r, None, aim());
    tick(&mut r, None, e());
    assert_eq!(r.room().kind, RoomKind::Ranch);
}
