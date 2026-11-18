import Base
import ./lane-schedule.bend as Schedule

law empty_lane_set_seats_nobody:
  {Schedule.schedule(Nil{}, 1, 4, False{}, None{}) == Nil{} : List<&2, Schedule.Lane>}

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
