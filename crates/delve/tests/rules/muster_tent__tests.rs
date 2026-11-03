//! The muster tent: every recruit drinks there and nowhere else, at a seat
//! of its own a knight can walk up to and pick, and the flaps lead in from
//! the mine-head and out again.
use super::*;
use crate::rules::{alehouse, recruit_ults, world};

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

fn press(r: &mut Run) {
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    r.step(&BTreeMap::new());
}

#[test]
fn every_recruit_has_a_seat_in_the_tent_and_the_company_stays_in_the_tavern() {
    let recruits: Vec<usize> = (0..alehouse::PATRONS.len()).filter(|&i| recruit_ults::recruit(alehouse::PATRONS[i].kit)).collect();
    assert_eq!(recruits.len(), TABLES.len() * SEATS_A_TABLE, "the tent seats them all");
    let seats: Vec<(f32, f32)> = (0..TABLES.len()).flat_map(|t| (0..SEATS_A_TABLE).map(move |k| seat(t, k))).collect();
    for &i in &recruits {
        assert_eq!(alehouse::patron_room(i), RoomKind::MusterTent);
        assert!(seats.contains(&alehouse::PATRONS[i].at), "{} sits at a tent seat", alehouse::PATRONS[i].kit);
    }
    for i in (0..alehouse::PATRONS.len()).filter(|i| !recruits.contains(i)) {
        assert_eq!(alehouse::patron_room(i), RoomKind::Alehouse, "{}", alehouse::PATRONS[i].kit);
    }
    let room = room((0, 0));
    for &(x, y) in &seats {
        assert_eq!(room.tile(x as i32, y as i32), Tile::Block, "a seat is at a table");
        assert_eq!(room.tile(x as i32, (y - 1.0) as i32), Tile::Floor, "the aisle before it is clear");
    }
}

#[test]
fn walking_up_to_a_recruit_in_the_tent_picks_them_and_no_one_from_the_tavern() {
    let mut r = home();
    for i in (0..alehouse::PATRONS.len()).filter(|&i| alehouse::patron_room(i) == RoomKind::MusterTent) {
        let p = &alehouse::PATRONS[i];
        go(&mut r, RoomKind::MusterTent, (p.at.0, p.at.1 - 1.0));
        press(&mut r);
        assert_eq!(r.players[&1].knight.as_deref(), Some(p.kit), "{} is picked in the tent", p.label);
    }
    // The tavern's company is not reachable from the tent, nor the tent's
    // recruits from the tavern.
    let company = (0..alehouse::PATRONS.len()).find(|&i| alehouse::patron_room(i) == RoomKind::Alehouse).unwrap();
    let at = alehouse::PATRONS[company].at;
    assert!(alehouse::spot_near_in(RoomKind::MusterTent, at.0 * TILE_UNITS, (at.1 - 1.0) * TILE_UNITS, 0, 0)
        .is_none_or(|s| s != alehouse::Spot::Patron(company)));
    let recruit = (0..alehouse::PATRONS.len()).find(|&i| alehouse::patron_room(i) == RoomKind::MusterTent).unwrap();
    let at = alehouse::PATRONS[recruit].at;
    assert_ne!(alehouse::spot_near_in(RoomKind::Alehouse, at.0 * TILE_UNITS, (at.1 - 1.0) * TILE_UNITS, 0, 0), Some(alehouse::Spot::Patron(recruit)));
}

#[test]
fn the_flaps_lead_in_from_the_mine_head_and_out_again() {
    let r = home();
    let head = &r.dungeon.rooms[world::room_of(&r.dungeon, RoomKind::MineHead).unwrap()];
    let (c, row, _, _) = OUTER_FLAP;
    assert_eq!(head.tile(c, row), Tile::Stairs, "the tent's flap on the mine-head");
    let (pc, pr, pw, ph) = PITCH;
    assert_eq!(head.tile(pc + pw / 2, pr + ph / 2), Tile::Block, "the tent stands solid");
    assert_eq!(head.tile(OUTSIDE.0 as i32, OUTSIDE.1 as i32), Tile::Floor, "one comes out on open ground");
    let tent = &r.dungeon.rooms[world::room_of(&r.dungeon, RoomKind::MusterTent).unwrap()];
    assert_eq!(tent.tile(FLAP.0, FLAP.1), Tile::Stairs);
    assert_eq!(tent.tile(ARRIVAL.0 as i32, ARRIVAL.1 as i32), Tile::Floor);
    assert!(world::ENTRANCES.iter().any(|e| e.id == "muster-in") && world::ENTRANCES.iter().any(|e| e.id == "muster-out"));
}
