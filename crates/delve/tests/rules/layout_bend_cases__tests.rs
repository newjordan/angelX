//! Records `rules/layout.rs` for the Bend port (`delve/rules/world/`).
use super::bend_cases::{Cases, Mix, to};
use super::*;
use serde_json::json;

const PACKS: [Pack; 7] = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp];

/// Floors across depths, packs and party sizes, laid from many seeds.
pub(crate) fn floors(count: usize, seed: u64) -> Vec<Floor> {
    let mut mix = Mix(seed);
    let mut out = Vec::new();
    for i in 0..count {
        let depth = 1 + (i as u32 % DEEPEST);
        let pack = PACKS[(mix.below(7)) as usize];
        let party = 1 + mix.below(4) as usize;
        let mut rng = Rng::new(mix.next());
        out.push(floor_party(depth, pack, party, &mut rng));
    }
    out
}

fn tile_name_of(t: Tile) -> &'static str {
    tile_name(t)
}

#[test]
#[ignore]
fn record_rooms() {
    let mut mix = Mix(7);
    let mut tile = Cases::new("layout.Room.tile");
    let mut set = Cases::new("layout.Room.set");
    let mut open_door = Cases::new("layout.Room.open_door");
    let mut open_stairs = Cases::new("layout.Room.open_stairs");
    let mut way_back = Cases::new("layout.Room.way_back");
    let mut sizes = Cases::new("layout.Room.size");
    let mut privy = Cases::new("layout.Room.privy");
    let mut banner = Cases::new("layout.Room.field_banner");
    let mut altar = Cases::new("layout.Room.altar");
    let mut regrow = Cases::new("layout.Room.regrow");
    let mut rle = Cases::new("layout.tiles_rle");
    let kinds = [Tile::Floor, Tile::Wall, Tile::Block, Tile::Hazard, Tile::Door, Tile::Stairs, Tile::Ledge, Tile::Bog];
    for floor in floors(60, 0xf100) {
        for room in &floor.rooms {
            let r = to(room);
            for _ in 0..24 {
                let col = mix.below(room.cols as u64 + 6) as i32 - 3;
                let row = mix.below(room.rows as u64 + 6) as i32 - 3;
                tile.add(json!({"room": r, "col": col, "row": row}), json!(tile_name_of(room.tile(col, row))));
            }
            for _ in 0..4 {
                let col = mix.below(room.cols as u64) as usize;
                let row = mix.below(room.rows as u64) as usize;
                let t = kinds[mix.below(8) as usize];
                let mut after = room.clone();
                after.set(col, row, t);
                set.add(json!({"room": r, "col": col, "row": row, "tile": tile_name_of(t)}), to(&after));
            }
            let dir = mix.below(4) as usize;
            let mut after = room.clone();
            after.open_door(dir);
            open_door.add(json!({"room": r, "dir": dir}), to(&after));
            let mut after = room.clone();
            after.open_stairs();
            open_stairs.add(json!({"room": r}), to(&after));
            way_back.add(json!({"room": r}), json!(room.way_back()));
            sizes.add(json!({"room": r}), json!({"width": room.width(), "height": room.height(), "great": room.great()}));
            privy.add(json!({"room": r}), json!(room.privy()));
            banner.add(json!({"room": r}), json!(room.field_banner()));
            altar.add(json!({"room": r}), json!(room.altar()));
            let (cols, rows) = [(24usize, 14usize), (34, 20), (42, 24), (48, 28), (36, 20), (40, 24), (32, 20)][mix.below(7) as usize];
            let mut after = room.clone();
            after.regrow(cols, rows);
            regrow.add(json!({"room": r, "cols": cols, "rows": rows}), to(&after));
            // The run string itself, both ways.
            let text = to(room)["tiles"].as_str().unwrap().to_string();
            let back = tiles_from_rle(&text).unwrap();
            rle.add(json!({"text": text}), json!({"tiles": back.iter().map(|&t| tile_name_of(t)).collect::<Vec<_>>().join(","), "again": tiles_rle(&back)}));
        }
    }
    for c in [&tile, &set, &open_door, &open_stairs, &way_back, &sizes, &privy, &banner, &altar, &regrow, &rle] {
        c.save();
    }
}

fn rng_of(v: u64) -> Rng {
    Rng::new(v)
}

fn kinds_all() -> [RoomKind; 9] {
    [
        RoomKind::Start,
        RoomKind::Fight,
        RoomKind::Treasure,
        RoomKind::Stairs,
        RoomKind::Lair,
        RoomKind::Hall,
        RoomKind::Sanctuary,
        RoomKind::Threshold,
        RoomKind::Pit,
    ]
}

