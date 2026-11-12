//! The blank-slate plan's facts (`crate::world::plan`): one tiny castle, a
//! camp for each model house on a ring of hillside, the Delve's door, and
//! nothing else. The first group builds the plan directly (pure data and
//! functions of `plan::tabula`) and runs in every build; the second reads the
//! global map through its modules and needs the plan standing, so it is
//! ignored unless asked for:
//!
//! `ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::plan -- --include-ignored`

use super::tabula::{self, CAMPS, DELVE_GATE, KEEP, KEEP_STAND};
use super::{Plan, Rect};
use crate::world::houses;
use crate::world::map::{CHUNK_H, CHUNK_W, MAP_H, MAP_W, Realm};

const NEEDS_TABULA: &str = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::plan -- --include-ignored";

fn inside(r: Rect, x: i32, y: i32) -> bool {
    x >= r.0 && y >= r.1 && x < r.0 + r.2 && y < r.1 + r.3
}

fn overlap(a: Rect, b: Rect, margin: i32) -> bool {
    a.0 < b.0 + b.2 + margin && b.0 < a.0 + a.2 + margin && a.1 < b.1 + b.3 + margin && b.1 < a.1 + a.3 + margin
}

#[test]
fn the_switch_is_one_constant_and_the_shipped_realm_is_the_blank_slate() {
    assert_eq!(super::PLAN, Plan::Tabula);
}

#[test]
fn one_camp_for_each_house_and_one_tiny_castle_all_inside_the_map() {
    assert_eq!(CAMPS.len(), houses::all().len());
    assert_eq!(CAMPS.len(), 16);
    // The tiny castle is small enough to see whole (the view is 24 x 14 tiles).
    assert!(KEEP.2 <= 10 && KEEP.3 <= 6, "the Keep is {KEEP:?}");
    for r in std::iter::once(KEEP).chain(CAMPS).chain([DELVE_GATE]) {
        assert!(r.0 > 1 && r.1 > 1 && r.0 + r.2 < MAP_W - 1 && r.1 + r.3 < MAP_H - 1, "{r:?} is off the map");
    }
}

#[test]
fn the_camps_ring_the_keep_on_loose_hillside_at_a_walking_distance() {
    let (kx, ky) = (KEEP.0 + KEEP.2 / 2, KEEP.1 + KEEP.3 / 2);
    let mut angles = Vec::new();
    for (i, c) in CAMPS.iter().enumerate() {
        let (cx, cy) = (c.0 + c.2 / 2, c.1 + c.3 / 2);
        let d = f64::from((cx - kx).pow(2) + (cy - ky).pow(2)).sqrt();
        // Five to nine seconds at the walking pace (6.5 tiles a second).
        assert!((25.0..=65.0).contains(&d), "camp {i} is {d:.0} tiles from the Keep");
        angles.push(f64::from(cy - ky).atan2(f64::from(cx - kx)));
        // Wholly inside one chunk, with its knight's tile and its name's rows
        // below it (so the 3D chunks never cut a camp).
        let whole = (c.0, c.1, c.2, c.3 + 3);
        assert!(tabula::chunk_of(whole).is_some(), "camp {i} {c:?} straddles a chunk edge");
        assert_eq!(c.2, tabula::CAMP_W);
        assert_eq!(c.3, tabula::CAMP_H);
    }
    // Round the Keep, not in a line on one side of it: the angles cover the
    // circle, each octant holding at least one camp.
    for octant in 0..8 {
        let lo = -std::f64::consts::PI + f64::from(octant) * std::f64::consts::FRAC_PI_4;
        let hi = lo + std::f64::consts::FRAC_PI_4;
        assert!(angles.iter().any(|&a| a >= lo && a < hi + 1e-9), "no camp in octant {octant}");
    }
    // Loose: no two share a row or a column, and none crowds another.
    for (i, a) in CAMPS.iter().enumerate() {
        for b in &CAMPS[i + 1..] {
            assert!((a.0 - b.0).abs().max((a.1 - b.1).abs()) >= 11, "{a:?} crowds {b:?}");
            assert!(!overlap(*a, *b, 3), "{a:?} touches {b:?}");
        }
    }
    let mut chunks: Vec<_> = CAMPS.iter().filter_map(|&c| tabula::chunk_of(c)).collect();
    chunks.sort_unstable();
    chunks.dedup();
    assert_eq!(chunks.len(), 16, "one camp to a chunk");
}

