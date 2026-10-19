use super::*;
use crate::rules::{Rng, Tile, layout};
use angel_stage::scene::{Node, TextureSource};

fn picture_of(n: &Node) -> Option<u32> {
    match n.material.source {
        TextureSource::Picture(id) => Some(id),
        TextureSource::None => None,
    }
}

/// A room of a seeded Crypt floor that no painted item is.
fn crypt_room() -> Room {
    let floor = layout::floor_party(1, Pack::Crypt, 1, &mut Rng::new(3));
    floor
        .rooms
        .iter()
        .find(|r| r.kind == RoomKind::Fight && super::super::assets::registry().item_for(r, Pack::Crypt).is_none())
        .expect("a fight room with no item")
        .clone()
}

/// The picture a test names kind `k`'s patch: one past a hundred.
fn swatch(k: Kind) -> u32 {
    100 + k as u32
}

fn unique_ids(nodes: &[Node]) -> bool {
    let mut ids: Vec<u64> = nodes.iter().map(|n| n.id).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.len() == nodes.len()
}

#[test]
fn a_room_with_no_item_stands_up_as_its_own_blockout_in_its_themes_patches() {
    use super::super::room::{self, enabled, set_enabled};
    super::set_on(true);
    let room = crypt_room();
    let size = ((room.cols * 16) as i32, (room.rows * 16) as i32);
    assert!(!enabled());
    assert!(room::world(&room, Pack::Crypt, 1, size).is_none(), "off: drawn as it always was");
    crate::world3d::rooms_lit::set_plates(true);
    set_enabled(true);
    let world = room::world(&room, Pack::Crypt, 1, size).expect("in 3D, every room stands up");
    set_enabled(false);
    let plain = world.plain().expect("no item: plain").clone();
    assert!(world.item().is_none() && world.set().is_none());
    assert_eq!((plain.theme, plain.grid), (Theme::Delve(Pack::Crypt), (1, 1)));
    // Its blocks are the room's own blockout.
    let piece = plain.piece((0, 0)).expect("its one piece");
    assert_eq!(&*piece.text, blockout::room_set(&room, &dressing_for(&room, Pack::Crypt).unwrap().0));
    let (floor, standing) = world.plain_nodes(&swatch, (0.0, 0.0), ([0.0, 0.0], 1.0), (384.0, 224.0), &|_, _| true);
    assert!(!floor.is_empty() && !standing.is_empty());
    let all: Vec<Node> = floor.iter().chain(&standing).cloned().collect();
    assert!(unique_ids(&all), "a scene draws one node an id");
    // Each block of its kind's patch of the theme's, or its colour two shades
    // down where the theme has none: never the raw capture colour.
    let raw: Vec<[f32; 3]> = Kind::ALL.iter().map(|k| k.capture().map(|v| f32::from(v) / 255.0)).collect();
    for node in all.iter().filter(|n| n.material.source == TextureSource::None) {
        let t = node.material.tint;
        assert!(!raw.iter().any(|r| (0..3).all(|k| (r[k] - t[k]).abs() < 1e-3)), "a raw capture colour: {t:?}");
    }
    let pictures: Vec<u32> = all.iter().filter_map(|n| picture_of(n)).collect();
    assert!(!pictures.is_empty() && pictures.iter().all(|p| (100..100 + 27).contains(p)), "the theme's patches");
    // Out of sight, nothing.
    let (none, nothing) = world.plain_nodes(&swatch, (0.0, 0.0), ([0.0, 0.0], 1.0), (384.0, 224.0), &|_, _| false);
    assert!(none.is_empty() && nothing.is_empty());
    // A painted room's draws are as they were: none of these.
    assert!(world.nodes(7, 9031, (0.0, 0.0), ([0.0, 0.0], 1.0)).0.is_empty());
    assert!(world.set_ref(7, 9031, (0.0, 0.0), ([0.0, 0.0], 1.0), true, true).is_none());
}

#[test]
fn the_kingdom_and_the_side_on_halls_never_stand_up_plain() {
    for kind in [RoomKind::Kingdom, RoomKind::Ledge, RoomKind::Wanted] {
        let mut room = Room::solid_rock((0, 0));
        room.kind = kind;
        assert!(dressing_for(&room, Pack::Crypt).is_none(), "{kind:?}");
    }
    // A room of a kind an item was made from is dressed as it, in this
    // floor's stone; a passage as a chamber of its delve.
    let mut nook = crate::rules::nooks::room(RoomKind::Kitchen, (0, 0), 3);
    nook.kind = RoomKind::Kitchen;
    let (d, theme) = dressing_for(&nook, Pack::Fungal).unwrap();
    assert_eq!((d.id, d.setting, theme), ("nook-kitchen", Setting::Dungeon(Pack::Fungal), Theme::Delve(Pack::Fungal)));
    let mut hall = Room::solid_rock((0, 0));
    hall.kind = RoomKind::Hall;
    assert_eq!(dressing_for(&hall, Pack::Archive).map(|(d, t)| (d.setting, t)), Some((Setting::Dungeon(Pack::Archive), Theme::Delve(Pack::Archive))));
}

