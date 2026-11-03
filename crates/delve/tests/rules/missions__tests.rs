use super::*;
use crate::render::overworld::{arena, ink, kingdom as geography};

/// The kingdom with two knights side by side at the keep: Friend (seat 1)
/// and Second (seat 2), both of one class.
fn kingdom_pair() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_kingdom(Some(geography::keep_stand()));
    r.join("Second");
    let (x, y) = {
        let one = &r.players[&1];
        (one.x, one.y)
    };
    for hero in r.players.values_mut() {
        hero.class = Some("Swordsman".into());
        (hero.x, hero.y) = (x, y);
    }
    r
}

/// Knight `id` holds the second rank of Stilt's wish: the fork questline
/// is open to them (the teacher waits at the millpond, at 10 3).
fn take_fork(r: &mut Run, id: u32) {
    let book = phrasebook::book();
    let fork = book.iter().find(|w| w.id == "fork").unwrap();
    r.players.get_mut(&id).unwrap().deck.push(fork.card_id(1));
}

/// The kingdom door the millpond is reached by.
fn millpond_door(r: &Run) -> (i32, i32) {
    let geo = geography::geography(&r.kingdom.wishes);
    geo.sites
        .iter()
        .find(|s| s.room == RoomKind::Millpond)
        .or_else(|| r.gateway(RoomKind::Millpond, &geo))
        .expect("the millpond has a door")
        .stand
}

#[test]
fn a_knight_holding_a_questline_is_shown_its_teachers_door_and_another_is_not() {
    let mut r = kingdom_pair();
    take_fork(&mut r, 1);
    let held = r.missions_held(1);
    assert!(
        held.iter().any(|h| h.name.starts_with("Stilt")
            && h.place == missions::Place::Room(RoomKind::Millpond, Some((10.0, 3.0)))),
        "Friend holds Stilt's questline, at the millpond: {held:?}"
    );
    assert!(!r.missions_held(2).iter().any(|h| h.name.starts_with("Stilt")));
    let door = millpond_door(&r);
    assert!(
        r.mission_doors(1).iter().any(|&(ref name, at)| at == door && name.starts_with("Stilt")),
        "{:?}",
        r.mission_doors(1)
    );
    assert!(r.mission_doors(2).iter().all(|&(_, at)| at != door));
    // Walked to its end, it is no longer held.
    let key = Home::knight_key(&r.players[&1]);
    r.home
        .questlines
        .knights
        .entry(key)
        .or_default()
        .entry("wish:fork".into())
        .or_default()
        .trial_done = true;
    assert!(r.mission_doors(1).iter().all(|&(_, at)| at != door));
}

#[test]
fn the_mission_pennant_is_drawn_on_the_holders_view_only() {
    let mut r = kingdom_pair();
    take_fork(&mut r, 1);
    let amber = ink::ink('@').unwrap();
    let count = |seat: u32| {
        let im = arena::frame_for(&r, 384, 224, Some(seat));
        (0..im.h)
            .flat_map(|y| (0..im.w).map(move |x| (x, y)))
            .filter(|&(x, y)| im.get(x, y) == Some(amber))
            .count()
    };
    let (one, two) = (count(1), count(2));
    assert!(one > two + 20, "Friend's view carries the mission's mark: {one} amber against {two}");
}

#[test]
fn in_the_teachers_room_the_holder_sees_the_spot_and_another_does_not() {
    let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    r.come_to_the_gate();
    r.join("Second");
    take_fork(&mut r, 1);
    r.arrive(world::room_of(&r.dungeon, RoomKind::Millpond).unwrap(), (12.0, 6.0));
    assert_eq!(r.mission_spots(1), vec![(String::from("Stilt the Heron-Warden"), (10.0, 3.0))]);
    assert!(r.mission_spots(2).is_empty());
}

