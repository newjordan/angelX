//! Lit rooms stand up with no plate: every item's room and the seeded
//! floors' rooms read as sets with a ground and their lamps, the ground at
//! the floor line wherever a knight walks and sunk where the floor is a
//! hazard; the same room is the same set on a window and on a page; a fight
//! room's fires burn while it is fought; a terminal sees the room lit from
//! above. (Their masses agree with the game: `blockout__tests`,
//! `lit_rooms_agree_with_the_game`.)

use super::*;
use crate::rules::{Rng, RoomKind, layout};

fn crypt_fight() -> Room {
    let floor = layout::floor_party(1, Pack::Crypt, 1, &mut Rng::new(3));
    floor.rooms.iter().find(|r| r.kind == RoomKind::Fight).expect("a fight room").clone()
}

/// The ground's height at the middle of tile `(c, r)`.
fn height(set: &Set, (c, r): (i32, i32)) -> f32 {
    set.ground.as_ref().expect("a ground").height_at((c as f32 + 0.5) * TILE, (r as f32 + 0.5) * TILE)
}

#[test]
fn every_items_room_and_every_seeded_room_stands_up_lit_with_no_plate() {
    let mut rooms = 0;
    for item in super::super::pilot::every() {
        let room = item.room();
        let pack = match item.dressing.setting {
            Setting::Dungeon(pack) => pack,
            _ => Pack::Crypt,
        };
        let Some(lit) = lit_for(&room, pack) else {
            continue;
        };
        let piece = lit.piece((0, 0)).unwrap_or_else(|| panic!("{}: its lit set reads", item.id()));
        assert!(piece.set.ground.is_some(), "{}: a ground", item.id());
        assert!(!piece.text.contains("\nplate "), "{}: no plate", item.id());
        assert!(piece.set.shapes.len() > 8, "{}: what stands", item.id());
        rooms += 1;
    }
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp] {
        let floor = layout::floor_party(2, pack, 2, &mut Rng::new(5));
        for room in &floor.rooms {
            if matches!(room.kind, RoomKind::Ledge | RoomKind::Wanted) {
                assert!(lit_for(room, pack).is_none(), "a side-on hall is never stood up");
                continue;
            }
            let lit = lit_for(room, pack).unwrap_or_else(|| panic!("{pack:?} {:?}: lit", room.kind));
            assert_eq!(lit.tone, Tone::Delve(pack));
            for piece in lit.every_piece() {
                // The ground: at the floor line wherever a knight walks, sunk
                // under every hazard.
                for r in piece.tiles.1..piece.tiles.1 + piece.tiles.3 {
                    for c in piece.tiles.0..piece.tiles.0 + piece.tiles.2 {
                        let h = height(&piece.set, (c, r));
                        if blockout::walkable(room.tile(c, r)) {
                            assert_eq!(h, 0.0, "{pack:?} {:?}: walkable {c},{r} at the floor line", room.kind);
                        }
                        if room.tile(c, r) == Tile::Hazard {
                            assert!(h < -2.0, "{pack:?} {:?}: hazard {c},{r} sunk ({h})", room.kind);
                        }
                    }
                }
            }
            rooms += 1;
        }
    }
    assert!(rooms > 150, "{rooms} rooms");
}

