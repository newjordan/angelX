import Base
import ./layout.bend as Layout
import ./nums.bend as N
import ./boss.bend as Boss
import ./map.bend as Map
import ./tiles.bend as Tiles

# These claims are about the authored world and the signed arithmetic under it.
# Each is a closed equality over literals, so the checker only has to reduce it:
# nothing about the session, the saves or the wire is claimed here.

# A column left of the map is the stone wall the body is closed in by.
law the_side_of_the_map_is_stone:
  {Tiles.at(Map.rows(), N.neg(1), 10) == 1 : U32}

# Above and below the map is open sky: a knight falls out of the world.
law the_sky_is_open:
  {Tiles.at(Map.rows(), 10, Map.height()) == 0 : U32}

# A mark glyph leaves air behind it, never a tile: the courtyard door's cell.
law a_mark_leaves_air:
  {Tiles.at(Map.rows(), 174, 23) == 0 : U32}

# Truncating division and remainder, as Rust's i32 operators.
law division_truncates_toward_zero:
  {N.div(N.neg(7), 2) == N.neg(3) : U32}

law the_remainder_takes_the_dividend_sign:
  {N.rem(N.neg(7), 2) == N.neg(1) : U32}

# A degree below zero reads as a degree in the wheel's turn.
law a_negative_degree_wraps_round:
  {N.rem_euclid(N.neg(1), 360) == 359 : U32}

# Eight banners, eight ring sparks: one bit each in a byte.
law eight_bits_is_a_full_byte:
  {N.popcount(255) == 8 : U32}

# Two's complement order is signed order.
law a_negative_is_under_every_non_negative:
  {N.lt(N.neg(1), 0) == True{} : Bool}

# A round trip of the marsh log: the first half climbs, the second returns.
law a_ping_pong_starts_at_its_low_end:
  {Layout.ping_pong(0, 100, 0, 10, 20) == 10 : U32}

law a_ping_pong_turns_at_its_high_end:
  {Layout.ping_pong(50, 100, 0, 10, 20) == 20 : U32}

# Three logs and the wheel's six buckets: the world moves nine platforms.
law the_world_moves_twelve_platforms:
  {Layout.count(Layout.movers(5), 0) == 9 : U32}

law the_world_moves_twelve_platforms_later_too:
  {Layout.count(Layout.movers(500), 0) == 9 : U32}

# The first log starts four pixels inside the marsh's gap and stays over it.
law the_first_log_starts_over_its_gap:
  {Layout.first_x(Layout.movers(0)) == 2276 : U32}

# King Cinder's crown: the summit's own floor row, where he stands, and how many
# times a crown tells before he kneels.
law the_arena_floor_is_the_summit_row:
  {Boss.floor() == 352 : U32}

law the_cinder_mark_is_where_he_stands:
  {Boss.home() == 6600 : U32}

law the_first_crown_tells_twice:
  {Boss.pattern_len(0) == 2 : U32}

law the_last_crown_tells_four_times:
  {Boss.pattern_len(2) == 4 : U32}
