use super::*;
use crate::rules::{Rng, layout};

/// A registry of the crate's own folder, apart from the process's.
fn folder() -> super::super::assets::Registry {
    super::super::assets::Registry::scan(&super::super::assets::plate_pack())
}

/// Chamber item `id`'s room, as the game lays it with `doors`.
fn chamber(id: &str, doors: [bool; 4]) -> (Room, Pack) {
    let item = pilot::item(id).unwrap();
    let Source::Chamber { pack, small, .. } = item.source else { panic!("{id} is a chamber") };
    let mut room = Room::solid_rock((2, 3));
    room.kind = RoomKind::Fight;
    room.doors = doors;
    let quadrant: Vec<Vec<u8>> = crate::rules::rooms::small(pack)[small].iter().map(|r| r.as_bytes().to_vec()).collect();
    layout::mirror(&mut room, doors, &quadrant);
    (room, pack)
}

#[test]
fn a_chamber_with_a_door_walled_up_is_its_item_and_draws_the_wall() {
    let registry = folder();
    let (room, pack) = chamber("crypt-bone-chapel", [true, false, true, true]);
    let fit = registry.item_for(&room, pack).expect("still the Bone Chapel");
    assert_eq!(fit.item.id, "crypt-bone-chapel");
    let variant = fit.variant.expect("laid otherwise than the item");
    // The east doorway's two tiles differ, and only they.
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    assert_eq!(variant.differs, vec![(cols - 1, rows / 2 - 1), (cols - 1, rows / 2)]);
    // The doorway's leaf, lintel and torches are cut from the item's set;
    // nothing else is.
    let cut = variant.text.lines().filter(|l| l.starts_with("# cut where the room differs")).count();
    assert!(cut >= 2, "the leaf and the lintel at least: {cut}");
    assert!(variant.set.shapes.len() < fit.item.set.shapes.len() + 4);
    assert!(!variant.set.shapes.iter().any(|s| s.id.as_deref() == Some("door-east")), "no east leaf");
    assert!(variant.set.shapes.iter().any(|s| s.id.as_deref() == Some("door-west")), "the west leaf stays");
    // A wall stands in the doorway, of the wall's own stone, as tall as the wall beside it.
    let wall: Vec<&Shape> = variant.extras.iter().filter(|s| s.shape == "box" && !s.on_floor()).collect();
    assert!(!wall.is_empty(), "the walled-up doorway");
    let x = cols as f32 * TILE - TILE / 2.0;
    for s in &wall {
        assert!((s.at[0] - x).abs() <= TILE / 2.0 + 0.01, "in the east wall: {s:?}");
        let z = s.at[2];
        assert!(z > (rows / 2 - 1) as f32 * TILE - 0.01 && z < (rows / 2 + 1) as f32 * TILE + 0.01, "across the doorway: {s:?}");
    }
    // Drawn with its kind's patch where the item has one, as box faces.
    let nodes = variant.extra_nodes(&fit.item, &|k| 9000 + k as u32);
    assert!(!nodes.is_empty());
    assert!(nodes.iter().all(|n| n.id >= EXTRA_IDS));
    // A page that makes the blocks from the set makes the same.
    let mut sets = angel_stage::set::Sets::default();
    let world = super::super::room::RoomWorld::painted(Arc::clone(&fit.item), Some(Arc::clone(&variant)), (768, 448));
    let mut far = angel_stage::scene::World { set: world.set_ref(5, 9031, (0.0, 0.0), ([0.0, 0.0], 1.0), true, false), ..Default::default() };
    assert!(sets.fill(&mut far));
    assert_eq!((far.floor, far.blocks), world.nodes(5, 9031, (0.0, 0.0), ([0.0, 0.0], 1.0)));
    assert_ne!(world.key(), fit.item.key);
}

