//! Bomrekedan's levels as rooms of the world: each level read at its size,
//! the stairs as `kingdom.json` links them, the kingdom's door in and the
//! depot's plate out, and a walk down every stair and back.
use super::*;
use crate::rules::home::DESCEND_HOLD;
use crate::rules::world::{self, ENTRANCES, entrance_at, entrance_centre, room_of};
use crate::rules::{Input, Run, TILE_UNITS};
use std::collections::BTreeMap;

fn json_levels() -> &'static [serde_json::Value] {
    kingdom()["levels"].as_array().expect("kingdom.json levels")
}

fn rect(v: &serde_json::Value) -> (i32, i32, i32, i32) {
    let n = |k: &str| v[k].as_i64().unwrap_or_else(|| panic!("{k} of {v}")) as i32;
    (n("x"), n("y"), n("w"), n("h"))
}

fn tile_of(v: &serde_json::Value) -> (f32, f32) {
    (v["x"].as_i64().unwrap() as f32 + 0.5, v["y"].as_i64().unwrap() as f32 + 0.5)
}

fn kind_of(slug: &str) -> RoomKind {
    LEVELS.iter().find(|d| d.slug == slug).unwrap_or_else(|| panic!("{slug} has no RoomKind")).kind
}

fn name_of(slug: &str) -> String {
    json_levels().iter().find(|l| l["slug"] == slug).unwrap()["name"].as_str().unwrap().to_uppercase()
}

/// `DOORS` as `kingdom.json` has them: the depot's plate out, then each
/// level's links as the file lists them.
fn doors_from_json() -> Vec<(String, RoomKind, (i32, i32, i32, i32), RoomKind, (f32, f32), String)> {
    let top = &json_levels()[0];
    let mut out = vec![(
        "bomrekedan-out".to_string(),
        kind_of(top["slug"].as_str().unwrap()),
        rect(&top["entrance"]["at"]),
        RoomKind::Gate,
        world::GATE_FRONT,
        "OUT TO THE KINGDOM".to_string(),
    )];
    for level in json_levels() {
        let slug = level["slug"].as_str().unwrap();
        for link in level["links"].as_array().unwrap() {
            let to = link["to"].as_str().unwrap();
            let dir = link["direction"].as_str().unwrap();
            // Where one lands: the far level's own link back, its `arrive`.
            let back = json_levels().iter().find(|l| l["slug"] == to).unwrap()["links"]
                .as_array()
                .unwrap()
                .iter()
                .find(|l| l["to"] == slug)
                .unwrap_or_else(|| panic!("{to} has no link back to {slug}"));
            out.push((
                format!("bom-{}-{dir}", slug.trim_start_matches("the-")),
                kind_of(slug),
                rect(&link["at"]),
                kind_of(to),
                tile_of(&back["arrive"]),
                format!("{} TO {}", dir.to_uppercase(), name_of(to)),
            ));
        }
    }
    out
}

fn door(id: &str) -> &'static Entrance {
    DOORS.iter().find(|e| e.id == id).unwrap_or_else(|| panic!("no door {id}"))
}

#[test]
fn the_levels_are_the_ones_the_import_kept_top_down() {
    let slugs: Vec<&str> = json_levels().iter().map(|l| l["slug"].as_str().unwrap()).collect();
    assert_eq!(LEVELS.iter().map(|d| d.slug).collect::<Vec<_>>(), slugs, "one RoomKind a kept level, in the file's order");
    assert_eq!(kingdom()["surface_level"], LEVELS[0].slug);
    let on_disk: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/bomrekedan/levels"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().trim_end_matches(".txt").to_string())
        .collect();
    for slug in &on_disk {
        assert!(slugs.contains(&slug.as_str()), "{slug}.txt is no level of kingdom.json");
    }
    for (i, def) in LEVELS.iter().enumerate() {
        assert_eq!(index(def.kind), Some(i));
        assert!(is_level(def.kind) && def.kind.in_world());
        assert_eq!(json_levels()[i]["file"], format!("levels/{}.txt", def.slug));
        assert!(world::ROOMS.contains(&(def.kind, def.cell)));
        assert_eq!(CELLS[i], (def.kind, def.cell));
    }
    assert!(!is_level(RoomKind::Gate) && !is_level(RoomKind::MusterTent));
    // Deeper down the file, deeper in the fort.
    assert!(levels().windows(2).all(|w| w[0].z > w[1].z));
}

