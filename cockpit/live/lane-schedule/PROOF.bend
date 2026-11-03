import Base
import ./LAWS.bend as Laws

# Every law above is a closed, concrete scheduling state, so the proof is the
# evaluation of that state: reflexivity after reduction.

def Laws.empty_lane_set_seats_nobody():
  {==}

def Laws.two_seats_keep_canonical_order():
  {==}

def Laws.unrunnable_lane_never_takes_the_seat():
  {==}

def Laws.a_due_lane_preempts_the_frontier_boost():
  {==}

def Laws.the_boost_serves_the_frontier_lane():
  {==}

def Laws.the_lane_that_did_not_serve_last_is_preferred():
  {==}