#[test]
fn what_burns_is_drawn_by_the_frame_and_lit_by_lamps() {
    let room = crypt_fight();
    let lit = lit_for(&room, Pack::Crypt).expect("lit");
    let piece = lit.piece((0, 0)).expect("reads");
    assert!(!piece.flames.is_empty(), "torches by the doors, braziers in the corners");
    assert!(piece.set.shapes.iter().all(|s| s.kind != Kind::Fire), "no fire in the set itself: the frame draws it");
    assert!(!piece.set.lamps.is_empty() && piece.set.lamps.len() <= piece.flames.len(), "a lamp to a flame or a cluster of them");
    // Fought, the flames are accents, full; won, low embers, not lights.
    let fire = Tone::Delve(Pack::Crypt).flame();
    let fought = flame_nodes(&[Arc::clone(&piece)], Activity::Fought, 0.0, fire);
    let cleared = flame_nodes(&[Arc::clone(&piece)], Activity::Cleared, 0.0, fire);
    assert!(fought.iter().all(|n| n.material.accent));
    assert!(cleared.iter().all(|n| !n.material.accent));
    // Lava burns whatever the room does: its lamps are steady.
    let lava_room = super::super::pilot::item("hellforge-lava-ford").expect("an item").room();
    let lava = lit_for(&lava_room, Pack::Hellforge).expect("lit").piece((0, 0)).expect("reads");
    assert!(!lava.lava.is_empty() && lava.steady.iter().any(|&s| s), "lava and its steady lamps");
}

#[test]
fn a_lit_room_is_the_same_set_on_a_window_and_on_a_page() {
    use super::super::room::{Moment, RoomWorld};
    let room = crypt_fight();
    let lit = lit_for(&room, Pack::Crypt).expect("lit");
    let world = RoomWorld::lit_world(Arc::clone(&lit), (room.cols as i32 * 16, room.rows as i32 * 16));
    let moment = Moment { activity: Activity::Fought, chest: Some([100.0, 13.0, 100.0]), time: 1.5, ..Moment::default() };
    let world = RoomWorld { moment, ..world };
    let spark = Lamp { at: [140.0, 8.0, 60.0], reach: 30.0, strength: 0.8 };
    let staged = world.lit_staged((40.0, 20.0), ([0.0, 0.0], 1.0), (384.0, 224.0), &[spark], &|_, _| true).expect("lit");
    assert_eq!(staged.look.lamps[0].at, [100.0, 8.0, 40.0], "an ember in flight lights the room first, scrolled with it");
    assert!(staged.look.lamps.len() <= angel_stage::look::LAMP_MAX);
    assert!(staged.look.lights() && staged.look.validate().is_ok(), "{:?}", staged.look.validate());
    assert_eq!(staged.look.metre, super::super::rooms_lit_look::METRE);
    let reveal = staged.look.reveal.expect("the knight's chest kept in sight");
    assert_eq!(reveal.at, [60.0, 13.0, 80.0], "on the stage, scrolled with the room");
    assert_eq!(staged.meshes.len(), 1, "its ground");
    // A page makes the same floor and blocks from the set's text.
    let (key, text) = staged.sets[0].clone();
    let (scale, offset) = world.placing((40.0, 20.0), ([0.0, 0.0], 1.0));
    let mut sets = angel_stage::set::Sets::default();
    let mut far = angel_stage::scene::World {
        set: Some(angel_stage::scene::SetRef { key, text: Some(text), plate: 0, png: None, scale, offset, patches: angel_stage::set::Patches::NONE }),
        look: Some(staged.look.clone()),
        ..Default::default()
    };
    assert!(sets.fill(&mut far));
    assert_eq!((far.floor, far.blocks), (staged.floor, staged.blocks));
    assert_eq!(far.meshes.len(), 1);
    // And a page sent the text of its host's room keeps it by its key.
    assert!(receive(lit.key, &lit.text().expect("a one-screen room's text")));
    assert!(!receive(1, "not a set"));
}

#[test]
fn a_terminal_sees_a_lit_room_from_above_on_its_ramp() {
    let room = crypt_fight();
    let lit = lit_for(&room, Pack::Crypt).expect("lit");
    let size = (room.cols as i32 * 16, room.rows as i32 * 16);
    let img = lit.picture(size);
    assert_eq!((img.w, img.h), size);
    let ramp = Tone::Delve(Pack::Crypt).ramp();
    let mut seen = std::collections::BTreeSet::new();
    for y in (0..img.h).step_by(3) {
        for x in (0..img.w).step_by(3) {
            seen.insert(img.get(x, y).expect("drawn"));
        }
    }
    let own = seen.iter().filter(|c| ramp.contains(c)).count();
    assert!(own >= 4, "its ramp's entries: {own} of {}", seen.len());
    assert_eq!(lit.picture(size), img, "kept");
}

