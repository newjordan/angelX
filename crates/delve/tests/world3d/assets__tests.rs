use super::*;
use crate::rules::{RoomKind, Run, home::Home, world};

/// A registry of the crate's own folder, apart from the process's.
fn crate_folder() -> Registry {
    Registry::scan(&super::super::assets::plate_pack())
}

#[test]
fn every_item_the_folder_has_a_plate_for_reads() {
    let registry = crate_folder();
    let ids = registry.ids();
    let stale = registry.stale();
    for id in ids.iter().filter(|id| !stale.contains(id)) {
        let item = registry.item(id).unwrap_or_else(|| panic!("{id} reads"));
        assert_eq!((item.id, item.set.id.as_str()), (*id, *id));
        let plate = item.plate().unwrap_or_else(|| panic!("{id}: the plate decodes"));
        assert_eq!((plate.width, plate.height), item.set.plate, "{id}: a plate is its capture's size");
        assert_eq!(plate.rgba.len(), (plate.width * plate.height * 4) as usize);
        // Asked again, the same item, read once.
        assert!(Arc::ptr_eq(&item, &registry.item(id).unwrap()));
        assert_eq!(Some(item.slot), pilot::slot(id));
    }
}

#[test]
fn no_plate_is_stale() {
    let stale = crate_folder().stale();
    assert!(
        stale.is_empty(),
        "plates painted from another set than the generator makes of their room today (repaint them, or put \
         back the layout they were painted from): {stale:?}"
    );
}

#[test]
fn a_missing_folder_has_no_items() {
    let none = Registry::scan(Path::new("/nonexistent/world3d"));
    assert!(none.ids().is_empty());
    let run = Run::at_home(0xD3D, 0xD3D, Some("Friend"), Home::default(), Default::default());
    let room = &run.dungeon.rooms[world::room_of(&run.dungeon, RoomKind::Village).expect("a place")];
    assert!(none.item_for(room, run.dungeon.pack).is_none());
}