#[test]
#[ignore]
fn record_rooms_generation() {
    let mut mix = Mix(0x600d);
    let mut room_c = Cases::new("layout.room");
    let mut quad = Cases::new("layout.great_quadrant");
    let mut conn = Cases::new("layout.connected");
    let mut approach = Cases::new("layout.approach_cells");
    let mut tunnel = Cases::new("layout.tunnel_cells");
    let mut mire = Cases::new("layout.swamp.mire");
    let mut salt = Cases::new("layout.swamp.salt");
    for i in 0..420 {
        let kind = kinds_all()[mix.below(9) as usize];
        let pack = PACKS[mix.below(7) as usize];
        let depth = 1 + mix.below(6) as u32;
        let party = 1 + mix.below(4) as usize;
        let cell = (mix.below(7) as i32 - 1, mix.below(7) as i32 - 1);
        let doors = [mix.below(2) == 1, mix.below(2) == 1, mix.below(2) == 1, mix.below(2) == 1];
        let seed = mix.next();
        let mut rng = rng_of(seed);
        let before = to(&rng);
        let made = room(cell, kind, doors, depth, pack, party, &mut rng);
        room_c.add(
            json!({"cell": cell, "kind": to(&kind), "doors": doors, "depth": depth, "pack": to(&pack), "party": party, "rng": before}),
            json!({"room": to(&made), "rng": to(&rng)}),
        );
        if i % 3 == 0 {
            let (w, h) = [(11usize, 6usize), (16, 9), (19, 11), (23, 13), (15, 9), (20, 11), (17, 9)][mix.below(7) as usize];
            let k = if mix.below(2) == 0 { RoomKind::Lair } else { RoomKind::Fight };
            let mut rng = rng_of(seed ^ 77);
            let before = to(&rng);
            let q = great_quadrant(w, h, k, &mut rng);
            let rows: Vec<String> = q.iter().map(|r| String::from_utf8(r.clone()).unwrap()).collect();
            quad.add(json!({"w": w, "h": h, "kind": to(&k), "rng": before}), json!({"rows": rows, "rng": to(&rng)}));
        }
        conn.add(json!({"room": to(&made)}), json!(connected(&made)));
    }
    for i in 0..160 {
        let requested = mix.below(30) as usize;
        let mut rng = rng_of(mix.next());
        let before = to(&rng);
        let cells = approach_cells(requested, &mut rng);
        approach.add(json!({"requested": requested, "rng": before}), json!({"cells": cells, "rng": to(&rng)}));
        let depth = 3 + (i % 9) as u32;
        let seed = mix.next();
        let width = [5, 5, 5, 3, 7, 1, 16, 0][mix.below(8) as usize];
        let req = mix.below(40) as usize;
        let cells = crate::world::routing::tunnel_cells(crate::world::routing::TunnelVariant::for_depth(depth), seed, width, req);
        tunnel.add(json!({"depth": depth, "seed": seed, "width": width, "requested": req}), json!(cells));
    }
    // The swamp's mire over quadrants.
    for _ in 0..120 {
        let depth = 1 + mix.below(4) as u32;
        let cell = (mix.below(7) as i32 - 1, mix.below(7) as i32 - 1);
        let s = crate::rules::swamp::salt(cell, depth);
        salt.add(json!({"cell": cell, "depth": depth}), json!(s));
        let mut rng = rng_of(mix.next());
        let q = great_quadrant(11, 6, RoomKind::Fight, &mut rng);
        let mut after = q.clone();
        crate::rules::swamp::mire(&mut after, depth, s);
        let rows = |q: &Vec<Vec<u8>>| q.iter().map(|r| String::from_utf8(r.clone()).unwrap()).collect::<Vec<_>>();
        mire.add(json!({"rows": rows(&q), "depth": depth, "salt": s}), json!(rows(&after)));
    }
    for c in [&room_c, &quad, &conn, &approach, &tunnel, &mire, &salt] {
        c.save();
    }
}

