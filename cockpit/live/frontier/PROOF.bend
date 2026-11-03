import Base
import ./LAWS.bend as Laws

# Each law is a closed equality between literal selections, so its proof only
# re-runs the same reduction: no arithmetic is assumed and nothing is claimed
# beyond the three boards above.

def Laws.an_empty_board_has_no_frontier():
  {==}

def Laws.a_lone_row_only_fills_the_global_seat():
  {==}

def Laws.a_missing_rank_loses_a_score_tie():
  {==}
