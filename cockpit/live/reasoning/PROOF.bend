import Base
import ./LAWS.bend as Laws

def Laws.zero_limit_never_demotes(levels, cumulative, submitted, locked):
  match submitted:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.zero_budget_never_expires(elapsed):
  {==}

def Laws.an_empty_seat_never_gets_an_effort(effort, friction):
  {==}