#[test]
fn rooms_take_their_tone_from_their_kind_of_place() {
    let tone = |id: &str| {
        let item = super::super::pilot::item(id).expect("an item");
        let pack = match item.dressing.setting {
            Setting::Dungeon(pack) => pack,
            _ => Pack::Crypt,
        };
        lit_for(&item.room(), pack).expect("lit").tone
    };
    assert_eq!(tone("village"), Tone::Open(super::super::rooms_lit_look::Ground::Meadow));
    assert_eq!(tone("kings-hall"), Tone::Keep);
    assert_eq!(tone("tavern"), Tone::Tavern);
    assert_eq!(tone("smithy"), Tone::Forge);
    assert_eq!(tone("wizard-study"), Tone::Tower);
    assert_eq!(tone("college-hall"), Tone::College);
    assert_eq!(tone("crypt-bone-chapel"), Tone::Delve(Pack::Crypt));
    assert_eq!(tone("archive-map-table"), Tone::Delve(Pack::Archive));
}

#[test]
fn a_delves_floor_is_lit_along_its_doors_and_dark_toward_its_rim() {
    let room = crypt_fight();
    let (dressing, _) = plain::dressing_for(&room, Pack::Crypt).expect("a dressing");
    let whole = (0, 0, room.cols as i32, room.rows as i32);
    let doors = floor_doors(&room, &dressing, whole).expect("a delve room's floor is lit from its doors");
    assert!(!doors.is_empty(), "a fight room has a door");
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let (cx, cz) = (w / 2.0, h / 2.0);
    let light = |x: f32, z: f32| floor_light(&room, &doors, x, z);
    // Half way along a door's lane the floor is brighter than the floor beside it.
    let (dx, dz) = doors[0];
    let (mx, mz) = ((dx + cx) / 2.0, (dz + cz) / 2.0);
    assert!(light(mx, mz) > 1.15, "a lane's middle is lit ({})", light(mx, mz));
    // The lane ends: across it, further than a door two tiles wide and a lane's
    // width can reach, the floor is as the rim's dark alone leaves it.
    let (ex, ez) = (cx - dx, cz - dz);
    let len = ex.hypot(ez);
    let across = TILE * (LANE_WIDTH + 3.5);
    let aside = [-1.0, 1.0].map(|s| light(mx - s * ez / len * across, mz + s * ex / len * across));
    assert!(aside[0].min(aside[1]) < light(mx, mz) - 0.2, "its edge falls away ({aside:?})");
    // The room's corner furthest from every lane is the darkest the floor gets.
    let corner = [(TILE, TILE), (w - TILE, TILE), (TILE, h - TILE), (w - TILE, h - TILE)]
        .into_iter()
        .min_by(|a, b| light(a.0, a.1).total_cmp(&light(b.0, b.1)))
        .expect("a corner");
    assert!(light(corner.0, corner.1) < 0.7, "the rim is dark ({})", light(corner.0, corner.1));
    assert!(light(corner.0, corner.1) > 1.0 - RIM_DARK - 0.001, "no darker than the rim's most");
    // With no door there is only the rim: the middle is as it was.
    assert_eq!(floor_light(&room, &[], cx, cz), 1.0);
    assert!(floor_light(&room, &[], corner.0, corner.1) < 0.7);
    // Nothing is ever dark or bright without bound.
    for j in 0..=40 {
        for i in 0..=40 {
            let k = light(w * i as f32 / 40.0, h * j as f32 / 40.0);
            assert!(k.is_finite() && (1.0 - RIM_DARK - 0.001..=1.0 + LANE_GAIN + 0.001).contains(&k), "{k}");
        }
    }
}

