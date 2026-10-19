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

/// The kingdom's layout at a glance: the whole map (and again with its
/// 24x16 chunks ruled), a tile chart of every site, lot and road, a walk
/// shot of each biome from its open green, a few fingerposts and finds as
/// a knight comes up to them, and the town's own places, all as PNG:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_kingdom_layout -- --ignored`.
#[test]
#[ignore]
fn write_kingdom_layout() {
    use crate::render::overworld::{MAP_H, MAP_W, TILE, map};
    use crate::world::{biome::Biome, finds, signs};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let png = |name: &str, img: &crate::render::overworld::Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let geo = geography::geography(&[]);
    let mut atlas = (*geography::atlas(&[])).clone();
    geography::letter_regions(&mut atlas, (0, 0));
    png("map.png", &atlas);
    let mut ruled = atlas.clone();
    for cx in 1..map::CHUNKS_X {
        ruled.line(cx * map::CHUNK_W * TILE, 0, cx * map::CHUNK_W * TILE, ruled.h - 1, 'X');
    }
    for cy in 1..map::CHUNKS_Y {
        ruled.line(0, cy * map::CHUNK_H * TILE, ruled.w - 1, cy * map::CHUNK_H * TILE, 'X');
    }
    png("map-chunks.png", &ruled);
    let mut chart = String::new();
    let mark = |i: usize| (if i < 26 { b'A' + i as u8 } else { b'a' + (i - 26) as u8 % 26 }) as char;
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            let site = geo.sites.iter().position(|s| s.stand == (x, y));
            let lot = geo.sites.iter().position(|s| s.lot.is_some_and(|l| l.contains(x, y)));
            chart.push(match (site, lot) {
                (Some(i), _) => mark(i),
                (_, Some(_)) => '#',
                _ if signs::posts().iter().any(|p| p.at == (x, y)) => '!',
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
    for p in signs::posts() {
        chart.push_str(&format!("post {:?} {:?}\n", p.at, p.arms));
    }
    for f in finds::finds(41).iter() {
        chart.push_str(&format!("find {:?} {:?} {:?}\n", f.kind, f.at, f.biome));
    }
    std::fs::write(dir.join("chart.txt"), chart).unwrap();
    let shoot = |at: (i32, i32), name: &str, ticks: u32| {
        let mut r = run();
        r.walk_kingdom(Some(at));
        for _ in 0..ticks {
            r.step(&BTreeMap::new());
        }
        png(&format!("{name}.png"), &arena::frame(&r, 768, 448));
    };
    // Each biome from its open green, a moment after coming into it (its
    // name still in the corner).
    for (i, region) in geography::REGIONS.iter().enumerate() {
        let (x, y) = geography::regions()[i].1;
        let at = (0..12)
            .flat_map(|d| [(x, y + 2 + d), (x, y - 2 - d), (x + 3 + d, y), (x - 3 - d, y)])
            .find(|&(x, y)| geo.room.tile(x, y) == Tile::Floor)
            .unwrap_or((x, y));
        let name: String = region.name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        shoot(at, &format!("biome-{i:02}-{name}"), 20);
        assert!(Biome::ALL.contains(&region.biome));
    }
    // Fingerposts as a knight comes up to them.
    for (i, p) in signs::posts().iter().enumerate().step_by(7) {
        shoot((p.road.0, p.road.1 + 1), &format!("post-{i:02}"), 4);
    }
    // Finds, a knight beside each of a few.
    for (i, f) in finds::finds(41).iter().enumerate().step_by(3) {
        shoot((f.at.0 + 1, f.at.1), &format!("find-{i:02}-{:?}", f.kind), 4);
    }
    // Standing in the town's own places: each shows its own name once.
    for name in ["Keep", "Village", "Colosseum", "Artisan Quarter", "Knights Tournament", "Delve"] {
        let site = geo.sites.iter().find(|s| s.name == name).unwrap();
        let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        shoot(site.stand, &format!("place-{name}"), 20);
    }
}

/// The kingdom's density at a glance, for the dressing pass: the whole map
/// (and ruled into its chunks), one walk shot at the middle of every chunk
/// (`census-<row>-<col>.png`), a shot at the camps of each biome with a
/// knight beside them and a find where one stands there, and close-ups of
/// the atlas at each kind of cluster, as PNG:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_kingdom_density -- --ignored`.
#[test]
#[ignore]
fn write_kingdom_density() {
    use crate::render::overworld::{Img, TILE, map};
    use crate::world::{biome, dressing, finds};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let png = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let geo = geography::geography(&[]);
    let mut atlas = (*geography::atlas(&[])).clone();
    geography::letter_regions(&mut atlas, (0, 0));
    png("map.png", &atlas);
    let mut ruled = atlas.clone();
    for cx in 1..map::CHUNKS_X {
        ruled.line(cx * map::CHUNK_W * TILE, 0, cx * map::CHUNK_W * TILE, ruled.h - 1, 'X');
    }
    for cy in 1..map::CHUNKS_Y {
        ruled.line(0, cy * map::CHUNK_H * TILE, ruled.w - 1, cy * map::CHUNK_H * TILE, 'X');
    }
    png("map-chunks.png", &ruled);
    let floor_near = |(x, y): (i32, i32)| {
        (0..12)
            .flat_map(|d| (-d..=d).flat_map(move |dx| [(x + dx, y - d), (x + dx, y + d), (x - d, y + dx), (x + d, y + dx)]))
            .find(|&(x, y)| geo.room.tile(x, y) == Tile::Floor && !geography::is_road(&geo, (x, y)))
            .unwrap_or((x, y))
    };
    let shoot = |at: (i32, i32), name: &str, ticks: u32| {
        let mut r = run();
        r.walk_kingdom(Some(at));
        for _ in 0..ticks {
            r.step(&BTreeMap::new());
        }
        png(&format!("{name}.png"), &arena::frame(&r, 768, 448));
    };
    // The numbers: clusters and things by biome, the footpaths, the finds.
    let d = dressing::get();
    let mut sheet = String::new();
    for b in biome::Biome::ALL {
        let wild = d.camps.iter().filter(|c| c.biome == b && c.class == dressing::Class::Wild).count();
        let way = d.camps.iter().filter(|c| c.biome == b && c.class == dressing::Class::Road).count();
        let things = d.pieces.iter().filter(|p| biome::at(p.at.0, p.at.1) == b).count();
        let mut stories: Vec<&str> = d.camps.iter().filter(|c| c.biome == b).map(|c| c.story).collect();
        stories.sort();
        stories.dedup();
        sheet.push_str(&format!("{:<16} {wild:>3} wild {way:>3} wayside {things:>4} things: {}\n", b.name(), stories.join(", ")));
    }
    sheet.push_str(&format!(
        "{} clusters, {} things, {} footpath tiles\n",
        d.camps.len(),
        d.pieces.len(),
        d.trails.iter().filter(|&&t| t).count()
    ));
    for (i, f) in finds::finds(41).iter().enumerate() {
        sheet.push_str(&format!("find {i:>2} {:?} {:?} {:?} trail {}\n", f.kind, f.at, f.biome, d.trail(f.at.0, f.at.1)));
    }
    std::fs::write(dir.join("dressing.txt"), sheet).unwrap();
    // A view's worth at the middle of every chunk.
    for cy in 0..map::CHUNKS_Y {
        for cx in 0..map::CHUNKS_X {
            let at = floor_near((cx * map::CHUNK_W + map::CHUNK_W / 2, cy * map::CHUNK_H + map::CHUNK_H / 2));
            shoot(at, &format!("census-{cy:02}-{cx:02}"), 2);
        }
    }
    // Each biome: its camps, a knight beside them.
    for b in biome::Biome::ALL {
        let name: String = b.name().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        for (k, c) in d.camps.iter().filter(|c| c.biome == b).step_by(3).take(4).enumerate() {
            shoot((c.at.0 + 2, c.at.1 + 2), &format!("camp-{name}-{k}"), 4);
        }
    }
    // A knight beside each find of the run, to see it among its dressing.
    for (i, f) in finds::finds(41).iter().enumerate() {
        shoot((f.at.0 + 2, f.at.1 + 1), &format!("find-{i:02}-{:?}", f.kind), 4);
    }
    // The most crowded view of all: the 24 x 14 tiles with the most things.
    let (mut peak, mut at) = (0, (0, 0));
    for wy in (7..map::MAP_H - 7).step_by(2) {
        for wx in (12..map::MAP_W - 12).step_by(2) {
            let n = d.pieces.iter().filter(|p| (p.at.0 - wx).abs() <= 12 && (p.at.1 - wy).abs() <= 7).count();
            if n > peak && geo.room.tile(wx, wy) == Tile::Floor {
                (peak, at) = (n, (wx, wy));
            }
        }
    }
    shoot(floor_near(at), "crowded-peak", 2);
    // Close-ups of the atlas at each kind of cluster, three times over.
    let mut seen = std::collections::BTreeSet::new();
    for c in &d.camps {
        if !seen.insert((c.biome, c.story)) {
            continue;
        }
        let (w, h) = (15 * TILE, 9 * TILE);
        let (x0, y0) = ((c.at.0 * TILE + TILE / 2 - w / 2).clamp(0, atlas.w - w), (c.at.1 * TILE + TILE / 2 - h / 2).clamp(0, atlas.h - h));
        let mut shot = Img::black(w * 3, h * 3);
        for y in 0..h * 3 {
            for x in 0..w * 3 {
                if let Some(px) = atlas.get(x0 + x / 3, y0 + y / 3) {
                    shot.set(x, y, px);
                }
            }
        }
        let name: String = c.story.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        png(&format!("close-{:?}-{name}.png", c.biome), &shot);
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

#[test]
fn coming_into_a_biome_names_it_small_in_the_corner_and_it_fades() {
    use crate::world::biome::{self, Biome};
    let mut r = run();
    r.step(&BTreeMap::new());
    let (town, _) = r.kingdom.entered[&1];
    assert_eq!(town, Biome::CastleTown.index());
    // Walk knight 1 into the Fen: the corner names it, in knight 1's view.
    let geo = geography::geography(&r.kingdom.wishes);
    let fen = (0..geo.room.rows as i32)
        .flat_map(|y| (0..geo.room.cols as i32).map(move |x| (x, y)))
        .find(|&(x, y)| biome::at(x, y) == Biome::Fen && geo.room.tile(x, y) == Tile::Floor)
        .unwrap();
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = ((fen.0 as f32 + 0.5) * 2.0, (fen.1 as f32 + 0.5) * 2.0);
    r.step(&BTreeMap::new());
    let (fen_index, since) = r.kingdom.entered[&1];
    assert_eq!(fen_index, Biome::Fen.index());
    assert!(r.tick - since <= 1, "just now");
    let with = arena::frame_for(&r, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    let mut quiet = r.clone();
    quiet.kingdom.entered.clear();
    let without = arena::frame_for(&quiet, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    let mut changed = Vec::new();
    for y in 0..arena::NATIVE_H {
        for x in 0..arena::NATIVE_W {
            if with.get(x, y) != without.get(x, y) {
                changed.push((x, y));
            }
        }
    }
    assert!(!changed.is_empty(), "the name shows");
    let corner = |x: i32, y: i32| y < 20 && (x < 140 || x >= arena::NATIVE_W - 140);
    assert!(changed.iter().all(|&(x, y)| corner(x, y)), "only in a top corner: {:?}", changed.last());
    // Three seconds on, it has gone.
    for _ in 0..3 * crate::rules::HZ {
        r.step(&BTreeMap::new());
    }
    let later = arena::frame_for(&r, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    let mut gone = r.clone();
    gone.kingdom.entered.clear();
    assert_eq!(later.rgba_bytes(), arena::frame_for(&gone, arena::NATIVE_W, arena::NATIVE_H, Some(1)).rgba_bytes());
}

// ===========================================================================
// PARTY CHECK (test-only, UNCOMMITTED): the live save through the new code,
// and the open kingdom walked. Env: LIVE_SESSION=<session.json>.
// ===========================================================================

/// Steps from `from` over floor to every tile (-1: not reached).
fn pc_dist(room: &Room, from: (i32, i32)) -> Vec<i32> {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let mut d = vec![-1; (cols * rows) as usize];
    if from.0 < 0 || from.1 < 0 || from.0 >= cols || from.1 >= rows || room.tile(from.0, from.1) != Tile::Floor {
        return d;
    }
    d[(from.1 * cols + from.0) as usize] = 0;
    let mut q = std::collections::VecDeque::from([from]);
    while let Some((x, y)) = q.pop_front() {
        let here = d[(y * cols + x) as usize];
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx >= 0 && ny >= 0 && nx < cols && ny < rows && d[(ny * cols + nx) as usize] < 0 && room.tile(nx, ny) == Tile::Floor {
                d[(ny * cols + nx) as usize] = here + 1;
                q.push_back((nx, ny));
            }
        }
    }
    d
}

fn pc_tile(h: &Hero) -> (i32, i32) {
    ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32)
}

/// Walk knight `id` on foot to tile `goal`, by the shortest way over floor,
/// each axis on its own as a keyboard does. Ticks taken, or why not.
fn pc_follow(run: &mut Run, id: u32, goal: (i32, i32), max_ticks: u32) -> Result<u32, String> {
    let hero = &run.players[&id];
    let goal_units = ((goal.0 as f32 + 0.5) * TILE_UNITS, (goal.1 as f32 + 0.5) * TILE_UNITS);
    let path = crate::rules::depths::open_path(run.room(), (hero.x, hero.y), goal_units, false)
        .ok_or_else(|| format!("no path from {:?} to {:?}", pc_tile(hero), goal))?;
    let mut i = 0;
    let mut last = (hero.x, hero.y);
    let mut still = 0u32;
    for t in 0..max_ticks {
        let hero = &run.players[&id];
        let here = pc_tile(hero);
        if here == goal {
            return Ok(t);
        }
        while i + 1 < path.len() && path[i] == here {
            i += 1;
        }
        let (c, r) = path[i];
        let axis = |d: f32| if d > 0.25 { 1 } else if d < -0.25 { -1 } else { 0 };
        let (dx, dy) = ((c as f32 + 0.5) * TILE_UNITS - hero.x, (r as f32 + 0.5) * TILE_UNITS - hero.y);
        let input = Input { move_x: axis(dx), move_y: axis(dy), ..Default::default() };
        run.step(&BTreeMap::from([(id, input)]));
        let hero = &run.players[&id];
        if (hero.x - last.0).hypot(hero.y - last.1) < 0.4 {
            still += 1;
            if still > 120 {
                return Err(format!("stuck at {:?} heading {:?} (goal {:?}) after {t} ticks", pc_tile(hero), (c, r), goal));
            }
        } else {
            (last, still) = ((hero.x, hero.y), 0);
        }
    }
    Err(format!("not there after {max_ticks} ticks: at {:?}, goal {:?}", pc_tile(&run.players[&id]), goal))
}

fn pc_place(run: &mut Run, id: u32, tile: (i32, i32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = ((tile.0 as f32 + 0.5) * TILE_UNITS, (tile.1 as f32 + 0.7) * TILE_UNITS);
    h.camera = Some((h.x, h.y - 1.0));
}

/// The open kingdom walked: every biome, post, find and place reachable on
/// foot from the Keep (Castle Town), and walked to.
/// `cargo test -p angel-delve --lib party_check_kingdom_walk -- --ignored --nocapture`
#[test]
#[ignore]
fn party_check_kingdom_walk() {
    use crate::world::biome::{self, Biome};
    use crate::world::{finds, signs};
    let base = run();
    let geo = geography::geography(&base.kingdom.wishes);
    let room = &geo.room;
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let keep = geography::keep_stand();
    let d = pc_dist(room, keep);
    let at = |p: (i32, i32)| d[(p.1 * cols + p.0) as usize];
    let mut fails: Vec<String> = Vec::new();
    println!("PCK,map,{cols}x{rows},keep,{keep:?},sites,{}", geo.sites.len());
    let (mut floor, mut reach) = (0, 0);
    // Per biome: floor tiles, reached, the farthest reached tile.
    let mut far: BTreeMap<u8, ((i32, i32), i32)> = BTreeMap::new();
    let mut tally: BTreeMap<u8, (u32, u32)> = BTreeMap::new();
    for y in 0..rows {
        for x in 0..cols {
            if room.tile(x, y) != Tile::Floor {
                continue;
            }
            floor += 1;
            let b = biome::at(x, y).index();
            let e = tally.entry(b).or_default();
            e.0 += 1;
            if at((x, y)) >= 0 {
                reach += 1;
                e.1 += 1;
                let f = far.entry(b).or_insert(((x, y), -1));
                if at((x, y)) > f.1 {
                    *f = ((x, y), at((x, y)));
                }
            }
        }
    }
    println!("PCK,floor,{floor},reached,{reach}");
    for b in Biome::ALL {
        let (t, r) = tally.get(&b.index()).copied().unwrap_or_default();
        let f = far.get(&b.index()).copied();
        println!("PCK,biome,{},floor,{t},reached,{r},farthest,{f:?}", b.name());
        if r == 0 {
            fails.push(format!("biome {} has no tile reachable from Castle Town", b.name()));
        } else if r * 100 < t * 80 {
            fails.push(format!("biome {}: only {r} of {t} floor tiles reachable", b.name()));
        }
    }
    // Chunks (24x16 tiles) with nothing reachable.
    let (cw, ch) = (cols / 10, rows / 11);
    let mut dead_chunks = 0;
    for cy in 0..11 {
        for cx in 0..10 {
            let any = (cy * ch..(cy + 1) * ch).any(|y| (cx * cw..(cx + 1) * cw).any(|x| at((x, y)) >= 0));
            if !any {
                dead_chunks += 1;
                println!("PCK,chunk-unreachable,{cx},{cy},{:?}", biome::chunk(cx, cy));
            }
        }
    }
    println!("PCK,chunks-unreachable,{dead_chunks}");
    if dead_chunks > 0 {
        fails.push(format!("{dead_chunks} chunks have nothing reachable"));
    }
    // Places.
    let mut targets: Vec<(String, (i32, i32))> = Vec::new();
    for s in &geo.sites {
        if at(s.stand) < 0 {
            fails.push(format!("place {} at {:?} unreachable", s.name, s.stand));
        }
        targets.push((format!("place {}", s.name), s.stand));
    }
    let gate = geography::gate_stand(&base.kingdom.wishes);
    println!("PCK,gate,{gate:?},steps,{}", at(gate));
    if at(gate) < 0 {
        fails.push("the delve entrance (the gate) is unreachable".into());
    }
    // Fingerposts: a reachable floor tile within two of each.
    let posts = signs::posts();
    let mut worst_post = 0;
    for p in posts {
        let near = (-3..=3)
            .flat_map(|dy| (-3..=3).map(move |dx| (p.at.0 + dx, p.at.1 + dy)))
            .filter(|&q| q.0 >= 0 && q.1 >= 0 && q.0 < cols && q.1 < rows && at(q) >= 0)
            .min_by_key(|&q| (q.0 - p.at.0).abs().max((q.1 - p.at.1).abs()));
        match near {
            Some(q) => {
                let gap = (q.0 - p.at.0).abs().max((q.1 - p.at.1).abs());
                worst_post = worst_post.max(gap);
                if gap > 2 {
                    fails.push(format!("post at {:?} is {gap} tiles from the nearest reachable floor", p.at));
                }
                targets.push((format!("post {:?}", p.at), q));
            }
            None => fails.push(format!("post at {:?} has no reachable floor within 3 tiles", p.at)),
        }
    }
    println!("PCK,posts,{},worst-gap,{worst_post}", posts.len());
    // Finds, for the run's seed and a spread of others.
    let mut seeds: Vec<u64> = vec![base.seed, 41, 1];
    seeds.extend(0..12u64);
    seeds.sort();
    seeds.dedup();
    for seed in &seeds {
        let all = finds::finds(*seed);
        let unreachable: Vec<_> = all.iter().filter(|f| at(f.at) < 0).collect();
        let kinds: std::collections::BTreeSet<String> = all.iter().map(|f| format!("{:?}", f.kind)).collect();
        println!("PCK,finds,seed,{seed},count,{},unreachable,{},kinds,{}", all.len(), unreachable.len(), kinds.len());
        if all.len() != finds::COUNT {
            fails.push(format!("seed {seed}: {} finds, not {}", all.len(), finds::COUNT));
        }
        if !unreachable.is_empty() {
            fails.push(format!("seed {seed}: {} finds unreachable, e.g. {:?}", unreachable.len(), unreachable[0]));
        }
    }
    for (i, f) in finds::finds(base.seed).iter().enumerate() {
        targets.push((format!("find {i} {:?} in {}", f.kind, f.biome.name()), f.at));
    }
    for b in Biome::ALL {
        if let Some((t, _)) = far.get(&b.index()) {
            targets.push((format!("biome {} (farthest)", b.name()), *t));
        }
    }
    // On foot, with a knight: place it at the Keep and walk it.
    let (mut ok, mut slowest) = (0, (0u32, String::new()));
    for (label, goal) in &targets {
        let mut w = base.clone();
        pc_place(&mut w, 1, keep);
        match pc_follow(&mut w, 1, *goal, 9000) {
            Ok(t) => {
                ok += 1;
                if t > slowest.0 {
                    slowest = (t, label.clone());
                }
                // A find answers E at its tile.
                if label.starts_with("find ") {
                    let before = w.found.clone();
                    w.step(&BTreeMap::new());
                    w.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
                    if w.found == before {
                        fails.push(format!("{label}: E at its tile did nothing"));
                    }
                }
            }
            Err(e) => fails.push(format!("{label}: {e}")),
        }
    }
    println!("PCK,walked,{ok},of,{},slowest-ticks,{},{}", targets.len(), slowest.0, slowest.1);
    // The delve entrance opens on foot.
    {
        let mut w = base.clone();
        pc_place(&mut w, 1, keep);
        match pc_follow(&mut w, 1, gate, 9000) {
            Ok(_) => {
                w.step(&BTreeMap::new());
                w.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
                println!("PCK,gate-door,room,{:?}", w.room().kind);
                if w.room().kind != RoomKind::Gate {
                    fails.push(format!("E at the gate stand led to {:?}, not the Gate", w.room().kind));
                }
            }
            Err(e) => fails.push(format!("gate: {e}")),
        }
    }
    for f in &fails {
        println!("PCK,FAIL,{f}");
    }
    assert!(fails.is_empty(), "{} failures", fails.len());
}

/// A party of four coming into the kingdom, by every way a run comes: nobody
/// is stuck at their spawn, and each can walk to the gate and to the far
/// reaches.
#[test]
#[ignore]
fn party_check_kingdom_spawn() {
    let mut fails: Vec<String> = Vec::new();
    for how in ["come_to_the_gate", "walk_kingdom(keep)", "walk_kingdom(None)", "leave_place"] {
        let mut r = Run::at_home(7, 1, None, Default::default(), Default::default());
        for (id, n) in [(2, "Wren"), (3, "Bors"), (4, "Ysolde")] {
            r.join_seat(id, n);
        }
        match how {
            "come_to_the_gate" => {
                r.come_to_the_gate();
                println!("PCK,come_to_the_gate lands in {:?}", r.room().kind);
                r.migrate_kingdom();
                r.leave_kingdom_place();
            }
            "walk_kingdom(keep)" => r.walk_kingdom(Some(geography::keep_stand())),
            "walk_kingdom(None)" => r.walk_kingdom(None),
            _ => {
                r.walk_kingdom(Some(geography::keep_stand()));
                let geo = geography::geography(&r.kingdom.wishes);
                let (i, _) = geo.sites.iter().enumerate().find(|(_, s)| s.room == RoomKind::Smithy).unwrap();
                r.enter_kingdom_place(i);
                r.leave_kingdom_place();
            }
        }
        assert_eq!(r.room().kind, RoomKind::Kingdom, "{how}");
        let geo = geography::geography(&r.kingdom.wishes);
        let keep = geography::keep_stand();
        let d = pc_dist(&geo.room, keep);
        let cols = geo.room.cols as i32;
        let gate = geography::gate_stand(&r.kingdom.wishes);
        for id in 1..=4u32 {
            let h = &r.players[&id];
            let t = pc_tile(h);
            let reached = d[(t.1 * cols + t.0) as usize] >= 0;
            let on_floor = r.settlement_walkable_for_test(h.x, h.y);
            println!("PCK,spawn,{how},knight {id},tile {t:?},floor {on_floor},reachable {reached}");
            if !(on_floor && reached) {
                fails.push(format!("{how}: knight {id} spawned at {t:?} (floor {on_floor}, reachable {reached})"));
                continue;
            }
            let mut w = r.clone();
            match pc_follow(&mut w, id, gate, 9000) {
                Ok(ticks) => println!("PCK,spawn-to-gate,{how},knight {id},ticks {ticks}"),
                Err(e) => fails.push(format!("{how}: knight {id} to the gate: {e}")),
            }
        }
    }
    for f in &fails {
        println!("PCK,FAIL,{f}");
    }
    assert!(fails.is_empty());
}

/// The live save, loaded as the host restores one (`Saved` in
/// `cockpit/src/app/control/dungeon_session.rs`), and then played on.
/// `LIVE_SESSION=<session.json> cargo test -p angel-delve --lib party_check_live_session -- --ignored --nocapture`
#[test]
#[ignore]
fn party_check_live_session() {
    #[derive(serde::Deserialize)]
    struct Saved {
        version: u32,
        run: Run,
        #[serde(default)]
        country_intents: BTreeMap<String, u32>,
    }
    let path = std::env::var_os("LIVE_SESSION").expect("LIVE_SESSION=<session.json>");
    let bytes = std::fs::read(&path).unwrap();
    println!("PCL,bytes,{}", bytes.len());
    assert!(bytes.len() <= 2 * 1024 * 1024, "dungeon save is too large");
    let mut saved: Saved = serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("does not read: {e}"));
    println!("PCL,version,{},intents,{}", saved.version, saved.country_intents.len());
    let mended = saved.run.mend();
    println!("PCL,mended,{mended:?}");
    let before_valid = saved.run.valid_snapshot();
    let kingdom_room = crate::rules::world::room_of(&saved.run.dungeon, RoomKind::Kingdom).unwrap();
    let r0 = &saved.run.dungeon.rooms[kingdom_room];
    println!("PCL,before,valid,{before_valid},at,{},kind,{:?},kingdom-room {}x{}", saved.run.at, saved.run.room().kind, r0.cols, r0.rows);
    for (id, h) in &saved.run.players {
        println!("PCL,before,knight {id} {} at ({:.1},{:.1}) hp {}/{} absent {} away {} stone {}", h.name, h.x, h.y, h.hp, h.max_hp, h.absent, h.away, h.stone);
    }
    assert!(before_valid, "invalid dungeon checkpoint");
    let mut run = saved.run;
    run.marks = saved.country_intents;
    crate::rules::world::raise_world(&mut run.dungeon);
    run.migrate_kingdom();
    let valid = run.valid_snapshot();
    let r1 = &run.dungeon.rooms[kingdom_room];
    println!("PCL,after,valid,{valid},at,{},kind,{:?},kingdom-room {}x{}", run.at, run.room().kind, r1.cols, r1.rows);
    assert!(valid, "invalid migrated dungeon checkpoint");
    let keep = geography::keep_stand();
    let d = pc_dist(&run.dungeon.rooms[kingdom_room], keep);
    let cols = run.dungeon.rooms[kingdom_room].cols as i32;
    let geo = geography::geography(&run.kingdom.wishes);
    let gate = geography::gate_stand(&run.kingdom.wishes);
    for (id, h) in &run.players {
        let t = pc_tile(h);
        let reach = run.room().kind == RoomKind::Kingdom && d[(t.1 * cols + t.0) as usize] >= 0;
        println!(
            "PCL,after,knight {id} at ({:.1},{:.1}) tile {t:?} keep {keep:?} walkable {} reachable-from-keep {reach} steps-to-gate {}",
            h.x,
            h.y,
            run.settlement_walkable_for_test(h.x, h.y),
            d[(gate.1 * cols + gate.0) as usize]
        );
        assert!(run.settlement_walkable_for_test(h.x, h.y));
        assert!(reach, "the knights land somewhere the kingdom cannot be walked from");
    }
    println!("PCL,return_to,{:?}", run.kingdom.return_to.as_ref().map(|b| (b.name.clone(), b.x, b.y)));
    {
        let all = crate::world::finds::finds(run.seed);
        let lost = all.iter().filter(|f| d[(f.at.1 * cols + f.at.0) as usize] < 0).count();
        let posts = crate::world::signs::posts();
        let post_lost = posts.iter().filter(|p| d[(p.at.1 * cols + p.at.0) as usize] < 0).count();
        println!("PCL,live-seed,{},finds {},unreachable {lost},posts {},unreachable {post_lost}", run.seed, all.len(), posts.len());
        assert_eq!((all.len(), lost, post_lost), (crate::world::finds::COUNT, 0, 0));
    }
    // The wire a client reads: the freeze of 10-17 came from here.
    let tree = crate::wire::publish(&run).unwrap();
    let back = crate::wire::read(&tree).unwrap();
    println!("PCL,wire,drawable,{}", crate::wire::drawable(&back));
    assert!(crate::wire::drawable(&back));
    // A save of it reads back.
    let again: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert!(again.valid_snapshot());
    // Friends take the seats the host leaves; the party plays: stands, walks
    // to the gate and far into the country, enters the delve's gate.
    for (id, n) in [(2, "Wren"), (3, "Bors"), (4, "Ysolde")] {
        run.join_seat(id, n);
    }
    let mut far = (keep, 0);
    for y in 0..geo.room.rows as i32 {
        for x in 0..cols {
            let s = d[(y * cols + x) as usize];
            if s > far.1 {
                far = ((x, y), s);
            }
        }
    }
    for id in 2..=4u32 {
        let t = pc_tile(&run.players[&id]);
        let ok = d[(t.1 * cols + t.0) as usize] >= 0 && run.settlement_walkable_for_test(run.players[&id].x, run.players[&id].y);
        println!("PCL,joined,knight {id},tile {t:?},ok {ok}");
        assert!(ok);
        let mut w = run.clone();
        let to_gate = pc_follow(&mut w, id, gate, 9000);
        let mut w = run.clone();
        let to_far = pc_follow(&mut w, id, far.0, 12000);
        println!("PCL,walk,knight {id},gate {to_gate:?},farthest {:?} {to_far:?}", far.0);
        assert!(to_gate.is_ok() && to_far.is_ok());
    }
    let mut w = run.clone();
    pc_follow(&mut w, 2, gate, 9000).unwrap();
    w.step(&BTreeMap::new());
    w.step(&BTreeMap::from([(2, Input { interact: true, ..Default::default() })]));
    println!("PCL,gate-door,{:?}", w.room().kind);
    assert_eq!(w.room().kind, RoomKind::Gate);
    // Four minutes of play, standing, then pacing: still a valid save and a
    // drawable wire.
    for (mode, walk) in [("stand", false), ("pace", true)] {
        let mut w = run.clone();
        for t in 0..4 * 60 * HZ {
            let inputs: BTreeMap<u32, Input> = (2..=4u32)
                .map(|id| {
                    let mut i = Input { aim_x: 1, ..Default::default() };
                    if walk {
                        i.move_x = ((t / 70 + id) % 3) as i8 - 1;
                        i.move_y = if (t / 45) % 2 == 0 { -1 } else { 1 };
                    }
                    (id, i)
                })
                .collect();
            w.step(&inputs);
            if t % (30 * HZ) == 0 {
                assert!(w.valid_snapshot(), "{mode}: invalid at tick {t}");
                let back = crate::wire::read(&crate::wire::publish(&w).unwrap()).unwrap();
                assert!(crate::wire::drawable(&back), "{mode}: undrawable at tick {t}");
            }
        }
        println!("PCL,play,{mode},ok,room {:?}", w.room().kind);
    }
}

/// Old saves in other places, made from the live run: a party out in the
/// legacy-size kingdom at various spots, and one inside each kind of place
/// with its return door on the old map. Each must migrate onto the new map
/// somewhere the kingdom can be walked from.
#[test]
#[ignore]
fn party_check_legacy_saves() {
    let keep = geography::keep_stand();
    let (lw, lh) = crate::world::map::LEGACY_MAP;
    let legacy = || Room::kingdom(vec![Tile::Floor; (lw * lh) as usize], lw as usize, lh as usize);
    let mut fails: Vec<String> = Vec::new();
    // Out on the old surface, at assorted old tiles.
    let spots = [(1, 1), (5, 5), (56, 60), (100, 110), (111, 120), (3, 118), (110, 2), (48, 14)];
    for &(sx, sy) in &spots {
        let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
        r.join_seat(3, "Bors");
        r.walk_kingdom(Some(keep));
        let i = crate::rules::world::room_of(&r.dungeon, RoomKind::Kingdom).unwrap();
        r.dungeon.rooms[i] = legacy();
        r.kingdom.return_to = None;
        for h in r.players.values_mut() {
            (h.x, h.y) = ((sx as f32 + 0.5) * TILE_UNITS, (sy as f32 + 0.5) * TILE_UNITS);
        }
        // As the server writes and reads it.
        let mut loaded: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        let pre = loaded.valid_snapshot();
        crate::rules::world::raise_world(&mut loaded.dungeon);
        loaded.migrate_kingdom();
        let post = loaded.valid_snapshot();
        let d = pc_dist(&loaded.dungeon.rooms[i], keep);
        let cols = loaded.dungeon.rooms[i].cols as i32;
        let lands: Vec<String> = loaded
            .players
            .values()
            .map(|h| {
                let t = pc_tile(h);
                let ok = loaded.settlement_walkable_for_test(h.x, h.y) && d[(t.1 * cols + t.0) as usize] >= 0;
                if !ok {
                    fails.push(format!("surface save at {:?}: a knight lands at {t:?}, not walkable", (sx, sy)));
                }
                format!("{t:?}:{ok}")
            })
            .collect();
        println!("PCK,legacy-surface,from ({sx},{sy}),valid-before {pre},valid-after {post},lands {lands:?},room {}x{}", loaded.dungeon.rooms[i].cols, loaded.dungeon.rooms[i].rows);
        if !(pre && post) {
            fails.push(format!("surface save at {:?}: valid before {pre}, after {post}", (sx, sy)));
        }
    }
    // Inside a place, the door home on the old map.
    let geo = geography::geography(&[]);
    for (si, site) in geo.sites.iter().enumerate() {
        let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
        r.join_seat(3, "Bors");
        r.walk_kingdom(Some(keep));
        r.enter_kingdom_place(si);
        let i = crate::rules::world::room_of(&r.dungeon, RoomKind::Kingdom).unwrap();
        r.dungeon.rooms[i] = legacy();
        if let Some(back) = &mut r.kingdom.return_to {
            (back.x, back.y) = (60.0, 70.0);
            for spot in back.party.values_mut() {
                *spot = (60.0, 70.0);
            }
        }
        let mut loaded: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        let pre = loaded.valid_snapshot();
        crate::rules::world::raise_world(&mut loaded.dungeon);
        loaded.migrate_kingdom();
        let post = loaded.valid_snapshot();
        let kind_in = loaded.room().kind;
        loaded.leave_kingdom_place();
        let d = pc_dist(&loaded.dungeon.rooms[i], keep);
        let cols = loaded.dungeon.rooms[i].cols as i32;
        let out_ok = loaded.room().kind == RoomKind::Kingdom
            && loaded.players.values().all(|h| {
                let t = pc_tile(h);
                loaded.settlement_walkable_for_test(h.x, h.y) && d[(t.1 * cols + t.0) as usize] >= 0
            });
        println!("PCK,legacy-inside,{} ({:?}),in {kind_in:?},valid-before {pre},valid-after {post},out on walkable ground {out_ok}", site.name, site.room);
        if !(pre && post && out_ok) {
            fails.push(format!("save inside {} ({:?}): valid {pre}/{post}, out ok {out_ok}", site.name, site.room));
        }
    }
    for f in &fails {
        println!("PCK,FAIL,{f}");
    }
    assert!(fails.is_empty(), "{} failures", fails.len());
}
