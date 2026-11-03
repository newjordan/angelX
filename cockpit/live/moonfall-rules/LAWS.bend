import Base
import ./moonfall-rules.bend as Moonfall

# Claims about Moonfall's clock and its smallest verbs, fixed to concrete numbers: a
# symbolic multiplication or subtraction would leave `U32.mul` and `U32.sub` stuck in
# the checker, so each claim pins the numbers it talks about.

# A loop is three days of `day` ticks; at the default day of 3000 it ends at 9000.
law the_loop_is_three_days:
  {Moonfall.c_end(3000) == 9000 : U32}

# The Final Hours begin at the Third Midnight, the 66th hour: tick 8250 of 9000.
law the_final_hours_begin_at_the_third_midnight:
  {Moonfall.c_final(3000) == 8250 : U32}

# The loop begins at six in the morning, and noon is half a day on.
law the_loop_begins_at_dawn:
  {Moonfall.c_hour(0, 3000) == 6 : U32}

law noon_is_half_a_day_on:
  {Moonfall.c_hour(750, 3000) == 12 : U32}

# The bridge is down by day and raised by night.
law the_bridge_is_down_at_noon:
  {Moonfall.c_bridge_up(750, 3000) == False{} : Bool}

law the_bridge_is_up_at_midnight:
  {Moonfall.c_bridge_up(2250, 3000) == True{} : Bool}

# Timers only run down: nothing is taken from zero.
law a_timer_never_runs_below_zero:
  {Moonfall.sat(0, 1) == 0 : U32}

law a_timer_runs_down_by_what_is_taken:
  {Moonfall.sat(10, 3) == 7 : U32}

# The bell winds time on to the next dusk (half a day from dawn) or the next dawn.
law the_next_dusk_after_dawn_is_half_a_day_on:
  {Moonfall.c_next_dusk(0, 3000) == 1500 : U32}

law the_next_dawn_after_noon_ends_the_day:
  {Moonfall.c_next_dawn(750, 3000) == 3000 : U32}