#[test]
fn nothing_stands_on_a_camp_the_castle_or_the_gate_and_they_keep_apart() {
    let closed = tabula::closed();
    assert_eq!(closed.len(), 18, "the Keep, the gate and sixteen camps");
    for (i, a) in closed.iter().enumerate() {
        for b in &closed[i + 1..] {
            assert!(!overlap(*a, *b, 2), "{a:?} and {b:?} touch");
        }
    }
    // The Keep and the Delve's gate share the middle chunk; neither is a camp.
    assert_eq!(tabula::chunk_of(KEEP), Some((5, 5)));
    assert_eq!(tabula::chunk_of(DELVE_GATE), Some((5, 5)));
    // The knight's stand and the spawn are open ground, never inside what they serve.
    for r in CAMPS {
        let (sx, sy) = tabula::camp_stand(r);
        assert!(tabula::walkable(sx, sy), "{r:?}'s stand {sx},{sy} is closed");
        assert!(!closed.iter().any(|&c| inside(c, sx, sy)));
    }
    assert!(tabula::walkable(KEEP_STAND.0, KEEP_STAND.1));
    let (dx, dy) = tabula::delve_stand();
    assert!(tabula::walkable(dx, dy));
    assert_eq!(KEEP_STAND, (KEEP.0 + KEEP.2 / 2, KEEP.1 + KEEP.3));
}

#[test]
fn every_camp_and_the_delve_door_are_reachable_on_foot_from_the_spawn() {
    assert!(tabula::reaches(tabula::delve_stand()), "the Delve's door is not reachable from the spawn");
    for r in CAMPS {
        assert!(tabula::reaches(tabula::camp_stand(r)), "{r:?} is not reachable");
    }
    // The only way to a closed tile is not on foot.
    assert!(!tabula::reaches((KEEP.0 + 1, KEEP.1 + 1)));
}

#[test]
fn the_ground_is_open_meadow_edge_to_edge_with_no_other_terrain() {
    let realm = Realm::tabula();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            assert_eq!(realm.at(x, y), b'.', "{x},{y}");
        }
    }
    // The camps' and the Keep's ground is meadow (a camp is built on grass, not on a road or in water).
    for r in std::iter::once(KEEP).chain(CAMPS) {
        for y in r.1..r.1 + r.3 {
            for x in r.0..r.0 + r.2 {
                assert!(crate::world::map::walkable(realm.at(x, y)));
            }
        }
    }
    let _ = (CHUNK_W, CHUNK_H);
}