/// The levels ride in every save and every guest's first snapshot of the
/// home floor (`Floor::rooms`, their ground one run-length string a room):
/// ten levels of the fort stay a few kilobytes each.
#[test]
fn the_levels_cost_a_save_and_a_snapshot_little() {
    let r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    let levels: Vec<_> = r.dungeon.rooms.iter().filter(|room| is_level(room.kind)).collect();
    assert_eq!(levels.len(), LEVELS.len());
    let bytes: usize = levels.iter().map(|room| serde_json::to_vec(room).unwrap().len()).sum();
    let run = serde_json::to_vec(&r).unwrap().len();
    eprintln!("the levels: {bytes} bytes of the run's {run}");
    assert!(bytes < 24 * 1024, "the levels are {bytes} bytes");
}

#[test]
fn every_level_reads_at_its_declared_size_and_is_a_valid_room() {
    for (def, level) in LEVELS.iter().zip(levels()) {
        let size = def.data.lines().nth(3).unwrap().trim_end();
        assert_eq!(size, format!("SIZE {} {}", level.size.0, level.size.1), "{}", def.slug);
        assert_eq!(json_levels()[index(def.kind).unwrap()]["name"], level.name);
        let (w, h) = level.size;
        assert_eq!((level.tiles.len(), level.dress.len(), level.rock.len()), (w * h, w * h, w * h));
        let room = room(def.kind, def.cell);
        assert_eq!((room.kind, room.cell, room.cols, room.rows), (def.kind, def.cell, w, h));
        assert!(room.valid_snapshot(), "{}", def.slug);
        assert_eq!(size_of(def.kind), Some((w, h)));
        // Its border is wall, as the import crops it.
        for x in 0..w as i32 {
            assert_eq!((room.tile(x, 0), room.tile(x, h as i32 - 1)), (Tile::Wall, Tile::Wall), "{} col {x}", def.slug);
        }
        // Every `>` of the file is a plate of `DOORS`, and every plate's
        // every tile a `>`.
        let tiles: Vec<&str> = def.data.lines().skip(6).take(h).collect();
        for (y, row) in tiles.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                let plate = entrance_at(def.kind, x as i32, y as i32).is_some();
                assert_eq!(ch == b'>', plate, "{} at {x},{y}", def.slug);
                assert_eq!(room.tile(x as i32, y as i32) == Tile::Stairs, plate, "{} at {x},{y}", def.slug);
            }
        }
    }
    // A wrong file is refused, not half read.
    let bad = Def { kind: RoomKind::BomGate, slug: "the-gate", cell: (0, 0), data: "LEVEL the-gate\nNAME X\nZ 1\nSIZE 2 1\nORIGIN 0 0\nTILES\n.Q\n" };
    assert!(parse(&bad).is_err());
}

fn size_of(kind: RoomKind) -> Option<(usize, usize)> {
    size(kind)
}

#[test]
fn the_doors_are_kingdom_jsons_links() {
    let want = doors_from_json();
    assert_eq!(DOORS.len(), want.len(), "run print_doors to write DOORS afresh");
    for (e, (id, room, at, to, arrive, label)) in DOORS.iter().zip(&want) {
        assert_eq!((e.id, e.room, e.at, e.label, e.needs), (id.as_str(), *room, *at, label.as_str(), None), "run print_doors");
        assert_eq!(e.to, Dest::Room(*to, *arrive), "{id}: run print_doors");
    }
    let top = &json_levels()[0]["entrance"];
    assert_eq!(ENTRANCE, rect(&top["at"]));
    assert_eq!(ARRIVAL, tile_of(&top["arrive"]));
    for e in &DOORS {
        assert!(ENTRANCES.iter().any(|o| o.id == e.id), "{} is an entrance of the world", e.id);
    }
}

/// `DOORS` written from `kingdom.json`, to paste over the const when the
/// import changes.
#[test]
#[ignore = "prints DOORS from kingdom.json"]
fn print_doors() {
    for (id, room, at, to, arrive, label) in doors_from_json() {
        if id == "bomrekedan-out" {
            continue;
        }
        println!("    door({id:?}, RoomKind::{room:?}, {at:?}, RoomKind::{to:?}, ({:?}, {:?}), {label:?}),", arrive.0, arrive.1);
    }
}