#[test]
fn the_hash_is_sha256() {
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn each_place_of_the_kingdom_has_its_item_as_a_run_raises_it() {
    let registry = crate_folder();
    let run = Run::at_home(0xD3D, 0xD3D, Some("Friend"), Home::default(), Default::default());
    for item in pilot::PILOT.iter() {
        let pilot::Source::Place(kind) = item.source else { continue };
        let room = &run.dungeon.rooms[world::room_of(&run.dungeon, kind).expect("a place")];
        let got = registry.item_for(room, run.dungeon.pack).map(|f| f.item.id);
        assert_eq!(got, Some(item.id()), "{kind:?}");
    }
    // A room with no plate has no item: a folder that holds only the
    // Village's (whichever plates this checkout's asset pack has).
    let dir = std::env::temp_dir().join(format!("world3d-only-village-{}", std::process::id()));
    let village = dir.join("village");
    std::fs::create_dir_all(&village).unwrap();
    let from = super::super::assets::plate_pack().join("village");
    for file in ["plate.png", "plate.set-sha256", "swatches.png"] {
        let _ = std::fs::copy(from.join(file), village.join(file));
    }
    let only = Registry::scan(&dir);
    let room = |kind| &run.dungeon.rooms[world::room_of(&run.dungeon, kind).expect("a place")];
    assert_eq!(only.item_for(room(RoomKind::Village), run.dungeon.pack).map(|f| f.item.id), Some("village"));
    assert!(only.item_for(room(RoomKind::Meadow), run.dungeon.pack).is_none(), "the meadow, with no plate");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_chamber_is_its_item_on_any_floor_that_lays_it_as_it_was_painted() {
    let registry = crate_folder();
    for item in pilot::PILOT.iter() {
        let pilot::Source::Chamber { pack, .. } = item.source else { continue };
        let mut room = item.room();
        room.cell = (3, 5);
        assert_eq!(registry.item_for(&room, pack).map(|f| f.item.id), Some(item.id()), "{}: wherever it lies", item.id());
        let other = if pack == Pack::Crypt { Pack::Cavern } else { Pack::Crypt };
        assert!(registry.item_for(&room, other).is_none(), "{}: only in its own delve", item.id());
    }
    // A floor that lays the Bone Chapel with its four doors has it (the dice
    // lay it so on about one one-screen fight room in thirty).
    let chapel = registry.item("crypt-bone-chapel").unwrap();
    let found = (0..64u64).any(|seed| {
        let floor = crate::rules::layout::floor(1, Pack::Crypt, &mut crate::rules::Rng::new(seed));
        floor.rooms.iter().any(|r| registry.item_for(r, Pack::Crypt).is_some_and(|f| Arc::ptr_eq(&f.item, &chapel)))
    });
    assert!(found, "a crypt floor lays the Bone Chapel in some seed");
}

#[test]
fn an_empty_or_broken_plate_or_a_refused_set_is_no_item() {
    let village = pilot::item("village").unwrap();
    let good = crate_folder().item("village").expect("the village reads");
    let (text, png) = (Arc::clone(&good.text), Arc::clone(&good.png));
    let none: Arc<[u8]> = Arc::from(&[][..]);
    assert!(Item::new(village, Arc::clone(&text), Arc::clone(&png), Arc::clone(&none)).is_ok());
    assert!(Item::new(village, Arc::clone(&text), Arc::clone(&none), Arc::clone(&none)).is_err(), "a missing plate");
    assert!(Item::new(village, Arc::clone(&text), Arc::from(&png[..20]), Arc::clone(&none)).is_err(), "cut short");
    assert!(Item::new(village, "set broken\n".into(), Arc::clone(&png), Arc::clone(&none)).is_err(), "no camera");
    let market = pilot::item("market").unwrap();
    assert!(Item::new(market, Arc::clone(&text), Arc::clone(&png), Arc::clone(&none)).is_err(), "another item's set");
    // The key follows the set and the plate.
    let more: Arc<str> = format!("{text}# one more line\n").into();
    assert_ne!(Item::new(village, more, Arc::clone(&png), Arc::clone(&none)).unwrap().key, good.key);
    // A PNG broken past its header reads as an item whose pixels are not there.
    let broken: Arc<[u8]> = Arc::from(&png[..4096]);
    assert!(Item::new(village, Arc::clone(&text), broken, none).is_ok_and(|i| i.plate().is_none()));
}

#[test]
fn a_stale_plate_is_no_item() {
    // A folder whose one item's plate was painted from another set.
    let dir = std::env::temp_dir().join(format!("world3d-stale-{}", std::process::id()));
    let item = dir.join("village");
    std::fs::create_dir_all(&item).unwrap();
    let good = crate_folder().item("village").unwrap();
    std::fs::write(item.join("plate.png"), &good.png[..]).unwrap();
    std::fs::write(item.join("plate.set-sha256"), sha256_hex(b"another set")).unwrap();
    let registry = Registry::scan(&dir);
    assert_eq!(registry.ids(), vec!["village"], "it has a plate");
    assert_eq!(registry.stale(), vec!["village"]);
    assert!(registry.item("village").is_none(), "but it is stale");
    // Painted from the set the generator makes today, it reads.
    std::fs::write(item.join("plate.set-sha256"), sha256_hex(good.text.as_bytes())).unwrap();
    assert!(Registry::scan(&dir).item("village").is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_page_keeps_the_items_it_is_sent() {
    // A page's registry: no folder.
    let page = Registry::scan(Path::new("/nonexistent/world3d"));
    let village = crate_folder().item("village").unwrap();
    let run = Run::at_home(0xD3D, 0xD3D, Some("Friend"), Home::default(), Default::default());
    let room = &run.dungeon.rooms[world::room_of(&run.dungeon, RoomKind::Village).unwrap()];
    assert!(page.item_for(room, run.dungeon.pack).is_none(), "before it is sent");
    assert!(page.receive("village", village.key, &village.text, &village.png, &village.swatches_png));
    let got = page.item_for(room, run.dungeon.pack).expect("sent, it is the village's");
    assert_eq!((got.item.id, got.item.key), ("village", village.key));
    // Sent again, the same one.
    assert!(page.receive("village", village.key, &village.text, &village.png, &[]));
    assert!(Arc::ptr_eq(&page.item("village").unwrap(), &got.item));
    // What does not read is not kept; an id that is no item neither.
    assert!(!page.receive("market", 1, &village.text, &village.png, &[]));
    assert!(!page.receive("no-such-item", 2, &village.text, &village.png, &[]));
    // It keeps the newest few.
    let chapel = crate_folder().item("crypt-bone-chapel").unwrap();
    assert!(page.receive("crypt-bone-chapel", chapel.key, &chapel.text, &chapel.png, &[]));
    assert!(page.item("village").is_some() && page.item("crypt-bone-chapel").is_some());
}