// ───────────────────────── the global map, with the plan standing ─────────

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::plan -- --include-ignored"]
fn the_only_door_is_the_delve_and_it_is_reachable_from_the_spawn() {
    assert!(super::tabula(), "{NEEDS_TABULA}");
    use crate::world::doors::{self, Door};
    assert_eq!(doors::doors(), &[(tabula::delve_stand(), Door::Delve)]);
    assert!(doors::walkable(crate::world::kingdom::keep_stand()));
    assert!(doors::walkable(tabula::delve_stand()));
    assert_eq!(crate::world::kingdom::keep_stand(), KEEP_STAND);
    let geo = crate::world::kingdom::geography(&[]);
    assert_eq!(geo.sites.len(), 1);
    assert_eq!(geo.sites[0].room, crate::rules::RoomKind::Gate);
    assert_eq!(geo.sites[0].stand, tabula::delve_stand());
    assert!(crate::world::kingdom::regions().is_empty());
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::plan -- --include-ignored"]
fn the_walkers_ground_is_everywhere_but_the_castle_the_camps_and_the_gate() {
    assert!(super::tabula(), "{NEEDS_TABULA}");
    use crate::world::kingdom::step_cost;
    let castles = crate::world::castles::castles();
    assert_eq!(castles.len(), 17, "sixteen camps and the Keep");
    assert_eq!(castles.iter().filter(|c| c.is_camp()).count(), 16);
    assert_eq!(castles.iter().filter(|c| c.is_tiny()).count(), 1);
    assert!(castles.iter().all(|c| c.doors.is_empty()));
    for r in tabula::closed() {
        for y in r.1..r.1 + r.3 {
            for x in r.0..r.0 + r.2 {
                assert_eq!(step_cost(x, y), None, "{x},{y} is walkable");
            }
        }
    }
    // Every other tile one in from the rim is walkable, level with the plan's own idea.
    for y in 1..MAP_H - 1 {
        for x in 1..MAP_W - 1 {
            assert_eq!(step_cost(x, y).is_some(), tabula::walkable(x, y), "{x},{y}");
        }
    }
    // The camps are the houses' own, in house order, each the house's footprint.
    for (id, camp) in houses::ids().zip(CAMPS) {
        assert_eq!(houses::footprint(id), camp);
        assert_eq!(houses::stand(id), tabula::camp_stand(camp));
        assert_eq!(crate::world::castles::of_house(id).foot, camp);
    }
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::plan -- --include-ignored"]
fn nothing_else_is_on_the_map_no_posts_halts_finds_dressing_or_set_pieces() {
    assert!(super::tabula(), "{NEEDS_TABULA}");
    assert!(crate::world::signs::posts().is_empty());
    assert!(crate::world::waystations::placed().is_empty());
    assert!(crate::world::finds::finds(7).is_empty());
    let dressing = crate::world::dressing::get();
    assert!(dressing.pieces.is_empty() && dressing.camps.is_empty());
    assert_eq!(crate::world::map::set_pieces().count(), 0);
    assert!(crate::world::map::structures().is_empty());
    let realm = Realm::get();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            assert_eq!(realm.at(x, y), b'.');
            assert_eq!(crate::world::biome::at(x, y), tabula::BIOME);
        }
    }
    // The old places are off the map: only the Keep stands.
    for place in crate::world::map::Place::ALL {
        assert_eq!(place.on_map(), place == crate::world::map::Place::Keep, "{place:?}");
    }
    assert!(crate::world::college::approach().is_empty());
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::plan -- --include-ignored"]
fn wished_buildings_stand_on_the_open_ground_and_never_on_a_camp_the_castle_or_the_gate() {
    assert!(super::tabula(), "{NEEDS_TABULA}");
    use crate::realm::Wish;
    let wishes: Vec<Wish> = (0..40)
        .map(|i| Wish {
            id: format!("w{i}"),
            name: format!("Wish {i}"),
            words: "a hall".into(),
            by: "Sir Test".into(),
            status: crate::realm::Status::Built,
            price: Default::default(),
            art: vec!["u".repeat(24); 24],
            near: ["keep", "smithy", "village", "mines", "swamp"][i % 5].to_string(),
            paid: Vec::new(),
            raised: i as u32 + 1,
            lot: None,
        })
        .collect();
    let lots = crate::world::wishes::lots(&wishes);
    assert_eq!(lots.len(), 40);
    let placed: Vec<Rect> = lots.iter().flatten().map(|l| (l.tx, l.ty, l.tw, l.th)).collect();
    assert_eq!(placed.len(), 40, "every wish stands: {:?}", crate::world::wishes::names_without_ground(&wishes, &lots));
    let closed = tabula::closed();
    let yards: Vec<Rect> = CAMPS.iter().map(|&c| (c.0, c.1, c.2, c.3 + 3)).collect();
    for (i, lot) in placed.iter().enumerate() {
        for c in closed.iter().chain(&yards) {
            assert!(!overlap(*lot, *c, 0), "wish {i} {lot:?} is on {c:?}");
        }
        for other in &placed[i + 1..] {
            assert!(!overlap(*lot, *other, 0), "{lot:?} overlaps {other:?}");
        }
        // And on ground the walkers can build on.
        for y in lot.1..lot.1 + lot.3 {
            for x in lot.0..lot.0 + lot.2 {
                assert_eq!(Realm::get().at(x, y), b'.');
            }
        }
    }
    // They gather round the Keep, where the knights are, before the camps.
    let (kx, ky) = (KEEP.0 + KEEP.2 / 2, KEEP.1 + KEEP.3 / 2);
    assert!(
        placed.iter().all(|l| (l.0 - kx).abs() < 30 && (l.1 - ky).abs() < 24),
        "a wish stands far from the Keep: {placed:?}"
    );
}

// ───────────────────────── a report: rooms that lost their way in ─────────

/// The rooms the full plan's map had a door to (the places, the countryside
/// places, the halls, the castles' hall, Bomrekedan's gate, the College), and
/// which of them can still be walked to from the Delve's gate along the home
/// floor's own trails and stairs (`rules::world::TRAILS`, `ENTRANCES`).
/// `cargo test -p angel-delve --lib world::plan::tests::report -- --ignored --nocapture`
#[test]
#[ignore = "a report, not a check: prints the rooms the blank-slate map no longer has a door to"]
fn report_rooms_with_no_way_in_once_the_places_are_off_the_map() {
    use crate::rules::RoomKind;
    use crate::rules::world::{Dest, ENTRANCES, TRAILS};
    use std::collections::{BTreeSet, VecDeque};
    // The full plan's doors, whichever plan stands: each place's room.
    let mut had: BTreeSet<String> = BTreeSet::new();
    for r in crate::world::kingdom::REGIONS.iter().flat_map(|r| r.places.iter()) {
        had.insert(format!("{:?}", r.0));
    }
    for room in [
        RoomKind::MineHead, RoomKind::Lists, RoomKind::Stables, RoomKind::Forest, RoomKind::Marsh, RoomKind::Meadow,
        RoomKind::Village, RoomKind::Rookery, RoomKind::Observatory, RoomKind::Smithy, RoomKind::Market,
        RoomKind::ColosseumHall, RoomKind::CollegeGrounds, RoomKind::RealmHall, RoomKind::LandOffice,
        RoomKind::Gate, RoomKind::Fairground,
    ] {
        had.insert(format!("{room:?}"));
    }
    // The graph, from the home floor as a run raises it: a room's open doors
    // to the rooms beside it, and each stair that goes to another room.
    let mut run = crate::rules::Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    let floor = &run.dungeon;
    let kind = |i: usize| format!("{:?}", floor.rooms[i].kind);
    let start = crate::rules::world::room_of(floor, RoomKind::Gate).expect("the Delve's gate courtyard is on the home floor");
    let mut seen: BTreeSet<String> = BTreeSet::from([kind(start)]);
    let mut open = VecDeque::from([start]);
    let mut visited = vec![false; floor.rooms.len()];
    visited[start] = true;
    while let Some(i) = open.pop_front() {
        let mut next: Vec<usize> = (0..4).filter_map(|dir| floor.neighbour(i, dir)).collect();
        for e in ENTRANCES.iter().filter(|e| e.room == floor.rooms[i].kind) {
            if let Dest::Room(to, _) = e.to
                && let Some(j) = crate::rules::world::room_of(floor, to)
            {
                next.push(j);
            }
        }
        for j in next {
            if !visited[j] {
                visited[j] = true;
                seen.insert(kind(j));
                open.push_back(j);
            }
        }
    }
    let _ = TRAILS;
    let lost: Vec<&String> = had.iter().filter(|r| !seen.contains(*r)).collect();
    println!("rooms the full map had a door to: {}", had.len());
    println!("reachable on from the Delve's gate by the home floor's trails and stairs: {}", had.iter().filter(|r| seen.contains(*r)).count());
    println!("NO WAY IN ({}): {:?}", lost.len(), lost);
}

// ───────────────────────── a knight, from the spawn to the Delve and back ─

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::plan -- --include-ignored"]
fn a_knight_walks_from_the_spawn_to_the_delves_door_goes_down_and_comes_back_up() {
    assert!(super::tabula(), "{NEEDS_TABULA}");
    use crate::rules::layout::TILE_UNITS;
    use crate::rules::{Input, Run, RoomKind};
    use std::collections::BTreeMap;
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(KEEP_STAND));
    assert_eq!(run.room().kind, RoomKind::Kingdom);
    let (sx, sy) = KEEP_STAND;
    let me = &run.players[&1];
    assert_eq!((me.x / TILE_UNITS) as i32, sx);
    assert_eq!((me.y / TILE_UNITS) as i32, sy);
    // The first goal on the surface is the Delve.
    let goal = run.kingdom_goal(1).expect("a goal");
    assert_eq!(goal.name, "The Delve");
    assert_eq!(goal.stand, tabula::delve_stand());
    // Walk to its stand (east along the row below the Keep and the gate), as a
    // knight does, and press E.
    let (gx, gy) = tabula::delve_stand();
    let target = ((gx as f32 + 0.5) * TILE_UNITS, (gy as f32 + 0.7) * TILE_UNITS);
    let mut near = false;
    for _ in 0..3000 {
        let me = &run.players[&1];
        let (dx, dy) = (target.0 - me.x, target.1 - me.y);
        if dx.hypot(dy) < 0.5 {
            near = true;
            break;
        }
        let sign = |d: f32| if d > 0.35 { 1 } else if d < -0.35 { -1 } else { 0 };
        run.step(&BTreeMap::from([(1, Input { move_x: sign(dx), move_y: sign(dy), ..Default::default() })]));
    }
    assert!(near, "the knight never reached the Delve's door");
    for _ in 0..5 {
        run.step(&BTreeMap::new());
    }
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    assert_eq!(run.room().kind, RoomKind::Gate, "E at the Delve's door goes down to the gate");
    // And up again: back at the door, on open ground.
    run.leave_kingdom_place();
    assert_eq!(run.room().kind, RoomKind::Kingdom);
    let me = &run.players[&1];
    let (tx, ty) = ((me.x / TILE_UNITS) as i32, (me.y / TILE_UNITS) as i32);
    assert!(tabula::walkable(tx, ty), "came up onto {tx},{ty}");
    assert!((tx - gx).abs() <= 2 && (ty - gy).abs() <= 2);
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::plan -- --include-ignored"]
fn an_old_save_with_a_knight_standing_in_the_new_castle_loads_and_moves_him_to_open_ground() {
    assert!(super::tabula(), "{NEEDS_TABULA}");
    use crate::rules::layout::TILE_UNITS;
    use crate::rules::{Run, RoomKind};
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(KEEP_STAND));
    // The save carried a knight where the Keep's tower now stands, and a return
    // door to the Smithy, a place that is off the map.
    let inside = ((KEEP.0 + 2) as f32 + 0.5, (KEEP.1 + 2) as f32 + 0.5);
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (inside.0 * TILE_UNITS, inside.1 * TILE_UNITS);
    }
    run.kingdom.return_to = Some(crate::rules::kingdom::Return {
        x: 17.0 * TILE_UNITS,
        y: 19.0 * TILE_UNITS,
        name: "Smithy".into(),
        root: RoomKind::Smithy,
        party: Default::default(),
    });
    // The saved kingdom room is another ground than the plan's (one tile of it
    // differs, as the old realm's did all over): reloading swaps the new map in.
    let i = crate::rules::world::room_of(&run.dungeon, RoomKind::Kingdom).unwrap();
    run.dungeon.rooms[i].set(5, 5, crate::rules::Tile::Wall);
    let json = serde_json::to_value(&run).unwrap();
    let mut loaded: Run = serde_json::from_value(json).unwrap();
    loaded.raise_kingdom();
    let me = &loaded.players[&1];
    let (tx, ty) = ((me.x / TILE_UNITS) as i32, (me.y / TILE_UNITS) as i32);
    assert!(tabula::walkable(tx, ty), "the knight is still on closed ground at {tx},{ty}");
    let back = loaded.kingdom.return_to.as_ref().expect("the return door is kept");
    let door = ((back.x / TILE_UNITS) as i32, (back.y / TILE_UNITS) as i32);
    assert_eq!(door, KEEP_STAND, "a return door to a place off the map comes out at the Keep");
}
