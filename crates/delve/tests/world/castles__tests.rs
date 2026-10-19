use super::*;
use crate::rules::{RoomKind, Tile};
use crate::world::map::{CHUNK_H, CHUNK_W, MAP_H, MAP_W, Realm};
use crate::world::{biome, doors, kingdom, roads, signs, wishes};

fn gap(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> i32 {
    let gx = (b.0 - (a.0 + a.2)).max(a.0 - (b.0 + b.2));
    let gy = (b.1 - (a.1 + a.3)).max(a.1 - (b.1 + b.3));
    gx.max(gy)
}

fn tiles((tx, ty, tw, th): (i32, i32, i32, i32)) -> impl Iterator<Item = (i32, i32)> {
    (ty..ty + th).flat_map(move |y| (tx..tx + tw).map(move |x| (x, y)))
}

#[test]
fn every_castle_is_a_great_walled_exterior_in_one_chunk() {
    let all = castles();
    assert_eq!(all.len(), houses::all().len() + 2);
    for c in all {
        let (tx, ty, tw, th) = c.foot;
        assert_eq!(tx / CHUNK_W, (tx + tw - 1) / CHUNK_W, "{} straddles a chunk edge", c.name);
        assert_eq!(ty / CHUNK_H, (ty + th - 1) / CHUNK_H, "{} straddles a chunk edge", c.name);
        assert!(tx > 0 && ty > 0 && tx + tw < MAP_W && ty + th < MAP_H, "{} at the rim", c.name);
        match c.kind {
            CastleKind::House(_) => {
                assert!((14..=22).contains(&tw) && (8..=12).contains(&th), "{} is {tw}x{th}", c.name);
                assert_eq!(biome::at(tx, ty), biome::Biome::March, "{} is not in the March", c.name);
                assert_eq!(biome::at(tx + tw - 1, ty + th - 1), biome::Biome::March);
            }
            _ => assert!(tw >= 14 && th >= 5, "{} is {tw}x{th}", c.name),
        }
        // The gate is in the south face, the stand a tile out before it.
        assert_eq!(c.facing, Facing::South);
        let (gx, gy, gw, gh) = c.gate;
        assert_eq!((gw, gh, gy), (GATE_W, 1, ty + th - 1), "{}'s gate", c.name);
        assert!(gx >= tx && gx + gw <= tx + tw, "{}'s gate is in its face", c.name);
        assert_eq!(c.stand, (gx + gw / 2, ty + th), "{}'s stand is before its gate", c.name);
    }
}

#[test]
fn the_houses_castles_stand_apart_in_two_staggered_rows() {
    let sites: Vec<_> = houses::footprints().collect();
    for (i, &a) in sites.iter().enumerate() {
        for &b in &sites[i + 1..] {
            // A road's width and a few tiles of open ground between any two.
            assert!(gap(a, b) >= 6, "{a:?} crowds {b:?}");
        }
    }
    let rows: std::collections::BTreeSet<i32> = sites.iter().map(|s| s.1 / CHUNK_H).collect();
    assert_eq!(rows.len(), 2, "two rows: {rows:?}");
    let tops: std::collections::BTreeSet<i32> = sites.iter().map(|s| s.1).collect();
    let fronts: std::collections::BTreeSet<i32> = sites.iter().map(|s| s.1 + s.3).collect();
    assert!(tops.len() >= 6 && fronts.len() >= 5, "castles stand in rows: {tops:?} {fronts:?}");
}

#[test]
fn the_keeps_doors_are_where_they_were() {
    let keep = castles().iter().find(|c| c.kind == CastleKind::Keep).unwrap();
    assert_eq!(keep.stand, Place::Keep.stand_world());
    assert_eq!(keep.foot, KEEP);
    let (kx, ky, kw, kh) = Place::Keep.footprint_world();
    assert!(keep.holds(kx, ky) && keep.holds(kx + kw - 1, ky + kh - 1), "the old keep is its gatehouse");
    let inside: Vec<Place> = keep.doors.iter().map(|d| d.0).collect();
    assert_eq!(inside, vec![Place::Rookery, Place::Chapel]);
    for (place, stand) in &keep.doors {
        assert_eq!(*stand, place.stand_world());
        assert_eq!(stand.1, KEEP.1 + KEEP.3, "{place:?}'s door opens in the Keep's face");
    }
    let dragon = castles().iter().find(|c| c.kind == CastleKind::DragonKeep).unwrap();
    assert_eq!(dragon.stand, Place::DragonKeep.stand_world());
    assert_eq!(dragon.foot, DRAGON_KEEP);
    assert!(dragon.doors.is_empty());
    // Every house castle's door is its own: the doors list it at its stand.
    for id in houses::ids() {
        let c = of_house(id);
        assert_eq!(c.kind, CastleKind::House(id));
        assert_eq!(c.stand, houses::stand(id));
        assert!(doors::doors().contains(&(c.stand, doors::Door::Castle(id))), "{} has its door", c.name);
    }
}

#[test]
fn a_castle_is_closed_ground_on_plain_ground_and_nothing_stands_on_it() {
    let geo = kingdom::geography(&[]);
    let realm = Realm::get();
    let raster = roads::raster();
    let mut taken = std::collections::BTreeMap::new();
    for c in castles() {
        for (x, y) in tiles(c.foot) {
            assert!(taken.insert((x, y), c.name.clone()).is_none(), "castles overlap at {x},{y}");
            assert_eq!(realm.at(x, y), c.ground(), "{} stands on {}", c.name, realm.at(x, y) as char);
            assert_eq!(raster[(y * MAP_W + x) as usize], 0, "a road runs through {} at {x},{y}", c.name);
            assert!(kingdom::step_cost(x, y).is_none(), "{} is walked at {x},{y}", c.name);
            assert_ne!(geo.room.tile(x, y), Tile::Floor, "{} is floor at {x},{y}", c.name);
            assert!(!geo.roads[(y * MAP_W + x) as usize], "a lane runs through {} at {x},{y}", c.name);
        }
    }
    let inside = |x: i32, y: i32| at(x, y).is_some();
    for s in &geo.sites {
        assert!(!inside(s.stand.0, s.stand.1), "{}'s door is inside a castle", s.name);
        if let Some(l) = s.lot {
            assert!(castles().iter().all(|c| gap(c.foot, (l.tx, l.ty, l.tw, l.th)) >= 1), "{}'s lot touches a castle", s.name);
        }
    }
    for p in signs::posts() {
        assert!(!inside(p.at.0, p.at.1), "a fingerpost at {:?} stands in a castle", p.at);
    }
    for f in crate::world::finds::finds(41).iter() {
        assert!(castles().iter().all(|c| gap(c.foot, (f.at.0, f.at.1, 1, 1)) >= 6), "a find at {:?} is at a castle", f.at);
    }
    let lots = wishes::lots(&[]);
    assert!(lots.iter().flatten().all(|l| castles().iter().all(|c| gap(c.foot, (l.tx, l.ty, l.tw, l.th)) >= 0)));
}

#[test]
fn every_gate_is_on_its_road_and_walked_to_from_the_keep() {
    let geo = kingdom::geography(&[]);
    for c in castles() {
        assert!(kingdom::is_road(&geo, c.stand), "{}'s gate has no road", c.name);
        assert!(doors::walkable(c.stand), "{}'s gate is not reached from the Keep", c.name);
        // A knight walks the whole of a house castle's south face.
        let (tx, ty, tw, th) = c.foot;
        if let CastleKind::House(_) = c.kind {
            for x in tx..tx + tw {
                assert!(doors::walkable((x, ty + th)), "{}: the ground before its face at {x} is shut", c.name);
            }
        }
    }
}

#[test]
fn the_rooms_behind_the_great_gates_still_open() {
    let geo = kingdom::geography(&[]);
    for name in ["Keep", "Dragon Keep"] {
        let site = geo.sites.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("{name} has a door"));
        assert_eq!(site.room, RoomKind::RealmHall);
        let castle = castles().iter().find(|c| c.stand == site.stand).unwrap();
        assert!(castle.place().is_some());
    }
}

