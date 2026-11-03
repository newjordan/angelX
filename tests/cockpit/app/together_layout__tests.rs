//! The Undercroft's and Maud's tavern's rules moved into `layout-rules.bend`.
//! The native generator that laid them stays in `angel_delve::rules::layout::
//! oracle` (compiled for tests only) as the parity oracle: the cockpit runs the
//! Bend leg and the oracle side by side over every pack the home floor can be
//! entered from, comparing the whole floor — tiles, kinds, doors, cells and the
//! save shape — every time.

use angel_delve::rules::Pack;
use angel_delve::rules::layout::{self, COLS, Floor, ROWS, RoomKind, Tile, oracle};

fn shape(floor: &Floor) -> serde_json::Value {
    serde_json::to_value(floor).expect("a floor is a save shape")
}

fn engine() {
    crate::agent::harness::live_policy::install_scripted_rules();
}

fn packs() -> [Pack; 7] {
    [
        Pack::Crypt,
        Pack::Cavern,
        Pack::Hellforge,
        Pack::Archive,
        Pack::Fungal,
        Pack::Unknown,
        Pack::Swamp,
    ]
}

#[test]
fn the_undercroft_bend_leg_lays_the_native_floor() {
    engine();
    for pack in packs() {
        let native = oracle::undercroft(pack);
        let bend = layout::undercroft(pack);
        assert_eq!(shape(&native), shape(&bend), "undercroft for {pack:?}");
    }
}

#[test]
fn the_tavern_dig_matches_the_native_dig() {
    engine();
    for pack in packs() {
        let mut native = oracle::undercroft(pack);
        let mut bend = layout::undercroft(pack);
        assert_eq!(
            oracle::dig_tavern(&mut native),
            layout::dig_tavern(&mut bend)
        );
        assert_eq!(shape(&native), shape(&bend), "tavern dig for {pack:?}");
        // The wing is dug once: a second dig is refused by both legs.
        assert_eq!(
            oracle::dig_tavern(&mut native),
            layout::dig_tavern(&mut bend)
        );
        assert_eq!(shape(&native), shape(&bend), "second dig for {pack:?}");
    }
}

#[test]
fn the_reserved_footprint_matches_the_native_footprint() {
    engine();
    let mut native = oracle::undercroft(Pack::Crypt);
    let mut bend = layout::undercroft(Pack::Crypt);
    assert_eq!(
        oracle::dig_tavern_reserved(&mut native),
        layout::dig_tavern_reserved(&mut bend)
    );
    assert_eq!(shape(&native), shape(&bend));
}

#[test]
fn a_floor_with_no_trophy_hall_digs_nothing_either_way() {
    engine();
    let mut native = oracle::undercroft(Pack::Crypt);
    let mut bend = layout::undercroft(Pack::Crypt);
    for floor in [&mut native, &mut bend] {
        for room in &mut floor.rooms {
            if room.kind == RoomKind::Trophies {
                room.kind = RoomKind::Fight;
            }
        }
    }
    assert!(!oracle::dig_tavern(&mut native));
    assert!(!layout::dig_tavern(&mut bend));
    assert_eq!(shape(&native), shape(&bend));
}

#[test]
fn a_deeper_floor_digs_nothing_either_way() {
    engine();
    let mut native = oracle::undercroft(Pack::Fungal);
    let mut bend = layout::undercroft(Pack::Fungal);
    native.depth = 2;
    bend.depth = 2;
    assert!(!oracle::dig_tavern(&mut native));
    assert!(!layout::dig_tavern(&mut bend));
    assert_eq!(shape(&native), shape(&bend));
}

#[test]
fn the_bend_rooms_are_rooms_a_save_would_keep() {
    engine();
    let floor = layout::undercroft(Pack::Cavern);
    assert_eq!(floor.rooms.len(), 4);
    assert_eq!(floor.rooms[0].tile(11, 6), Tile::Stairs);
    assert_eq!(floor.rooms[0].tile(0, 0), Tile::Wall);
    assert_eq!(floor.rooms[0].tile(11, 0), Tile::Door);
    for room in &floor.rooms {
        assert_eq!((room.cols, room.rows), (COLS, ROWS));
    }
    let mut dug = layout::undercroft(Pack::Cavern);
    assert!(layout::dig_tavern(&mut dug));
    assert_eq!(dug.rooms.len(), 7);
}
