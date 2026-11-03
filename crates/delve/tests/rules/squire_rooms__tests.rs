//! The squires' rooms off the Training Yard: a door in the Yard to each, a
//! way back from each, and each squire game answering in its own room.
use super::*;
use crate::rules::world;

trait NotTheRoom { fn is_ok_or_not_the_room(&self) -> bool; }
impl NotTheRoom for Result<String, String> {
    fn is_ok_or_not_the_room(&self) -> bool { self.as_ref().err().is_none_or(|e| !e.contains("Visit the Academy")) }
}

fn home() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_alehouse();
    r
}

fn go(r: &mut Run, kind: RoomKind, at: (f32, f32)) {
    r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS);
}

#[test]
fn the_yard_has_a_door_to_each_room_and_each_room_a_way_back() {
    let r = home();
    let yard = &r.dungeon.rooms[world::room_of(&r.dungeon, RoomKind::Yard).unwrap()];
    for (i, (kind, _, _)) in ROOMS.iter().enumerate() {
        let (c, row, _, _) = YARD_DOORS[i];
        assert_eq!(yard.tile(c, row), Tile::Stairs, "{kind:?}: its door in the Yard");
        let into = world::entrance_at(RoomKind::Yard, c, row).expect("the door is an entrance");
        assert!(matches!(into.to, world::Dest::Room(k, _) if k == *kind), "{kind:?}: the door leads in");
        let room = &r.dungeon.rooms[world::room_of(&r.dungeon, *kind).unwrap()];
        assert_eq!(room.tile(BACK.0, BACK.1), Tile::Stairs, "{kind:?}: the way back");
        assert_eq!(room.tile(ARRIVAL.0 as i32, ARRIVAL.1 as i32), Tile::Floor, "{kind:?}: one arrives on open floor");
        let back = world::entrance_at(*kind, BACK.0, BACK.1).expect("the way back is an entrance");
        assert!(matches!(back.to, world::Dest::Room(RoomKind::Yard, at) if yard.tile(at.0 as i32, at.1 as i32) == Tile::Floor),
            "{kind:?}: back to open floor in the Yard");
    }
}

#[test]
fn walking_through_a_door_and_back() {
    for (i, (kind, _, _)) in ROOMS.iter().enumerate() {
        let mut r = home();
        go(&mut r, RoomKind::Yard, yard_front(i));
        let into = world::entrance_at(RoomKind::Yard, YARD_DOORS[i].0, YARD_DOORS[i].1).unwrap();
        r.take_entrance(into);
        assert_eq!(r.room().kind, *kind);
        let back = world::entrance_at(*kind, BACK.0, BACK.1).unwrap();
        r.take_entrance(back);
        assert_eq!(r.room().kind, RoomKind::Yard);
    }
}

#[test]
fn each_squire_game_answers_in_its_own_room_and_not_in_the_yard() {
    let mut r = home();
    // The Academy's actions are its room's.
    go(&mut r, RoomKind::Yard, (12.0, 6.0));
    use crate::rules::squire_academy::Action;
    assert!(r.academy_squire_action(1, Action::Hire(0)).unwrap_err().contains("Academy"), "no Academy business in the Yard");
    go(&mut r, RoomKind::SquireAcademy, ARRIVAL);
    assert!(r.academy_squire_action(1, Action::Hire(0)).is_ok_or_not_the_room(), "the Academy answers in its room");
    // The Guild's menu opens on its plate in its room, not on the same spot
    // in the Yard.
    let (c, row, _, _) = crate::rules::squire_guild::PLATES[0];
    for (kind, opens) in [(RoomKind::Yard, false), (RoomKind::SquireGuild, true)] {
        go(&mut r, kind, (c as f32 + 0.5, row as f32 + 0.5));
        r.step(&BTreeMap::new());
        r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
        r.step(&BTreeMap::new());
        assert_eq!(r.squire_guild.menus.contains_key(&1), opens, "{kind:?}");
        r.squire_guild.menus.clear();
    }
}