#[test]
fn only_a_whole_delve_room_has_its_floor_lit_from_its_doors() {
    let room = crypt_fight();
    let (dressing, _) = plain::dressing_for(&room, Pack::Crypt).expect("a dressing");
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    assert!(floor_doors(&room, &dressing, (0, 0, cols, rows)).is_some());
    // A piece of an open floor's map has no middle of its own.
    assert!(floor_doors(&room, &dressing, (0, 0, cols - 1, rows)).is_none());
    assert!(floor_doors(&room, &dressing, (2, 0, cols, rows)).is_none());
    // Nor does a place of the kingdom, indoors or out, have a delve's dark.
    let place = super::super::pilot::every().into_iter().find(|i| !matches!(i.dressing.setting, Setting::Dungeon(_))).expect("a place");
    let room = place.room();
    let whole = (0, 0, room.cols as i32, room.rows as i32);
    assert!(floor_doors(&room, &place.dressing, whole).is_none(), "{}", place.id());
}

/// An open floor's map and its dressing.
fn open_map() -> (Room, Dressing) {
    let floor = layout::open_floor(2, Pack::Crypt, 2, &mut Rng::new(11));
    let map = floor.rooms.iter().find(|r| r.kind == RoomKind::Depths).expect("an open floor's map").clone();
    let (dressing, _) = plain::dressing_for(&map, Pack::Crypt).expect("a map is drawn so");
    (map, dressing)
}

#[test]
fn an_open_floors_passages_open_on_their_chambers_through_mouths() {
    let (map, _) = open_map();
    let all = mouths(&map, (0, 0, map.cols as i32, map.rows as i32));
    assert!(all.len() >= 12, "a mouth at each end of a passage and a doorway or two: {}", all.len());
    for m in &all {
        for (c, r) in m.jambs {
            assert_eq!(map.tile(c, r), Tile::Wall, "{m:?}: walled either side");
        }
        // A step into the chamber from the gap's middle is floor, the gap
        // itself a few tiles wide.
        let tile = |k: f32| (((m.at[0] + m.into[0] as f32 * k) / TILE).floor() as i32, ((m.at[1] + m.into[1] as f32 * k) / TILE).floor() as i32);
        let (c, r) = tile(TILE / 2.0);
        assert!(blockout::walkable(map.tile(c, r)), "{m:?}: the chamber's floor lies before it");
        assert!((2.0 * TILE..=8.0 * TILE).contains(&(m.half * 2.0)), "{m:?}: a gap of a few tiles");
    }
}

#[test]
fn an_open_floor_is_lit_at_its_mouths_and_dark_toward_its_walls() {
    let (map, d) = open_map();
    let whole = (0, 0, map.cols as i32, map.rows as i32);
    let light = open_light(&map, &d, whole).expect("a delve's open map is lit");
    let plain = OpenLight { map: &map, mouths: Vec::new() };
    // In a lane the floor is brighter than the same floor without the mouth,
    // past a lane's length it is not.
    let one = OpenLight { map: &map, mouths: vec![light.mouths[0]] };
    let m = light.mouths[0];
    let at = |k: f32| (m.at[0] + m.into[0] as f32 * k, m.at[1] + m.into[1] as f32 * k);
    let (x, z) = at(2.0 * TILE);
    assert!(one.at(x, z) > plain.at(x, z) * 1.3, "a lane's middle is lit ({} against {})", one.at(x, z), plain.at(x, z));
    let (x, z) = at((MOUTH_REACH + 1.0) * TILE);
    assert_eq!(one.at(x, z), plain.at(x, z), "past its length, a mouth lights nothing");
    // Beside a wall the floor is dark; far from every wall it is as it was.
    let beside = (0..map.rows as i32)
        .flat_map(|r| (0..map.cols as i32).map(move |c| (c, r)))
        .find(|&(c, r)| blockout::walkable(map.tile(c, r)) && map.tile(c + 1, r) == Tile::Wall)
        .expect("a floor tile by a wall");
    let k = plain.at((beside.0 as f32 + 0.9) * TILE, (beside.1 as f32 + 0.5) * TILE);
    assert!(k < 1.0 - WALL_DARK * 0.6, "the rim is dark ({k})");
    assert!(k >= 1.0 - WALL_DARK - 0.001, "and no darker than its most ({k})");
    let middle = (0..map.rows as i32)
        .flat_map(|r| (0..map.cols as i32).map(move |c| (c, r)))
        .any(|(c, r)| plain.at((c as f32 + 0.5) * TILE, (r as f32 + 0.5) * TILE) == 1.0);
    assert!(middle, "a chamber's middle is far enough from its walls to be lit as it was");
    // Nothing is dark or bright without bound.
    for j in 0..60 {
        for i in 0..80 {
            let k = light.at(i as f32 * 2.1 * TILE, j as f32 * 1.4 * TILE);
            assert!(k.is_finite() && (1.0 - WALL_DARK - 0.001..=1.0 + MOUTH_GAIN + 0.001).contains(&k), "{k}");
        }
    }
}

