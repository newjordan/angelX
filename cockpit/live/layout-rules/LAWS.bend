import Base
import ./layout-rules.bend as Layout

# Honest small laws about the layout leg's building blocks. Full parity against
# the native generator is the cockpit test suite's job; these are the equalities
# the checker can settle on its own.

# The middle of the Winding Stair's parapet, which becomes its stairs.
law the_stairwell_middle_is_one_inset:
  {Layout.inner(Layout.Rect{10n, 5n, 4n, 4n}) == Layout.Rect{11n, 6n, 2n, 2n} : Layout.Rect}

# A door's own two tiles stand in the middle columns of a 24 wide room.
law a_north_door_takes_the_middle_columns:
  {
    Layout.and(
      Layout.lane_mid(24n, 11n),
      Layout.and(Layout.lane_mid(24n, 12n), Layout.negate(Layout.lane_mid(24n, 9n)))
    ) == True{} : Bool
  }

# The paint order holds: a guard's tile covers whatever lies under it.
law a_guard_covers_the_tile_below_it:
  {Layout.layered(True{}, 5n, 1n) == 5n : Nat}

# The room's own edge is wall, the inside is not.
law the_room_edge_is_wall:
  {Layout.is_border(24n, 14n, 0n, 7n) == True{} : Bool}

# A tavern standing once is a tavern the crew does not dig again.
law a_dig_once_tavern_is_found:
  {
    Layout.has_kind(
      Con{0n, Con{4n, Nil{}}}, Layout.kind_tavern()
    ) == True{} : Bool
  }