/// A save from before the castles grew: a party in Sol's hall with its
/// return door at the old castle's gate, now inside the new wall, and a
/// party out on the surface where a castle now stands.
#[test]
fn an_old_save_comes_out_at_the_new_gates() {
    use crate::rules::Run;
    use crate::rules::layout::TILE_UNITS;
    let sol = houses::HouseId(0);
    let old_stand = (9, 151);
    assert!(at(old_stand.0, old_stand.1).is_some(), "the old gate is inside the new wall");
    let blank = || crate::rules::Room::kingdom(vec![Tile::Floor; (MAP_W * MAP_H) as usize], MAP_W as usize, MAP_H as usize);
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.join_seat(3, "Bors");
    r.walk_kingdom(Some(kingdom::keep_stand()));
    let geo = kingdom::geography(&r.kingdom.wishes);
    let si = geo.sites.iter().position(|s| s.stand == houses::stand(sol)).unwrap();
    r.enter_kingdom_place(si);
    let i = crate::rules::world::room_of(&r.dungeon, RoomKind::Kingdom).unwrap();
    // The save carries the old ground and the old door.
    r.dungeon.rooms[i] = blank();
    let old = ((old_stand.0 as f32 + 0.5) * TILE_UNITS, (old_stand.1 as f32 + 0.7) * TILE_UNITS);
    if let Some(back) = &mut r.kingdom.return_to {
        (back.x, back.y) = old;
        for spot in back.party.values_mut() {
            *spot = old;
        }
    }
    let mut loaded: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
    assert!(loaded.valid_snapshot());
    loaded.migrate_kingdom();
    loaded.leave_kingdom_place();
    assert_eq!(loaded.room().kind, RoomKind::Kingdom);
    let new = houses::stand(sol);
    for h in loaded.players.values() {
        let t = ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32);
        assert!(loaded.settlement_walkable_for_test(h.x, h.y), "a knight came out at {t:?}, not on open ground");
        assert!((t.0 - new.0).abs() <= 1 && (t.1 - new.1).abs() <= 1, "a knight came out at {t:?}, not at the new gate {new:?}");
    }
    // Out on the surface where a castle now stands.
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.join_seat(3, "Bors");
    r.walk_kingdom(Some(kingdom::keep_stand()));
    r.dungeon.rooms[i] = blank();
    let (cx, cy, cw, ch) = houses::footprint(sol);
    for h in r.players.values_mut() {
        (h.x, h.y) = (((cx + cw / 2) as f32 + 0.5) * TILE_UNITS, ((cy + ch / 2) as f32 + 0.5) * TILE_UNITS);
    }
    let mut loaded: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
    loaded.migrate_kingdom();
    for h in loaded.players.values() {
        let t = ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32);
        assert!(loaded.settlement_walkable_for_test(h.x, h.y), "a knight stands in a wall at {t:?}");
        assert!(at(t.0, t.1).is_none(), "a knight stands in a castle at {t:?}");
    }
}

