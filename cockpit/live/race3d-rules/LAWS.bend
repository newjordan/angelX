import Base
import ./race3d-rules.bend as Rules

law every_other_plank_is_rotten:
  {Rules.rotten(1) == True{} : Bool}

law an_even_plank_holds:
  {Rules.rotten(2) == False{} : Bool}

law the_left_cart_line_sits_off_the_middle:
  {Rules.lane_of(True{}, False{}) == (0.0 - 1.7 : F32) : F32}

law the_middle_cart_line_is_the_middle:
  {Rules.lane_of(False{}, False{}) == 0.0 : F32}

law a_null_bend_leaves_the_line:
  {Rules.finish_of(120.0, False{}) == 120.0 : F32}

law the_touge_runs_three_laps:
  {Rules.finish_of(120.0, True{}) == F32.mul(120.0, 3.0) : F32}
