import Base
import ./layout-rules.bend as Layout

law the_stairwell_middle_is_one_inset:
  {Layout.inner(Layout.Rect{10n, 5n, 4n, 4n}) == Layout.Rect{11n, 6n, 2n, 2n} : Layout.Rect}

law a_north_door_takes_the_middle_columns:
  {
    Layout.and(
      Layout.lane_mid(24n, 11n),
      Layout.and(Layout.lane_mid(24n, 12n), Layout.negate(Layout.lane_mid(24n, 9n)))
    ) == True{} : Bool
  }

law a_guard_covers_the_tile_below_it:
  {Layout.layered(True{}, 5n, 1n) == 5n : Nat}

law the_room_edge_is_wall:
  {Layout.is_border(24n, 14n, 0n, 7n) == True{} : Bool}

law a_dig_once_tavern_is_found:
  {
    Layout.has_kind(
      Con{0n, Con{4n, Nil{}}}, Layout.kind_tavern()
    ) == True{} : Bool
  }