/// The castles to look at: the whole March and each great castle cut from
/// the kingdom's atlas (the flat map), and a knight at each gate as the
/// game draws the kingdom, as PNG:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve --lib write_castle_shots -- --ignored`.
#[test]
#[ignore = "writes castle review shots"]
fn write_castle_shots() {
    use crate::render::overworld::{Img, TILE, arena, kingdom as geography};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let png = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes()).unwrap().save(dir.join(name)).unwrap();
    };
    let atlas = geography::atlas(&[]);
    let cut = |(x, y, w, h): (i32, i32, i32, i32)| {
        let mut out = Img::new(w, h);
        out.stamp(&atlas, -x, -y);
        out
    };
    png("march.png", &cut((0, 140 * TILE, MAP_W * TILE, 36 * TILE)));
    for (i, c) in castles().iter().enumerate() {
        let (tx, ty, tw, th) = c.foot;
        let name: String = c.name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        png(&format!("castle-{i:02}-{name}.png"), &cut(((tx - 6) * TILE, (ty - 4) * TILE, (tw + 12) * TILE, (th + 9) * TILE)));
        let mut r = crate::rules::Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
        r.walk_kingdom(Some(c.stand));
        for _ in 0..20 {
            r.step(&Default::default());
        }
        png(&format!("gate-{i:02}-{name}.png"), &arena::frame(&r, 768, 448));
    }
}