#[test]
fn a_chamber_with_a_boulder_rolled_in_is_its_item_and_draws_the_boulder() {
    let registry = folder();
    let (mut room, pack) = chamber("cavern-boulder-rill", [true; 4]);
    assert_eq!(registry.item_for(&room, pack).map(|f| f.variant.is_none()), Some(true), "as painted");
    // A loose boulder, mirrored four ways as the dice lay it, on open floor.
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let (c, r) = (1..cols / 2 - 1)
        .flat_map(|c| (1..rows / 2 - 1).map(move |r| (c, r)))
        .find(|&(c, r)| room.tile(c, r) == Tile::Floor && room.tile(cols - 1 - c, r) == Tile::Floor)
        .expect("open floor");
    for (x, y) in [(c, r), (cols - 1 - c, r), (c, rows - 1 - r), (cols - 1 - c, rows - 1 - r)] {
        room.set(x as usize, y as usize, Tile::Block);
    }
    let fit = registry.item_for(&room, pack).expect("still the Boulder Rill");
    let variant = fit.variant.expect("with a boulder the item has not");
    assert_eq!(variant.differs.len(), 4);
    for t in &variant.differs {
        assert!(variant.over_tile(*t).is_some_and(|s| !s.on_floor()), "a block stands on {t:?}");
    }
    // Nothing of the item's is cut: the floor stays, the boulder is extra.
    assert!(!variant.text.contains("# cut where the room differs"));
    // A terminal paints the tile with it.
    let world = super::super::room::RoomWorld::painted(Arc::clone(&fit.item), Some(Arc::clone(&variant)), (384, 224));
    let laid = world.picture().expect("the plate decodes");
    let plain = super::super::room::RoomWorld::of(Arc::clone(&fit.item), (384, 224)).picture().unwrap();
    let mid = ((c as f32 + 0.5) * TILE) as i32;
    let row = ((r as f32 + 0.5) * TILE) as i32;
    assert_ne!(laid.get(mid, row), plain.get(mid, row), "the boulder's tile is painted");
    assert_eq!(laid.get(192, 112), plain.get(192, 112), "the rest is the plate");
}

#[test]
fn a_room_of_another_template_or_past_the_dice_is_not_the_item() {
    let registry = folder();
    // Another of the Mines' chambers.
    let (other, pack) = chamber("cavern-stalactite-garden", [true; 4]);
    assert!(registry.item_for(&other, pack).is_none_or(|f| f.item.id != "cavern-boulder-rill"));
    // The Crypt scatters nothing: a block on the chapel's floor makes it another room.
    let (mut room, pack) = chamber("crypt-bone-chapel", [true; 4]);
    let (c, r) = (2..room.rows as i32 - 2)
        .flat_map(|r| (2..room.cols as i32 - 2).map(move |c| (c, r)))
        .find(|&(c, r)| room.tile(c, r) == Tile::Floor)
        .expect("open floor");
    room.set(c as usize, r as usize, Tile::Block);
    assert!(registry.item_for(&room, pack).is_none(), "no loose blocks in the Crypt");
    // More loose blocks than the Mines' dice lay: another room.
    let (mut room, pack) = chamber("cavern-boulder-rill", [true; 4]);
    let open: Vec<(i32, i32)> = (1..room.rows as i32 - 1)
        .flat_map(|r| (1..room.cols as i32 - 1).map(move |c| (c, r)))
        .filter(|&(c, r)| room.tile(c, r) == Tile::Floor)
        .take(4 * pack.scatter() + 1)
        .collect();
    for &(c, r) in &open {
        room.set(c as usize, r as usize, Tile::Block);
    }
    assert!(registry.item_for(&room, pack).is_none(), "past the dice");
    // A hazard filled in, or a wall knocked through inside, is another room too.
    let (mut room, pack) = chamber("cavern-boulder-rill", [true; 4]);
    let pool = (0..room.rows as i32).flat_map(|r| (0..room.cols as i32).map(move |c| (c, r))).find(|&(c, r)| room.tile(c, r) == Tile::Hazard);
    if let Some((c, r)) = pool {
        room.set(c as usize, r as usize, Tile::Floor);
        assert!(registry.item_for(&room, pack).is_none(), "a pool filled in");
    }
}