#[test]
fn every_stair_is_two_way_and_lands_on_open_floor() {
    for e in DOORS.iter() {
        let Dest::Room(to, (x, y)) = e.to else { panic!("{} goes to a room", e.id) };
        if e.id == "bomrekedan-out" {
            assert_eq!(e.room, TOP);
            continue;
        }
        let back = DOORS
            .iter()
            .find(|o| o.room == to && matches!(o.to, Dest::Room(k, _) if k == e.room))
            .unwrap_or_else(|| panic!("{} has no stair back", e.id));
        assert_eq!(back.at, e.at, "{}: one rect on both levels", e.id);
        let room = room(to, (0, 0));
        // The whole party lands on plain floor, off every plate: five tiles
        // in a row about where it stands, and the four-knight fan on them.
        let (tx, ty) = (x.floor() as i32, y.floor() as i32);
        for dx in -2..=2 {
            assert_eq!(room.tile(tx + dx, ty), Tile::Floor, "{} lands at {tx},{ty}", e.id);
        }
        for n in 0..4 {
            let hx = x - 1.8 + n as f32 * 1.2;
            assert!(entrance_at(to, hx.floor() as i32, ty).is_none(), "{} lands on a plate", e.id);
        }
    }
    // The up and down plates of a level are apart.
    for def in &LEVELS {
        let mine: Vec<_> = DOORS.iter().filter(|e| e.room == def.kind).collect();
        for (i, a) in mine.iter().enumerate() {
            for b in &mine[i + 1..] {
                let (ac, ar, aw, ah) = a.at;
                let (bc, br, bw, bh) = b.at;
                assert!(ac + aw <= bc || bc + bw <= ac || ar + ah <= br || br + bh <= ar, "{} and {} overlap", a.id, b.id);
            }
        }
    }
}

fn run() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    r
}

/// The party stands on `e` long enough to take it (a step first, where
/// they stand: one arriving on stairs steps off before they work).
fn take(r: &mut Run, e: &Entrance) {
    // A long walk with no keys down would leave the knights away, and an
    // away knight takes no stair (`fellowship`).
    r.idle.clear();
    r.step(&BTreeMap::new());
    let (x, y) = entrance_centre(e);
    for hero in r.players.values_mut() {
        (hero.x, hero.y) = (x, y);
    }
    for _ in 0..DESCEND_HOLD + 2 {
        r.step(&BTreeMap::new());
    }
}

/// Out by the depot's plate: each knight leaves for the kingdom on their
/// own (`exits`), the last one out closing the fort behind them.
fn take_out(r: &mut Run) {
    take(r, door("bomrekedan-out"));
    for _ in 0..4 * DESCEND_HOLD {
        if r.room().kind == RoomKind::Kingdom {
            break;
        }
        r.step(&BTreeMap::new());
    }
}

/// Through the kingdom's door: E at the Gate of Bomrekedan.
fn in_at_the_gate(r: &mut Run) -> (f32, f32) {
    let geo = crate::rules::map::geography(&r.kingdom.wishes);
    let site = geo.sites.iter().find(|s| s.room == TOP).expect("the gate's door on the kingdom's map");
    assert_eq!(site.name, GATE_NAME);
    eprintln!("{GATE_NAME}: lot {:?}, stand {:?}, biome {:?}", site.lot.map(|l| (l.tx, l.ty, l.tw, l.th)), site.stand, crate::world::biome::at(site.stand.0, site.stand.1));
    assert_eq!(geo.sites.iter().filter(|s| is_level(s.room)).count(), 1, "one door to the fort");
    let at = ((site.stand.0 as f32 + 0.5) * 2.0, (site.stand.1 as f32 + 0.7) * 2.0);
    for h in r.players.values_mut() {
        (h.x, h.y) = at;
    }
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    at
}