#[test]
fn on_a_floor_a_dares_room_is_marked_on_each_knights_map_and_not_the_partys() {
    let mut run = Run::new(5, 17, None);
    let pack = Pack::at(2, Pack::Crypt);
    run.dungeon = layout::floor_for(fortune::Mode::LongWayDown, 2, pack, &mut run.rng);
    run.populate_boss_gates();
    run.enter(0, None);
    run.join("Second");
    for room in &mut run.dungeon.rooms {
        room.visited = true;
    }
    let marked = |run: &Run, seat: Option<u32>| -> Vec<usize> {
        run.floor_map(seat)
            .nodes
            .iter()
            .filter(|n| n.icons.contains(&floor_map::Icon::Mission))
            .map(|n| n.id)
            .collect()
    };
    // No dare is kept, so no room is marked as a mission.
    run.dare = None;
    assert!(marked(&run, Some(1)).is_empty(), "{:?}", marked(&run, Some(1)));
    // Make It Quick names the floor's guardian to every knight who keeps it.
    run.dare = Some(dares::Dare {
        kind: dares::DareKind::MakeItQuick,
        since: run.tick,
        count: 0,
        kept: false,
        broken: false,
        rose: None,
    });
    let guardian = run.boss_gates.as_ref().map(|g| g.guardian).expect("the floor keeps a guardian");
    assert!(marked(&run, Some(1)).contains(&guardian), "{:?}", marked(&run, Some(1)));
    assert!(marked(&run, Some(2)).contains(&guardian), "{:?}", marked(&run, Some(2)));
    assert!(marked(&run, None).is_empty(), "the party's map marks no one's missions");
}

#[test]
fn every_teacher_but_the_archives_has_a_door_in_the_kingdom() {
    let mut r = kingdom_pair();
    // Old Carom keeps the tavern's skittle alley: a home that has dug it.
    assert!(layout::dig_tavern_reserved(&mut r.dungeon));
    let geo = geography::geography(&r.kingdom.wishes);
    let lost: Vec<String> = questline::book()
        .lines
        .iter()
        .filter(|q| {
            !geo.sites.iter().any(|s| s.room == q.kind())
                && r.gateway(q.kind(), &geo).is_none()
                && r.stair_to(q.kind(), &geo).is_none()
        })
        .map(|q| format!("{} ({:?})", q.teacher, q.kind()))
        .collect();
    // Maren's file names the Drowned Archive, which the realm reads as a
    // lair (`Lair`): no room of home is one, so no road leads to her.
    assert_eq!(lost, ["Maren of the Deepwater (Lair)"], "teachers with no way to them");
}

#[test]
#[ignore = "writes the missions' marks for visual review"]
fn write_mission_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: &ink::Img| {
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgba_scaled(3),
            im.w as u32 * 3,
            im.h as u32 * 3,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    // Friend by the millpond's door, Stilt's questline held: the pennant
    // beside the door; at the keep, the arrowheads toward the places.
    let mut r = kingdom_pair();
    take_fork(&mut r, 1);
    let door = millpond_door(&r);
    r.walk_kingdom(Some((door.0 - 3, door.1 + 3)));
    r.players.get_mut(&1).unwrap().class = Some("Archer".into());
    save("mission-kingdom-door", &arena::frame_for(&r, 384, 224, Some(1)));
    save("mission-kingdom-door-other-seat", &arena::frame_for(&r, 384, 224, Some(2)));
    let mut r = kingdom_pair();
    take_fork(&mut r, 1);
    save("mission-kingdom-keep", &arena::frame_for(&r, 384, 224, Some(1)));
    // In the millpond, over Stilt.
    let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    r.come_to_the_gate();
    take_fork(&mut r, 1);
    r.arrive(world::room_of(&r.dungeon, RoomKind::Millpond).unwrap(), (14.0, 7.0));
    save("mission-millpond", &arena::frame_for(&r, 384, 224, Some(1)));
    // A floor's map: the guardian's room in the pennant's amber.
    let mut run = Run::new(5, 17, None);
    let pack = Pack::at(2, Pack::Crypt);
    run.dungeon = layout::floor_for(fortune::Mode::LongWayDown, 2, pack, &mut run.rng);
    run.populate_boss_gates();
    run.enter(0, None);
    run.players.get_mut(&1).unwrap().class = Some("Swordsman".into());
    for room in &mut run.dungeon.rooms {
        room.visited = true;
    }
    let map = run.floor_map(Some(1));
    save("mission-floor-map", &arena::dungeon_map::overlay_of(&map).unwrap());
    // From a room beside the guardian's: its rim in the pennant's amber.
    let guardian = run.boss_gates.as_ref().unwrap().guardian;
    let beside = (0..4).find_map(|d| run.dungeon.neighbour(guardian, d)).unwrap();
    run.enter(beside, None);
    run.enemies.clear();
    save("mission-minimap-beside-guardian", &arena::dungeon_map::minimap_of(&run.floor_map(Some(1))).unwrap());
}
