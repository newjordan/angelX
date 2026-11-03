import Base
import ./race3d-rules.bend as Rules

# Every other plank in a collapsing drift is rotten: the odd ones, counted from
# the start of the course. A ball that reaches one after ROT begins to go.
law every_other_plank_is_rotten:
  {Rules.rotten(1) == True{} : Bool}

law an_even_plank_holds:
  {Rules.rotten(2) == False{} : Bool}

# The Ore Cart runs three lines across the gallery: the left one sits 1.7 tiles
# left of the middle, the middle one on it.
law the_left_cart_line_sits_off_the_middle:
  {Rules.lane_of(True{}, False{}) == (0.0 - 1.7 : F32) : F32}

law the_middle_cart_line_is_the_middle:
  {Rules.lane_of(False{}, False{}) == 0.0 : F32}

# Seven hundredths of a tile a tick is the only thing the sluice adds to a
# ball still on the Sorter's flat.
law a_null_bend_leaves_the_line:
  {Rules.finish_of(120.0, False{}) == 120.0 : F32}

law the_touge_runs_three_laps:
  {Rules.finish_of(120.0, True{}) == F32.mul(120.0, 3.0) : F32}