#[test]
fn the_kingdoms_door_opens_on_the_gate_and_the_depots_plate_leads_back_to_it() {
    let mut r = run();
    let stand = in_at_the_gate(&mut r);
    assert_eq!(r.room().kind, TOP);
    for h in r.players.values() {
        assert!(r.settlement_walkable_for_test(h.x, h.y), "arrives on open ground");
        assert!((h.y / 2.0 - ARRIVAL.1).abs() < 1.0 && (h.x / 2.0 - ARRIVAL.0).abs() < 3.0);
    }
    assert_eq!(r.kingdom.return_to.as_ref().unwrap().root, TOP);
    let saved: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
    assert!(saved.valid_snapshot());
    // Standing a while by the plate takes nobody anywhere.
    for _ in 0..DESCEND_HOLD * 2 {
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.room().kind, TOP);
    take_out(&mut r);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    for h in r.players.values() {
        assert!((h.x - stand.0).abs() < 2.0 && (h.y - stand.1).abs() < 0.01, "back at the door");
    }
    assert!(r.valid_snapshot());
}

#[test]
fn a_party_walks_down_every_stair_to_the_forges_and_back_up_and_out() {
    let mut r = run();
    let stand = in_at_the_gate(&mut r);
    assert_eq!(r.room().kind, TOP);
    let down: Vec<&Entrance> = DOORS.iter().filter(|e| e.id.ends_with("-down")).collect();
    assert_eq!(down.len(), LEVELS.len() - 1);
    for (i, e) in down.iter().enumerate() {
        assert_eq!(r.room().kind, LEVELS[i].kind);
        take(&mut r, e);
        assert_eq!(r.room().kind, LEVELS[i + 1].kind, "down {}", e.id);
        assert_eq!(r.dungeon.depth, 0, "the fort is home");
        for h in r.players.values() {
            assert!(r.settlement_walkable_for_test(h.x, h.y), "{} lands on open ground", e.id);
        }
        // The visit is still the gate's.
        assert_eq!(r.kingdom.return_to.as_ref().unwrap().root, TOP);
    }
    assert!(r.valid_snapshot());
    for i in (1..LEVELS.len()).rev() {
        let up = DOORS
            .iter()
            .find(|e| e.room == LEVELS[i].kind && e.id.ends_with("-up"))
            .unwrap();
        take(&mut r, up);
        assert_eq!(r.room().kind, LEVELS[i - 1].kind, "up {}", up.id);
    }
    take_out(&mut r);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert!((r.players[&1].x - stand.0).abs() < 2.0);
}

fn people_json() -> serde_json::Value {
    serde_json::from_str(include_str!("../../assets/bomrekedan/people.json")).unwrap()
}

#[test]
fn everyone_of_the_fort_is_read_and_stands_on_their_own_level() {
    let json = people_json();
    let raw = json["people"].as_array().unwrap();
    let all = people::all();
    assert_eq!(all.len(), raw.len(), "everyone in people.json has a place");
    assert_eq!(all.len() as u64, json["count"].as_u64().unwrap());
    let mut seen = std::collections::BTreeSet::new();
    for (slot, p) in all.iter().enumerate() {
        assert_eq!(p.slot, slot);
        assert!(seen.insert(p.id), "{} twice", p.id);
        let r = raw.iter().find(|r| r["id"] == p.id).unwrap();
        // On the level the import had them on, or their district's.
        let want = r["level"].as_str().or(r["district"].as_str()).unwrap();
        assert_eq!(LEVELS[index(p.level).unwrap()].slug, want, "{}", p.name);
        assert_eq!(p.from_data, r["level"].is_string());
        assert!(!p.name.is_empty() && p.name.is_ascii(), "{:?}", p.name);
        assert!(!p.lines.is_empty());
    }
    // Every level the data names has its people, and `on` is theirs alone.
    for def in &LEVELS {
        let here = people::on(def.kind);
        assert!(here.iter().all(|p| p.level == def.kind));
        assert_eq!(here.len(), all.iter().filter(|p| p.level == def.kind).count());
        eprintln!("{}: {} people", def.slug, here.len());
    }
    assert!(people::on(RoomKind::Gate).is_empty());
    // Of every people the fort has.
    for race in [people::Race::Dwarf, people::Race::Human, people::Race::Erinys, people::Race::Gnoll] {
        assert!(all.iter().any(|p| p.race == race), "{race:?}");
    }
}

