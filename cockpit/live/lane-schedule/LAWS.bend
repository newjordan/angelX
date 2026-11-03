import Base
import ./lane-schedule.bend as Schedule

# These claims are about the lane decision only, and each is a concrete state:
# the checker proves them by evaluating the scheduled state, so they claim
# nothing about scheduling in general.

# A scheduler with no lanes seats nobody.
law empty_lane_set_seats_nobody:
  {Schedule.schedule(Nil{}, 1, 4, False{}, None{}) == Nil{} : List<&2, Schedule.Lane>}

# Two or more seats reserve every runnable lane, in canonical order.
law two_seats_keep_canonical_order:
  {
    Schedule.schedule(
      Con{Schedule.Lane{0, True{}, 5}, Con{Schedule.Lane{1, True{}, 2}, Nil{}}},
      2,
      4,
      False{},
      None{})
    ==
    Con{Schedule.Lane{0, True{}, 5}, Con{Schedule.Lane{1, True{}, 2}, Nil{}}}
    : List<&2, Schedule.Lane>
  }

# A lane Rust flagged not runnable never takes the single seat.
law unrunnable_lane_never_takes_the_seat:
  {
    Schedule.schedule(
      Con{Schedule.Lane{0, False{}, 0}, Con{Schedule.Lane{1, True{}, 0}, Nil{}}},
      1,
      4,
      False{},
      None{})
    ==
    Con{Schedule.Lane{1, True{}, 0}, Nil{}}
    : List<&2, Schedule.Lane>
  }

# A lane whose next age reaches the service gap takes the seat even while a
# frontier boost is pending: the deadline outranks the boost.
law a_due_lane_preempts_the_frontier_boost:
  {
    Schedule.schedule(
      Con{Schedule.Lane{0, True{}, 0}, Con{Schedule.Lane{1, True{}, 3}, Nil{}}},
      1,
      4,
      True{},
      None{})
    ==
    Con{Schedule.Lane{1, True{}, 3}, Nil{}}
    : List<&2, Schedule.Lane>
  }

# With nothing due, the boost hands the seat to the frontier lane.
law the_boost_serves_the_frontier_lane:
  {
    Schedule.schedule(
      Con{Schedule.Lane{0, True{}, 0}, Con{Schedule.Lane{1, True{}, 1}, Nil{}}},
      1,
      4,
      True{},
      None{})
    ==
    Con{Schedule.Lane{0, True{}, 0}, Nil{}}
    : List<&2, Schedule.Lane>
  }

# Equal ages with nothing due: the lane that did not serve last tick wins, and
# the single seat holds exactly one lane.
law the_lane_that_did_not_serve_last_is_preferred:
  {
    Schedule.schedule(
      Con{Schedule.Lane{0, True{}, 1}, Con{Schedule.Lane{1, True{}, 1}, Nil{}}},
      1,
      4,
      False{},
      Some{0})
    ==
    Con{Schedule.Lane{1, True{}, 1}, Nil{}}
    : List<&2, Schedule.Lane>
  }