#[test]
fn an_open_floors_map_stands_up_in_pieces_near_the_view() {
    let floor = layout::open_floor(2, Pack::Cavern, 1, &mut Rng::new(29));
    let map = floor.rooms.iter().find(|r| r.kind == RoomKind::Depths).expect("the map").clone();
    let plain = plain_for(&map, Pack::Cavern).expect("the map stands up");
    assert!(plain.open() && plain.grid.0 > 1 && plain.grid.1 > 1);
    // A view of the map's middle: the pieces it overlaps made at once, a few
    // of the ring round them a call.
    let (w, h) = plain.stage();
    let view = [w / 2.0 - 192.0, h / 2.0 - 112.0, w / 2.0 + 192.0, h / 2.0 + 112.0];
    let first = plain.near(view);
    let overlapped = super::super::kingdom::near(view).len();
    assert!(first.len() >= 4 && first.len() <= 4 + RING_A_CALL, "{} pieces at first", first.len());
    let mut all = first.clone();
    for _ in 0..16 {
        all = plain.near(view);
    }
    assert_eq!(all.len(), overlapped.min(16), "then the ring round them too");
    // Each piece stands on its own tiles: nothing of it past them (a round
    // thing's middle in them), its ground never under another's.
    for piece in &all {
        let [x0, z0, x1, z1] = blockout::px(piece.tiles);
        for s in &piece.shapes {
            if super::super::fit::is_box(s) {
                let (hx, hz) = (s.size[0] / 2.0, s.size[2] / 2.0);
                assert!(s.at[0] - hx >= x0 - 0.3 && s.at[0] + hx <= x1 + 0.3 && s.at[2] - hz >= z0 - 0.3 && s.at[2] + hz <= z1 + 0.3, "{:?}: {s:?}", piece.cell);
            } else {
                assert!(s.at[0] >= x0 && s.at[0] < x1 && s.at[2] >= z0 && s.at[2] < z1, "{:?}: {s:?}", piece.cell);
            }
        }
        // Every wall tile of its own under a block, every walkable one under none taller than a boot.
        let (c0, r0, pw, ph) = piece.tiles;
        for r in r0..r0 + ph {
            for c in c0..c0 + pw {
                let top = over_tile(&piece.shapes, (c, r)).map(|s| s.at[1] + s.size[1]);
                match map.tile(c, r) {
                    Tile::Wall => assert!(top.is_some_and(|t| t >= 2.0), "{c},{r}: a wall with nothing on it: {top:?}"),
                    t if blockout::walkable(t) => assert!(top.is_none_or(|t| t <= blockout::LOW), "{c},{r}: walkable under {top:?}"),
                    _ => {}
                }
            }
        }
        // Its blocks' ids its own.
        let blocks = plain.blocks(piece, &swatch);
        let nodes: Vec<Node> = blocks.floor.iter().chain(&blocks.standing).cloned().collect();
        assert!(unique_ids(&nodes));
    }
    let ids: Vec<u64> = all.iter().flat_map(|p| {
        let b = plain.blocks(p, &swatch);
        b.floor.iter().chain(&b.standing).map(|n| n.id).collect::<Vec<_>>()
    }).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "every piece's nodes their own ids");
    // The map changed in one piece (a gate shut): the others are kept as made.
    let mut shut = map.clone();
    let (c, r) = (0..map.rows as i32).flat_map(|r| (0..map.cols as i32).map(move |c| (c, r))).find(|&(c, r)| map.tile(c, r) == Tile::Floor).unwrap();
    shut.set(c as usize, r as usize, Tile::Door);
    let again = plain_for(&shut, Pack::Cavern).unwrap();
    let far = (0..plain.grid.1).flat_map(|z| (0..plain.grid.0).map(move |x| (x, z))).find(|&(x, z)| {
        let (c0, r0) = (x * PIECE.0 - 1, z * PIECE.1 - 1);
        !(c0..c0 + PIECE.0 + 2).contains(&c) || !(r0..r0 + PIECE.1 + 2).contains(&r)
    }).unwrap();
    assert!(Arc::ptr_eq(&plain.piece(far).unwrap(), &again.piece(far).unwrap()), "an unchanged piece is kept");
}