#[test]
fn an_open_floor_burns_braziers_on_what_a_knight_cannot_walk() {
    use super::super::rooms_lit_mass;
    let (map, dressing) = open_map();
    let lit = lit_for(&map, Pack::Crypt).expect("lit");
    let pieces = lit.every_piece();
    let (mut flames, mut on_walls) = (0, 0);
    for cz in 0..lit.grid.1 {
        for cx in 0..lit.grid.0 {
            let (tiles, crop) = plain::piece_rects(&map, true, (cx, cz));
            let cut = plain::crop_room(&map, crop);
            let mut groups = blockout::blockout(&cut, &dressing);
            rooms_lit_mass::monolith_in(&mut groups, &cut, &dressing, &map, (crop.0, crop.1));
            for f in open_braziers(&map, &dressing, tiles, crop, &groups).iter().filter(|s| s.kind == Kind::Fire) {
                let (c, r) = (crop.0 + (f.at[0] / TILE).floor() as i32, crop.1 + (f.at[2] / TILE).floor() as i32);
                assert!((tiles.0..tiles.0 + tiles.2).contains(&c) && (tiles.1..tiles.1 + tiles.3).contains(&r), "{c},{r}: in its own piece");
                match map.tile(c, r) {
                    // On a wall that looks on a chamber from the north, west
                    // or east, never on the near wall; or on what stands.
                    Tile::Wall => {
                        assert!([(0, 1), (1, 0), (-1, 0)].iter().any(|&(dc, dr)| blockout::walkable(map.tile(c + dc, r + dr))), "{c},{r}: a wall that looks on floor");
                        on_walls += 1;
                    }
                    Tile::Block => {}
                    other => panic!("{c},{r}: a brazier stands over {other:?}, ground a knight walks"),
                }
                flames += 1;
            }
        }
    }
    assert!(flames >= 20 && on_walls >= 10, "braziers along its walls and on what stands: {flames}, {on_walls} on walls");
    // Each is a flame of its piece's set with a lamp over it.
    assert!(pieces.iter().map(|p| p.flames.len()).sum::<usize>() >= flames);
    for piece in &pieces {
        assert!(piece.flames.is_empty() || !piece.set.lamps.is_empty(), "a piece that burns has lamps");
    }
    // Only an open delve floor: a room's own braziers are the generator's.
    let room = crypt_fight();
    let (dressing, _) = plain::dressing_for(&room, Pack::Crypt).expect("a dressing");
    let whole = (0, 0, room.cols as i32, room.rows as i32);
    assert!(open_light(&room, &dressing, whole).is_none());
    assert!(open_braziers(&room, &dressing, whole, whole, &[]).is_empty());
}

