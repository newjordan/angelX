import Base
import ./lanternward-rules.bend as E
import ./g.bend as G
import ./gd.bend as Gd
import ./gr.bend as Gr

# These claims are about numbers and tiles alone: no session, no players, no
# clock. Each one is a closed equality between the result and the literal the
# Rust ground answers, so the proof is the computation and nothing else.

# The sine is Q at a quarter turn, -Q at three quarters, and zero at none.
law a_quarter_turn_is_q:
  {G.sin_q(90) == 256 : U32}

law three_quarter_turns_are_minus_q:
  {G.sin_q(270) == 4294967040 : U32}

law east_has_no_sine:
  {G.sin_q(0) == 0 : U32}

law a_cosine_starts_at_q:
  {G.cos_q(0) == 256 : U32}

# The turn is the short way round, signed.
law the_turn_wraps_forward:
  {G.turn(350, 10) == 20 : U32}

law the_turn_wraps_backward:
  {G.turn(10, 350) == 4294967276 : U32}

# A tile is sixteen pixels, and the middle of the first one is half of that.
law a_tile_follows_the_floor_shift:
  {G.tile_of(4096) == 1 : U32}

law the_first_tiles_middle_is_half_a_tile:
  {G.middle(0) == 2048 : U32}

# A knight is on the floor at the middle of the first tile that is not a wall, in
# the Stair; the wall past its edge is a wall; its way on is the way on.
law the_stair_corner_is_a_wall:
  {Gd.at(Gd.of(0), 0, 0) == 1 : U32}

law the_stair_is_floor_inside:
  {Gd.at(Gd.of(0), 3, 3) == 0 : U32}

law past_the_stair_is_a_wall:
  {Gd.at(Gd.of(0), 22, 3) == 1 : U32}

law the_stair_leaves_by_the_way_on:
  {Gd.at(Gd.of(0), 9, 16) == 4 : U32}

law the_stair_has_a_pit:
  {Gd.at(Gd.of(0), 15, 3) == 3 : U32}

law the_stair_is_twenty_two_tiles_wide:
  {Gd.width(Gd.of(0)) == 22 : U32}

law the_stair_is_eighteen_tiles_deep:
  {Gd.height(Gd.of(0)) == 18 : U32}