#[test]
fn every_person_stands_on_open_floor_clear_of_the_stairs_and_each_other() {
    for def in &LEVELS {
        let room = room(def.kind, (0, 0));
        let here = people::on(def.kind);
        let mut homes = std::collections::BTreeSet::new();
        let teachers: Vec<(i32, i32)> = crate::rules::questline::book().lines.iter().filter(|q| q.kind() == def.kind).map(|q| q.at).collect();
        for p in here {
            assert!(homes.insert(p.home), "{} shares {:?}", p.name, p.home);
            for &(tx, ty) in &teachers {
                let d = (p.home.0 - tx).abs().max((p.home.1 - ty).abs());
                assert!(d >= people::TEACHER_CLEAR, "{} stands {d} tiles from the teacher at {tx},{ty}", p.name);
            }
            assert!(p.route[0] == p.home && p.route.len() <= 4);
            for &(x, y) in &p.route {
                assert_eq!(room.tile(x, y), Tile::Floor, "{} at {x},{y} on {}", p.name, def.slug);
                for w in works::WORKS.iter().filter(|w| w.level == def.kind) {
                    let (c, r, sw, sh) = w.sign;
                    assert!(!(x >= c - 1 && x < c + sw + 1 && y >= r - 1 && y < r + sh + 1), "{} walks onto the sign of {}", p.name, w.name);
                }
                assert!(entrance_at(def.kind, x, y).is_none());
                for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
                    assert_ne!(room.tile(x + dx, y + dy), Tile::Stairs, "{} beside a stair", p.name);
                }
            }
            // Wherever their round takes them, at every moment, they are on
            // a floor tile of their level.
            for tick in (0..p.period * p.route.len() as u64 * 2).step_by(7) {
                let (x, y) = p.at(tick);
                assert_eq!(room.tile(x.floor() as i32, y.floor() as i32), Tile::Floor, "{} at tick {tick}", p.name);
                assert_eq!(p.at(tick), p.at(tick), "a pure function of the tick");
            }
        }
    }
    // Those the game had on a kept level stand where it had them, or near.
    let json = people_json();
    let mut moved = 0;
    for p in people::all().iter().filter(|p| p.from_data) {
        let r = json["people"].as_array().unwrap().iter().find(|r| r["id"] == p.id).unwrap();
        let (x, y) = (r["x"].as_i64().unwrap() as i32, r["y"].as_i64().unwrap() as i32);
        let d = (p.home.0 - x).abs().max((p.home.1 - y).abs());
        assert!(d <= 8, "{} placed {d} tiles from where the game had them", p.name);
        moved += (d > 0) as u32;
    }
    eprintln!("{moved} of those on kept levels stepped off their tile");
}

