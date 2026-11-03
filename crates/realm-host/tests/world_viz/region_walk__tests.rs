use super::*;
#[test]
fn authored_mine_walk_visits_all_nine_rooms_and_parks() {
    assert_eq!(room_grid(Region::TheMines), Some((3, 3)));
    assert_eq!(room_grid(Region::CastleTown), None);
    for destination in 0..9 {
        assert_eq!(station_at(destination, 0), (destination + 8) % 9);
        assert_eq!(station_at(destination, 1000), destination);
        assert_eq!(station_at(destination, u128::MAX), destination);
    }
    assert_eq!(station_at(1, 45), 0);
    assert_eq!(station_at(1, 46), 1);
}