#[test]
fn a_terminal_paints_each_tile_with_what_stands_tallest_on_it() {
    let room = crypt_room();
    let plain = plain_for(&room, Pack::Crypt).expect("the Crypt has patches");
    let size = ((room.cols * 16) as i32, (room.rows * 16) as i32);
    let img = plain.picture(size);
    assert_eq!((img.w, img.h), size);
    let piece = plain.piece((0, 0)).unwrap();
    // A wall tile is its wall's patch; an open tile its floor's, shaded under
    // what stands; a pit black.
    let mid = |(c, r): (i32, i32)| img.get(c * 16 + 8, r * 16 + 8).unwrap();
    let wall = (room.cols as i32 / 2, 0);
    let top = over_tile(&piece.shapes, wall).expect("the far wall");
    assert!(top.at[1] + top.size[1] > 24.0 && matches!(top.kind, Kind::Stone | Kind::Door));
    let floor = (1..room.rows as i32 - 1).flat_map(|r| (1..room.cols as i32 - 1).map(move |c| (c, r))).find(|&(c, r)| room.tile(c, r) == Tile::Floor && over_tile(&piece.shapes, (c, r)).is_some_and(|s| s.on_floor())).unwrap();
    assert_ne!(mid(wall), mid(floor));
    if let Some(pit) = (0..room.rows as i32).flat_map(|r| (0..room.cols as i32).map(move |c| (c, r))).find(|&(c, r)| room.tile(c, r) == Tile::Hazard) {
        assert_eq!(mid(pit), [0, 0, 0], "a pit, a hole in the floor");
    }
    assert_eq!(plain.picture(size), img, "kept");
}

#[test]
fn a_page_draws_a_room_with_no_item_from_the_theme_it_was_sent() {
    let room = crypt_room();
    let host = plain_for(&room, Pack::Crypt).expect("the host's folder has the Crypt's patches");
    // A page: no folder of its own (an empty one), only what it is sent.
    let empty = std::env::temp_dir().join(format!("world3d-plain-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    super::super::assets::with_folder(&empty, || {
        assert!(plain_for(&room, Pack::Crypt).is_none(), "nothing sent: drawn flat");
        assert!(super::super::assets::themes().receive("crypt", host.patches.key, &host.patches.png));
        let page = plain_for(&room, Pack::Crypt).expect("sent the Crypt's patches");
        assert_eq!(page.key, host.key, "the same patches, the same room");
        let made = |p: &Plain| {
            let piece = p.piece((0, 0)).unwrap();
            let b = p.blocks(&piece, &swatch);
            b.floor.iter().chain(&b.standing).map(|n| (n.id, n.transform, picture_of(n))).collect::<Vec<_>>()
        };
        assert_eq!(made(&page), made(&host), "the same blocks, the same patches");
    });
}

#[test]
fn the_tree_carries_the_rooms_theme_only_while_rooms_are_drawn_in_3d() {
    use crate::wire::{WORLD_THEME, publish, take_world_theme};
    super::set_on(true);
    let room = crypt_room();
    let mut run = crate::rules::Run::new(3, 3, Some("Friend"));
    run.dungeon = layout::floor_party(1, Pack::Crypt, 1, &mut Rng::new(3));
    let at = run.dungeon.rooms.iter().position(|r| r.cell == room.cell).unwrap();
    run.enter_for_test(at);
    let off = publish(&run).unwrap();
    assert!(off.field(WORLD_THEME).is_none(), "off: the tree is as it was");
    crate::world3d::rooms_lit::set_plates(true);
    super::super::set_enabled(true);
    let on = publish(&run).unwrap();
    super::super::set_enabled(false);
    let theme = on.field(WORLD_THEME).expect("the Crypt's patches");
    let want = super::super::assets::themes().get(Theme::Delve(Pack::Crypt)).unwrap();
    assert_eq!(theme.field("theme"), Some(&angel_net::doc::value::Value::Str("crypt".into())));
    assert_eq!(theme.field("key"), Some(&angel_net::doc::value::Value::U(want.key)));
    assert!(take_world_theme(&on) && !take_world_theme(&off));
}

/// `cargo test -p angel-delve plain_timing -- --ignored --nocapture`: how
/// long a room's blockout and an open floor's piece take to make.
#[test]
#[ignore = "prints timings"]
fn plain_timing() {
    let floor = layout::floor(1, Pack::Crypt, &mut Rng::new(7));
    let t = std::time::Instant::now();
    let mut n = 0;
    for room in &floor.rooms {
        if let Some((dressing, _)) = dressing_for(room, Pack::Crypt) {
            let _ = blockout::room_set(room, &dressing);
            n += 1;
        }
    }
    println!("{n} rooms' blockouts: {:.1} ms a room", t.elapsed().as_secs_f64() * 1000.0 / n.max(1) as f64);
    let open = layout::open_floor(2, Pack::Cavern, 1, &mut Rng::new(7));
    let map = open.rooms.iter().find(|r| r.kind == RoomKind::Depths).expect("the map");
    let (dressing, _) = dressing_for(map, Pack::Cavern).unwrap();
    let grid = ((map.cols as i32 + PIECE.0 - 1) / PIECE.0, (map.rows as i32 + PIECE.1 - 1) / PIECE.1);
    let t = std::time::Instant::now();
    let mut shapes = 0;
    for cz in 0..grid.1 {
        for cx in 0..grid.0 {
            shapes += piece_of(map, &dressing, true, (cx, cz)).map_or(0, |p| p.shapes.len());
        }
    }
    println!(
        "open map {}x{}: {} pieces, {shapes} shapes, {:.1} ms a piece",
        map.cols,
        map.rows,
        grid.0 * grid.1,
        t.elapsed().as_secs_f64() * 1000.0 / (grid.0 * grid.1) as f64
    );
}