#[test]
fn rooms_the_dice_lay_from_a_template_with_other_doors_and_boulders_are_its_item() {
    // Over the dice, some fight rooms of a delve that scatters boulders are
    // a chamber item laid otherwise, and are drawn as it.
    let registry = folder();
    let mut found = 0;
    for seed in 0..200u64 {
        let floor = layout::floor(1, Pack::Cavern, &mut Rng::new(seed));
        for room in &floor.rooms {
            if let Some(fit) = registry.item_for(room, Pack::Cavern)
                && fit.variant.is_some()
            {
                found += 1;
            }
        }
    }
    assert!(found > 0, "a Mines floor lays a painted chamber with other doors or loose boulders");
}

/// `cargo test -p angel-delve world3d_floor_sample -- --ignored --nocapture`:
/// how many rooms of seeded floors (each delve, floors 1 to 3, a party of one
/// and of two) have an item, laid exactly as it or otherwise, and how many
/// stand up in 3D at all (an item, or plain: `world3d::plain`); and the same
/// of open floors (`ANGEL_DELVE_OPEN`), its map and its side rooms.
#[test]
#[ignore = "prints a sample"]
fn world3d_floor_sample() {
    let registry = folder();
    let plain = |room: &Room, pack: Pack| super::super::plain::plain_for(room, pack).is_some();
    for party in [1usize, 2] {
        let mut flat = Vec::new();
        let (mut rooms, mut exact, mut loose, mut stood) = (0, 0, 0, 0);
        let mut by_pack = Vec::new();
        for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp] {
            let (mut r0, mut e0, mut l0, mut s0) = (0, 0, 0, 0);
            for seed in 0..40u64 {
                for depth in 1..=3 {
                    let floor = layout::floor_party(depth, pack, party, &mut Rng::new(seed * 7 + u64::from(depth)));
                    for room in &floor.rooms {
                        r0 += 1;
                        match registry.item_for(room, pack) {
                            Some(f) if f.variant.is_none() => (e0, s0) = (e0 + 1, s0 + 1),
                            Some(_) => (l0, s0) = (l0 + 1, s0 + 1),
                            None if plain(room, pack) => s0 += 1,
                            None => flat.push(room.kind),
                        }
                    }
                }
            }
            by_pack.push(format!("{pack:?} {e0}+{l0}/{r0} ({s0} in 3D)"));
            (rooms, exact, loose, stood) = (rooms + r0, exact + e0, loose + l0, stood + s0);
        }
        println!(
            "party of {party}: {} of {rooms} rooms have their item ({exact} exact, {loose} laid otherwise), {stood} ({:.1}%) stand up in 3D: {}",
            exact + loose,
            100.0 * stood as f64 / rooms.max(1) as f64,
            by_pack.join(", ")
        );
        let mut kinds: Vec<(String, usize)> = Vec::new();
        for kind in &flat {
            let word = format!("{kind:?}");
            match kinds.iter_mut().find(|(w, _)| *w == word) {
                Some((_, n)) => *n += 1,
                None => kinds.push((word, 1)),
            }
        }
        println!("  drawn flat: {}", kinds.iter().map(|(w, n)| format!("{w} {n}")).collect::<Vec<_>>().join(", "));
    }
    let (mut rooms, mut items, mut stood) = (0, 0, 0);
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp] {
        for seed in 0..10u64 {
            for depth in 1..=3 {
                let floor = layout::open_floor(depth, pack, 1, &mut Rng::new(seed * 7 + u64::from(depth)));
                for room in &floor.rooms {
                    rooms += 1;
                    if registry.item_for(room, pack).is_some() {
                        (items, stood) = (items + 1, stood + 1);
                    } else if plain(room, pack) {
                        stood += 1;
                    }
                }
            }
        }
    }
    println!("open floors, a party of one: {items} of {rooms} rooms have their item, {stood} ({:.1}%) stand up in 3D", 100.0 * stood as f64 / rooms.max(1) as f64);
}