#[test]
#[ignore]
fn record_floors() {
    let mut mix = Mix(0xf100d);
    let mut party_c = Cases::new("layout.floor_party");
    let mut for_c = Cases::new("layout.floor_for_party");
    let mut steps = Cases::new("layout.Floor.steps");
    let mut ramps = Cases::new("layout.Floor.ramps");
    let mut doors = Cases::new("layout.Floor.stairs_doors");
    let mut guardian = Cases::new("layout.Floor.guardian");
    let mut neighbour = Cases::new("layout.Floor.neighbour");
    let mut chief = Cases::new("layout.Floor.chief_rank");
    let mut force = Cases::new("layout.force_secret");
    let mut identity = Cases::new("layout.identity_floor");
    let mut ident_room = Cases::new("layout.identity_room");
    let mut under = Cases::new("layout.undercroft");
    let mut tavern = Cases::new("layout.dig_tavern");
    for i in 0..360 {
        let depth = 1 + (i as u32 % DEEPEST);
        let pack = PACKS[mix.below(7) as usize];
        let party = 1 + mix.below(4) as usize;
        let mut rng = rng_of(mix.next());
        let before = to(&rng);
        let floor = floor_party(depth, pack, party, &mut rng);
        party_c.add(
            json!({"depth": depth, "pack": to(&pack), "party": party, "rng": before}),
            json!({"floor": to(&floor), "rng": to(&rng)}),
        );
        let f = to(&floor);
        steps.add(json!({"floor": f}), json!(floor.steps().iter().map(|&d| if d == usize::MAX { u32::MAX as u64 } else { d as u64 }).collect::<Vec<_>>()));
        ramps.add(json!({"floor": f}), json!(floor.ramps()));
        doors.add(json!({"floor": f}), json!(floor.stairs_doors()));
        guardian.add(json!({"floor": f}), json!(floor.guardian()));
        for room in 0..floor.rooms.len() {
            let dir = mix.below(4) as usize;
            neighbour.add(json!({"floor": f, "from": room, "dir": dir}), json!(floor.neighbour(room, dir)));
        }
        let steps_v = floor.steps();
        for room in 0..floor.rooms.len() {
            let (a, b, c, d) = floor.chief_rank(room, &steps_v);
            let d = if d >= 1 << 31 { u32::MAX as u64 - (usize::MAX as u64 - d as u64) } else { d as u64 };
            chief.add(json!({"floor": f, "room": room}), json!({"guard": a, "show": b, "early": c, "mid": d}));
        }
        let mut forced = floor.clone();
        force_secret(&mut forced);
        force.add(json!({"floor": f}), to(&forced));
        // The wheel's modes.
        let mode: crate::rules::fortune::Mode = serde_json::from_value(json!(["long_way_down", "hold_the_stair", "gauntlet", "hollow_walls"][mix.below(4) as usize])).unwrap();
        let mut rng = rng_of(mix.next());
        let before = to(&rng);
        let floor = floor_for_party(mode, depth, pack, party, &mut rng);
        for_c.add(
            json!({"mode": to(&mode), "depth": depth, "pack": to(&pack), "party": party, "rng": before}),
            json!({"floor": to(&floor), "rng": to(&rng)}),
        );
        let mut rng = rng_of(mix.next());
        let before = to(&rng);
        let floor = identity_floor(pack, party, &mut rng);
        identity.add(json!({"pack": to(&pack), "party": party, "rng": before}), json!({"floor": to(&floor), "rng": to(&rng)}));
        let mut rng = rng_of(mix.next());
        let before = to(&rng);
        let floor = identity_room(pack, party, &mut rng);
        ident_room.add(json!({"pack": to(&pack), "party": party, "rng": before}), json!({"floor": to(&floor), "rng": to(&rng)}));
    }
    for pack in PACKS {
        let floor = undercroft(pack);
        under.add(json!({"pack": to(&pack)}), to(&floor));
        let mut dug = floor.clone();
        let ok = dig_tavern(&mut dug);
        tavern.add(json!({"floor": to(&floor)}), json!({"floor": to(&dug), "ok": ok}));
        let mut again = dug.clone();
        let ok2 = dig_tavern(&mut again);
        tavern.add(json!({"floor": to(&dug)}), json!({"floor": to(&again), "ok": ok2}));
    }
    for c in [&party_c, &for_c, &steps, &ramps, &doors, &guardian, &neighbour, &chief, &force, &identity, &ident_room, &under, &tavern] {
        c.save();
    }
}

/// The world's authored tables (rooms, trails, entrances) as JSON, for
/// `delve/rules/world/_gen/tables.py` to write as Bend data.
#[test]
#[ignore]
fn dump_world_tables() {
    use crate::rules::world::{Dest, ENTRANCES, ROOMS, TRAILS};
    let rooms: Vec<_> = ROOMS.iter().map(|(k, c)| json!({"kind": to(k), "cell": c})).collect();
    let trails: Vec<_> = TRAILS.iter().map(|(a, b)| json!([to(a), to(b)])).collect();
    let entrances: Vec<_> = ENTRANCES
        .iter()
        .map(|e| {
            let to_v = match e.to {
                Dest::Room(k, (x, y)) => json!({"room": to(&k), "at": [x, y]}),
                Dest::Delve(p) => json!({"delve": to(&p)}),
                Dest::Cave => json!({"cave": true}),
            };
            json!({"id": e.id, "room": to(&e.room), "at": e.at, "to": to_v, "label": e.label, "needs": e.needs})
        })
        .collect();
    let dir = super::bend_cases::dir().join("_tables");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("world.json"),
        serde_json::to_string(&json!({"rooms": rooms, "trails": trails, "entrances": entrances})).unwrap(),
    )
    .unwrap();
}