#[test]
fn a_fresh_e_beside_someone_has_them_say_their_line() {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    let kind = RoomKind::BomGreatHall;
    let i = room_of(&r.dungeon, kind).unwrap();
    let p = &people::on(kind)[0];
    r.arrive(i, p.at(r.tick));
    r.step(&BTreeMap::new());
    // The knight stands on the very spot they will be at next tick.
    let stand = |r: &mut Run| {
        let (x, y) = p.at(r.tick + 1);
        for h in r.players.values_mut() {
            (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
        }
    };
    stand(&mut r);
    r.found = None;
    let e = BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    r.step(&e);
    let (_, id, line) = r.found.clone().expect("they speak");
    assert_eq!(id, 1);
    let (hx, hy) = (r.players[&1].x / TILE_UNITS, r.players[&1].y / TILE_UNITS);
    let speaker = people::on(kind)
        .iter()
        .find(|o| line.starts_with(&format!("{}, ", o.name)))
        .unwrap_or_else(|| panic!("nobody of the hall says {line:?}"));
    let (sx, sy) = speaker.at(r.tick);
    assert!((sx - hx).hypot(sy - hy) < people::TALK_REACH + 0.5, "{} is out of reach", speaker.name);
    assert!(line.contains(" of the ") && line.contains(": "), "{line}");
    eprintln!("{line}");
    // Holding E says nothing more; a fresh press speaks again.
    r.found = None;
    stand(&mut r);
    r.step(&e);
    assert!(r.found.is_none(), "a hold is no press");
    stand(&mut r);
    r.step(&BTreeMap::new());
    stand(&mut r);
    r.step(&e);
    assert!(r.found.is_some());
    // Far from anyone, E says nothing.
    r.found = None;
    r.step(&BTreeMap::new());
    let lone = (1..r.room().cols as i32 - 1)
        .flat_map(|x| (1..r.room().rows as i32 - 1).map(move |y| (x, y)))
        .find(|&(x, y)| {
            r.room().tile(x, y) == Tile::Floor
                && people::on(kind).iter().all(|o| {
                    let (ox, oy) = o.at(r.tick + 1);
                    (ox - x as f32 - 0.5).hypot(oy - y as f32 - 0.5) > 6.0
                })
        })
        .unwrap();
    for h in r.players.values_mut() {
        (h.x, h.y) = ((lone.0 as f32 + 0.5) * TILE_UNITS, (lone.1 as f32 + 0.5) * TILE_UNITS);
    }
    r.step(&e);
    assert!(r.found.is_none());
    assert!(r.valid_snapshot());
}

#[test]
fn names_come_out_in_the_delves_letters() {
    assert_eq!(people::ascii("Ùngiz Aludedëm"), "Ungiz Aludedem");
    assert_eq!(people::ascii("Pebu Ñabsizabo"), "Pebu Nabsizabo");
    assert_eq!(people::ascii("Kolgídthur"), "Kolgidthur");
}

#[test]
fn a_party_that_never_came_by_the_kingdom_is_given_its_door_back() {
    // Arriving on a lower level of an old save, the visit is the gate's.
    let mut r = Run::at_home(7, 1, None, Default::default(), Default::default());
    r.raise_kingdom();
    let i = room_of(&r.dungeon, RoomKind::BomForges).unwrap();
    r.arrive(i, (38.5, 22.5));
    assert_eq!(r.kingdom.return_to.as_ref().unwrap().root, TOP);
    assert!(r.valid_snapshot());
    // Lower levels have no door on the map; the top level does.
    assert!(crate::rules::map::outdoor(TOP));
    assert!(LEVELS[1..].iter().all(|d| !crate::rules::map::outdoor(d.kind)));
    assert!(places_on(RoomKind::BomGreatHall).count() > 0);
}

fn pad(floor: &mut crate::rules::Floor, n: usize) {
    let start = floor.rooms.len();
    while floor.rooms.len() < n {
        let i = (floor.rooms.len() - start) as i32;
        floor.rooms.push(crate::rules::Room::solid_rock((100 + i, 40)));
    }
}

/// A home that already filled the old 128 slots still receives every fort
/// level, and now the whole surface too: the world is never cut to fit a
/// crowded settlement (2026-11-07: the roads lane's `HOME_ROOM_LIMIT` fix
/// folded into the fort's admission).
#[test]
fn a_full_old_home_still_receives_the_fort_and_the_surface() {
    let mut floor = crate::rules::layout::undercroft(crate::rules::Pack::Crypt);
    pad(&mut floor, 128);
    assert!(world::raise_world(&mut floor));
    assert!(floor.rooms.len() <= world::HOME_ROOM_LIMIT);
    assert!(LEVELS.iter().all(|d| floor.rooms.iter().any(|r| r.kind == d.kind)));
    assert!(world::ROOMS.iter().all(|&(k, c)| floor.rooms.iter().any(|r| r.kind == k && r.cell == c)));
    assert!(floor.fort_admit.omitted.is_empty(), "{:?}", floor.fort_admit.omitted);
    assert_eq!(floor.fort_admit.held_back, 0);
    assert!(!floor.fort_admit.line().contains("HELD BACK"));
    assert!(!world::raise_world(&mut floor), "a second raise changes nothing");
}

/// A home nearly at its surface budget still gets every fort level in the
/// slots kept for them; the surface rooms that no longer fit are counted,
/// not dropped in silence.
#[test]
fn a_home_past_the_surface_budget_still_receives_the_fort_and_counts_what_is_held_back() {
    let mut floor = crate::rules::layout::undercroft(crate::rules::Pack::Crypt);
    pad(&mut floor, 240);
    assert!(world::raise_world(&mut floor));
    assert!(floor.rooms.len() <= world::HOME_ROOM_LIMIT);
    assert!(LEVELS.iter().all(|d| floor.rooms.iter().any(|r| r.kind == d.kind)));
    assert!(floor.fort_admit.omitted.is_empty(), "{:?}", floor.fort_admit.omitted);
    assert!(floor.fort_admit.held_back > 0);
    assert!(floor.fort_admit.line().contains("HELD BACK"));
}

/// Past the new cap the levels that do not fit are named.
#[test]
fn a_home_past_the_cap_names_the_levels_it_leaves_out() {
    let mut floor = crate::rules::layout::undercroft(crate::rules::Pack::Crypt);
    pad(&mut floor, world::HOME_ROOM_LIMIT - 3);
    assert!(world::raise_world(&mut floor));
    let got = floor.rooms.iter().filter(|r| is_level(r.kind)).count();
    assert_eq!(got, 3);
    assert_eq!(floor.fort_admit.omitted.len(), 7);
    let line = floor.fort_admit.line();
    assert!(line.contains("LEFT OUT") && line.contains("THE HOME IS FULL"), "{line}");
    assert!(floor.rooms.len() <= world::HOME_ROOM_LIMIT);
}

/// Untouched saved ground follows a re-import. Goods stay, and the works
/// ledger is not rewritten.
#[test]
fn untouched_saved_ground_follows_the_import_and_goods_stay() {
    let mut floor = crate::rules::layout::undercroft(crate::rules::Pack::Crypt);
    world::raise_world(&mut floor);
    let idx = floor.rooms.iter().position(|r| r.kind == RoomKind::BomWorkshops).unwrap();
    let (col, row) = {
        let room = &floor.rooms[idx];
        (4..room.cols)
            .flat_map(|c| (4..room.rows).map(move |r| (c, r)))
            .find(|&(c, r)| room.tile(c as i32, r as i32) == Tile::Floor)
            .unwrap()
    };
    floor.rooms[idx].set(col, row, Tile::Wall);
    assert!(world::raise_world(&mut floor));
    assert_eq!(floor.rooms[idx].tile(col as i32, row as i32), Tile::Floor);
    assert!(floor.fort_admit.refreshed.iter().any(|n| n.contains("WORKSHOP")), "{:?}", floor.fort_admit.refreshed);

    floor.rooms[idx].set(col, row, Tile::Wall);
    floor.rooms[idx].items.push(crate::rules::Item {
        card: "potion".into(),
        x: 1.0,
        y: 1.0,
        held_off: None,
        took: Vec::new(),
        dropped: 0,
        owner: None,
    });
    assert!(!world::raise_world(&mut floor), "keeping goods does not rewrite the floor");
    assert_eq!(floor.rooms[idx].tile(col as i32, row as i32), Tile::Wall, "goods keep the saved ground");
    assert!(floor.fort_admit.kept.iter().any(|n| n.contains("WORKSHOP")), "{:?}", floor.fort_admit.kept);
    assert_eq!(floor.rooms[idx].items.len(), 1);

    let mut r = run();
    r.home.bomrekedan.built.insert("still".into(), 2);
    let before = serde_json::to_string(&r.home).unwrap();
    r.raise_home_world();
    assert_eq!(serde_json::to_string(&r.home).unwrap(), before);
}

/// A knight standing where a re-import closes the ground is moved to the
/// level's arrival, and told.
#[test]
fn a_knight_on_closed_ground_is_set_on_the_arrival() {
    let mut r = run();
    let kind = RoomKind::BomWorkshops;
    let i = room_of(&r.dungeon, kind).unwrap();
    let (col, row) = {
        let room = &r.dungeon.rooms[i];
        (2..room.cols)
            .flat_map(|c| (2..room.rows).map(move |r| (c, r)))
            .find(|&(c, r)| room.tile(c as i32, r as i32) == Tile::Wall)
            .unwrap()
    };
    r.dungeon.rooms[i].set(col, row, Tile::Floor);
    r.arrive(i, (col as f32 + 0.5, row as f32 + 0.5));
    r.found = None;
    r.raise_home_world();
    let (x, y) = ((r.players[&1].x / TILE_UNITS).floor() as i32, (r.players[&1].y / TILE_UNITS).floor() as i32);
    assert!(matches!(r.room().tile(x, y), Tile::Floor | Tile::Door | Tile::Stairs), "standing on {:?}", r.room().tile(x, y));
    let line = r.found.clone().expect("the fort says what it did").2;
    assert!(line.contains("BROUGHT"), "{line}");
}