#[test]
fn the_west_wall_of_a_tall_room_is_open_to_the_low_sun() {
    use super::super::rooms_lit_mass;
    let floor = layout::floor_party(1, Pack::Crypt, 2, &mut Rng::new(3));
    let room = floor.rooms.iter().find(|r| r.kind == RoomKind::Fight).expect("a fight room").clone();
    let lit = lit_for(&room, Pack::Crypt).expect("lit");
    let set = &lit.piece((0, 0)).expect("reads").set;
    let (w, _) = lit.stage();
    // Whether a line along x through the wall, at height `y` and `z` across,
    // meets any of the set.
    let clear = |z: f32, y: f32, (x0, x1): (f32, f32)| {
        !set.shapes.iter().any(|s| {
            let (hx, hz) = (s.size[0] / 2.0, s.size[2] / 2.0);
            s.at[0] - hx < x1 && s.at[0] + hx > x0 && s.at[2] - hz < z && s.at[2] + hz > z && s.at[1] <= y && s.at[1] + s.size[1] >= y
        })
    };
    // An opening is a sill and a lintel with nothing between: the way a
    // ray of the low sun takes through the wall's thickness is clear.
    let (sill, lintel) = (rooms_lit_mass::SHAFT_SILL, rooms_lit_mass::SHAFT_TOP);
    let frames = |set: &Set, east: bool| -> Vec<(f32, f32)> {
        set.shapes
            .iter()
            .filter(|s| (s.at[1] + s.size[1] - sill).abs() < 0.01 && (s.at[0] > w / 2.0) == east && (s.size[0] - rooms_lit_mass::SHAFT_DEPTH).abs() < 0.01)
            .filter(|sill| set.shapes.iter().any(|l| (l.at[1] - lintel).abs() < 0.01 && (l.at[0] - sill.at[0]).abs() < 0.01 && (l.at[2] - sill.at[2]).abs() < 0.01 && (l.size[2] - sill.size[2]).abs() < 0.01))
            .map(|s| (s.at[2] - s.size[2] / 2.0, s.at[2] + s.size[2] / 2.0))
            .collect()
    };
    let west = frames(set, false);
    assert!(west.len() >= 2, "a window to a run of the west wall: {west:?}");
    for (a, b) in west {
        assert!(b - a >= 2.0 * TILE, "wide enough to light a floor: {a}..{b}");
        for z in (a as i32 + 1..b as i32).step_by(3) {
            assert!(clear(z as f32, (sill + lintel) / 2.0, (-26.0, 10.0)), "z {z} is open through the wall's thickness");
        }
        assert!(!clear(a - 1.0, (sill + lintel) / 2.0, (-26.0, 10.0)), "and the wall stands beside it");
    }
    assert!(frames(set, true).is_empty(), "the east wall is not opened: the sun is in the west");
    // Only a delve has that sun: the kingdom's halls stand as they were.
    let hall = super::super::pilot::item("kings-hall").expect("an item").room();
    let hall = lit_for(&hall, Pack::Crypt).expect("lit").piece((0, 0)).expect("reads");
    assert!(frames(&hall.set, false).is_empty() && frames(&hall.set, true).is_empty());
}

/// `cargo test -p angel-delve --release lit_room_timings -- --ignored --nocapture`:
/// how long a lit room's set and an open floor's piece take to make.
#[test]
#[ignore = "prints timings"]
fn lit_room_timings() {
    let room = crypt_fight();
    let (dressing, _) = plain::dressing_for(&room, Pack::Crypt).unwrap();
    let t = std::time::Instant::now();
    let text = room_text(&room, &dressing, true, None);
    println!("a {}x{} room: {:?}, {} bytes", room.cols, room.rows, t.elapsed(), text.len());
    let floor = layout::open_floor(2, Pack::Crypt, 2, &mut Rng::new(11));
    let map = floor.rooms.iter().find(|r| r.kind == RoomKind::Depths).unwrap();
    let (dressing, _) = plain::dressing_for(map, Pack::Crypt).unwrap();
    let t = std::time::Instant::now();
    let rects = plain::piece_rects(map, true, (1, 1));
    let text = room_text(map, &dressing, false, Some(rects));
    println!("an open floor's piece: {:?}, {} bytes", t.elapsed(), text.len());
}
